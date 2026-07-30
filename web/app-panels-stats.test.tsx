import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Plan } from "./api/types";
import {
  activeMemory,
  agentTeamSnapshot,
  aiStatistics,
  aiStatisticsDetail,
  appTestState,
  changeInput,
  chatSummary,
  defaultComposerPlaceholder,
  type Deferred,
  chatMemory,
  chatMessages,
  chatStatistics,
  contextUsage,
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
  planFixture,
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
  beforeEach(() => {
    // Prior tests may install fake timers; always restore real timers first.
    vi.useRealTimers();
    resetAppTestEnvironment();
  });

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

  function fetchCallUrls() {
    return vi.mocked(fetch).mock.calls.map((call) => {
      const rawPath =
        typeof call[0] === "string"
          ? call[0]
          : call[0] instanceof URL
            ? call[0].toString()
            : call[0].url;

      return new URL(rawPath, "http://localhost");
    });
  }

  function configureRemoteSessionStatistics() {
    const workspaceId = "workspace-remote";
    const chatId = "remote-chat-1";
    const title = "Remote metrics";
    const remoteChat = chatSummary(
      chatId,
      title,
      "2026-07-12T08:00:00Z",
      "2026-07-12T08:05:00Z",
    );
    const remoteWorkspace = {
      ...secondaryWorkspace,
      chatPagination: { hasMore: false, limit: 5, nextCursor: null, total: 1 },
      chats: [remoteChat],
      connectionStatus: "ready",
      displayPath: "dev-box:/home/fonla/repos/remote-project",
      id: workspaceId,
      name: "Remote project",
      path: "dev-box:/home/fonla/repos/remote-project",
      remotePath: "/home/fonla/repos/remote-project",
      serverId: "server-1",
      serverName: "dev-box",
    };
    const chatKey = `${workspaceId}/${chatId}`;
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [
        ...appTestState.settingsResponse.workspaces,
        {
          commonCommands: remoteWorkspace.commonCommands,
          connectionStatus: remoteWorkspace.connectionStatus,
          displayPath: remoteWorkspace.displayPath,
          id: remoteWorkspace.id,
          isDefault: false,
          lastRemoteError: remoteWorkspace.lastRemoteError,
          logoUrl: remoteWorkspace.logoUrl,
          name: remoteWorkspace.name,
          path: remoteWorkspace.path,
          pinned: remoteWorkspace.pinned,
          remotePath: remoteWorkspace.remotePath,
          serverId: remoteWorkspace.serverId,
          serverName: remoteWorkspace.serverName,
          terminalShell: remoteWorkspace.terminalShell,
        },
      ],
    };
    appTestState.chatMessagesResponsesByChatKey = {
      [chatKey]: {
        ...chatMessages,
        chat: { ...chatMessages.chat, id: chatId, title },
      },
    };
    appTestState.chatStatisticsResponsesByChatKey = {
      [chatKey]: {
        ...chatStatistics,
        chatId,
        compression: {
          ...chatStatistics.compression,
          llmSnapshotCount: 2,
          runtimeToolStateSnapshotCount: 9,
          savedTokenCount: 7654,
        },
        modelBreakdown: [{ modelId: "remote-gpt", requestCount: 3, totalTokens: 23456 }],
        providerBreakdown: [
          {
            averageLatencyMs: 3200,
            failedCount: 0,
            providerId: "remote-provider",
            requestCount: 3,
            successCount: 3,
            successRate: 1,
            totalTokens: 23456,
          },
        ],
        toolBreakdown: [{ callCount: 4, toolName: "read_file" }],
        workspaceId,
      },
    };

    return { chatId, remoteWorkspace, title, workspaceId };
  }

  it("defaults Source Control diff to the active isolated coordinator worktree", async () => {
    const worktreePath = `${workspace.path}\\.foco\\agent-worktrees\\agent-instance-coordinator`;
    appTestState.agentTeamSnapshotResponse = {
      ...agentTeamSnapshot,
      instances: agentTeamSnapshot.instances.map((instance) =>
        instance.id === agentTeamSnapshot.team.coordinatorInstanceId
          ? {
              ...instance,
              executionRootPath: worktreePath,
              executionWorkspaceMode: "isolated_worktree",
              worktreeBaseRevision: "base-revision",
              worktreeBranch: "foco/agent-worktrees/agent-instance-coordinator",
              worktreeStatus: "active",
            }
          : instance,
      ),
    };
    const branchesResponse = {
      branches: ["main", "foco/agent-worktrees/agent-instance-coordinator"],
      currentBranch: "main",
      isGitRepository: true,
      worktrees: [
        {
          branch: "main",
          isCurrent: true,
          name: "workspace",
          path: workspace.path,
        },
        {
          branch: "foco/agent-worktrees/agent-instance-coordinator",
          isCurrent: false,
          name: "agent-instance-coordinator",
          path: worktreePath,
        },
      ],
    };
    appTestState.workspaceGitBranchesResponses = [
      branchesResponse,
      branchesResponse,
    ];
    appTestState.workspaceGitDiffResponsesByWorktreePath[worktreePath] =
      generatedGitDiff;

    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Git" }));
    const selectedTarget = (await screen.findAllByText(
      "foco/agent-worktrees/agent-instance-coordinator",
    )).find((element) => element.dataset.slot === "select-value");
    if (!selectedTarget) {
      throw new Error("Source Control target value is missing");
    }
    const targetSelect = selectedTarget.closest("button");
    if (!targetSelect) {
      throw new Error("Source Control target trigger is missing");
    }
    expect(targetSelect).toHaveAttribute("aria-label", "Source Control target");
    expect(
      screen.queryByRole("heading", {
        name: "foco/agent-worktrees/agent-instance-coordinator",
      }),
    ).toBeNull();
    expect(
      screen.queryByRole("heading", { name: "Workspace changes" }),
    ).toBeNull();
    await userEvent.click(targetSelect);
    expect(
      await screen.findByRole("option", {
        name: "foco/agent-worktrees/agent-instance-coordinator",
      }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(
        fetchCallUrls().some(
          (url) =>
            url.pathname === "/api/workspaces/workspace-1/git/diff" &&
            url.searchParams.get("worktreePath") === worktreePath,
        ),
      ).toBe(true),
    );
  });

  it("switches Source Control targets without calling branch switch", async () => {
    const firstWorktreePath = `${workspace.path}\\.foco\\agent-worktrees\\agent-instance-coordinator`;
    const secondWorktreePath = `${workspace.path}\\.foco\\agent-worktrees\\agent-instance-review`;
    appTestState.agentTeamSnapshotResponse = {
      ...agentTeamSnapshot,
      instances: agentTeamSnapshot.instances.map((instance) =>
        instance.id === agentTeamSnapshot.team.coordinatorInstanceId
          ? {
              ...instance,
              executionRootPath: firstWorktreePath,
              executionWorkspaceMode: "isolated_worktree",
              worktreeBaseRevision: "base-revision",
              worktreeBranch: "foco/agent-worktrees/agent-instance-coordinator",
              worktreeStatus: "active",
            }
          : instance,
      ),
    };
    const branchesResponse = {
      branches: [
        "main",
        "foco/agent-worktrees/agent-instance-coordinator",
        "foco/agent-worktrees/agent-instance-review",
      ],
      currentBranch: "main",
      isGitRepository: true,
      worktrees: [
        {
          branch: "main",
          isCurrent: true,
          name: "workspace",
          path: workspace.path,
        },
        {
          branch: "foco/agent-worktrees/agent-instance-coordinator",
          isCurrent: false,
          name: "agent-instance-coordinator",
          path: firstWorktreePath,
        },
        {
          branch: "foco/agent-worktrees/agent-instance-review",
          isCurrent: false,
          name: "agent-instance-review",
          path: secondWorktreePath,
        },
      ],
    };
    appTestState.workspaceGitBranchesResponses = [
      branchesResponse,
      branchesResponse,
    ];
    appTestState.workspaceGitDiffResponsesByWorktreePath[firstWorktreePath] =
      generatedGitDiff;
    appTestState.workspaceGitDiffResponsesByWorktreePath[secondWorktreePath] = {
      ...generatedGitDiff,
      diff: [
        "diff --git a/review.md b/review.md",
        "--- /dev/null",
        "+++ b/review.md",
        "@@ -0,0 +1 @@",
        "+review",
        "",
      ].join("\n"),
      files: [{ indexStatus: " ", path: "review.md", worktreeStatus: "M" }],
      status: " M review.md\n",
    };

    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Git" }));
    const selectedTarget = (await screen.findAllByText(
      "foco/agent-worktrees/agent-instance-coordinator",
    )).find((element) => element.dataset.slot === "select-value");
    if (!selectedTarget) {
      throw new Error("Source Control target value is missing");
    }
    const targetSelect = selectedTarget.closest("button");
    if (!targetSelect) {
      throw new Error("Source Control target trigger is missing");
    }
    expect(targetSelect).toHaveAttribute("aria-label", "Source Control target");
    expect(
      screen.queryByRole("heading", {
        name: "foco/agent-worktrees/agent-instance-coordinator",
      }),
    ).toBeNull();
    await userEvent.click(targetSelect);
    await userEvent.click(
      await screen.findByRole("option", {
        name: "foco/agent-worktrees/agent-instance-review",
      }),
    );

    await screen.findByText("review.md");
    expect(targetSelect).toHaveTextContent(
      "foco/agent-worktrees/agent-instance-review",
    );
    expect(
      fetchCallUrls().some(
        (url) =>
          url.pathname === "/api/workspaces/workspace-1/git/branches/switch",
      ),
    ).toBe(false);
    expect(
      fetchCallUrls().some(
        (url) =>
          url.pathname === "/api/workspaces/workspace-1/git/diff" &&
          url.searchParams.get("worktreePath") === secondWorktreePath,
      ),
    ).toBe(true);
  });

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
    const streamCall = vi.mocked(fetch).mock.calls.findLast(([input]) => {
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

    const appRow = await screen.findByRole("button", {
      name: /web\/App\.tsx M/,
    });
    const changesButton = screen.getByRole("button", { name: /Changes/ });
    const appFileName = within(appRow).getByText("App.tsx");
    const appDirectory = within(appRow).getByText("web");

    expect(changesButton).toHaveClass("button--ghost");
    expect(appRow).toHaveClass("button--ghost");
    expect(appFileName.compareDocumentPosition(appDirectory)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(appFileName).toHaveClass("text-[var(--foreground)]");
    expect(appDirectory).toHaveClass("text-[var(--muted)]");
    expect(within(appRow).queryByText("web/App.tsx")).not.toBeInTheDocument();
  });

  it("toggles the context panel and opens the terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(
      screen.getByRole("button", { name: "Close context panel" }),
    );
    expect(screen.queryByRole("tab", { name: "ToDo" })).not.toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Open context panel" }),
    );
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(
      await screen.findAllByRole("button", { name: /README\.md M/ }),
    ).toHaveLength(2);
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(
      screen.getAllByRole("button", { name: /README\.md M/ })[0],
    );

    expect((await screen.findAllByText(/hello world/))[0]).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: /new-note\.txt U/ }),
    ).toHaveLength(2);

    await userEvent.click(
      screen.getByRole("button", { name: "Open terminal" }),
    );

    expect(await screen.findByLabelText("connected")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/terminal/session",
      expect.objectContaining({ method: "POST" }),
    );

    const newTerminalButton = screen.getByRole("button", { name: "New terminal" });
    expect(newTerminalButton.closest(".terminal-panel")).toHaveAttribute(
      "data-theme",
      "dark",
    );
    const terminalCloseButtons = screen.getAllByRole("button", {
      name: "Close terminal",
    });
    expect(newTerminalButton).toHaveClass("button--ghost", "button--icon-only");
    expect(terminalCloseButtons[1]).toHaveClass(
      "button--ghost",
      "button--icon-only",
    );
    await userEvent.click(newTerminalButton);

    const terminalList = await screen.findByRole("complementary", {
      name: "Terminal sessions",
    });
    expect(within(terminalList).getByText("Terminal 1")).toBeInTheDocument();
    expect(within(terminalList).getByText("Terminal 2")).toBeInTheDocument();
    expect(within(terminalList).getAllByLabelText("connected")).toHaveLength(2);
    expect(
      within(terminalList).getAllByText(workspace.path)[0],
    ).toHaveAttribute("title", workspace.path);
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
    expect(
      within(terminalList).queryByText("Terminal 2"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("complementary", { name: "Terminal sessions" }),
    ).not.toBeInTheDocument();

    await userEvent.click(
      screen.getAllByRole("button", { name: "Close terminal" })[1],
    );

    await waitFor(() => {
      expect(screen.queryByLabelText("connected")).not.toBeInTheDocument();
    });
  }, 10000);

  it("runs a remote plan from start through pause and resume without losing its implementation chat", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T04:30:00Z";
    const { chatId: remoteChatId, remoteWorkspace, workspaceId } =
      configureRemoteSessionStatistics();
    const phaseStep = {
      acceptance: ["Start queues a remote Coordinator chat."],
      checkedAt: null,
      createdAt: timestamp,
      detail:
        "The remote workspace chat list shows the created implementation session.",
      id: "plan-step-1",
      phaseId: "plan-phase-1",
      planId: "plan-1",
      sequence: 0,
      status: "pending",
      title: "Wire start action",
      updatedAt: timestamp,
    };
    const pendingPhase = {
      agentTaskId: null,
      agentTeamId: null,
      commitId: null,
      completedAt: null,
      createdAt: timestamp,
      errorMessage: null,
      id: "plan-phase-1",
      implementationChatId: null,
      mergeAttemptCount: 0,
      planId: "plan-1",
      sequence: 0,
      startedAt: null,
      status: "pending",
      steps: [phaseStep],
      summary: "Use the existing chat runtime.",
      title: "Phase 1",
      updatedAt: timestamp,
    };
    const readyPlan = {
      activePhaseId: null,
      completedAt: null,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      sharedMergeCommitId: null,
      id: "plan-1",
      overview: "Run the remote implementation through normal visible chats.",
      pauseRequestedAt: null,
      phases: [pendingPhase],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "ready",
      title: "Build remote plan runner UI",
      updatedAt: timestamp,
    };
    let planState: "ready" | "running" | "paused" = "ready";
    const actionRequests: string[] = [];
    const currentPlan = () => {
      if (planState === "ready") {
        return readyPlan;
      }

      return {
        ...readyPlan,
        activePhaseId: "plan-phase-1",
        pauseRequestedAt: planState === "paused" ? timestamp : null,
        phases: [
          {
            ...pendingPhase,
            agentTaskId: "agent-task-plan-1",
            agentTeamId: "agent-team-plan-1",
            implementationChatId: "plan-chat-1",
            startedAt: timestamp,
            status: "running",
          },
        ],
        status: planState,
      };
    };
    const planChat = chatSummary(
      "plan-chat-1",
      "Plan phase implementation",
      timestamp,
      timestamp,
      { additions: 0, deletions: 0 },
      {
        chatId: "plan-chat-1",
        lastSequence: 0,
        runId: "agent-task-plan-1",
        workspaceId,
      },
    );
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = new URL(url, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspaceId,
            workspaces: [
              workspace,
              {
                ...remoteWorkspace,
                chats:
                  planState === "ready"
                    ? remoteWorkspace.chats
                    : [planChat, ...remoteWorkspace.chats],
              },
            ],
          });
        }

        if (path === `/api/workspaces/${workspaceId}/plans`) {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [currentPlan()],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (path === `/api/workspaces/${workspaceId}/plans/plan-1/action`) {
          const request = JSON.parse(String(init?.body ?? "{}")) as {
            action?: string;
          };
          actionRequests.push(request.action ?? "");
          if (request.action === "start" || request.action === "resume") {
            planState = "running";
          } else if (request.action === "pause") {
            planState = "paused";
          }
          return jsonResponse({ plan: currentPlan() });
        }

        if (path === `/api/workspaces/${workspaceId}/chats/plan-chat-1/messages`) {
          return jsonResponse({
            activeRun: {
              chatId: "plan-chat-1",
              lastSequence: 0,
              runId: "agent-task-plan-1",
              workspaceId,
            },
            chat: {
              id: "plan-chat-1",
              kind: null,
              readOnly: false,
              title: "Plan phase implementation",
            },
            messages: [
              {
                content: "Plan phase implementation request.",
                createdAt: timestamp,
                extractedMemories: [],
                id: "plan-message-user",
                memoriesUsed: [],
                metrics: null,
                parts: [
                  { text: "Plan phase implementation request.", type: "text" },
                ],
                reasoning: null,
                role: "user",
                specUpdates: [],
                toolCalls: [],
              },
            ],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", `/${workspaceId}/${remoteChatId}`);

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Build remote plan runner UI")).toBeInTheDocument();
    expect(screen.queryByText("Wire start action")).not.toBeInTheDocument();

    const phaseButton = (await screen.findByText("Phase 1")).closest("button");
    if (!phaseButton) {
      throw new Error("Expected phase row button");
    }
    await user.click(phaseButton);

    expect(await screen.findByText("Wire start action")).toBeInTheDocument();
    expect(
      screen.getByText("Start queues a remote Coordinator chat."),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        `/api/workspaces/${workspaceId}/plans/plan-1/action`,
        expect.objectContaining({
          body: JSON.stringify({ action: "start" }),
          method: "POST",
        }),
      );
    });
    expect(
      await screen.findByText("Plan phase implementation request."),
    ).toBeInTheDocument();

    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Pause" }));
    expect(await screen.findByRole("button", { name: "Resume" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Resume" }));
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
    expect(actionRequests).toEqual(["start", "pause", "resume"]);
    expect(screen.queryByText(/already running/i)).not.toBeInTheDocument();
  });

  it("opens an existing implementation chat from a plan phase", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T06:00:00Z";
    const implementationChat = chatSummary(
      "plan-chat-open",
      "Existing implementation chat",
      timestamp,
      timestamp,
    );
    const plan = {
      activePhaseId: null,
      completedAt: null,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      sharedMergeCommitId: null,
      id: "plan-open-chat",
      overview: "Open the phase implementation transcript.",
      pauseRequestedAt: null,
      phases: [
        {
          agentTaskId: "agent-task-open",
          agentTeamId: "agent-team-open",
          commitId: null,
          completedAt: null,
          createdAt: timestamp,
          errorMessage: null,
          id: "plan-phase-open",
          implementationChatId: "plan-chat-open",
          mergeAttemptCount: 0,
          planId: "plan-open-chat",
          sequence: 0,
          startedAt: timestamp,
          status: "running",
          steps: [],
          summary: "Existing chat is available.",
          title: "Open chat phase",
          updatedAt: timestamp,
        },
      ],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "running",
      title: "Open implementation chat plan",
      updatedAt: timestamp,
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = new URL(url, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspace.id,
            workspaces: [
              { ...workspace, chats: [implementationChat, ...workspace.chats] },
              secondaryWorkspace,
            ],
          });
        }

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [plan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chats/plan-chat-open/messages"
        ) {
          return jsonResponse({
            activeRun: null,
            chat: {
              id: "plan-chat-open",
              kind: null,
              readOnly: false,
              title: "Existing implementation chat",
            },
            messages: [
              {
                content: "Existing implementation transcript.",
                createdAt: timestamp,
                extractedMemories: [],
                id: "plan-open-message-user",
                memoriesUsed: [],
                metrics: null,
                parts: [
                  { text: "Existing implementation transcript.", type: "text" },
                ],
                reasoning: null,
                role: "user",
                specUpdates: [],
                toolCalls: [],
              },
            ],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(
      await screen.findByText("Open implementation chat plan"),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Open implementation chat" }),
    );

    expect(
      await screen.findByText("Existing implementation transcript."),
    ).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/chats/plan-chat-open/messages?limit=60",
      expect.any(Object),
    );
  });

  it("distinguishes an unbound dispatch reservation from running and retryable plan phases", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-30T01:00:00Z";
    const attempt = (
      phaseId: string,
      status: string,
      overrides: Record<string, unknown> = {},
    ) => ({
      agentTaskId: null,
      agentTeamId: null,
      commitId: null,
      completedAt: null,
      createdAt: timestamp,
      errorMessage: null,
      id: `attempt-${phaseId}`,
      implementationChatId: null,
      modelId: null,
      phaseId,
      planId: "plan-dispatch-presentation",
      providerId: null,
      sequence: 0,
      startedAt: null,
      status,
      thinkingLevel: null,
      trigger: "initial",
      updatedAt: timestamp,
      ...overrides,
    });
    const phase = (
      id: string,
      title: string,
      overrides: Record<string, unknown> = {},
    ) => ({
      agentTaskId: null,
      agentTeamId: null,
      attempts: [],
      commitId: null,
      completedAt: null,
      createdAt: timestamp,
      errorMessage: null,
      id,
      implementationChatId: null,
      mergeAttemptCount: 0,
      planId: "plan-dispatch-presentation",
      sequence: 0,
      startedAt: null,
      status: "running",
      steps: [],
      summary: "Plan dispatch presentation coverage.",
      title,
      updatedAt: timestamp,
      ...overrides,
    });
    const plan = {
      activePhaseId: "phase-preparing",
      completedAt: null,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      id: "plan-dispatch-presentation",
      overview: "Render reservation state without inventing a chat link.",
      pauseRequestedAt: null,
      phases: [
        phase("phase-preparing", "Preparing phase", {
          attempts: [attempt("phase-preparing", "queued")],
        }),
        phase("phase-bound", "Bound phase", {
          agentTaskId: "agent-task-bound",
          agentTeamId: "agent-team-bound",
          attempts: [
            attempt("phase-bound", "running", {
              agentTaskId: "agent-task-bound",
              agentTeamId: "agent-team-bound",
              implementationChatId: "implementation-chat-bound",
              startedAt: timestamp,
            }),
            attempt("phase-bound-merge", "queued", {
              id: "attempt-phase-bound-merge",
              phaseId: "phase-bound",
              sequence: 1,
              trigger: "merge_auto",
            }),
          ],
          implementationChatId: "implementation-chat-bound",
          startedAt: timestamp,
        }),
        phase("phase-timed-out", "Timed out phase", {
          attempts: [
            attempt("phase-timed-out", "failed", {
              completedAt: timestamp,
              errorMessage: "plan_phase_dispatch_timed_out",
            }),
          ],
          errorMessage: "plan_phase_dispatch_timed_out",
          status: "failed",
        }),
        // Legacy plan history did not always include attempt rows. It must keep
        // rendering the persisted phase status rather than assuming a reservation.
        phase("phase-legacy", "Legacy phase", { attempts: undefined }),
        phase("phase-legacy-retry", "Legacy retry phase", {
          attempts: undefined,
          status: "failed",
        }),
      ],
      sharedMergeCommitId: null,
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "running",
      title: "Plan dispatch presentation",
      updatedAt: timestamp,
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        if (
          new URL(url, "http://127.0.0.1").pathname ===
          "/api/workspaces/workspace-1/plans"
        ) {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [plan],
            totalCount: 1,
            totalPages: 1,
          });
        }
        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Plan dispatch presentation")).toBeInTheDocument();

    const preparingPhase = screen.getByText("Preparing phase").closest("section");
    const boundPhase = screen.getByText("Bound phase").closest("section");
    const timedOutPhase = screen.getByText("Timed out phase").closest("section");
    const legacyPhase = screen.getByText("Legacy phase").closest("section");
    const legacyRetryPhase = screen
      .getByText("Legacy retry phase")
      .closest("section");
    if (
      !preparingPhase ||
      !boundPhase ||
      !timedOutPhase ||
      !legacyPhase ||
      !legacyRetryPhase
    ) {
      throw new Error("Expected all plan phase presentation rows");
    }

    expect(within(preparingPhase).getByText("Preparing session")).toBeInTheDocument();
    expect(
      within(preparingPhase).queryByRole("button", {
        name: "Open implementation chat",
      }),
    ).not.toBeInTheDocument();
    expect(within(boundPhase).getByText("Running")).toBeInTheDocument();
    expect(
      within(boundPhase).getByRole("button", {
        name: "Open implementation chat",
      }),
    ).toBeInTheDocument();
    expect(within(timedOutPhase).getByText("Failed")).toBeInTheDocument();
    expect(
      within(timedOutPhase).getByRole("button", { name: "Retry plan phase" }),
    ).toBeInTheDocument();
    expect(within(legacyPhase).getByText("Running")).toBeInTheDocument();

    await user.click(within(timedOutPhase).getByText("Timed out phase"));
    expect(
      await within(timedOutPhase).findByText("plan_phase_dispatch_timed_out"),
    ).toBeInTheDocument();

    await user.click(
      within(legacyRetryPhase).getByRole("button", {
        name: "Retry phase options",
      }),
    );
    await user.click(
      await screen.findByRole("menuitem", {
        name: "Retry with another model…",
      }),
    );
    expect(
      screen.getByRole("dialog", { name: "Retry with another model" }),
    ).toBeInTheDocument();
  });

  it("confirms plan deletion and skips the request when cancelled", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T06:15:00Z";
    const confirmSpy = vi
      .spyOn(window, "confirm")
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);
    let deleted = false;
    const plan = {
      activePhaseId: null,
      completedAt: null,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      sharedMergeCommitId: null,
      id: "plan-delete-ui",
      overview: "Delete this plan from the active list.",
      pauseRequestedAt: null,
      phases: [],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "ready",
      title: "Delete me from plan panel",
      updatedAt: timestamp,
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = new URL(url, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: deleted ? [] : [plan],
            totalCount: deleted ? 0 : 1,
            totalPages: 1,
          });
        }

        if (path === "/api/workspaces/workspace-1/plans/plan-delete-ui") {
          deleted = true;
          return jsonResponse({ deleted: true });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(
      await screen.findByText("Delete me from plan panel"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete plan" }));
    expect(confirmSpy).toHaveBeenCalledWith("Delete plan confirmation");
    expect(
      fetchMock.mock.calls.filter(([input, init]) => {
        const path = new URL(String(input), "http://127.0.0.1").pathname;
        return (
          path === "/api/workspaces/workspace-1/plans/plan-delete-ui" &&
          init?.method === "DELETE"
        );
      }),
    ).toHaveLength(0);
    expect(screen.getByText("Delete me from plan panel")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete plan" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(([input, init]) => {
          const path = new URL(String(input), "http://127.0.0.1").pathname;
          return (
            path === "/api/workspaces/workspace-1/plans/plan-delete-ui" &&
            init?.method === "DELETE"
          );
        }),
      ).toHaveLength(1);
    });
    await waitFor(() => {
      expect(
        screen.queryByText("Delete me from plan panel"),
      ).not.toBeInTheDocument();
    });
    expect(
      screen.getByText("No active plans for this workspace."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Locate running or failed plan" }),
    ).toBeDisabled();
  });

  it("hides Resume behind Retry when the earliest incomplete phase is cancelled", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-10T12:00:00Z";
    const cancelledPlan: Plan = {
      ...planFixture,
      activePhaseId: null,
      errorMessage: "user cancelled phase one",
      id: "plan-cancelled-barrier-ui",
      pauseRequestedAt: timestamp,
      status: "paused",
      title: "Cancelled phase barrier",
      phases: [
        {
          ...planFixture.phases[0],
          errorMessage: "user cancelled phase one",
          id: "phase-cancelled-barrier-ui",
          planId: "plan-cancelled-barrier-ui",
          status: "cancelled",
          title: "Cancelled first phase",
          steps: planFixture.phases[0].steps.map((step) => ({
            ...step,
            phaseId: "phase-cancelled-barrier-ui",
            planId: "plan-cancelled-barrier-ui",
            status: "cancelled",
          })),
        },
        {
          ...planFixture.phases[0],
          id: "phase-later-pending-ui",
          planId: "plan-cancelled-barrier-ui",
          sequence: 1,
          status: "pending",
          title: "Later pending phase",
          steps: planFixture.phases[0].steps.map((step) => ({
            ...step,
            id: "step-later-pending-ui",
            phaseId: "phase-later-pending-ui",
            planId: "plan-cancelled-barrier-ui",
          })),
        },
      ],
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const path = new URL(String(input), "http://127.0.0.1").pathname;
        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [cancelledPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }
        if (path === "/api/workspaces/workspace-1/plans/auto-run") {
          return jsonResponse({ busy: false, enabled: false });
        }
        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Cancelled phase barrier")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Resume" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry plan phase" })).toBeInTheDocument();
    expect(
      screen.getByText((_, element) =>
        element?.textContent ===
        "Cancelled first phase: Retry the cancelled phase to continue this plan.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("user cancelled phase one")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", {
        name: "Auto run plansRun every active plan in order",
      }),
    ).not.toBeChecked();
  });
  it.each(["failed", "cancelled"] as const)(
    "retries a %s plan phase through the phase retry endpoint",
    async (phaseStatus) => {
      const user = userEvent.setup();
      const timestamp = "2026-06-28T05:00:00Z";
      const failedStep = {
        acceptance: ["Retry uses the phase retry endpoint."],
        checkedAt: null,
        createdAt: timestamp,
        detail:
          "The Plan runner should see the same task complete after retry.",
        id: "plan-step-failed",
        phaseId: "plan-phase-failed",
        planId: "plan-failed",
        sequence: 0,
        status: "failed",
        title: "Recover failed phase",
        updatedAt: timestamp,
      };
      const failedPhase = {
        agentTaskId: null,
        agentTeamId: "agent-team-failed",
        attempts: [
          {
            agentTaskId: "agent-task-failed",
            agentTeamId: "agent-team-failed",
            commitId: null,
            completedAt: timestamp,
            createdAt: timestamp,
            errorMessage: "provider rate limited",
            id: "plan-phase-attempt-failed",
            implementationChatId: null,
            modelId: "gpt-test",
            phaseId: "plan-phase-failed",
            planId: "plan-failed",
            providerId: "openai",
            sequence: 0,
            startedAt: timestamp,
            status: phaseStatus,
            thinkingLevel: null,
            trigger: "start",
            updatedAt: timestamp,
          },
        ],
        commitId: null,
        completedAt: timestamp,
        createdAt: timestamp,
        errorMessage: "provider rate limited",
        id: "plan-phase-failed",
        implementationChatId: null,
        mergeAttemptCount: 0,
        planId: "plan-failed",
        sequence: 0,
        startedAt: timestamp,
        status: phaseStatus,
        steps: [failedStep],
        summary: "The model request failed.",
        title: "Failed phase",
        updatedAt: timestamp,
      };
      const failedPlan = {
        activePhaseId: null,
        completedAt: timestamp,
        completedByUserAt: null,
        createdAt: timestamp,
        errorMessage: "provider rate limited",
        sharedMergeCommitId: null,
        id: "plan-failed",
        overview: "Expose an explicit phase retry control.",
        pauseRequestedAt: null,
        phases: [failedPhase],
        sortOrder: 0,
        sourceChatId: "chat-1",
        status: "failed",
        title: "Retry failed plan phase",
        updatedAt: timestamp,
      };
      const retriedPlan = {
        ...failedPlan,
        activePhaseId: "plan-phase-failed",
        completedAt: null,
        errorMessage: null,
        phases: [
          {
            ...failedPhase,
            agentTaskId: "agent-task-retried",
            implementationChatId: "chat-retried",
            completedAt: null,
            errorMessage: null,
            status: "running",
          },
        ],
        status: "running",
      };
      const implementedPlan = {
        ...retriedPlan,
        activePhaseId: null,
        completedAt: timestamp,
        phases: [
          {
            ...retriedPlan.phases[0],
            completedAt: timestamp,
            status: "completed",
            steps: [
              {
                ...failedStep,
                checkedAt: timestamp,
                status: "completed",
              },
            ],
          },
        ],
        status: "implemented",
      };
      let didRetry = false;
      let planRequestCount = 0;
      const fetchMock = vi.fn(
        async (input: RequestInfo | URL, init?: RequestInit) => {
          const url = typeof input === "string" ? input : input.toString();
          const path = url.startsWith("http://127.0.0.1")
            ? new URL(url).pathname
            : url.split("?")[0];

          if (path === "/api/workspaces/workspace-1/plans") {
            planRequestCount += 1;
            return jsonResponse({
              page: 1,
              pageSize: 50,
              plans: [
                !didRetry
                  ? failedPlan
                  : planRequestCount >= 3
                    ? implementedPlan
                    : retriedPlan,
              ],
              totalCount: 1,
              totalPages: 1,
            });
          }

          if (
            path ===
            "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry"
          ) {
            didRetry = true;
            return jsonResponse({ plan: retriedPlan });
          }

          return mockFetch(input, init);
        },
      );
      vi.stubGlobal("fetch", fetchMock);
      window.history.replaceState(null, "", "/workspace-1/chat-1");

      renderApp();

      await user.click(await screen.findByRole("tab", { name: "Plan" }));
      expect(
        await screen.findByText("Retry failed plan phase"),
      ).toBeInTheDocument();

      const retryButton = screen.getByRole("button", {
        name: "Retry plan phase",
      });
      const retryOptionsButton = screen.getByRole("button", {
        name: "Retry phase options",
      });
      const retryButtonGroup = retryButton.closest('[data-slot="button-group"]');
      expect(retryButtonGroup).not.toBeNull();
      if (!retryButtonGroup) {
        throw new Error("Retry controls must be wrapped by a ButtonGroup.");
      }
      expect(
        retryOptionsButton.closest('[data-slot="button-group"]'),
      ).toBe(retryButtonGroup);
      expect(
        retryButtonGroup.querySelector('[data-slot="button-group-separator"]'),
      ).toBeInTheDocument();

      await user.click(retryButton);

      await waitFor(() => {
        expect(fetchMock).toHaveBeenCalledWith(
          "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry",
          expect.objectContaining({
            body: JSON.stringify({}),
            method: "POST",
          }),
        );
      });
      expect((await screen.findAllByText("Running")).length).toBeGreaterThan(0);
      await waitFor(
        () => {
          expect(screen.getByText("Implemented")).toBeInTheDocument();
        },
        { timeout: 5000 },
      );
    },
  );

  it("opens phase retry model override dialog", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T05:00:00Z";
    const failedPhase = {
      agentTaskId: "agent-task-failed",
      agentTeamId: "agent-team-failed",
      attempts: [
        {
          agentTaskId: "agent-task-failed",
          agentTeamId: "agent-team-failed",
          commitId: null,
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: "provider rate limited",
          id: "plan-phase-attempt-failed",
          implementationChatId: null,
          modelId: "gpt-test",
          phaseId: "plan-phase-failed",
          planId: "plan-failed",
          providerId: "openai",
          sequence: 0,
          startedAt: timestamp,
          status: "failed",
          thinkingLevel: null,
          trigger: "start",
          updatedAt: timestamp,
        },
      ],
      commitId: null,
      completedAt: timestamp,
      createdAt: timestamp,
      errorMessage: "provider rate limited",
      id: "plan-phase-failed",
      implementationChatId: null,
      mergeAttemptCount: 0,
      planId: "plan-failed",
      sequence: 0,
      startedAt: timestamp,
      status: "failed",
      steps: [],
      summary: "The model request failed.",
      title: "Failed phase",
      updatedAt: timestamp,
    };
    const failedPlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: "provider rate limited",
      sharedMergeCommitId: null,
      id: "plan-failed",
      overview: "Expose an explicit phase retry control.",
      pauseRequestedAt: null,
      phases: [failedPhase],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "failed",
      title: "Retry failed plan phase",
      updatedAt: timestamp,
    };
    const retriedPlan = {
      ...failedPlan,
      activePhaseId: "plan-phase-failed",
      completedAt: null,
      errorMessage: null,
      phases: [{ ...failedPhase, status: "running", errorMessage: null }],
      status: "running",
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [failedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (
          path ===
          "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry"
        ) {
          return jsonResponse({ plan: retriedPlan });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    await user.click(
      screen.getByRole("button", { name: "Retry phase options" }),
    );
    await user.click(
      screen.getByRole("menuitem", { name: "Retry with another model…" }),
    );

    expect(
      screen.getByRole("dialog", { name: "Retry with another model" }),
    ).toBeInTheDocument();
    // Provider is derived from the selected model's active route.
    expect(screen.queryByLabelText("Provider")).toBeNull();
    expect(
      screen.getByRole("button", {
        name: /Model default.*Thinking level/i,
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Thinking level/ }));
    await user.click(screen.getByRole("option", { name: /high/i }));
    await user.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry",
        expect.objectContaining({
          body: JSON.stringify({
            modelId: "gpt-test",
            providerId: "openai",
            thinkingLevel: "high",
          }),
          method: "POST",
        }),
      );
    });
  });

  it("shows the auto-run checkbox when the plan list is empty", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [],
            totalCount: 0,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Plan" }));

    expect(
      await screen.findByRole("checkbox", { name: /Auto run plans/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Run every active plan in order"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("No active plans for this workspace."),
    ).toBeInTheDocument();
  });

  it("scrolls the running plan into view when the Plan tab opens", async () => {
    const readyPlan = {
      ...planFixture,
      activePhaseId: null,
      id: "plan-ready-scroll",
      sortOrder: 0,
      status: "ready" as const,
      title: "Ready scroll decoy",
    };
    const runningPlan = {
      ...planFixture,
      activePhaseId: "phase-running-scroll",
      id: "plan-running-scroll",
      phases: [
        {
          ...planFixture.phases[0],
          id: "phase-running-scroll",
          planId: "plan-running-scroll",
          status: "running" as const,
        },
      ],
      sortOrder: 1,
      status: "running" as const,
      title: "Running scroll target",
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [readyPlan, runningPlan],
            totalCount: 2,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    const rect = (top: number, height: number) =>
      ({
        bottom: top + height,
        height,
        left: 0,
        right: 320,
        top,
        width: 320,
        x: 0,
        y: top,
        toJSON: () => ({}),
      }) as DOMRect;
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(
      function (this: HTMLElement) {
        return this.classList.contains("context-list-panel") ? 200 : 0;
      },
    );
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function (this: HTMLElement) {
        if (this.classList.contains("context-list-panel")) {
          return rect(0, 200);
        }
        if (this.textContent?.includes("Ready scroll decoy")) {
          return rect(0, 100);
        }
        if (this.textContent?.includes("Running scroll target")) {
          return rect(500, 100);
        }
        return rect(0, 0);
      },
    );

    renderApp();

    const planTab = await screen.findByRole("tab", { name: "Plan" });
    const scrollIntoView = vi.mocked(HTMLElement.prototype.scrollIntoView);
    scrollIntoView.mockClear();

    await userEvent.click(planTab);

    const runningTitle = await screen.findByText("Running scroll target");
    const planListPanel = runningTitle.closest(
      ".context-list-panel",
    ) as HTMLElement | null;
    expect(planListPanel).not.toBeNull();
    await waitFor(() => {
      expect(planListPanel?.scrollTop).toBe(450);
    });
    planListPanel!.scrollTop = 0;
    await userEvent.click(
      screen.getByRole("button", { name: "Locate running or failed plan" }),
    );
    expect(planListPanel?.scrollTop).toBe(450);
    expect(screen.getByText("Ready scroll decoy")).toBeInTheDocument();
    expect(
      scrollIntoView.mock.contexts.some(
        (context) =>
          context instanceof HTMLElement &&
          context.textContent?.includes("Running scroll target"),
      ),
    ).toBe(false);
  });

  it("locates a failed plan when no plan is running", async () => {
    const failedPlan: Plan = {
      ...planFixture,
      activePhaseId: null,
      id: "plan-failed-scroll",
      sortOrder: 0,
      status: "failed",
      title: "Failed scroll target",
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [failedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    const rect = (top: number, height: number) =>
      ({
        bottom: top + height,
        height,
        left: 0,
        right: 320,
        top,
        width: 320,
        x: 0,
        y: top,
        toJSON: () => ({}),
      }) as DOMRect;
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(
      function (this: HTMLElement) {
        return this.classList.contains("context-list-panel") ? 200 : 0;
      },
    );
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function (this: HTMLElement) {
        if (this.classList.contains("context-list-panel")) {
          return rect(0, 200);
        }
        if (this.textContent?.includes("Failed scroll target")) {
          return rect(500, 100);
        }
        return rect(0, 0);
      },
    );

    renderApp();
    await userEvent.click(await screen.findByRole("tab", { name: "Plan" }));

    const failedTitle = await screen.findByText("Failed scroll target");
    const planListPanel = failedTitle.closest(
      ".context-list-panel",
    ) as HTMLElement | null;
    expect(planListPanel).not.toBeNull();
    const locateButton = screen.getByRole("button", {
      name: "Locate running or failed plan",
    });
    expect(locateButton).toBeEnabled();
    await waitFor(() => {
      expect(planListPanel?.scrollTop).toBe(450);
    });

    planListPanel!.scrollTop = 0;
    await userEvent.click(locateButton);
    expect(planListPanel?.scrollTop).toBe(450);
  });

  it("prefers the running plan over a failed plan when locating", async () => {
    const failedPlan: Plan = {
      ...planFixture,
      activePhaseId: null,
      id: "plan-failed-decoy",
      sortOrder: 0,
      status: "failed",
      title: "Failed scroll decoy",
    };
    const runningPlan: Plan = {
      ...planFixture,
      id: "plan-running-target",
      sortOrder: 1,
      status: "running",
      title: "Running priority target",
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [failedPlan, runningPlan],
            totalCount: 2,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    const rect = (top: number, height: number) =>
      ({
        bottom: top + height,
        height,
        left: 0,
        right: 320,
        top,
        width: 320,
        x: 0,
        y: top,
        toJSON: () => ({}),
      }) as DOMRect;
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(
      function (this: HTMLElement) {
        return this.classList.contains("context-list-panel") ? 200 : 0;
      },
    );
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function (this: HTMLElement) {
        if (this.classList.contains("context-list-panel")) {
          return rect(0, 200);
        }
        if (this.textContent?.includes("Failed scroll decoy")) {
          return rect(300, 100);
        }
        if (this.textContent?.includes("Running priority target")) {
          return rect(700, 100);
        }
        return rect(0, 0);
      },
    );

    renderApp();
    await userEvent.click(await screen.findByRole("tab", { name: "Plan" }));

    const runningTitle = await screen.findByText("Running priority target");
    const planListPanel = runningTitle.closest(
      ".context-list-panel",
    ) as HTMLElement | null;
    expect(planListPanel).not.toBeNull();
    expect(screen.getByText("Failed scroll decoy")).toBeInTheDocument();
    await waitFor(() => {
      expect(planListPanel?.scrollTop).toBe(650);
    });

    planListPanel!.scrollTop = 0;
    await userEvent.click(
      screen.getByRole("button", { name: "Locate running or failed plan" }),
    );
    expect(planListPanel?.scrollTop).toBe(650);
  });

  it("hides the Plan locate label only under the phone breakpoint", () => {
    const stylesCss = readFileSync("styles.css", "utf8");

    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?\.plan-locate-button-label\s*\{[\s\S]*?clip:\s*rect\(0,\s*0,\s*0,\s*0\)/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?\.plan-locate-button\s*\{[\s\S]*?width:\s*2rem/,
    );
    // Label must remain visible outside the phone breakpoint (no global hide).
    expect(stylesCss).not.toMatch(
      /^\.plan-locate-button-label\s*\{[\s\S]*?clip:\s*rect/m,
    );
  });

  it("loads and toggles backend plan auto-run state", async () => {
    const user = userEvent.setup();
    const autoRunRequests: Array<{ enabled?: boolean; method: string }> = [];
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans/auto-run") {
          const body = init?.body
            ? (JSON.parse(String(init.body)) as { enabled?: boolean })
            : {};
          autoRunRequests.push({
            enabled: body.enabled,
            method: init?.method ?? "GET",
          });
          return jsonResponse({
            busy: body.enabled ?? false,
            enabled: body.enabled ?? false,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    const autoRunCheckbox = await screen.findByRole("checkbox", {
      name: /Auto run plans/,
    });
    expect(autoRunCheckbox).not.toBeChecked();

    await user.click(autoRunCheckbox);

    await waitFor(() => {
      expect(autoRunRequests).toContainEqual({ enabled: true, method: "PUT" });
    });
    expect(autoRunCheckbox).toBeChecked();
    expect(screen.queryByText("Auto running")).not.toBeInTheDocument();
  });

  it("refreshes active plans when backend auto-run becomes busy", async () => {
    const user = userEvent.setup();
    const readyPlan: Plan = {
      ...planFixture,
      activePhaseId: null,
      phases: planFixture.phases.map((phase) => ({
        ...phase,
        status: "pending",
      })),
      status: "ready",
      title: "Auto-run refresh target",
    };
    const runningPlan: Plan = {
      ...readyPlan,
      activePhaseId: readyPlan.phases[0]?.id ?? null,
      phases: readyPlan.phases.map((phase, index) => ({
        ...phase,
        status: index === 0 ? "running" : phase.status,
      })),
      status: "running",
    };
    let planRequestCount = 0;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans/auto-run") {
          const body = init?.body
            ? (JSON.parse(String(init.body)) as { enabled?: boolean })
            : {};
          return jsonResponse({
            busy: body.enabled ?? false,
            enabled: body.enabled ?? false,
          });
        }

        if (path === "/api/workspaces/workspace-1/plans") {
          planRequestCount += 1;
          const plan = planRequestCount === 1 ? readyPlan : runningPlan;
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [plan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(
      await screen.findByText("Auto-run refresh target"),
    ).toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: /Auto run plans/ }));

    await waitFor(() => {
      expect(planRequestCount).toBeGreaterThanOrEqual(2);
    });
    const planArticle = screen
      .getByText("Auto-run refresh target")
      .closest("article");
    if (!planArticle) {
      throw new Error("Expected plan article");
    }
    expect(within(planArticle).getAllByText("Running").length).toBeGreaterThan(
      0,
    );
  });

  it("single-flights plan and auto-run polling while auto-run is enabled", async () => {
    const user = userEvent.setup();
    const intervalCallbacks = new Map<
      number,
      { handler: TimerHandler; timeout?: number }
    >();
    let nextIntervalId = 0;
    let nowMs = 0;
    let holdPlanRequests = false;
    const heldPlanResponses: Array<Deferred<Response>> = [];
    const runningPlan = { ...planFixture, status: "running" };
    const plansResponse = () =>
      jsonResponse({
        page: 1,
        pageSize: 50,
        plans: [runningPlan],
        totalCount: 1,
        totalPages: 1,
      });
    const performanceNowSpy = vi
      .spyOn(performance, "now")
      .mockImplementation(() => nowMs);
    const setIntervalSpy = vi.spyOn(window, "setInterval").mockImplementation(((
      handler: TimerHandler,
      timeout?: number,
    ) => {
      nextIntervalId += 1;
      intervalCallbacks.set(nextIntervalId, { handler, timeout });
      return nextIntervalId;
    }) as typeof window.setInterval);
    const clearIntervalSpy = vi
      .spyOn(window, "clearInterval")
      .mockImplementation((id) => {
        intervalCallbacks.delete(Number(id));
      });
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans/auto-run") {
          return jsonResponse({ busy: false, enabled: true });
        }

        if (path === "/api/workspaces/workspace-1/plans") {
          if (holdPlanRequests) {
            const response = deferred<Response>();
            heldPlanResponses.push(response);
            return response.promise;
          }
          return plansResponse();
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();

      await user.click(await screen.findByRole("tab", { name: "Plan" }));
      expect(await screen.findByText(runningPlan.title)).toBeInTheDocument();
      await waitFor(() => {
        expect(
          Array.from(intervalCallbacks.values()).filter(
            ({ timeout }) => timeout === 3000,
          ),
        ).toHaveLength(1);
      });

      fetchMock.mockClear();
      holdPlanRequests = true;
      nowMs = 1000;
      const poll = Array.from(intervalCallbacks.values()).find(
        (item) => item.timeout === 3000,
      )?.handler;
      if (typeof poll !== "function") {
        throw new Error("Expected plan polling interval");
      }
      await act(async () => {
        poll();
        poll();
      });

      const requestCount = (pathname: string) =>
        fetchMock.mock.calls.filter(([input]) => {
          const rawUrl = typeof input === "string" ? input : input.toString();
          return new URL(rawUrl, "http://127.0.0.1").pathname === pathname;
        }).length;

      expect(heldPlanResponses).toHaveLength(1);
      expect(requestCount("/api/workspaces/workspace-1/plans")).toBe(1);
      expect(requestCount("/api/workspaces/workspace-1/plans/auto-run")).toBe(
        1,
      );

      await act(async () => {
        heldPlanResponses[0].resolve(plansResponse());
      });
      await waitFor(() => {
        expect(heldPlanResponses).toHaveLength(2);
      });
      expect(requestCount("/api/workspaces/workspace-1/plans")).toBe(2);

      await act(async () => {
        heldPlanResponses[1].resolve(plansResponse());
      });
    } finally {
      performanceNowSpy.mockRestore();
      setIntervalSpy.mockRestore();
      clearIntervalSpy.mockRestore();
    }
  });

  it("keeps plan auto-run state scoped to the active workspace", async () => {
    const user = userEvent.setup();
    const autoRunEnabledByWorkspace: Record<string, boolean> = {
      "workspace-1": true,
      "workspace-2": false,
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;
        const match = path.match(
          /^\/api\/workspaces\/([^/]+)\/plans\/auto-run$/,
        );

        if (match) {
          const workspaceId = decodeURIComponent(match[1] ?? "");
          if (init?.method === "PUT") {
            const body = JSON.parse(String(init.body ?? "{}")) as {
              enabled?: boolean;
            };
            autoRunEnabledByWorkspace[workspaceId] = body.enabled ?? false;
          }
          return jsonResponse({
            busy: false,
            enabled: autoRunEnabledByWorkspace[workspaceId] ?? false,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    const autoRunCheckbox = await screen.findByRole("checkbox", {
      name: /Auto run plans/,
    });
    await waitFor(() => {
      expect(autoRunCheckbox).toBeChecked();
    });

    await user.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") &&
          accessibleName.startsWith("Side project"),
      }),
    );
    await user.click(screen.getByRole("button", { name: /Side note/ }));

    await waitFor(() => {
      expect(autoRunCheckbox).not.toBeChecked();
    });

    await user.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") &&
          accessibleName.startsWith("Default"),
      }),
    );
    await user.click(screen.getByRole("tab", { name: /Tool run/ }));

    await waitFor(() => {
      expect(autoRunCheckbox).toBeChecked();
    });
  });

  it("refreshes running plan phases while the visible plan panel is open", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T06:00:00Z";
    const phase1 = {
      ...planFixture.phases[0],
      agentTaskId: "task-phase-1",
      agentTeamId: "team-phase-1",
      startedAt: timestamp,
      status: "running",
      title: "Phase 1",
      updatedAt: timestamp,
    };
    const phase2 = {
      ...planFixture.phases[0],
      agentTaskId: null,
      agentTeamId: null,
      id: "phase-2",
      planId: planFixture.id,
      sequence: 1,
      startedAt: null,
      status: "pending",
      steps: planFixture.phases[0].steps.map((step) => ({
        ...step,
        id: "step-2",
        phaseId: "phase-2",
        status: "pending",
        title: "Open next settings view",
      })),
      title: "Phase 2",
      updatedAt: timestamp,
    };
    const runningPhase1Plan = {
      ...planFixture,
      activePhaseId: "phase-1",
      phases: [phase1, phase2],
      status: "running",
      title: "Plan panel polling regression",
      updatedAt: timestamp,
    };
    const runningPhase2Plan = {
      ...runningPhase1Plan,
      activePhaseId: "phase-2",
      phases: [
        {
          ...phase1,
          completedAt: timestamp,
          commitId: "abc1234",
          status: "completed",
          steps: phase1.steps.map((step) => ({
            ...step,
            checkedAt: timestamp,
            status: "completed",
          })),
        },
        {
          ...phase2,
          agentTaskId: "task-phase-2",
          agentTeamId: "team-phase-2",
          startedAt: timestamp,
          status: "running",
        },
      ],
    };
    let planStage = 0;
    const waitForPlanPoll = () =>
      act(async () => {
        await new Promise((resolve) => window.setTimeout(resolve, 3100));
      });
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans") {
          const plan = planStage === 0 ? runningPhase1Plan : runningPhase2Plan;
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [plan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await screen.findAllByText("Default");
    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(
      await screen.findByRole("checkbox", { name: /Auto run plans/ }),
    ).not.toBeChecked();
    expect(
      await screen.findByText("Plan panel polling regression"),
    ).toBeInTheDocument();
    const phase1Section = screen.getByText("Phase 1").closest("section");
    if (!phase1Section) {
      throw new Error("Expected phase 1 section");
    }
    expect(within(phase1Section).getByText("Running")).toBeInTheDocument();

    planStage = 1;
    await waitForPlanPoll();

    await waitFor(() => {
      const updatedPhase1Section = screen
        .getByText("Phase 1")
        .closest("section");
      const updatedPhase2Section = screen
        .getByText("Phase 2")
        .closest("section");
      if (!updatedPhase1Section || !updatedPhase2Section) {
        throw new Error("Expected refreshed phase sections");
      }
      expect(
        within(updatedPhase1Section).getByText("Completed"),
      ).toBeInTheDocument();
      expect(
        within(updatedPhase2Section).getByText("Running"),
      ).toBeInTheDocument();
    });
  });

  it("reorders active plans without frontend auto-running the new first plan", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-02T11:00:00Z";
    const makePlan = (id: string, title: string, sortOrder: number) => ({
      ...planFixture,
      activePhaseId: null,
      id,
      overview: `${title} overview`,
      phases: planFixture.phases.map((phase) => ({
        ...phase,
        id: `${id}-phase-1`,
        planId: id,
        steps: phase.steps.map((step) => ({
          ...step,
          id: `${id}-step-1`,
          phaseId: `${id}-phase-1`,
          planId: id,
          title: `${title} step`,
        })),
        title: `${title} phase`,
      })),
      sortOrder,
      status: "ready" as const,
      title,
      updatedAt: timestamp,
    });
    const firstPlan = makePlan("plan-1", "First queue plan", 0);
    const secondPlan = makePlan("plan-2", "Second queue plan", 1);
    let plans: Plan[] = [firstPlan, secondPlan];
    const orderRequests: Array<{ planIds: string[] }> = [];
    const autoRunRequests: Array<{ enabled?: boolean; method: string }> = [];
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const rawUrl = typeof input === "string" ? input : input.toString();
        const path = new URL(rawUrl, "http://127.0.0.1").pathname;

        if (path === "/api/workspaces/workspace-1/plans/auto-run") {
          const body = init?.body
            ? (JSON.parse(String(init.body)) as { enabled?: boolean })
            : {};
          autoRunRequests.push({
            enabled: body.enabled,
            method: init?.method ?? "GET",
          });
          return jsonResponse({ busy: false, enabled: body.enabled ?? false });
        }

        if (path === "/api/workspaces/workspace-1/plans/order") {
          const body = JSON.parse(String(init?.body ?? "{}")) as {
            planIds: string[];
          };
          orderRequests.push(body);
          plans = body.planIds.map((planId, index) => ({
            ...(plans.find((plan) => plan.id === planId) ?? firstPlan),
            sortOrder: index,
          }));
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans,
            totalCount: plans.length,
            totalPages: 1,
          });
        }

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans,
            totalCount: plans.length,
            totalPages: 1,
          });
        }

        const actionMatch = path.match(
          /^\/api\/workspaces\/workspace-1\/plans\/([^/]+)\/action$/,
        );
        if (actionMatch) {
          const planId = decodeURIComponent(actionMatch[1] ?? "");
          const plan =
            plans.find((candidate) => candidate.id === planId) ?? secondPlan;
          return jsonResponse({ plan });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await screen.findAllByText("Default");
    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    const firstTitle = await screen.findByText("First queue plan");
    const secondTitle = await screen.findByText("Second queue plan");
    expect(firstTitle.compareDocumentPosition(secondTitle)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );

    const dragData = {
      dropEffect: "move",
      effectAllowed: "move",
      getData: vi.fn(() => "plan-2"),
      setData: vi.fn(),
    };
    const reorderHandles = screen.getAllByRole("button", {
      name: "Reorder plan",
    });
    const firstArticle = firstTitle.closest("article");
    if (!firstArticle) {
      throw new Error("Expected first plan article");
    }
    fireEvent.dragStart(reorderHandles[1], { dataTransfer: dragData });
    fireEvent.dragOver(firstArticle, { dataTransfer: dragData });
    fireEvent.drop(firstArticle, { dataTransfer: dragData });
    fireEvent.dragEnd(reorderHandles[1], { dataTransfer: dragData });

    await waitFor(() => {
      expect(orderRequests).toEqual([{ planIds: ["plan-2", "plan-1"] }]);
    });
    await waitFor(() => {
      expect(
        screen
          .getByText("Second queue plan")
          .compareDocumentPosition(screen.getByText("First queue plan")),
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });

    await user.click(
      await screen.findByRole("checkbox", { name: /Auto run plans/ }),
    );
    await waitFor(() => {
      expect(autoRunRequests).toContainEqual({ enabled: true, method: "PUT" });
    });
    expect(fetchMock).not.toHaveBeenCalledWith(
      expect.stringMatching(
        /^\/api\/workspaces\/workspace-1\/plans\/[^/]+\/action$/,
      ),
      expect.anything(),
    );
  });

  it("shows retry merge for dirty merge blocked plans and refreshes after retry", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-02T10:00:00Z";
    const blockedMessage =
      "cannot merge Agent worktree while shared workspace has uncommitted changes";
    const blockedPlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: blockedMessage,
      sharedMergeCommitId: null,
      id: "plan-merge-blocked",
      overview: "Retry the existing plan worktree merge.",
      pauseRequestedAt: null,
      phases: [],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "blocked",
      title: "Blocked merge plan",
      updatedAt: timestamp,
    };
    const mergedPlan = {
      ...blockedPlan,
      errorMessage: null,
      sharedMergeCommitId: "1234567890abcdef",
      updatedAt: "2026-07-02T10:01:00Z",
    };
    let didRetryMerge = false;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [didRetryMerge ? mergedPlan : blockedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (
          path === "/api/workspaces/workspace-1/plans/plan-merge-blocked/action"
        ) {
          didRetryMerge = true;
          return jsonResponse({ plan: mergedPlan });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await user.click(screen.getByRole("tab", { name: "Plan" }));

    const planCard = (await screen.findByText("Blocked merge plan")).closest(
      "article",
    );
    expect(planCard).not.toBeNull();
    const retryButton = within(planCard as HTMLElement).getByRole("button", {
      name: "Retry Merge",
    });
    expect(retryButton).toHaveAttribute(
      "aria-describedby",
      "plan-merge-retry-hint-plan-merge-blocked",
    );
    expect(
      within(planCard as HTMLElement).getByText(
        "Clean the shared workspace, then retry merge",
      ),
    ).toHaveClass("sr-only");
    expect(
      within(planCard as HTMLElement).getByText(blockedMessage),
    ).toBeInTheDocument();

    await user.click(retryButton);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/plans/plan-merge-blocked/action",
        expect.objectContaining({
          body: JSON.stringify({ action: "retry_merge" }),
          method: "POST",
        }),
      );
    });
    expect(await screen.findByText("1234567")).toHaveAttribute(
      "aria-describedby",
      "plan-merge-status-plan-merge-blocked",
    );
    expect(
      within(planCard as HTMLElement).getByText("Merged into shared workspace"),
    ).toHaveClass("sr-only");
    expect(
      screen.queryByRole("button", { name: "Retry Merge" }),
    ).not.toBeInTheDocument();
  });

  it("shows active merge status, opens its merge chat, and switches to the shared commit after refresh", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-23T12:00:00Z";
    const mergeChat = chatSummary(
      "plan-chat-active-merge",
      "Active merge coordinator chat",
      timestamp,
      timestamp,
    );
    const completedPhase = {
      agentTaskId: "agent-task-implementation",
      agentTeamId: "agent-team-implementation",
      attempts: [
        {
          agentTaskId: "agent-task-implementation",
          agentTeamId: "agent-team-implementation",
          commitId: "implementation123456",
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: null,
          id: "attempt-implementation",
          implementationChatId: "plan-chat-implementation",
          modelId: null,
          phaseId: "plan-phase-active-merge",
          planId: "plan-active-merge",
          providerId: null,
          sequence: 0,
          startedAt: timestamp,
          status: "completed",
          thinkingLevel: null,
          trigger: "start",
          updatedAt: timestamp,
        },
        {
          agentTaskId: "agent-task-old-merge",
          agentTeamId: "agent-team-old-merge",
          commitId: null,
          completedAt: null,
          createdAt: timestamp,
          errorMessage: null,
          id: "attempt-old-merge",
          implementationChatId: "plan-chat-old-merge",
          modelId: null,
          phaseId: "plan-phase-active-merge",
          planId: "plan-active-merge",
          providerId: null,
          sequence: 1,
          startedAt: timestamp,
          status: "queued",
          thinkingLevel: null,
          trigger: "merge_auto",
          updatedAt: timestamp,
        },
        {
          agentTaskId: "agent-task-active-merge",
          agentTeamId: "agent-team-active-merge",
          commitId: null,
          completedAt: null,
          createdAt: timestamp,
          errorMessage: null,
          id: "attempt-active-merge",
          implementationChatId: "plan-chat-active-merge",
          modelId: null,
          phaseId: "plan-phase-active-merge",
          planId: "plan-active-merge",
          providerId: null,
          sequence: 2,
          startedAt: timestamp,
          status: "running",
          thinkingLevel: null,
          trigger: "merge_retry",
          updatedAt: timestamp,
        },
      ],
      commitId: "implementation123456",
      completedAt: timestamp,
      createdAt: timestamp,
      errorMessage: null,
      id: "plan-phase-active-merge",
      implementationChatId: "plan-chat-implementation",
      mergeAttemptCount: 1,
      planId: "plan-active-merge",
      sequence: 0,
      startedAt: timestamp,
      status: "completed",
      steps: [],
      summary: "Implementation is complete while merge is running.",
      title: "Completed implementation",
      updatedAt: timestamp,
    };
    const activeMergePlan = {
      activePhaseId: null,
      completedAt: null,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      sharedMergeCommitId: null,
      id: "plan-active-merge",
      overview: "Open the active merge coordinator transcript.",
      pauseRequestedAt: null,
      phases: [completedPhase],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "running",
      title: "Active merge plan",
      updatedAt: timestamp,
    };
    const mergeWithoutChatPlan = {
      ...activeMergePlan,
      id: "plan-merge-without-chat",
      phases: [
        {
          ...completedPhase,
          attempts: [
            {
              ...completedPhase.attempts[2],
              id: "attempt-merge-without-chat",
              implementationChatId: null,
              phaseId: "plan-phase-merge-without-chat",
              planId: "plan-merge-without-chat",
            },
          ],
          id: "plan-phase-merge-without-chat",
          planId: "plan-merge-without-chat",
        },
      ],
      title: "Merge without chat plan",
    };
    const mergeFallbackPlan = {
      ...activeMergePlan,
      id: "plan-merge-fallback",
      phases: [
        {
          ...completedPhase,
          attempts: [],
          id: "plan-phase-merge-fallback",
          planId: "plan-merge-fallback",
        },
      ],
      title: "Merge fallback plan",
    };
    const mergedPlan = {
      ...activeMergePlan,
      sharedMergeCommitId: "fedcba987654321",
      status: "implemented",
      updatedAt: "2026-07-23T12:01:00Z",
    };
    let showMergedPlan = false;
    appTestState.workspaceResponseWorkspaces = [
      { ...workspace, chats: [mergeChat, ...workspace.chats] },
      secondaryWorkspace,
    ];
    appTestState.chatMessagesResponsesByChatKey = {
      "workspace-1/plan-chat-active-merge": {
        ...chatMessages,
        chat: {
          ...chatMessages.chat,
          id: "plan-chat-active-merge",
          title: "Active merge coordinator chat",
        },
        messages: [
          {
            ...chatMessages.messages[0],
            content: "Active merge coordinator transcript.",
            parts: [
              { text: "Active merge coordinator transcript.", type: "text" },
            ],
          },
        ],
      },
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = new URL(url, "http://127.0.0.1").pathname;
        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: showMergedPlan
              ? [mergedPlan]
              : [activeMergePlan, mergeWithoutChatPlan, mergeFallbackPlan],
            totalCount: showMergedPlan ? 1 : 3,
            totalPages: 1,
          });
        }
        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    const activeMergeCard = (await screen.findByText("Active merge plan")).closest(
      "article",
    ) as HTMLElement;
    expect(within(activeMergeCard).getAllByText("Merging")).toHaveLength(2);
    expect(
      within(activeMergeCard).getByRole("button", { name: "Open merge chat" }),
    ).toHaveClass("h-auto", "min-h-0");
    const fallbackMergeCard = screen.getByText("Merge fallback plan").closest("article") as HTMLElement;
    expect(within(fallbackMergeCard).getAllByText("Merging")).toHaveLength(2);
    expect(
      within(fallbackMergeCard).queryByRole("button", { name: "Open merge chat" }),
    ).not.toBeInTheDocument();
    const noChatCard = screen.getByText("Merge without chat plan").closest("article") as HTMLElement;
    expect(within(noChatCard).getAllByText("Merging")).toHaveLength(2);
    expect(
      within(noChatCard).queryByRole("button", { name: "Open merge chat" }),
    ).not.toBeInTheDocument();
    await user.click(
      within(activeMergeCard).getByRole("button", { name: "Open merge chat" }),
    );
    expect(
      await screen.findByText("Active merge coordinator transcript."),
    ).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/chats/plan-chat-active-merge/messages?limit=60",
      expect.any(Object),
    );
    expect(fetchMock).not.toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/chats/plan-chat-implementation/messages?limit=60",
      expect.any(Object),
    );

    showMergedPlan = true;
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 3100));
    });
    expect(await screen.findByText("fedcba9")).toBeInTheDocument();
    expect(screen.queryByText("Merging")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Open merge chat" }),
    ).not.toBeInTheDocument();
  });

  it("shows retry merge for LLM merge failures without phase retry", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-22T18:00:00Z";
    const mergeError = "LLM merge failed: conflict";
    const completedPhase = {
      agentTaskId: "agent-task-impl-1",
      agentTeamId: "agent-team-impl-1",
      attempts: [
        {
          agentTaskId: "agent-task-impl-1",
          agentTeamId: "agent-team-impl-1",
          commitId: "implcommit123456",
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: null,
          id: "attempt-impl-1",
          implementationChatId: "plan-chat-impl-1",
          modelId: null,
          phaseId: "plan-phase-llm-merge-fail",
          planId: "plan-llm-merge-fail",
          providerId: null,
          sequence: 0,
          startedAt: timestamp,
          status: "completed",
          thinkingLevel: null,
          trigger: "start",
          updatedAt: timestamp,
        },
        {
          agentTaskId: "agent-task-merge-1",
          agentTeamId: "agent-team-merge-1",
          commitId: null,
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: mergeError,
          id: "attempt-merge-1",
          implementationChatId: "plan-chat-merge-1",
          modelId: null,
          phaseId: "plan-phase-llm-merge-fail",
          planId: "plan-llm-merge-fail",
          providerId: null,
          sequence: 1,
          startedAt: timestamp,
          status: "failed",
          thinkingLevel: null,
          trigger: "merge_auto",
          updatedAt: timestamp,
        },
      ],
      commitId: "implcommit123456",
      completedAt: timestamp,
      createdAt: timestamp,
      errorMessage: mergeError,
      id: "plan-phase-llm-merge-fail",
      implementationChatId: "plan-chat-impl-1",
      mergeAttemptCount: 1,
      planId: "plan-llm-merge-fail",
      sequence: 0,
      startedAt: timestamp,
      status: "completed",
      steps: [
        {
          acceptance: ["Phase implementation is done."],
          checkedAt: timestamp,
          createdAt: timestamp,
          detail: "Implementation finished before merge.",
          id: "plan-step-llm-merge-fail",
          phaseId: "plan-phase-llm-merge-fail",
          planId: "plan-llm-merge-fail",
          sequence: 0,
          status: "completed",
          title: "Implement phase",
          updatedAt: timestamp,
        },
      ],
      summary: "Implementation completed; merge failed.",
      title: "Final phase",
      updatedAt: timestamp,
    };
    const failedMergePlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: mergeError,
      sharedMergeCommitId: null,
      id: "plan-llm-merge-fail",
      overview: "All phases implemented; shared merge failed.",
      pauseRequestedAt: null,
      phases: [completedPhase],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "implemented",
      title: "LLM merge failed plan",
      updatedAt: timestamp,
    };
    const runningMergePlan = {
      ...failedMergePlan,
      completedAt: null,
      errorMessage: null,
      status: "running",
      updatedAt: "2026-07-22T18:01:00Z",
    };
    let didRetryMerge = false;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [didRetryMerge ? runningMergePlan : failedMergePlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (
          path ===
          "/api/workspaces/workspace-1/plans/plan-llm-merge-fail/action"
        ) {
          didRetryMerge = true;
          return jsonResponse({ plan: runningMergePlan });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await user.click(screen.getByRole("tab", { name: "Plan" }));

    const planCard = (await screen.findByText("LLM merge failed plan")).closest(
      "article",
    );
    expect(planCard).not.toBeNull();
    const card = planCard as HTMLElement;

    const retryButton = within(card).getByRole("button", {
      name: "Retry Merge",
    });
    expect(retryButton).toHaveAttribute(
      "aria-describedby",
      "plan-merge-retry-hint-plan-llm-merge-fail",
    );
    expect(
      within(card).getByText("Retry merging into the shared workspace"),
    ).toHaveClass("sr-only");
    expect(within(card).getByText(mergeError)).toBeInTheDocument();
    expect(within(card).getByText("Completed")).toBeInTheDocument();
    expect(
      within(card).queryByRole("button", { name: "Retry plan phase" }),
    ).not.toBeInTheDocument();

    await user.click(retryButton);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/plans/plan-llm-merge-fail/action",
        expect.objectContaining({
          body: JSON.stringify({ action: "retry_merge" }),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(didRetryMerge).toBe(true);
      expect(within(card).getByText("Running")).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("button", { name: "Retry Merge" }),
    ).not.toBeInTheDocument();
  });

  it("keeps retry merge action response visible when the refresh returns stale plan data", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-04T12:20:00Z";
    const blockedPlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage:
        "cannot merge Agent worktree while shared workspace has uncommitted changes",
      sharedMergeCommitId: null,
      id: "plan-merge-stale-refresh",
      overview: "Retry should show that merge work has started.",
      pauseRequestedAt: null,
      phases: [],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "implemented",
      title: "Stale retry merge plan",
      updatedAt: timestamp,
    };
    const runningPlan = {
      ...blockedPlan,
      completedAt: null,
      errorMessage: null,
      status: "running",
      updatedAt: "2026-07-04T12:21:00Z",
    };
    let didRetryMerge = false;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [blockedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        if (
          path ===
          "/api/workspaces/workspace-1/plans/plan-merge-stale-refresh/action"
        ) {
          didRetryMerge = true;
          return jsonResponse({ plan: runningPlan });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await user.click(screen.getByRole("tab", { name: "Plan" }));
    const retryButton = await screen.findByRole("button", {
      name: "Retry Merge",
    });

    await user.click(retryButton);

    await waitFor(() => {
      expect(didRetryMerge).toBe(true);
      expect(screen.getByText("Running")).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("button", { name: "Retry Merge" }),
    ).not.toBeInTheDocument();
  });

  it("marks implemented plans as merged in the plan panel", async () => {
    const timestamp = "2026-06-28T04:45:00Z";
    const completedStep = {
      acceptance: ["The shared workspace contains the phase result."],
      checkedAt: timestamp,
      createdAt: timestamp,
      detail: "The runner completed the phase merge path.",
      id: "plan-step-merged-1",
      phaseId: "plan-phase-merged-1",
      planId: "plan-merged",
      sequence: 0,
      status: "completed",
      title: "Merge phase changes",
      updatedAt: timestamp,
    };
    const implementedPlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: null,
      sharedMergeCommitId: "fedcba987654321",
      id: "plan-merged",
      overview: "Every phase has completed its implementation chat.",
      pauseRequestedAt: null,
      phases: [
        {
          agentTaskId: "agent-task-merged-1",
          agentTeamId: "agent-team-merged-1",
          commitId: "1111111aaa2222",
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: null,
          id: "plan-phase-merged-1",
          implementationChatId: "plan-chat-merged-1",
          mergeAttemptCount: 0,
          planId: "plan-merged",
          sequence: 0,
          startedAt: timestamp,
          status: "completed",
          steps: [completedStep],
          summary: "Changed files were committed.",
          title: "Committed phase",
          updatedAt: timestamp,
        },
        {
          agentTaskId: "agent-task-merged-2",
          agentTeamId: "agent-team-merged-2",
          commitId: "abc1234def5678",
          completedAt: timestamp,
          createdAt: timestamp,
          errorMessage: null,
          id: "plan-phase-merged-2",
          implementationChatId: "plan-chat-merged-2",
          mergeAttemptCount: 0,
          planId: "plan-merged",
          sequence: 1,
          startedAt: timestamp,
          status: "completed",
          steps: [
            {
              ...completedStep,
              id: "plan-step-merged-2",
              phaseId: "plan-phase-merged-2",
              title: "Run verification",
            },
          ],
          summary: "No file changes were left to commit.",
          title: "No-op phase",
          updatedAt: timestamp,
        },
      ],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "implemented",
      title: "Merged implementation plan",
      updatedAt: timestamp,
    };
    const phaseCommitOnlyImplementedPlan = {
      ...implementedPlan,
      id: "plan-implemented-phase-commit-only",
      phases: implementedPlan.phases.map((phase) => ({
        ...phase,
        id: `${phase.id}-phase-commit-only`,
        planId: "plan-implemented-phase-commit-only",
        steps: phase.steps.map((step) => ({
          ...step,
          id: `${step.id}-phase-commit-only`,
          phaseId: `${phase.id}-phase-commit-only`,
          planId: "plan-implemented-phase-commit-only",
        })),
      })),
      sharedMergeCommitId: null,
      title: "Implemented plan with phase commit only",
    };
    const statusColorPlans = [
      {
        ...implementedPlan,
        completedByUserAt: timestamp,
        id: "plan-color-completed",
        sharedMergeCommitId: null,
        phases: [
          {
            ...implementedPlan.phases[0],
            id: "plan-phase-color-completed",
            planId: "plan-color-completed",
            status: "completed",
            steps: [],
            title: "Completed color phase",
          },
        ],
        status: "completed",
        title: "Completed status colors",
      },
      {
        ...implementedPlan,
        completedAt: null,
        id: "plan-color-failed",
        sharedMergeCommitId: null,
        phases: [
          {
            ...implementedPlan.phases[0],
            completedAt: null,
            id: "plan-phase-color-failed",
            planId: "plan-color-failed",
            status: "failed",
            steps: [],
            title: "Failed color phase",
          },
        ],
        status: "failed",
        title: "Failed status colors",
      },
      {
        ...implementedPlan,
        completedAt: null,
        id: "plan-color-cancelled",
        sharedMergeCommitId: null,
        phases: [
          {
            ...implementedPlan.phases[0],
            completedAt: null,
            id: "plan-phase-color-cancelled",
            planId: "plan-color-cancelled",
            status: "cancelled",
            steps: [],
            title: "Cancelled color phase",
          },
        ],
        status: "cancelled",
        title: "Cancelled status colors",
      },
      {
        ...implementedPlan,
        completedAt: null,
        id: "plan-color-ready",
        sharedMergeCommitId: null,
        phases: [
          {
            ...implementedPlan.phases[0],
            completedAt: null,
            id: "plan-phase-color-ready",
            planId: "plan-color-ready",
            status: "ready",
            steps: [],
            title: "Ready color phase",
          },
        ],
        status: "ready",
        title: "Ready status colors",
      },
    ];
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [
              implementedPlan,
              phaseCommitOnlyImplementedPlan,
              ...statusColorPlans,
            ],
            totalCount: 6,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Plan" }));

    expect(
      await screen.findByText("Merged implementation plan"),
    ).toBeInTheDocument();
    const mergedCommitBadge = screen.getByText("fedcba9");
    expect(mergedCommitBadge).toHaveAttribute(
      "aria-describedby",
      "plan-merge-status-plan-merged",
    );
    expect(screen.getByText("Merged into shared workspace")).toHaveClass(
      "sr-only",
    );

    const phaseCommitOnlyPlanCard = screen
      .getByText("Implemented plan with phase commit only")
      .closest("article");
    expect(phaseCommitOnlyPlanCard).not.toBeNull();
    expect(
      within(phaseCommitOnlyPlanCard as HTMLElement).queryByText(
        "Merged into shared workspace",
      ),
    ).not.toBeInTheDocument();

    function expectPlanStatusTone(
      planTitle: string,
      status: string,
      classes: string[],
    ) {
      const planCard = screen.getByText(planTitle).closest("article");
      if (!planCard) {
        throw new Error(`Expected plan card for ${planTitle}`);
      }

      for (const statusPill of within(planCard).getAllByText(status)) {
        expect(statusPill).toHaveClass(...classes);
      }
    }

    function expectPhaseExpandButtonGhost(planTitle: string, phaseTitle: string) {
      const planCard = screen.getByText(planTitle).closest("article");
      if (!planCard) {
        throw new Error(`Expected plan card for ${planTitle}`);
      }
      const phaseButton = within(planCard)
        .getAllByRole("button")
        .find(
          (button) =>
            button.getAttribute("aria-expanded") != null &&
            button.textContent?.includes(phaseTitle),
        );
      if (!phaseButton) {
        throw new Error(`Expected phase expand button for ${phaseTitle}`);
      }
      expect(phaseButton).toHaveClass("button--ghost");
    }

    expectPlanStatusTone("Merged implementation plan", "Implemented", [
      "bg-[var(--success-soft)]",
      "text-[var(--success-soft-foreground)]",
    ]);
    expectPlanStatusTone("Completed status colors", "Completed", [
      "bg-[var(--success-soft)]",
      "text-[var(--success-soft-foreground)]",
    ]);
    expectPlanStatusTone("Failed status colors", "Failed", [
      "bg-[var(--danger-soft)]",
      "text-[var(--danger)]",
    ]);
    expectPlanStatusTone("Cancelled status colors", "Cancelled", [
      "bg-[var(--surface-secondary)]",
      "text-[var(--muted)]",
    ]);
    expectPlanStatusTone("Ready status colors", "Ready", [
      "bg-[var(--warning-soft)]",
      "text-[var(--warning)]",
    ]);
    expectPhaseExpandButtonGhost(
      "Completed status colors",
      "Completed color phase",
    );
    expectPhaseExpandButtonGhost("Failed status colors", "Failed color phase");
    expectPhaseExpandButtonGhost(
      "Cancelled status colors",
      "Cancelled color phase",
    );
    expectPhaseExpandButtonGhost("Ready status colors", "Ready color phase");
  });

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

      if (
        path === "/api/workspaces/workspace-1/files" &&
        holdNextRequest.files
      ) {
        const request = deferred<Response>();
        heldRequests.files.push(request);
        return request.promise;
      }

      if (
        path === "/api/workspaces/workspace-1/git/diff" &&
        holdNextRequest.diff
      ) {
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
      expect(
        screen.getByRole("button", { name: "Refresh files" }),
      ).not.toBeDisabled(),
    );
    holdNextRequest.files = true;
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh files" }),
    );
    await waitFor(() => expect(heldRequests.files).toHaveLength(1));
    await expectRefreshIconLoading("Refresh files");
    await act(async () => {
      heldRequests.files[0]?.resolve(jsonResponse(workspaceFilesResponse));
    });

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));
    await screen.findByText("Source Control");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Refresh diff" }),
      ).not.toBeDisabled(),
    );
    holdNextRequest.diff = true;
    await userEvent.click(screen.getByRole("button", { name: "Refresh diff" }));
    await waitFor(() => expect(heldRequests.diff).toHaveLength(1));
    await expectRefreshIconLoading("Refresh diff");
    await act(async () => {
      heldRequests.diff[0]?.resolve(
        jsonResponse(appTestState.workspaceGitDiffResponse),
      );
    });

    await userEvent.click(screen.getByRole("tab", { name: "Spec" }));
    await screen.findAllByRole("heading", { name: "Project Spec" });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Reload spec" }),
      ).not.toBeDisabled(),
    );
    holdNextRequest.spec = true;
    await userEvent.click(screen.getByRole("button", { name: "Reload spec" }));
    await waitFor(() => expect(heldRequests.spec).toHaveLength(1));
    await expectRefreshIconLoading("Reload spec");
    await act(async () => {
      heldRequests.spec[0]?.resolve(
        jsonResponse(appTestState.workspaceSpecResponse),
      );
    });

    await userEvent.click(screen.getByRole("tab", { name: "Agents" }));
    expect(
      (await screen.findByRole("heading", { name: "Agents" })).closest(
        ".context-panel-page-header",
      ),
    ).not.toBeNull();
    const agentRefreshButton = await screen.findByRole("button", {
      name: "Refresh",
    });
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
    const specHeadings = await openSpecPanel();

    expect(
      specHeadings.some((heading) =>
        Boolean(heading.closest(".context-panel-page-header")),
      ),
    ).toBe(true);
    expect(
      screen.getByRole("button", { name: "Edit markdown" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("heading", { name: "Purpose" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Describe the current workspace."),
    ).toBeInTheDocument();
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

    expect(
      await screen.findByRole("heading", { name: "项目 Spec" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "编辑 Markdown" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("heading", { name: "Purpose" }),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("项目 Spec Markdown")).toBeNull();
    expect(
      screen.queryByRole("checkbox", { name: "启用项目 Spec" }),
    ).toBeNull();
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

    await userEvent.click(
      screen.getByRole("button", { name: "Inject into new chats" }),
    );
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

    await userEvent.click(
      screen.getByRole("button", { name: "Inject into new chats" }),
    );
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
    await userEvent.click(
      screen.getByRole("button", { name: "Edit markdown" }),
    );
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
        contentMarkdown:
          "# Project Spec\n\n## Purpose\n\nUpdated from the right panel.",
        expectedRevision: 3,
      });
    });
    expect((await screen.findAllByText(/Revision 4/)).length).toBeGreaterThan(
      0,
    );
  });

  it("queues Project Spec generation from the right panel", async () => {
    const fetchMock = vi.mocked(fetch);
    await openSpecPanel();

    await userEvent.click(
      screen.getByRole("button", { name: "Regenerate spec" }),
    );

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/spec/generate",
      );
      expect(call).toBeDefined();
      expect(JSON.parse(String(call?.[1]?.body))).toEqual({ modelId: null });
    });
    expect(
      await screen.findByText(new RegExp(workspaceSpecQueuedJob.id)),
    ).toBeInTheDocument();
    expect(screen.getByText(/Latest job: Queued/)).toBeInTheDocument();
  });

  it("auto-reloads Project Spec content after generation completes", async () => {
    appTestState.workspaceSpecGenerateCompletes = true;
    await openSpecPanel();

    await userEvent.click(
      screen.getByRole("button", { name: "Regenerate spec" }),
    );

    await waitFor(
      () => {
        expect(screen.getByText("Regenerated by the LLM.")).toBeInTheDocument();
      },
      { timeout: 5000 },
    );
  });

  it("keeps observing remote Project Spec generation beyond the old polling ceiling", async () => {
    appTestState.workspaceSpecJobPollsBeforeCompletion = 8;
    await openSpecPanel();

    // Fake only timeout APIs so long backoff can be advanced without waiting
    // real wall-clock time. Prefer fireEvent over userEvent under fake timers.
    // Also fake Date so module-level request storm dedupe (400ms wall clock)
    // advances with the same timer stream as poll delays.
    vi.useFakeTimers({
      toFake: ["setTimeout", "clearTimeout", "Date"],
    });
    try {
      fireEvent.click(screen.getByRole("button", { name: "Regenerate spec" }));
      await act(async () => {
        await Promise.resolve();
        // Sum of WORKSPACE_SPEC_JOB_POLL_DELAYS_MS for 9 polls exceeds the old
        // hard ceiling; advance past the full schedule plus steady interval.
        await vi.advanceTimersByTimeAsync(225_000);
      });

      expect(appTestState.workspaceSpecJobPollCount).toBeGreaterThanOrEqual(9);
      expect(screen.getByText("Regenerated by the LLM.")).toBeInTheDocument();
      expect(screen.getByText(/Latest job: Completed/)).toBeInTheDocument();
      expect(screen.getAllByText(/Revision 4/).length).toBeGreaterThan(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("recovers Project Spec polling after a temporary remote proxy failure", async () => {
    appTestState.workspaceSpecJobPollFailuresRemaining = 1;
    appTestState.workspaceSpecJobPollsBeforeCompletion = 0;
    await openSpecPanel();

    await userEvent.click(
      screen.getByRole("button", { name: "Regenerate spec" }),
    );

    expect(
      await screen.findByText("temporary remote spec proxy failure", {}, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("Regenerated by the LLM.", {}, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(screen.queryByText("temporary remote spec proxy failure")).toBeNull();
  });

  it("shows Project Spec save conflicts with a reload action", async () => {
    appTestState.workspaceSpecSaveConflict = true;
    await openSpecPanel();
    await userEvent.click(
      screen.getByRole("button", { name: "Edit markdown" }),
    );
    changeInput(
      screen.getByLabelText("Project Spec Markdown"),
      "# Project Spec\n\n## Purpose\n\nConflicting edit.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(
      await screen.findByText(
        "workspace spec revision changed; reload and retry",
      ),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getAllByRole("button", { name: "Reload spec" })[1],
    );
    await waitFor(() => {
      expect(
        screen.getByText("Describe the current workspace."),
      ).toBeInTheDocument();
      expect(screen.queryByLabelText("Project Spec Markdown")).toBeNull();
    });
  });

  it("keeps workspace terminals mounted while switching workspaces", async () => {
    const fetchMock = vi.mocked(fetch);
    const closeSpy = vi.spyOn(window.WebSocket.prototype, "close");

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(
      screen.getByRole("button", { name: "Open terminal" }),
    );
    expect(await screen.findByLabelText("connected")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/terminal/session",
      ),
    ).toHaveLength(1);

    await userEvent.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") &&
          accessibleName.startsWith("Side project"),
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: /Side note/ }));
    expect(
      screen.getByRole("button", { name: "Open terminal" }),
    ).toBeInTheDocument();
    expect(closeSpy).not.toHaveBeenCalled();

    await userEvent.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") &&
          accessibleName.startsWith("Default"),
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: /Tool run/ }));
    expect(
      screen.getAllByRole("button", { name: "Close terminal" }),
    ).toHaveLength(2);
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
        return Promise.resolve(
          jsonResponse({
            activeWorkspaceId: commandWorkspace.id,
            workspaces: [commandWorkspace, secondaryWorkspace],
          }),
        );
      }

      if (path === "/api/settings") {
        return Promise.resolve(
          jsonResponse({
            ...settings,
            workspaces: [
              {
                ...settings.workspaces[0],
                commonCommands: commandWorkspace.commonCommands,
              },
            ],
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });
    const sendSpy = vi.spyOn(window.WebSocket.prototype, "send");

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(
      screen.getByRole("button", { name: "Open terminal" }),
    );
    expect(await screen.findByLabelText("connected")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Run common command Dev" }),
    );

    await waitFor(() => {
      const sentInput = sendSpy.mock.calls
        .map(
          ([data]) =>
            JSON.parse(String(data)) as { data?: string; type: string },
        )
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

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "plan",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
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
    const contextPanel = todoTaskButton.closest(
      ".context-panel",
    ) as HTMLElement;
    await userEvent.click(todoTaskButton);
    expect(
      await screen.findByText("README.md diff is visible"),
    ).toBeInTheDocument();
    expect(
      within(contextPanel).queryByText(/hello world/),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(screen.getByText("Source Control")).toBeInTheDocument();
    expect(screen.getByText("Source Control").closest(".context-panel-page-header")).not.toBeNull();
    expect(contextPanel.querySelector(".context-panel-tabs")).toHaveClass("tabs--secondary");
    expect(
      screen.getAllByRole("button", { name: /README\.md M/ }),
    ).toHaveLength(2);
    expect(
      screen.getAllByRole("button", { name: /new-note\.txt U/ }),
    ).toHaveLength(2);
    expect(
      screen.getAllByRole("button", { name: /asset\.bin M/ }).length,
    ).toBeGreaterThan(0);
    expect(
      within(contextPanel).queryByText(/hello world/),
    ).not.toBeInTheDocument();

    await userEvent.click(
      screen.getAllByRole("button", { name: /README\.md M/ })[0],
    );

    const inlineDiffLine = (
      await within(contextPanel).findAllByText(/hello world/)
    )[0];
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
    expect(
      within(contextPanel).queryByText("Inspect workspace changes"),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows an inline diff notice for binary changed files", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));
    await userEvent.click(
      await screen.findByRole("button", { name: /asset\.bin M/ }),
    );

    expect(
      await screen.findByText(
        "Inline diff is unavailable for binary or non-text files.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Binary files a/asset.bin and b/asset.bin differ"),
    ).not.toBeInTheDocument();
  });

  it("deletes memories from the right panel memory tab", async () => {
    const fetchMock = vi.mocked(fetch);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Memory" }));

    const globalItem = (await screen.findByText(activeMemory.fact)).closest(
      "article",
    );
    const workspaceItem = (
      await screen.findByText(workspaceMemory.fact)
    ).closest("article");
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

    const statsTitle = await screen.findByText("Session statistics");
    expect(statsTitle.closest(".context-panel-page-header")).not.toBeNull();
    expect(document.querySelector(".context-stats-panel")).toHaveClass(
      "min-h-0",
      "flex-1",
    );
    expect(screen.getByText("17.6K")).toBeInTheDocument();
    expect(
      within(
        screen.getByText("Memory refs").closest(".context-stat-metric")!,
      ).getByText("3"),
    ).toBeInTheDocument();
    expect(
      within(
        screen.getByText("New memories").closest(".context-stat-metric")!,
      ).getByText("2"),
    ).toBeInTheDocument();
    expect(screen.getByText("+12 / -3")).toBeInTheDocument();
    expect(
      within(screen.getByText("Model calls").parentElement!).getByText(
        "gpt-test",
      ),
    ).toBeInTheDocument();
    const toolsSection = screen.getByText(
      "Tools and compression",
    ).parentElement!;
    expect(within(toolsSection).getByText("Read")).toBeInTheDocument();
    expect(
      within(toolsSection).queryByText("Rule compression snapshots"),
    ).not.toBeInTheDocument();
    expect(
      within(toolsSection).queryByText("Compression snapshots"),
    ).not.toBeInTheDocument();
    expect(
      within(toolsSection).queryByText("Tool history compression"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("52,340 / 110,960")).not.toBeInTheDocument();
    const contextTimeline = screen.getByLabelText("Context usage timeline");
    expect(within(contextTimeline).getByText("47%")).toBeInTheDocument();
    expect(
      within(contextTimeline).queryByText("Snapshot 1"),
    ).not.toBeInTheDocument();
    expect(
      within(contextTimeline).queryByText("Snapshot 2"),
    ).not.toBeInTheDocument();
    expect(
      within(contextTimeline).queryByText(/llm \/ ctx-/),
    ).not.toBeInTheDocument();
    expect(
      within(contextTimeline).queryByText("Past 80%"),
    ).not.toBeInTheDocument();
    expect(within(contextTimeline).queryByText("80%")).not.toBeInTheDocument();
    expect(
      contextTimeline.querySelector(".context-usage-bar-threshold.is-tool-state"),
    ).toBeNull();
    expect(
      contextTimeline.querySelector(
        ".context-usage-trigger-marker.is-tool-state",
      ),
    ).toBeNull();
    expect(within(contextTimeline).getAllByText("95%")).not.toHaveLength(0);
    const contextLegend = within(contextTimeline).getByLabelText(
      "Context usage legend",
    );
    expect(within(contextLegend).getByText("Prompt/tools")).toBeInTheDocument();
    expect(within(contextLegend).getByText("History")).toBeInTheDocument();
    expect(
      within(contextLegend).getByText("Compression snapshot"),
    ).toBeInTheDocument();
    expect(
      within(contextLegend).queryByText("Reserved output"),
    ).not.toBeInTheDocument();
    expect(
      within(contextTimeline).getAllByLabelText(/Prompt\/tools:/),
    ).not.toHaveLength(0);
    expect(
      within(contextTimeline).getAllByLabelText(/History:/),
    ).not.toHaveLength(0);
    expect(
      within(contextTimeline).getAllByLabelText(/Compression snapshot:/),
    ).not.toHaveLength(0);
    expect(
      within(contextTimeline).queryByLabelText(/Reserved output:/),
    ).not.toBeInTheDocument();
    expect(
      contextTimeline.querySelector(".context-usage-history-stack"),
    ).not.toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(
        ([url]) => url === "/api/workspaces/workspace-1/context-usage",
      ),
    ).toBe(true);
  });

  it("renders remote tools, compression, model/provider, and current context usage", async () => {
    const { chatId, workspaceId } = configureRemoteSessionStatistics();
    const chatKey = `${workspaceId}/${chatId}`;
    appTestState.contextUsageResponseQueuesByChatKey = {
      [chatKey]: [
        {
          ...contextUsage,
          totalUsedContextTokens: 78080,
          usagePercent: 61,
        },
      ],
    };
    window.history.replaceState(null, "", `/${workspaceId}/${chatId}`);
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Stats" }));

    expect(await screen.findByText("remote-gpt")).toBeInTheDocument();
    expect(screen.getByText("remote-provider")).toBeInTheDocument();
    expect(screen.getByText("Provider calls")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();
    expect(screen.getByText("LLM compression snapshots")).toBeInTheDocument();
    const tokensSaved = screen
      .getByText("Tokens saved")
      .closest(".context-stats-row") as HTMLElement | null;
    expect(tokensSaved).not.toBeNull();
    expect(within(tokensSaved!).getByText("7,654")).toBeInTheDocument();
    expect(screen.queryByText("Runtime tool-state snapshots")).not.toBeInTheDocument();

    const timeline = await screen.findByLabelText("Context usage timeline");
    expect(within(timeline).getByText("61%")).toBeInTheDocument();
    expect(
      timeline.querySelector(".context-usage-history-stack"),
    ).not.toBeInTheDocument();
  });

  it("shows tool history compression when runtime tool-state compression is enabled", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      general: {
        ...appTestState.settingsResponse.general,
        runtimeToolStateCompressionEnabled: true,
      },
    };
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Stats" }));

    const toolHistoryCompression = await screen.findByText(
      "Tool history compression",
    );
    expect(
      within(toolHistoryCompression.parentElement!).getByText("2"),
    ).toBeInTheDocument();

    const contextTimeline = screen.getByLabelText("Context usage timeline");
    expect(within(contextTimeline).getAllByText("80%")).not.toHaveLength(0);
    expect(within(contextTimeline).getAllByText("95%")).not.toHaveLength(0);
    expect(
      contextTimeline.querySelector(".context-usage-bar-threshold.is-tool-state"),
    ).not.toBeNull();
    expect(
      contextTimeline.querySelector(
        ".context-usage-trigger-marker.is-tool-state",
      ),
    ).not.toBeNull();
  });

  it("renders partial active chat statistics and context usage payloads without crashing", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input, init) => {
      const url =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = url.startsWith("http") ? new URL(url).pathname : url;
      if (path === "/api/workspaces/workspace-1/chats/chat-1/statistics") {
        return jsonResponse({ workspaceId: workspace.id, chatId: "chat-1" });
      }
      if (path === "/api/workspaces/workspace-1/context-usage") {
        return jsonResponse({ contextWindow: 0 });
      }
      return mockFetch(input, init);
    });
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Stats" }));

    expect(await screen.findByText("Session statistics")).toBeInTheDocument();
    expect(
      within(
        screen.getByText("Total tokens").closest(".context-stat-metric")!,
      ).getByText("0"),
    ).toBeInTheDocument();
    expect(screen.getByText("+0 / -0")).toBeInTheDocument();
    expect(
      within(screen.getByText("Model calls").parentElement!).getByText(
        "No model calls yet.",
      ),
    ).toBeInTheDocument();
    const toolsSection = screen.getByText(
      "Tools and compression",
    ).parentElement!;
    expect(
      within(toolsSection).getByText("LLM compression snapshots"),
    ).toBeInTheDocument();
    expect(
      within(toolsSection).queryByText("Tool history compression"),
    ).not.toBeInTheDocument();
    expect(
      within(toolsSection).getAllByText("0").length,
    ).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("No context usage yet.")).toBeInTheDocument();
  });

  it("shows context usage only once in the stats context mix", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByText("Tool run"));
    await user.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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

    await user.click(await screen.findByRole("tab", { name: "Stats" }));

    const contextTimeline = screen.getByLabelText("Context usage timeline");
    expect(within(contextTimeline).getByText("55%")).toBeInTheDocument();
    expect(
      contextTimeline.querySelector<HTMLElement>(".context-usage-bar-track"),
    ).toHaveAttribute("title", "70,000 / 128,000");
    expect(within(contextTimeline).getByText("70,000")).toBeInTheDocument();

    const contextMix = screen.getByText("Context mix").parentElement!;
    expect(
      within(contextMix).queryByText("52,340 / 110,960"),
    ).not.toBeInTheDocument();
    expect(contextMix.querySelector(".context-mini-chart-bars")).not.toBeNull();
    expect(contextMix.querySelector(".context-stats-rows")).toBeNull();
    expect(within(contextMix).getAllByText("History")).toHaveLength(1);
    expect(within(contextMix).getAllByText("Current user")).toHaveLength(1);
    expect(within(contextMix).getAllByText("Tools")).toHaveLength(1);
    expect(within(contextMix).queryByText("ToDo")).not.toBeInTheDocument();
    expect(within(contextMix).getAllByText("32,000")).toHaveLength(1);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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

  it("single-flights duplicate active chat statistics refreshes", async () => {
    const user = userEvent.setup();
    let nowMs = 0;
    const performanceNowSpy = vi
      .spyOn(performance, "now")
      .mockImplementation(() => nowMs);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Stats" }));
    expect(await screen.findByText("Session statistics")).toBeInTheDocument();
    nowMs = 1000;
    await user.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "save memory",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const fetchMock = vi.mocked(fetch);
    fetchMock.mockClear();
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
        extractedMemories: [],
        type: "memoryExtractionComplete",
      });
    });

    await screen.findByText("Saved.");
    const statisticsRequests = fetchMock.mock.calls.filter(([input]) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      return (
        new URL(rawUrl, "http://127.0.0.1").pathname ===
        "/api/workspaces/workspace-1/chats/chat-1/statistics"
      );
    });
    expect(statisticsRequests).toHaveLength(1);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
    performanceNowSpy.mockRestore();
  });

  it("aborts active streams and chat message loads on app unmount", async () => {
    const user = userEvent.setup();
    const observedSignals: {
      loading: AbortSignal | null;
      stream: AbortSignal | null;
    } = {
      loading: null,
      stream: null,
    };
    vi.mocked(fetch).mockImplementation(async (input, init) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;
      if (path === "/api/workspaces/workspace-1/chat/stream") {
        observedSignals.stream = init?.signal ?? null;
      }
      if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
        observedSignals.loading = init?.signal ?? null;
        return new Promise<Response>((_, reject) => {
          const signal = init?.signal;
          if (signal?.aborted) {
            reject(
              new DOMException("The operation was aborted.", "AbortError"),
            );
            return;
          }
          signal?.addEventListener(
            "abort",
            () =>
              reject(
                new DOMException("The operation was aborted.", "AbortError"),
              ),
            { once: true },
          );
        });
      }
      return mockFetch(input, init);
    });

    const { unmount } = renderApp();
    await user.click(await screen.findByText("Tool run"));
    await user.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(observedSignals.stream).not.toBeNull());
    await user.click(await screen.findByText("Second chat"));
    await waitFor(() => expect(observedSignals.loading).not.toBeNull());

    unmount();

    expect(observedSignals.stream?.aborted).toBe(true);
    expect(observedSignals.loading?.aborted).toBe(true);
  });

  it("ignores stale chat statistics after switching chats", async () => {
    const user = userEvent.setup();
    const staleStats = deferred<Response>();
    vi.mocked(fetch).mockImplementation(async (input, init) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;
      if (path === "/api/workspaces/workspace-1/chats/chat-1/statistics") {
        return staleStats.promise;
      }
      if (path === "/api/workspaces/workspace-1/chats/chat-2/statistics") {
        return jsonResponse({
          ...chatStatistics,
          chatId: "chat-2",
          messageCount: 4,
          totalTokens: 42000,
        });
      }
      return mockFetch(input, init);
    });
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Stats" }));
    await waitFor(() => expect(staleStats.resolve).toBeDefined());
    await user.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("42K")).toBeInTheDocument();

    await act(async () => {
      staleStats.resolve(jsonResponse(chatStatistics));
      await staleStats.promise;
    });

    expect(screen.getByText("42K")).toBeInTheDocument();
    expect(screen.queryByText("17.6K")).not.toBeInTheDocument();
  });

  it("opens the todo graph sidebar when a todo graph refresh arrives", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
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
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "plan",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
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
    expect(
      within(todoPanel).getByText("Inspect workspace changes"),
    ).toBeInTheDocument();
    const todoTaskButton = within(todoPanel).getByRole("button", {
      name: /Inspect workspace changes/,
    });
    expect(todoTaskButton).toHaveClass("button--ghost");
    expect(todoTaskButton.closest(".card")).toHaveClass(
      "todo-graph-task-card",
      "card--default",
    );
    expect(within(todoPanel).getByText("running")).toHaveClass(
      "bg-[var(--warning-soft)]",
      "text-[var(--warning)]",
    );
    expect(within(todoPanel).getByText("completed")).toHaveClass(
      "bg-[var(--success-soft)]",
      "text-[var(--success-soft-foreground)]",
    );
    expect(within(todoPanel).getByText("pending")).toHaveClass(
      "bg-[var(--surface-secondary)]",
      "text-[var(--muted)]",
    );
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

    expect(
      await screen.findByText("Please inspect README."),
    ).toBeInTheDocument();
    await waitFor(() => expect(todoGraphRequests).toHaveLength(1));

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
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

    expect(
      await screen.findByText("Inspect workspace changes"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Failed to fetch")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows the active workspace identity in the empty chat area", async () => {
    renderApp();

    expect(
      await screen.findByRole("heading", { name: workspace.name }),
    ).toBeInTheDocument();
    expect(screen.getByText("Workspace")).toBeInTheDocument();
    const workspaceLogo = document.querySelector(
      `.api-overview-panel img[src="${workspace.logoUrl}"]`,
    );
    expect(workspaceLogo).toHaveClass("size-20");
    expect(workspaceLogo).toHaveClass("rounded-2xl");
    expect(workspaceLogo).toHaveClass("object-cover");
    expect(workspaceLogo?.parentElement).toHaveClass("overflow-hidden");
    expect(workspaceLogo?.parentElement).not.toHaveClass("border");
    expect(workspaceLogo?.parentElement).not.toHaveClass("bg-white");
    expect(aiStatisticsCallUrls()).toHaveLength(0);
  });

  it("shows AI statistics and request details", async () => {
    renderApp();

    expect(
      await screen.findByRole("heading", { name: workspace.name }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Workspace shell is ready"),
    ).not.toBeInTheDocument();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );

    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getAllByText("17.6K").length).toBeGreaterThan(0),
    );
    expect(screen.getByText("Total requests")).toBeInTheDocument();
    expect(screen.getByText("Total tokens")).toBeInTheDocument();
    expect(screen.getByText("Average latency")).toBeInTheDocument();
    expect(screen.getByText("Failed requests")).toBeInTheDocument();
    expect(
      await screen.findByText("Requests and tokens trend"),
    ).toBeInTheDocument();
    expect(screen.getByText("Model distribution")).toBeInTheDocument();
    expect(screen.getByText("Channel distribution")).toBeInTheDocument();
    expect(screen.getByText("Channel quality")).toBeInTheDocument();
    expect(screen.getByText("Request audit")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Provider/ }));
    await userEvent.click(await screen.findByRole("option", { name: "OpenAI" }));
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) =>
            url.searchParams.get("providerId") === "openai" &&
            url.searchParams.get("page") === "1",
        ),
      ).toBe(true),
    );
    const table = screen.getByRole("table");
    const requestTimeCell = table.querySelector("tbody tr td");
    const requestTimeLines = requestTimeCell?.querySelectorAll(
      "div.space-y-1 > div",
    );
    expect(requestTimeLines).toHaveLength(2);
    expect(requestTimeLines?.[0].textContent).toContain("2026");
    const tableScroller = table.parentElement;
    const statsScroller = table.closest(
      ".overflow-y-auto",
    ) as HTMLElement | null;
    expect(tableScroller).toHaveClass("panel-scroll");
    expect(tableScroller).toHaveClass("overflow-x-auto");
    expect(tableScroller).toHaveClass("overflow-y-hidden");
    expect(tableScroller).not.toHaveClass("overflow-auto");
    expect(statsScroller).toHaveClass("panel-scroll");
    if (!tableScroller || !statsScroller) {
      throw new Error(
        "Expected request audit table to live inside stats scroller",
      );
    }
    statsScroller.style.overflowY = "auto";
    Object.defineProperties(statsScroller, {
      clientHeight: { configurable: true, value: 360 },
      scrollHeight: { configurable: true, value: 960 },
    });
    statsScroller.scrollTop = 0;
    fireEvent.touchStart(tableScroller, {
      touches: [{ clientX: 20, clientY: 140 }],
    });
    const verticalTouchMove = new Event("touchmove", {
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperty(verticalTouchMove, "touches", {
      value: [{ clientX: 24, clientY: 90 }],
    });
    tableScroller.dispatchEvent(verticalTouchMove);
    expect(verticalTouchMove.defaultPrevented).toBe(false);
    expect(statsScroller.scrollTop).toBe(0);
    fireEvent.touchEnd(tableScroller);
    await waitFor(() =>
      expect(within(table).getByText("OpenAI")).toBeInTheDocument(),
    );
    expect(within(table).getByText("(HTTP)")).toBeInTheDocument();
    const modelLine = within(table).getByText("GPT Test");
    expect(modelLine).toBeInTheDocument();
    expect(modelLine.textContent).toBe("GPT Test");
    expect(modelLine.textContent).not.toMatch(/HTTP|WebSocket|Unknown/);
    expect(within(table).queryByText("GPT Test(HTTP)")).not.toBeInTheDocument();
    // Provider title includes transport; Request type stays requestKind.
    expect(
      within(table).getByText("OpenAI").closest("div")?.getAttribute("title"),
    ).toBe("OpenAI(HTTP)");
    expect(within(table).getByText("Chat completion")).toBeInTheDocument();
    expect(
      screen.getByRole("navigation", { name: "Request audit pagination" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Go to page 2" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Page size")).toHaveValue(20);

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Provider / model" }),
    );
    expect(within(table).queryByText("OpenAI")).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "View request details" }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Request details",
    });
    expect(
      within(dialog).getByText("Actual provider request"),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText("Final provider response"),
    ).toBeInTheDocument();
    const transportMeta = within(dialog)
      .getByText("Transport")
      .parentElement as HTMLElement;
    expect(within(transportMeta).getByText("HTTP")).toBeInTheDocument();
    expect(within(dialog).getByText("POST")).toBeInTheDocument();
    expect(
      within(dialog).getByText("https://api.example.test/v1/responses"),
    ).toBeInTheDocument();
    const requestHeadersBlock = within(dialog)
      .getByText("Request headers")
      .closest(".audit-json-block");
    expect(requestHeadersBlock).not.toBeNull();
    const requestHeadersViewer = requestHeadersBlock as HTMLElement;
    for (const header of [
      '"accept"',
      '"authorization"',
      '"content-type"',
      '"cookie"',
      '"x-api-key"',
      '"x-real-ip"',
    ]) {
      expect(within(requestHeadersViewer).getByText(header)).toBeInTheDocument();
    }
    expect(
      within(requestHeadersViewer).getAllByText('"application/json"'),
    ).toHaveLength(2);
    for (const value of [
      '"********"',
      '"session=fixture-cookie"',
      '"fixture-api-key"',
      '"203.0.113.42"',
      '"[REDACTED]"',
    ]) {
      expect(within(requestHeadersViewer).getByText(value)).toBeInTheDocument();
    }
    await userEvent.click(
      within(requestHeadersViewer).getByRole("button", {
        name: "Copy Request headers",
      }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      JSON.stringify(aiStatisticsDetail.request.requestBody.headers, null, 2),
    );

    const httpStatusMeta = within(dialog)
      .getByText("HTTP status")
      .parentElement as HTMLElement;
    expect(within(httpStatusMeta).getByText("200")).toBeInTheDocument();
    expect(within(dialog).getByText("HTTP version")).toBeInTheDocument();
    expect(within(dialog).getByText("HTTP/2.0")).toBeInTheDocument();
    const responseHeadersBlock = within(dialog)
      .getByText("Response headers")
      .closest(".audit-json-block");
    expect(responseHeadersBlock).not.toBeNull();
    const responseHeadersViewer = responseHeadersBlock as HTMLElement;
    for (const value of [
      '"********"',
      '"response-session=fixture-cookie"',
      '"fixture-response-api-key"',
      '"request-fixture-1"',
    ]) {
      expect(within(responseHeadersViewer).getByText(value)).toBeInTheDocument();
    }
    await userEvent.click(
      within(responseHeadersViewer).getByRole("button", {
        name: "Copy Response headers",
      }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      JSON.stringify(aiStatisticsDetail.request.responseBody.http.headers, null, 2),
    );

    const responseBodyBlock = within(dialog)
      .getByText("Response body")
      .closest(".audit-json-block");
    expect(responseBodyBlock).not.toBeNull();
    const responseBodyViewer = responseBodyBlock as HTMLElement;
    expect(within(responseBodyViewer).getByText('"Done."')).toBeInTheDocument();
    expect(
      within(responseBodyViewer).getByText('"Finished reasoning."'),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("Stop reason")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Response ID")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Final text")).not.toBeInTheDocument();
    expect(
      within(dialog).queryByText("Final reasoning"),
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Tool calls")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Usage")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Invalidated")).not.toBeInTheDocument();
    expect(
      within(dialog).queryByText("Invalidated reason"),
    ).not.toBeInTheDocument();
    const requestBodyBlock = within(dialog)
      .getByText("Request body")
      .closest(".audit-json-block");
    expect(requestBodyBlock).not.toBeNull();
    const requestBodyViewer = requestBodyBlock as HTMLElement;
    expect(requestBodyViewer).toHaveClass("audit-json-block");

    const requestHeadersCode = requestHeadersViewer.querySelector(
      ".audit-json-code",
    );
    const responseHeadersCode = responseHeadersViewer.querySelector(
      ".audit-json-code",
    );
    const requestBodyCode = requestBodyViewer.querySelector(".audit-json-code");
    const responseBodyCode = responseBodyViewer.querySelector(
      ".audit-json-code",
    );
    for (const codeViewer of [
      requestHeadersCode,
      responseHeadersCode,
      requestBodyCode,
      responseBodyCode,
    ]) {
      expect(codeViewer).not.toBeNull();
      expect(codeViewer).toHaveClass("audit-json-code", "panel-scroll");
    }
    expect(requestHeadersCode).toHaveAttribute(
      "data-audit-json-size",
      "headers",
    );
    expect(responseHeadersCode).toHaveAttribute(
      "data-audit-json-size",
      "headers",
    );
    expect(requestBodyCode).toHaveAttribute("data-audit-json-size", "body");
    expect(responseBodyCode).toHaveAttribute("data-audit-json-size", "body");
    expect(requestHeadersCode?.getAttribute("data-audit-json-size")).toBe(
      responseHeadersCode?.getAttribute("data-audit-json-size"),
    );
    expect(requestBodyCode?.getAttribute("data-audit-json-size")).toBe(
      responseBodyCode?.getAttribute("data-audit-json-size"),
    );
    expect(requestHeadersCode?.getAttribute("data-audit-json-size")).not.toBe(
      requestBodyCode?.getAttribute("data-audit-json-size"),
    );

    expect(within(requestBodyViewer).getByText('"input"')).toHaveClass(
      "audit-json-token-key",
    );
    await userEvent.click(
      within(requestBodyViewer).getByRole("button", {
        name: "Collapse all Request body",
      }),
    );
    expect(within(requestBodyViewer).queryByText('"input"')).not.toBeInTheDocument();
    await userEvent.click(
      within(requestBodyViewer).getByRole("button", {
        name: "Expand all Request body",
      }),
    );
    expect(within(requestBodyViewer).getByText('"input"')).toHaveClass(
      "audit-json-token-key",
    );
    expect(within(dialog).queryByText("Stream events")).not.toBeInTheDocument();
    fireEvent.click(dialog);
    expect(
      screen.getByRole("dialog", { name: "Request details" }),
    ).toBeInTheDocument();
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close request details" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Request details" })).not.toBeInTheDocument(),
    );
  });

  it("forwards audit JSON wheel input only at vertical boundaries", async () => {
    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(
      screen.getByRole("button", { name: "View request details" }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Request details",
    });
    const responseBodyBlock = within(dialog)
      .getByText("Response body")
      .closest(".audit-json-block");
    const codeScroller = responseBodyBlock?.querySelector(
      ".audit-json-code",
    ) as HTMLElement | null;
    const detailScroller = codeScroller?.closest(
      ".overflow-y-auto",
    ) as HTMLElement | null;
    if (!codeScroller || !detailScroller) {
      throw new Error("Expected nested response JSON and detail scrollers");
    }

    detailScroller.style.overflowY = "auto";
    Object.defineProperties(detailScroller, {
      clientHeight: { configurable: true, value: 300 },
      scrollHeight: { configurable: true, value: 900 },
    });
    Object.defineProperties(codeScroller, {
      clientHeight: { configurable: true, value: 120 },
      scrollHeight: { configurable: true, value: 400 },
    });
    detailScroller.scrollTop = 200;

    codeScroller.scrollTop = 100;
    const internalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    codeScroller.dispatchEvent(internalWheel);
    expect(detailScroller.scrollTop).toBe(200);
    expect(internalWheel.defaultPrevented).toBe(false);

    codeScroller.scrollTop = 280;
    const bottomWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    codeScroller.dispatchEvent(bottomWheel);
    expect(detailScroller.scrollTop).toBe(240);
    expect(bottomWheel.defaultPrevented).toBe(true);

    codeScroller.scrollTop = 0;
    const topWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: -30,
    });
    codeScroller.dispatchEvent(topWheel);
    expect(detailScroller.scrollTop).toBe(210);
    expect(topWheel.defaultPrevented).toBe(true);

    codeScroller.scrollTop = 280;
    const horizontalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaX: 50,
      deltaY: 10,
    });
    codeScroller.dispatchEvent(horizontalWheel);
    expect(detailScroller.scrollTop).toBe(210);
    expect(horizontalWheel.defaultPrevented).toBe(false);
  });

  it("lets four-digit API audit page buttons grow with content", async () => {
    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            totalCount: 24680,
            totalPages: 1234,
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );

    const lastPageButton = await screen.findByRole("button", {
      name: "Go to page 1,234",
    });
    expect(lastPageButton).toHaveClass("h-9", "min-w-9", "px-2");
    expect(lastPageButton).not.toHaveClass("size-9");
  });

  it("uses semantic colors for API request status pills", async () => {
    const requests = ["succeeded", "failed", "running", "cancelled"].map(
      (finalState, index) => ({
        ...aiStatistics.requests[0],
        finalState,
        id: `request-status-${index}`,
      }),
    );

    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            page: 1,
            requests,
            totalCount: requests.length,
            totalPages: 1,
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    const table = await screen.findByRole("table");

    expect(await within(table).findByText("succeeded")).toHaveClass(
      "bg-[var(--success-soft)]",
      "text-[var(--success-soft-foreground)]",
    );
    expect(within(table).getByText("failed")).toHaveClass(
      "bg-[var(--danger-soft)]",
      "text-[var(--danger)]",
    );
    expect(within(table).getByText("running")).toHaveClass(
      "bg-[var(--warning-soft)]",
      "text-[var(--warning)]",
    );
    expect(within(table).getByText("cancelled")).toHaveClass(
      "bg-[var(--surface-secondary)]",
      "text-[var(--muted)]",
    );
  });

  it("shows wire-derived transport only after the provider name in request audit", async () => {
    const requests = (
      [
        ["req-http", "http"],
        ["req-ws", "websocket"],
        ["req-unknown", "unknown"],
      ] as const
    ).map(([id, transport]) => ({
      ...aiStatistics.requests[0],
      id,
      transport,
    }));

    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            page: 1,
            requests,
            totalCount: requests.length,
            totalPages: 1,
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    const table = await screen.findByRole("table");
    await waitFor(() =>
      expect(within(table).getAllByText("OpenAI")).toHaveLength(3),
    );

    expect(within(table).getByText("(HTTP)")).toBeInTheDocument();
    expect(within(table).getByText("(WebSocket)")).toBeInTheDocument();
    expect(within(table).getByText("(Unknown)")).toBeInTheDocument();

    const providerTitles = within(table)
      .getAllByText("OpenAI")
      .map((node) => node.closest("div")?.getAttribute("title"));
    expect(providerTitles).toEqual(
      expect.arrayContaining([
        "OpenAI(HTTP)",
        "OpenAI(WebSocket)",
        "OpenAI(Unknown)",
      ]),
    );

    const modelLines = within(table).getAllByText("GPT Test");
    expect(modelLines).toHaveLength(3);
    for (const modelLine of modelLines) {
      expect(modelLine.textContent).toBe("GPT Test");
      expect(modelLine.textContent).not.toMatch(/HTTP|WebSocket|Unknown/);
    }
    for (const suffix of ["(HTTP)", "(WebSocket)", "(Unknown)"]) {
      expect(
        within(table).queryByText(`GPT Test${suffix}`),
      ).not.toBeInTheDocument();
    }

    // Request type continues to mean requestKind; no transport filter control.
    expect(within(table).getAllByText("Chat completion")).toHaveLength(3);
    expect(
      screen.queryByRole("button", { name: /transport/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Request type/ }),
    ).toBeInTheDocument();
  });

  it("localizes provider-row transport suffixes and request kinds for zh-CN", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" },
    };
    const requests = (
      [
        ["req-http", "http"],
        ["req-ws", "websocket"],
        ["req-unknown", "unknown"],
      ] as const
    ).map(([id, transport]) => ({
      ...aiStatistics.requests[0],
      id,
      requestKind: "skill store translation",
      transport,
    }));
    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/settings") {
        return Promise.resolve(jsonResponse(zhSettings));
      }

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            page: 1,
            requests,
            totalCount: requests.length,
            totalPages: 1,
          }),
        );
      }

      if (path === "/api/workspaces/workspace-1/ai-statistics/req-http") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              requestKind: "skill store translation",
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API 详情" }))[0],
    );
    const table = await screen.findByRole("table");
    await waitFor(() =>
      expect(within(table).getAllByText("OpenAI")).toHaveLength(3),
    );

    expect(within(table).getByText("（HTTP）")).toBeInTheDocument();
    expect(within(table).getByText("（WebSocket）")).toBeInTheDocument();
    expect(within(table).getByText("（未知）")).toBeInTheDocument();

    const modelLines = within(table).getAllByText("GPT Test");
    expect(modelLines).toHaveLength(3);
    for (const modelLine of modelLines) {
      expect(modelLine.textContent).toBe("GPT Test");
      expect(modelLine.textContent).not.toMatch(/HTTP|WebSocket|未知/);
    }

    const requestTypeFilter = screen.getByRole("button", { name: /请求类型/ });
    await userEvent.click(requestTypeFilter);
    expect(
      await screen.findByRole("option", {
        name: "技能商店翻译",
      }),
    ).toHaveAttribute("data-key", "skill store translation");
    expect(within(table).getAllByText("技能商店翻译")).toHaveLength(3);
    await userEvent.keyboard("{Escape}");

    await userEvent.click(
      within(table).getAllByRole("button", { name: "查看请求详情" })[0],
    );
    const dialog = await screen.findByRole("dialog", { name: "请求详情" });
    expect(within(dialog).getByText("请求类型")).toBeInTheDocument();
    expect(within(dialog).getByText("技能商店翻译")).toBeInTheDocument();
  });

  it("forwards vertical request audit wheel input with a non-passive listener", async () => {
    const addEventListenerSpy = vi.spyOn(
      HTMLElement.prototype,
      "addEventListener",
    );
    const removeEventListenerSpy = vi.spyOn(
      HTMLElement.prototype,
      "removeEventListener",
    );
    window.history.replaceState(null, "", "/stats?page=1");

    const { unmount } = renderApp();

    expect(await screen.findByText("API details")).toBeInTheDocument();
    const table = await screen.findByRole("table");
    const tableScroller = table.parentElement;
    const statsScroller = table.closest(
      ".overflow-y-auto",
    ) as HTMLElement | null;
    if (!tableScroller || !statsScroller) {
      throw new Error(
        "Expected request audit table to live inside stats scroller",
      );
    }

    const wheelRegistrationIndex = addEventListenerSpy.mock.calls.findIndex(
      ([type], index) =>
        type === "wheel" &&
        addEventListenerSpy.mock.instances[index] === tableScroller,
    );
    expect(wheelRegistrationIndex).toBeGreaterThanOrEqual(0);
    const wheelRegistration =
      addEventListenerSpy.mock.calls[wheelRegistrationIndex];
    expect(wheelRegistration[2]).toEqual({ passive: false });

    statsScroller.style.overflowY = "auto";
    Object.defineProperties(statsScroller, {
      clientHeight: { configurable: true, value: 360 },
      scrollHeight: { configurable: true, value: 960 },
    });
    statsScroller.scrollTop = 10;

    const verticalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaMode: WheelEvent.DOM_DELTA_LINE,
      deltaX: 2,
      deltaY: 3,
    });
    tableScroller.dispatchEvent(verticalWheel);
    expect(statsScroller.scrollTop).toBe(58);
    expect(verticalWheel.defaultPrevented).toBe(true);

    const horizontalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaX: 20,
      deltaY: 5,
    });
    tableScroller.dispatchEvent(horizontalWheel);
    expect(statsScroller.scrollTop).toBe(58);
    expect(horizontalWheel.defaultPrevented).toBe(false);

    const wheelListener = wheelRegistration[1];
    unmount();
    expect(
      removeEventListenerSpy.mock.calls.some(
        ([type, listener], index) =>
          type === "wheel" &&
          listener === wheelListener &&
          removeEventListenerSpy.mock.instances[index] === tableScroller,
      ),
    ).toBe(true);
  });

  it("persists request kind filters through stats routing and pagination", async () => {
    window.history.replaceState(
      null,
      "",
      "/stats?page=2&requestKind=contextCompression",
    );

    renderApp();

    const requestTypeFilter = await screen.findByRole("button", { name: /Request type/ });
    expect(requestTypeFilter).toHaveAccessibleName(/Context compression.*Request type/);
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) =>
            url.searchParams.get("requestKind") === "contextCompression" &&
            url.searchParams.get("page") === "2",
        ),
      ).toBe(true),
    );

    await userEvent.click(screen.getByRole("button", { name: "Go to page 3" }));
    await waitFor(() => {
      const params = new URLSearchParams(window.location.search);
      expect(params.get("page")).toBe("3");
      expect(params.get("requestKind")).toBe("contextCompression");
    });
  });

  it("keeps request kind filters, badges, and detail metadata without the usage breakdown", async () => {
    const unknownKind = "background maintenance";
    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            requests: [
              {
                ...aiStatistics.requests[0],
                requestKind: unknownKind,
              },
            ],
            summary: {
              ...aiStatistics.summary,
              requestKindBreakdown: [
                ...aiStatistics.summary.requestKindBreakdown,
                {
                  averageLatencyMs: 300,
                  failedRequests: 1,
                  requestCount: 2,
                  requestKind: unknownKind,
                  totalCacheReadTokens: 0,
                  totalCacheWriteTokens: 0,
                  totalInputTokens: 90,
                  totalLatencyMs: 600,
                  totalOutputTokens: 10,
                  totalReasoningTokens: 0,
                  totalTokens: 100,
                },
              ],
            },
          }),
        );
      }

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              requestKind: unknownKind,
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );

    const tables = await screen.findAllByRole("table");
    const auditTable = tables.find((table) =>
      within(table).queryByRole("button", { name: "View request details" }),
    );
    expect(auditTable).toBeDefined();
    expect(within(auditTable as HTMLElement).getByText(unknownKind)).toBeInTheDocument();
    expect(screen.queryByText("Request usage breakdown")).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "Compression usage is additional model cost and does not change the Current context usage metric.",
      ),
    ).not.toBeInTheDocument();

    const requestTypeFilter = screen.getByRole("button", { name: /Request type/ });
    await userEvent.click(requestTypeFilter);
    expect(
      await screen.findByRole("option", {
        name: "Context compression",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: unknownKind }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");

    await userEvent.click(screen.getByRole("button", { name: "View request details" }));
    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(within(dialog).getByText("Request type")).toBeInTheDocument();
    expect(within(dialog).getByText(unknownKind)).toBeInTheDocument();
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
    expect(
      screen.getByRole("button", { name: "Go to page 2" }),
    ).toHaveAttribute("aria-current", "page");
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

  it("toggles the API details auto-refresh control between pause and resume", async () => {
    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    expect(await screen.findByText("API details")).toBeInTheDocument();

    const pauseButton = await screen.findByRole("button", {
      name: "Pause auto refresh",
    });
    expect(pauseButton.querySelector("svg")).toHaveClass("lucide-pause");
    expect(pauseButton).not.toBeDisabled();

    await userEvent.click(pauseButton);

    const resumeButton = await screen.findByRole("button", {
      name: "Resume auto refresh",
    });
    expect(resumeButton.querySelector("svg")).toHaveClass("lucide-play");
    expect(screen.queryByRole("button", { name: "Pause auto refresh" })).toBeNull();

    await userEvent.click(resumeButton);
    expect(
      await screen.findByRole("button", { name: "Pause auto refresh" }),
    ).toBeInTheDocument();
  });

  it("stops API statistics polling and visibility refresh while auto refresh is paused", async () => {
    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() => expect(aiStatisticsCallUrls().length).toBeGreaterThan(0));

    await userEvent.click(
      screen.getByRole("button", { name: "Pause auto refresh" }),
    );

    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    try {
      const pausedCount = aiStatisticsCallUrls().length;

      await act(async () => {
        await vi.advanceTimersByTimeAsync(15_000);
      });
      expect(aiStatisticsCallUrls().length).toBe(pausedCount);

      setDocumentVisibility("hidden");
      fireEvent(document, new Event("visibilitychange"));
      setDocumentVisibility("visible");
      fireEvent(document, new Event("visibilitychange"));
      expect(aiStatisticsCallUrls().length).toBe(pausedCount);
    } finally {
      vi.useRealTimers();
      setDocumentVisibility("visible");
    }

    const beforeResume = aiStatisticsCallUrls().length;
    await userEvent.click(
      screen.getByRole("button", { name: "Resume auto refresh" }),
    );
    await waitFor(() =>
      expect(aiStatisticsCallUrls().length).toBeGreaterThan(beforeResume),
    );
  });

  it("pauses running request detail polling and resumes after auto refresh starts", async () => {
    const fetchMock = vi.mocked(fetch);
    let detailCalls = 0;
    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            requests: [
              {
                ...aiStatistics.requests[0],
                finalState: "running",
              },
            ],
          }),
        );
      }

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        detailCalls += 1;
        const isFinal = detailCalls >= 3;
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              finalState: isFinal ? "succeeded" : "running",
              responseBody: isFinal
                ? aiStatisticsDetail.request.responseBody
                : null,
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "View request details" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    await waitFor(() => expect(detailCalls).toBeGreaterThanOrEqual(1));
    expect(within(dialog).getByText("running")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Pause auto refresh" }),
    );

    const pausedDetailCalls = detailCalls;
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 1500));
    });
    expect(detailCalls).toBe(pausedDetailCalls);

    await userEvent.click(
      screen.getByRole("button", { name: "Resume auto refresh" }),
    );
    await waitFor(
      () => {
        expect(detailCalls).toBeGreaterThan(pausedDetailCalls);
        expect(
          within(dialog).getByText("Final provider response"),
        ).toBeInTheDocument();
      },
      { timeout: 4000 },
    );
  });

  it("updates the stats URL when request audit pagination changes", async () => {
    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(window.location.pathname + window.location.search).toBe(
        "/stats?page=1",
      ),
    );

    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));

    await waitFor(() => {
      expect(window.location.pathname).toBe("/stats");
      expect(new URLSearchParams(window.location.search).get("page")).toBe("2");
    });
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) => url.searchParams.get("page") === "2",
        ),
      ).toBe(true),
    );
  });

  it("updates request audit pagination when browser navigation changes stats page", async () => {
    window.history.replaceState(null, "", "/stats?page=1");

    renderApp();

    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) => url.searchParams.get("page") === "1",
        ),
      ).toBe(true),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Go to page 1" }),
      ).toHaveAttribute("aria-current", "page"),
    );

    await act(async () => {
      window.history.pushState(null, "", "/stats?page=3");
      fireEvent.popState(window);
    });

    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) => url.searchParams.get("page") === "3",
        ),
      ).toBe(true),
    );
    expect(
      screen.getByRole("button", { name: "Go to page 3" }),
    ).toHaveAttribute("aria-current", "page");
  });

  it("polls running API request details every second until the final response arrives", async () => {
    const fetchMock = vi.mocked(fetch);
    let detailCalls = 0;
    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/settings") {
        return Promise.resolve(
          jsonResponse({
            ...settings,
            general: {
              ...settings.general,
              language: "zh-CN",
            },
          }),
        );
      }

      if (path === "/api/ai-statistics") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatistics,
            requests: [
              {
                ...aiStatistics.requests[0],
                finalState: "running",
              },
            ],
          }),
        );
      }

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        detailCalls += 1;
        const isFinal = detailCalls >= 2;
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              finalState: isFinal ? "succeeded" : "running",
              responseBody: isFinal ? aiStatisticsDetail.request.responseBody : null,
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API 详情" }))[0],
    );
    const table = await screen.findByRole("table");
    expect(within(table).getByText("运行中")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "查看请求详情" }));

    const dialog = await screen.findByRole("dialog", { name: "请求详情" });
    expect(within(dialog).getByText("状态")).toBeInTheDocument();
    expect(within(dialog).getByText("运行中")).toBeInTheDocument();
    expect(within(dialog).getByText("正在等待供应商最终响应…")).toBeInTheDocument();
    await waitFor(
      () => {
        expect(detailCalls).toBeGreaterThanOrEqual(2);
        expect(within(dialog).getByText("供应商最终响应")).toBeInTheDocument();
        expect(within(dialog).getByText('"Done."')).toBeInTheDocument();
      },
      { timeout: 2500 },
    );
  });

  it("distinguishes malformed, failed partial, and pruned API request details", async () => {
    const fetchMock = vi.mocked(fetch);
    let detailMode: "failed" | "malformed" | "pruned" = "malformed";
    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        const request =
          detailMode === "malformed"
            ? {
                ...aiStatisticsDetail.request,
                requestBody: { messages: [{ content: "legacy" }] },
                requestDetailStatus: "malformed",
                responseBody: { text: "legacy response" },
                responseDetailStatus: "malformed",
              }
            : detailMode === "failed"
              ? {
                  ...aiStatisticsDetail.request,
                  finalState: "failed",
                  responseBody: {
                    error: "connection reset",
                    format: "provider_final_response_v1",
                    http: {
                      headers: {
                        authorization: ["********"],
                        "retry-after": ["1"],
                      },
                      status: 502,
                      version: "HTTP/1.1",
                    },
                    partial: true,
                    state: "failed",
                    statusCode: 502,
                    version: 1,
                  },
                  responseDetailStatus: "partial",
                }
              : {
                  ...aiStatisticsDetail.request,
                  finalState: "succeeded",
                  requestBody: null,
                  requestDetailStatus: "unavailable",
                  responseBody: null,
                  responseDetailStatus: "unavailable",
                };
        return Promise.resolve(jsonResponse({ ...aiStatisticsDetail, request }));
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(screen.getByRole("button", { name: "View request details" }));
    let dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(
      within(dialog).getByText("Stored request detail is malformed or unsupported."),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText("Stored response detail is malformed or unsupported."),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("legacy response")).not.toBeInTheDocument();

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close request details" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Request details" })).not.toBeInTheDocument(),
    );
    detailMode = "failed";
    await userEvent.click(screen.getByRole("button", { name: "View request details" }));
    dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(
      within(dialog).getByText(
        /The stream ended before a complete response was received\./,
      ),
    ).toBeInTheDocument();
    expect(within(dialog).getByText('"connection reset"')).toBeInTheDocument();
    const failedHttpStatusMeta = within(dialog)
      .getByText("HTTP status")
      .parentElement as HTMLElement;
    expect(within(failedHttpStatusMeta).getByText("502")).toBeInTheDocument();
    expect(within(dialog).getByText("HTTP/1.1")).toBeInTheDocument();
    const failedResponseHeaders = within(dialog)
      .getByText("Response headers")
      .closest(".audit-json-block");
    expect(failedResponseHeaders).not.toBeNull();
    expect(
      within(failedResponseHeaders as HTMLElement).getByText('"retry-after"'),
    ).toBeInTheDocument();
    expect(
      within(failedResponseHeaders as HTMLElement).getByText('"1"'),
    ).toBeInTheDocument();

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close request details" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Request details" })).not.toBeInTheDocument(),
    );
    detailMode = "pruned";
    await userEvent.click(screen.getByRole("button", { name: "View request details" }));
    dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(
      within(dialog).getByText("Request detail was not captured or was pruned."),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText("Final response detail is unavailable or was pruned."),
    ).toBeInTheDocument();
  });

  it("shows historical provider_final_response_v1 envelopes and unavailable cancelled details", async () => {
    const fetchMock = vi.mocked(fetch);
    let responseMode: "cancelled" | "historical" = "historical";
    const historicalResponse = {
      format: "provider_final_response_v1",
      reasoning: "Historical reasoning.",
      responseId: "resp-historical",
      state: "succeeded",
      stopReason: "stop",
      text: "Historical response.",
      toolCalls: [],
      usage: null,
      version: 1,
    };

    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              finalState: responseMode === "cancelled" ? "cancelled" : "succeeded",
              responseBody: responseMode === "cancelled" ? null : historicalResponse,
              responseDetailStatus:
                responseMode === "cancelled" ? "unavailable" : "captured",
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(
      screen.getByRole("button", { name: "View request details" }),
    );
    let dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(
      within(dialog).getByText(
        "Response head was not captured for this historical record.",
      ),
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByText("Response headers"),
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getByText('"Historical response."'),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText('"Historical reasoning."'),
    ).toBeInTheDocument();

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close request details" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Request details" })).not.toBeInTheDocument(),
    );
    responseMode = "cancelled";
    await userEvent.click(
      screen.getByRole("button", { name: "View request details" }),
    );
    dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(
      within(dialog).getByText("Final response detail is unavailable or was pruned."),
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByText("Response headers"),
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('"cancelled"')).not.toBeInTheDocument();
  });

  it("renders and copies a failed provider stream diagnostic separately from the response envelope", async () => {
    const fetchMock = vi.mocked(fetch);
    const streamDiagnostic = {
      eventType: "response.failed",
      kind: "response_failed",
      payload: {
        kind: "json",
        value: {
          error: "malformed upstream frame",
          type: "response.failed",
        },
      },
      previousEventSequence: 4,
      previousEventType: "response.created",
      providerError: {
        code: "rate_limit",
        errorType: "rate_limit_error",
        message: "retry later",
        param: "model",
      },
      payloadTruncated: false,
      transport: "http_sse",
      rawPayloadBytes: 183,
      rawPayloadSha256: "14f8c7d1e1d9a3c5",
    };

    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              finalState: "failed",
              responseBody: {
                error: "provider stream failed",
                format: "provider_final_response_v1",
                http: null,
                partial: false,
                state: "failed",
                statusCode: 200,
                streamDiagnostic,
                version: 1,
              },
              responseDetailStatus: "failed",
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(screen.getByRole("button", { name: "View request details" }));

    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    const diagnosticCard = within(dialog)
      .getByText("Stream diagnostic")
      .closest("section");
    expect(diagnosticCard).not.toBeNull();
    const diagnostic = within(diagnosticCard as HTMLElement);
    expect(diagnostic.getByText("response_failed")).toBeInTheDocument();
    expect(diagnostic.getByText("http_sse")).toBeInTheDocument();
    expect(diagnostic.getByText("response.failed")).toBeInTheDocument();
    expect(diagnostic.getByText("rate_limit")).toBeInTheDocument();
    expect(diagnostic.getByText("retry later")).toBeInTheDocument();
    const frameBytesMeta = diagnostic.getByText("Frame bytes")
      .parentElement as HTMLElement;
    expect(within(frameBytesMeta).getByText("183")).toBeInTheDocument();
    expect(diagnostic.getByText("14f8c7d1e1d9a3c5")).toBeInTheDocument();
    const payloadExcerpt = diagnostic
      .getByText("Payload excerpt")
      .closest(".audit-json-block");
    expect(payloadExcerpt).not.toBeNull();
    expect(
      within(payloadExcerpt as HTMLElement).getByText(
        '"malformed upstream frame"',
      ),
    ).toBeInTheDocument();

    const diagnosticJson = diagnostic
      .getByText("Diagnostic JSON")
      .closest(".audit-json-block");
    expect(diagnosticJson).not.toBeNull();
    await userEvent.click(
      within(diagnosticJson as HTMLElement).getByRole("button", {
        name: "Copy Diagnostic JSON",
      }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      JSON.stringify(streamDiagnostic, null, 2),
    );
  });

  it("labels an audit-compacted diagnostic separately from the original failure frame", async () => {
    const fetchMock = vi.mocked(fetch);
    const streamDiagnostic = {
      originalBytes: 48192,
      sha256: "a7c96af2ef467b9a",
      truncated: true,
    };

    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              finalState: "failed",
              responseBody: {
                error: "provider stream failed",
                format: "provider_final_response_v1",
                http: null,
                partial: false,
                state: "failed",
                statusCode: 200,
                streamDiagnostic,
                version: 1,
              },
              responseDetailStatus: "failed",
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await userEvent.click(screen.getByRole("button", { name: "View request details" }));

    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    const diagnosticCard = within(dialog)
      .getByText("Stream diagnostic")
      .closest("section");
    expect(diagnosticCard).not.toBeNull();
    const diagnostic = within(diagnosticCard as HTMLElement);
    expect(
      diagnostic.getByText("The diagnostic was compacted for audit storage."),
    ).toBeInTheDocument();
    expect(diagnostic.getByText("Stored diagnostic bytes")).toBeInTheDocument();
    const storedBytesMeta = diagnostic.getByText("Stored diagnostic bytes")
      .parentElement as HTMLElement;
    expect(within(storedBytesMeta).getByText("48192")).toBeInTheDocument();
    const storedHashMeta = diagnostic.getByText("Stored diagnostic SHA-256")
      .parentElement as HTMLElement;
    expect(
      within(storedHashMeta).getByText("a7c96af2ef467b9a"),
    ).toBeInTheDocument();
    expect(diagnostic.queryByText("Frame bytes")).not.toBeInTheDocument();
  });

  it("renders provider_websocket_request_v1 create frame and connection reuse", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(
          jsonResponse({
            ...aiStatisticsDetail,
            request: {
              ...aiStatisticsDetail.request,
              transport: "websocket",
              requestBody: {
                connectionReused: true,
                createFrame: JSON.stringify({
                  type: "response.create",
                  model: "gpt-test",
                }),
                createFrameEncoding: "utf8",
                frameSent: true,
                format: "provider_websocket_request_v1",
                headers: {
                  authorization: ["********"],
                },
                url: "wss://api.example.test/v1/responses",
                version: 1,
              },
              requestDetailStatus: "captured",
              responseBody: {
                ...aiStatisticsDetail.request.responseBody,
                http: null,
              },
              statusCode: null,
            },
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "View request details" })).toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "View request details" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    // Overview + wire request card both surface Transport=WebSocket.
    expect(within(dialog).getAllByText("WebSocket").length).toBeGreaterThanOrEqual(
      2,
    );
    expect(within(dialog).getAllByText("Transport").length).toBeGreaterThanOrEqual(
      2,
    );
    expect(
      within(dialog).getByText("wss://api.example.test/v1/responses"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("Connection reused")).toBeInTheDocument();
    expect(within(dialog).getByText("Frame sent")).toBeInTheDocument();
    expect(within(dialog).getAllByText("Yes").length).toBeGreaterThanOrEqual(2);
    expect(within(dialog).getByText("response.create frame")).toBeInTheDocument();
    expect(within(dialog).getByText('"response.create"')).toBeInTheDocument();
    expect(within(dialog).queryByText("HTTP method")).not.toBeInTheDocument();
  });

  it("loads saved API request audit column settings", async () => {
    const { unmount } = renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    const table = await screen.findByRole("table");
    await waitFor(() =>
      expect(within(table).getByText("OpenAI")).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Provider / model" }),
    );
    expect(within(table).queryByText("OpenAI")).not.toBeInTheDocument();
    await waitFor(() => {
      const savedColumns = JSON.parse(
        window.localStorage.getItem("foco.aiStats.visibleColumns") ?? "[]",
      );
      expect(savedColumns).not.toContain("providerModel");
    });

    unmount();
    window.history.replaceState(null, "", "/");
    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    const reloadedTable = await screen.findByRole("table");
    expect(within(reloadedTable).queryByText("OpenAI")).not.toBeInTheDocument();
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
      fetchMock.mock.calls.some((call) =>
        String(call[0]).includes("/files/children"),
      ),
    ).toBe(false);

    const pagesRow = screen.getByText("pages").closest("div[role='treeitem']");
    expect(pagesRow).not.toBeNull();
    await userEvent.click(
      within(pagesRow as HTMLElement).getByRole("button", {
        name: "Expand folder",
      }),
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
    expect(
      within(tabList).getByRole("tab", { name: /main.ts/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(new URLSearchParams(window.location.search).getAll("file")).toEqual([
      "workspace-1/src%2Fmain.ts",
    ]);
    expect(new URLSearchParams(window.location.search).get("activeFile")).toBe(
      "workspace-1/src%2Fmain.ts",
    );

    vi.mocked(fetch).mockClear();
    unmount();
    renderApp();

    const restoredTabList = await screen.findByRole("tablist", {
      name: "Chat",
    });
    await waitFor(() =>
      expect(
        within(restoredTabList).getByRole("tab", { name: /main.ts/ }),
      ).toHaveAttribute("aria-selected", "true"),
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

  it("opens image files from the workspace file tree without Monaco", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    fetchMock.mockClear();

    await userEvent.click(await screen.findByText("logo.png"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(
      within(tabList).getByRole("tab", { name: /logo\.png/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(
      screen.queryByRole("toolbar", { name: "Editor toolbar" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "logo.png" })).toHaveAttribute(
      "src",
      "/api/workspaces/workspace-1/files/blob?path=assets%2Flogo.png",
    );
    expect(
      fetchMock.mock.calls.some(
        ([url]) => url === "/api/workspaces/workspace-1/files/content",
      ),
    ).toBe(false);
  });

  it("copies file tree context menu values", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));

    const componentsRow = screen
      .getByText("components")
      .closest("div[role='treeitem']");
    expect(componentsRow).not.toBeNull();
    await userEvent.click(
      within(componentsRow as HTMLElement).getByRole("button", {
        name: "Expand folder",
      }),
    );

    const fileRow = (await screen.findByText("button.tsx")).closest(
      "div[role='treeitem']",
    );
    expect(fileRow).not.toBeNull();

    fireEvent.contextMenu(fileRow as HTMLElement);
    const menu = await screen.findByRole("menu", { name: "button.tsx" });
    for (const item of [
      "Open",
      "Download",
      "Rename",
      "Delete",
      "Copy file name",
      "Copy relative path",
      "Copy absolute path",
    ]) {
      expect(
        within(menu).getByRole("menuitem", { name: item }),
      ).toBeInTheDocument();
    }

    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Copy file name" }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      "button.tsx",
    );

    fireEvent.contextMenu(fileRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "button.tsx" })).getByRole(
        "menuitem",
        {
          name: "Copy relative path",
        },
      ),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      "src/components/button.tsx",
    );

    fireEvent.contextMenu(fileRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "button.tsx" })).getByRole(
        "menuitem",
        {
          name: "Copy absolute path",
        },
      ),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      `${workspace.path}\\src\\components\\button.tsx`,
    );
  });

  it("downloads a file from the file tree context menu", async () => {
    const createdAnchors: HTMLAnchorElement[] = [];
    const originalCreateElement = document.createElement.bind(document);
    const createElementSpy = vi
      .spyOn(document, "createElement")
      .mockImplementation((tagName: string, options?: ElementCreationOptions) => {
        const element = originalCreateElement(tagName, options);
        if (tagName.toLowerCase() === "a") {
          createdAnchors.push(element as HTMLAnchorElement);
          vi.spyOn(element as HTMLAnchorElement, "click").mockImplementation(() => {});
        }
        return element;
      });

    try {
      renderApp();

      await screen.findAllByText("Default");
      await userEvent.click(screen.getByRole("tab", { name: "Files" }));

      const componentsRow = screen
        .getByText("components")
        .closest("div[role='treeitem']");
      expect(componentsRow).not.toBeNull();
      await userEvent.click(
        within(componentsRow as HTMLElement).getByRole("button", {
          name: "Expand folder",
        }),
      );

      const fileRow = (await screen.findByText("button.tsx")).closest(
        "div[role='treeitem']",
      );
      expect(fileRow).not.toBeNull();
      fireEvent.contextMenu(fileRow as HTMLElement);

      const fileMenu = await screen.findByRole("menu", { name: "button.tsx" });
      await userEvent.click(
        within(fileMenu).getByRole("menuitem", { name: "Download" }),
      );

      expect(createdAnchors).toHaveLength(1);
      expect(createdAnchors[0]?.getAttribute("href")).toBe(
        "/api/workspaces/workspace-1/files/download?path=src%2Fcomponents%2Fbutton.tsx",
      );
      expect(createdAnchors[0]?.download).toBe("button.tsx");
      expect(createdAnchors[0]?.click).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole("menu", { name: "button.tsx" })).not.toBeInTheDocument();

      const directoryRow = screen
        .getByText("components")
        .closest("div[role='treeitem']");
      expect(directoryRow).not.toBeNull();
      fireEvent.contextMenu(directoryRow as HTMLElement);
      const directoryMenu = await screen.findByRole("menu", {
        name: "components",
      });
      expect(
        within(directoryMenu).queryByRole("menuitem", { name: "Download" }),
      ).not.toBeInTheDocument();
    } finally {
      createElementSpy.mockRestore();
    }
  });

  it("closes the file tree context menu when the context panel scrolls", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));

    const fileRow = (await screen.findByText("README.md")).closest(
      "div[role='treeitem']",
    );
    expect(fileRow).not.toBeNull();
    fireEvent.contextMenu(fileRow as HTMLElement);
    expect(await screen.findByRole("menu", { name: "README.md" })).toBeInTheDocument();

    const fileTree = document.querySelector(".workspace-file-tree");
    const fileScrollContainer = fileTree?.closest(".panel-scroll");
    if (!(fileScrollContainer instanceof HTMLElement)) {
      throw new Error("Expected file tree scroll container inside context panel");
    }
    expect(fileScrollContainer.closest(".context-panel")).not.toBeNull();
    fireEvent.scroll(fileScrollContainer);

    expect(screen.queryByRole("menu", { name: "README.md" })).not.toBeInTheDocument();
  });

  it("clamps the file tree context menu into the viewport", async () => {
    const originalInnerWidth = window.innerWidth;
    const originalInnerHeight = window.innerHeight;
    const menuWidth = 220;
    const menuHeight = 280;
    const clientX = 780;
    const clientY = 560;
    const margin = 8;

    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 800,
    });
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 600,
    });

    const originalGetBoundingClientRect =
      HTMLElement.prototype.getBoundingClientRect;
    const getBoundingClientRectSpy = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        if (this.classList.contains("workspace-file-context-menu")) {
          return {
            bottom: menuHeight,
            height: menuHeight,
            left: 0,
            right: menuWidth,
            top: 0,
            toJSON: () => ({}),
            width: menuWidth,
            x: 0,
            y: 0,
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      });

    try {
      renderApp();

      await screen.findAllByText("Default");
      await userEvent.click(screen.getByRole("tab", { name: "Files" }));

      const fileRow = (await screen.findByText("README.md")).closest(
        "div[role='treeitem']",
      );
      expect(fileRow).not.toBeNull();
      fireEvent.contextMenu(fileRow as HTMLElement, { clientX, clientY });

      const menu = await screen.findByRole("menu", { name: "README.md" });
      const expectedLeft = Math.max(
        margin,
        Math.min(clientX, 800 - menuWidth - margin),
      );
      const expectedTop = Math.max(
        margin,
        Math.min(clientY, 600 - menuHeight - margin),
      );

      // Clamp measures the popover (className hook); trigger receives clamped left/top.
      const popover = document.querySelector(
        ".workspace-file-context-menu",
      ) as HTMLElement | null;
      expect(popover).not.toBeNull();
      await waitFor(() => {
        expect(popover).toHaveStyle({ visibility: "visible" });
        const trigger = document.querySelector(
          '[data-slot="dropdown-trigger"]',
        ) as HTMLElement | null;
        expect(trigger).toHaveStyle({
          left: `${expectedLeft}px`,
          top: `${expectedTop}px`,
        });
      });
      expect(menu).toBeInTheDocument();
      expect(expectedLeft).toBe(572);
      expect(expectedTop).toBe(312);
      expect(expectedLeft + menuWidth).toBeLessThanOrEqual(800 - margin);
      expect(expectedTop + menuHeight).toBeLessThanOrEqual(600 - margin);
    } finally {
      getBoundingClientRectSpy.mockRestore();
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
    expect(
      previewButton.querySelector(".lucide-eye-off"),
    ).not.toBeInTheDocument();
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
    expect(screen.queryByText(/<\/?div/i)).not.toBeInTheDocument();
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
    const markdownPreview = document.querySelector<HTMLDivElement>(
      ".workspace-file-markdown-preview",
    );
    if (!markdownPreview) {
      throw new Error("Expected markdown preview container");
    }
    markdownPreview.scrollTop = 160;

    await userEvent.click(await screen.findByText("index.html"));
    const tabList = screen.getByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabList).getByRole("tab", { name: /README\.md/ }),
    );

    expect(
      await screen.findByRole("heading", { name: "Preview title" }),
    ).toBeInTheDocument();
    const restoredEditButton = screen.getByRole("button", {
      name: "Edit markdown",
    });
    expect(restoredEditButton).toHaveAttribute("aria-pressed", "true");
    expect(
      document.querySelector<HTMLDivElement>(".workspace-file-markdown-preview")
        ?.scrollTop,
    ).toBe(160);

    await userEvent.click(restoredEditButton);
    expect(
      screen.queryByRole("heading", { name: "Preview title" }),
    ).not.toBeInTheDocument();
  });

  it("opens an HTML preview tab from the file tree and restores it from the URL", async () => {
    const fetchMock = vi.mocked(fetch);
    const { unmount } = renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));

    const htmlRow = (await screen.findByText("index.html")).closest(
      "div[role='treeitem']",
    );
    expect(htmlRow).not.toBeNull();
    fireEvent.contextMenu(htmlRow as HTMLElement);
    const menu = await screen.findByRole("menu", { name: "index.html" });
    expect(
      within(menu).getByRole("menuitem", { name: "Preview in new tab" }),
    ).toBeInTheDocument();
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Preview in new tab" }),
    );

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const previewTab = within(tabList).getByRole("tab", {
      name: /index\.html/,
    });
    expect(previewTab).toHaveAttribute("aria-selected", "true");
    expect(previewTab).toHaveAttribute("title", "index.html · Preview");

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([url, init]) =>
            String(url).includes("/preview/sessions") &&
            (!init || init.method === "POST" || !init.method),
        ),
      ).toBe(true),
    );

    expect(
      await screen.findByTitle("HTML preview for index.html"),
    ).toHaveAttribute(
      "src",
      expect.stringMatching(/^http:\/\/previewtoken.+\.preview\.localhost:3210\/index\.html$/),
    );
    expect(screen.getByTitle("HTML preview for index.html")).toHaveAttribute(
      "sandbox",
      "allow-scripts allow-same-origin",
    );

    expect(new URLSearchParams(window.location.search).getAll("preview")).toEqual([
      "workspace-1/demo%2Findex.html",
    ]);
    expect(new URLSearchParams(window.location.search).get("activePreview")).toBe(
      "workspace-1/demo%2Findex.html",
    );

    // Reuse: opening the same path focuses the existing tab and does not stack duplicates.
    fireEvent.contextMenu(htmlRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "index.html" })).getByRole(
        "menuitem",
        { name: "Preview in new tab" },
      ),
    );
    expect(
      within(tabList).getAllByRole("tab", { name: /index\.html/ }),
    ).toHaveLength(1);

    fetchMock.mockClear();
    unmount();
    renderApp();

    const restoredTabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() =>
      expect(
        within(restoredTabList).getByRole("tab", { name: /index\.html/ }),
      ).toHaveAttribute("aria-selected", "true"),
    );
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some((call) =>
          String(call[0]).includes("/api/workspaces/workspace-1/preview/sessions"),
        ),
      ).toBe(true),
    );
    expect(
      await screen.findByTitle("HTML preview for index.html"),
    ).toBeInTheDocument();
  });

  it("closes HTML preview tabs and releases the session", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    const htmlRow = (await screen.findByText("index.html")).closest(
      "div[role='treeitem']",
    );
    expect(htmlRow).not.toBeNull();
    fireEvent.contextMenu(htmlRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "index.html" })).getByRole(
        "menuitem",
        { name: "Preview in new tab" },
      ),
    );

    expect(await screen.findByTitle("HTML preview for index.html")).toBeInTheDocument();
    await waitFor(() => expect(appTestState.activePreviewSessions.length).toBe(1));
    const token = appTestState.activePreviewSessions[0]?.token;
    expect(token).toBeTruthy();
    fetchMock.mockClear();

    const tabList = screen.getByRole("tablist", { name: "Chat" });
    const previewTab = within(tabList).getByRole("tab", { name: /index\.html/ });
    const tabGroup = previewTab.closest("div.group") ?? previewTab.parentElement;
    expect(tabGroup).not.toBeNull();
    await userEvent.click(
      within(tabGroup as HTMLElement).getByRole("button", {
        name: "Close chat tab index.html · Preview",
      }),
    );

    await waitFor(() =>
      expect(
        within(tabList).queryByRole("tab", { name: /index\.html/ }),
      ).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([url, init]) =>
            String(url) ===
              `/api/workspaces/workspace-1/preview/sessions/${token}` &&
            init?.method === "DELETE",
        ),
      ).toBe(true),
    );
    await waitFor(() =>
      expect(appTestState.activePreviewSessions.some((session) => session.token === token))
        .toBe(false),
    );
    expect(new URLSearchParams(window.location.search).getAll("preview")).toEqual([]);
  });

  it("keeps HTML preview sessions alive when switching main tabs", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    // Open a file tab so we can switch away from the preview without closing it.
    await userEvent.click(await screen.findByText("README.md"));
    const htmlRow = (await screen.findByText("index.html")).closest(
      "div[role='treeitem']",
    );
    expect(htmlRow).not.toBeNull();
    fireEvent.contextMenu(htmlRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "index.html" })).getByRole(
        "menuitem",
        { name: "Preview in new tab" },
      ),
    );

    const iframe = await screen.findByTitle("HTML preview for index.html");
    expect(iframe).toBeInTheDocument();
    await waitFor(() => expect(appTestState.activePreviewSessions.length).toBe(1));
    const token = appTestState.activePreviewSessions[0]?.token;
    expect(token).toBeTruthy();
    fetchMock.mockClear();

    const tabList = screen.getByRole("tablist", { name: "Chat" });
    await userEvent.click(within(tabList).getByRole("tab", { name: /README\.md/ }));

    // Keep-alive: panel stays mounted (hidden) and does not DELETE the session.
    expect(screen.getByTitle("HTML preview for index.html")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(
        ([url, init]) =>
          String(url).includes("/preview/sessions/") && init?.method === "DELETE",
      ),
    ).toBe(false);
    expect(
      appTestState.activePreviewSessions.some((session) => session.token === token),
    ).toBe(true);

    await userEvent.click(within(tabList).getByRole("tab", { name: /index\.html/ }));
    expect(screen.getByTitle("HTML preview for index.html")).toBeInTheDocument();
    expect(
      appTestState.activePreviewSessions.some((session) => session.token === token),
    ).toBe(true);
    expect(
      fetchMock.mock.calls.some(
        ([url, init]) =>
          String(url).includes("/preview/sessions/") && init?.method === "DELETE",
      ),
    ).toBe(false);
  });

  it("reloads the active file from the leftmost editor toolbar button", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    await userEvent.click(await screen.findByText("README.md"));

    const toolbar = await screen.findByRole("toolbar", {
      name: "Editor toolbar",
    });
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
