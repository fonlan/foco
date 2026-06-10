import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mermaidMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  render: vi.fn(async () => ({
    bindFunctions: vi.fn(),
    diagramType: "flowchart",
    svg: '<svg data-testid="mermaid-svg"><text>Rendered Mermaid</text></svg>',
  })),
}));

vi.mock("mermaid", () => ({
  default: mermaidMock,
}));

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
    ...Array.from({ length: 10 }, (_, index) => ({
      createdAt: `2026-06-04T${String(10 - index).padStart(2, "0")}:00:00Z`,
      id: `older-chat-${index + 1}`,
      title: `Older chat ${index + 1}`,
      updatedAt: `2026-06-04T${String(10 - index).padStart(2, "0")}:05:00Z`,
    })),
  ],
  id: "workspace-1",
  logoUrl: "/api/workspaces/workspace-1/logo?v=1",
  name: "Default",
  path: "C:\\Users\\fonla\\.foco\\workspace",
  pinned: false,
  terminalShell: "powershell",
};

const secondaryWorkspace = {
  chats: [
    {
      createdAt: "2026-06-05T12:00:00Z",
      id: "side-chat-1",
      title: "Side note",
      updatedAt: "2026-06-05T12:05:00Z",
    },
  ],
  id: "workspace-2",
  logoUrl: null,
  name: "Side project",
  path: "C:\\Users\\fonla\\Documents\\Repos\\SideProject",
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
      providerIds: ["openai", "anthropic"],
      supportsThinking: true,
      thinkingLevel: null,
      warnings: [],
    },
  ],
  general: {
    hookAuditEnabled: false,
    language: "en",
    supportedLanguages: [
      { id: "en", name: "English" },
      { id: "zh-CN", name: "简体中文" },
    ],
    supportedThemes: [
      { id: "light", name: "Light" },
      { id: "dark", name: "Dark" },
    ],
    theme: "light",
    webServer: {
      listenHost: "127.0.0.1",
      listenPort: 3210,
      passwordEnabled: false,
    },
  },
  nativeTools: {
    ripgrep: {
      available: true,
      installDir: "C:\\Users\\fonla\\.foco\\bin",
      path: "C:\\Windows\\System32\\rg.exe",
    },
  },
  memory: {
    enabled: false,
    extractionMode: "manual",
    extractionModelId: null,
    extractionModes: [
      { label: "Manual", value: "manual" },
      { label: "Pending review", value: "pending_review" },
      { label: "Automatic", value: "automatic" },
      { label: "Disabled", value: "disabled" },
    ],
    retentionDays: null,
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
      apiProxy: {
        enabled: false,
        proxyType: "http",
        supportedTypes: [
          { label: "HTTP", proxyType: "http" },
          { label: "SOCKS", proxyType: "socks" },
        ],
        url: "",
      },
      baseUrl: "https://api.openai.com/v1",
      enabled: true,
      hasApiKey: true,
      id: "openai",
      kind: "openai-chat",
      kindLabel: "OpenAI Chat",
      name: "OpenAI",
      warnings: [],
    },
    {
      apiProxy: {
        enabled: false,
        proxyType: "http",
        supportedTypes: [
          { label: "HTTP", proxyType: "http" },
          { label: "SOCKS", proxyType: "socks" },
        ],
        url: "",
      },
      baseUrl: "https://api.anthropic.test/v1",
      enabled: true,
      hasApiKey: true,
      id: "anthropic",
      kind: "openai-chat",
      kindLabel: "OpenAI Chat",
      name: "Anthropic",
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
      logoUrl: workspace.logoUrl,
      pinned: workspace.pinned,
      terminalShell: workspace.terminalShell,
    },
  ],
};

const activeMemory = {
  chatId: null,
  confidence: null,
  createdAt: "2026-06-09T02:00:00Z",
  expiresAt: null,
  fact: "Stored test preference",
  id: "memory-active-1",
  isLatest: true,
  kind: "preference",
  metadataJson: "{}",
  pinned: true,
  scope: "global",
  status: "active",
  updatedAt: "2026-06-09T02:05:00Z",
};

const pendingMemory = {
  ...activeMemory,
  fact: "Pending extracted memory",
  id: "memory-pending-1",
  pinned: false,
  status: "pending",
};

const memorySource = {
  chatId: null,
  content: "Manual source content",
  createdAt: "2026-06-09T02:00:00Z",
  id: "memory-source-1",
  metadataJson: "{}",
  scope: "global",
  sourceId: null,
  sourceType: "manual_note",
  title: "Manual memory",
  updatedAt: "2026-06-09T02:00:00Z",
};

const memoryExtractionJob = {
  chatId: "chat-test",
  completedAt: "2026-06-09T02:10:00Z",
  createdAt: "2026-06-09T02:09:00Z",
  errorMessage: "malformed memory extraction JSON",
  id: "memory-job-1",
  modelId: "gpt-test",
  scope: "chat",
  startedAt: "2026-06-09T02:09:30Z",
  status: "failed",
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
  summary: {
    averageLatencyMs: 2000,
    failedRequests: 1,
    modelBreakdown: [
      {
        modelId: "gpt-test",
        requestCount: 124,
        totalTokens: 17360,
      },
      {
        modelId: "gpt-alt",
        requestCount: 1,
        totalTokens: 200,
      },
    ],
    providerBreakdown: [
      {
        averageLatencyMs: 2000,
        failedCount: 1,
        providerId: "openai",
        requestCount: 125,
        successCount: 124,
        successRate: 0.992,
        totalTokens: 17560,
      },
    ],
    totalCacheReadTokens: 10,
    totalCacheWriteTokens: 2,
    totalInputTokens: 12500,
    totalOutputTokens: 5060,
    totalRequests: 125,
    totalTokens: 17560,
    trend: [
      {
        bucket: "2026-06-05",
        requestCount: 60,
        totalTokens: 8200,
      },
      {
        bucket: "2026-06-06",
        requestCount: 65,
        totalTokens: 9360,
      },
    ],
  },
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
        apiProxy: {
          enabled: true,
          proxyType: "socks",
          supportedTypes: [
            { label: "HTTP", proxyType: "http" },
            { label: "SOCKS", proxyType: "socks" },
          ],
          url: "socks5h://127.0.0.1:7891",
        },
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
  general: {
    ...settings,
    general: {
      ...settings.general,
      webServer: {
        ...settings.general.webServer,
        passwordEnabled: true,
      },
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
  diff: [
    "diff --git a/README.md b/README.md",
    "--- a/README.md",
    "+++ b/README.md",
    "@@ -1 +1 @@",
    "-hello",
    "+hello world",
    "diff --git a/new-note.txt b/new-note.txt",
    "--- /dev/null",
    "+++ b/new-note.txt",
    "@@ -0,0 +1 @@",
    "+new note",
    "diff --git a/asset.bin b/asset.bin",
    "Binary files a/asset.bin and b/asset.bin differ",
    "",
  ].join("\n"),
  files: [
    {
      indexStatus: "M",
      path: "README.md",
      worktreeStatus: "M",
    },
    {
      indexStatus: "?",
      path: "new-note.txt",
      worktreeStatus: "?",
    },
    {
      indexStatus: " ",
      path: "asset.bin",
      worktreeStatus: "M",
    },
  ],
  path: null,
  stagedDiff: "",
  status: " M README.md\n?? new-note.txt\n M asset.bin\n",
};

const chatMessages = {
  messages: [
    {
      content: "Please inspect README.",
      createdAt: "2026-06-10T08:00:00.000Z",
      id: "message-user",
      memoriesUsed: [],
      metrics: null,
      parts: [{ text: "Please inspect README.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    },
    {
      content: "Done.",
      createdAt: "2026-06-10T08:00:02.000Z",
      id: "message-assistant",
      memoriesUsed: [
        {
          chatId: null,
          fact: "Use memory graph retrieval.",
          id: "fact-1",
          kind: "project_fact",
          pinned: false,
          scope: "workspace",
          source: "direct",
        },
      ],
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
        {
          text: "Done.\n\n```mermaid\nflowchart TD\n  A --> B\n```",
          type: "text",
        },
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
      createdAt: "2026-06-10T09:00:00.000Z",
      id: "message-user-2",
      memoriesUsed: [],
      metrics: null,
      parts: [{ text: "Second question.", type: "text" }],
      reasoning: null,
      role: "user",
      toolCalls: [],
    },
    {
      content: "Second answer.",
      createdAt: "2026-06-10T09:00:02.000Z",
      id: "message-assistant-2",
      memoriesUsed: [],
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

const contextUsage = {
  availableMessageTokens: 110960,
  compressionTriggerPercent: 80,
  compressionTriggerTokens: 88768,
  memoryBudgetTokens: 13315,
  memoryContextTokens: 120,
  usagePercent: 47,
  usedMessageTokens: 52340,
  willCompressOnNextSend: false,
};

const hookSettings = {
  supportedEvents: [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "Elicitation",
    "ElicitationResult",
  ],
  unsupportedEvents: ["Setup"],
  global: {
    config: {
      PreToolUse: [
        {
          hooks: [
            {
              command: "node global-hook.js",
              enabled: true,
              type: "command",
            },
          ],
          matcher: "run_command",
        },
      ],
      disableAllHooks: false,
    },
    path: "C:\\Users\\fonla\\.foco\\config.json",
    source: "global",
    workspaceId: null,
  },
  workspace: {
    config: {
      UserPromptSubmit: [
        {
          hooks: [
            {
              enabled: true,
              statusMessage: "checking prompt",
              type: "http",
              url: "http://127.0.0.1:8787/hook",
            },
          ],
        },
      ],
      disableAllHooks: false,
    },
    path: "C:\\Users\\fonla\\.foco\\workspace\\.foco\\hooks.json",
    source: "workspace",
    workspaceId: "workspace-1",
  },
  effective: [
    {
      asyncHook: false,
      command: "node global-hook.js",
      event: "PreToolUse",
      handlerType: "command",
      matcher: "run_command",
      serverId: null,
      source: "global",
      statusMessage: null,
      toolName: null,
      url: null,
    },
    {
      asyncHook: false,
      command: null,
      event: "UserPromptSubmit",
      handlerType: "http",
      matcher: null,
      serverId: null,
      source: "workspace",
      statusMessage: "checking prompt",
      toolName: null,
      url: "http://127.0.0.1:8787/hook",
    },
  ],
  recentRuns: [
    {
      chatId: "chat-1",
      completedAt: "2026-06-08T10:00:01Z",
      event: "PreToolUse",
      exitCode: 0,
      handlerType: "command",
      hookSource: "global",
      id: "hook-run-1",
      runId: "run-1",
      startedAt: "2026-06-08T10:00:00Z",
      status: "succeeded",
      stderrPreview: null,
      stdoutPreview: "ok",
      toolCallId: "tool-1",
      workspaceId: "workspace-1",
    },
  ],
};

const hookRunDetail = {
  run: {
    ...hookSettings.recentRuns[0],
    input: { payload: { toolInput: { command: "git status" } } },
    output: { systemMessage: "ok" },
  },
};

const importedHooks = {
  config: { disableAllHooks: false },
  importedFiles: ["C:\\Users\\fonla\\.claude\\settings.json"],
  path: "C:\\Users\\fonla\\.foco\\config.json",
  saved: true,
  target: "global",
  validationErrors: [],
};

let activeChatStreamController: ReadableStreamDefaultController<Uint8Array> | null =
  null;
let terminalSessionCounter = 0;

function savedGeneralSettings(init?: RequestInit) {
  const body =
    typeof init?.body === "string"
      ? (JSON.parse(init.body) as Record<string, unknown>)
      : {};

  return {
    ...settings,
    general: {
      ...settings.general,
      hookAuditEnabled:
        typeof body.hookAuditEnabled === "boolean"
          ? body.hookAuditEnabled
          : settings.general.hookAuditEnabled,
      language:
        body.language === "zh-CN" || body.language === "en"
          ? body.language
          : settings.general.language,
      theme:
        body.theme === "dark" || body.theme === "light"
          ? body.theme
          : settings.general.theme,
      webServer: {
        ...settings.general.webServer,
        listenHost:
          typeof body.listenHost === "string"
            ? body.listenHost
            : settings.general.webServer.listenHost,
        listenPort:
          typeof body.listenPort === "number"
            ? body.listenPort
            : settings.general.webServer.listenPort,
        passwordEnabled:
          typeof body.password === "string" && body.password.length > 0
            ? true
            : settings.general.webServer.passwordEnabled,
      },
    },
  };
}

describe("App verification surfaces", () => {
  beforeEach(() => {
    activeChatStreamController = null;
    terminalSessionCounter = 0;
    window.history.replaceState(null, "", "/");
    document.documentElement.removeAttribute("data-foco-theme");
    mermaidMock.initialize.mockClear();
    mermaidMock.render.mockClear();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
    vi.stubGlobal("fetch", vi.fn(mockFetch));
  });

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    render(<App />);

    expect(await screen.findAllByText("Default")).not.toHaveLength(0);
    expect(screen.getAllByText("Tool run").length).toBeGreaterThan(0);

    await userEvent.click(screen.getByText("Tool run"));

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const userBubble = screen
      .getByText("Please inspect README.")
      .closest(".message-bubble") as HTMLElement | null;
    const assistantBubble = screen
      .getByText("Done.")
      .closest(".message-bubble") as HTMLElement | null;
    expect(userBubble).toHaveClass("message-bubble-user");
    expect(userBubble).not.toHaveClass("bg-teal-800", "text-white");
    expect(userBubble?.getAttribute("style")).toContain(
      "background-color: var(--foco-user-surface)",
    );
    expect(userBubble?.getAttribute("style")).toContain(
      "border-color: var(--foco-user-border)",
    );
    expect(assistantBubble).toHaveClass("message-bubble-assistant");
    expect(assistantBubble?.getAttribute("style")).toContain(
      "background-color: var(--foco-panel)",
    );
    expect(assistantBubble?.getAttribute("style")).toContain(
      "border-color: var(--foco-border)",
    );
    expect(userBubble?.querySelector("time")).toHaveAttribute(
      "dateTime",
      "2026-06-10T08:00:00.000Z",
    );
    expect(assistantBubble?.querySelector("time")).toHaveAttribute(
      "dateTime",
      "2026-06-10T08:00:02.000Z",
    );
    const userRow = userBubble?.closest(".message-row") as HTMLElement | null;
    const assistantRow = assistantBubble?.closest(
      ".message-row",
    ) as HTMLElement | null;
    if (!userBubble || !assistantBubble || !userRow || !assistantRow) {
      throw new Error("Expected message rows");
    }
    const userCopyButton = within(userBubble).getByRole(
      "button",
      { name: "Copy message" },
    );
    const assistantCopyButton = within(assistantBubble).getByRole(
      "button",
      { name: "Copy message" },
    );
    expect(userCopyButton.closest(".message-author-row")).toBe(
      userBubble?.querySelector(".message-author-row"),
    );
    expect(assistantCopyButton.closest(".message-author-row")).toBe(
      assistantBubble?.querySelector(".message-author-row"),
    );
    await userEvent.click(userCopyButton);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      "Please inspect README.",
    );
    expect(
      within(userRow).getByRole("button", { name: "Copied message" }),
    ).toBeInTheDocument();
    await userEvent.click(assistantCopyButton);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Done.");
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
    expect(await screen.findByTestId("mermaid-svg")).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Channel: openai")).toBeInTheDocument();
    expect(screen.getByText("Total time: 2 s")).toBeInTheDocument();
    expect(screen.getByText("tokens/s: 20")).toBeInTheDocument();
    expect(screen.getByText("First token latency: 0.25 s")).toBeInTheDocument();
    await userEvent.click(screen.getByText("Memories used"));
    expect(screen.getByText("Use memory graph retrieval.")).toBeInTheDocument();
  });

  it("opens a settings section from the URL and writes section changes back to the URL", async () => {
    window.history.replaceState(null, "", "/settings/models");
    render(<App />);

    expect(await screen.findByText("Model settings")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/models");

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "General" }));

    expect(await screen.findByText("General settings")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/general");
  });

  it("opens a chat from the URL and writes chat selection changes back to the URL", async () => {
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    render(<App />);

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/workspace-1/chat-1");

    await userEvent.click(screen.getByText("Second chat"));

    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/workspace-1/chat-2");
  });

  it("resizes the workspace sidebar from the panel splitter", async () => {
    render(<App />);

    const splitter = await screen.findByRole("separator", {
      name: "Resize workspace sidebar",
    });
    const sidebar = splitter.closest(".workspace-sidebar") as HTMLElement | null;
    const appShell = splitter.closest(".app-shell") as HTMLElement | null;

    if (!sidebar || !appShell) {
      throw new Error("Expected workspace sidebar splitter inside app shell");
    }

    expect(splitter).not.toHaveClass("hidden");
    expect(splitter).not.toHaveClass("lg:block");

    vi.spyOn(sidebar, "getBoundingClientRect").mockReturnValue({
      bottom: 800,
      height: 800,
      left: 48,
      right: 336,
      toJSON: () => ({}),
      top: 0,
      width: 288,
      x: 48,
      y: 0,
    } as DOMRect);

    fireEvent.pointerDown(splitter, { clientX: 336, pointerId: 1 });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("col-resize");
    });

    fireEvent.pointerMove(window, { clientX: 348 });

    await waitFor(() => {
      expect(appShell.style.getPropertyValue("--sidebar-width")).toBe("300px");
      expect(splitter).toHaveAttribute("aria-valuenow", "300");
    });

    fireEvent.pointerUp(window);

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
    });
  });

  it("keeps context panel resize from selecting panel text", async () => {
    render(<App />);

    const splitter = await screen.findByRole("separator", {
      name: "Resize context panel",
    });

    fireEvent.pointerDown(splitter, { clientX: 900, pointerId: 1 });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("col-resize");
      expect(document.body.style.userSelect).toBe("none");
    });

    fireEvent.pointerMove(window, { clientX: 880 });
    fireEvent.pointerUp(window);

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
      expect(document.body.style.userSelect).toBe("");
    });
  });

  it("prompts to install ripgrep when the search dependency is missing", async () => {
    const missingRipgrepSettings = {
      ...settings,
      nativeTools: {
        ripgrep: {
          available: false,
          installDir: "C:\\Users\\fonla\\.foco\\bin",
          path: null,
        },
      },
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/settings") {
        return jsonResponse(missingRipgrepSettings);
      }

      if (path === "/api/native/install-ripgrep") {
        expect(init?.method).toBe("POST");
        return jsonResponse({
          ripgrep: {
            available: true,
            installDir: "C:\\Users\\fonla\\.foco\\bin",
            path: "C:\\Users\\fonla\\.foco\\bin\\rg.exe",
          },
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    const dialog = await screen.findByRole("dialog", {
      name: "rg command was not found",
    });
    expect(within(dialog).getByText("C:\\Users\\fonla\\.foco\\bin")).toBeInTheDocument();

    await userEvent.click(within(dialog).getByRole("button", { name: "Download ripgrep" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/native/install-ripgrep",
        expect.objectContaining({ method: "POST" }),
      );
    });
    await waitFor(() => {
      expect(
        screen.queryByRole("dialog", { name: "rg command was not found" }),
      ).not.toBeInTheDocument();
    });
  });

  it("closes composer menus when clicking outside", async () => {
    const user = userEvent.setup();
    const { container } = render(<App />);

    const modelSummary = await screen.findByLabelText("Model");
    const thinkingSummary = await screen.findByLabelText("Thinking");
    const branchDetails = await waitFor(() => {
      const element = container.querySelector<HTMLDetailsElement>(
        ".composer-branch-select",
      );
      if (!element) {
        throw new Error("branch selector details was not rendered");
      }
      expect(element.tagName).toBe("DETAILS");
      return element;
    });
    const branchSummary = branchDetails.querySelector("summary");
    expect(branchSummary).not.toBeNull();

    await user.click(modelSummary);
    expect(modelSummary.closest("details")).toHaveAttribute("open");
    await user.click(document.body);
    expect(modelSummary.closest("details")).not.toHaveAttribute("open");

    await user.click(thinkingSummary);
    expect(thinkingSummary.closest("details")).toHaveAttribute("open");
    await user.click(document.body);
    expect(thinkingSummary.closest("details")).not.toHaveAttribute("open");

    await user.click(branchSummary as HTMLElement);
    expect(branchDetails).toHaveAttribute("open");
    await user.click(document.body);
    expect(branchDetails).not.toHaveAttribute("open");
  });

  it("keeps Shift+Enter in the composer as a newline", async () => {
    const fetchMock = vi.mocked(fetch);
    const user = userEvent.setup();
    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask Foco anything...");
    await user.click(composer);
    await user.keyboard("Line one{Shift>}{Enter}{/Shift}Line two");

    expect(composer).toHaveValue("Line one\nLine two");
    expect(
      fetchMock.mock.calls.some(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      ),
    ).toBe(false);
  });

  it("shows context usage beside the send button", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "continue");

    const usage = await screen.findByRole("status", {
      name: "Context usage 47%",
    });
    expect(usage).toHaveTextContent("47%");
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/context-usage",
      expect.objectContaining({
        body: JSON.stringify({
          attachments: [],
          chatId: "chat-1",
          draftMessage: "continue",
          modelId: "gpt-test",
          providerId: "openai",
          skillIds: null,
          thinkingLevel: null,
        }),
        method: "POST",
      }),
    );
  });

  it("adds native path attachments into the composer and sends them with the chat request", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findByText("Tool run");
    await userEvent.click(screen.getByRole("button", { name: "Add attachment" }));
    expect(await screen.findByText("note.txt")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Provider"));
    await userEvent.click(screen.getByRole("button", { name: "Provider: Anthropic" }));
    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "Review it");
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
            contentType: "text/plain",
            name: "note.txt",
            path: "C:/Users/fonla/Desktop/note.txt",
            sizeBytes: 5,
          }),
        ],
        message: "Review it",
        providerId: "anthropic",
      }),
    );
    expect(body.attachments[0]).not.toHaveProperty("contentBase64");

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("waits for a streaming Mermaid fence to close before rendering", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "diagram");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "```mermaid\nflowchart TD",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("flowchart TD")).toBeInTheDocument();
    expect(screen.queryByText("Mermaid diagram failed to render.")).not.toBeInTheDocument();
    expect(mermaidMock.render).not.toHaveBeenCalled();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "\n  A --> B\n```",
        type: "textDelta",
      });
    });

    expect(await screen.findByTestId("mermaid-svg")).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("appends stream errors after already rendered assistant text", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText("Ask Foco anything..."), "debug");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        message: "agent run exceeded 128 tool continuation rounds",
        type: "error",
      });
    });

    expect(await screen.findByText("Partial answer.")).toBeInTheDocument();
    expect(
      screen.getAllByText("agent run exceeded 128 tool continuation rounds").length,
    ).toBeGreaterThan(0);

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("shows hook blocking notifications in the active chat", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText("Ask Foco anything..."), "danger");
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

    expect(await screen.findByText("Hook blocked run_command: denied")).toBeInTheDocument();
    expect(
      screen.getByText("[PreToolUse] Hook blocked run_command: denied"),
    ).toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("expands a collapsed workspace without adding a placeholder chat row", async () => {
    render(<App />);

    const workspaceToggle = await screen.findByRole("button", { name: "Default" });
    await userEvent.click(workspaceToggle);
    expect(workspaceToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );

    expect(workspaceToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("button", { name: "New chat" })).not.toBeInTheDocument();
  });

  it("sends a workspace plus chat as a new chat request", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    expect(screen.queryByRole("button", { name: "New chat" })).not.toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "Fresh task");
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

    expect(JSON.parse(String(chatStreamCall?.[1]?.body))).toEqual(
      expect.objectContaining({
        chatId: null,
        message: "Fresh task",
      }),
    );

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("opens the selected chat workspace and collapses the previous workspace", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });
    await userEvent.click(sideToggle);
    await userEvent.click(await screen.findByText("Side note"));

    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByText("Side note").length).toBeGreaterThan(0);
  });

  it("allows workspace toggles after selecting a historical chat", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });

    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");

    await userEvent.click(sideToggle);
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByText("Side note")).toBeInTheDocument();
  });

  it("shows only the first 5 workspace chats until the menu item expands more", async () => {
    render(<App />);

    expect(await screen.findByText("Older chat 3")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();
    expect(screen.getByText("7 hidden chats")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Show 5 more chats in Default" }),
    );

    expect(screen.getByText("Older chat 4")).toBeInTheDocument();
    expect(screen.getByText("Older chat 8")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 9")).not.toBeInTheDocument();
    expect(screen.getByText("2 hidden chats")).toBeInTheDocument();
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

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list to exist");
    }
    messageList.scrollTop = 480;

    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Second chat" }),
    );

    expect(await screen.findByText("API overview")).toBeInTheDocument();
    expect(messageList.scrollTop).toBe(0);
  });

  it("reflects chat tab and running state in workspace chat dots", async () => {
    render(<App />);

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

    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-running"),
    );

    await act(async () => {
      activeChatStreamController?.close();
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
    render(<App />);

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
    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "continue");
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
      activeChatStreamController?.close();
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

  it("shows chat tab scroll controls only when tabs overflow and supports wheel scrolling", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(screen.getByText("Second chat"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const tabsContainer = tabList.parentElement;
    if (!tabsContainer) {
      throw new Error("Expected chat tab list to have a container");
    }
    expect(tabsContainer).toHaveClass("flex", "flex-nowrap", "overflow-hidden");
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs left" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs right" }),
    ).not.toBeInTheDocument();

    Object.defineProperties(tabsContainer, {
      clientWidth: { configurable: true, value: 360 },
    });
    Object.defineProperties(tabList, {
      clientWidth: { configurable: true, value: 300 },
      scrollWidth: { configurable: true, value: 340 },
    });
    fireEvent.scroll(tabList);
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs left" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs right" }),
    ).not.toBeInTheDocument();

    Object.defineProperties(tabList, {
      clientWidth: { configurable: true, value: 180 },
      scrollWidth: { configurable: true, value: 720 },
    });
    tabList.scrollLeft = 0;
    fireEvent.scroll(tabList);

    const leftButton = await screen.findByRole("button", {
      name: "Scroll chat tabs left",
    });
    const rightButton = screen.getByRole("button", {
      name: "Scroll chat tabs right",
    });
    expect(leftButton).toBeDisabled();
    expect(rightButton).toBeEnabled();

    fireEvent.wheel(tabList, { deltaY: 120 });
    expect(tabList.scrollLeft).toBe(120);
    await waitFor(() => expect(leftButton).toBeEnabled());
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

    await userEvent.type(screen.getByPlaceholderText("Ask Foco anything..."), "continue");
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

  it("uploads and clears a workspace icon in workspace settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Default" }),
    );

    const iconInput = await screen.findByLabelText("Workspace icon file");
    await userEvent.upload(
      iconInput,
      new File([new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])], "logo.png", {
        type: "image/png",
      }),
    );

    await waitFor(() => {
      const uploadCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/workspace-1/logo" && init?.method === "POST",
      );
      expect(uploadCall).toBeDefined();
      expect(JSON.parse(String(uploadCall?.[1]?.body))).toEqual({
        contentBase64: expect.any(String),
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Clear workspace icon" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/logo",
        expect.objectContaining({ method: "DELETE" }),
      );
    });
  });

  it("shows settings sections for providers, models, MCP servers, and skills", async () => {
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(screen.getByRole("navigation", { name: "Foco" })).toBeInTheDocument();
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    const settingsSidebar = settingsNav.closest("aside");
    expect(settingsSidebar).not.toBeNull();
    expect(within(settingsSidebar as HTMLElement).getByText("Settings")).toBeInTheDocument();
    expect(await screen.findByText("General settings")).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:3210")).toBeInTheDocument();
    expect(screen.getByText("Password is disabled")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    expect(screen.getByText("Configured providers")).toBeInTheDocument();
    const providersSection = screen.getByText("Configured providers").closest("section");
    expect(providersSection).not.toBeNull();
    expect(within(providersSection as HTMLElement).getByText("OpenAI")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Models" }));
    expect(screen.getByText("Model settings")).toBeInTheDocument();
    expect(screen.getByText("GPT Test")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "MCP" }));
    expect(screen.getByText("MCP servers")).toBeInTheDocument();
    expect(screen.getByText("CodeGraph")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));
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

  it("toggles the app theme from the nav rail", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(
      await screen.findByRole("button", { name: "Switch to dark theme" }),
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"theme":"dark"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(document.documentElement.dataset.focoTheme).toBe("dark");
    });
  });

  it("saves the app theme from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.selectOptions(
      await screen.findByRole("combobox", { name: /Theme/ }),
      "dark",
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"theme":"dark"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(document.documentElement.dataset.focoTheme).toBe("dark");
    });
  });

  it("saves memory settings and manages manual memories", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);
    expect(await screen.findByText(memorySource.content)).toBeInTheDocument();
    expect(await screen.findByText(memoryExtractionJob.errorMessage)).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.selectOptions(screen.getByLabelText("Extraction mode"), "automatic");
    await userEvent.type(screen.getByLabelText("Retention days"), "30");
    await userEvent.selectOptions(screen.getByLabelText("Extraction model"), "gpt-test");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/settings/memory",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        enabled: true,
        extractionMode: "automatic",
        extractionModelId: "gpt-test",
        retentionDays: 30,
      });
    });

    await userEvent.type(
      screen.getAllByLabelText("Memory fact")[0],
      "Remember local memory graph.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Create memory" }));

    await waitFor(() => {
      const createCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/manual",
      );
      expect(createCall).toBeDefined();
      expect(JSON.parse(String(createCall?.[1]?.body))).toEqual({
        chatId: null,
        fact: "Remember local memory graph.",
        kind: "user_note",
        pinned: false,
        scope: "global",
        workspaceId: null,
      });
    });

    const editFactInput = screen.getAllByLabelText("Memory fact")[1];
    await userEvent.clear(editFactInput);
    await userEvent.type(editFactInput, "Updated memory preference.");
    await userEvent.click(screen.getByRole("button", { name: "Save memory" }));

    await waitFor(() => {
      const editCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/edit",
      );
      expect(editCall).toBeDefined();
      expect(JSON.parse(String(editCall?.[1]?.body))).toEqual({
        fact: "Updated memory preference.",
        kind: "preference",
        memoryId: activeMemory.id,
        pinned: true,
        scope: "global",
        workspaceId: null,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory status"), "pending");
    expect((await screen.findAllByText(pendingMemory.fact)).length).toBeGreaterThan(0);
    await userEvent.click(screen.getByRole("button", { name: "Approve memory" }));

    await waitFor(() => {
      const statusCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/status",
      );
      expect(statusCall).toBeDefined();
      expect(JSON.parse(String(statusCall?.[1]?.body))).toEqual({
        memoryId: pendingMemory.id,
        scope: "global",
        status: "active",
        workspaceId: null,
      });
    });
  });

  it("shows translated hook settings and imports Claude hooks by target scope", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Hooks" }));

    expect(await screen.findByText("Hook settings")).toBeInTheDocument();
    expect(screen.getAllByText("Pre tool use").length).toBeGreaterThan(0);
    expect(screen.getAllByText("User prompt submit").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Command").length).toBeGreaterThan(0);
    expect(screen.getAllByText("HTTP").length).toBeGreaterThan(0);
    expect(screen.getByText("Record hook run logs")).toBeInTheDocument();
    expect(
      screen.getByText("Global import reads user Claude settings; workspace import reads the selected workspace."),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Import to global hooks" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks/import-claude",
        expect.objectContaining({
          body: JSON.stringify({ target: "global", workspaceId: null }),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Import to workspace hooks" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks/import-claude",
        expect.objectContaining({
          body: JSON.stringify({ target: "workspace", workspaceId: "workspace-1" }),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: /Pre tool use/ }));
    const dialog = await screen.findByRole("dialog", { name: "Hook run detail" });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByText("succeeded")).toBeInTheDocument();
  });

  it("logs in before loading the browser UI when authentication is enabled", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/auth/status") {
        return jsonResponse({ authenticated: false, enabled: true });
      }

      if (path === "/api/auth/login") {
        expect(init?.body).toBe(JSON.stringify({ password: "secret" }));
        return jsonResponse({ authenticated: true, enabled: true });
      }

      return mockFetch(input);
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    expect(await screen.findByText("Password required")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Password"), "secret");
    await userEvent.click(screen.getByRole("button", { name: "Log in" }));

    expect(await screen.findByText("Tool run")).toBeInTheDocument();
  });

  it("saves browser authentication password from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const passwordInput = await screen.findByLabelText("Authentication password");
    expect(passwordInput).toHaveAttribute("type", "password");
    expect(screen.queryByRole("button", { name: "Log out" })).not.toBeInTheDocument();

    await userEvent.type(passwordInput, "secret");
    await userEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(passwordInput).toHaveAttribute("type", "text");
    expect(screen.queryByRole("checkbox", { name: "Clear browser password" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear browser password" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"password":"secret"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(passwordInput).toHaveValue("********");
    });
    expect(screen.getByRole("button", { name: "Show password" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Log out" })).toBeInTheDocument();

    await userEvent.click(passwordInput);
    await userEvent.type(passwordInput, "replacement");
    await userEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(passwordInput).toHaveAttribute("type", "text");
    expect(passwordInput).toHaveValue("replacement");
  });

  it("saves provider, model, MCP server, and skill settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);

    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));
    await userEvent.type(screen.getByLabelText("Name"), "Test Provider");
    await userEvent.click(screen.getByRole("checkbox", { name: "Enable AI API proxy" }));
    await userEvent.selectOptions(screen.getByLabelText("Proxy type"), "socks");
    await userEvent.type(screen.getByLabelText("Proxy server"), "127.0.0.1:7891");
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
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/providers/manual",
      expect.objectContaining({
        body: expect.stringContaining(
          '"apiProxy":{"enabled":true,"proxyType":"socks","url":"127.0.0.1:7891"}',
        ),
        method: "POST",
      }),
    );

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

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "MCP" }));
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

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));
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

  it("toggles the context panel and opens the terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Close context panel" }));
    expect(screen.queryByRole("tab", { name: "Task" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open context panel" }));
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(await screen.findByText("README.md")).toBeInTheDocument();
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /README\.md M/ }));

    expect(await screen.findByText(/hello world/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /new-note\.txt U/ })).toBeInTheDocument();

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

  it("keeps task graph and git diff in separate context tabs", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText("Ask Foco anything..."), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        type: "taskGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByText("Inspect workspace changes")).toBeInTheDocument();
    expect(screen.getByText("README.md diff is visible")).toBeInTheDocument();
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(screen.getByText("Git diff")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /README\.md M/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /new-note\.txt U/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /asset\.bin M/ })).toBeInTheDocument();
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /README\.md M/ }));

    expect(await screen.findByText(/hello world/)).toBeInTheDocument();
    expect(screen.queryByText("Inspect workspace changes")).not.toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("shows an inline diff notice for binary changed files", async () => {
    render(<App />);

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));
    await userEvent.click(await screen.findByRole("button", { name: /asset\.bin M/ }));

    expect(
      await screen.findByText("Inline diff is unavailable for binary or non-text files."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Binary files a/asset.bin and b/asset.bin differ")).not.toBeInTheDocument();
  });

  it("opens the task graph sidebar when a task graph refresh arrives", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText("Ask Foco anything..."), "plan");
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
    expect(screen.queryByText("Git diff")).not.toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("shows AI statistics and request details", async () => {
    render(<App />);

    expect(await screen.findByText("API overview")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getAllByText("17.6K").length).toBeGreaterThan(0),
    );
    expect(screen.queryByText("Workspace shell is ready")).not.toBeInTheDocument();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);

    expect(await screen.findByText("API details")).toBeInTheDocument();
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

async function mockFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const url = typeof input === "string" ? input : input.toString();
  const path = url.startsWith("http://127.0.0.1") ? new URL(url).pathname : url.split("?")[0];
  const requestUrl = new URL(url, "http://127.0.0.1");

  if (path === "/api/auth/status") {
    return jsonResponse({ authenticated: true, enabled: false });
  }

  if (path === "/api/auth/login") {
    return jsonResponse({ authenticated: true, enabled: true });
  }

  if (path === "/api/auth/logout") {
    return jsonResponse({ authenticated: false, enabled: true });
  }

  if (path === "/api/workspaces") {
    return jsonResponse({
      activeWorkspaceId: workspace.id,
      workspaces: [workspace, secondaryWorkspace],
    });
  }

  if (path === "/api/native/select-directory") {
    return jsonResponse({ path: "C:/Users/fonla/Documents/Repos/NewWorkspace" });
  }

  if (path === "/api/native/select-files") {
    return jsonResponse({
      files: [
        {
          contentBase64: null,
          contentType: "text/plain",
          name: "note.txt",
          path: "C:/Users/fonla/Desktop/note.txt",
          sizeBytes: 5,
        },
      ],
    });
  }

  if (path === "/api/native/install-ripgrep") {
    return jsonResponse({
      ripgrep: {
        available: true,
        installDir: "C:\\Users\\fonla\\.foco\\bin",
        path: "C:\\Users\\fonla\\.foco\\bin\\rg.exe",
      },
    });
  }

  if (path === "/api/workspaces/add") {
    return jsonResponse({
      activeWorkspaceId: "new-workspace",
      workspaces: [
        workspace,
        secondaryWorkspace,
        {
          chats: [],
          id: "new-workspace",
          logoUrl: null,
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

  if (path === "/api/settings/general") {
    return jsonResponse(savedGeneralSettings(init));
  }

  if (path === "/api/settings/memory") {
    return jsonResponse({
      ...settings,
      memory: {
        ...settings.memory,
        enabled: true,
        extractionMode: "pending_review",
        extractionModelId: "gpt-test",
        retentionDays: 30,
      },
    });
  }

  if (path === "/api/memory") {
    const status = requestUrl.searchParams.get("status");
    return jsonResponse({
      extractionJobs: [memoryExtractionJob],
      memories: status === "pending" ? [pendingMemory] : [activeMemory],
    });
  }

  if (path === "/api/memory/sources") {
    return jsonResponse({ sources: [memorySource] });
  }

  if (
    path === "/api/memory/manual" ||
    path === "/api/memory/edit" ||
    path === "/api/memory/status" ||
    path === "/api/memory/forget" ||
    path === "/api/memory/promote"
  ) {
    return jsonResponse({ memory: activeMemory });
  }

  if (path === "/api/hooks") {
    return jsonResponse(hookSettings);
  }

  if (path === "/api/hooks/import-claude") {
    return jsonResponse(importedHooks);
  }

  if (path === "/api/hooks/test") {
    return jsonResponse({
      additionalContext: [],
      decisions: [],
      errors: [],
      hookSpecificOutputs: [],
      systemMessages: [],
    });
  }

  if (path === "/api/workspaces/manual" || path === "/api/workspaces/order") {
    return jsonResponse(savedSettings.workspace);
  }

  if (path === "/api/workspaces/workspace-1/logo") {
    return jsonResponse({
      ...settings,
      workspaces: [
        {
          ...settings.workspaces[0],
          logoUrl:
            init?.method === "DELETE"
              ? null
              : "/api/workspaces/workspace-1/logo?v=2",
        },
      ],
    });
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
    const selectedPath = requestUrl.searchParams.get("path");
    if (!selectedPath) {
      return jsonResponse(gitDiff);
    }

    const file = gitDiff.files.find((summary) => summary.path === selectedPath);
    const escapedPath = selectedPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const sectionMatch = gitDiff.diff.match(
      new RegExp(`diff --git a/${escapedPath} b/${escapedPath}[\\s\\S]*?(?=diff --git a/|$)`),
    );

    return jsonResponse({
      ...gitDiff,
      diff: sectionMatch?.[0] ?? "",
      files: gitDiff.files,
      path: selectedPath,
      status: file
        ? `${file.indexStatus}${file.worktreeStatus} ${file.path}\n`
        : gitDiff.status,
    });
  }

  if (path === "/api/workspaces/workspace-1/context-usage") {
    return jsonResponse(contextUsage);
  }

  if (path === "/api/workspaces/workspace-1/hooks/runs") {
    return jsonResponse({ runs: hookSettings.recentRuns });
  }

  if (path === "/api/workspaces/workspace-1/hooks/runs/hook-run-1") {
    return jsonResponse(hookRunDetail);
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
        secondaryWorkspace,
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
