import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const workspace = {
  chats: [
    {
      createdAt: "2026-06-05T10:00:00Z",
      id: "chat-1",
      title: "Tool run",
      updatedAt: "2026-06-05T10:05:00Z",
    },
    {
      createdAt: "2026-06-05T11:00:00Z",
      id: "chat-2",
      title: "Second chat",
      updatedAt: "2026-06-05T11:05:00Z",
    },
  ],
  id: "workspace-1",
  name: "Default",
  path: "C:\\Users\\fonla\\.foco\\workspace",
  pinned: false,
  terminalShell: "powershell",
};

const settings = {
  configuredModels: [
    {
      activeProviderId: "openai",
      canEnable: true,
      contextWindow: 128000,
      displayName: "GPT Test",
      enabled: true,
      id: "gpt-test",
      maxOutputTokens: 4096,
      metadataKey: null,
      metadataRefreshedAt: null,
      metadataSourceUrl: null,
      missingLimits: [],
      providerIds: ["openai"],
      supportsThinking: true,
      thinkingLevel: null,
      warnings: [],
    },
  ],
  general: {
    language: "en",
    supportedLanguages: [
      { id: "en", name: "English" },
      { id: "zh-CN", name: "简体中文" },
    ],
    webServer: { listenHost: "127.0.0.1", listenPort: 3210 },
  },
  mcpServers: [
    {
      args: ["serve"],
      command: "foco-mcp-test",
      enabled: true,
      error: null,
      id: "codegraph",
      name: "CodeGraph",
      state: "connected",
      toolCount: 2,
      transport: "stdio",
      transportLabel: "stdio",
      url: null,
      warnings: [],
    },
  ],
  mcpTransports: [
    { label: "stdio", transport: "stdio" },
    { label: "streamable-http", transport: "streamable-http" },
  ],
  terminalShells: [
    { label: "PowerShell", shell: "powershell" },
    { label: "Command Prompt", shell: "cmd" },
    { label: "Bash", shell: "bash" },
    { label: "Zsh", shell: "zsh" },
  ],
  providerKinds: [
    {
      defaultBaseUrl: "https://api.openai.com/v1",
      kind: "openai-chat",
      label: "OpenAI Chat",
    },
    {
      defaultBaseUrl: "https://api.openai.com/v1",
      kind: "openai-responses",
      label: "OpenAI Responses",
    },
  ],
  providers: [
    {
      baseUrl: "https://api.openai.com/v1",
      enabled: true,
      hasApiKey: true,
      id: "openai",
      kind: "openai-chat",
      kindLabel: "OpenAI Chat",
      name: "OpenAI",
      warnings: [],
    },
  ],
  skills: {
    detected: [
      {
        description: "Project memory.",
        enabled: true,
        key: "global:gitmemo",
        id: "gitmemo",
        name: "gitmemo",
        path: "C:\\Users\\fonla\\.agents\\skills\\gitmemo\\SKILL.md",
        scope: "global",
        workspaceId: null,
        workspaceName: null,
        warnings: [],
      },
    ],
    directories: ["C:\\Users\\fonla\\.agents\\skills"],
    errors: [],
  },
  thinkingLevels: [
    { label: "Low", value: "low" },
    { label: "High", value: "high" },
  ],
  workspaces: [
    {
      id: workspace.id,
      isDefault: true,
      name: workspace.name,
      path: workspace.path,
      pinned: workspace.pinned,
      terminalShell: workspace.terminalShell,
    },
  ],
};

const aiStatistics = {
  page: 1,
  pageSize: 50,
  requests: [
    {
      cacheRatio: 0.25,
      cacheReadTokens: 10,
      cacheWriteTokens: 2,
      chatId: "chat-1",
      chatTitle: "Tool run",
      completedAt: "2026-06-05T10:00:02Z",
      finalState: "succeeded",
      firstTokenAt: "2026-06-05T10:00:01Z",
      firstTokenLatencyMs: 1000,
      id: "request-1",
      inputTokens: 100,
      modelId: "gpt-test",
      outputTokens: 40,
      providerId: "openai",
      requestStartedAt: "2026-06-05T10:00:00Z",
      statusCode: 200,
      totalLatencyMs: 2000,
      workspaceId: "workspace-1",
      workspaceName: "Default",
    },
  ],
  totalCount: 125,
  totalPages: 3,
};

const aiStatisticsDetail = {
  events: [
    {
      eventAt: "2026-06-05T10:00:01Z",
      eventType: "textDelta",
      id: "event-1",
      normalizedEvent: { delta: "Done.", type: "textDelta" },
      rawChunk: { choices: [] },
      sequence: 1,
    },
  ],
  request: {
    ...aiStatistics.requests[0],
    requestBody: { messages: [{ content: "Hello", role: "user" }] },
    responseBody: { text: "Done." },
  },
};

const savedSettings = {
  mcp: {
    ...settings,
    mcpServers: [
      ...settings.mcpServers,
      {
        args: [],
        command: "foco-test-mcp",
        enabled: true,
        error: null,
        id: "test-mcp",
        name: "Test MCP",
        state: "stopped",
        toolCount: 0,
        transport: "stdio",
        transportLabel: "stdio",
        url: null,
        warnings: [],
      },
    ],
  },
  provider: {
    ...settings,
    providers: [
      ...settings.providers,
      {
        baseUrl: null,
        enabled: true,
        hasApiKey: false,
        id: "test-provider",
        kind: "openai-chat",
        kindLabel: "OpenAI Chat",
        name: "Test Provider",
        warnings: [],
      },
    ],
  },
  skills: {
    ...settings,
    skills: {
      ...settings.skills,
      directories: ["C:\\Users\\fonla\\.agents\\skills", ".agents\\skills"],
    },
  },
  workspace: {
    ...settings,
    workspaces: [
      {
        ...settings.workspaces[0],
        name: "Renamed Workspace",
        pinned: true,
        terminalShell: "cmd",
      },
    ],
  },
};

const savedModelMetadata = {
  cachePath: "C:\\Users\\fonla\\.foco\\models.dev.json",
  configuredModels: [
    ...settings.configuredModels,
    {
      activeProviderId: "openai",
      canEnable: true,
      contextWindow: 32000,
      displayName: "Created Model",
      enabled: true,
      id: "created-model",
      maxOutputTokens: 2048,
      metadataKey: null,
      metadataRefreshedAt: null,
      metadataSourceUrl: null,
      missingLimits: [],
      providerIds: ["openai"],
      supportsThinking: false,
      thinkingLevel: null,
      warnings: [],
    },
  ],
  fetchedAt: "2026-06-05T10:00:00Z",
  models: [],
  sourceUrl: "https://models.dev/api.json",
};

const gitDiff = {
  diff: "diff --git a/README.md b/README.md\n+hello\n",
  files: [
    {
      indexStatus: "M",
      path: "README.md",
      worktreeStatus: "M",
    },
  ],
  path: null,
  stagedDiff: "",
  status: " M README.md\n",
};

const chatMessages = {
  messages: [
    {
      content: "Please inspect README.",
      id: "message-user",
      metrics: null,
      parts: [{ text: "Please inspect README.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    },
    {
      content: "Done.",
      id: "message-assistant",
      metrics: {
        firstTokenLatencyMs: 250,
        modelId: "gpt-test",
        outputTokens: 40,
        providerId: "openai",
        totalLatencyMs: 2000,
      },
      parts: [
        { text: "Need file context.\n\nThen answer.", type: "reasoning" },
        {
          toolCall: {
            id: "tool-1",
            input: { path: "README.md" },
            isError: false,
            name: "read_file",
            output: { content: "hello" },
            status: "completed",
          },
          type: "toolCall",
        },
        { text: "Done.", type: "text" },
      ],
      reasoning: "Need file context.\n\nThen answer.",
      role: "assistant",
      toolCalls: [
        {
          id: "tool-1",
          input: { path: "README.md" },
          isError: false,
          name: "read_file",
          output: { content: "hello" },
          status: "completed",
        },
      ],
    },
  ],
};

const secondChatMessages = {
  messages: [
    {
      content: "Second question.",
      id: "message-user-2",
      metrics: null,
      parts: [{ text: "Second question.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    },
    {
      content: "Second answer.",
      id: "message-assistant-2",
      metrics: null,
      parts: [{ text: "Second answer.", type: "text" }],
      reasoning: null,
      role: "assistant",
      toolCalls: [],
    },
  ],
};

const taskGraph = {
  chatId: "chat-1",
  createdAt: "2026-06-05T10:01:00Z",
  exists: true,
  tasks: [
    {
      acceptance: ["README.md diff is visible"],
      createdAt: "2026-06-05T10:01:00Z",
      dependsOn: [],
      id: "task-1",
      status: "running",
      subtasks: [
        {
          acceptance: ["Tool result is persisted"],
          createdAt: "2026-06-05T10:02:00Z",
          dependsOn: ["task-1"],
          id: "task-1.1",
          status: "completed",
          subtasks: [],
          summary: "read_file returned README context.",
          title: "Persist tool result",
          updatedAt: "2026-06-05T10:04:00Z",
        },
      ],
      summary: "Coordinate the current tool run.",
      title: "Inspect workspace changes",
      updatedAt: "2026-06-05T10:05:00Z",
    },
  ],
  updatedAt: "2026-06-05T10:05:00Z",
};

let activeChatStreamController: ReadableStreamDefaultController<Uint8Array> | null =
  null;
let terminalSessionCounter = 0;

describe("App verification surfaces", () => {
  beforeEach(() => {
    activeChatStreamController = null;
    terminalSessionCounter = 0;
    vi.stubGlobal("fetch", vi.fn(mockFetch));
  });

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    render(<App />);

    expect(await screen.findAllByText("Default")).not.toHaveLength(0);
    expect(screen.getByText("Tool run")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Tool run"));

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const reasoningToggle = screen.getByRole("button", {
      name: "Expand thinking",
    });
    expect(reasoningToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Need file context. Then answer.")).toBeInTheDocument();
    expect(screen.queryByText("Then answer.")).not.toBeInTheDocument();

    await userEvent.click(reasoningToggle);

    expect(reasoningToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Need file context.")).toBeInTheDocument();
    expect(screen.getByText("Then answer.")).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("README.md")).toBeInTheDocument();
    expect(
      screen.getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"path": "README.md')),
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Channel: openai")).toBeInTheDocument();
    expect(screen.getByText("Total time: 2,000 ms")).toBeInTheDocument();
    expect(screen.getByText("tokens/s: 20")).toBeInTheDocument();
    expect(screen.getByText("First token latency: 250 ms")).toBeInTheDocument();
  });

  it("expands a collapsed workspace after starting a workspace chat", async () => {
    render(<App />);

    const workspaceToggle = await screen.findByRole("button", { name: "Default" });
    await userEvent.click(workspaceToggle);
    expect(workspaceToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );

    expect(workspaceToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "New chat" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("opens center chat tabs and closes tabs without deleting chat history", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(within(tabList).getByText("Default")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Tool run/ }));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    );

    expect(
      within(tabList).queryByRole("tab", { name: /Tool run/ }),
    ).not.toBeInTheDocument();
    expect(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Tool run")).toBeInTheDocument();
  });

  it("asks for confirmation before deleting a chat", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findByText("Tool run");
    await userEvent.click(
      screen.getByRole("button", { name: "Delete chat Tool run" }),
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

  it("shows a running spinner instead of a close button on a streaming chat tab", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    ).toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText("Message Foco"), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    expect(
      await within(tabList).findByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    expect(
      within(tabList).queryByRole("button", { name: "Close chat tab Tool run" }),
    ).not.toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("adds a workspace with a selectable slash-style path", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Add workspace" }));

    const dialog = await screen.findByRole("dialog", { name: "Add workspace" });
    const nameInput = within(dialog).getByPlaceholderText("Workspace name");
    const pathInput = within(dialog).getByPlaceholderText("C:/Users/name/workspace");
    expect(pathInput).toBeInTheDocument();

    await userEvent.click(within(dialog).getByRole("button", { name: "Choose workspace path" }));

    await waitFor(() => {
      expect(pathInput).toHaveValue("C:/Users/fonla/Documents/Repos/NewWorkspace");
      expect(nameInput).toHaveValue("NewWorkspace");
    });

    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/add",
        expect.objectContaining({
          body: JSON.stringify({
            name: "NewWorkspace",
            path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
          }),
          method: "POST",
        }),
      );
    });

    expect(screen.queryByRole("dialog", { name: "Add workspace" })).not.toBeInTheDocument();
  });

  it("shows settings sections for providers, models, MCP servers, and skills", async () => {
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(await screen.findByText("General settings")).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:3210")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(screen.getByText("Configured providers")).toBeInTheDocument();
    expect(screen.getByText("OpenAI")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    expect(screen.getByText("Model settings")).toBeInTheDocument();
    expect(screen.getByText("GPT Test")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "MCP" }));
    expect(screen.getByText("MCP servers")).toBeInTheDocument();
    expect(screen.getByText("CodeGraph")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Skills" }));
    expect(screen.getByText("Detected skills")).toBeInTheDocument();
    expect(screen.getByText("Skill locations")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Refresh skill discovery" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Save skills" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Global skill")).toBeInTheDocument();
    expect(screen.getAllByText("gitmemo")).not.toHaveLength(0);
  });

  it("saves provider, model, MCP server, and skill settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);

    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));
    await userEvent.type(screen.getByLabelText("Name"), "Test Provider");
    await userEvent.click(screen.getByRole("button", { name: "Save provider" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/providers/manual",
        expect.objectContaining({
          body: expect.stringContaining('"name":"Test Provider"'),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    await userEvent.type(screen.getByLabelText("Model id"), "created-model");
    await userEvent.type(screen.getByLabelText("Display name"), "Created Model");
    await userEvent.type(screen.getByLabelText("Context window"), "32000");
    await userEvent.type(screen.getByLabelText("Max output tokens"), "2048");
    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/models/manual",
        expect.objectContaining({
          body: expect.stringContaining('"modelId":"created-model"'),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "MCP" }));
    await userEvent.click(screen.getByRole("button", { name: "Add MCP server" }));
    await userEvent.type(screen.getByLabelText("Name"), "Test MCP");
    await userEvent.type(screen.getByLabelText("Command"), "foco-test-mcp");
    await userEvent.click(screen.getByRole("button", { name: "Save MCP server" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/mcp/servers/manual",
        expect.objectContaining({
          body: expect.stringContaining('"name":"Test MCP"'),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Skills" }));
    await userEvent.click(screen.getByLabelText("Enable skill gitmemo"));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skills/manual",
        expect.objectContaining({
          body: JSON.stringify({
            disabled: ["global:gitmemo"],
            enabled: [],
          }),
          method: "POST",
        }),
      );
    });
  });

  it("opens the git diff panel and terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Open git diff" }));

    expect(await screen.findByText("README.md")).toBeInTheDocument();
    expect(screen.getByText(/\+hello/)).toBeInTheDocument();

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
  });

  it("shows the active chat task graph above the git diff panel", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(screen.getByRole("button", { name: "Open git diff" }));

    expect(await screen.findByText("Task graph")).toBeInTheDocument();
    expect(screen.getByText("Inspect workspace changes")).toBeInTheDocument();
    expect(screen.getByText("README.md diff is visible")).toBeInTheDocument();
    expect(screen.getByText("Git diff")).toBeInTheDocument();
    expect(screen.getByText(/\+hello/)).toBeInTheDocument();
  });

  it("opens the task graph sidebar when a task graph refresh arrives", async () => {
    render(<App />);

    await userEvent.type(screen.getByPlaceholderText("Message Foco"), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        type: "taskGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByText("Task graph")).toBeInTheDocument();
    expect(screen.getByText("Inspect workspace changes")).toBeInTheDocument();
    expect(screen.getByText("Git diff")).toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("shows AI statistics and request details", async () => {
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Stats" }))[0]);

    expect(await screen.findByText("API statistics")).toBeInTheDocument();
    expect(screen.getByText("Request audit")).toBeInTheDocument();
    const table = screen.getByRole("table");
    expect(within(table).getByText("openai")).toBeInTheDocument();
    expect(within(table).getByText("gpt-test")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Request audit pagination" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to page 2" })).toBeInTheDocument();
    expect(screen.getByLabelText("Page size")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(screen.getByRole("checkbox", { name: "Provider" }));
    expect(within(table).queryByText("openai")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "View request details" }));

    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(within(dialog).getByText("Request body")).toBeInTheDocument();
    expect(within(dialog).getByText("Response body")).toBeInTheDocument();
    expect(within(dialog).queryByText("Stream events")).not.toBeInTheDocument();
  });
});

async function mockFetch(input: RequestInfo | URL): Promise<Response> {
  const url = typeof input === "string" ? input : input.toString();
  const path = url.startsWith("http://127.0.0.1") ? new URL(url).pathname : url.split("?")[0];

  if (path === "/api/workspaces") {
    return jsonResponse({ activeWorkspaceId: workspace.id, workspaces: [workspace] });
  }

  if (path === "/api/native/select-directory") {
    return jsonResponse({ path: "C:/Users/fonla/Documents/Repos/NewWorkspace" });
  }

  if (path === "/api/workspaces/add") {
    return jsonResponse({
      activeWorkspaceId: "new-workspace",
      workspaces: [
        workspace,
        {
          chats: [],
          id: "new-workspace",
          name: "New Workspace",
          path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
          pinned: false,
          terminalShell: "powershell",
        },
      ],
    });
  }

  if (path === "/api/settings") {
    return jsonResponse(settings);
  }

  if (path === "/api/workspaces/manual" || path === "/api/workspaces/order") {
    return jsonResponse(savedSettings.workspace);
  }

  if (path === "/api/model-metadata") {
    return jsonResponse({
      cachePath: "C:\\Users\\fonla\\.foco\\models.dev.json",
      configuredModels: settings.configuredModels,
      fetchedAt: "2026-06-05T10:00:00Z",
      models: [],
      sourceUrl: "https://models.dev/api.json",
    });
  }

  if (path === "/api/providers/manual") {
    return jsonResponse(savedSettings.provider);
  }

  if (path === "/api/models/manual") {
    return jsonResponse(savedModelMetadata);
  }

  if (path === "/api/mcp/servers/manual") {
    return jsonResponse(savedSettings.mcp);
  }

  if (path === "/api/skills/manual") {
    return jsonResponse(savedSettings.skills);
  }

  if (path === "/api/ai-statistics") {
    return jsonResponse(aiStatistics);
  }

  if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
    return jsonResponse(aiStatisticsDetail);
  }

  if (path === "/api/workspaces/workspace-1/git/branches") {
    return jsonResponse({
      branches: ["main"],
      currentBranch: "main",
      isGitRepository: true,
    });
  }

  if (path === "/api/workspaces/workspace-1/git/diff") {
    return jsonResponse(gitDiff);
  }

  if (path === "/api/workspaces/workspace-1/terminal/session") {
    terminalSessionCounter += 1;
    return jsonResponse({
      id: `terminal-${terminalSessionCounter}`,
      name: `Terminal ${terminalSessionCounter}`,
      workingDirectory: workspace.path,
    });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
    return jsonResponse(chatMessages);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/task-graph") {
    return jsonResponse(taskGraph);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
    return jsonResponse(secondChatMessages);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-2/task-graph") {
    return jsonResponse({
      chatId: "chat-2",
      createdAt: null,
      exists: false,
      tasks: [],
      updatedAt: null,
    });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/delete") {
    return jsonResponse({
      activeWorkspaceId: workspace.id,
      workspaces: [
        {
          ...workspace,
          chats: workspace.chats.filter((chat) => chat.id !== "chat-1"),
        },
      ],
    });
  }

  if (path === "/api/workspaces/workspace-1/chat/stream") {
    return chatStreamResponse();
  }

  return jsonResponse({ error: `Unhandled test route: ${url}` }, { status: 404 });
}

function chatStreamResponse() {
  const encoder = new TextEncoder();
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      activeChatStreamController = controller;
      controller.enqueue(
        encoder.encode(
          `data: ${JSON.stringify({
            type: "start",
            chatId: "chat-1",
            userMessageId: "message-user-stream",
            assistantMessageId: "message-assistant-stream",
            llmRequestId: "request-stream",
          })}\n\n`,
        ),
      );
    },
  });

  return new Response(stream, {
    headers: { "Content-Type": "text/event-stream" },
    status: 200,
  });
}

function enqueueChatStreamEvent(value: unknown) {
  if (!activeChatStreamController) {
    throw new Error("chat stream is not active");
  }

  const encoder = new TextEncoder();
  activeChatStreamController.enqueue(
    encoder.encode(`data: ${JSON.stringify(value)}\n\n`),
  );
}

function jsonResponse(value: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(value), {
    headers: { "Content-Type": "application/json" },
    status: 200,
    ...init,
  });
}
