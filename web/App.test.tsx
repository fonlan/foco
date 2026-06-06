import { render, screen, waitFor, within } from "@testing-library/react";
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
  ],
  id: "workspace-1",
  name: "Default Workspace",
  path: "C:\\Users\\fonla\\.foco\\workspace",
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
        id: "gitmemo",
        name: "gitmemo",
        path: "C:\\Users\\fonla\\.agents\\skills\\gitmemo\\SKILL.md",
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
      workspaceName: "Default Workspace",
    },
  ],
  totalCount: 1,
  totalPages: 1,
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
        { text: "Need file context.", type: "reasoning" },
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
      reasoning: "Need file context.",
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

describe("App verification surfaces", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn(mockFetch));
  });

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    render(<App />);

    expect(await screen.findAllByText("Default Workspace")).not.toHaveLength(0);
    expect(screen.getByText("Tool run")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Tool run"));

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(screen.getByText("Need file context.")).toBeInTheDocument();
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

  it("marks the new chat row as selected after starting a workspace chat", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default Workspace" }),
    );

    expect(screen.getByRole("button", { name: "New chat" })).toHaveAttribute(
      "aria-current",
      "page",
    );
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
    await userEvent.type(screen.getByLabelText("Directories"), "{enter}.agents\\skills");
    await userEvent.click(screen.getByRole("button", { name: "Save skills" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skills/manual",
        expect.objectContaining({
          body: expect.stringContaining(".agents\\\\skills"),
          method: "POST",
        }),
      );
    });
  });

  it("opens the git diff panel and terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findAllByText("Default Workspace");
    await userEvent.click(screen.getByRole("button", { name: "Open git diff" }));

    expect(await screen.findByText("README.md")).toBeInTheDocument();
    expect(screen.getByText(/\+hello/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Open terminal" }));

    expect(await screen.findByText("connected")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/terminal/session",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("shows AI statistics and request details", async () => {
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Stats" }))[0]);

    expect(await screen.findByText("API statistics")).toBeInTheDocument();
    expect(screen.getByText("Request audit")).toBeInTheDocument();
    expect(screen.getByText("openai")).toBeInTheDocument();
    expect(screen.getByText("gpt-test")).toBeInTheDocument();

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

  if (path === "/api/settings") {
    return jsonResponse(settings);
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
    return jsonResponse({
      id: "terminal-1",
      name: "Terminal",
      workingDirectory: workspace.path,
    });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
    return jsonResponse(chatMessages);
  }

  return jsonResponse({ error: `Unhandled test route: ${url}` }, { status: 404 });
}

function jsonResponse(value: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(value), {
    headers: { "Content-Type": "application/json" },
    status: 200,
    ...init,
  });
}
