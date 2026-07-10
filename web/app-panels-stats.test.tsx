import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
    appTestState.workspaceGitBranchesResponses = [branchesResponse, branchesResponse];
    appTestState.workspaceGitDiffResponsesByWorktreePath[worktreePath] = generatedGitDiff;

    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Git" }));
    expect(
      await screen.findAllByText("foco/agent-worktrees/agent-instance-coordinator"),
    ).not.toHaveLength(0);
    await waitFor(() =>
      expect(
        screen.getByRole("combobox", { name: "Source Control target" }),
      ).toHaveValue(`worktree:${worktreePath}`),
    );
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
    appTestState.workspaceGitBranchesResponses = [branchesResponse, branchesResponse];
    appTestState.workspaceGitDiffResponsesByWorktreePath[firstWorktreePath] = generatedGitDiff;
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
    await waitFor(() =>
      expect(
        screen.getByRole("combobox", { name: "Source Control target" }),
      ).toHaveValue(`worktree:${firstWorktreePath}`),
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Source Control target" }),
      `worktree:${secondWorktreePath}`,
    );

    await screen.findByText("review.md");
    expect(
      fetchCallUrls().some(
        (url) => url.pathname === "/api/workspaces/workspace-1/git/branches/switch",
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

  it("expands plan phases and opens the implementation chat after start", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T04:30:00Z";
    const phaseStep = {
      acceptance: ["Start queues a team chat."],
      checkedAt: null,
      createdAt: timestamp,
      detail: "The workspace chat list shows the created implementation session.",
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
      overview: "Run the implementation through normal visible chats.",
      pauseRequestedAt: null,
      phases: [pendingPhase],
      sortOrder: 0,
      sourceChatId: "chat-1",
      status: "ready",
      title: "Build plan runner UI",
      updatedAt: timestamp,
    };
    const runningPlan = {
      ...readyPlan,
      activePhaseId: "plan-phase-1",
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
      status: "running",
    };
    let didStartPlan = false;
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
        workspaceId: "workspace-1",
      },
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces") {
        return jsonResponse({
          activeWorkspaceId: workspace.id,
          workspaces: [
            {
              ...workspace,
              chats: didStartPlan ? [planChat, ...workspace.chats] : workspace.chats,
            },
            secondaryWorkspace,
          ],
        });
      }

      if (path === "/api/workspaces/workspace-1/plans") {
        const plan = didStartPlan ? runningPlan : readyPlan;
        return jsonResponse({
          page: 1,
          pageSize: 50,
          plans: [plan],
          totalCount: 1,
          totalPages: 1,
        });
      }

      if (path === "/api/workspaces/workspace-1/plans/plan-1/action") {
        didStartPlan = true;
        return jsonResponse({ plan: runningPlan });
      }

      if (path === "/api/workspaces/workspace-1/chats/plan-chat-1/messages") {
        return jsonResponse({
          activeRun: {
            chatId: "plan-chat-1",
            lastSequence: 0,
            runId: "agent-task-plan-1",
            workspaceId: "workspace-1",
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
              parts: [{ text: "Plan phase implementation request.", type: "text" }],
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Build plan runner UI")).toBeInTheDocument();
    expect(screen.queryByText("Wire start action")).not.toBeInTheDocument();

    const phaseButton = (await screen.findByText("Phase 1")).closest("button");
    if (!phaseButton) {
      throw new Error("Expected phase row button");
    }
    await user.click(phaseButton);

    expect(await screen.findByText("Wire start action")).toBeInTheDocument();
    expect(screen.getByText("Start queues a team chat.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/plans/plan-1/action",
        expect.objectContaining({ method: "POST" }),
      );
    });
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    expect(
      await within(workspaceList).findByText("Plan phase implementation"),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("Plan phase implementation request."),
    ).toBeInTheDocument();
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/chats/plan-chat-open/messages") {
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
              parts: [{ text: "Existing implementation transcript.", type: "text" }],
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Open implementation chat plan")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open implementation chat" }));

    expect(await screen.findByText("Existing implementation transcript.")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/chats/plan-chat-open/messages?limit=100",
      expect.any(Object),
    );
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Delete me from plan panel")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete plan" }));
    expect(confirmSpy).toHaveBeenCalledWith("Delete plan confirmation");
    expect(
      fetchMock.mock.calls.filter(([input, init]) => {
        const path = new URL(String(input), "http://127.0.0.1").pathname;
        return path === "/api/workspaces/workspace-1/plans/plan-delete-ui" && init?.method === "DELETE";
      }),
    ).toHaveLength(0);
    expect(screen.getByText("Delete me from plan panel")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete plan" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(([input, init]) => {
          const path = new URL(String(input), "http://127.0.0.1").pathname;
          return path === "/api/workspaces/workspace-1/plans/plan-delete-ui" && init?.method === "DELETE";
        }),
      ).toHaveLength(1);
    });
    await waitFor(() => {
      expect(screen.queryByText("Delete me from plan panel")).not.toBeInTheDocument();
    });
    expect(screen.getByText("No active plans for this workspace.")).toBeInTheDocument();
  });

  it.each(["failed", "cancelled"] as const)("retries a %s plan phase through the phase retry endpoint", async (phaseStatus) => {
    const user = userEvent.setup();
    const timestamp = "2026-06-28T05:00:00Z";
    const failedStep = {
      acceptance: ["Retry uses the phase retry endpoint."],
      checkedAt: null,
      createdAt: timestamp,
      detail: "The Plan runner should see the same task complete after retry.",
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry") {
        didRetry = true;
        return jsonResponse({ plan: retriedPlan });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Retry failed plan phase")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Retry plan phase" }));

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
  });


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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry") {
        return jsonResponse({ plan: retriedPlan });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    await user.click(screen.getByRole("button", { name: "Retry phase options" }));
    await user.click(screen.getByRole("menuitem", { name: "Retry with another model..." }));

    expect(screen.getByRole("dialog", { name: "Retry with another model" })).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Provider"), "anthropic");
    await user.selectOptions(screen.getByLabelText("Thinking level"), "high");
    await user.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/plans/plan-failed/phases/plan-phase-failed/retry",
        expect.objectContaining({
          body: JSON.stringify({
            modelId: "gpt-test",
            providerId: "anthropic",
            thinkingLevel: "high",
          }),
          method: "POST",
        }),
      );
    });
  });


  it("shows the auto-run checkbox when the plan list is empty", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Plan" }));

    expect(
      await screen.findByRole("checkbox", { name: /Auto run plans/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("Run every active plan in order")).toBeInTheDocument();
    expect(screen.getByText("No active plans for this workspace.")).toBeInTheDocument();
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    const rect = (top: number, height: number) => ({
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
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this.classList.contains("context-list-panel") ? 200 : 0;
    });
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
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
    });

    renderApp();

    const planTab = await screen.findByRole("tab", { name: "Plan" });
    const scrollIntoView = vi.mocked(HTMLElement.prototype.scrollIntoView);
    scrollIntoView.mockClear();

    await userEvent.click(planTab);

    const runningTitle = await screen.findByText("Running scroll target");
    const planListPanel = runningTitle.closest(".context-list-panel") as HTMLElement | null;
    expect(planListPanel).not.toBeNull();
    await waitFor(() => {
      expect(planListPanel?.scrollTop).toBe(450);
    });
    expect(screen.getByText("Ready scroll decoy")).toBeInTheDocument();
    expect(
      scrollIntoView.mock.contexts.some(
        (context) =>
          context instanceof HTMLElement &&
          context.textContent?.includes("Running scroll target"),
      ),
    ).toBe(false);
  });

  it("toggles the plan worktree audit view back to the plan list", async () => {
    const user = userEvent.setup();
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/plans/worktrees/audit") {
        return jsonResponse({
          items: [
            {
              agentInstanceId: "agent-instance-audit",
              agentTaskId: "agent-task-audit",
              agentTaskStatus: "completed",
              baseRevision: "main",
              branch: "foco/plan-audit",
              cleanupAllowed: true,
              commitId: "abcdef1234567890",
              errorMessage: null,
              headCommitId: "abcdef1234567890",
              headCommitShort: "abcdef1",
              implementationChatId: "chat-audit",
              phaseId: "phase-audit",
              phaseStatus: "completed",
              planId: "plan-audit",
              planStatus: "implemented",
              refName: "refs/heads/foco/plan-audit",
              worktreePath: "C:\\work\\foco\\.worktrees\\plan-audit",
              worktreeStatus: "kept",
            },
          ],
          recoveryNote: "Recover manually.",
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("No active plans for this workspace.")).toBeInTheDocument();

    const auditButton = screen.getByRole("button", { name: "Audit plan worktrees" });
    await user.click(auditButton);

    expect(await screen.findByText("Legacy worktrees")).toBeInTheDocument();
    expect(screen.getByText("plan-audit / phase-audit")).toBeInTheDocument();
    expect(screen.queryByText("No active plans for this workspace.")).not.toBeInTheDocument();

    await user.click(auditButton);

    await waitFor(() => {
      expect(screen.queryByText("Legacy worktrees")).not.toBeInTheDocument();
    });
    expect(screen.getByText("No active plans for this workspace.")).toBeInTheDocument();
  });

  it("loads and toggles backend plan auto-run state", async () => {
    const user = userEvent.setup();
    const autoRunRequests: Array<{ enabled?: boolean; method: string }> = [];
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;

      if (path === "/api/workspaces/workspace-1/plans/auto-run") {
        const body = init?.body
          ? (JSON.parse(String(init.body)) as { enabled?: boolean })
          : {};
        autoRunRequests.push({ enabled: body.enabled, method: init?.method ?? "GET" });
        return jsonResponse({ busy: body.enabled ?? false, enabled: body.enabled ?? false });
      }

      return mockFetch(input, init);
    });
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
    expect(await screen.findByText("Auto running")).toBeInTheDocument();
  });

  it("refreshes active plans when backend auto-run becomes busy", async () => {
    const user = userEvent.setup();
    const readyPlan: Plan = {
      ...planFixture,
      activePhaseId: null,
      phases: planFixture.phases.map((phase) => ({ ...phase, status: "pending" })),
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;

      if (path === "/api/workspaces/workspace-1/plans/auto-run") {
        const body = init?.body
          ? (JSON.parse(String(init.body)) as { enabled?: boolean })
          : {};
        return jsonResponse({ busy: body.enabled ?? false, enabled: body.enabled ?? false });
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(await screen.findByText("Auto-run refresh target")).toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: /Auto run plans/ }));

    await waitFor(() => {
      expect(planRequestCount).toBeGreaterThanOrEqual(2);
    });
    const planArticle = screen.getByText("Auto-run refresh target").closest("article");
    if (!planArticle) {
      throw new Error("Expected plan article");
    }
    expect(within(planArticle).getAllByText("Running").length).toBeGreaterThan(0);
  });

  it("single-flights plan and auto-run polling while auto-run is enabled", async () => {
    const user = userEvent.setup();
    const intervalCallbacks = new Map<number, { handler: TimerHandler; timeout?: number }>();
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
    const performanceNowSpy = vi.spyOn(performance, "now").mockImplementation(() => nowMs);
    const setIntervalSpy = vi.spyOn(window, "setInterval").mockImplementation(
      ((handler: TimerHandler, timeout?: number) => {
        nextIntervalId += 1;
        intervalCallbacks.set(nextIntervalId, { handler, timeout });
        return nextIntervalId;
      }) as typeof window.setInterval,
    );
    const clearIntervalSpy = vi.spyOn(window, "clearInterval").mockImplementation((id) => {
      intervalCallbacks.delete(Number(id));
    });
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();

      await user.click(await screen.findByRole("tab", { name: "Plan" }));
      expect(await screen.findByText(runningPlan.title)).toBeInTheDocument();
      await waitFor(() => {
        expect(
          Array.from(intervalCallbacks.values()).filter(({ timeout }) => timeout === 3000),
        ).toHaveLength(1);
      });

      fetchMock.mockClear();
      holdPlanRequests = true;
      nowMs = 1000;
      const poll = Array.from(intervalCallbacks.values()).find((item) => item.timeout === 3000)?.handler;
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
      expect(requestCount("/api/workspaces/workspace-1/plans/auto-run")).toBe(1);

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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;
      const match = path.match(/^\/api\/workspaces\/([^/]+)\/plans\/auto-run$/);

      if (match) {
        const workspaceId = decodeURIComponent(match[1] ?? "");
        if (init?.method === "PUT") {
          const body = JSON.parse(String(init.body ?? "{}")) as { enabled?: boolean };
          autoRunEnabledByWorkspace[workspaceId] = body.enabled ?? false;
        }
        return jsonResponse({
          busy: false,
          enabled: autoRunEnabledByWorkspace[workspaceId] ?? false,
        });
      }

      return mockFetch(input, init);
    });
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
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
      }),
    );
    await user.click(screen.getByRole("button", { name: /Side note/ }));

    await waitFor(() => {
      expect(autoRunCheckbox).not.toBeChecked();
    });

    await user.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    await screen.findAllByText("Default");
    await user.click(await screen.findByRole("tab", { name: "Plan" }));
    expect(
      await screen.findByRole("checkbox", { name: /Auto run plans/ }),
    ).not.toBeChecked();
    expect(await screen.findByText("Plan panel polling regression")).toBeInTheDocument();
    const phase1Section = screen.getByText("Phase 1").closest("section");
    if (!phase1Section) {
      throw new Error("Expected phase 1 section");
    }
    expect(within(phase1Section).getByText("Running")).toBeInTheDocument();

    planStage = 1;
    await waitForPlanPoll();

    await waitFor(() => {
      const updatedPhase1Section = screen.getByText("Phase 1").closest("section");
      const updatedPhase2Section = screen.getByText("Phase 2").closest("section");
      if (!updatedPhase1Section || !updatedPhase2Section) {
        throw new Error("Expected refreshed phase sections");
      }
      expect(within(updatedPhase1Section).getByText("Completed")).toBeInTheDocument();
      expect(within(updatedPhase2Section).getByText("Running")).toBeInTheDocument();
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const path = new URL(rawUrl, "http://127.0.0.1").pathname;

      if (path === "/api/workspaces/workspace-1/plans/auto-run") {
        const body = init?.body
          ? (JSON.parse(String(init.body)) as { enabled?: boolean })
          : {};
        autoRunRequests.push({ enabled: body.enabled, method: init?.method ?? "GET" });
        return jsonResponse({ busy: false, enabled: body.enabled ?? false });
      }

      if (path === "/api/workspaces/workspace-1/plans/order") {
        const body = JSON.parse(String(init?.body ?? "{}")) as { planIds: string[] };
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
        const plan = plans.find((candidate) => candidate.id === planId) ?? secondPlan;
        return jsonResponse({ plan });
      }

      return mockFetch(input, init);
    });
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
    const reorderHandles = screen.getAllByRole("button", { name: "Reorder plan" });
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
      expect(screen.getByText("Second queue plan").compareDocumentPosition(
        screen.getByText("First queue plan"),
      )).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });

    await user.click(await screen.findByRole("checkbox", { name: /Auto run plans/ }));
    await waitFor(() => {
      expect(autoRunRequests).toContainEqual({ enabled: true, method: "PUT" });
    });
    expect(fetchMock).not.toHaveBeenCalledWith(
      expect.stringMatching(/^\/api\/workspaces\/workspace-1\/plans\/[^/]+\/action$/),
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/plans/plan-merge-blocked/action") {
        didRetryMerge = true;
        return jsonResponse({ plan: mergedPlan });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await user.click(screen.getByRole("tab", { name: "Plan" }));

    const planCard = (await screen.findByText("Blocked merge plan")).closest("article");
    expect(planCard).not.toBeNull();
    const retryButton = within(planCard as HTMLElement).getByRole("button", {
      name: "Retry Merge",
    });
    expect(retryButton).toHaveAttribute(
      "title",
      "Clean the shared workspace, then retry merge",
    );
    expect(within(planCard as HTMLElement).getByText(blockedMessage)).toBeInTheDocument();

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
      "title",
      "Merged into shared workspace",
    );
    expect(screen.queryByRole("button", { name: "Retry Merge" })).not.toBeInTheDocument();
  });

  it("keeps retry merge action response visible when the refresh returns stale plan data", async () => {
    const user = userEvent.setup();
    const timestamp = "2026-07-04T12:20:00Z";
    const blockedPlan = {
      activePhaseId: null,
      completedAt: timestamp,
      completedByUserAt: null,
      createdAt: timestamp,
      errorMessage: "cannot merge Agent worktree while shared workspace has uncommitted changes",
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      if (path === "/api/workspaces/workspace-1/plans/plan-merge-stale-refresh/action") {
        didRetryMerge = true;
        return jsonResponse({ plan: runningPlan });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await user.click(screen.getByRole("tab", { name: "Plan" }));
    const retryButton = await screen.findByRole("button", { name: "Retry Merge" });

    await user.click(retryButton);

    await waitFor(() => {
      expect(didRetryMerge).toBe(true);
      expect(screen.getByText("Running")).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "Retry Merge" })).not.toBeInTheDocument();
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
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/plans") {
        return jsonResponse({
          page: 1,
          pageSize: 50,
          plans: [implementedPlan, phaseCommitOnlyImplementedPlan, ...statusColorPlans],
          totalCount: 6,
          totalPages: 1,
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Plan" }));

    expect(await screen.findByText("Merged implementation plan")).toBeInTheDocument();
    const mergedCommitBadge = screen.getByText("fedcba9");
    expect(mergedCommitBadge).toHaveAttribute(
      "title",
      "Merged into shared workspace",
    );
    expect(screen.queryByText("Merged")).not.toBeInTheDocument();

    const phaseCommitOnlyPlanCard = screen
      .getByText("Implemented plan with phase commit only")
      .closest("article");
    expect(phaseCommitOnlyPlanCard).not.toBeNull();
    expect(
      within(phaseCommitOnlyPlanCard as HTMLElement).queryByTitle(
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

    expectPlanStatusTone("Merged implementation plan", "Implemented", [
      "bg-emerald-100",
      "text-emerald-800",
    ]);
    expectPlanStatusTone("Completed status colors", "Completed", [
      "bg-emerald-100",
      "text-emerald-800",
    ]);
    expectPlanStatusTone("Failed status colors", "Failed", [
      "bg-rose-100",
      "text-rose-700",
    ]);
    expectPlanStatusTone("Cancelled status colors", "Cancelled", [
      "bg-stone-100",
      "text-stone-600",
    ]);
    expectPlanStatusTone("Ready status colors", "Ready", [
      "bg-amber-100",
      "text-amber-800",
    ]);
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

    await userEvent.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: /Side note/ }));
    expect(screen.getByRole("button", { name: "Open terminal" })).toBeInTheDocument();
    expect(closeSpy).not.toHaveBeenCalled();

    await userEvent.click(
      screen.getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
      }),
    );
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
    const toolsSection = screen.getByText("Tools and compression").parentElement!;
    expect(within(toolsSection).getByText("Read")).toBeInTheDocument();
    expect(within(toolsSection).queryByText("Rule compression snapshots")).not.toBeInTheDocument();
    expect(within(toolsSection).queryByText("Compression snapshots")).not.toBeInTheDocument();
    expect(screen.getByText("Tool history compression")).toBeInTheDocument();
    expect(
      within(screen.getByText("Tool history compression").parentElement!)
        .getByText("2"),
    ).toBeInTheDocument();
    expect(screen.queryByText("52,340 / 110,960")).not.toBeInTheDocument();
    const contextTimeline = screen.getByLabelText("Context usage timeline");
    expect(within(contextTimeline).getByText("47%")).toBeInTheDocument();
    expect(within(contextTimeline).queryByText("Snapshot 1")).not.toBeInTheDocument();
    expect(within(contextTimeline).queryByText("Snapshot 2")).not.toBeInTheDocument();
    expect(within(contextTimeline).queryByText(/llm \/ ctx-/)).not.toBeInTheDocument();
    expect(within(contextTimeline).queryByText("Past 80%")).not.toBeInTheDocument();
    expect(within(contextTimeline).getAllByText("80%")).not.toHaveLength(0);
    expect(within(contextTimeline).getAllByText("95%")).not.toHaveLength(0);
    const contextLegend = within(contextTimeline).getByLabelText("Context usage legend");
    expect(within(contextLegend).getByText("Prompt/tools")).toBeInTheDocument();
    expect(within(contextLegend).getByText("History")).toBeInTheDocument();
    expect(within(contextLegend).getByText("Compression snapshot")).toBeInTheDocument();
    expect(within(contextLegend).queryByText("Reserved output")).not.toBeInTheDocument();
    expect(within(contextTimeline).getAllByLabelText(/Prompt\/tools:/)).not.toHaveLength(0);
    expect(within(contextTimeline).getAllByLabelText(/History:/)).not.toHaveLength(0);
    expect(within(contextTimeline).getAllByLabelText(/Compression snapshot:/)).not.toHaveLength(0);
    expect(within(contextTimeline).queryByLabelText(/Reserved output:/)).not.toBeInTheDocument();
    expect(contextTimeline.querySelector(".context-usage-history-stack")).not.toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(([url]) => url === "/api/workspaces/workspace-1/context-usage"),
    ).toBe(true);
  });

  it("renders partial active chat statistics and context usage payloads without crashing", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input, init) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
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
    expect(within(screen.getByText("Total tokens").closest(".context-stat-metric")!).getByText("0"))
      .toBeInTheDocument();
    expect(screen.getByText("+0 / -0")).toBeInTheDocument();
    expect(within(screen.getByText("Model calls").parentElement!).getByText("No model calls yet."))
      .toBeInTheDocument();
    const toolsSection = screen.getByText("Tools and compression").parentElement!;
    expect(within(toolsSection).getByText("LLM compression snapshots")).toBeInTheDocument();
    expect(within(toolsSection).getByText("Tool history compression")).toBeInTheDocument();
    expect(within(toolsSection).getAllByText("0").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("No context usage yet."))
      .toBeInTheDocument();
  });


  it("shows context usage only once in the stats context mix", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByText("Tool run"));
    await user.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "continue");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

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

    const contextMix = screen.getByText("Context mix").parentElement!;
    expect(within(contextMix).queryByText("52,340 / 110,960")).not.toBeInTheDocument();
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

  it("single-flights duplicate active chat statistics refreshes", async () => {
    const user = userEvent.setup();
    let nowMs = 0;
    const performanceNowSpy = vi.spyOn(performance, "now").mockImplementation(() => nowMs);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Stats" }));
    expect(await screen.findByText("Session statistics")).toBeInTheDocument();
    nowMs = 1000;
    await user.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "save memory");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

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
      return new URL(rawUrl, "http://127.0.0.1").pathname ===
        "/api/workspaces/workspace-1/chats/chat-1/statistics";
    });
    expect(statisticsRequests).toHaveLength(1);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
    performanceNowSpy.mockRestore();
  });

  it("aborts active streams and chat message loads on app unmount", async () => {
    const user = userEvent.setup();
    const observedSignals: { loading: AbortSignal | null; stream: AbortSignal | null } = {
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
            reject(new DOMException("The operation was aborted.", "AbortError"));
            return;
          }
          signal?.addEventListener(
            "abort",
            () => reject(new DOMException("The operation was aborted.", "AbortError")),
            { once: true },
          );
        });
      }
      return mockFetch(input, init);
    });

    const { unmount } = renderApp();
    await user.click(await screen.findByText("Tool run"));
    await user.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "continue");
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

  it("shows the active workspace identity in the empty chat area", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: workspace.name })).toBeInTheDocument();
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

    expect(await screen.findByRole("heading", { name: workspace.name })).toBeInTheDocument();
    expect(screen.queryByText("Workspace shell is ready")).not.toBeInTheDocument();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);

    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getAllByText("17.6K").length).toBeGreaterThan(0),
    );
    expect(screen.getByText("Total requests")).toBeInTheDocument();
    expect(screen.getByText("Total tokens")).toBeInTheDocument();
    expect(screen.getByText("Average latency")).toBeInTheDocument();
    expect(screen.getByText("Failed requests")).toBeInTheDocument();
    expect(await screen.findByText("Requests and tokens trend")).toBeInTheDocument();
    expect(screen.getByText("Model distribution")).toBeInTheDocument();
    expect(screen.getByText("Channel distribution")).toBeInTheDocument();
    expect(screen.getByText("Channel quality")).toBeInTheDocument();
    expect(screen.getByText("Request audit")).toBeInTheDocument();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Provider" }),
      "openai",
    );
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
    const requestTimeLines = requestTimeCell?.querySelectorAll("div.space-y-1 > div");
    expect(requestTimeLines).toHaveLength(2);
    expect(requestTimeLines?.[0].textContent).toContain("2026");
    const tableScroller = table.parentElement;
    const statsScroller = table.closest(".overflow-y-auto") as HTMLElement | null;
    expect(tableScroller).toHaveClass("panel-scroll");
    expect(tableScroller).toHaveClass("overflow-x-auto");
    expect(tableScroller).toHaveClass("overflow-y-hidden");
    expect(tableScroller).not.toHaveClass("overflow-auto");
    expect(statsScroller).toHaveClass("panel-scroll");
    if (!tableScroller || !statsScroller) {
      throw new Error("Expected request audit table to live inside stats scroller");
    }
    statsScroller.style.overflowY = "auto";
    Object.defineProperties(statsScroller, {
      clientHeight: { configurable: true, value: 360 },
      scrollHeight: { configurable: true, value: 960 },
    });
    statsScroller.scrollTop = 0;
    fireEvent.touchStart(tableScroller, { touches: [{ clientX: 20, clientY: 140 }] });
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
    expect(within(table).getByText("GPT Test")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Request audit pagination" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to page 2" })).toBeInTheDocument();
    expect(screen.getByLabelText("Page size")).toHaveValue(20);

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(screen.getByRole("checkbox", { name: "Provider / model" }));
    expect(within(table).queryByText("OpenAI")).not.toBeInTheDocument();

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
        return Promise.resolve(jsonResponse({
          ...aiStatistics,
          totalCount: 24680,
          totalPages: 1234,
        }));
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);

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
        return Promise.resolve(jsonResponse({
          ...aiStatistics,
          page: 1,
          requests,
          totalCount: requests.length,
          totalPages: 1,
        }));
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    const table = await screen.findByRole("table");

    expect(within(table).getByText("succeeded")).toHaveClass(
      "bg-emerald-100",
      "text-emerald-800",
    );
    expect(within(table).getByText("failed")).toHaveClass(
      "bg-rose-100",
      "text-rose-700",
    );
    expect(within(table).getByText("running")).toHaveClass(
      "bg-amber-100",
      "text-amber-800",
    );
    expect(within(table).getByText("cancelled")).toHaveClass(
      "bg-stone-100",
      "text-stone-600",
    );
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
    expect(within(table).getByText("OpenAI")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(screen.getByRole("checkbox", { name: "Provider / model" }));
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

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    const reloadedTable = await screen.findByRole("table");
    expect(within(reloadedTable).queryByText("OpenAI")).not.toBeInTheDocument();
    await userEvent.click(screen.getByText("Columns"));
    expect(screen.getByRole("checkbox", { name: "Provider / model" })).not.toBeChecked();
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

  it("opens image files from the workspace file tree without Monaco", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    fetchMock.mockClear();

    await userEvent.click(await screen.findByText("logo.png"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /logo\.png/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.queryByRole("toolbar", { name: "Editor toolbar" })).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "logo.png" })).toHaveAttribute(
      "src",
      "/api/workspaces/workspace-1/files/blob?path=assets%2Flogo.png",
    );
    expect(
      fetchMock.mock.calls.some(([url]) => url === "/api/workspaces/workspace-1/files/content"),
    ).toBe(false);
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
