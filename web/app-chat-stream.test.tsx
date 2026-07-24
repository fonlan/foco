import { readFileSync } from "node:fs";

import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ConfiguredSkillSummary, WorkspaceSummary } from "./api/types";
import {
  activeMemory,
  aiStatistics,
  appTestState,
  changeInput,
  chatStreamResponse,
  chatSummary,
  defaultComposerPlaceholder,
  chatMemory,
  chatMessages,
  contextUsage,
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
  secondChatMessages,
  settings,
  todoGraph,
  workspace,
  workspaceMemory,
  type Deferred,
} from "./test-utils/app-test-harness";

function activePlan(id: string, title: string) {
  const timestamp = "2026-06-28T09:00:00Z";
  const phase = {
    agentTaskId: null,
    agentTeamId: null,
    commitId: null,
    completedAt: null,
    createdAt: timestamp,
    errorMessage: null,
    id: `${id}-phase-1`,
    implementationChatId: null,
    mergeAttemptCount: 0,
    planId: id,
    sequence: 0,
    startedAt: null,
    status: "pending",
    steps: [],
    summary: "Refresh active plans.",
    title: "Phase 1",
    updatedAt: timestamp,
  };

  return {
    activePhaseId: null,
    completedAt: null,
    completedByUserAt: null,
    createdAt: timestamp,
    errorMessage: null,
    sharedMergeCommitId: null,
    id,
    overview: "Refresh the plan panel.",
    pauseRequestedAt: null,
    phases: [phase],
    sortOrder: 0,
    sourceChatId: "chat-1",
    status: "ready",
    title,
    updatedAt: timestamp,
  };
}

const remoteWorkspaceId = "workspace-remote";
const remoteChatId = "remote-chat-1";
const remoteChatTitle = "Remote tool run";

function configureRemoteChat() {
  const remoteChat = chatSummary(
    remoteChatId,
    remoteChatTitle,
    "2026-07-12T08:00:00Z",
    "2026-07-12T08:05:00Z",
  );
  const remoteWorkspace = {
    ...secondaryWorkspace,
    chatPagination: { hasMore: false, limit: 5, nextCursor: null, total: 1 },
    chats: [remoteChat],
    connectionStatus: "ready",
    displayPath: "dev-box:/home/fonla/repos/remote-project",
    id: remoteWorkspaceId,
    name: "Remote project",
    path: "dev-box:/home/fonla/repos/remote-project",
    remotePath: "/home/fonla/repos/remote-project",
    serverId: "server-1",
    serverName: "dev-box",
  };
  const chatKey = `${remoteWorkspaceId}/${remoteChatId}`;
  appTestState.workspaceResponseWorkspaces = [
    { ...workspace },
    remoteWorkspace,
  ];
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
      chat: { ...chatMessages.chat, id: remoteChatId, title: remoteChatTitle },
    },
  };

  return { chatKey, remoteWorkspace };
}

function isDisabledControl(el: HTMLElement) {
  return (
    el.getAttribute("aria-disabled") === "true" ||
    el.getAttribute("data-disabled") === "true" ||
    el.hasAttribute("disabled") ||
    (el as HTMLButtonElement).disabled === true
  );
}

describe("app-chat-stream verification surfaces", () => {
  beforeEach(() => {
    resetAppTestEnvironment();
    delete (
      globalThis as {
        __FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__?: (
          update: () => void,
        ) => void;
      }
    ).__FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__;
  });

  it("edits a persisted user message, confirms truncation, and starts one replacement stream", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const editButton = await screen.findByRole("button", {
      name: "Edit message",
    });
    await userEvent.click(editButton);
    const editor = screen.getByRole("textbox", { name: "Edit message" });
    await userEvent.clear(editor);
    await userEvent.type(editor, "Please inspect CONTRIBUTING.md.");
    await userEvent.click(
      screen.getByRole("button", { name: "Save and regenerate" }),
    );

    expect(confirmSpy).toHaveBeenCalledWith(
      "Editing this message will remove 1 later messages and regenerate the reply. Continue?",
    );
    await waitFor(() => {
      expect(
        screen.getByText("Please inspect CONTRIBUTING.md."),
      ).toBeInTheDocument();
      expect(screen.queryByText("Done.")).not.toBeInTheDocument();
      expect(
        screen.queryByRole("textbox", { name: "Edit message" }),
      ).not.toBeInTheDocument();
    });
    const editRequest = vi
      .mocked(fetch)
      .mock.calls.find(([input]) =>
        input.toString().includes("/messages/message-user/edit"),
      );
    expect(editRequest).toBeDefined();
    expect(JSON.parse(String(editRequest?.[1]?.body))).toMatchObject({
      expectedContent: "Please inspect README.",
      message: "Please inspect CONTRIBUTING.md.",
      modelId: "gpt-test",
      providerId: "openai",
    });
    const streamRequests = vi
      .mocked(fetch)
      .mock.calls.filter(([input]) =>
        input.toString().includes("/chat/stream"),
      );
    expect(streamRequests).toHaveLength(1);
    expect(JSON.parse(String(streamRequests[0]?.[1]?.body))).toMatchObject({
      queuedUserMessageId: "message-user",
    });
  });

  it("keeps edit mode and restores history when the edit request fails", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      if (input.toString().includes("/messages/message-user/edit")) {
        return Promise.resolve(
          jsonResponse({ error: "edit failed" }, { status: 409 }),
        );
      }
      return mockFetch(input, init);
    });
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit message" }),
    );
    const editor = screen.getByRole("textbox", { name: "Edit message" });
    await userEvent.clear(editor);
    await userEvent.type(editor, "Edited text");
    await userEvent.click(
      screen.getByRole("button", { name: "Save and regenerate" }),
    );

    expect(await screen.findByText("edit failed")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Edit message" })).toHaveValue(
      "Edited text",
    );
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([input]) =>
        input.toString().includes("/chat/stream"),
      ),
    ).toHaveLength(0);
  });

  it("cancels inline editing without changing the composer or message", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    const composer = screen.getByPlaceholderText(defaultComposerPlaceholder);
    await userEvent.type(composer, "composer draft");
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit message" }),
    );
    const editor = screen.getByRole("textbox", { name: "Edit message" });
    await userEvent.clear(editor);
    await userEvent.type(editor, "discarded edit");
    await userEvent.click(
      screen.getByRole("button", { name: "Cancel editing" }),
    );

    expect(
      screen.queryByRole("textbox", { name: "Edit message" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Please inspect README.")).toBeInTheDocument();
    expect(composer).toHaveValue("composer draft");
  });

  it("collapses selected skill content blocks in user messages", async () => {
    const skillPath =
      "C:\\Users\\fonla\\.agents\\skills\\web-design-guidelines\\SKILL.md";
    const selectedSkillContent = [
      "# Selected Skills",
      "",
      "```json",
      JSON.stringify(
        [{ name: "web-design-guidelines", path: skillPath }],
        null,
        2,
      ),
      "```",
      "",
      "## Skill 1: web-design-guidelines",
      "",
      `Path: \`${skillPath}\``,
      "",
      "### Instructions",
      "",
      "---",
      "name: web-design-guidelines",
      "description: UI design guidance.",
      "---",
      "",
      "# Web Design Guidelines",
      "",
      "Use the existing product UI conventions.",
      "",
      "## End Selected Skills",
      "",
      "Settings single-column layout.",
    ].join("\n");
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
            messages: [
              {
                ...chatMessages.messages[0],
                content: selectedSkillContent,
                parts: [{ text: selectedSkillContent, type: "text" }],
              },
            ],
          }),
        );
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    expect(
      await screen.findByText("web-design-guidelines"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Settings single-column layout."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Use the existing product UI conventions."),
    ).not.toBeInTheDocument();
  });

  it("preserves single newlines in user message paragraphs", async () => {
    const content = "第一行\n第二行";
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
            messages: [
              {
                ...chatMessages.messages[0],
                content,
                parts: [{ text: content, type: "text" }],
              },
            ],
          }),
        );
      }

      return mockFetch(input, init);
    });

    const stylesCss = readFileSync("styles.css", "utf8");
    const userParagraphRule = stylesCss.match(
      /\.markdown-content-user p\s*\{[^}]*\}/,
    )?.[0];
    expect(userParagraphRule).toContain("white-space: pre-wrap;");
    const style = document.createElement("style");
    style.textContent = userParagraphRule ?? "";
    document.head.append(style);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const paragraph = await screen.findByText((_, element) => {
      return (
        element?.tagName.toLowerCase() === "p" &&
        element.textContent === content
      );
    });
    expect(getComputedStyle(paragraph).whiteSpace).toBe("pre-wrap");
    style.remove();
  });

  it("shows only enabled skills available to the active workspace in the composer picker", async () => {
    const detectedSkills: ConfiguredSkillSummary[] = [
      ...settings.skills.detected,
      {
        canEnable: true,
        description: "Current workspace helper.",
        enabled: true,
        id: "current-skill",
        key: "workspace:workspace-1:current-skill",
        name: "Current skill",
        path: "C:\\Users\\fonla\\.foco\\workspace\\.agents\\skills\\current-skill\\SKILL.md",
        scope: "workspace",
        workspaceId: workspace.id,
        workspaceName: workspace.name,
        warnings: [],
      },
      {
        canEnable: true,
        description: "Disabled helper.",
        enabled: false,
        id: "disabled-skill",
        key: "global:disabled-skill",
        name: "Disabled skill",
        path: "C:\\Users\\fonla\\.agents\\skills\\disabled-skill\\SKILL.md",
        scope: "global",
        workspaceId: null,
        workspaceName: null,
        warnings: [],
      },
      {
        canEnable: true,
        description: "Other workspace helper.",
        enabled: true,
        id: "other-skill",
        key: "workspace:workspace-2:other-skill",
        name: "Other skill",
        path: "C:\\Users\\fonla\\Documents\\Repos\\SideProject\\.agents\\skills\\other-skill\\SKILL.md",
        scope: "workspace",
        workspaceId: secondaryWorkspace.id,
        workspaceName: secondaryWorkspace.name,
        warnings: [],
      },
    ];

    appTestState.settingsResponse = {
      ...settings,
      skills: {
        ...settings.skills,
        detected: detectedSkills,
      },
    } as unknown as typeof settings;

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "/",
    );

    expect(
      await screen.findByRole("option", { name: "Select skill gitmemo" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Select skill Current skill" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Disabled skill" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Other skill" }),
    ).not.toBeInTheDocument();
  });

  it("shows remote workspace and remote global skills from the workspace skills API", async () => {
    const { remoteWorkspace } = configureRemoteChat();
    const localOnlySkill: ConfiguredSkillSummary = {
      canEnable: true,
      description: "Only on host settings.",
      enabled: true,
      id: "host-only",
      key: "global:host-only",
      name: "Host only skill",
      path: "C:\\Users\\fonla\\.agents\\skills\\host-only\\SKILL.md",
      scope: "global",
      workspaceId: null,
      workspaceName: null,
      warnings: [],
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      skills: {
        ...appTestState.settingsResponse.skills,
        detected: [
          ...appTestState.settingsResponse.skills.detected,
          localOnlySkill,
        ],
      },
    };

    const remoteGlobalSkill: ConfiguredSkillSummary = {
      canEnable: true,
      description: "Remote host global skill.",
      enabled: true,
      id: "remote-global",
      key: "global:remote-global",
      name: "Remote global skill",
      path: "/home/fonla/.agents/skills/remote-global/SKILL.md",
      scope: "global",
      workspaceId: null,
      workspaceName: null,
      warnings: [],
    };
    const remoteWorkspaceSkill: ConfiguredSkillSummary = {
      canEnable: true,
      description: "Remote workspace skill.",
      enabled: true,
      id: "remote-ws",
      key: `workspace:${remoteWorkspaceId}:remote-ws`,
      name: "Remote workspace skill",
      path: "/home/fonla/repos/remote-project/.agents/skills/remote-ws/SKILL.md",
      scope: "workspace",
      workspaceId: remoteWorkspaceId,
      workspaceName: remoteWorkspace.name,
      warnings: [],
    };
    const localGlobalFallback: ConfiguredSkillSummary = {
      canEnable: true,
      description: "Synced local global fallback.",
      enabled: true,
      id: "gitmemo",
      key: "global:gitmemo",
      name: "gitmemo",
      path: "C:\\Users\\fonla\\.agents\\skills\\gitmemo\\SKILL.md",
      scope: "global",
      workspaceId: null,
      workspaceName: null,
      warnings: [],
    };

    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: {
        skills: appTestState.settingsResponse.skills.detected.filter(
          (skill) =>
            skill.scope === "global" ||
            (skill.scope === "workspace" && skill.workspaceId === workspace.id),
        ),
      },
      [remoteWorkspaceId]: {
        skills: [localGlobalFallback, remoteGlobalSkill, remoteWorkspaceSkill],
      },
    };

    renderApp();
    await userEvent.click(await screen.findByText(remoteWorkspace.name));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await userEvent.type(
      await screen.findByPlaceholderText(
        "Ask Foco anything about Remote project…",
      ),
      "/",
    );

    expect(
      await screen.findByRole("option", {
        name: "Select skill Remote workspace skill",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Select skill Remote global skill" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Select skill gitmemo" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Host only skill" }),
    ).not.toBeInTheDocument();
  });

  it("lets remote workspace skills be selected from the slash menu", async () => {
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspaceId]: {
        skills: [
          {
            canEnable: true,
            description: "Remote workspace helper.",
            enabled: true,
            id: "remote-ws",
            key: `workspace:${remoteWorkspaceId}:remote-ws`,
            name: "Remote workspace skill",
            path: "/home/fonla/repos/remote-project/.agents/skills/remote-ws/SKILL.md",
            scope: "workspace",
            workspaceId: remoteWorkspaceId,
            workspaceName: "Remote project",
            warnings: [],
          },
        ],
      },
    };

    renderApp();
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    const composer = await screen.findByPlaceholderText(
      "Ask Foco anything about Remote project…",
    );
    await userEvent.type(composer, "/");
    await userEvent.click(
      await screen.findByRole("option", {
        name: "Select skill Remote workspace skill",
      }),
    );

    expect(screen.getByText("Remote workspace skill")).toBeInTheDocument();
    expect(composer).toHaveValue("");
  });

  it("shows a single remote-global winner for a key (API already deduped)", async () => {
    configureRemoteChat();
    // Backend effective catalog merges local-global + remote-global and keeps
    // only the remote winner for the same global:<id> key. The frontend menu
    // must render that single API entry without inventing a host-local twin.
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspaceId]: {
        skills: [
          {
            canEnable: true,
            description: "Remote copy wins.",
            enabled: true,
            id: "gitmemo",
            key: "global:gitmemo",
            name: "gitmemo",
            path: "/home/fonla/.agents/skills/gitmemo/SKILL.md",
            scope: "global",
            workspaceId: null,
            workspaceName: null,
            warnings: [],
          },
        ],
      },
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      skills: {
        ...appTestState.settingsResponse.skills,
        detected: [
          ...appTestState.settingsResponse.skills.detected,
          {
            canEnable: true,
            description:
              "Local host copy that must not appear via settings fallback.",
            enabled: true,
            id: "gitmemo",
            key: "global:gitmemo",
            name: "gitmemo",
            path: "C:\\Users\\fonla\\.agents\\skills\\gitmemo\\SKILL.md",
            scope: "global",
            workspaceId: null,
            workspaceName: null,
            warnings: [],
          },
        ],
      },
    };

    renderApp();
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await userEvent.type(
      await screen.findByPlaceholderText(
        "Ask Foco anything about Remote project…",
      ),
      "/",
    );

    const matches = await screen.findAllByRole("option", {
      name: "Select skill gitmemo",
    });
    expect(matches).toHaveLength(1);
    expect(matches[0]).toHaveAccessibleDescription("Remote copy wins.");
  });

  it("does not flash a cached prior workspace skill when switching workspaces", async () => {
    const remoteGate = deferred<Response>();
    configureRemoteChat();
    const localWorkspaceSkill: ConfiguredSkillSummary = {
      canEnable: true,
      description: "Current workspace helper.",
      enabled: true,
      id: "current-skill",
      key: "workspace:workspace-1:current-skill",
      name: "Current skill",
      path: "C:\\Users\\fonla\\.foco\\workspace\\.agents\\skills\\current-skill\\SKILL.md",
      scope: "workspace",
      workspaceId: workspace.id,
      workspaceName: workspace.name,
      warnings: [],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: {
        skills: [
          ...settings.skills.detected.filter(
            (skill) => skill.scope === "global",
          ),
          localWorkspaceSkill,
        ],
      },
      // Same pending promise twice so a remounted effect cannot fall through to
      // the host-local default catalog and skip the loading gate.
      [remoteWorkspaceId]: [remoteGate.promise, remoteGate.promise],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "/",
    );
    expect(
      await screen.findByRole("option", { name: "Select skill Current skill" }),
    ).toBeInTheDocument();

    // Switch while the local catalog is already cached; the remote menu must
    // never paint the prior workspace-scoped skill.
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/skills`,
        ),
      ).toBe(true);
    });

    const remoteComposer = await screen.findByPlaceholderText(
      "Ask Foco anything about Remote project…",
    );
    await userEvent.clear(remoteComposer);
    await userEvent.type(remoteComposer, "/");

    expect(
      screen.queryByRole("option", { name: "Select skill Current skill" }),
    ).not.toBeInTheDocument();
    expect(await screen.findByText("Loading skills…")).toBeInTheDocument();

    remoteGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Remote workspace skill.",
            enabled: true,
            id: "remote-ws",
            key: `workspace:${remoteWorkspaceId}:remote-ws`,
            name: "Remote workspace skill",
            path: "/home/fonla/repos/remote-project/.agents/skills/remote-ws/SKILL.md",
            scope: "workspace",
            workspaceId: remoteWorkspaceId,
            workspaceName: "Remote project",
            warnings: [],
          },
        ],
        errors: [],
      }),
    );

    expect(
      await screen.findByRole("option", {
        name: "Select skill Remote workspace skill",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Current skill" }),
    ).not.toBeInTheDocument();
  });

  it("does not send prior workspace skill ids while the new catalog is loading", async () => {
    const remoteGate = deferred<Response>();
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: {
        skills: [
          {
            canEnable: true,
            description: "Project memory.",
            enabled: true,
            id: "gitmemo",
            key: "global:gitmemo",
            name: "gitmemo",
            path: settings.skills.detected[0]!.path,
            scope: "global",
            workspaceId: null,
            workspaceName: null,
            warnings: [],
          },
          {
            canEnable: true,
            description: "Local only workspace skill.",
            enabled: true,
            id: "current-skill",
            key: "workspace:workspace-1:current-skill",
            name: "Current skill",
            path: "C:\\Users\\fonla\\.foco\\workspace\\.agents\\skills\\current-skill\\SKILL.md",
            scope: "workspace",
            workspaceId: workspace.id,
            workspaceName: workspace.name,
            warnings: [],
          },
        ],
      },
      [remoteWorkspaceId]: [remoteGate.promise, remoteGate.promise],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "/",
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Select skill Current skill" }),
    );
    expect(
      screen.getByLabelText("Remove skill Current skill"),
    ).toBeInTheDocument();

    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/skills`,
        ),
      ).toBe(true);
    });

    const remoteComposer = await screen.findByPlaceholderText(
      "Ask Foco anything about Remote project…",
    );
    await userEvent.clear(remoteComposer);
    await userEvent.type(remoteComposer, "Send before catalog ready");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === `/api/workspaces/${remoteWorkspaceId}/chat/stream`,
        ),
      ).toBe(true);
    });

    const streamCall = (
      globalThis.fetch as ReturnType<typeof vi.fn>
    ).mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === `/api/workspaces/${remoteWorkspaceId}/chat/stream`,
    );
    const streamBody = JSON.parse(String(streamCall?.[1]?.body)) as {
      message?: string;
      skillIds?: string[] | null;
    };
    // Empty draft selections serialize as null; the critical assertion is that
    // the previous workspace-scoped key is never attached to this workspace.
    expect(streamBody.skillIds ?? []).toEqual([]);
    expect(streamBody.message).toBe("Send before catalog ready");

    remoteGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Only remote skill.",
            enabled: true,
            id: "remote-only",
            key: `workspace:${remoteWorkspaceId}:remote-only`,
            name: "Remote only skill",
            path: "/home/fonla/repos/remote-project/.agents/skills/remote-only/SKILL.md",
            scope: "workspace",
            workspaceId: remoteWorkspaceId,
            workspaceName: "Remote project",
            warnings: [],
          },
        ],
        errors: [],
      }),
    );
  });

  it("ignores late skill catalog responses after a workspace switch", async () => {
    const localGate = deferred<Response>();
    const remoteGate = deferred<Response>();
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: [localGate.promise],
      [remoteWorkspaceId]: [remoteGate.promise],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${workspace.id}/skills`,
        ),
      ).toBe(true);
    });

    // Switch away before the first workspace catalog arrives.
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/skills`,
        ),
      ).toBe(true);
    });

    // Late local response must not paint onto the remote workspace menu.
    localGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Late local skill.",
            enabled: true,
            id: "late-local",
            key: "workspace:workspace-1:late-local",
            name: "Late local skill",
            path: "C:\\Users\\fonla\\.foco\\workspace\\.agents\\skills\\late-local\\SKILL.md",
            scope: "workspace",
            workspaceId: workspace.id,
            workspaceName: workspace.name,
            warnings: [],
          },
        ],
        errors: [],
      }),
    );

    await userEvent.type(
      await screen.findByPlaceholderText(
        "Ask Foco anything about Remote project…",
      ),
      "/",
    );
    expect(await screen.findByText("Loading skills…")).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Late local skill" }),
    ).not.toBeInTheDocument();

    remoteGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Remote workspace skill.",
            enabled: true,
            id: "remote-ws",
            key: `workspace:${remoteWorkspaceId}:remote-ws`,
            name: "Remote workspace skill",
            path: "/home/fonla/repos/remote-project/.agents/skills/remote-ws/SKILL.md",
            scope: "workspace",
            workspaceId: remoteWorkspaceId,
            workspaceName: "Remote project",
            warnings: [],
          },
        ],
        errors: [],
      }),
    );

    expect(
      await screen.findByRole("option", {
        name: "Select skill Remote workspace skill",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill Late local skill" }),
    ).not.toBeInTheDocument();
  });

  it("keeps selected skills while the catalog is loading and prunes only after ready", async () => {
    const remoteGate = deferred<Response>();
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: {
        skills: [
          {
            canEnable: true,
            description: "Project memory.",
            enabled: true,
            id: "gitmemo",
            key: "global:gitmemo",
            name: "gitmemo",
            path: settings.skills.detected[0]!.path,
            scope: "global",
            workspaceId: null,
            workspaceName: null,
            warnings: [],
          },
        ],
      },
      [remoteWorkspaceId]: [remoteGate.promise],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "/",
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Select skill gitmemo" }),
    );
    expect(screen.getByLabelText("Remove skill gitmemo")).toBeInTheDocument();

    // Switch to remote while its catalog is still loading.
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await userEvent.type(
      await screen.findByPlaceholderText(
        "Ask Foco anything about Remote project…",
      ),
      "/",
    );
    expect(await screen.findByText("Loading skills…")).toBeInTheDocument();

    // Authoritative ready catalog without gitmemo prunes the selection.
    remoteGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Only remote skill.",
            enabled: true,
            id: "remote-only",
            key: `workspace:${remoteWorkspaceId}:remote-only`,
            name: "Remote only skill",
            path: "/home/fonla/repos/remote-project/.agents/skills/remote-only/SKILL.md",
            scope: "workspace",
            workspaceId: remoteWorkspaceId,
            workspaceName: "Remote project",
            warnings: [],
          },
        ],
        errors: [],
      }),
    );

    expect(
      await screen.findByRole("option", {
        name: "Select skill Remote only skill",
      }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.queryByLabelText("Remove skill gitmemo"),
      ).not.toBeInTheDocument();
    });
  });

  it("preserves a still-valid selection across a loading workspace catalog", async () => {
    const remoteGate = deferred<Response>();
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [workspace.id]: {
        skills: settings.skills.detected,
      },
      [remoteWorkspaceId]: [remoteGate.promise],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "/",
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Select skill gitmemo" }),
    );

    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/skills`,
        ),
      ).toBe(true);
    });

    remoteGate.resolve(
      jsonResponse({
        skills: [
          {
            canEnable: true,
            description: "Still available remotely.",
            enabled: true,
            id: "gitmemo",
            key: "global:gitmemo",
            name: "gitmemo",
            path: "/home/fonla/.agents/skills/gitmemo/SKILL.md",
            scope: "global",
            workspaceId: null,
            workspaceName: null,
            warnings: [],
          },
        ],
        errors: [],
      }),
    );

    await waitFor(() => {
      expect(screen.getByLabelText("Remove skill gitmemo")).toBeInTheDocument();
    });
  });

  it("shows a discovery failure instead of host-local skills only", async () => {
    configureRemoteChat();
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspaceId]: [
        jsonResponse(
          { error: "sidecar skill discovery failed" },
          { status: 502 },
        ),
      ],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Remote project"));
    await userEvent.click(await screen.findByText(remoteChatTitle));
    await userEvent.type(
      await screen.findByPlaceholderText(
        "Ask Foco anything about Remote project…",
      ),
      "/",
    );

    expect(
      await screen.findByText(/Failed to load skills/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Select skill gitmemo" }),
    ).not.toBeInTheDocument();
  });

  it("reloads the workspace skill catalog after skills refresh from settings", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await waitFor(() => {
      expect(
        (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.some(
          ([url]) => url === `/api/workspaces/${workspace.id}/skills`,
        ),
      ).toBe(true);
    });

    const fetchMock = globalThis.fetch as ReturnType<typeof vi.fn>;
    const skillsCallsBefore = fetchMock.mock.calls.filter(
      ([url]) => url === `/api/workspaces/${workspace.id}/skills`,
    ).length;

    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    const settingsNav = await screen.findByRole("navigation", {
      name: "Settings",
    });
    await userEvent.click(
      within(settingsNav).getByRole("button", { name: "Skills" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Refresh skill discovery" }),
    );

    await waitFor(() => {
      const skillsCallsAfter = fetchMock.mock.calls.filter(
        ([url]) => url === `/api/workspaces/${workspace.id}/skills`,
      ).length;
      expect(skillsCallsAfter).toBeGreaterThan(skillsCallsBefore);
    });
  });

  it("lists enabled models in the flat composer model picker", async () => {
    appTestState.settingsResponse = {
      ...settings,
      providers: settings.providers.map((provider) =>
        provider.id === "anthropic"
          ? { ...provider, enabled: false }
          : provider,
      ),
    } as typeof settings;

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));

    expect(
      screen.getByRole("option", { name: "GPT Test" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "OpenAI: GPT Test" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Anthropic: GPT Test" }),
    ).not.toBeInTheDocument();
  });

  it("uses text-only trigger values and micro labels in composer pickers", async () => {
    const stylesCss = readFileSync("styles.css", "utf8");

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const modelTrigger = screen.getByRole("button", { name: /Model:/ });
    const selectValue = modelTrigger.querySelector(
      '[data-slot="select-value"]',
    );
    expect(selectValue).toHaveTextContent("GPT Test");
    expect(
      selectValue?.querySelector('[data-slot="label"]'),
    ).not.toBeInTheDocument();

    await userEvent.click(modelTrigger);
    const modelOption = await screen.findByRole("option", { name: "GPT Test" });
    expect(
      within(modelOption).getByText("GPT Test"),
    ).toHaveClass("composer-select-option-label");

    expect(stylesCss).toMatch(
      /\.composer-select-label,\s*\.composer-select-popover \.composer-select-option-label\s*\{\s*font-size:\s*var\(--foco-font-micro\)/,
    );
  });

  it("compacts the Fast toggle to a 1.75rem icon button under the phone breakpoint", () => {
    const stylesCss = readFileSync("styles.css", "utf8");
    const chatPanelSource = readFileSync("features/chat/ChatPanel.tsx", "utf8");

    expect(chatPanelSource).toMatch(
      /className=["']composer-fast-toggle-label["']/,
    );
    expect(chatPanelSource).toMatch(/aria-label=\{t\(["']Fast mode["']\)\}/);

    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?\.composer-fast-toggle[\s\S]*?width:\s*1\.75rem[\s\S]*?min-width:\s*1\.75rem[\s\S]*?max-width:\s*1\.75rem[\s\S]*?height:\s*1\.75rem[\s\S]*?flex:\s*0 0 1\.75rem/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?\.composer-fast-toggle[\s\S]*?padding-inline:\s*0/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?\.composer-fast-toggle-label\s*\{[\s\S]*?display:\s*none/,
    );
    // Visible Fast label must remain shown outside the phone breakpoint.
    expect(stylesCss).not.toMatch(
      /^\.composer-fast-toggle-label\s*\{[\s\S]*?display:\s*none/m,
    );
  });

  it("does not mark Fast-capable models in the composer model picker", async () => {
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        { ...settings.configuredModels[0]!, supportsFast: true },
      ],
    } as typeof settings;

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    expect(
      await screen.findByRole("button", { name: "Fast mode" }),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    const modelOption = screen.getByRole("option", {
      name: "GPT Test",
    });
    expect(within(modelOption).queryByText("Fast")).not.toBeInTheDocument();
  });

  it("confirms Fast once, keeps preference across unsupported models, and snapshots Fast on send", async () => {
    const fastModel = { ...settings.configuredModels[0]!, supportsFast: true };
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        fastModel,
        {
          ...fastModel,
          activeProviderId: "anthropic",
          displayName: "GPT Standard",
          id: "gpt-standard",
          providerIds: ["anthropic"],
          supportsFast: false,
        },
      ],
    } as typeof settings;
    const fetchMock = vi.mocked(fetch);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const fastToggle = await screen.findByRole("button", { name: "Fast mode" });
    expect(fastToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(fastToggle);
    const confirmation = await screen.findByRole("dialog", {
      name: "Enable Fast mode?",
    });
    await userEvent.click(
      within(confirmation).getByRole("button", { name: "Enable Fast" }),
    );
    expect(fastToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.click(fastToggle);
    expect(fastToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(fastToggle);
    expect(
      screen.queryByRole("dialog", { name: "Enable Fast mode?" }),
    ).not.toBeInTheDocument();
    expect(fastToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    await userEvent.click(screen.getByRole("option", { name: "GPT Standard" }));
    expect(
      screen.queryByText("Fast mode is not available for the selected model."),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Fast mode" }),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    await userEvent.click(screen.getByRole("option", { name: "GPT Test" }));
    const restoredFastToggle = await screen.findByRole("button", {
      name: "Fast mode",
    });
    // Preference is kept while the model temporarily cannot use Fast; UI and
    // send path clamp via selectedRequestLatencyMode, so no error is shown.
    expect(restoredFastToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Use Fast for this request.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const streamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(JSON.parse(String(streamCall?.[1]?.body))).toMatchObject({
      latencyMode: "fast",
      modelId: "gpt-test",
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("sends Standard while Fast preference is inactive for an unsupported model", async () => {
    const fastModel = { ...settings.configuredModels[0]!, supportsFast: true };
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        fastModel,
        {
          ...fastModel,
          activeProviderId: "anthropic",
          displayName: "Anthropic Standard",
          id: "anthropic-standard",
          providerIds: ["anthropic"],
          supportsFast: false,
        },
      ],
    } as typeof settings;
    const fetchMock = vi.mocked(fetch);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(
      await screen.findByRole("button", { name: "Fast mode" }),
    );
    await userEvent.click(
      within(
        await screen.findByRole("dialog", { name: "Enable Fast mode?" }),
      ).getByRole("button", { name: "Enable Fast" }),
    );

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    await userEvent.click(
      screen.getByRole("option", { name: "Anthropic Standard" }),
    );
    expect(
      screen.queryByText("Fast mode is not available for the selected model."),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Fast mode" }),
    ).not.toBeInTheDocument();
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Use the standard provider route.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const streamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(JSON.parse(String(streamCall?.[1]?.body))).toMatchObject({
      latencyMode: "standard",
      modelId: "anthropic-standard",
      providerId: "anthropic",
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("restores Fast from a failed run and retries with the committed latency mode", async () => {
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        { ...settings.configuredModels[0]!, supportsFast: true },
      ],
    } as typeof settings;
    appTestState.chatMessagesResponsesByChatKey = {
      "workspace-1/chat-1": {
        ...chatMessages,
        messages: [
          {
            ...chatMessages.messages[0]!,
            runConfig: {
              modelId: "gpt-test",
              providerId: "openai",
              thinkingLevel: "high",
              latencyMode: "fast",
              selectedSkillIds: [],
              sessionMode: null,
              teamModeEnabled: false,
            },
          },
          {
            ...chatMessages.messages[1]!,
            content: "provider failed",
            parts: [{ text: "provider failed", type: "error" }],
            status: "error",
          },
        ],
      } as unknown as typeof chatMessages,
    };
    const fetchMock = vi.mocked(fetch);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    expect(
      await screen.findByRole("button", { name: "Retry last run" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fast mode" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await userEvent.click(
      screen.getByRole("button", { name: "Retry last run" }),
    );
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const streamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(JSON.parse(String(streamCall?.[1]?.body))).toMatchObject({
      latencyMode: "fast",
      thinkingLevel: "high",
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("ignores connecting chat stream progress events", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        message: "connecting to remote broker",
        type: "connecting",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Remote answer.",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("Remote answer.")).toBeInTheDocument();
    expect(screen.queryByText(/unknown event/i)).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("refreshes assembled context usage after a stream completes", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    expect(
      screen.getByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
    });

    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCalls).toHaveLength(2);

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
      await screen.findByRole("status", { name: "Context usage 55%" }),
    ).toHaveTextContent("55%");
    const usageCallsAfterUsage = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsAfterUsage).toHaveLength(usageCalls.length);

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
    expect(usageCallsAfterComplete).toHaveLength(
      usageCallCountBeforeComplete + 1,
    );
    expect(
      screen.getByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("uses a terminal remote context usage refresh after streamEnd", async () => {
    const { chatKey } = configureRemoteChat();
    window.history.replaceState(
      null,
      "",
      `/${remoteWorkspaceId}/${remoteChatId}`,
    );
    renderApp();

    const composer = await screen.findByPlaceholderText(
      `Ask Foco anything about Remote project…`,
    );
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(composer, "continue remotely");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    appTestState.contextUsageResponseQueuesByChatKey[chatKey] = [
      { ...contextUsage, usagePercent: 33 },
    ];

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
      await screen.findByRole("status", { name: "Context usage 55%" }),
    ).toHaveTextContent("55%");

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
    });
    expect(
      await screen.findByRole("status", { name: "Context usage 33%" }),
    ).toHaveTextContent("33%");
  });

  it("sends a second remote message normally after a delayed terminal active-run snapshot", async () => {
    const fetchMock = vi.mocked(fetch);
    const { chatKey } = configureRemoteChat();
    window.history.replaceState(
      null,
      "",
      `/${remoteWorkspaceId}/${remoteChatId}`,
    );
    renderApp();

    const composer = await screen.findByPlaceholderText(
      `Ask Foco anything about Remote project…`,
    );
    await userEvent.type(composer, "first remote task");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "remote-assistant-1",
        chatId: remoteChatId,
        memoriesUsed: [],
        runId: "remote-run-1",
        type: "start",
        userMessageId: "remote-user-1",
      });
    });

    const lateActiveRun = {
      acceptingGuidance: true,
      chatId: remoteChatId,
      lastSequence: 8,
      runId: "remote-run-1",
      workspaceId: remoteWorkspaceId,
    };
    const workspaceSummaries =
      appTestState.workspaceResponseWorkspaces as WorkspaceSummary[];
    appTestState.workspaceResponseWorkspaces = workspaceSummaries.map(
      (workspaceSummary) =>
        workspaceSummary.id === remoteWorkspaceId
          ? {
              ...workspaceSummary,
              chats: workspaceSummary.chats.map((chat) =>
                chat.id === remoteChatId
                  ? { ...chat, activeRun: lateActiveRun }
                  : chat,
              ),
            }
          : workspaceSummary,
    );
    appTestState.chatMessagesResponsesByChatKey[chatKey] = {
      ...appTestState.chatMessagesResponsesByChatKey[chatKey],
      activeRun: lateActiveRun,
    } as typeof chatMessages;

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "remote-assistant-1",
        chatId: remoteChatId,
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 1,
          providerId: "provider-1",
          totalLatencyMs: 100,
        },
        reasoning: null,
        stopReason: "completed",
        text: "First remote answer.",
        type: "complete",
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    await userEvent.type(composer, "second remote task");
    const sendButton = await screen.findByRole("button", {
      name: "Send message",
    });
    expect(sendButton).toBeEnabled();
    await userEvent.click(sendButton);

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/chat/stream`,
      );
      expect(streamCalls).toHaveLength(2);
    });
    const streamCalls = fetchMock.mock.calls.filter(
      ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/chat/stream`,
    );
    expect(JSON.parse(String(streamCalls[1]?.[1]?.body))).toMatchObject({
      chatId: remoteChatId,
      queuedUserMessageId: "queued-user-2",
    });
    const queueCalls = fetchMock.mock.calls.filter(
      ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/chat/queue`,
    );
    const guidanceCalls = fetchMock.mock.calls.filter(
      ([url]) => url === `/api/workspaces/${remoteWorkspaceId}/chat/guidance`,
    );
    expect(queueCalls).toHaveLength(2);
    expect(guidanceCalls).toHaveLength(0);
  });

  it("refreshes terminal context usage after a remote stream error", async () => {
    const { chatKey } = configureRemoteChat();
    window.history.replaceState(
      null,
      "",
      `/${remoteWorkspaceId}/${remoteChatId}`,
    );
    renderApp();

    const composer = await screen.findByPlaceholderText(
      `Ask Foco anything about Remote project…`,
    );
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(composer, "fail remotely");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    appTestState.contextUsageResponseQueuesByChatKey[chatKey] = [
      { ...contextUsage, usagePercent: 39 },
    ];

    await act(async () => {
      enqueueChatStreamEvent({
        message: "Remote broker failed.",
        type: "error",
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });
    expect(
      await screen.findByRole("status", { name: "Context usage 39%" }),
    ).toHaveTextContent("39%");
  });

  it("refreshes terminal context usage after a remote run is cancelled", async () => {
    const { chatKey } = configureRemoteChat();
    window.history.replaceState(
      null,
      "",
      `/${remoteWorkspaceId}/${remoteChatId}`,
    );
    renderApp();

    const composer = await screen.findByPlaceholderText(
      `Ask Foco anything about Remote project…`,
    );
    await userEvent.type(composer, "cancel remotely");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    appTestState.contextUsageResponseQueuesByChatKey[chatKey] = [
      { ...contextUsage, usagePercent: 42 },
    ];

    await userEvent.click(screen.getByRole("button", { name: "Cancel run" }));
    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
    expect(
      await screen.findByRole("status", { name: "Context usage 42%" }),
    ).toHaveTextContent("42%");
  });

  it("refreshes context usage on complete when no usage event was sent", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 1000,
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
          inputTokens: 70000,
          outputTokens: 1000,
        },
      });
    });

    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCalls).toHaveLength(3);
    const [, usageInit] = usageCalls.at(-1)!;
    expect(typeof usageInit?.body).toBe("string");
    expect(JSON.parse(usageInit?.body as string)).toMatchObject({
      chatId: "chat-1",
    });
    expect(JSON.parse(usageInit?.body as string)).not.toHaveProperty(
      "latestResponseUsage",
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("refreshes context usage from streaming assistant drafts on a 5 second throttle", async () => {
    const fetchMock = vi.mocked(fetch);
    const contextUsageCalls = () =>
      fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      );
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await waitFor(() => expect(contextUsageCalls().length).toBeGreaterThan(0));
    let now = 1_000_000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const usageCallCountBeforeDeltas = contextUsageCalls().length;

    now += 4_999;
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Part one. ",
        type: "textDelta",
      });
    });

    expect(contextUsageCalls()).toHaveLength(usageCallCountBeforeDeltas);

    now += 1;
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Part two.",
        type: "textDelta",
      });
    });

    await waitFor(() =>
      expect(contextUsageCalls()).toHaveLength(usageCallCountBeforeDeltas + 1),
    );
    const liveUsageBody = JSON.parse(
      contextUsageCalls().at(-1)?.[1]?.body as string,
    );
    expect(liveUsageBody).toMatchObject({
      assistantDraft: "Part one. Part two.",
      chatId: "chat-1",
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 1000,
          providerId: "provider-1",
          totalLatencyMs: 1000,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Part one. Part two.",
        type: "complete",
      });
    });

    await waitFor(() =>
      expect(contextUsageCalls()).toHaveLength(usageCallCountBeforeDeltas + 2),
    );
    const finalUsageBody = JSON.parse(
      contextUsageCalls().at(-1)?.[1]?.body as string,
    );
    expect(finalUsageBody).toMatchObject({ chatId: "chat-1" });
    expect(finalUsageBody).not.toHaveProperty("assistantDraft");
    expect(finalUsageBody).not.toHaveProperty("assistantDraftReasoning");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps bottom lock through non-user scroll events while streaming", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }

    let scrollHeight = 1000;
    const clientHeight = 500;
    let scrollTop = 0;
    Object.defineProperties(messageList, {
      clientHeight: { configurable: true, get: () => clientHeight },
      scrollHeight: { configurable: true, get: () => scrollHeight },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value) => {
          scrollTop = Math.min(value, Math.max(0, scrollHeight - clientHeight));
        },
      },
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "First lock chunk. ",
        type: "textDelta",
      });
    });
    expect(await screen.findByText("First lock chunk.")).toBeInTheDocument();
    await waitFor(() => expect(messageList.scrollTop).toBe(500));

    scrollHeight = 1080;
    fireEvent.scroll(messageList);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Second lock chunk.",
        type: "textDelta",
      });
    });

    await waitFor(() => expect(messageList.scrollTop).toBe(580));

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("does not re-lock after a native scrollbar drag moves away from the bottom", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }

    let scrollHeight = 1000;
    const clientHeight = 500;
    let scrollTop = 0;
    Object.defineProperties(messageList, {
      clientHeight: { configurable: true, get: () => clientHeight },
      scrollHeight: { configurable: true, get: () => scrollHeight },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value) => {
          scrollTop = Math.min(value, Math.max(0, scrollHeight - clientHeight));
        },
      },
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "First lock chunk. ",
        type: "textDelta",
      });
    });
    expect(await screen.findByText("First lock chunk.")).toBeInTheDocument();
    await waitFor(() => expect(messageList.scrollTop).toBe(500));

    // Browser scrollbar drags may emit only a scroll event, without wheel or
    // pointer events on the message list.
    scrollTop = 350;
    fireEvent.scroll(messageList);

    scrollHeight = 1080;
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Second unlocked chunk.",
        type: "textDelta",
      });
    });

    expect(
      await screen.findByText("First lock chunk. Second unlocked chunk."),
    ).toBeInTheDocument();
    await waitFor(() => expect(messageList.scrollTop).toBe(350));

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("batches adjacent text deltas before flushing them to the bubble", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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

    expect(screen.queryByText("Part one. Part two.")).not.toBeInTheDocument();
    expect(await screen.findByText("Part one. Part two.")).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("restores streaming parts when a provider attempt resets", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "retry",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Stable thinking.",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Before.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-stable",
          input: {},
          isError: false,
          name: "read_file",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    expect(await screen.findByText("Before.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        llmRequestId: "llm-retry",
        type: "streamAttemptStart",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Dropped thinking.",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Dropped answer.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-dropped",
          input: {},
          isError: false,
          name: "dropped_tool",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        reason: "provider completed without assistant text or tool calls",
        reasoning: "Flattened thinking.",
        text: "Flattened answer.",
        toolCalls: [
          {
            id: "call-stable",
            input: {},
            isError: false,
            name: "read_file",
            output: null,
            status: "running",
          },
          {
            id: "call-dropped",
            input: {},
            isError: false,
            name: "dropped_tool",
            output: null,
            status: "running",
          },
        ],
        type: "streamReset",
      });
    });

    await waitFor(() => {
      expect(screen.queryByText("Dropped answer.")).not.toBeInTheDocument();
      expect(screen.queryByText("Flattened answer.")).not.toBeInTheDocument();
      expect(screen.queryByText("dropped_tool")).not.toBeInTheDocument();
    });
    expect(screen.getByText("Before.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows context compression parts from stream side events", async () => {
    const fetchMock = vi.mocked(fetch);
    const contextUsageCallCount = () =>
      fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      ).length;

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const contextUsageCallCountBeforeStart = contextUsageCallCount();
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        kind: "rule",
        status: "start",
        type: "contextCompression",
        detail: {
          kind: "rule",
          originalTokenCount: 1200,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt: "2026-07-06T06:00:00Z",
          status: "start",
        },
      });
    });

    expect(await screen.findByText("Context compression")).toBeInTheDocument();
    expect(screen.getByText("Compressing")).toBeInTheDocument();
    expect(
      screen.getByText("Context compression in progress"),
    ).toBeInTheDocument();

    expect(contextUsageCallCount()).toBe(contextUsageCallCountBeforeStart);
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        kind: "rule",
        snapshotId: "snapshot-rule-1",
        status: "completed",
        type: "contextCompression",
        detail: {
          kind: "rule",
          snapshotId: "snapshot-rule-1",
          originalTokenCount: 1200,
          summaryTokenCount: 320,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt: "2026-07-06T06:00:00Z",
          completedAt: "2026-07-06T06:00:01Z",
          status: "completed",
        },
      });
    });

    expect(await screen.findByText("Compressed")).toBeInTheDocument();
    expect(
      screen.getByText("Context compression completed"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Saved 880 tokens/)).toBeInTheDocument();
    await waitFor(() =>
      expect(contextUsageCallCount()).toBe(
        contextUsageCallCountBeforeStart + 1,
      ),
    );
    expect(screen.getAllByText("Context compression")).toHaveLength(1);
    await userEvent.click(screen.getByText("Context compression"));
    expect(screen.getByText("Original tokens")).toBeInTheDocument();
    expect(screen.getByText("Compressed tokens")).toBeInTheDocument();
    expect(screen.getByText("snapshot-rule-1")).toBeInTheDocument();
    expect(screen.getAllByText("openai").length).toBeGreaterThan(0);
    expect(screen.getAllByText("gpt-test").length).toBeGreaterThan(0);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps a single in-progress LLM compression part after start then completed", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "compress please",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-live",
        kind: "llm",
        status: "start",
        type: "contextCompression",
        detail: {
          compressionId: "compression-live",
          kind: "llm",
          originalTokenCount: 5000,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt: "2026-07-06T07:00:00Z",
          status: "start",
        },
      });
    });

    expect(await screen.findByText("Compressing")).toBeInTheDocument();
    expect(screen.getAllByText("Context compression")).toHaveLength(1);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-live",
        kind: "llm",
        snapshotId: "llm-snapshot-live",
        status: "completed",
        type: "contextCompression",
        detail: {
          compressionId: "compression-live",
          kind: "llm",
          snapshotId: "llm-snapshot-live",
          originalTokenCount: 5000,
          summaryTokenCount: 900,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt: "2026-07-06T07:00:00Z",
          completedAt: "2026-07-06T07:00:02Z",
          status: "completed",
        },
      });
    });

    expect(await screen.findByText("Compressed")).toBeInTheDocument();
    expect(screen.queryByText("Compressing")).not.toBeInTheDocument();
    expect(screen.getAllByText("Context compression")).toHaveLength(1);
    expect(
      screen.getByText(/Saved 4,100 tokens|Saved 4100 tokens/),
    ).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps same-second compression attempts distinct by compression ID", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "compress twice",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const startedAt = "2026-07-06T07:00:00Z";
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-interrupted",
        kind: "llm",
        status: "start",
        type: "contextCompression",
        detail: {
          compressionId: "compression-interrupted",
          kind: "llm",
          originalTokenCount: 5000,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt,
          status: "start",
        },
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-success",
        kind: "llm",
        status: "start",
        type: "contextCompression",
        detail: {
          compressionId: "compression-success",
          kind: "llm",
          originalTokenCount: 4200,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt,
          status: "start",
        },
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-success",
        kind: "llm",
        snapshotId: "llm-snapshot-success",
        status: "completed",
        type: "contextCompression",
        detail: {
          compressionId: "compression-success",
          kind: "llm",
          snapshotId: "llm-snapshot-success",
          originalTokenCount: 4200,
          summaryTokenCount: 800,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt,
          completedAt: "2026-07-06T07:00:02Z",
          status: "completed",
        },
      });
    });

    expect(await screen.findByText("Compressed")).toBeInTheDocument();
    expect(screen.getByText("Compressing")).toBeInTheDocument();
    expect(screen.getAllByText("Context compression")).toHaveLength(2);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("restores three durable compression parts when chat messages are reloaded", async () => {
    const durableMessages = {
      activeRun: null,
      messages: [
        {
          id: "message-user-history",
          role: "user",
          content: "history",
          sequence: 0,
          createdAt: "2026-07-06T07:00:00Z",
          parts: [{ type: "text", text: "history" }],
          toolCalls: [],
        },
        {
          id: "message-assistant-history",
          role: "assistant",
          content: "Recovered answer.",
          sequence: 1,
          createdAt: "2026-07-06T07:00:01Z",
          parts: [
            {
              type: "contextCompression",
              id: "llm-snapshot-history",
              status: "completed",
              kind: "llm",
              detail: {
                status: "completed",
                kind: "llm",
                snapshotId: "llm-snapshot-history",
                originalTokenCount: 4200,
                summaryTokenCount: 800,
                startedAt: "2026-07-06T07:00:00Z",
                completedAt: "2026-07-06T07:00:01Z",
                providerId: "openai",
                modelId: "gpt-test",
              },
            },
            {
              type: "contextCompression",
              id: "llm-snapshot-history-two",
              status: "completed",
              kind: "llm",
              detail: {
                status: "completed",
                kind: "llm",
                snapshotId: "llm-snapshot-history-two",
                originalTokenCount: 3600,
                summaryTokenCount: 700,
                startedAt: "2026-07-06T07:00:02Z",
                completedAt: "2026-07-06T07:00:03Z",
                providerId: "openai",
                modelId: "gpt-test",
              },
            },
            {
              type: "contextCompression",
              id: "llm-snapshot-history-three",
              status: "completed",
              kind: "llm",
              detail: {
                status: "completed",
                kind: "llm",
                snapshotId: "llm-snapshot-history-three",
                originalTokenCount: 2800,
                summaryTokenCount: 600,
                startedAt: "2026-07-06T07:00:04Z",
                completedAt: "2026-07-06T07:00:05Z",
                providerId: "openai",
                modelId: "gpt-test",
              },
            },
            { type: "text", text: "Recovered answer." },
          ],
          toolCalls: [],
        },
      ],
    };

    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse(durableMessages);
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    expect(await screen.findByText("Recovered answer.")).toBeInTheDocument();
    expect(screen.getAllByText("Context compression")).toHaveLength(3);
    expect(screen.getAllByText("Compressed")).toHaveLength(3);
    expect(
      screen.getByText(/Saved 3,400 tokens|Saved 3400 tokens/),
    ).toBeInTheDocument();
  });

  it("opens and reloads active plans after a plan refresh side event", async () => {
    let activePlanRequests = 0;
    const refreshedPlan = activePlan("plan-refresh-1", "Refresh-visible plan");
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/plans") {
          activePlanRequests += 1;
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [refreshedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        type: "plan_refresh",
        workspace_id: "workspace-1",
      });
    });

    expect(await screen.findByRole("tab", { name: "Plan" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(await screen.findByText("Refresh-visible plan")).toBeInTheDocument();
    expect(activePlanRequests).toBeGreaterThanOrEqual(1);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("opens and reloads active plans after a running chat plan refresh event", async () => {
    let activePlanRequests = 0;
    const refreshedPlan = activePlan(
      "plan-refresh-running",
      "Running-refresh plan",
    );
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
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

        if (path === "/api/workspaces/workspace-1/plans") {
          activePlanRequests += 1;
          return jsonResponse({
            page: 1,
            pageSize: 50,
            plans: [refreshedPlan],
            totalCount: 1,
            totalPages: 1,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
    );

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        type: "planRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByRole("tab", { name: "Plan" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(await screen.findByText("Running-refresh plan")).toBeInTheDocument();
    expect(activePlanRequests).toBeGreaterThanOrEqual(1);

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("keeps context usage isolated between open chats", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
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
      await screen.findByRole("status", { name: "Context usage 55%" }),
    ).toHaveTextContent("55%");

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(
      await screen.findByRole("status", { name: "Context usage 23%" }),
    ).toHaveTextContent("23%");

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
      screen.getByRole("status", { name: "Context usage 23%" }),
    ).toHaveTextContent("23%");

    await userEvent.click(screen.getByRole("tab", { name: /Tool run/ }));
    expect(
      await screen.findByRole("status", { name: "Context usage 55%" }),
    ).toHaveTextContent("55%");

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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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
      await userEvent.click(
        screen.getByRole("button", { name: "Send message" }),
      );
      await waitFor(() =>
        expect(appTestState.activeChatStreamController).not.toBeNull(),
      );

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
      expect(
        within(assistantRow as HTMLElement).getByText("First plan.", {
          selector: "span",
        }),
      ).toBeInTheDocument();
      expect(
        within(assistantRow as HTMLElement).getByText("Second plan.", {
          selector: "span",
        }),
      ).toBeInTheDocument();
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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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
    const interruptedAssistantRow = screen
      .getByText("Initial answer.")
      .closest(".message-row");
    expect(interruptedAssistantRow).not.toBeNull();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        "Model: gpt-test",
      ),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        "Channel: openai",
      ),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        "Total time: 2 sec",
      ),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("tokens/s: 5"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).queryByText(
        /First token latency/,
      ),
    ).not.toBeInTheDocument();

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
    const initialAnswerRow = screen
      .getByText("Initial answer.")
      .closest(".message-row");
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

  it("shows reasoning-loop recovery as a user bubble without error UI", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "thinking in a loop ",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        content: "repeated reasoning loop, check and continue",
        id: "reasoning-loop-1",
        interruptedAssistantId: "message-assistant-stream",
        interruptedAssistantMetrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 1500,
        },
        parts: [],
        source: "reasoningLoopGuard",
        type: "guidanceApplied",
      });
    });

    const recoveryText = await screen.findByText(
      "repeated reasoning loop, check and continue",
    );
    const recoveryRow = recoveryText.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(recoveryRow).not.toBeNull();
    expect(recoveryRow?.className).toContain("message-row-user");
    expect(
      within(recoveryRow as HTMLElement).queryByText("Guidance pending"),
    ).not.toBeInTheDocument();
    expect(
      within(recoveryRow as HTMLElement).queryByText("Queued"),
    ).not.toBeInTheDocument();
    expect(
      within(recoveryRow as HTMLElement).queryByRole("button", {
        name: "Edit message",
      }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/error/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Recovered answer.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-after-recovery",
          input: {},
          isError: false,
          name: "noop",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    const recoveredAnswer = await screen.findByText("Recovered answer.");
    const recoveredRow = recoveredAnswer.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(recoveredRow).not.toBeNull();
    expect(recoveredRow).not.toBe(recoveryRow);
    expect(
      recoveryText.compareDocumentPosition(recoveredAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      within(recoveredRow as HTMLElement).getByText(/noop/),
    ).toBeInTheDocument();

    const interruptedRow = screen
      .getByText("thinking in a loop")
      .closest(".message-row") as HTMLElement | null;
    expect(interruptedRow).not.toBeNull();
    expect(
      within(interruptedRow as HTMLElement).queryByText("Recovered answer."),
    ).not.toBeInTheDocument();
    expect(
      within(interruptedRow as HTMLElement).queryByText(/noop/),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("routes consecutive reasoning-loop recoveries to the latest assistant bubble", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const durableAssistantId = "message-assistant-stream";
    const recoveryText = "repeated reasoning loop, check and continue";

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        delta: "first loop reasoning",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        content: recoveryText,
        id: "reasoning-loop-1",
        interruptedAssistantId: durableAssistantId,
        interruptedAssistantMetrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 1000,
        },
        parts: [],
        source: "reasoningLoopGuard",
        type: "guidanceApplied",
      });
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        delta: "first recovery answer",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        toolCall: {
          id: "call-after-first-recovery",
          input: {},
          isError: false,
          name: "first_tool",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    const firstRecoveryAnswer = await screen.findByText(
      "first recovery answer",
    );
    const firstRecoveryAnswerRow = firstRecoveryAnswer.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(firstRecoveryAnswerRow).not.toBeNull();
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).getByText(/first_tool/),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        delta: "second loop reasoning",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        content: recoveryText,
        id: "reasoning-loop-2",
        interruptedAssistantId: durableAssistantId,
        interruptedAssistantMetrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 1500,
        },
        parts: [],
        source: "reasoningLoopGuard",
        type: "guidanceApplied",
      });
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        delta: "second recovery answer",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: durableAssistantId,
        toolCall: {
          id: "call-after-second-recovery",
          input: {},
          isError: false,
          name: "second_tool",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    const secondRecoveryAnswer = await screen.findByText(
      "second recovery answer",
    );
    const secondRecoveryAnswerRow = secondRecoveryAnswer.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(secondRecoveryAnswerRow).not.toBeNull();
    expect(secondRecoveryAnswerRow).not.toBe(firstRecoveryAnswerRow);

    const recoveryBubbles = screen.getAllByText(recoveryText);
    expect(recoveryBubbles).toHaveLength(2);
    const firstRecoveryRow = recoveryBubbles[0].closest(
      ".message-row",
    ) as HTMLElement;
    const secondRecoveryRow = recoveryBubbles[1].closest(
      ".message-row",
    ) as HTMLElement;
    expect(firstRecoveryRow.className).toContain("message-row-user");
    expect(secondRecoveryRow.className).toContain("message-row-user");

    const initialReasoningRow = screen
      .getByText("first loop reasoning")
      .closest(".message-row") as HTMLElement;
    expect(initialReasoningRow).not.toBe(firstRecoveryAnswerRow);
    expect(initialReasoningRow).not.toBe(secondRecoveryAnswerRow);

    // Order: interrupted assistant → recovery user → first recovery assistant →
    // recovery user → latest assistant.
    expect(
      initialReasoningRow.compareDocumentPosition(recoveryBubbles[0]) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      recoveryBubbles[0].compareDocumentPosition(firstRecoveryAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      firstRecoveryAnswer.compareDocumentPosition(recoveryBubbles[1]) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      recoveryBubbles[1].compareDocumentPosition(secondRecoveryAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    // Second recovery content only lands on the newest assistant bubble.
    expect(
      within(secondRecoveryAnswerRow as HTMLElement).getByText(/second_tool/),
    ).toBeInTheDocument();
    expect(
      within(secondRecoveryAnswerRow as HTMLElement).queryByText(
        "first recovery answer",
      ),
    ).not.toBeInTheDocument();
    expect(
      within(secondRecoveryAnswerRow as HTMLElement).queryByText(/first_tool/),
    ).not.toBeInTheDocument();
    expect(
      within(secondRecoveryAnswerRow as HTMLElement).queryByText(
        "first loop reasoning",
      ),
    ).not.toBeInTheDocument();

    // Older assistant bubbles must not absorb second-recovery events.
    expect(
      within(initialReasoningRow).queryByText("second recovery answer"),
    ).not.toBeInTheDocument();
    expect(
      within(initialReasoningRow).queryByText(/second_tool/),
    ).not.toBeInTheDocument();
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).queryByText(
        "second recovery answer",
      ),
    ).not.toBeInTheDocument();
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).queryByText(/second_tool/),
    ).not.toBeInTheDocument();
    // First recovery content stays on its own bubble.
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).getByText(
        "first recovery answer",
      ),
    ).toBeInTheDocument();
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).getByText(/first_tool/),
    ).toBeInTheDocument();
    expect(
      within(firstRecoveryAnswerRow as HTMLElement).getByText(
        "second loop reasoning",
      ),
    ).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("expands history userInterruption parts into stable non-editable user bubbles", async () => {
    appTestState.chatMessagesResponsesByChatKey = {
      "workspace-1/chat-1": {
        ...chatMessages,
        messages: [
          {
            content: "trigger loop",
            createdAt: "2026-06-10T08:00:00.000Z",
            extractedMemories: [],
            id: "message-user-loop",
            memoriesUsed: [],
            metrics: null,
            parts: [{ text: "trigger loop", type: "text" }],
            reasoning: null,
            role: "user",
            toolCalls: [],
          },
          {
            content: "partial final answer",
            createdAt: "2026-06-10T08:00:02.000Z",
            extractedMemories: [],
            id: "message-assistant-loop",
            memoriesUsed: [],
            metrics: {
              firstTokenLatencyMs: 200,
              llmRequestIds: ["req-final"],
              modelId: "gpt-test",
              outputTokens: 20,
              providerId: "openai",
              totalLatencyMs: 4000,
            },
            parts: [
              { text: "looping reasoning", type: "reasoning" },
              {
                content: "repeated reasoning loop, check and continue",
                id: "interrupt-hist-1",
                interruptedAssistantMetrics: {
                  firstTokenLatencyMs: 100,
                  llmRequestIds: ["req-1"],
                  modelId: "gpt-test",
                  outputTokens: 5,
                  providerId: "openai",
                  totalLatencyMs: 1200,
                },
                source: "reasoningLoopGuard",
                type: "userInterruption",
              },
              { text: "final answer", type: "text" },
              {
                content: "repeated reasoning loop, check and continue",
                id: "interrupt-hist-2",
                interruptedAssistantMetrics: {
                  firstTokenLatencyMs: 150,
                  llmRequestIds: ["req-2"],
                  modelId: "gpt-test",
                  outputTokens: 8,
                  providerId: "openai",
                  totalLatencyMs: 2000,
                },
                source: "reasoningLoopGuard",
                type: "userInterruption",
              },
              { text: "after second recovery", type: "text" },
            ],
            reasoning: "looping reasoning",
            role: "assistant",
            toolCalls: [],
          },
        ] as typeof chatMessages.messages,
        pagination: { hasMoreBefore: false, nextBeforeSequence: null },
      },
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const recoveryBubbles = await screen.findAllByText(
      "repeated reasoning loop, check and continue",
    );
    expect(recoveryBubbles).toHaveLength(2);

    const firstRecoveryRow = recoveryBubbles[0].closest(
      ".message-row",
    ) as HTMLElement | null;
    const secondRecoveryRow = recoveryBubbles[1].closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(firstRecoveryRow?.className).toContain("message-row-user");
    expect(secondRecoveryRow?.className).toContain("message-row-user");
    expect(
      within(firstRecoveryRow as HTMLElement).queryByRole("button", {
        name: "Edit message",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(secondRecoveryRow as HTMLElement).queryByRole("button", {
        name: "Edit message",
      }),
    ).not.toBeInTheDocument();

    const realUserRow = screen
      .getByText("trigger loop")
      .closest(".message-row") as HTMLElement | null;
    expect(
      within(realUserRow as HTMLElement).getByRole("button", {
        name: "Edit message",
      }),
    ).toBeInTheDocument();

    const firstAnswer = screen.getByText("final answer");
    const secondAnswer = screen.getByText("after second recovery");
    expect(
      recoveryBubbles[0].compareDocumentPosition(firstAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      firstAnswer.compareDocumentPosition(recoveryBubbles[1]) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      recoveryBubbles[1].compareDocumentPosition(secondAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    // Final metrics on last assistant segment only.
    const secondAnswerRow = secondAnswer.closest(".message-row") as HTMLElement;
    expect(
      within(secondAnswerRow).getByText("Total time: 4 sec"),
    ).toBeInTheDocument();
    const firstAnswerRow = firstAnswer.closest(".message-row") as HTMLElement;
    expect(
      within(firstAnswerRow).getByText("Total time: 2 sec"),
    ).toBeInTheDocument();
  });

  it("keeps updating a pre-guidance tool block after guidance is applied", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-before-guidance",
          input: { path: "src/index.ts" },
          isError: false,
          name: "pre_guidance_tool",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    const toolName = await screen.findByText("pre_guidance_tool");
    const interruptedAssistantRow = toolName.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(interruptedAssistantRow).not.toBeNull();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("running"),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        content: "avoid the risky path",
        id: "guidance-before-tool-finish",
        interruptedAssistantMetrics: null,
        parts: [],
        type: "guidanceApplied",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Use safer option.",
        type: "textDelta",
      });
    });

    const guidedAnswer = await screen.findByText("Use safer option.");
    const guidedAssistantRow = guidedAnswer.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(guidedAssistantRow).not.toBeNull();
    expect(guidedAssistantRow).not.toBe(interruptedAssistantRow);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "partial output",
        stream: "stdout",
        toolCallId: "call-before-guidance",
        type: "toolOutputDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: "finished output",
        toolCallId: "call-before-guidance",
        type: "toolResult",
      });
    });

    await waitFor(() =>
      expect(
        within(interruptedAssistantRow as HTMLElement).queryByText("running"),
      ).not.toBeInTheDocument(),
    );
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("completed"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        /finished output/,
      ),
    ).toBeInTheDocument();
    expect(
      within(guidedAssistantRow as HTMLElement).queryByText(
        "Pre Guidance Tool",
      ),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("updates a streaming run_command preview in place when full input arrives", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "run tests",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const assistantMessageId = "message-assistant-stream";
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Before command.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-run-command",
          input: '{"',
          isError: false,
          name: "run_command",
          output: null,
          startedAt: "started-at",
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "After command.",
        type: "textDelta",
      });
    });

    const toolName = await screen.findByText("Run");
    const assistantRow = toolName.closest(".message-row") as HTMLElement | null;
    expect(assistantRow).not.toBeNull();
    const row = assistantRow as HTMLElement;
    const beforeText = within(row).getByText("Before command.");
    expect(within(row).getAllByText("Run")).toHaveLength(1);
    expect(within(row).getByText("running")).toBeInTheDocument();
    expect(
      beforeText.compareDocumentPosition(toolName) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const afterText = await within(row).findByText("After command.");
    expect(
      toolName.compareDocumentPosition(afterText) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-run-command",
          input: {
            args: ["run", "test", "--", "--watch=false"],
            command: "npm",
            cwd: "web",
          },
          isError: false,
          name: "run_command",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    const fullCommand = "npm run test -- --watch=false | cwd: web";
    expect(await within(row).findByText(fullCommand)).toBeInTheDocument();
    const updatedToolName = within(row).getByText("Run");
    expect(within(row).getAllByText("Run")).toHaveLength(1);
    expect(
      beforeText.compareDocumentPosition(updatedToolName) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      updatedToolName.compareDocumentPosition(afterText) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "tests still running",
        stream: "stdout",
        toolCallId: "call-run-command",
        type: "toolOutputDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-run-command",
          input: {
            args: ["run", "test", "--", "--watch=false"],
            command: "npm",
            cwd: "web",
          },
          isError: false,
          name: "run_command",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });
    expect(within(row).getByText(/tests still running/)).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        isError: false,
        output: "tests done",
        startedAt: "started-at",
        completedAt: "completed-at",
        toolCallId: "call-run-command",
        type: "toolResult",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-run-command",
          input: {
            args: ["run", "test", "--", "--watch=false"],
            command: "npm",
            cwd: "web",
          },
          isError: false,
          name: "run_command",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
    });

    expect(within(row).queryByText("running")).not.toBeInTheDocument();
    expect(within(row).getByText("completed")).toBeInTheDocument();
    expect(within(row).getByText("Started")).toBeInTheDocument();
    expect(within(row).getByText("started-at")).toBeInTheDocument();
    expect(within(row).getByText("Ended")).toBeInTheDocument();
    expect(within(row).getByText("completed-at")).toBeInTheDocument();
    expect(within(row).getByText(/tests done/)).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  // Regression: bubble-visible stream events must paint on the active tab without a
  // tab switch (cache write alone is not enough when setMessages was deferred).
  // Hold deferStreamAuxiliaryUpdate so a mistaken re-wrap of bubble updates via that
  // helper never flushes — sync getByText must still pass. Does not intercept a bare
  // startTransition(() => setMessagesForChatKey(...)) call outside the helper.
  function installHeldStreamAuxiliaryUpdateScheduler() {
    const pending: Array<() => void> = [];
    const globalWithHook = globalThis as {
      __FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__?: (
        update: () => void,
      ) => void;
    };
    globalWithHook.__FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__ = (
      update,
    ) => {
      pending.push(update);
    };
    return {
      flush() {
        const queued = pending.splice(0, pending.length);
        for (const update of queued) {
          update();
        }
      },
      uninstall() {
        delete globalWithHook.__FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__;
      },
    };
  }

  it("renders toolCall and toolResult on the active tab without switching tabs", async () => {
    const auxiliaryScheduler = installHeldStreamAuxiliaryUpdateScheduler();
    try {
      renderApp();
      await userEvent.click(await screen.findByText("Tool run"));
      await userEvent.type(
        await screen.findByPlaceholderText(defaultComposerPlaceholder),
        "inspect files",
      );
      await userEvent.click(
        screen.getByRole("button", { name: "Send message" }),
      );
      await waitFor(() =>
        expect(appTestState.activeChatStreamController).not.toBeNull(),
      );

      const assistantMessageId = "message-assistant-stream";
      // Interleave high-frequency deltas with a sparse bubble event so deferred
      // message updates (startTransition) would lag behind the 32ms text flush path.
      await act(async () => {
        for (let index = 0; index < 12; index += 1) {
          enqueueChatStreamEvent({
            assistantMessageId,
            delta: `chunk-${index} `,
            type: "textDelta",
          });
          enqueueChatStreamEvent({
            assistantMessageId,
            delta: `think-${index} `,
            type: "reasoningDelta",
          });
        }
        enqueueChatStreamEvent({
          assistantMessageId,
          toolCall: {
            id: "call-live-read",
            input: { path: "README.md" },
            isError: false,
            name: "read_file",
            output: null,
            status: "running",
          },
          type: "toolCall",
        });
      });

      // Sync DOM assert while auxiliary updates stay held — no tab switch / reload.
      const toolLabel = screen.getByText("Read");
      const assistantRow = toolLabel.closest(
        ".message-row",
      ) as HTMLElement | null;
      expect(assistantRow).not.toBeNull();
      expect(
        within(assistantRow as HTMLElement).getByText("running"),
      ).toBeInTheDocument();

      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId,
          isError: false,
          output: { content: "# README" },
          toolCallId: "call-live-read",
          type: "toolResult",
        });
      });

      expect(
        within(assistantRow as HTMLElement).queryByText("running"),
      ).not.toBeInTheDocument();
      expect(
        within(assistantRow as HTMLElement).getByText("completed"),
      ).toBeInTheDocument();
      expect(screen.getAllByText("Read")).toHaveLength(1);
    } finally {
      await act(async () => {
        auxiliaryScheduler.flush();
      });
      auxiliaryScheduler.uninstall();
      await act(async () => {
        appTestState.activeChatStreamController?.close();
      });
    }
  });

  it("renders contextCompression on the active tab during continuous text deltas", async () => {
    const auxiliaryScheduler = installHeldStreamAuxiliaryUpdateScheduler();
    try {
      renderApp();
      await userEvent.click(await screen.findByText("Tool run"));
      await userEvent.type(
        await screen.findByPlaceholderText(defaultComposerPlaceholder),
        "keep streaming",
      );
      await userEvent.click(
        screen.getByRole("button", { name: "Send message" }),
      );
      await waitFor(() =>
        expect(appTestState.activeChatStreamController).not.toBeNull(),
      );

      const assistantMessageId = "message-assistant-stream";
      await act(async () => {
        for (let index = 0; index < 8; index += 1) {
          enqueueChatStreamEvent({
            assistantMessageId,
            delta: `delta-${index} `,
            type: "textDelta",
          });
        }
        enqueueChatStreamEvent({
          assistantMessageId,
          kind: "llm",
          status: "start",
          type: "contextCompression",
          detail: {
            kind: "llm",
            originalTokenCount: 9000,
            providerId: "openai",
            modelId: "gpt-test",
            startedAt: "2026-07-19T12:00:00Z",
            status: "start",
          },
        });
      });

      expect(screen.getByText("Context compression")).toBeInTheDocument();
      expect(screen.getByText("Compressing")).toBeInTheDocument();
      expect(
        screen.getByText("Context compression in progress"),
      ).toBeInTheDocument();
      expect(screen.getAllByText("Context compression")).toHaveLength(1);

      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId,
          kind: "llm",
          snapshotId: "live-compress-1",
          status: "completed",
          type: "contextCompression",
          detail: {
            kind: "llm",
            snapshotId: "live-compress-1",
            originalTokenCount: 9000,
            summaryTokenCount: 1200,
            providerId: "openai",
            modelId: "gpt-test",
            startedAt: "2026-07-19T12:00:00Z",
            completedAt: "2026-07-19T12:00:02Z",
            status: "completed",
          },
        });
      });

      expect(screen.getByText("Compressed")).toBeInTheDocument();
      expect(screen.getAllByText("Context compression")).toHaveLength(1);
    } finally {
      await act(async () => {
        auxiliaryScheduler.flush();
      });
      auxiliaryScheduler.uninstall();
      await act(async () => {
        appTestState.activeChatStreamController?.close();
      });
    }
  });

  it("renders tool and compression events on GET active-run reattach without switching tabs", async () => {
    const auxiliaryScheduler = installHeldStreamAuxiliaryUpdateScheduler();
    try {
      const fetchMock = vi.fn(
        async (input: RequestInfo | URL, init?: RequestInit) => {
          const url = typeof input === "string" ? input : input.toString();
          const path = url.startsWith("http://127.0.0.1")
            ? new URL(url).pathname
            : url.split("?")[0];

          if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
            return jsonResponse({
              messages: [
                chatMessages.messages[0],
                {
                  ...chatMessages.messages[1],
                  content: "",
                  id: "message-assistant-stream",
                  metrics: null,
                  parts: [],
                  reasoning: null,
                  status: "streaming",
                  toolCalls: [],
                },
              ],
              activeRun: {
                chatId: "chat-1",
                lastSequence: 0,
                runId: "request-stream",
                workspaceId: "workspace-1",
              },
            });
          }

          return mockFetch(input, init);
        },
      );
      vi.stubGlobal("fetch", fetchMock);
      window.history.replaceState(null, "", "/workspace-1/chat-1");
      renderApp();

      await waitFor(() => {
        expect(
          fetchMock.mock.calls.some(
            ([url]) =>
              typeof url === "string" &&
              url ===
                "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
          ),
        ).toBe(true);
      });
      await waitFor(() =>
        expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
          true,
        ),
      );

      const assistantMessageId = "message-assistant-stream";
      await act(async () => {
        for (let index = 0; index < 6; index += 1) {
          enqueueChatStreamEventForRun("request-stream", {
            assistantMessageId,
            delta: `reattach-${index} `,
            type: "textDelta",
          });
        }
        enqueueChatStreamEventForRun("request-stream", {
          assistantMessageId,
          toolCall: {
            id: "call-reattach-read",
            input: { path: "src/main.rs" },
            isError: false,
            name: "read_file",
            output: null,
            status: "running",
          },
          type: "toolCall",
        });
        enqueueChatStreamEventForRun("request-stream", {
          assistantMessageId,
          kind: "rule",
          status: "start",
          type: "contextCompression",
          detail: {
            kind: "rule",
            originalTokenCount: 1500,
            providerId: "openai",
            modelId: "gpt-test",
            startedAt: "2026-07-19T13:00:00Z",
            status: "start",
          },
        });
      });

      expect(screen.getByText("Read")).toBeInTheDocument();
      const reattachToolLabel = screen.getByText("Read");
      const reattachAssistantRow = reattachToolLabel.closest(
        ".message-row",
      ) as HTMLElement | null;
      expect(reattachAssistantRow).not.toBeNull();
      expect(
        within(reattachAssistantRow as HTMLElement).getByText("running"),
      ).toBeInTheDocument();
      expect(screen.getByText("Context compression")).toBeInTheDocument();
      expect(screen.getByText("Compressing")).toBeInTheDocument();

      await act(async () => {
        enqueueChatStreamEventForRun("request-stream", {
          assistantMessageId,
          isError: false,
          output: { content: "fn main() {}" },
          toolCallId: "call-reattach-read",
          type: "toolResult",
        });
      });

      expect(
        within(reattachAssistantRow as HTMLElement).queryByText("running"),
      ).not.toBeInTheDocument();
      expect(
        within(reattachAssistantRow as HTMLElement).getByText("completed"),
      ).toBeInTheDocument();
    } finally {
      await act(async () => {
        auxiliaryScheduler.flush();
      });
      auxiliaryScheduler.uninstall();
      await act(async () => {
        appTestState.chatStreamControllers.get("request-stream")?.close();
      });
    }
  });

  it.each([
    ["start", "request-stream", "Compressing", true],
    ["completed", "request-stream", "Compressed", true],
    ["start then completed", "request-stream", "Compressed", true],
    ["completed from a replacement run", "request-replacement", null, false],
  ] as const)(
    "keeps only the applicable live compression after a delayed GET active-run messages snapshot resolves (%s)",
    async (lifecycle, delayedSnapshotRunId, expectedStatus, shouldPreserve) => {
      const delayedMessages = deferred<Response>();
      let messagesRequestCount = 0;
      const assistantMessageId = "message-assistant-stream";
      const messagesPayload = {
        messages: [
          chatMessages.messages[0],
          {
            ...chatMessages.messages[1],
            content: "",
            id: assistantMessageId,
            metrics: null,
            parts: [],
            reasoning: null,
            status: "streaming",
            toolCalls: [],
          },
        ],
        activeRun: {
          assistantMessageId,
          chatId: "chat-1",
          lastSequence: 0,
          runId: "request-stream",
          workspaceId: "workspace-1",
        },
      };
      const fetchMock = vi.fn(
        async (input: RequestInfo | URL, init?: RequestInit) => {
          const url = typeof input === "string" ? input : input.toString();
          const path = url.startsWith("http://127.0.0.1")
            ? new URL(url).pathname
            : url.split("?")[0];
          if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
            messagesRequestCount += 1;
            return messagesRequestCount === 1
              ? jsonResponse(messagesPayload)
              : delayedMessages.promise;
          }
          return mockFetch(input, init);
        },
      );
      vi.stubGlobal("fetch", fetchMock);
      window.history.replaceState(null, "", "/workspace-1/chat-1");
      renderApp();

      await waitFor(() =>
        expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
          true,
        ),
      );

      // An identity correction starts a refresh, but the server response is held
      // to model the pre-SSE `/messages` snapshot that caused the regression.
      await act(async () => {
        enqueueChatStreamEventForRun("request-stream", {
          assistantMessageId: "stale-alias",
          delta: "trigger refresh",
          type: "textDelta",
        });
      });
      await waitFor(() => expect(messagesRequestCount).toBe(2));

      await act(async () => {
        if (lifecycle !== "completed") {
          enqueueChatStreamEventForRun("request-stream", {
            assistantMessageId,
            kind: "llm",
            status: "start",
            type: "contextCompression",
            detail: {
              compressionId: "compression-race-1",
              kind: "llm",
              originalTokenCount: 9000,
              providerId: "openai",
              modelId: "gpt-test",
              startedAt: "2026-07-23T10:00:00Z",
              status: "start",
            },
          });
        }
        if (lifecycle !== "start") {
          enqueueChatStreamEventForRun("request-stream", {
            assistantMessageId,
            compressionId: "compression-race-1",
            kind: "llm",
            snapshotId: "snapshot-race-1",
            status: "completed",
            type: "contextCompression",
            detail: {
              completedAt: "2026-07-23T10:00:02Z",
              compressionId: "compression-race-1",
              kind: "llm",
              modelId: "gpt-test",
              originalTokenCount: 9000,
              providerId: "openai",
              snapshotId: "snapshot-race-1",
              startedAt: "2026-07-23T10:00:00Z",
              status: "completed",
              summaryTokenCount: 1200,
            },
          });
        }
      });
      expect(
        screen.getByText(expectedStatus ?? "Compressed"),
      ).toBeInTheDocument();

      await act(async () => {
        delayedMessages.resolve(
          jsonResponse({
            ...messagesPayload,
            activeRun: {
              ...messagesPayload.activeRun,
              runId: delayedSnapshotRunId,
            },
          }),
        );
      });

      await waitFor(() => {
        if (shouldPreserve) {
          expect(
            screen.getByText(expectedStatus ?? "Compressed"),
          ).toBeInTheDocument();
          expect(screen.getAllByText("Context compression")).toHaveLength(1);
        } else {
          expect(
            screen.queryByText("Context compression"),
          ).not.toBeInTheDocument();
        }
      });
      if (shouldPreserve && lifecycle !== "start") {
        await userEvent.click(screen.getByText("Context compression"));
        expect(screen.getByText("snapshot-race-1")).toBeInTheDocument();
        expect(
          screen.getByText(/Saved 7,800 tokens|Saved 7800 tokens/),
        ).toBeInTheDocument();
      }
      await act(async () => {
        appTestState.chatStreamControllers.get("request-stream")?.close();
      });
    },
  );

  it("keeps a background compression lifecycle cached until its tab is restored", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "background work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
    );

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-background-read",
          input: { path: "hidden.md" },
          isError: false,
          name: "read_file",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: { content: "secret" },
        toolCallId: "call-background-read",
        type: "toolResult",
      });
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        kind: "llm",
        status: "start",
        type: "contextCompression",
        detail: {
          kind: "llm",
          originalTokenCount: 2000,
          providerId: "openai",
          modelId: "gpt-test",
          startedAt: "2026-07-19T14:00:00Z",
          status: "start",
        },
      });
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        compressionId: "compression-background",
        kind: "llm",
        snapshotId: "snapshot-background",
        status: "completed",
        type: "contextCompression",
        detail: {
          completedAt: "2026-07-19T14:00:02Z",
          compressionId: "compression-background",
          kind: "llm",
          modelId: "gpt-test",
          originalTokenCount: 2000,
          providerId: "openai",
          snapshotId: "snapshot-background",
          startedAt: "2026-07-19T14:00:00Z",
          status: "completed",
          summaryTokenCount: 400,
        },
      });
    });

    // Background stream updates cache only — active tab stays on Second chat.
    expect(screen.getByText("Second answer.")).toBeInTheDocument();
    expect(screen.queryByText("Read")).not.toBeInTheDocument();
    expect(screen.queryByText("Context compression")).not.toBeInTheDocument();

    const chatTabList = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(within(chatTabList).getByText("Tool run"));

    const backgroundToolLabel = await screen.findByText("Read");
    const backgroundAssistantRow = backgroundToolLabel.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(backgroundAssistantRow).not.toBeNull();
    expect(
      within(backgroundAssistantRow as HTMLElement).getByText("completed"),
    ).toBeInTheDocument();
    expect(screen.getByText("Compressed")).toBeInTheDocument();
    expect(screen.queryByText("Compressing")).not.toBeInTheDocument();
    expect(screen.getAllByText("Read")).toHaveLength(1);
    expect(screen.getAllByText("Context compression")).toHaveLength(1);
    await userEvent.click(screen.getByText("Context compression"));
    expect(screen.getByText("snapshot-background")).toBeInTheDocument();
    expect(
      screen.getByText(/Saved 1,600 tokens|Saved 1600 tokens/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Second answer.")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("shows generated image files from direct and delegated tool results", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "generate an image",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const directPath = ".foco/sessions/chat-1/image_gen/run-1/image.png";
    const delegatedPath = ".foco/sessions/chat-1/image_gen/run-2/image.png";
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-image-gen",
          input: { prompt: "a quiet workspace" },
          isError: false,
          name: "image_gen",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: {
          files: [
            {
              bytes: 2048,
              mimeType: "image/png",
              path: directPath,
            },
          ],
        },
        toolCallId: "call-image-gen",
        type: "toolResult",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-agent-wait",
          input: { taskIds: ["agent-task-image"] },
          isError: false,
          name: "agent_wait_tasks",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: {
          dependencies: [
            {
              result: {
                text: `Generated image: ${delegatedPath}`,
              },
              status: "completed",
              taskId: "agent-task-image",
            },
          ],
          waiting: false,
        },
        toolCallId: "call-agent-wait",
        type: "toolResult",
      });
    });

    const directImage = await screen.findByAltText(directPath);
    const delegatedImage = await screen.findByAltText(delegatedPath);
    expect(directImage).toHaveAttribute(
      "src",
      `/api/workspaces/workspace-1/files/blob?path=${encodeURIComponent(directPath)}`,
    );
    expect(delegatedImage).toHaveAttribute(
      "src",
      `/api/workspaces/workspace-1/files/blob?path=${encodeURIComponent(delegatedPath)}`,
    );
    expect(screen.getAllByText(directPath).length).toBeGreaterThan(0);
    expect(screen.getAllByText(delegatedPath).length).toBeGreaterThan(0);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps a resumed agent-team reply in the original assistant bubble", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "test multi-agent resume",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const assistantMessageId = "message-assistant-stream";

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Planning handoff.",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Waiting for worker.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-delegate",
          input: {
            targetInstanceId: "agent-instance-worker-1",
            input: { message: "do work" },
          },
          isError: false,
          name: "agent_delegate_task",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        isError: false,
        output: { taskId: "agent-task-worker-1" },
        toolCallId: "call-delegate",
        type: "toolResult",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-wait",
          input: { mode: "all", taskIds: ["agent-task-worker-1"] },
          isError: false,
          name: "agent_wait_tasks",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        isError: false,
        output: { waiting: true, suspend: true },
        toolCallId: "call-wait",
        type: "toolResult",
      });
    });
    expect(await screen.findByText("Waiting for worker.")).toBeInTheDocument();
    expect(screen.getByText("Planning handoff.")).toBeInTheDocument();
    expect(screen.getByText("Delegate Task")).toBeInTheDocument();
    expect(screen.getByText("Wait Tasks")).toBeInTheDocument();
    const waitingRow = screen
      .getByText("Waiting for worker.")
      .closest(".message-row") as HTMLElement | null;
    expect(waitingRow).not.toBeNull();
    expect(
      within(waitingRow as HTMLElement).getAllByText("Waiting for worker.")
        .length,
    ).toBeGreaterThan(0);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        chatId: "queued-chat-1",
        llmRequestId: "request-stream",
        memoriesUsed: [],
        type: "start",
        userMessageId: "message-user-stream",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Final worker summary.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        chatId: "queued-chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 3,
          providerId: "provider-1",
          totalLatencyMs: 1000,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Final worker summary.",
        type: "complete",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 10,
          outputTokens: 3,
        },
      });
    });

    await waitFor(() =>
      expect(waitingRow).toHaveTextContent("Final worker summary."),
    );
    expect(waitingRow).toHaveTextContent("Waiting for worker.");
    expect(waitingRow).toHaveTextContent("Planning handoff.");
    expect(
      within(waitingRow as HTMLElement).getByText("Delegate Task"),
    ).toBeInTheDocument();
    expect(
      within(waitingRow as HTMLElement).getByText("Wait Tasks"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Waiting for worker.").length).toBeGreaterThan(
      0,
    );
    expect(screen.getAllByText("Delegate Task")).toHaveLength(1);

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
      appTestState.activeChatStreamController?.close();
    });
  });

  it("preserves pre-delegate history across GET active-run reattach and start", async () => {
    const assistantMessageId = "message-assistant-stream";
    const fetchMock = vi.mocked(fetch);
    let reattachActiveRun = false;
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return Promise.resolve(
          jsonResponse({
            ...chatMessages,
            messages: [
              chatMessages.messages[0],
              chatMessages.messages[1],
              {
                content: "delegate then wait",
                createdAt: "2026-06-10T08:00:00.000Z",
                extractedMemories: [],
                id: "message-user-stream",
                memoriesUsed: [],
                metrics: null,
                parts: [{ text: "delegate then wait", type: "text" }],
                reasoning: null,
                role: "user",
                toolCalls: [],
              },
              {
                content: "Before handoff.",
                createdAt: "2026-06-10T08:00:01.000Z",
                extractedMemories: [],
                id: assistantMessageId,
                memoriesUsed: [],
                metrics: null,
                parts: [
                  { text: "Pre-delegate reasoning.", type: "reasoning" },
                  { text: "Before handoff.", type: "text" },
                  {
                    toolCall: {
                      id: "call-delegate-reattach",
                      input: { targetInstanceId: "agent-instance-worker-2" },
                      isError: false,
                      name: "agent_delegate_task",
                      output: { taskId: "agent-task-worker-2" },
                      status: "completed",
                    },
                    type: "toolCall",
                  },
                  {
                    toolCall: {
                      id: "call-wait-reattach",
                      input: {
                        mode: "all",
                        taskIds: ["agent-task-worker-2"],
                      },
                      isError: false,
                      name: "agent_wait_tasks",
                      output: { waiting: true, suspend: true },
                      status: "completed",
                    },
                    type: "toolCall",
                  },
                ],
                reasoning: "Pre-delegate reasoning.",
                role: "assistant",
                status: "streaming",
                toolCalls: [
                  {
                    id: "call-delegate-reattach",
                    input: { targetInstanceId: "agent-instance-worker-2" },
                    isError: false,
                    name: "agent_delegate_task",
                    output: { taskId: "agent-task-worker-2" },
                    status: "completed",
                  },
                  {
                    id: "call-wait-reattach",
                    input: {
                      mode: "all",
                      taskIds: ["agent-task-worker-2"],
                    },
                    isError: false,
                    name: "agent_wait_tasks",
                    output: { waiting: true, suspend: true },
                    status: "completed",
                  },
                ],
              },
            ],
            activeRun: reattachActiveRun
              ? {
                  acceptingGuidance: true,
                  chatId: "chat-1",
                  lastSequence: 12,
                  runId: "request-stream-resumed",
                  workspaceId: "workspace-1",
                }
              : null,
          }),
        );
      }
      if (
        path ===
        "/api/workspaces/workspace-1/chat/runs/request-stream-resumed/stream"
      ) {
        const encoder = new TextEncoder();
        const stream = new ReadableStream<Uint8Array>({
          start(controller) {
            appTestState.chatStreamControllers.set(
              "request-stream-resumed",
              controller,
            );
            appTestState.activeChatStreamController = controller;
            controller.enqueue(
              encoder.encode(
                `id: 13\ndata: ${JSON.stringify({
                  assistantMessageId,
                  chatId: "chat-1",
                  llmRequestId: "request-stream-resumed",
                  memoriesUsed: [],
                  type: "start",
                  userMessageId: "message-user-stream",
                })}\n\n`,
              ),
            );
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

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "delegate then wait",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Pre-delegate reasoning.",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        delta: "Before handoff.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-delegate-reattach",
          input: { targetInstanceId: "agent-instance-worker-2" },
          isError: false,
          name: "agent_delegate_task",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        isError: false,
        output: { taskId: "agent-task-worker-2" },
        toolCallId: "call-delegate-reattach",
        type: "toolResult",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        toolCall: {
          id: "call-wait-reattach",
          input: { mode: "all", taskIds: ["agent-task-worker-2"] },
          isError: false,
          name: "agent_wait_tasks",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId,
        isError: false,
        output: { waiting: true, suspend: true },
        toolCallId: "call-wait-reattach",
        type: "toolResult",
      });
    });

    const historyRows = await screen.findAllByText("Before handoff.");
    const historyRow = historyRows[0]?.closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(historyRow).not.toBeNull();
    expect(historyRow).toHaveTextContent("Pre-delegate reasoning.");
    expect(
      within(historyRow as HTMLElement).getByText("Delegate Task"),
    ).toBeInTheDocument();
    expect(
      within(historyRow as HTMLElement).getByText("Wait Tasks"),
    ).toBeInTheDocument();
    // Content may appear in both summary and body nodes inside one bubble.
    expect(
      new Set(
        screen
          .getAllByText("Before handoff.")
          .map((node) => node.closest(".message-row")),
      ).size,
    ).toBe(1);

    // Wait gap: activeRun temporarily null while durable run remains running.
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: null,
                queuedRun: {
                  assistantMessageId,
                  content: "delegate then wait",
                  modelId: "gpt-test",
                  providerId: "openai",
                  skillIds: [],
                  status: "running",
                  thinkingLevel: null,
                  userMessageId: "message-user-stream",
                },
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Cancel run" }),
      ).not.toBeInTheDocument(),
    );

    // Later attempt appears under a new runId; reopen chat to take GET reattach path.
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: {
                  acceptingGuidance: true,
                  chatId: "chat-1",
                  lastSequence: 12,
                  runId: "request-stream-resumed",
                  workspaceId: "workspace-1",
                },
                queuedRun: {
                  assistantMessageId,
                  content: "delegate then wait",
                  modelId: "gpt-test",
                  providerId: "openai",
                  skillIds: [],
                  status: "running",
                  thinkingLevel: null,
                  userMessageId: "message-user-stream",
                },
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];
    reattachActiveRun = true;

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh workspaces" }),
    );
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await userEvent.click(within(workspaceList).getByText("Tool run"));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url.includes(
              "/api/workspaces/workspace-1/chat/runs/request-stream-resumed/stream",
            ),
        ),
      ).toBe(true);
    });

    // History must survive the reattach `start` before later deltas arrive.
    const historyAfterStart = await screen.findAllByText("Before handoff.");
    expect(
      new Set(historyAfterStart.map((node) => node.closest(".message-row")))
        .size,
    ).toBe(1);
    expect(screen.getByText("Pre-delegate reasoning.")).toBeInTheDocument();
    expect(screen.getByText("Delegate Task")).toBeInTheDocument();
    expect(screen.getByText("Wait Tasks")).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream-resumed", {
        assistantMessageId,
        delta: "Post-wait summary.",
        type: "textDelta",
      });
    });

    const resumedRow = (await screen.findByText("Post-wait summary.")).closest(
      ".message-row",
    ) as HTMLElement | null;
    expect(resumedRow).not.toBeNull();
    expect(resumedRow).toHaveTextContent("Before handoff.");
    expect(resumedRow).toHaveTextContent("Pre-delegate reasoning.");
    expect(
      within(resumedRow as HTMLElement).getByText("Delegate Task"),
    ).toBeInTheDocument();
    expect(
      within(resumedRow as HTMLElement).getByText("Wait Tasks"),
    ).toBeInTheDocument();
    expect(
      new Set(
        screen
          .getAllByText("Before handoff.")
          .map((node) => node.closest(".message-row")),
      ).size,
    ).toBe(1);
    expect(screen.getAllByText("Delegate Task")).toHaveLength(1);

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream-resumed")?.close();
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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/cancel",
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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    await userEvent.click(screen.getByRole("option", { name: "GPT Test" }));
    await userEvent.click(screen.getByRole("button", { name: /Thinking/ }));
    await userEvent.click(screen.getByRole("option", { name: "High" }));
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
      modelId: "gpt-test",
      // Composer derives provider from the model's active route (openai in harness).
      providerId: "openai",
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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
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
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
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
    const encoder = new TextEncoder();
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
    const runningStatus = within(tabList).getByRole("status", {
      name: "Chat is running",
    });
    expect(runningStatus).toBeInTheDocument();
    expect(runningStatus.querySelector("svg")).toHaveClass(
      "chat-tab-running-spinner",
    );
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

    const contextUsageCallsBeforeStart = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    ).length;
    await act(async () => {
      delayedStreamController?.enqueue(
        encoder.encode(
          `data: ${JSON.stringify({
            type: "start",
            chatId: "server-chat-new",
            userMessageId: "server-user-new",
            assistantMessageId: "server-assistant-new",
            llmRequestId: "server-run-new",
            memoriesUsed: [],
          })}\n\n`,
        ),
      );
    });

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/context-usage",
        ),
      ).toHaveLength(contextUsageCallsBeforeStart + 1);
    });
    const contextUsageCalls = fetchMock.mock.calls.filter(
      ([url]) => typeof url === "string" && url.endsWith("/context-usage"),
    );
    const [, contextUsageInit] = contextUsageCalls.at(-1)!;
    expect(JSON.parse(String(contextUsageInit?.body))).toMatchObject({
      chatId: "server-chat-new",
    });
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");

    await act(async () => {
      delayedStreamController?.close();
    });
  });

  it("keeps workspace identity idle while queueing a new chat", async () => {
    const fetchMock = vi.mocked(fetch);
    let statsSignal: AbortSignal | null = null;
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/ai-statistics") {
        statsSignal = init?.signal ?? null;
        return Promise.resolve(jsonResponse(aiStatistics));
      }

      return mockFetch(input, init);
    });
    renderApp();

    expect(
      await screen.findByRole("heading", { name: workspace.name }),
    ).toBeInTheDocument();
    expect(statsSignal).toBeNull();
    await userEvent.type(
      await screen.findByRole("textbox"),
      "stats must not block",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/chat/queue",
        ),
      ).toBe(true),
    );
    expect(statsSignal).toBeNull();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("toggles the API details auto-refresh control without a loading spinner", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/ai-statistics") {
        return Promise.resolve(jsonResponse(aiStatistics));
      }

      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(
      (await screen.findAllByRole("button", { name: "API details" }))[0],
    );
    expect(await screen.findByText("API details")).toBeInTheDocument();

    const pauseButton = await screen.findByRole("button", {
      name: "Pause auto refresh",
    });
    expect(pauseButton).not.toBeDisabled();
    const pauseIcon = pauseButton.querySelector("svg");
    if (!(pauseIcon instanceof SVGElement)) {
      throw new Error("pause icon was not rendered");
    }
    expect(pauseIcon).toHaveClass("lucide-pause");
    expect(pauseIcon).not.toHaveClass("api-refresh-icon");
    expect(pauseIcon).not.toHaveAttribute("data-loading");

    await userEvent.click(pauseButton);

    const resumeButton = await screen.findByRole("button", {
      name: "Resume auto refresh",
    });
    const resumeIcon = resumeButton.querySelector("svg");
    if (!(resumeIcon instanceof SVGElement)) {
      throw new Error("resume icon was not rendered");
    }
    expect(resumeIcon).toHaveClass("lucide-play");
    expect(resumeButton).not.toBeDisabled();
  });

  it("schedules a new workspace chat until the current workspace run finishes", async () => {
    const fetchMock = vi.mocked(fetch);
    const consoleErrorSpy = vi.spyOn(console, "error");
    fetchMock.mockImplementation(async (input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chat/stream") {
        const body =
          typeof init?.body === "string"
            ? (JSON.parse(init.body) as {
                chatId?: string | null;
                message?: string;
              })
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
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
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

    const queueCall = fetchMock.mock.calls.find(
      ([url, init]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/queue" &&
        typeof init?.body === "string" &&
        JSON.parse(init.body).message === "Scheduled task",
    );
    expect(queueCall).toBeDefined();
    expect(JSON.parse(String(queueCall?.[1]?.body))).toMatchObject({
      deferStart: true,
      message: "Scheduled task",
    });

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const scheduledHistoryTitle =
      await within(workspaceList).findByText("Scheduled task");
    expect(within(workspaceList).getAllByText("Scheduled task")).toHaveLength(
      1,
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

    const queuedTabList = await screen.findByRole("tablist", { name: "Chat" });
    const queuedTabs = within(queuedTabList).getAllByRole("tab", {
      name: /Scheduled task/,
    });
    expect(queuedTabs).toHaveLength(1);
    expect(queuedTabs[0]).toHaveAttribute("aria-selected", "true");

    const tabListBeforeComplete = await screen.findByRole("tablist", {
      name: "Chat",
    });
    await userEvent.click(
      within(tabListBeforeComplete).getByRole("tab", { name: /Tool run/ }),
    );
    expect(
      await screen.findByText("Please inspect README."),
    ).toBeInTheDocument();

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
    expect(
      within(tabList).getByRole("tab", { name: /Tool run/ }),
    ).toHaveAttribute("aria-selected", "true");
    const activeMessageList = document.querySelector(".message-list");
    if (!(activeMessageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }
    expect(
      within(activeMessageList).getByText("Please inspect README."),
    ).toBeInTheDocument();
    expect(
      within(activeMessageList).queryByText("Scheduled task"),
    ).not.toBeInTheDocument();
    expect(
      within(activeMessageList).queryByText("Scheduled answer."),
    ).not.toBeInTheDocument();

    await userEvent.click(
      within(tabList).getByRole("tab", { name: /Scheduled task/ }),
    );
    const scheduledMessageList = document.querySelector(".message-list");
    if (!(scheduledMessageList instanceof HTMLElement)) {
      throw new Error("Expected scheduled message list");
    }
    expect(screen.getAllByText("Scheduled task").length).toBeGreaterThan(0);
    expect(await screen.findByText("Scheduled answer.")).toBeInTheDocument();
    expect(
      consoleErrorSpy.mock.calls
        .flat()
        .some(
          (entry) =>
            String(entry).includes(
              "Encountered two children with the same key",
            ) && String(entry).includes("queued-chat-2"),
        ),
    ).toBe(false);

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
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
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

    const heldCtrlQueueCall = fetchMock.mock.calls.find(
      ([url, init]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/queue" &&
        typeof init?.body === "string" &&
        JSON.parse(init.body).message === "Held Ctrl scheduled task",
    );
    expect(heldCtrlQueueCall).toBeDefined();
    expect(JSON.parse(String(heldCtrlQueueCall?.[1]?.body))).toMatchObject({
      deferStart: true,
      message: "Held Ctrl scheduled task",
    });

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
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    const composer = screen.getByPlaceholderText(defaultComposerPlaceholder);
    changeInput(composer, "Keyboard scheduled task");
    composer.focus();
    await userEvent.keyboard("{Control>}{Enter}{/Control}");

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
    changeInput(
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
    const addAttachmentButton = screen.getByRole("button", {
      name: "Add attachment",
    });
    await waitFor(() => expect(addAttachmentButton).toBeEnabled());
    await userEvent.click(addAttachmentButton);
    const picker = await screen.findByRole("dialog", {
      name: "Add attachment",
    });

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) => typeof url === "string" && url === "/api/file-picker/list",
        ),
      ).toBe(true);
    });
    const listCall = fetchMock.mock.calls.find(
      ([url]) => typeof url === "string" && url === "/api/file-picker/list",
    );
    const listBody = JSON.parse(String(listCall?.[1]?.body ?? "{}")) as {
      allowOutsideWorkspace?: boolean;
      target?: { kind?: string; workspaceId?: string };
    };
    expect(listBody.target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });
    expect(listBody.allowOutsideWorkspace).toBe(true);

    await userEvent.click(
      within(picker).getByRole("button", { name: /note\.txt/ }),
    );
    await userEvent.click(
      within(picker).getByRole("button", { name: "Select" }),
    );
    expect(await screen.findByText("note.txt")).toBeInTheDocument();

    const readCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" && url === "/api/file-picker/read-files",
    );
    expect(readCall).toBeDefined();
    const readBody = JSON.parse(String(readCall?.[1]?.body ?? "{}")) as {
      allowOutsideWorkspace?: boolean;
      target?: { kind?: string; workspaceId?: string };
    };
    expect(readBody.allowOutsideWorkspace).toBe(true);
    expect(readBody.target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    await userEvent.click(screen.getByRole("option", { name: "GPT Test" }));
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Review it",
    );
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
        modelId: "gpt-test",
        // Composer derives provider from the model's active route (openai in harness).
        providerId: "openai",
      }),
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("opens the attachment picker with allowOutsideWorkspace from message edit UI", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit message" }),
    );

    const editAttachmentButtons = screen.getAllByRole("button", {
      name: "Add attachment",
    });
    // Composer + inline edit both expose Add attachment; use the last (edit UI).
    await userEvent.click(
      editAttachmentButtons[editAttachmentButtons.length - 1]!,
    );
    const picker = await screen.findByRole("dialog", {
      name: "Add attachment",
    });

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) => typeof url === "string" && url === "/api/file-picker/list",
        ),
      ).toBe(true);
    });
    const listCall = [...fetchMock.mock.calls]
      .reverse()
      .find(
        ([url]) => typeof url === "string" && url === "/api/file-picker/list",
      );
    const listBody = JSON.parse(String(listCall?.[1]?.body ?? "{}")) as {
      allowOutsideWorkspace?: boolean;
      target?: { kind?: string; workspaceId?: string };
    };
    expect(listBody.allowOutsideWorkspace).toBe(true);
    expect(listBody.target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });

    await userEvent.click(
      within(picker).getByRole("button", { name: /note\.txt/ }),
    );
    await userEvent.click(
      within(picker).getByRole("button", { name: "Select" }),
    );

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" && url === "/api/file-picker/read-files",
        ),
      ).toBe(true);
    });
    const readCall = [...fetchMock.mock.calls]
      .reverse()
      .find(
        ([url]) =>
          typeof url === "string" && url === "/api/file-picker/read-files",
      );
    const readBody = JSON.parse(String(readCall?.[1]?.body ?? "{}")) as {
      allowOutsideWorkspace?: boolean;
    };
    expect(readBody.allowOutsideWorkspace).toBe(true);
  });

  it("blocks unsupported media attachments for the selected model", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findByText("Tool run");
    await userEvent.click(
      screen.getByRole("button", { name: "Add attachment" }),
    );
    const picker = await screen.findByRole("dialog", {
      name: "Add attachment",
    });
    await userEvent.click(
      within(picker).getByRole("button", { name: /screen\.png/ }),
    );
    await userEvent.click(
      within(picker).getByRole("button", { name: "Select" }),
    );

    expect(
      await screen.findByText(
        "Selected model does not support image attachments: screen.png",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("screen.png")).toBeNull();

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Review it",
    );
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
    expect(body.attachments).toEqual([]);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("allows media attachments when the selected model supports their modality", async () => {
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        {
          ...settings.configuredModels[0]!,
          inputModalities: ["text", "image"],
        },
      ],
    };
    renderApp();

    await screen.findByText("Tool run");
    await userEvent.click(
      screen.getByRole("button", { name: "Add attachment" }),
    );
    const picker = await screen.findByRole("dialog", {
      name: "Add attachment",
    });
    await userEvent.click(
      within(picker).getByRole("button", { name: /screen\.png/ }),
    );
    await userEvent.click(
      within(picker).getByRole("button", { name: "Select" }),
    );

    expect(await screen.findByText("screen.png")).toBeInTheDocument();
    expect(
      screen.queryByText(
        "Selected model does not support image attachments: screen.png",
      ),
    ).toBeNull();
  });

  it("defers streaming Mermaid rendering until the message completes", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "diagram",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "```mermaid\nflowchart TD",
        type: "textDelta",
      });
    });

    expect(await screen.findByText(/flowchart TD/)).toBeInTheDocument();
    expect(
      screen.queryByText("Mermaid diagram failed to render."),
    ).not.toBeInTheDocument();
    expect(mermaidMock.render).not.toHaveBeenCalled();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "\n  A --> B\n```",
        type: "textDelta",
      });
    });

    expect(await screen.findByText(/A --> B/)).toBeInTheDocument();
    expect(mermaidMock.render).not.toHaveBeenCalled();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-2",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 12,
          providerId: "provider-1",
          totalLatencyMs: 100,
        },
        reasoning: null,
        stopReason: "completed",
        text: "```mermaid\nflowchart TD\n  A --> B\n```",
        type: "complete",
        usage: null,
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

  it("renders streaming markdown as plain text and full markdown after complete", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "markdown",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    const markdown = [
      "Here is [docs](https://example.com).",
      "",
      "```ts",
      "console.log(1)",
      "```",
      "",
      "| A | B |",
      "| - | - |",
      "| 1 | 2 |",
      "",
      "$x^2$",
    ].join("\n");

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: markdown,
        type: "textDelta",
      });
    });

    const rawMarkdown = await screen.findByText(
      /\[docs\]\(https:\/\/example\.com\)/,
    );
    const streamingBubble = rawMarkdown.closest(
      ".message-bubble",
    ) as HTMLElement;
    expect(streamingBubble).not.toBeNull();
    expect(
      within(streamingBubble).queryByRole("link", { name: "docs" }),
    ).toBeNull();
    expect(streamingBubble.querySelector("pre code")).toBeNull();
    expect(streamingBubble.querySelector("table")).toBeNull();
    expect(streamingBubble.querySelector(".katex")).toBeNull();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-markdown",
          input: { path: "docs.md" },
          isError: false,
          name: "read_file",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: "# docs",
        toolCallId: "call-markdown",
        type: "toolResult",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 30,
          providerId: "provider-1",
          totalLatencyMs: 100,
        },
        reasoning: null,
        stopReason: "completed",
        text: markdown,
        type: "complete",
        usage: null,
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    const link = await screen.findByRole("link", { name: "docs" });
    const completeBubble = link.closest(".message-bubble") as HTMLElement;
    expect(link).toHaveAttribute("href", "https://example.com");
    expect(completeBubble.querySelector("pre code")).not.toBeNull();
    expect(completeBubble.querySelector("table")).not.toBeNull();
    expect(completeBubble.querySelector(".katex")).not.toBeNull();
    expect(within(completeBubble).getByText("Read")).toBeInTheDocument();
    expect(
      within(completeBubble).getByText("Model: model-1"),
    ).toBeInTheDocument();
  });

  it("renders POST stream markdown after streamEnd without a complete event", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "markdown stream end",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    const markdown = "# Stream-ended summary\n\n- **Rendered terminal item**";
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: markdown,
        type: "textDelta",
      });
    });

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    const heading = await screen.findByRole("heading", {
      name: "Stream-ended summary",
    });
    const completedBubble = heading.closest(".message-bubble") as HTMLElement;
    expect(
      completedBubble.querySelector(".markdown-content-assistant"),
    ).not.toBeNull();
    expect(within(completedBubble).getByRole("list")).toBeInTheDocument();
    expect(
      within(completedBubble).getByText("Rendered terminal item").tagName,
    ).toBe("STRONG");
  });

  it("renders reattached active-run markdown after streamEnd without a complete event", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                content: "",
                id: "message-assistant-stream",
                metrics: null,
                parts: [],
                reasoning: null,
                status: "streaming",
                toolCalls: [],
              },
            ],
            activeRun: {
              assistantMessageId: "message-assistant-stream",
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(
        true,
      ),
    );

    const markdown = "# Plan summary\n\n- **Rendered after reconnect**";
    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        delta: markdown,
        type: "textDelta",
      });
    });

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", { type: "streamEnd" });
    });

    const heading = await screen.findByRole("heading", {
      name: "Plan summary",
    });
    const completedBubble = heading.closest(".message-bubble") as HTMLElement;
    expect(
      completedBubble.querySelector(".markdown-content-assistant"),
    ).not.toBeNull();
    expect(within(completedBubble).getByRole("list")).toBeInTheDocument();
    expect(
      within(completedBubble).getByText("Rendered after reconnect").tagName,
    ).toBe("STRONG");
  });

  it("shows retrieved memories as soon as the chat stream starts", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "use memory",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await userEvent.click(await screen.findByText("Memories used"));
    expect(
      screen.getByText("Use memory before streaming."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Model: gpt-test")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps a memory-updated streaming placeholder through message reloads", async () => {
    let exposeReloadQuestion = false;
    let refreshedMessages = 0;
    const pendingQuestion = {
      chatId: "chat-1",
      id: "reload-running-chat-question",
      questions: [
        {
          allowFreeText: true,
          id: "reload-running-chat-question-item",
          options: [],
          question: "Keep going?",
        },
      ],
      toolCallId: "ask-question-call",
      workspaceId: "workspace-1",
    };
    const resolvedMemory = {
      chatId: null,
      fact: "Matched memory survives reload.",
      id: "stream-memory-resolved-1",
      kind: "project_fact",
      pinned: false,
      scope: "workspace",
      source: "direct",
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/chat/questions/pending") {
          return jsonResponse({
            questions: exposeReloadQuestion ? [pendingQuestion] : [],
          });
        }

        if (
          exposeReloadQuestion &&
          path === "/api/workspaces/workspace-1/chats/chat-1/messages"
        ) {
          refreshedMessages += 1;
          return jsonResponse({
            ...chatMessages,
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
            messages: [
              {
                ...chatMessages.messages[0],
                content: "use memory",
                id: "message-user-stream",
                parts: [{ text: "use memory", type: "text" }],
              },
            ],
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "use memory",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    await waitFor(() =>
      expect(document.querySelector(".message-waiting-spinner")).not.toBeNull(),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        memoriesUsed: [resolvedMemory],
        type: "memoryResolved",
      });
    });

    exposeReloadQuestion = true;
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });

    await waitFor(() => expect(refreshedMessages).toBeGreaterThan(0));
    expect(document.querySelector(".message-waiting-spinner")).not.toBeNull();
    expect(screen.getAllByText("Memories used").length).toBeGreaterThan(0);

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
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

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

  it("restores a durable stream error after closing and reopening the chat tab", async () => {
    const failureMessage = "Provider connection failed. Please retry.";
    const retryPrompt = "Retry the persisted failure.";
    const fetchMock = vi.mocked(fetch);

    renderApp();
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      retryPrompt,
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        message: failureMessage,
        type: "error",
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
      appTestState.activeChatStreamController?.close();
    });

    expect(await screen.findByText(failureMessage)).toBeInTheDocument();

    appTestState.chatMessagesResponsesByChatKey = {
      "workspace-1/chat-1": {
        ...chatMessages,
        messages: [
          ...chatMessages.messages,
          {
            content: retryPrompt,
            createdAt: "2026-07-20T08:00:00.000Z",
            extractedMemories: [],
            id: "message-user-stream",
            memoriesUsed: [],
            metrics: null,
            parts: [{ text: retryPrompt, type: "text" }],
            reasoning: null,
            role: "user",
            runConfig: {
              latencyMode: "standard",
              modelId: "gpt-test",
              providerId: "openai",
              selectedSkillIds: [],
              sessionMode: null,
              teamModeEnabled: false,
              thinkingLevel: "high",
            },
            toolCalls: [],
          },
          {
            content: failureMessage,
            createdAt: "2026-07-20T08:00:01.000Z",
            extractedMemories: [],
            id: "message-assistant-stream",
            memoriesUsed: [],
            metrics: null,
            parts: [{ text: failureMessage, type: "error" }],
            reasoning: null,
            role: "assistant",
            status: "error",
            toolCalls: [],
          },
        ],
      } as unknown as typeof chatMessages,
    };
    const messageRequestsBeforeReopen = fetchMock.mock.calls.filter(([url]) =>
      url.toString().includes("/chats/chat-1/messages"),
    ).length;

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabList).getByRole("button", {
        name: "Close chat tab Retry the persisted failure.",
      }),
    );
    await userEvent.click(await screen.findByText("Tool run"));

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.filter(([url]) =>
          url.toString().includes("/chats/chat-1/messages"),
        ).length,
      ).toBeGreaterThan(messageRequestsBeforeReopen),
    );
    expect(await screen.findByText(failureMessage)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Retry last run" }),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Retry last run" }),
    );
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );
    const retryRequest = fetchMock.mock.calls
      .filter(([url]) => url.toString().includes("/chat/stream"))
      .at(-1);
    expect(JSON.parse(String(retryRequest?.[1]?.body))).toMatchObject({
      message: retryPrompt,
      modelId: "gpt-test",
      providerId: "openai",
      thinkingLevel: "high",
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("appends stream errors after already rendered assistant text", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "debug",
    );
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

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "danger",
    );
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

    expect(
      await screen.findByText("Hook blocked run_command: denied"),
    ).toBeInTheDocument();
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

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
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
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
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

  it("keeps a close button available on a streaming chat tab without stopping the run", async () => {
    window.history.replaceState(
      null,
      "",
      "/?tab=workspace-1%2Fchat-1&tab=workspace-1%2Fchat-2&file=workspace-1%2FREADME.md&activeFile=workspace-1%2FREADME.md",
    );
    renderApp();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() =>
      expect(
        within(tabList).getByRole("tab", { name: /README\.md/ }),
      ).toHaveAttribute("aria-selected", "true"),
    );
    await userEvent.click(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    );
    expect(
      within(tabList).getByRole("button", {
        name: "Close chat tab Second chat",
      }),
    ).toBeInTheDocument();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const secondChatButton = within(workspaceList)
      .getByText("Second chat")
      .closest("button");
    if (!secondChatButton) {
      throw new Error("Expected Second chat history item button");
    }
    const statusDot = () =>
      secondChatButton.querySelector(".session-status-dot");

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    expect(
      await within(tabList).findByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-running"),
    );
    const closeButton = within(tabList).getByRole("button", {
      name: "Close chat tab Second chat",
    });
    expect(closeButton).toBeEnabled();
    expect(closeButton).toHaveClass("button--ghost", "button--icon-only");

    await userEvent.click(closeButton);

    await waitFor(() =>
      expect(
        within(tabList).queryByRole("tab", { name: /Second chat/ }),
      ).not.toBeInTheDocument(),
    );
    expect(statusDot()).toHaveClass("session-status-dot-running");
    expect(
      vi.mocked(fetch).mock.calls.some(([input]) => {
        const url = typeof input === "string" ? input : input.toString();
        return url.includes(
          "/api/workspaces/workspace-1/chat/runs/request-stream/cancel",
        );
      }),
    ).toBe(false);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("closes a running chat tab through the tab context menu batch actions", async () => {
    window.history.replaceState(
      null,
      "",
      "/?tab=workspace-1%2Fchat-1&tab=workspace-1%2Fchat-2&file=workspace-1%2FREADME.md&activeFile=workspace-1%2FREADME.md",
    );
    renderApp();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() =>
      expect(
        within(tabList).getByRole("tab", { name: /README\.md/ }),
      ).toHaveAttribute("aria-selected", "true"),
    );
    await userEvent.click(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    expect(
      await within(tabList).findByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const secondChatButton = within(workspaceList)
      .getByText("Second chat")
      .closest("button");
    if (!secondChatButton) {
      throw new Error("Expected Second chat history item button");
    }
    await waitFor(() =>
      expect(secondChatButton.querySelector(".session-status-dot")).toHaveClass(
        "session-status-dot-running",
      ),
    );

    const toolRunTabItem = within(tabList)
      .getByRole("tab", { name: /Tool run/ })
      .closest(".chat-tab-item");
    expect(toolRunTabItem).not.toBeNull();
    fireEvent.contextMenu(toolRunTabItem as HTMLElement);
    const menu = await screen.findByRole("menu", { name: "Tool run" });
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Close other tabs" }),
    );

    await waitFor(() =>
      expect(
        within(tabList).queryByRole("tab", { name: /Second chat/ }),
      ).not.toBeInTheDocument(),
    );
    expect(
      within(tabList).getByRole("tab", { name: /Tool run/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(secondChatButton.querySelector(".session-status-dot")).toHaveClass(
      "session-status-dot-running",
    );
    expect(
      vi.mocked(fetch).mock.calls.some(([input]) => {
        const url = typeof input === "string" ? input : input.toString();
        return url.includes(
          "/api/workspaces/workspace-1/chat/runs/request-stream/cancel",
        );
      }),
    ).toBe(false);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps tab and sidebar running icons while coordinator waits with queuedRun.running", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "wait for worker",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyButton = within(workspaceList)
      .getByText("Tool run")
      .closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }
    const statusDot = () => historyButton.querySelector(".session-status-dot");

    expect(
      await within(tabList).findByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-running"),
    );
    expect(
      screen.getByRole("button", { name: "Cancel run" }),
    ).toBeInTheDocument();

    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: null,
                queuedRun: {
                  assistantMessageId: "message-assistant-stream",
                  content: "wait for worker",
                  modelId: "gpt-test",
                  providerId: "openai",
                  skillIds: [],
                  status: "running",
                  thinkingLevel: null,
                  userMessageId: "message-user-stream",
                },
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Cancel run" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      within(tabList).getByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    expect(tabList.querySelector(".chat-tab-running-spinner")).not.toBeNull();
    expect(statusDot()).toHaveClass("session-status-dot-running");
    expect(
      screen.getByRole("button", { name: "Send message" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Send guidance" }),
    ).not.toBeInTheDocument();

    // Still durable-running while a later activeRun appears (resume handoff).
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: {
                  acceptingGuidance: true,
                  chatId: "chat-1",
                  lastSequence: 2,
                  runId: "request-stream-resumed",
                  workspaceId: "workspace-1",
                },
                queuedRun: {
                  assistantMessageId: "message-assistant-stream",
                  content: "wait for worker",
                  modelId: "gpt-test",
                  providerId: "openai",
                  skillIds: [],
                  status: "running",
                  thinkingLevel: null,
                  userMessageId: "message-user-stream",
                },
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh workspaces" }),
    );

    await waitFor(() =>
      expect(
        within(tabList).getByRole("status", { name: "Chat is running" }),
      ).toBeInTheDocument(),
    );
    expect(statusDot()).toHaveClass("session-status-dot-running");

    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: null,
                queuedRun: null,
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh workspaces" }),
    );

    await waitFor(() =>
      expect(
        within(tabList).queryByRole("status", { name: "Chat is running" }),
      ).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(statusDot()).not.toHaveClass("session-status-dot-running"),
    );
  });

  it("keeps the tab context menu open when the active stream scrolls messages", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const toolRunTabItem = within(tabList)
      .getByRole("tab", { name: /Tool run/ })
      .closest(".chat-tab-item");
    expect(toolRunTabItem).not.toBeNull();
    fireEvent.contextMenu(toolRunTabItem as HTMLElement);
    expect(
      await screen.findByRole("menu", { name: "Tool run" }),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
    });
    expect(await screen.findByText("Partial answer.")).toBeInTheDocument();

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }
    fireEvent.scroll(messageList);

    expect(screen.getByRole("menu", { name: "Tool run" })).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps the file tree context menu open when the active stream scrolls messages", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.activeChatStreamController).not.toBeNull(),
    );

    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    const contextPanel = document.querySelector(".context-panel");
    if (!(contextPanel instanceof HTMLElement)) {
      throw new Error("Expected context panel");
    }
    const fileRow = (
      await within(contextPanel).findByText("README.md")
    ).closest("div[role='treeitem']");
    expect(fileRow).not.toBeNull();
    fireEvent.contextMenu(fileRow as HTMLElement);
    expect(
      await screen.findByRole("menu", { name: "README.md" }),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
    });
    expect(await screen.findByText("Partial answer.")).toBeInTheDocument();

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }
    fireEvent.scroll(messageList);

    expect(screen.getByRole("menu", { name: "README.md" })).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("reattaches from refreshed workspace active run when reopening cached chat", async () => {
    const fetchMock = vi.mocked(fetch);
    let reattachActiveRun = false;
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return Promise.resolve(
          jsonResponse({
            ...chatMessages,
            activeRun: reattachActiveRun
              ? {
                  acceptingGuidance: true,
                  chatId: "chat-1",
                  lastSequence: 0,
                  runId: "request-stream",
                  workspaceId: "workspace-1",
                }
              : null,
          }),
        );
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByText("Please inspect README."),
    ).toBeInTheDocument();
    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();

    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats.map((chat) =>
          chat.id === "chat-1"
            ? {
                ...chat,
                activeRun: {
                  chatId: "chat-1",
                  lastSequence: 0,
                  runId: "request-stream",
                  workspaceId: "workspace-1",
                },
              }
            : chat,
        ),
      },
      secondaryWorkspace,
    ];
    reattachActiveRun = true;

    const initialWorkspaceRequests = fetchMock.mock.calls.filter(([url]) =>
      String(url).includes("/api/workspaces"),
    ).length;
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh workspaces" }),
    );
    await waitFor(() => {
      const workspaceRequests = fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces"),
      ).length;
      expect(workspaceRequests).toBeGreaterThan(initialWorkspaceRequests);
    });

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await userEvent.click(within(workspaceList).getByText("Tool run"));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
        ),
      ).toBe(true);
    });

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        delta: "Reattached from workspace list.",
        type: "textDelta",
      });
    });

    expect(
      await screen.findByText("Reattached from workspace list."),
    ).toBeInTheDocument();

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("restores a pending question when loading an active chat", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            ...chatMessages,
            activeRun: {
              acceptingGuidance: true,
              chatId: "chat-1",
              lastSequence: 853,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
            pendingQuestion: {
              chatId: "chat-1",
              id: "read-file-question-1",
              questions: [
                {
                  allowFreeText: false,
                  id: "read-file-question-1-item-1",
                  options: [
                    {
                      description: "Allow this read.",
                      label: "Allow",
                      value: "allow",
                    },
                  ],
                  question: "read_file wants to read outside the workspace.",
                },
              ],
              toolCallId: "read-file-call-1",
              workspaceId: "workspace-1",
            },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    const dialog = await screen.findByRole("dialog", {
      name: "Foco needs your answer",
    });
    expect(
      within(dialog).getByText(
        "read_file wants to read outside the workspace.",
      ),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("Allow")).toBeInTheDocument();

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("opens the pending question chat on startup and answers through the question endpoint", async () => {
    appTestState.pendingQuestionsResponse = [
      {
        chatId: "chat-2",
        id: "pending-startup-question",
        questions: [
          {
            allowFreeText: false,
            id: "pending-startup-question-item",
            options: [
              {
                description: null,
                label: "Proceed",
                value: "proceed",
              },
            ],
            question: "Should the background run continue?",
          },
        ],
        toolCallId: "ask-question-call",
        workspaceId: "workspace-1",
      },
    ];
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
          return jsonResponse({
            ...secondChatMessages,
            activeRun: null,
            pendingQuestion: appTestState.pendingQuestionsResponse[0],
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    renderApp();

    expect(
      await screen.findByRole("tab", { name: /Second chat/, hidden: true }),
    ).toHaveAttribute("aria-selected", "true");
    const dialog = await screen.findByRole("dialog", {
      name: "Foco needs your answer",
    });
    expect(
      within(dialog).getByText("Should the background run continue?"),
    ).toBeInTheDocument();

    await userEvent.click(within(dialog).getByText("Proceed"));
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Continue run" }),
    );

    await waitFor(() => {
      expect(appTestState.answeredQuestionIds).toEqual([
        "pending-startup-question",
      ]);
    });
  });

  it("reports a missing pending question chat without blocking the app", async () => {
    appTestState.pendingQuestionsResponse = [
      {
        chatId: "missing-chat",
        id: "missing-chat-question",
        questions: [
          {
            allowFreeText: true,
            id: "missing-chat-question-item",
            options: [],
            question: "This chat is gone.",
          },
        ],
        toolCallId: "ask-question-call",
        workspaceId: "workspace-1",
      },
    ];

    renderApp();

    expect(
      await screen.findByText(
        "Pending question chat is no longer available: workspace-1/missing-chat",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Default").length).toBeGreaterThan(0);
  });

  it("reconnects an idle active run stream from the last processed sequence", async () => {
    (
      globalThis as { __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: number }
    ).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__ = 20;
    let runStreamRequests = 0;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                id: "message-assistant-stream",
                status: "streaming",
              },
            ],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chat/runs/request-stream/stream"
        ) {
          runStreamRequests += 1;
          const encoder = new TextEncoder();
          const stream = new ReadableStream<Uint8Array>({
            start(controller) {
              appTestState.chatStreamControllers.set(
                "request-stream",
                controller,
              );
              if (runStreamRequests === 1) {
                controller.enqueue(
                  encoder.encode(
                    `id: 1\ndata: ${JSON.stringify({
                      assistantMessageId: "message-assistant-stream",
                      delta: "Still alive.",
                      type: "textDelta",
                    })}\n\n`,
                  ),
                );
              }
            },
          });
          return new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();
      await waitFor(() => {
        expect(
          fetchMock.mock.calls.some(
            ([url]) =>
              typeof url === "string" &&
              url ===
                "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
          ),
        ).toBe(true);
      });

      await waitFor(() => {
        const streamRequests = fetchMock.mock.calls.filter(
          ([url]) =>
            typeof url === "string" &&
            url.includes("/chat/runs/request-stream/stream"),
        );
        expect(streamRequests.length).toBeGreaterThan(1);
        expect(
          streamRequests.some(
            ([url]) =>
              typeof url === "string" &&
              url ===
                "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=1",
          ),
        ).toBe(true);
      });
    } finally {
      try {
        appTestState.chatStreamControllers.get("request-stream")?.close();
      } catch {
        // Stream may already be cancelled by the idle watchdog.
      }
      delete (
        globalThis as { __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: number }
      ).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__;
    }
  });

  it("silently clears a stale active run stream and reloads chat history", async () => {
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
    let messageRequests = 0;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          messageRequests += 1;
          return jsonResponse({
            ...chatMessages,
            activeRun:
              messageRequests === 1
                ? {
                    acceptingGuidance: false,
                    chatId: "chat-1",
                    lastSequence: 0,
                    runId: "stale-run",
                    workspaceId: "workspace-1",
                  }
                : null,
          });
        }

        if (path === "/api/workspaces/workspace-1/chat/runs/stale-run/stream") {
          return jsonResponse(
            { error: "active chat run was not found: stale-run" },
            { status: 400 },
          );
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyButton = (
      await within(workspaceList).findByText("Tool run")
    ).closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }
    const statusDot = () => historyButton.querySelector(".session-status-dot");

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).includes(
            "/api/workspaces/workspace-1/chat/runs/stale-run/stream",
          ),
        ),
      ).toBe(true),
    );
    await waitFor(() => expect(messageRequests).toBeGreaterThan(1));
    await waitFor(() =>
      expect(statusDot()).not.toHaveClass("session-status-dot-running"),
    );
    expect(
      screen.queryByText("active chat run was not found: stale-run"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Cancel run" }),
    ).not.toBeInTheDocument();
  });

  it("shows non-stale active run stream backend errors", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            ...chatMessages,
            activeRun: {
              acceptingGuidance: false,
              chatId: "chat-1",
              lastSequence: 0,
              runId: "broken-run",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chat/runs/broken-run/stream"
        ) {
          return jsonResponse({ error: "stream exploded" }, { status: 500 });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "stream exploded",
    );
  });

  it("keeps a pending ask_question dialog and draft through idle active run reconnect", async () => {
    (
      globalThis as { __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: number }
    ).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__ = 20;
    let runStreamRequests = 0;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                id: "message-assistant-stream",
                status: "streaming",
              },
            ],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chat/runs/request-stream/stream"
        ) {
          runStreamRequests += 1;
          const encoder = new TextEncoder();
          const stream = new ReadableStream<Uint8Array>({
            start(controller) {
              appTestState.chatStreamControllers.set(
                "request-stream",
                controller,
              );
              if (runStreamRequests === 1) {
                controller.enqueue(
                  encoder.encode(
                    `id: 1\ndata: ${JSON.stringify({
                      assistantMessageId: "message-assistant-stream",
                      request: {
                        chatId: "chat-1",
                        id: "idle-reconnect-question",
                        questions: [
                          {
                            allowFreeText: true,
                            id: "idle-reconnect-question-item",
                            options: [],
                            question: "What should Foco do next?",
                          },
                        ],
                        toolCallId: "ask-question-call",
                        workspaceId: "workspace-1",
                      },
                      type: "questionRequest",
                    })}\n\n`,
                  ),
                );
              }
            },
          });
          return new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();
      const dialog = await screen.findByRole("dialog", {
        name: "Foco needs your answer",
      });
      const answerDraft = within(dialog).getByLabelText("Custom answer");
      fireEvent.change(answerDraft, { target: { value: "Keep this draft" } });

      await waitFor(() => {
        const streamRequests = fetchMock.mock.calls.filter(
          ([url]) =>
            typeof url === "string" &&
            url.includes("/chat/runs/request-stream/stream"),
        );
        expect(streamRequests.length).toBeGreaterThan(1);
      });

      await act(async () => {
        appTestState.chatStreamControllers.get("request-stream")?.close();
      });

      const reconnectedDialog = screen.getByRole("dialog", {
        name: "Foco needs your answer",
      });
      expect(reconnectedDialog).toBeInTheDocument();
      expect(
        within(reconnectedDialog).getByLabelText("Custom answer"),
      ).toHaveValue("Keep this draft");
    } finally {
      try {
        appTestState.chatStreamControllers.get("request-stream")?.close();
      } catch {
        // Stream may already be cancelled by the idle watchdog.
      }
      delete (
        globalThis as { __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: number }
      ).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__;
    }
  });

  it("does not duplicate active run subscriptions on recovery events while the stream is alive", async () => {
    let runStreamRequests = 0;
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                id: "message-assistant-stream",
                status: "streaming",
              },
            ],
            activeRun: {
              acceptingGuidance: true,
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chat/runs/request-stream/stream"
        ) {
          runStreamRequests += 1;
          return chatStreamResponse("chat-1");
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => expect(runStreamRequests).toBe(1));

    await act(async () => {
      Object.defineProperty(document, "visibilityState", {
        configurable: true,
        value: "visible",
      });
      fireEvent(document, new Event("visibilitychange"));
      window.dispatchEvent(new Event("online"));
      await Promise.resolve();
    });

    expect(runStreamRequests).toBe(1);

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("holds legacy reattach deltas until start resolves the durable assistant id", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                content: "",
                id: "message-assistant-stream",
                parts: [],
                status: "streaming",
              },
            ],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "legacy-reattach-run",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path ===
          "/api/workspaces/workspace-1/chat/runs/legacy-reattach-run/stream"
        ) {
          const encoder = new TextEncoder();
          const stream = new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(
                encoder.encode(
                  `data: ${JSON.stringify({
                    delta: "Buffered legacy text.",
                    type: "textDelta",
                  })}\n\n`,
                ),
              );
              controller.enqueue(
                encoder.encode(
                  `data: ${JSON.stringify({
                    assistantMessageId: "message-assistant-stream",
                    chatId: "chat-1",
                    memoriesUsed: [],
                    type: "start",
                    userMessageId: "message-user-stream",
                  })}\n\n`,
                ),
              );
              controller.close();
            },
          });
          return new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    const legacyText = await screen.findByText("Buffered legacy text.");
    expect(legacyText.closest(".message-row")).not.toBeNull();
    expect(
      new Set(
        screen
          .getAllByText("Buffered legacy text.")
          .map((node) => node.closest(".message-row")),
      ).size,
    ).toBe(1);
  });

  it("keeps one durable assistant row when refresh restores an active streaming message", async () => {
    const durableAssistantId = "message-assistant-durable";
    const encoder = new TextEncoder();
    const streamControllerRef: {
      current: ReadableStreamDefaultController<Uint8Array> | null;
    } = { current: null };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                content: "",
                id: durableAssistantId,
                parts: [],
                status: "streaming",
              },
            ],
            activeRun: {
              assistantMessageId: durableAssistantId,
              chatId: "chat-1",
              lastSequence: 0,
              runId: "refresh-durable-run",
              workspaceId: "workspace-1",
            },
          });
        }

        if (
          path ===
          "/api/workspaces/workspace-1/chat/runs/refresh-durable-run/stream"
        ) {
          const stream = new ReadableStream<Uint8Array>({
            start(nextController) {
              streamControllerRef.current = nextController;
            },
          });
          return new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();

      await waitFor(() => expect(streamControllerRef.current).not.toBeNull());
      await act(async () => {
        streamControllerRef.current?.enqueue(
          encoder.encode(
            `data: ${JSON.stringify({
              assistantMessageId: durableAssistantId,
              chatId: "chat-1",
              memoriesUsed: [],
              type: "start",
              userMessageId: "message-user-stream",
            })}\n\n`,
          ),
        );
        streamControllerRef.current?.enqueue(
          encoder.encode(
            `data: ${JSON.stringify({
              assistantMessageId: durableAssistantId,
              delta: "Refreshed durable delta.",
              type: "textDelta",
            })}\n\n`,
          ),
        );
      });

      const delta = await screen.findByText("Refreshed durable delta.");
      const assistantRow = delta.closest(".message-row");
      expect(assistantRow).not.toBeNull();
      expect(document.querySelectorAll(".message-row")).toHaveLength(2);
      expect(
        screen
          .getAllByText("Refreshed durable delta.")
          .map((node) => node.closest(".message-row")),
      ).toEqual([assistantRow]);
    } finally {
      try {
        await act(async () => {
          streamControllerRef.current?.close();
        });
      } catch {
        // The stream may already be cancelled when the test cleanup runs.
      }
    }
  });

  it("ignores late events from a displaced active-run session", async () => {
    const encoder = new TextEncoder();
    let serveReplacementRun = false;
    let sharedRunStreamRequests = 0;
    let oldController: ReadableStreamDefaultController<Uint8Array> | null =
      null;
    let replacementController: ReadableStreamDefaultController<Uint8Array> | null =
      null;
    const emit = (
      controller: ReadableStreamDefaultController<Uint8Array>,
      event: Record<string, unknown>,
    ) => {
      controller.enqueue(encoder.encode(`data: ${JSON.stringify(event)}\n\n`));
    };
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          const activeRun = serveReplacementRun
            ? {
                assistantMessageId: "replacement-assistant",
                chatId: "chat-1",
                lastSequence: 0,
                runId: "shared-run",
                workspaceId: "workspace-1",
              }
            : {
                assistantMessageId: "old-assistant",
                chatId: "chat-1",
                lastSequence: 0,
                runId: "shared-run",
                workspaceId: "workspace-1",
              };
          const assistant = {
            ...chatMessages.messages[1],
            content: "",
            id: activeRun.assistantMessageId,
            parts: [],
            status: "streaming",
          };
          return jsonResponse({
            activeRun,
            messages: [chatMessages.messages[0], assistant],
          });
        }

        if (
          path === "/api/workspaces/workspace-1/chat/runs/shared-run/stream"
        ) {
          sharedRunStreamRequests += 1;
          const stream = new ReadableStream<Uint8Array>({
            start(controller) {
              if (sharedRunStreamRequests === 1) {
                oldController = controller;
              } else {
                replacementController = controller;
              }
            },
          });
          return new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    try {
      renderApp();
      await waitFor(() => expect(oldController).not.toBeNull());

      await act(async () => {
        emit(oldController as ReadableStreamDefaultController<Uint8Array>, {
          assistantMessageId: "old-assistant",
          chatId: "chat-1",
          memoriesUsed: [],
          type: "start",
          userMessageId: "message-user-stream",
        });
      });

      serveReplacementRun = true;
      await act(async () => {
        emit(oldController as ReadableStreamDefaultController<Uint8Array>, {
          assistantMessageId: "old-assistant-alias",
          chatId: "chat-1",
          memoriesUsed: [],
          type: "start",
          userMessageId: "message-user-stream",
        });
      });

      await waitFor(() => expect(replacementController).not.toBeNull());
      await act(async () => {
        emit(
          replacementController as ReadableStreamDefaultController<Uint8Array>,
          {
            assistantMessageId: "replacement-assistant",
            chatId: "chat-1",
            memoriesUsed: [],
            type: "start",
            userMessageId: "message-user-stream",
          },
        );
        emit(
          replacementController as ReadableStreamDefaultController<Uint8Array>,
          {
            assistantMessageId: "replacement-assistant",
            delta: "Replacement owner text.",
            type: "textDelta",
          },
        );
      });
      expect(
        await screen.findByText("Replacement owner text."),
      ).toBeInTheDocument();

      await act(async () => {
        const controller =
          oldController as ReadableStreamDefaultController<Uint8Array>;
        emit(controller, {
          assistantMessageId: "old-assistant",
          delta: "Late old text.",
          type: "textDelta",
        });
        emit(controller, {
          assistantMessageId: "old-assistant",
          delta: "Late old reasoning.",
          type: "reasoningDelta",
        });
        emit(controller, {
          assistantMessageId: "old-assistant",
          toolCall: {
            id: "late-old-tool",
            input: {},
            isError: false,
            name: "late_old_tool",
            output: null,
            status: "running",
          },
          type: "toolCall",
        });
        emit(controller, {
          assistantMessageId: "old-assistant",
          metrics: {
            modelId: "gpt-test",
            providerId: "openai",
            totalLatencyMs: 1,
          },
          reasoning: null,
          stopReason: "completed",
          text: "Late old completion.",
          type: "complete",
          usage: {
            cacheReadTokens: 0,
            cacheWriteTokens: 0,
            inputTokens: 1,
            outputTokens: 1,
          },
        });
        emit(controller, { type: "streamEnd" });
      });

      expect(screen.getByText("Replacement owner text.")).toBeInTheDocument();
      expect(screen.queryByText("Late old text.")).not.toBeInTheDocument();
      expect(screen.queryByText("Late old reasoning.")).not.toBeInTheDocument();
      expect(screen.queryByText("late_old_tool")).not.toBeInTheDocument();
      expect(
        screen.queryByText("Late old completion."),
      ).not.toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "Cancel run" }),
      ).toBeInTheDocument();
      expect(document.querySelectorAll(".message-row")).toHaveLength(2);
    } finally {
      await act(async () => {
        for (const controller of [oldController, replacementController]) {
          try {
            controller?.close();
          } catch {
            // A displaced stream may already have been cancelled by the client.
          }
        }
      });
    }
  });

  it("reattaches to an active run when loading chat messages", async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                content: "Persisted fallback text.",
                id: "message-assistant-stream",
                metrics: null,
                parts: [
                  { text: "Persisted fallback reasoning.", type: "reasoning" },
                  { text: "Persisted fallback text.", type: "text" },
                ],
                reasoning: "Persisted fallback reasoning.",
                toolCalls: [],
              },
            ],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
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
    expect(
      screen.queryByText("Persisted fallback text."),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("Persisted fallback reasoning."),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();

    const usageCallCountBeforeUsage = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    ).length;
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
      await screen.findByRole("status", { name: "Context usage 55%" }),
    ).toHaveTextContent("55%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCalls).toHaveLength(usageCallCountBeforeUsage);

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
    expect(usageCallsAfterComplete).toHaveLength(
      usageCallCountBeforeComplete + 1,
    );
    expect(
      screen.getByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("restores composer model after active-run reattach when settings load late", async () => {
    const settingsGate = deferred<Response>();
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/settings") {
          return settingsGate.promise;
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              chatMessages.messages[0],
              {
                ...chatMessages.messages[1],
                content: "Persisted fallback text.",
                id: "message-assistant-stream",
                metrics: null,
                parts: [
                  { text: "Persisted fallback reasoning.", type: "reasoning" },
                  { text: "Persisted fallback text.", type: "text" },
                ],
                reasoning: "Persisted fallback reasoning.",
                toolCalls: [],
              },
            ],
            activeRun: {
              acceptingGuidance: true,
              assistantMessageId: "message-assistant-stream",
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
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

    // Settings still pending: model catalog is empty, but selection must not
    // permanently clear to the empty-label state.
    expect(
      isDisabledControl(screen.getByRole("button", { name: /Model:/ })),
    ).toBe(true);

    await act(async () => {
      settingsGate.resolve(jsonResponse(settings));
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Model:/ })).toHaveTextContent(
        "GPT Test",
      );
      expect(
        isDisabledControl(screen.getByRole("button", { name: /Model:/ })),
      ).toBe(false);
    });

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    expect(
      screen.getByRole("option", { name: "GPT Test" }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "gpt-test",
          outputTokens: 1,
          providerId: "openai",
          totalLatencyMs: 20,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Still running.",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() => {
      expect(
        screen.queryByRole("status", { name: "Chat is running" }),
      ).not.toBeInTheDocument();
    });

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue after reattach",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(String(queueCall![1]?.body))).toMatchObject({
        message: "continue after reattach",
        modelId: "gpt-test",
        providerId: "openai",
      });
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps composer model when messages resolve after settings during active-run reattach", async () => {
    // Race: loadChatMessages starts while settings are still loading (empty
    // catalog in the request closure). Settings then finish and reconcile the
    // model; messages resolve later. applyComposerModelForPlanMode must use the
    // latest catalog, not the stale empty one from request start.
    const settingsGate = deferred<Response>();
    const messagesGate = deferred<Response>();
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/settings") {
          return settingsGate.promise;
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return messagesGate.promise;
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([url]) => {
          const value = typeof url === "string" ? url : url.toString();
          return value.includes(
            "/api/workspaces/workspace-1/chats/chat-1/messages",
          );
        }),
      ).toBe(true);
    });

    await act(async () => {
      settingsGate.resolve(jsonResponse(settings));
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Model:/ })).toHaveTextContent(
        "GPT Test",
      );
      expect(
        isDisabledControl(screen.getByRole("button", { name: /Model:/ })),
      ).toBe(false);
    });

    await act(async () => {
      messagesGate.resolve(
        jsonResponse({
          messages: [
            chatMessages.messages[0],
            {
              ...chatMessages.messages[1],
              content: "Persisted fallback text.",
              id: "message-assistant-stream",
              metrics: null,
              parts: [
                { text: "Persisted fallback reasoning.", type: "reasoning" },
                { text: "Persisted fallback text.", type: "text" },
              ],
              reasoning: "Persisted fallback reasoning.",
              toolCalls: [],
            },
          ],
          activeRun: {
            acceptingGuidance: true,
            assistantMessageId: "message-assistant-stream",
            chatId: "chat-1",
            lastSequence: 0,
            runId: "request-stream",
            workspaceId: "workspace-1",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
        ),
      ).toBe(true);
    });

    // Messages resolved after settings: model must stay restored, not cleared
    // by a stale empty-catalog applyComposerModelForPlanMode closure.
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Model:/ })).toHaveTextContent(
        "GPT Test",
      );
      expect(
        isDisabledControl(screen.getByRole("button", { name: /Model:/ })),
      ).toBe(false);
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Still running.",
        type: "textDelta",
      });
    });
    expect(await screen.findByText("Still running.")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    expect(
      screen.getByRole("option", { name: "GPT Test" }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "gpt-test",
          outputTokens: 1,
          providerId: "openai",
          totalLatencyMs: 20,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Still running.",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() => {
      expect(
        screen.queryByRole("status", { name: "Chat is running" }),
      ).not.toBeInTheDocument();
    });

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "continue after late messages",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(String(queueCall![1]?.body))).toMatchObject({
        message: "continue after late messages",
        modelId: "gpt-test",
        providerId: "openai",
      });
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("restores plan mode model after active-run reattach when settings load late", async () => {
    const baseModel = settings.configuredModels[0]!;
    const settingsWithPlanModel = {
      ...settings,
      configuredModels: [
        baseModel,
        {
          ...baseModel,
          activeProviderId: "anthropic",
          displayName: "GPT Alt",
          id: "gpt-alt",
          providerIds: ["anthropic"],
          thinkingLevel: null,
        },
      ],
      plan: {
        ...settings.plan,
        modeModelId: "gpt-alt",
      },
    };
    const settingsGate = deferred<Response>();
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/settings") {
          return settingsGate.promise;
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({
            messages: [
              {
                ...chatMessages.messages[0],
                content: "Plan this feature.",
                parts: [{ text: "Plan this feature.", type: "text" }],
                sessionMode: "plan",
              },
              {
                ...chatMessages.messages[1],
                content: "Planning…",
                id: "message-assistant-stream",
                metrics: null,
                parts: [{ text: "Planning…", type: "text" }],
                reasoning: null,
                toolCalls: [],
              },
            ],
            activeRun: {
              acceptingGuidance: true,
              assistantMessageId: "message-assistant-stream",
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          });
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=0",
        ),
      ).toBe(true);
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });

    await act(async () => {
      settingsGate.resolve(jsonResponse(settingsWithPlanModel));
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Model:/ })).toHaveTextContent(
        "GPT Alt",
      );
      expect(
        isDisabledControl(screen.getByRole("button", { name: /Model:/ })),
      ).toBe(false);
    });

    await userEvent.click(screen.getByRole("button", { name: /Model:/ }));
    expect(screen.getByRole("option", { name: "GPT Alt" })).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "GPT Test" }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "gpt-alt",
          outputTokens: 1,
          providerId: "anthropic",
          totalLatencyMs: 20,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Planning…",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() => {
      expect(
        screen.queryByRole("status", { name: "Chat is running" }),
      ).not.toBeInTheDocument();
    });

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "plan follow-up",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(String(queueCall![1]?.body))).toMatchObject({
        message: "plan follow-up",
        modelId: "gpt-alt",
        providerId: "anthropic",
        sessionMode: "plan",
      });
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });
});
