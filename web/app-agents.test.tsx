import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentTranscriptResponse } from "./api/types";

import {
  agentDefinitions as agentDefinitionFixtures,
  agentTranscriptResponse,
  agentTeamSnapshot,
  appTestState,
  chatMessages,
  defaultComposerPlaceholder,
  defaultReviewSystemPrompt,
  deferred,
  enqueueChatStreamEvent,
  jsonResponse,
  mockFetch,
  renderApp,
  resetAppTestEnvironment,
  settings,
  workspace,
  workspaceChats,
} from "./test-utils/app-test-harness";

function installMessageListScrollMetrics(
  messageList: HTMLElement,
  options?: { clientHeight?: number; scrollHeight?: number; scrollTop?: number },
) {
  let scrollHeight = options?.scrollHeight ?? 1000;
  const clientHeight = options?.clientHeight ?? 500;
  let scrollTop = options?.scrollTop ?? 0;
  Object.defineProperties(messageList, {
    clientHeight: { configurable: true, get: () => clientHeight },
    scrollHeight: {
      configurable: true,
      get: () => scrollHeight,
      set: (value: number) => {
        scrollHeight = value;
      },
    },
    scrollTop: {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = Math.min(value, Math.max(0, scrollHeight - clientHeight));
      },
    },
  });
  return {
    get clientHeight() {
      return clientHeight;
    },
    get scrollHeight() {
      return scrollHeight;
    },
    set scrollHeight(value: number) {
      scrollHeight = value;
    },
    get scrollTop() {
      return scrollTop;
    },
    set scrollTop(value: number) {
      scrollTop = Math.min(value, Math.max(0, scrollHeight - clientHeight));
    },
  };
}

async function withMessageListScrollPrototypeMocks<T>(
  run: () => Promise<T>,
  options?: { clientHeight?: number; scrollHeight?: number },
): Promise<T> {
  const scrollHeight = options?.scrollHeight ?? 1000;
  const clientHeight = options?.clientHeight ?? 500;
  const scrollTops = new WeakMap<HTMLElement, number>();
  const previousScrollHeight = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollHeight",
  );
  const previousClientHeight = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "clientHeight",
  );
  const previousScrollTop = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollTop",
  );

  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get(this: HTMLElement) {
      if (this.classList?.contains("message-list")) {
        return scrollHeight;
      }
      return previousScrollHeight?.get?.call(this) ?? 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get(this: HTMLElement) {
      if (this.classList?.contains("message-list")) {
        return clientHeight;
      }
      return previousClientHeight?.get?.call(this) ?? 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollTop", {
    configurable: true,
    get(this: HTMLElement) {
      if (this.classList?.contains("message-list")) {
        return scrollTops.get(this) ?? 0;
      }
      return previousScrollTop?.get?.call(this) ?? 0;
    },
    set(this: HTMLElement, value: number) {
      if (this.classList?.contains("message-list")) {
        scrollTops.set(
          this,
          Math.min(Number(value), Math.max(0, scrollHeight - clientHeight)),
        );
        return;
      }
      previousScrollTop?.set?.call(this, value);
    },
  });

  try {
    return await run();
  } finally {
    if (previousScrollHeight) {
      Object.defineProperty(HTMLElement.prototype, "scrollHeight", previousScrollHeight);
    }
    if (previousClientHeight) {
      Object.defineProperty(HTMLElement.prototype, "clientHeight", previousClientHeight);
    }
    if (previousScrollTop) {
      Object.defineProperty(HTMLElement.prototype, "scrollTop", previousScrollTop);
    }
  }
}

function agentTranscriptPanelMessageList() {
  const transcriptPanel = screen
    .getByText("Worker, inspect the current task.")
    .closest(".chat-panel");
  if (!(transcriptPanel instanceof HTMLElement)) {
    throw new Error("Expected agent transcript panel");
  }
  const messageList = transcriptPanel.querySelector(".message-list");
  if (!(messageList instanceof HTMLElement)) {
    throw new Error("Expected agent transcript message list");
  }
  return messageList;
}

function installTrackingResizeObserver() {
  type TrackedObserver = {
    callback: ResizeObserverCallback;
    targets: Set<Element>;
  };
  const observers: TrackedObserver[] = [];
  const PreviousResizeObserver = window.ResizeObserver;

  class TrackingResizeObserver implements ResizeObserver {
    private readonly tracked: TrackedObserver;

    constructor(callback: ResizeObserverCallback) {
      this.tracked = { callback, targets: new Set() };
      observers.push(this.tracked);
    }

    observe(target: Element) {
      this.tracked.targets.add(target);
      const contentRect = {
        bottom: 300,
        height: 300,
        left: 0,
        right: 800,
        toJSON: () => ({}),
        top: 0,
        width: 800,
        x: 0,
        y: 0,
      } satisfies DOMRectReadOnly;
      this.tracked.callback(
        [
          {
            borderBoxSize: [],
            contentBoxSize: [],
            contentRect,
            devicePixelContentBoxSize: [],
            target,
          } satisfies ResizeObserverEntry,
        ],
        this,
      );
    }

    unobserve(target: Element) {
      this.tracked.targets.delete(target);
    }

    disconnect() {
      this.tracked.targets.clear();
    }
  }

  Object.defineProperty(window, "ResizeObserver", {
    configurable: true,
    value: TrackingResizeObserver,
  });

  return {
    flush(target?: Element) {
      for (const observer of observers) {
        const targets =
          target != null
            ? observer.targets.has(target)
              ? [target]
              : []
            : [...observer.targets];
        for (const observed of targets) {
          const contentRect = {
            bottom: 300,
            height: 300,
            left: 0,
            right: 800,
            toJSON: () => ({}),
            top: 0,
            width: 800,
            x: 0,
            y: 0,
          } satisfies DOMRectReadOnly;
          observer.callback(
            [
              {
                borderBoxSize: [],
                contentBoxSize: [],
                contentRect,
                devicePixelContentBoxSize: [],
                target: observed,
              } satisfies ResizeObserverEntry,
            ],
            observer as unknown as ResizeObserver,
          );
        }
      }
    },
    restore() {
      Object.defineProperty(window, "ResizeObserver", {
        configurable: true,
        value: PreviousResizeObserver,
      });
    },
  };
}

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
      "# Image Generation Agent\n\n## Identity\n\nYou are Foco's image generation agent.\n\n## Instructions\n\nTurn the user's request into a precise image prompt, call image_gen, and return the generated file paths with concise notes. Do not modify source files unless explicitly asked.\n\n## Tool Defaults\n\nUse image_gen with model \"gpt-image-2\" unless the user explicitly asks for another configured image model.",
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
      "## Tool Defaults",
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
      "## Tool Defaults",
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
        definition: {
          modelId: string;
          providerId?: string;
          systemPrompt: string;
        };
        id: string;
      };
      expect(body.id).toBe("agent-definition-image-gen");
      expect(body.definition.modelId).toBe("gpt-test");
      // Provider is derived server-side from model routing; the form no longer posts it.
      expect(body.definition.providerId).toBeUndefined();
      expect(body.definition.systemPrompt).toContain("## Tool Defaults");
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
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(await screen.findByText("Found the issue in the workspace notes.")).toBeInTheDocument();
    expect(await screen.findByText("Inspection complete.")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Open agent Coordinator" }));

    expect(await screen.findByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
  it("shows active remote Agent team errors without white-screening", async () => {
    vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1") ? new URL(url).pathname : url.split("?")[0];
      if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
        return jsonResponse({ error: "Remote agent actions are unsupported" }, { status: 400 });
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));

    expect(await screen.findByText("Remote agent actions are unsupported")).toBeInTheDocument();
  });

  it("ignores stale remote Agent team errors after switching chats", async () => {
    const delayedChatOneResponse = deferred<Response>();
    const emptyChatTwoSnapshot = {
      ...agentTeamSnapshot,
      team: {
        ...agentTeamSnapshot.team,
        chatId: "chat-2",
        coordinatorInstanceId: "agent-instance-remote-coordinator",
        id: "agent-team-chat-2",
      },
      instances: [],
      runEvents: [],
      tasks: [],
    };
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1") ? new URL(url).pathname : url.split("?")[0];
      if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
        return delayedChatOneResponse.promise;
      }
      if (path === "/api/workspaces/workspace-1/chats/chat-2/agent-team") {
        return jsonResponse(emptyChatTwoSnapshot);
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await waitFor(() => {
      expect(fetchMock.mock.calls.some(([url]) => url === "/api/workspaces/workspace-1/chats/chat-1/agent-team"))
        .toBe(true);
    });

    await userEvent.click(await screen.findByText("Second chat"));
    await waitFor(() => {
      expect(fetchMock.mock.calls.some(([url]) => url === "/api/workspaces/workspace-1/chats/chat-2/agent-team"))
        .toBe(true);
    });
    expect(await screen.findByText("No agent instances in this chat yet.")).toBeInTheDocument();

    await act(async () => {
      delayedChatOneResponse.resolve(
        jsonResponse({ error: "Remote agent actions are unsupported" }, { status: 400 }),
      );
    });

    await waitFor(() => {
      expect(screen.queryByText("Remote agent actions are unsupported")).not.toBeInTheDocument();
      expect(screen.queryByText("agent-instance-coordinator")).not.toBeInTheDocument();
    });
  });


  it("loads Agent transcript items from the instance transcript API", async () => {
    const snakeCaseTranscriptResponse = {
      ...agentTranscriptResponse,
      items: agentTranscriptResponse.items.map((item) =>
        item.id === "task:agent-task-1:run"
          ? {
              ...item,
              parts: item.parts.map((part) =>
                part.type === "toolCall"
                  ? { type: "toolCall", tool_call: part.toolCall }
                  : part,
              ),
            }
          : item,
      ),
    } as unknown as AgentTranscriptResponse;
    vi.mocked(fetch).mockImplementation(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(snakeCaseTranscriptResponse);
        }
        return mockFetch(input, init);
      },
    );
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([url]) =>
          String(url).includes(
            "/api/workspaces/workspace-1/agent-team/instances/agent-instance-worker/transcript?page=1&pageSize=25",
          ),
        ),
      ).toBe(true);
    });
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.getByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("Inspection complete.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();
  });

  it("refreshes empty Agent transcripts without replaying snapshot events", async () => {
    vi.mocked(fetch).mockImplementation(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse({
            ...agentTranscriptResponse,
            hasMore: false,
            items: [],
            totalCount: 0,
            totalPages: 1,
          });
        }
        return mockFetch(input, init);
      },
    );
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));

    expect(await screen.findByText("No agent messages yet.")).toBeInTheDocument();
    const transcriptRegion = screen
      .getByText("No agent messages yet.")
      .closest(".chat-panel");
    expect(transcriptRegion).not.toBeNull();
    await userEvent.click(
      within(transcriptRegion as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    await waitFor(() => {
      const transcriptCalls = fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/agent-instance-worker/transcript"),
      );
      expect(transcriptCalls.length).toBeGreaterThanOrEqual(2);
    });
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
    const firstTranscriptResponse: AgentTranscriptResponse = {
      ...agentTranscriptResponse,
      items: [
        {
          ...agentTranscriptResponse.items[1]!,
          content: "",
          kind: "Task run",
          parts: [
            {
              type: "reasoning" as const,
              text: "Checking workspace state.",
              durationMs: 1200,
            },
            {
              type: "toolCall" as const,
              toolCall: {
                completedAt: null,
                id: "tool-read-file",
                input: { path: "notes.md" },
                isError: false,
                name: "read_file",
                output: null,
                startedAt: "2026-06-05T10:00:03Z",
                status: "running",
              },
            },
          ],
          status: "streaming" as const,
          role: "assistant",
          taskStatus: "running",
        },
      ],
    };
    const secondTranscriptResponse = {
      ...firstTranscriptResponse,
      items: [
        {
          ...firstTranscriptResponse.items[0]!,
          content: "Still inspecting.",
          parts: [
            {
              type: "reasoning" as const,
              text: "Checking workspace state.",
              durationMs: 3500,
            },
            firstTranscriptResponse.items[0]!.parts[1]!,
            { type: "text" as const, text: "Still inspecting." },
          ],
        },
      ],
    };
    const snapshotRefreshGate = deferred<void>();
    const transcriptRefreshGate = deferred<void>();
    let deferRefreshes = false;
    let snapshotRequestCount = 0;
    let transcriptRequestCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          snapshotRequestCount += 1;
          if (deferRefreshes) {
            await snapshotRefreshGate.promise;
            return jsonResponse(secondSnapshot);
          }
          return jsonResponse(firstSnapshot);
        }
        if (path.endsWith("/agent-instance-worker/transcript")) {
          transcriptRequestCount += 1;
          if (deferRefreshes) {
            await transcriptRefreshGate.promise;
            return jsonResponse(secondTranscriptResponse);
          }
          return jsonResponse(firstTranscriptResponse);
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
    expect(screen.getByText("Read")).toBeInTheDocument();
    expect(screen.getByText("1.2 s")).toBeInTheDocument();
    expect(screen.queryByText("n/a")).not.toBeInTheDocument();
    expect(screen.queryByText("Inspection complete.")).not.toBeInTheDocument();

    const snapshotRequestsBeforeRefresh = snapshotRequestCount;
    const transcriptRequestsBeforeRefresh = transcriptRequestCount;
    deferRefreshes = true;
    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        instanceId: "agent-instance-worker",
        reason: "task_updated",
        revealPanel: false,
        teamId: "agent-team-1",
        type: "agentTeamRefresh",
        workspaceId: "workspace-1",
      });
    });

    await waitFor(() =>
      expect(snapshotRequestCount).toBeGreaterThan(snapshotRequestsBeforeRefresh),
    );
    const transcriptPanel = screen
      .getByText("Checking workspace state.")
      .closest(".chat-panel");
    expect(transcriptPanel).not.toBeNull();
    expect(screen.getByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();
    expect(screen.getByText("1.2 s")).toBeInTheDocument();
    expect(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    ).toBeEnabled();
    expect(screen.queryByText("Loading agent messages...")).not.toBeInTheDocument();

    await act(async () => {
      snapshotRefreshGate.resolve();
    });
    await waitFor(() =>
      expect(transcriptRequestCount).toBeGreaterThan(transcriptRequestsBeforeRefresh),
    );
    expect(screen.getByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();
    expect(screen.getByText("1.2 s")).toBeInTheDocument();
    expect(screen.queryByText("Loading agent messages...")).not.toBeInTheDocument();

    await act(async () => {
      transcriptRefreshGate.resolve();
    });
    await waitFor(
      () => expect(screen.getByText("Still inspecting.")).toBeInTheDocument(),
      { timeout: 2500 },
    );
    expect(screen.getByText("3.5 s")).toBeInTheDocument();
    expect(screen.queryByText("1.2 s")).not.toBeInTheDocument();
    expect(screen.queryByText("n/a")).not.toBeInTheDocument();
  });

  it("restores cached Worker transcript immediately when switching back while refresh is deferred", async () => {
    const snapshotRefreshGate = deferred<void>();
    const transcriptRefreshGate = deferred<void>();
    let deferBackgroundRefresh = false;
    let snapshotRequestCount = 0;
    let transcriptRequestCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          snapshotRequestCount += 1;
          if (deferBackgroundRefresh && snapshotRequestCount > 1) {
            await snapshotRefreshGate.promise;
          }
          return jsonResponse(agentTeamSnapshot);
        }
        if (path.endsWith("/agent-instance-worker/transcript")) {
          transcriptRequestCount += 1;
          if (deferBackgroundRefresh && transcriptRequestCount > 1) {
            await transcriptRefreshGate.promise;
          }
          return jsonResponse(agentTranscriptResponse);
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));

    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.getByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("Inspection complete.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();

    deferBackgroundRefresh = true;
    const snapshotCountAfterOpen = snapshotRequestCount;
    const transcriptCountAfterOpen = transcriptRequestCount;

    await userEvent.click(await screen.findByRole("tab", { name: /Tool run/ }));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.queryByText("Worker, inspect the current task.")).not.toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: /Worker/ }));

    expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.getByText("Checking workspace state.")).toBeInTheDocument();
    expect(screen.getByText("Inspection complete.")).toBeInTheDocument();
    expect(screen.getByText("Read")).toBeInTheDocument();
    expect(screen.queryByText("Loading agent messages...")).not.toBeInTheDocument();

    const transcriptPanel = screen
      .getByText("Worker, inspect the current task.")
      .closest(".chat-panel");
    expect(transcriptPanel).not.toBeNull();
    expect(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    ).toBeEnabled();

    await waitFor(() => {
      expect(snapshotRequestCount).toBeGreaterThan(snapshotCountAfterOpen);
      expect(transcriptRequestCount).toBeGreaterThan(transcriptCountAfterOpen);
    });

    expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.queryByText("Loading agent messages...")).not.toBeInTheDocument();

    await act(async () => {
      snapshotRefreshGate.resolve();
      transcriptRefreshGate.resolve();
    });

    await waitFor(() => {
      expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
    });
  });

  it("keeps agent transcript caches isolated across chats and prunes closed tabs", async () => {
    const chat2Snapshot = {
      ...agentTeamSnapshot,
      team: {
        ...agentTeamSnapshot.team,
        chatId: "chat-2",
        id: "agent-team-2",
      },
      instances: agentTeamSnapshot.instances.map((instance) => ({
        ...instance,
        id:
          instance.id === "agent-instance-worker"
            ? "agent-instance-worker-2"
            : "agent-instance-coordinator-2",
        teamId: "agent-team-2",
      })),
      tasks: agentTeamSnapshot.tasks.map((task) => ({
        ...task,
        id: "agent-task-2",
        ownerInstanceId: "agent-instance-worker-2",
        originInstanceId: "agent-instance-coordinator-2",
        teamId: "agent-team-2",
      })),
    };
    const chat2Transcript = {
      ...agentTranscriptResponse,
      items: [
        {
          ...agentTranscriptResponse.items[0]!,
          content: "Chat two worker message.",
          id: "message:agent-message-chat-2",
        },
        {
          ...agentTranscriptResponse.items[1]!,
          content: "Chat two inspection complete.",
          id: "task:agent-task-2:run",
        },
      ],
    };
    let chat1TranscriptRequestCount = 0;
    let deferChat1Reload = false;
    const chat1ReloadGate = deferred<void>();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }
        if (path === "/api/workspaces/workspace-1/chats/chat-2/agent-team") {
          return jsonResponse(chat2Snapshot);
        }
        if (path.includes("/agent-team/instances/agent-instance-worker/transcript")) {
          chat1TranscriptRequestCount += 1;
          if (deferChat1Reload && chat1TranscriptRequestCount > 1) {
            await chat1ReloadGate.promise;
          }
          return jsonResponse(agentTranscriptResponse);
        }
        if (path.includes("/agent-team/instances/agent-instance-worker-2/transcript")) {
          return jsonResponse(chat2Transcript);
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    await userEvent.click(await screen.findByText("Second chat"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Chat two worker message.")).toBeInTheDocument();
    expect(screen.queryByText("Worker, inspect the current task.")).not.toBeInTheDocument();

    deferChat1Reload = true;
    const workerTabs = screen.getAllByRole("tab", { name: /Worker/ });
    expect(workerTabs.length).toBeGreaterThanOrEqual(2);
    const inactiveWorkerTab =
      workerTabs.find((tab) => tab.getAttribute("aria-selected") !== "true") ??
      workerTabs[0]!;
    await userEvent.click(inactiveWorkerTab);

    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.queryByText("Chat two worker message.")).not.toBeInTheDocument();
    expect(screen.queryByText("Loading agent messages...")).not.toBeInTheDocument();

    await act(async () => {
      chat1ReloadGate.resolve();
    });

    // Close the currently selected chat-1 Worker tab so its cache is pruned.
    const selectedWorkerTab =
      screen.getAllByRole("tab", { name: /Worker/ }).find(
        (tab) => tab.getAttribute("aria-selected") === "true",
      ) ?? screen.getAllByRole("tab", { name: /Worker/ })[0]!;
    await userEvent.click(
      within(selectedWorkerTab.parentElement as HTMLElement).getByRole("button", {
        name: "Close chat tab Worker",
      }),
    );

    // Re-open Worker for chat-1 from the Agents panel: must hard-load again (cache pruned).
    await userEvent.click(await screen.findByRole("tab", { name: /Tool run/ }));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    const transcriptCountBeforeReopen = chat1TranscriptRequestCount;
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    await waitFor(() => {
      expect(chat1TranscriptRequestCount).toBeGreaterThan(transcriptCountBeforeReopen);
    });
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
  });

  it("locks the agent transcript list to the bottom on first load and keeps lock without user intent", async () => {
    let transcriptVersion = 0;
    const resizeObserver = installTrackingResizeObserver();
    try {
      vi.stubGlobal(
        "fetch",
        vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
          const url = typeof input === "string" ? input : input.toString();
          const path = url.startsWith("http://127.0.0.1")
            ? new URL(url).pathname
            : url.split("?")[0];
          if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
            return jsonResponse(agentTeamSnapshot);
          }
          if (path.endsWith("/agent-instance-worker/transcript")) {
            if (transcriptVersion === 0) {
              return jsonResponse(agentTranscriptResponse);
            }
            return jsonResponse({
              ...agentTranscriptResponse,
              items: [
                ...agentTranscriptResponse.items,
                {
                  author: "Worker",
                  content: "Streaming follow-up line.",
                  createdAt: "2026-06-05T10:00:05Z",
                  id: "message:agent-message-follow-up",
                  kind: "Reply",
                  metrics: null,
                  parts: [],
                  role: "assistant",
                  status: null,
                  taskStatus: "running",
                },
              ],
              totalCount: 4,
            });
          }
          return mockFetch(input, init);
        }),
      );
      renderApp();

      await userEvent.click(await screen.findByText("Tool run"));
      await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
      await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
      expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

      const messageList = agentTranscriptPanelMessageList();
      const metrics = installMessageListScrollMetrics(messageList);
      const transcriptPanel = messageList.closest(".chat-panel");
      expect(transcriptPanel).not.toBeNull();

      // Metrics are installed after first paint; refresh re-runs lock layout with mocked heights.
      await userEvent.click(
        within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
      );
      await waitFor(() => expect(messageList.scrollTop).toBe(500));

      // Programmatic scroll without user intent must not unlock.
      fireEvent.scroll(messageList);
      metrics.scrollHeight = 1200;
      transcriptVersion = 1;
      await userEvent.click(
        within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
      );
      expect(await screen.findByText("Streaming follow-up line.")).toBeInTheDocument();
      await waitFor(() => expect(messageList.scrollTop).toBe(700));

      // Panel ResizeObserver must keep stick-to-bottom when content grows.
      metrics.scrollHeight = 1300;
      await act(async () => {
        resizeObserver.flush(messageList.querySelector(".message-stack") as Element);
      });
      await waitFor(() => expect(messageList.scrollTop).toBe(800));
    } finally {
      resizeObserver.restore();
    }
  });

  it("unlocks agent transcript bottom lock after user scroll intent and relocks near bottom", async () => {
    let transcriptVersion = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }
        if (path.endsWith("/agent-instance-worker/transcript")) {
          if (transcriptVersion === 0) {
            return jsonResponse(agentTranscriptResponse);
          }
          return jsonResponse({
            ...agentTranscriptResponse,
            items: [
              ...agentTranscriptResponse.items,
              {
                author: "Worker",
                content: `Unlocked growth ${transcriptVersion}.`,
                createdAt: "2026-06-05T10:00:06Z",
                id: `message:agent-message-growth-${transcriptVersion}`,
                kind: "Reply",
                metrics: null,
                parts: [],
                role: "assistant",
                status: null,
                taskStatus: "running",
              },
            ],
            totalCount: 4,
          });
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const messageList = agentTranscriptPanelMessageList();
    const metrics = installMessageListScrollMetrics(messageList);
    const transcriptPanel = messageList.closest(".chat-panel");
    expect(transcriptPanel).not.toBeNull();
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    await waitFor(() => expect(messageList.scrollTop).toBe(500));

    fireEvent.wheel(messageList);
    messageList.scrollTop = 120;
    fireEvent.scroll(messageList);
    expect(messageList.scrollTop).toBe(120);

    transcriptVersion = 1;
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    expect(await screen.findByText("Unlocked growth 1.")).toBeInTheDocument();
    metrics.scrollHeight = 1300;
    await waitFor(() => {
      expect(messageList.scrollTop).toBe(120);
    });

    // Soft refresh that grows items after unlock must not re-apply cache bottom restore.
    transcriptVersion = 2;
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    expect(await screen.findByText("Unlocked growth 2.")).toBeInTheDocument();
    metrics.scrollHeight = 1400;
    await waitFor(() => {
      expect(messageList.scrollTop).toBe(120);
    });

    // Relock when user scrolls back into the bottom threshold.
    messageList.scrollTop = metrics.scrollHeight - metrics.clientHeight;
    fireEvent.scroll(messageList);
    transcriptVersion = 3;
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    expect(await screen.findByText("Unlocked growth 3.")).toBeInTheDocument();
    await waitFor(() => expect(messageList.scrollTop).toBe(900));
  });

  it("restores stick-to-bottom and scrollTop from agent transcript tab cache", async () => {
    const transcriptRefreshGate = deferred<void>();
    let deferBackgroundRefresh = false;
    let transcriptRequestCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }
        if (path.endsWith("/agent-instance-worker/transcript")) {
          transcriptRequestCount += 1;
          if (deferBackgroundRefresh && transcriptRequestCount > 1) {
            await transcriptRefreshGate.promise;
          }
          return jsonResponse(agentTranscriptResponse);
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const lockedList = agentTranscriptPanelMessageList();
    installMessageListScrollMetrics(lockedList);
    const lockedPanel = lockedList.closest(".chat-panel");
    expect(lockedPanel).not.toBeNull();
    await userEvent.click(
      within(lockedPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    await waitFor(() => expect(lockedList.scrollTop).toBe(500));

    deferBackgroundRefresh = true;
    await withMessageListScrollPrototypeMocks(async () => {
      await userEvent.click(await screen.findByRole("tab", { name: /Tool run/ }));
      await userEvent.click(await screen.findByRole("tab", { name: /Worker/ }));
      expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
      const restoredLockedList = agentTranscriptPanelMessageList();
      await waitFor(() => expect(restoredLockedList.scrollTop).toBe(500));

      fireEvent.wheel(restoredLockedList);
      restoredLockedList.scrollTop = 80;
      fireEvent.scroll(restoredLockedList);
      expect(restoredLockedList.scrollTop).toBe(80);

      await userEvent.click(await screen.findByRole("tab", { name: /Tool run/ }));
      await userEvent.click(await screen.findByRole("tab", { name: /Worker/ }));
      expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
      const restoredScrolledList = agentTranscriptPanelMessageList();
      await waitFor(() => expect(restoredScrolledList.scrollTop).toBe(80));

      await act(async () => {
        transcriptRefreshGate.resolve();
      });
      await waitFor(() => expect(restoredScrolledList.scrollTop).toBe(80));
    });
  });

  it("does not re-lock after soft refresh once the user unlocks on the same mount", async () => {
    let transcriptVersion = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }
        if (path.endsWith("/agent-instance-worker/transcript")) {
          if (transcriptVersion === 0) {
            return jsonResponse(agentTranscriptResponse);
          }
          const growthItems = Array.from({ length: transcriptVersion }, (_, index) => {
            const version = index + 1;
            return {
              author: "Worker",
              content: `Soft growth ${version}.`,
              createdAt: `2026-06-05T10:00:0${version}Z`,
              id: `message:agent-message-soft-${version}`,
              kind: "Reply",
              metrics: null,
              parts: [],
              role: "assistant",
              status: null,
              taskStatus: "running",
            };
          });
          return jsonResponse({
            ...agentTranscriptResponse,
            items: [...agentTranscriptResponse.items, ...growthItems],
            totalCount: agentTranscriptResponse.items.length + growthItems.length,
          });
        }
        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const messageList = agentTranscriptPanelMessageList();
    const metrics = installMessageListScrollMetrics(messageList);
    const transcriptPanel = messageList.closest(".chat-panel");
    expect(transcriptPanel).not.toBeNull();
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    await waitFor(() => expect(messageList.scrollTop).toBe(500));

    // Unlock without remounting (simulates reading history while running soft refresh continues).
    fireEvent.wheel(messageList);
    messageList.scrollTop = 90;
    fireEvent.scroll(messageList);
    expect(messageList.scrollTop).toBe(90);

    transcriptVersion = 1;
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    expect(await screen.findByText("Soft growth 1.")).toBeInTheDocument();
    metrics.scrollHeight = 1500;
    await waitFor(() => {
      expect(messageList.scrollTop).toBe(90);
    });

    transcriptVersion = 2;
    await userEvent.click(
      within(transcriptPanel as HTMLElement).getByRole("button", { name: "Refresh" }),
    );
    expect(await screen.findByText("Soft growth 2.")).toBeInTheDocument();
    metrics.scrollHeight = 1600;
    await waitFor(() => {
      expect(messageList.scrollTop).toBe(90);
    });
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

  it("restores enabled Team mode after leaving Plan mode", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "handle this after planning",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const body = JSON.parse(queueCall![1]?.body as string);
      expect(body).toMatchObject({
        message: "handle this after planning",
        sessionMode: null,
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

  it("does not restore Plan mode from an unsent draft toggle", async () => {
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
    // Fixture last real user message has no sessionMode=plan; draft must not stick.
    expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("restores Plan mode from the last real user message when switching chats", async () => {
    const planChatMessages = {
      ...chatMessages,
      messages: [
        {
          ...chatMessages.messages[0],
          content: "Plan this feature.",
          id: "message-user-plan-last",
          parts: [{ text: "Plan this feature.", type: "text" }],
          sessionMode: "plan",
        },
        chatMessages.messages[1],
      ],
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
        return mockFetch(input, init);
      }),
    );
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await userEvent.click(await within(workspaceList).findByText("Tool run"));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });

    await userEvent.click(within(workspaceList).getByText("Second chat"));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "false",
      );
    });

    await userEvent.click(within(workspaceList).getByText("Tool run"));
    expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("keeps disabled Team mode after leaving Plan mode", async () => {
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

    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "stay without team tools",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const body = JSON.parse(queueCall![1]?.body as string);
      expect(body).toMatchObject({
        message: "stay without team tools",
        sessionMode: null,
        teamModeEnabled: false,
      });
    });
  });

  it("uses the default agent model provider and thinking level for a new composer", async () => {
    stubDefaultAgentComposerDefaults();
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Alt");
    });
    expect(screen.getByLabelText("Thinking")).toHaveTextContent("High");

    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "Model: GPT Test" }));
    await userEvent.click(screen.getByLabelText("Thinking"));
    await userEvent.click(screen.getByRole("button", { name: "Thinking: Low" }));
    expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    expect(screen.getByLabelText("Thinking")).toHaveTextContent("Low");

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Alt");
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

  it("switches composer model when Plan mode uses a configured plan model", async () => {
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
    const definitionsWithDefaultAgent = {
      agentDefinitions: agentDefinitionFixtures.agentDefinitions.map((definition) =>
        definition.id === "agent-definition-default"
          ? {
            ...definition,
            modelId: "gpt-test",
            modelOptions: { maxOutputTokens: null, thinkingLevel: "low" },
            providerId: "openai",
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
          return jsonResponse(settingsWithPlanModel);
        }
        if (path === "/api/agent-definitions") {
          return jsonResponse(definitionsWithDefaultAgent);
        }
        return mockFetch(input, init);
      }),
    );

    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    });

    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Alt");
    });

    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "false");
    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    });
  });

  it("keeps a manual model pick after sending while Plan mode stays enabled", async () => {
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
    const definitionsWithDefaultAgent = {
      agentDefinitions: agentDefinitionFixtures.agentDefinitions.map((definition) =>
        definition.id === "agent-definition-default"
          ? {
            ...definition,
            modelId: "gpt-test",
            modelOptions: { maxOutputTokens: null, thinkingLevel: "low" },
            providerId: "openai",
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
          return jsonResponse(settingsWithPlanModel);
        }
        if (path === "/api/agent-definitions") {
          return jsonResponse(definitionsWithDefaultAgent);
        }
        return mockFetch(input, init);
      }),
    );
    const fetchMock = vi.mocked(fetch);

    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    });

    const planModeToggle = await screen.findByRole("button", { name: "Plan mode" });
    await userEvent.click(planModeToggle);
    expect(planModeToggle).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Alt");
    });

    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "Model: GPT Test" }));
    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    });

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "keep manual plan model",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      expect(JSON.parse(queueCall![1]?.body as string)).toMatchObject({
        message: "keep manual plan model",
        modelId: "gpt-test",
        providerId: "openai",
        sessionMode: "plan",
      });
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Test");
    });
  });

  it("lets composer model provider and thinking selections override the default agent", async () => {
    stubDefaultAgentComposerDefaults();
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveTextContent("GPT Alt");
    });
    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "Model: GPT Test" }));
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

  it("keeps loaded earlier main-chat history after returning from a worker while parent run is active", async () => {
    const parentActiveRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 0,
      runId: "run-parent-1",
      workspaceId: "workspace-1",
    };
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier main-chat note.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-main",
      parts: [{ text: "Earlier main-chat note.", type: "text" }],
    };
    let latestPageLoads = 0;
    const hangingStream = () =>
      new Response(new ReadableStream({ start() {} }), {
        headers: { "Content-Type": "text/event-stream" },
        status: 200,
      });
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const requestUrl = new URL(url, "http://127.0.0.1");
        const path = requestUrl.pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspace.id,
            workspaces: [
              {
                ...workspace,
                chats: workspaceChats.slice(0, 5).map((chat) =>
                  chat.id === "chat-1" ? { ...chat, activeRun: parentActiveRun } : chat,
                ),
              },
            ],
          });
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          if (requestUrl.searchParams.get("beforeSequence") === "200") {
            return jsonResponse({
              ...chatMessages,
              activeRun: parentActiveRun,
              messages: [olderMessage],
              pagination: { hasMoreBefore: false, nextBeforeSequence: null },
            });
          }
          latestPageLoads += 1;
          return jsonResponse({
            ...chatMessages,
            activeRun: parentActiveRun,
            pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
          });
        }

        if (path.startsWith("/api/workspaces/workspace-1/chat/runs/run-parent-1/stream")) {
          return hangingStream();
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }

        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(agentTranscriptResponse);
        }

        return mockFetch(input, init);
      }),
    );

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Load earlier messages" }));
    expect(await screen.findByText("Earlier main-chat note.")).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.queryByText("Earlier main-chat note.")).not.toBeInTheDocument();

    const loadsBeforeReturn = latestPageLoads;
    await userEvent.click(screen.getByRole("button", { name: "Main chat" }));

    await waitFor(() => expect(latestPageLoads).toBeGreaterThan(loadsBeforeReturn));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.getByText("Earlier main-chat note.")).toBeInTheDocument();
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(screen.getAllByText("Earlier main-chat note.")).toHaveLength(1);
    expect(screen.getAllByText("Please inspect README.")).toHaveLength(1);
  });

  it("keeps streaming main assistant parts when returning from worker with temporary null activeRun", async () => {
    let messageLoads = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const requestUrl = new URL(url, "http://127.0.0.1");
        const path = requestUrl.pathname;

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          messageLoads += 1;
          // After the stream has started, the messages refresh can briefly omit
          // activeRun and the unfinished assistant message.
          const omitLiveAssistant = messageLoads > 1;
          return jsonResponse({
            ...chatMessages,
            activeRun: null,
            messages: omitLiveAssistant
              ? [
                  ...chatMessages.messages,
                  {
                    content: "Continue with workers",
                    createdAt: "2026-06-10T08:01:00.000Z",
                    extractedMemories: [],
                    id: "queued-user-1",
                    memoriesUsed: [],
                    metrics: null,
                    parts: [{ text: "Continue with workers", type: "text" }],
                    reasoning: null,
                    role: "user",
                    toolCalls: [],
                  },
                ]
              : chatMessages.messages,
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          });
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }

        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(agentTranscriptResponse);
        }

        return mockFetch(input, init);
      }),
    );

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "Continue with workers",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Live reasoning about workers",
        type: "reasoningDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Streaming main answer",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("Live reasoning about workers")).toBeInTheDocument();
    expect(await screen.findByText("Streaming main answer")).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const loadsBeforeReturn = messageLoads;
    await userEvent.click(screen.getByRole("button", { name: "Main chat" }));
    await waitFor(() => expect(messageLoads).toBeGreaterThan(loadsBeforeReturn));

    expect(await screen.findByText("Live reasoning about workers")).toBeInTheDocument();
    expect(screen.getByText("Streaming main answer")).toBeInTheDocument();
    expect(screen.getByText("Continue with workers")).toBeInTheDocument();
  });

  it("restores main chat history when closing the last agent tab", async () => {
    const parentActiveRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 0,
      runId: "run-parent-close",
      workspaceId: "workspace-1",
    };
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "History before agent tab close.",
      createdAt: "2026-06-10T07:58:00.000Z",
      id: "message-older-close",
      parts: [{ text: "History before agent tab close.", type: "text" }],
    };
    let latestPageLoads = 0;
    const hangingStream = () =>
      new Response(new ReadableStream({ start() {} }), {
        headers: { "Content-Type": "text/event-stream" },
        status: 200,
      });
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const requestUrl = new URL(url, "http://127.0.0.1");
        const path = requestUrl.pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspace.id,
            workspaces: [
              {
                ...workspace,
                chats: workspaceChats.slice(0, 5).map((chat) =>
                  chat.id === "chat-1" ? { ...chat, activeRun: parentActiveRun } : chat,
                ),
              },
            ],
          });
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          if (requestUrl.searchParams.get("beforeSequence") === "200") {
            return jsonResponse({
              ...chatMessages,
              activeRun: parentActiveRun,
              messages: [olderMessage],
              pagination: { hasMoreBefore: false, nextBeforeSequence: null },
            });
          }
          latestPageLoads += 1;
          return jsonResponse({
            ...chatMessages,
            activeRun: parentActiveRun,
            pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
          });
        }

        if (path.startsWith("/api/workspaces/workspace-1/chat/runs/run-parent-close/stream")) {
          return hangingStream();
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }

        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(agentTranscriptResponse);
        }

        return mockFetch(input, init);
      }),
    );

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Load earlier messages" }));
    expect(await screen.findByText("History before agent tab close.")).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const workerTab =
      screen.getAllByRole("tab", { name: /Worker/ }).find(
        (tab) => tab.getAttribute("aria-selected") === "true",
      ) ?? screen.getAllByRole("tab", { name: /Worker/ })[0]!;
    await userEvent.click(
      within(workerTab.parentElement as HTMLElement).getByRole("button", {
        name: "Close chat tab Worker",
      }),
    );

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.getByText("History before agent tab close.")).toBeInTheDocument();
    expect(latestPageLoads).toBeGreaterThanOrEqual(1);
  });

  it("keeps main-chat history across subagent return with disjoint latest page and temporary null activeRun", async () => {
    const parentActiveRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 0,
      runId: "run-parent-disjoint",
      workspaceId: "workspace-1",
    };
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier main-chat note before worker.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-disjoint",
      parts: [{ text: "Earlier main-chat note before worker.", type: "text" }],
    };
    const newAttemptUser = {
      content: "Continue after worker completed.",
      createdAt: "2026-06-10T08:05:00.000Z",
      extractedMemories: [],
      id: "message-user-post-worker",
      memoriesUsed: [],
      metrics: null,
      parts: [{ text: "Continue after worker completed.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    };
    let latestPageLoads = 0;
    let returnToMain = false;
    const hangingStream = () => {
      const encoder = new TextEncoder();
      return new Response(
        new ReadableStream({
          start(controller) {
            appTestState.activeChatStreamController = controller;
            appTestState.chatStreamControllers.set(parentActiveRun.runId, controller);
            controller.enqueue(
              encoder.encode(
                `data: ${JSON.stringify({
                  type: "start",
                  chatId: "chat-1",
                  userMessageId: "message-user-pre-worker",
                  assistantMessageId: "message-assistant-pre-worker",
                  llmRequestId: "request-pre-worker",
                  memoriesUsed: [],
                })}\n\n`,
              ),
            );
          },
        }),
        {
          headers: { "Content-Type": "text/event-stream" },
          status: 200,
        },
      );
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const requestUrl = new URL(url, "http://127.0.0.1");
        const path = requestUrl.pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspace.id,
            workspaces: [
              {
                ...workspace,
                chats: workspaceChats.slice(0, 5).map((chat) =>
                  chat.id === "chat-1" ? { ...chat, activeRun: parentActiveRun } : chat,
                ),
              },
            ],
          });
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          if (requestUrl.searchParams.get("beforeSequence") === "200") {
            return jsonResponse({
              ...chatMessages,
              activeRun: parentActiveRun,
              messages: [olderMessage],
              pagination: { hasMoreBefore: false, nextBeforeSequence: null },
            });
          }
          latestPageLoads += 1;
          // After returning from the worker, the messages API briefly returns only
          // the new attempt tail with zero id overlap and temporary null activeRun.
          if (returnToMain) {
            return jsonResponse({
              ...chatMessages,
              activeRun: null,
              messages: [newAttemptUser],
              pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
            });
          }
          return jsonResponse({
            ...chatMessages,
            activeRun: parentActiveRun,
            pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
          });
        }

        if (path.startsWith("/api/workspaces/workspace-1/chat/runs/run-parent-disjoint/stream")) {
          return hangingStream();
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }

        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(agentTranscriptResponse);
        }

        return mockFetch(input, init);
      }),
    );

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.click(screen.getByRole("button", { name: "Load earlier messages" }));
    expect(await screen.findByText("Earlier main-chat note before worker.")).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();
    expect(screen.queryByText("Earlier main-chat note before worker.")).not.toBeInTheDocument();

    const loadsBeforeReturn = latestPageLoads;
    returnToMain = true;
    await userEvent.click(screen.getByRole("button", { name: "Main chat" }));

    await waitFor(() => expect(latestPageLoads).toBeGreaterThan(loadsBeforeReturn));
    expect(await screen.findByText("Continue after worker completed.")).toBeInTheDocument();
    expect(screen.getByText("Earlier main-chat note before worker.")).toBeInTheDocument();
    expect(screen.getByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(screen.getAllByText("Earlier main-chat note before worker.")).toHaveLength(1);
    expect(screen.getAllByText("Please inspect README.")).toHaveLength(1);
    expect(screen.getAllByText("Continue after worker completed.")).toHaveLength(1);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-post-worker",
        delta: "Post-worker recovery answer",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("Post-worker recovery answer")).toBeInTheDocument();
    expect(screen.getByText("Earlier main-chat note before worker.")).toBeInTheDocument();
    expect(screen.getByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.getByText("Continue after worker completed.")).toBeInTheDocument();
    expect(screen.getAllByText("Post-worker recovery answer")).toHaveLength(1);
  });

  it("does not resurrect deleted history when zero-overlap latest page reports a different activeRun", async () => {
    const parentActiveRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 0,
      runId: "run-parent-old-thread",
      workspaceId: "workspace-1",
    };
    const rewrittenActiveRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 0,
      runId: "run-parent-edited-thread",
      workspaceId: "workspace-1",
    };
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Deleted main-chat history before edit rewrite.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-edited-away",
      parts: [{ text: "Deleted main-chat history before edit rewrite.", type: "text" }],
    };
    const rewrittenUser = {
      content: "Edited rewrite prompt after other client.",
      createdAt: "2026-06-10T08:06:00.000Z",
      extractedMemories: [],
      id: "message-user-rewritten",
      memoriesUsed: [],
      metrics: null,
      parts: [{ text: "Edited rewrite prompt after other client.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    };
    let latestPageLoads = 0;
    let returnToMain = false;
    const hangingStream = (runId: string) => {
      const encoder = new TextEncoder();
      return new Response(
        new ReadableStream({
          start(controller) {
            appTestState.activeChatStreamController = controller;
            appTestState.chatStreamControllers.set(runId, controller);
            controller.enqueue(
              encoder.encode(
                `data: ${JSON.stringify({
                  type: "start",
                  chatId: "chat-1",
                  userMessageId:
                    runId === rewrittenActiveRun.runId
                      ? "message-user-rewritten"
                      : "message-user-pre-worker",
                  assistantMessageId:
                    runId === rewrittenActiveRun.runId
                      ? "message-assistant-rewritten"
                      : "message-assistant-pre-worker",
                  llmRequestId: `request-${runId}`,
                  memoriesUsed: [],
                })}\n\n`,
              ),
            );
          },
        }),
        {
          headers: { "Content-Type": "text/event-stream" },
          status: 200,
        },
      );
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const requestUrl = new URL(url, "http://127.0.0.1");
        const path = requestUrl.pathname;

        if (path === "/api/workspaces") {
          return jsonResponse({
            activeWorkspaceId: workspace.id,
            workspaces: [
              {
                ...workspace,
                chats: workspaceChats.slice(0, 5).map((chat) =>
                  chat.id === "chat-1"
                    ? {
                        ...chat,
                        activeRun: returnToMain ? rewrittenActiveRun : parentActiveRun,
                      }
                    : chat,
                ),
              },
            ],
          });
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          if (requestUrl.searchParams.get("beforeSequence") === "200") {
            return jsonResponse({
              ...chatMessages,
              activeRun: parentActiveRun,
              messages: [olderMessage],
              pagination: { hasMoreBefore: false, nextBeforeSequence: null },
            });
          }
          latestPageLoads += 1;
          // Zero-overlap rewritten thread with a *different* active run id must
          // not keep the discarded cache history even if this tab still thinks
          // the old parent run is live.
          if (returnToMain) {
            return jsonResponse({
              ...chatMessages,
              activeRun: rewrittenActiveRun,
              messages: [rewrittenUser],
              pagination: { hasMoreBefore: false, nextBeforeSequence: null },
            });
          }
          return jsonResponse({
            ...chatMessages,
            activeRun: parentActiveRun,
            pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
          });
        }

        if (path.startsWith("/api/workspaces/workspace-1/chat/runs/run-parent-old-thread/stream")) {
          return hangingStream(parentActiveRun.runId);
        }
        if (path.startsWith("/api/workspaces/workspace-1/chat/runs/run-parent-edited-thread/stream")) {
          return hangingStream(rewrittenActiveRun.runId);
        }

        if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
          return jsonResponse(agentTeamSnapshot);
        }

        if (path.endsWith("/agent-instance-worker/transcript")) {
          return jsonResponse(agentTranscriptResponse);
        }

        return mockFetch(input, init);
      }),
    );

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.click(screen.getByRole("button", { name: "Load earlier messages" }));
    expect(
      await screen.findByText("Deleted main-chat history before edit rewrite."),
    ).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("tab", { name: "Agents" }));
    await userEvent.click(await screen.findByRole("button", { name: "Open agent Worker" }));
    expect(await screen.findByText("Worker, inspect the current task.")).toBeInTheDocument();

    const loadsBeforeReturn = latestPageLoads;
    returnToMain = true;
    await userEvent.click(screen.getByRole("button", { name: "Main chat" }));

    await waitFor(() => expect(latestPageLoads).toBeGreaterThan(loadsBeforeReturn));
    expect(
      await screen.findByText("Edited rewrite prompt after other client."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Deleted main-chat history before edit rewrite."),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Please inspect README.")).not.toBeInTheDocument();
    expect(screen.queryByText("Done.")).not.toBeInTheDocument();
    expect(
      screen.getAllByText("Edited rewrite prompt after other client."),
    ).toHaveLength(1);
  });
});
