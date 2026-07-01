import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  agentDefinitions as agentDefinitionFixtures,
  agentTeamSnapshot,
  appTestState,
  defaultComposerPlaceholder,
  defaultReviewSystemPrompt,
  enqueueChatStreamEvent,
  jsonResponse,
  mockFetch,
  renderApp,
  resetAppTestEnvironment,
  settings,
} from "./test-utils/app-test-harness";

function stubDefaultAgentComposerDefaults() {
  const baseModel = settings.configuredModels[0]!;
  const settingsWithAltModel = {
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
  };
  const definitionsWithDefaultAgent = {
    agentDefinitions: agentDefinitionFixtures.agentDefinitions.map((definition) =>
      definition.id === "agent-definition-default"
        ? {
          ...definition,
          modelId: "gpt-alt",
          modelOptions: { maxOutputTokens: null, thinkingLevel: "high" },
          providerId: "anthropic",
        }
        : definition,
    ),
    defaultRolePrompts: {
      ...agentDefinitionFixtures.defaultRolePrompts,
      "agent-definition-default": "Default built-in prompt.",
    },
  };

  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/settings") {
        return jsonResponse(settingsWithAltModel);
      }
      if (path === "/api/agent-definitions") {
        return jsonResponse(definitionsWithDefaultAgent);
      }
      return mockFetch(input, init);
    }),
  );
}

function stubImageAgentSettings() {
  const textModel = settings.configuredModels[0]!;
  const imageModel = {
    ...textModel,
    canEnable: true,
    contextWindow: null,
    displayName: "GPT Image 2",
    id: "gpt-image-2",
    maxOutputTokens: null,
    outputModalities: ["image"],
    providerIds: ["openai"],
    supportsThinking: false,
    systemPromptName: "Default",
  };
  const altImageModel = {
    ...imageModel,
    displayName: "GPT Image 3",
    id: "gpt-image-3",
  };
  const imageAgentDefinition = {
    ...agentDefinitionFixtures.agentDefinitions[0],
    allowedExecutionWorkspaceModes: ["shared"],
    allowedTools: ["image_gen"],
    description: "Built-in image generation agent.",
    id: "agent-definition-image-gen",
    maxInstances: 1,
    modelId: textModel.id,
    modelOptions: { maxOutputTokens: null, thinkingLevel: null },
    name: "Image generation agent",
    permissions: {
      allowedAgentDefinitionIds: [],
      canCreateInstances: false,
      canDelegate: false,
    },
    providerId: textModel.activeProviderId!,
    revision: 1,
    systemPrompt:
      "<agent_definition_prompt>\n<identity>You are Foco's image generation agent.</identity>\n<instructions>Turn the user's request into a precise image prompt, call image_gen, and return the generated file paths with concise notes. Do not modify source files unless explicitly asked.</instructions>\n<tool_defaults>Use image_gen with model &quot;gpt-image-2&quot; unless the user explicitly asks for another configured image model.</tool_defaults>\n</agent_definition_prompt>",
  };
  const settingsWithImageModels = {
    ...settings,
    configuredModels: [textModel, imageModel, altImageModel],
  };
  const definitionsWithImageAgent = {
    agentDefinitions: [...agentDefinitionFixtures.agentDefinitions, imageAgentDefinition],
    defaultRolePrompts: {
      "agent-definition-image-gen": imageAgentDefinition.systemPrompt,
    },
  };

  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/settings") {
        return jsonResponse(settingsWithImageModels);
      }
      if (path === "/api/agent-definitions") {
        return jsonResponse(definitionsWithImageAgent);
      }
      if (path === "/api/agent-definitions/update") {
        const body = JSON.parse(String(init?.body ?? "{}")) as {
          definition?: typeof imageAgentDefinition;
          id?: string;
        };
        return jsonResponse({
          agentDefinitions: definitionsWithImageAgent.agentDefinitions.map((definition) =>
            definition.id === body.id && body.definition
              ? { ...body.definition, id: body.id, revision: 2 }
              : definition,
          ),
          defaultRolePrompts: definitionsWithImageAgent.defaultRolePrompts,
        });
      }
      return mockFetch(input, init);
    }),
  );
}

describe("app agents verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("shows Agent definitions in settings", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));

    expect(await screen.findByText("Agent definitions")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Agent settings" })).toBeInTheDocument();
    expect(screen.getAllByText("Coordinator").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Worker").length).toBeGreaterThan(0);
    expect(screen.getByText("Coordinates the Agent team.")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Default Team mode for new chats" }),
    ).toBeChecked();
    expect(screen.getByRole("button", { name: "Edit agent Coordinator" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete agent Coordinator" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Edit agent Coordinator" }));
    const editDialog = screen.getByRole("dialog", { name: "Edit agent" });
    const promptContent = within(editDialog).getByLabelText("Agent role prompt");
    expect(promptContent).toHaveValue("Coordinate the team.");
    await userEvent.clear(promptContent);
    await userEvent.type(promptContent, "Custom coordinator prompt.");
    expect(promptContent).toHaveValue("Custom coordinator prompt.");
    await userEvent.click(within(editDialog).getByRole("button", { name: "Cancel" }));

    await userEvent.click(screen.getByRole("button", { name: "Add agent definition" }));
    const dialog = screen.getByRole("dialog", { name: "Create agent" });
    expect(within(dialog).queryByLabelText("System prompt")).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText("Agent role prompt")).toHaveValue("");
    await userEvent.click(within(dialog).getByText("Allowed tools"));
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "read_file" }));
    expect(within(dialog).getByText("1 selected")).toBeInTheDocument();
  });

  it("hides built-in agent deletion and restores its default role prompt", async () => {
    stubDefaultAgentComposerDefaults();
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));

    expect(
      await screen.findByRole("button", { name: "Edit agent Default agent" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete agent Default agent" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete agent Review" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete agent Coordinator" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Edit agent Default agent" }));
    const dialog = screen.getByRole("dialog", { name: "Edit agent" });
    const promptContent = within(dialog).getByLabelText("Agent role prompt");
    await userEvent.clear(promptContent);
    await userEvent.type(promptContent, "Custom default agent role.");
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Restore default Agent role prompt",
      }),
    );

    expect(promptContent).toHaveValue("Default built-in prompt.");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await userEvent.click(screen.getByRole("button", { name: "Edit agent Review" }));
    const reviewDialog = screen.getByRole("dialog", { name: "Edit agent" });
    const reviewPromptContent = within(reviewDialog).getByLabelText("Agent role prompt");
    await userEvent.clear(reviewPromptContent);
    await userEvent.type(reviewPromptContent, "Custom review agent role.");
    await userEvent.click(
      within(reviewDialog).getByRole("button", {
        name: "Restore default Agent role prompt",
      }),
    );

    expect(reviewPromptContent).toHaveValue(defaultReviewSystemPrompt);
  });

  it("edits the image generation agent without embedding an image model in the role prompt", async () => {
    stubImageAgentSettings();
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));
    const imageAgentCard = (await screen.findByText("Image generation agent")).closest("article");
    expect(imageAgentCard).not.toBeNull();
    expect(within(imageAgentCard!).getByText("GPT Test", { exact: false })).toBeInTheDocument();

    await userEvent.click(
      await screen.findByRole("button", { name: "Edit agent Image generation agent" }),
    );
    const dialog = screen.getByRole("dialog", { name: "Edit agent" });
    const modelSelect = within(dialog).getByLabelText("Model");
    expect(within(dialog).queryByRole("option", { name: "GPT Image 2" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("option", { name: "GPT Image 3" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("option", { name: "GPT Test" })).toBeInTheDocument();
    expect(modelSelect).toHaveValue("gpt-test");
    expect(
      within(dialog).getByText(
        "Uses the current chat workspace directly. Simpler, but file changes land in the shared workspace.",
      ),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(
        "Creates a Foco-managed Git worktree for the instance. File changes stay isolated until you explicitly merge or delete them.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete agent Image generation agent" }),
    ).not.toBeInTheDocument();

    const promptContent = within(dialog).getByLabelText("Agent role prompt");
    expect((promptContent as HTMLTextAreaElement).value).toContain(
      "<tool_defaults>",
    );
    expect((promptContent as HTMLTextAreaElement).value).toContain("gpt-image-2");
    await userEvent.clear(promptContent);
    await userEvent.type(promptContent, "Custom image role prompt.");
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Restore default Agent role prompt",
      }),
    );
    expect((promptContent as HTMLTextAreaElement).value).toContain(
      "<tool_defaults>",
    );
    expect((promptContent as HTMLTextAreaElement).value).toContain("gpt-image-2");

    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/agent-definitions/update" && init?.method === "POST",
      );
      expect(saveCall).toBeDefined();
      const body = JSON.parse(saveCall![1]?.body as string) as {
        definition: { modelId: string; providerId: string; systemPrompt: string };
        id: string;
      };
      expect(body.id).toBe("agent-definition-image-gen");
      expect(body.definition.modelId).toBe("gpt-test");
      expect(body.definition.providerId).toBe("openai");
      expect(body.definition.systemPrompt).toContain("<tool_defaults>");
      expect(body.definition.systemPrompt).toContain("gpt-image-2");
    });
    await waitFor(() => {
      const updatedImageAgentCard = screen.getByText("Image generation agent").closest("article");
      expect(updatedImageAgentCard).not.toBeNull();
      expect(
        within(updatedImageAgentCard!).getByText("GPT Test", { exact: false }),
      ).toBeInTheDocument();
    });
  });

  it("saves the default Team mode setting from the Agents panel", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));
    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Default Team mode for new chats",
      }),
    );

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/settings/general",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(saveCall![1]?.body as string)).toMatchObject({
        defaultTeamModeEnabled: false,
      });
    });
  });

  it("localizes the Agents settings surface", async () => {
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

    await userEvent.click((await screen.findAllByRole("button", { name: "设置" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "设置" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "智能体" }));

    expect(await screen.findByRole("heading", { name: "智能体设置" })).toBeInTheDocument();
    expect(screen.getByText("智能体定义、模型、工具与权限")).toBeInTheDocument();
    expect(screen.queryByText("技能设置")).not.toBeInTheDocument();
  });

  it("opens the Agents panel and shows current chat Agent instances", async () => {
    const runningSnapshot = {
      ...agentTeamSnapshot,
      instances: agentTeamSnapshot.instances.map((instance) =>
        instance.id === "agent-instance-worker"
          ? { ...instance, status: "running" }
          : instance,
      ),
    };
    vi.mocked(fetch).mockImplementation(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(runningSnapshot);
        }
        return mockFetch(input, init);
      },
    );
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) => url === "/api/workspaces/workspace-1/chats/chat-1/agent-team",
        ),
      ).toBe(true);
    });
    expect(await screen.findByText("Current chat agent instances")).toBeInTheDocument();
    expect(screen.getByText("agent-instance-coordinator")).toBeInTheDocument();
    expect(screen.getByText("agent-instance-worker")).toBeInTheDocument();
    expect(screen.getByText("foco/agent-instance-worker")).toBeInTheDocument();
    expect(screen.getByLabelText("Agent status running").firstElementChild).toHaveClass(
      "agent-running-status-spinner",
    );
    expect(screen.queryByRole("button", { name: "Enable" })).not.toBeInTheDocument();
    expect(screen.queryByText("Observability")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Open agent Worker" }));

    expect(await screen.findByRole("tab", { name: /Worker/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.getByText("Found the issue in the workspace notes.")).toBeInTheDocument();
    expect(screen.getByText("Inspection complete.")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Open agent Coordinator" }));

    expect(await screen.findByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("renders worker LLM run events while the Agent task is still running", async () => {
    const firstSnapshot = {
      ...agentTeamSnapshot,
      runEvents: [
        {
          createdAt: "2026-06-05T10:00:02Z",
          eventType: "reasoning_delta",
          payload: {
            assistantMessageId: "agent-task-1-assistant",
            delta: "Checking workspace state.",
            type: "reasoningDelta",
          },
          runId: "agent-task-1",
          sequence: 0,
        },
        {
          createdAt: "2026-06-05T10:00:03Z",
          eventType: "tool_call",
          payload: {
            assistant_message_id: "agent-task-1-assistant",
            tool_call: {
              id: "tool-read-file",
              input: { path: "notes.md" },
              is_error: false,
              name: "read_file",
              output: null,
              status: "running",
            },
            type: "toolCall",
          },
          runId: "agent-task-1",
          sequence: 1,
        },
      ],
      tasks: agentTeamSnapshot.tasks.map((task) =>
        task.id === "agent-task-1"
          ? {
              ...task,
              completedAt: null,
              result: null,
              status: "running",
              updatedAt: "2026-06-05T10:00:03Z",
            }
          : task,
      ),
    };
    const secondSnapshot = {
      ...firstSnapshot,
      runEvents: [
        ...firstSnapshot.runEvents,
        {
          createdAt: "2026-06-05T10:00:04Z",
          eventType: "text_delta",
          payload: {
            assistantMessageId: "agent-task-1-assistant",
            delta: "Still inspecting.",
            type: "textDelta",
          },
          runId: "agent-task-1",
          sequence: 2,
        },
      ],
    };
    let snapshot = firstSnapshot;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(snapshot);
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "trigger refresh",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));

    expect(await screen.findByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.queryByText("Inspection complete.")).not.toBeInTheDocument();

    snapshot = secondSnapshot;

    await waitFor(
      () => expect(screen.getByText("Still inspecting.")).toBeInTheDocument(),
      { timeout: 2500 },
    );
  });

  it("reveals the Agents panel and refreshes when an Agent instance is created", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "create a worker",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    const snapshotCallsBefore = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/workspaces/workspace-1/chats/chat-1/agent-team",
    ).length;

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        instanceId: "agent-instance-worker",
        reason: "instance_created",
        revealPanel: true,
        teamId: "agent-team-1",
        type: "agentTeamRefresh",
        workspaceId: "workspace-1",
      });
    });

    await waitFor(() => {
      const snapshotCallsAfter = fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/chats/chat-1/agent-team",
      ).length;
      expect(snapshotCallsAfter).toBeGreaterThan(snapshotCallsBefore);
    });
    expect(await screen.findByText("Current chat agent instances")).toBeInTheDocument();
    expect(screen.getAllByLabelText("Agent status active").length).toBeGreaterThan(0);
  });

  it("refreshes the active chat Agent snapshot when a chat stream starts and ends", async () => {
    const fetchMock = vi.mocked(fetch);
    const snapshotCallCount = () =>
      fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/chats/chat-1/agent-team",
      ).length;
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await waitFor(() => expect(snapshotCallCount()).toBeGreaterThan(0));
    const callsBeforeStart = snapshotCallCount();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "refresh agent state",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await waitFor(() => expect(snapshotCallCount()).toBeGreaterThan(callsBeforeStart));
    const callsAfterStart = snapshotCallCount();

    await act(async () => {
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    await waitFor(() => expect(snapshotCallCount()).toBeGreaterThan(callsAfterStart));
  });

  it("queues the first message with Team tools enabled by default from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Plan mode" });
    expect(teamToggle).toHaveAttribute("aria-pressed", "false");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "handle this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const [, init] = queueCall!;
      expect(JSON.parse(init?.body as string)).toMatchObject({
        message: "handle this",
        teamModeEnabled: true,
      });
    });
  });

  it("queues a Plan mode first message from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "coordinate this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const [, init] = queueCall!;
      expect(JSON.parse(init?.body as string)).toMatchObject({
        message: "coordinate this",
        sessionMode: "plan",
        teamModeEnabled: false,
      });
    });
  });

  it("restores Plan mode per chat tab", async () => {
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await userEvent.click(await within(workspaceList).findByText("Tool run"));
    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");

    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.click(within(workspaceList).getByText("Second chat"));
    expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );

    await userEvent.click(within(workspaceList).getByText("Tool run"));
    expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("uses the configured Team mode default for a new composer", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings"
          ? jsonResponse({
            ...settings,
            general: {
              ...settings.general,
              defaultTeamModeEnabled: false,
            },
          })
          : mockFetch(input, init);
      }),
    );
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findByRole("button", { name: "Plan mode" });

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "use the default",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(queueCall![1]?.body as string)).toMatchObject({
        message: "use the default",
        teamModeEnabled: false,
      });
    });
  });

  it("uses the default agent model provider and thinking level for a new composer", async () => {
    stubDefaultAgentComposerDefaults();
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent(
        "Anthropic / GPT Alt",
      );
    });
    expect(screen.getByLabelText("Thinking")).toHaveTextContent("High");

    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "OpenAI: GPT Test" }));
    await userEvent.click(screen.getByLabelText("Thinking"));
    await userEvent.click(screen.getByRole("button", { name: "Thinking: Low" }));
    expect(screen.getByLabelText("Model")).toHaveTextContent("OpenAI / GPT Test");
    expect(screen.getByLabelText("Thinking")).toHaveTextContent("Low");

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    expect(screen.getByLabelText("Model")).toHaveTextContent(
      "Anthropic / GPT Alt",
    );
    expect(screen.getByLabelText("Thinking")).toHaveTextContent("High");

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "use default agent defaults",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(queueCall![1]?.body as string)).toMatchObject({
        message: "use default agent defaults",
        modelId: "gpt-alt",
        providerId: "anthropic",
        thinkingLevel: "high",
      });
    });
  });

  it("lets composer model provider and thinking selections override the default agent", async () => {
    stubDefaultAgentComposerDefaults();
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent(
        "Anthropic / GPT Alt",
      );
    });
    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "OpenAI: GPT Test" }));
    await userEvent.click(screen.getByLabelText("Thinking"));
    await userEvent.click(screen.getByRole("button", { name: "Thinking: Low" }));

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "override defaults",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(queueCall![1]?.body as string)).toMatchObject({
        message: "override defaults",
        modelId: "gpt-test",
        providerId: "openai",
        thinkingLevel: "low",
      });
    });
  });
});
