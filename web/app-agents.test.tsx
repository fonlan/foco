import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  agentDefinitions as agentDefinitionFixtures,
  appTestState,
  defaultComposerPlaceholder,
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
    agentDefinitions: [
      {
        ...agentDefinitionFixtures.agentDefinitions[0],
        id: "agent-definition-default",
        modelId: "gpt-alt",
        modelOptions: { maxOutputTokens: null, thinkingLevel: "high" },
        name: "Default agent",
        providerId: "anthropic",
      },
      ...agentDefinitionFixtures.agentDefinitions,
    ],
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
    ).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Edit agent Coordinator" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete agent Coordinator" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Add agent definition" }));
    const dialog = screen.getByRole("dialog", { name: "Create agent" });
    expect(within(dialog).getByLabelText("System prompt")).toHaveValue("Default");
    await userEvent.click(within(dialog).getByText("Allowed tools"));
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "read_file" }));
    expect(within(dialog).getByText("1 selected")).toBeInTheDocument();
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
        defaultTeamModeEnabled: true,
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

  it("queues the first message with Team tools disabled by default from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Team mode" });
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
        teamModeEnabled: false,
      });
    });
  });

  it("queues the first message with Team tools enabled from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Team mode" });
    expect(teamToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(teamToggle);
    expect(teamToggle).toHaveAttribute("aria-pressed", "true");

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
        teamModeEnabled: true,
      });
    });
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
              defaultTeamModeEnabled: true,
            },
          })
          : mockFetch(input, init);
      }),
    );
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Team mode" });
    expect(teamToggle).toHaveAttribute("aria-pressed", "true");

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
        teamModeEnabled: true,
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
