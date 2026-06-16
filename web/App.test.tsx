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

const defaultComposerPlaceholder = "Ask Foco anything about Default...";
const sideProjectComposerPlaceholder = "Ask Foco anything about Side project...";

function chatSummary(
  id: string,
  title: string,
  createdAt: string,
  updatedAt: string,
  codeChangeStats = { additions: 0, deletions: 0 },
  activeRun: {
    chatId: string;
    lastSequence: number | null;
    runId: string;
    workspaceId: string;
  } | null = null,
) {
  return {
    activeRun,
    codeChangeStats,
    createdAt,
    id,
    title,
    updatedAt,
  };
}

const workspace = {
  chats: [
    chatSummary(
      "chat-1",
      "Tool run",
      "2026-06-05T10:00:00Z",
      "2026-06-05T10:05:00Z",
    ),
    chatSummary(
      "chat-2",
      "Second chat",
      "2026-06-05T11:00:00Z",
      "2026-06-05T11:05:00Z",
    ),
    ...Array.from({ length: 10 }, (_, index) => ({
      activeRun: null,
      codeChangeStats: { additions: 0, deletions: 0 },
      createdAt: `2026-06-04T${String(10 - index).padStart(2, "0")}:00:00Z`,
      id: `older-chat-${index + 1}`,
      title: `Older chat ${index + 1}`,
      updatedAt: `2026-06-04T${String(10 - index).padStart(2, "0")}:05:00Z`,
    })),
  ],
  commonCommands: [],
  id: "workspace-1",
  logoUrl: "/api/workspaces/workspace-1/logo?v=1",
  name: "Default",
  path: "C:\\Users\\fonla\\.foco\\workspace",
  pinned: false,
  terminalShell: "powershell",
};

const secondaryWorkspace = {
  chats: [
    chatSummary(
      "side-chat-1",
      "Side note",
      "2026-06-05T12:00:00Z",
      "2026-06-05T12:05:00Z",
    ),
  ],
  commonCommands: [],
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
      systemPromptName: "Default",
      thinkingLevel: null,
      warnings: [],
    },
  ],
  general: {
    autoStartEnabled: false,
    hookAuditEnabled: false,
    language: "en",
    llmRequestRetryCount: 3,
    maxLlmRequestRetryCount: 10,
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
    browserProbePort: 3210,
    ripgrep: {
      available: true,
      installDir: "C:\\Users\\fonla\\.foco\\bin",
      path: "C:\\Windows\\System32\\rg.exe",
    },
  },
  memory: {
    enabled: false,
    extractionMode: "manual",
    retrievalMode: "fts",
    extractionModelId: null,
    retrievalModelId: null,
    extractionModes: [
      { label: "Manual", value: "manual" },
      { label: "Pending review", value: "pending_review" },
      { label: "Automatic", value: "automatic" },
      { label: "Disabled", value: "disabled" },
    ],
    retrievalModes: [
      { label: "SQLite FTS", value: "fts" },
      { label: "Model matching", value: "llm" },
    ],
    retentionDays: null,
  },
  prompts: {
    defaultSystemPrompt: "You are Foco, a local coding agent.",
    extraText: "",
    files: [],
    systemPrompt: null,
    systemPrompts: [
      {
        content: "You are Foco, a local coding agent.",
        name: "Default",
      },
    ],
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
        canEnable: true,
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
      commonCommands: workspace.commonCommands,
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

const workspaceMemory = {
  ...activeMemory,
  fact: "Workspace scoped memory",
  id: "memory-workspace-1",
  scope: "workspace",
};

const chatMemory = {
  ...activeMemory,
  chatId: "chat-test",
  fact: "Chat scoped memory",
  id: "memory-chat-1",
  scope: "chat",
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
  content: "{\"note\":\"Manual source content\",\"details\":{\"origin\":\"test\"}}",
  createdAt: "2026-06-09T02:00:00Z",
  id: "memory-source-1",
  metadataJson: "{\"source\":\"manual\"}",
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
  errorMessage: "memory extraction provider failed",
  id: "memory-job-1",
  modelId: "gpt-test",
  scope: "chat",
  startedAt: "2026-06-09T02:09:30Z",
  status: "failed",
};

const aiStatistics = {
  page: 1,
  pageSize: 20,
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
      systemPromptName: "Default",
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

const emptyGitDiff = {
  diff: "",
  files: [],
  path: null,
  stagedDiff: "",
  status: "",
};

const generatedGitDiff = {
  diff: [
    "diff --git a/web/App.tsx b/web/App.tsx",
    "--- a/web/App.tsx",
    "+++ b/web/App.tsx",
    "@@ -1 +1,2 @@",
    "-old component",
    "+new component",
    "+extra line",
    "diff --git a/app/main.rs b/app/main.rs",
    "--- a/app/main.rs",
    "+++ b/app/main.rs",
    "@@ -4 +4 @@",
    "-old handler",
    "+new handler",
    "",
  ].join("\n"),
  files: [
    {
      indexStatus: " ",
      path: "web/App.tsx",
      worktreeStatus: "M",
    },
    {
      indexStatus: " ",
      path: "app/main.rs",
      worktreeStatus: "M",
    },
  ],
  path: null,
  stagedDiff: "",
  status: " M web/App.tsx\n M app/main.rs\n",
};

const chatMessages = {
  messages: [
    {
      content: "Please inspect README.",
      createdAt: "2026-06-10T08:00:00.000Z",
      extractedMemories: [],
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
      extractedMemories: [
        {
          chatId: "chat-1",
          fact: "Remember that README was inspected after completion.",
          id: "extracted-fact-1",
          kind: "episode",
          scope: "chat",
          status: "pending",
        },
      ],
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
            input: { path: "README.md", oldStr: "hello", newStr: "hello world" },
            isError: false,
            name: "edit_file",
            output: { bytes: 11, linesAdded: 1, linesRemoved: 1, path: "README.md" },
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
          input: { path: "README.md", oldStr: "hello", newStr: "hello world" },
          isError: false,
          name: "edit_file",
          output: { bytes: 11, linesAdded: 1, linesRemoved: 1, path: "README.md" },
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
      extractedMemories: [],
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
      extractedMemories: [],
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

const todoGraph = {
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
  tokenBreakdown: {
    bySource: [
      {
        compressibleTokens: 32000,
        optionalTokens: 32000,
        requiredTokens: 0,
        source: "persistedHistory",
        tokens: 32000,
      },
      {
        compressibleTokens: 120,
        optionalTokens: 120,
        requiredTokens: 0,
        source: "turnMemory",
        tokens: 120,
      },
      {
        compressibleTokens: 0,
        optionalTokens: 0,
        requiredTokens: 20220,
        source: "currentUser",
        tokens: 20220,
      },
    ],
    compressibleTokens: 32120,
    optionalTokens: 32120,
    requiredTokens: 20220,
  },
  usagePercent: 47,
  usedMessageTokens: 52340,
  willCompressOnNextSend: false,
};

const chatStatistics = {
  assistantMessageCount: 1,
  averageLatencyMs: 6200,
  chatId: "chat-1",
  codeChangeStats: { additions: 12, deletions: 3 },
  compression: {
    originalTokenCount: 9000,
    savedTokenCount: 6800,
    snapshotCount: 1,
    summaryTokenCount: 2200,
  },
  createdMemories: 2,
  failedRequests: 0,
  memoryReferences: 3,
  messageCount: 2,
  modelBreakdown: [
    { modelId: "gpt-test", requestCount: 2, totalTokens: 17600 },
  ],
  providerBreakdown: [
    {
      averageLatencyMs: 6200,
      failedCount: 0,
      providerId: "openai",
      requestCount: 2,
      successCount: 2,
      successRate: 1,
      totalTokens: 17600,
    },
  ],
  toolBreakdown: [{ callCount: 1, toolName: "read_file" }],
  toolMessageCount: 0,
  totalCacheReadTokens: 1200,
  totalCacheWriteTokens: 600,
  totalInputTokens: 12000,
  totalLatencyMs: 12400,
  totalOutputTokens: 5600,
  totalRequests: 2,
  totalTokens: 17600,
  userMessageCount: 1,
  workspaceId: "workspace-1",
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
let chatStreamControllers = new Map<
  string,
  ReadableStreamDefaultController<Uint8Array>
>();
let terminalSessionCounter = 0;
let chatStreamCounter = 0;
let workspaceGitDiffResponse = gitDiff;
let workspaceResponseWorkspaces = [workspace, secondaryWorkspace];

function savedGeneralSettings(init?: RequestInit) {
  const body =
    typeof init?.body === "string"
      ? (JSON.parse(init.body) as Record<string, unknown>)
      : {};

  return {
    ...settings,
    general: {
      ...settings.general,
      autoStartEnabled:
        typeof body.autoStartEnabled === "boolean"
          ? body.autoStartEnabled
          : settings.general.autoStartEnabled,
      hookAuditEnabled:
        typeof body.hookAuditEnabled === "boolean"
          ? body.hookAuditEnabled
          : settings.general.hookAuditEnabled,
      language:
        body.language === "zh-CN" || body.language === "en"
          ? body.language
          : settings.general.language,
      llmRequestRetryCount:
        typeof body.llmRequestRetryCount === "number"
          ? body.llmRequestRetryCount
          : settings.general.llmRequestRetryCount,
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
    chatStreamControllers = new Map();
    terminalSessionCounter = 0;
    chatStreamCounter = 0;
    workspaceGitDiffResponse = gitDiff;
    workspaceResponseWorkspaces = [workspace, secondaryWorkspace];
    window.history.replaceState(null, "", "/");
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-foco-theme");
    mermaidMock.initialize.mockClear();
    mermaidMock.render.mockClear();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
    vi.stubGlobal(
      "Image",
      class {
        onerror: ((event: Event) => void) | null = null;
        onload: ((event: Event) => void) | null = null;

        set src(_value: string) {
          window.setTimeout(() => {
            this.onload?.(new Event("load"));
          }, 0);
        }
      },
    );
    vi.stubGlobal("fetch", vi.fn(mockFetch));
  });

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    render(<App />);

    expect(await screen.findAllByText("Default")).not.toHaveLength(0);
    expect(screen.getAllByText("Tool run").length).toBeGreaterThan(0);
    expect(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
    ).toBeInTheDocument();

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
    expect(userBubble?.querySelector(".message-model-id")).toBeNull();
    expect(assistantBubble?.querySelector("time")).toHaveAttribute(
      "dateTime",
      "2026-06-10T08:00:02.000Z",
    );
    expect(
      assistantBubble?.querySelector(".message-model-id"),
    ).toHaveTextContent("gpt-test");
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
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context. Then answer.")).toBeInTheDocument();
    expect(screen.queryByText("Then answer.")).not.toBeInTheDocument();

    await userEvent.click(reasoningToggle);

    expect(reasoningToggle).toHaveAttribute("aria-expanded", "true");
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context.")).toBeInTheDocument();
    expect(screen.getByText("Then answer.")).toBeInTheDocument();
    expect(screen.getByText("edit_file")).toBeInTheDocument();
    expect(screen.getByText("+1")).toHaveClass("text-emerald-700");
    expect(screen.getByText("-1")).toHaveClass("text-rose-700");
    expect(screen.getByText("README.md")).toBeInTheDocument();
    expect(
      screen.getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"oldStr": "hello"')),
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(await screen.findByTestId("mermaid-svg", undefined, { timeout: 5000 })).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Channel: openai")).toBeInTheDocument();
    expect(screen.getByText("Total time: 2 s")).toBeInTheDocument();
    expect(screen.getByText("tokens/s: 20")).toBeInTheDocument();
    expect(screen.getByText("First token latency: 0.25 s")).toBeInTheDocument();
    const memoriesUsedLabel = within(assistantBubble!).getByText("Memories used");
    const finalAnswer = within(assistantBubble!).getByText("Done.");
    expect(
      memoriesUsedLabel.compareDocumentPosition(finalAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    await userEvent.click(memoriesUsedLabel);
    expect(screen.getByText("Use memory graph retrieval.")).toBeInTheDocument();
    const memoriesSavedLabel = within(assistantBubble!).getByText("Memories saved");
    expect(
      finalAnswer.compareDocumentPosition(memoriesSavedLabel) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    await userEvent.click(memoriesSavedLabel);
    expect(
      screen.getByText("Remember that README was inspected after completion."),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Side project" }),
    );
    expect(
      await screen.findByPlaceholderText(sideProjectComposerPlaceholder),
    ).toBeInTheDocument();
  });

  it("keeps the thinking duration visible when reply latency is unavailable", async () => {
    const messagesWithUnknownThinkingDuration = {
      ...chatMessages,
      messages: chatMessages.messages.map((message) =>
        message.id === "message-assistant"
          ? {
              ...message,
              metrics: message.metrics
                ? { ...message.metrics, totalLatencyMs: null }
                : message.metrics,
            }
          : message,
      ),
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...messagesWithUnknownThinkingDuration, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    render(<App />);

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const reasoningToggle = screen.getByRole("button", {
      name: "Expand thinking",
    });

    expect(within(reasoningToggle).getByText("n/a")).toBeInTheDocument();
  });

  it("stops reading after the stream end event without surfacing transport close errors", async () => {
    render(<App />);

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "finish cleanly",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Done without transport error.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 4,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    expect(await screen.findByText("Done without transport error.")).toBeInTheDocument();
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      activeChatStreamController?.error(new TypeError("network error"));
    });

    expect(screen.queryByText("network error")).not.toBeInTheDocument();
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
  });

  it("shows LLM reconnect and context compression badges in the assistant bubble", async () => {
    render(<App />);

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "recover and compact",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        reason: "provider stream failed",
        reasoning: null,
        text: "",
        toolCalls: [],
        type: "streamReset",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        snapshotId: "ctx-1",
        type: "contextCompression",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Recovered after compaction.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 4,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    const assistantText = await screen.findByText("Recovered after compaction.");
    const assistantRow = assistantText.closest(".message-row");
    expect(assistantRow).not.toBeNull();
    expect(
      within(assistantRow as HTMLElement).getByText("Reconnected"),
    ).toBeInTheDocument();
    expect(
      within(assistantRow as HTMLElement).getByText("Compressed"),
    ).toBeInTheDocument();
  });

  it("localizes completed tool status and uses success color", async () => {
    const zhSettings = {
      ...settings,
      general: {
        ...settings.general,
        language: "zh-CN",
      },
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/settings") {
        return jsonResponse(zhSettings);
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    render(<App />);

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const assistantBubble = screen
      .getByText("Done.")
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const completedPill = within(assistantBubble).getByText("已完成");
    expect(completedPill).toHaveClass("bg-emerald-50", "text-emerald-700");
    expect(within(assistantBubble).queryByText("completed")).not.toBeInTheDocument();
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

    const composer = await screen.findByPlaceholderText(defaultComposerPlaceholder);
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

  it("loads opened chat context usage without recomputing while drafting", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    const usage = await screen.findByRole("status", {
      name: "Context usage 47%",
    });
    expect(usage).toHaveTextContent("47%");

    const usageCallsBeforeDraft = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsBeforeDraft.length).toBeGreaterThan(0);
    const [, init] = usageCallsBeforeDraft.at(-1)!;
    expect(typeof init?.body).toBe("string");
    expect(JSON.parse(init?.body as string)).toMatchObject({
      assistantDraft: null,
      assistantDraftReasoning: null,
      chatId: "chat-1",
      draftMessage: null,
    });

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");

    const usageCallsAfterDraft = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsAfterDraft).toHaveLength(usageCallsBeforeDraft.length);
  });

  it("updates context usage from latest response usage during a stream", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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
    expect(usageCalls.length).toBeGreaterThan(0);
    const [, init] = usageCalls.at(-1)!;
    expect(typeof init?.body).toBe("string");
    expect(JSON.parse(init?.body as string)).toMatchObject({
      assistantDraftReasoning: null,
      chatId: "chat-1",
      draftMessage: null,
      latestResponseUsage: null,
    });
    expect(JSON.parse(init?.body as string).assistantDraft).not.toBe(
      "Partial answer.",
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

    expect(
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");
    const usageCallsAfterUsage = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    const [, usageInit] = usageCallsAfterUsage.at(-1)!;
    expect(typeof usageInit?.body).toBe("string");
    expect(JSON.parse(usageInit?.body as string)).toMatchObject({
      assistantDraft: "Partial answer.",
      assistantDraftReasoning: null,
      chatId: "chat-1",
      draftMessage: null,
      latestResponseUsage: {
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        inputTokens: 70000,
        outputTokens: 1000,
      },
    });

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

    await waitFor(() => {
      const usageCallsAfterComplete = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      );
      expect(usageCallsAfterComplete.length).toBeGreaterThan(
        usageCallCountBeforeComplete,
      );
      const [, completeUsageInit] = usageCallsAfterComplete.at(-1)!;
      expect(typeof completeUsageInit?.body).toBe("string");
      expect(JSON.parse(completeUsageInit?.body as string)).toMatchObject({
        assistantDraft: null,
        assistantDraftReasoning: null,
        chatId: "chat-1",
        draftMessage: null,
        latestResponseUsage: null,
      });
    });

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("coalesces streaming draft context usage refreshes", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    const usageCallCountBeforeDeltas = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    ).length;

    const originalSetTimeout = window.setTimeout.bind(window);
    const originalClearTimeout = window.clearTimeout.bind(window);
    let scheduledRefresh: (() => void) | null = null;
    type WindowSetTimeout = typeof window.setTimeout;
    type WindowSetTimeoutReturn = ReturnType<WindowSetTimeout>;
    const scheduledRefreshTimeoutHandle =
      Symbol("context-usage-refresh") as unknown as WindowSetTimeoutReturn;
    let scheduledRefreshTimeout: WindowSetTimeoutReturn | null = null;
    let scheduledRefreshCount = 0;
    let cancelledScheduledRefreshCount = 0;
    const setTimeoutSpy = vi
      .spyOn(window, "setTimeout")
      .mockImplementation((
        handler: Parameters<WindowSetTimeout>[0],
        timeout?: Parameters<WindowSetTimeout>[1],
        ...args: unknown[]
      ): WindowSetTimeoutReturn => {
        if (timeout === 1200 && typeof handler === "function") {
          scheduledRefreshCount += 1;
          scheduledRefresh = () => handler(...args);
          scheduledRefreshTimeout = scheduledRefreshTimeoutHandle;
          return scheduledRefreshTimeoutHandle;
        }

        return originalSetTimeout(
          handler as (...handlerArgs: unknown[]) => void,
          timeout,
          ...args,
        ) as unknown as WindowSetTimeoutReturn;
      });
    const clearTimeoutSpy = vi
      .spyOn(window, "clearTimeout")
      .mockImplementation((timeoutId) => {
        if (timeoutId === scheduledRefreshTimeout) {
          cancelledScheduledRefreshCount += 1;
        } else {
          originalClearTimeout(timeoutId);
        }
      });
    try {
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
      expect(setTimeoutSpy).toHaveBeenCalled();
      expect(scheduledRefreshCount).toBe(1);
      expect(cancelledScheduledRefreshCount).toBe(0);
      expect(scheduledRefresh).not.toBeNull();

      expect(
        fetchMock.mock.calls.filter(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/context-usage",
        ),
      ).toHaveLength(usageCallCountBeforeDeltas);

      await act(async () => {
        scheduledRefresh?.();
        await Promise.resolve();
      });

      const usageCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      );
      expect(usageCalls).toHaveLength(usageCallCountBeforeDeltas + 1);
      const [, init] = usageCalls.at(-1)!;
      expect(typeof init?.body).toBe("string");
      expect(JSON.parse(init?.body as string)).toMatchObject({
        assistantDraft: "Part one. Part two.",
        assistantDraftReasoning: null,
        chatId: "chat-1",
        draftMessage: null,
        latestResponseUsage: null,
      });
    } finally {
      setTimeoutSpy.mockRestore();
      clearTimeoutSpy.mockRestore();
    }

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("keeps context usage isolated between open chats", async () => {
    render(<App />);

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
      expect(chatStreamControllers.has("request-stream")).toBe(true),
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
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

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
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

    await act(async () => {
      chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("collapses streaming thinking once answer text starts", async () => {
    render(<App />);
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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
      activeChatStreamController?.close();
    });
  });

  it("sends guidance to the active run without ending the current stream", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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
    const interruptedAssistantRow = initialAnswer.closest(".message-row");
    expect(interruptedAssistantRow).not.toBeNull();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Model: gpt-test"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Channel: openai"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Total time: 2 s"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("tokens/s: 5"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        "First token latency: 0.25 s",
      ),
    ).toBeInTheDocument();

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
    const initialAnswerRow = initialAnswer.closest(".message-row");
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
      activeChatStreamController?.close();
    });
  });

  it("cancels the active run id after a later provider attempt starts", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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
            url === "/api/workspaces/workspace-1/chat/runs/request-stream/cancel",
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
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    await userEvent.click(screen.getByLabelText("Provider"));
    await userEvent.click(screen.getByRole("button", { name: "Provider: Anthropic" }));
    await userEvent.click(screen.getByLabelText("Thinking"));
    await userEvent.click(screen.getByRole("button", { name: "Thinking: High" }));
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = screen.getByText("next task");
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
      activeChatStreamController?.close();
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
      providerId: "anthropic",
      thinkingLevel: "high",
    });
    const effectiveQueuedMessage = screen.getByText("next task");
    const effectiveQueuedRow = effectiveQueuedMessage.closest(".message-row");
    expect(effectiveQueuedRow).not.toBeNull();
    expect(
      within(effectiveQueuedRow as HTMLElement).queryByText("Queued"),
    ).not.toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("withdraws a queued message before it is sent", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = screen.getByText("next task");
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
      activeChatStreamController?.close();
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
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = screen.getByText("next task");
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
      activeChatStreamController?.close();
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
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(chatStreamControllers.has("request-stream")).toBe(true),
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
      chatStreamControllers.get("request-stream")?.close();
      chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("starts a new chat instead of sending guidance while another chat is running", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(chatStreamControllers.has("request-stream")).toBe(true),
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
      chatId: null,
      message: "new chat task",
    });

    await act(async () => {
      chatStreamControllers.get("request-stream")?.close();
      chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("opens a new chat tab before the stream start event arrives", async () => {
    const fetchMock = vi.mocked(fetch);
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
    render(<App />);

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
    expect(
      within(tabList).getByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    const streamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(JSON.parse(String(streamCall?.[1]?.body))).toMatchObject({
      chatId: null,
      message: "memory-gated chat",
    });

    await act(async () => {
      delayedStreamController?.close();
    });
  });

  it("schedules a new workspace chat until the current workspace run finishes", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chat/stream") {
        const body =
          typeof init?.body === "string"
            ? (JSON.parse(init.body) as { chatId?: string | null; message?: string })
            : {};
        if (body.chatId === null && body.message === "Scheduled task") {
          workspaceResponseWorkspaces = [
            {
              ...workspace,
              chats: [
                ...workspace.chats,
                chatSummary(
                  "chat-scheduled",
                  "Scheduled task",
                  "2026-06-05T12:00:00Z",
                  "2026-06-05T12:00:00Z",
                ),
              ],
            },
            secondaryWorkspace,
          ];
          return chatStreamResponse("chat-scheduled");
        }
      }

      return mockFetch(input, init);
    });
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(chatStreamControllers.has("request-stream")).toBe(true),
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

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const scheduledHistoryTitle =
      within(workspaceList).getByText("Scheduled task");
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

    const tabListBeforeComplete = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabListBeforeComplete).getByRole("tab", { name: /Tool run/ }),
    );
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

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
      chatStreamControllers.get("request-stream")?.close();
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
      chatId: null,
      message: "Scheduled task",
    });

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream-2", {
        assistantMessageId: "message-assistant-stream-2",
        delta: "Scheduled answer.",
        type: "textDelta",
      });
    });

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    const activeMessageList = document.querySelector(".message-list");
    if (!(activeMessageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }
    expect(within(activeMessageList).getByText("Please inspect README.")).toBeInTheDocument();
    expect(within(activeMessageList).queryByText("Scheduled task")).not.toBeInTheDocument();
    expect(within(activeMessageList).queryByText("Scheduled answer.")).not.toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Scheduled task/ }));
    const scheduledMessageList = document.querySelector(".message-list");
    if (!(scheduledMessageList instanceof HTMLElement)) {
      throw new Error("Expected scheduled message list");
    }
    expect(await within(scheduledMessageList).findByText("Scheduled task")).toBeInTheDocument();
    expect(await within(scheduledMessageList).findByText("Scheduled answer.")).toBeInTheDocument();

    await act(async () => {
      chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("schedules a new workspace chat when Ctrl is held before clicking send", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(chatStreamControllers.has("request-stream")).toBe(true),
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

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const scheduledHistoryButton = within(workspaceList)
      .getByText("Held Ctrl scheduled task")
      .closest("button");
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
      chatStreamControllers.get("request-stream")?.close();
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
      chatStreamControllers.get("request-stream-2")?.close();
    });
  }, 10000);
  it("schedules a new workspace chat with Ctrl+Enter", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    const composer = screen.getByPlaceholderText(defaultComposerPlaceholder);
    await userEvent.type(composer, "Keyboard scheduled task");
    fireEvent.keyDown(composer, {
      ctrlKey: true,
      key: "Enter",
    });

    const streamCallsBeforeComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCallsBeforeComplete).toHaveLength(1);
    const scheduledMessageRow = screen
      .getAllByText("Keyboard scheduled task")
      .find((element) => element.closest(".message-row"))
      ?.closest(".message-row");
    expect(scheduledMessageRow).not.toBeNull();
    expect(
      within(scheduledMessageRow as HTMLElement).getByText("Queued"),
    ).toBeInTheDocument();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const firstScheduledHistoryButton = within(workspaceList)
      .getByText("Keyboard scheduled task")
      .closest("button");
    if (!firstScheduledHistoryButton) {
      throw new Error("Expected first scheduled chat history button");
    }
    expect(
      firstScheduledHistoryButton.querySelector(".session-status-dot"),
    ).toHaveClass("session-status-dot-scheduled");

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Click scheduled task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send message" }), {
      ctrlKey: true,
    });

    const secondScheduledHistoryButton = within(workspaceList)
      .getByText("Click scheduled task")
      .closest("button");
    if (!secondScheduledHistoryButton) {
      throw new Error("Expected second scheduled chat history button");
    }
    expect(
      secondScheduledHistoryButton.compareDocumentPosition(firstScheduledHistoryButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    await act(async () => {
      chatStreamControllers.get("request-stream")?.close();
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
      chatStreamControllers.get("request-stream-2")?.close();
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
      chatStreamControllers.get("request-stream-3")?.close();
    });
  });

  it("shows the queue tooltip while Ctrl is held over the send button", async () => {
    render(<App />);

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
    render(<App />);

    await screen.findByText("Tool run");
    const addAttachmentButton = screen.getByRole("button", { name: "Add attachment" });
    await waitFor(() => expect(addAttachmentButton).toBeEnabled());
    const fileInput = document.querySelector<HTMLInputElement>(
      'input[type="file"][multiple]',
    );
    expect(fileInput).not.toBeNull();
    await userEvent.upload(
      fileInput as HTMLInputElement,
      new File(["Hello"], "note.txt", { type: "text/plain" }),
    );
    expect(await screen.findByText("note.txt")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Provider"));
    await userEvent.click(screen.getByRole("button", { name: "Provider: Anthropic" }));
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "Review it");
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
        providerId: "anthropic",
      }),
    );


    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("waits for a streaming Mermaid fence to close before rendering", async () => {
    render(<App />);

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "diagram");
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

  it("shows retrieved memories as soon as the chat stream starts", async () => {
    render(<App />);

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "use memory",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await userEvent.click(await screen.findByText("Memories used"));
    expect(screen.getByText("Use memory before streaming.")).toBeInTheDocument();
    expect(screen.queryByText("Model: gpt-test")).not.toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("shows saved memories from the current chat stream", async () => {
    render(<App />);

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "remember this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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

  it("appends stream errors after already rendered assistant text", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "debug");
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
      activeChatStreamController?.close();
    });
  });

  it("shows hook blocking notifications in the active chat", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "danger");
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

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "Fresh task");
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

  it("keeps the active non-default workspace expanded after sending a new chat", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    const defaultToggle = await screen.findByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });
    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Side project" }),
    );

    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");

    await userEvent.type(
      screen.getByPlaceholderText(sideProjectComposerPlaceholder),
      "Side task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-2/chat/stream",
        ),
      ).toBe(true);
    });

    await act(async () => {
      activeChatStreamController?.close();
    });

    await waitFor(() => {
      expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
      expect(sideToggle).toHaveAttribute("aria-expanded", "true");
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

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Older chat 3")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();
    expect(screen.getByText("7 hidden chats")).toBeInTheDocument();
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

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
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

  it("keeps workspace chat dot running from workspace active run summary", async () => {
    workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...workspace.chats[0],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          },
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];

    render(<App />);

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    expect(historyButton.querySelector(".session-status-dot")).toHaveClass(
      "session-status-dot-running",
    );
  });

  it("clears stale workspace active run summary when loaded chat has no active run", async () => {
    workspaceResponseWorkspaces = [
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
    expect(statusDot()).toHaveClass("session-status-dot-running");

    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-open"),
    );
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
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
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

  it("shows persisted code line changes beside each workspace chat time", async () => {
    render(<App />);

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    expect(
      within(historyButton).queryByLabelText("Code changes +3 -2"),
    ).not.toBeInTheDocument();

    workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...workspace.chats[0],
            codeChangeStats: { additions: 3, deletions: 2 },
          },
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];
    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());
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
      activeChatStreamController?.close();
    });

    const updatedHistoryTitle = await within(workspaceList).findByText("Tool run");
    const updatedHistoryButton = updatedHistoryTitle.closest("button");
    if (!updatedHistoryButton) {
      throw new Error("Expected updated Tool run history item button");
    }

    expect(
      await within(updatedHistoryButton).findByLabelText("Code changes +3 -2"),
    ).toBeInTheDocument();
    expect(within(updatedHistoryButton).getByText("+3")).toHaveClass("chat-diff-add");
    expect(within(updatedHistoryButton).getByText("-2")).toHaveClass(
      "chat-diff-delete",
    );
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

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
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


  it("reattaches to an active run when loading chat messages", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    render(<App />);

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=-1",
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
    expect(screen.getByRole("status", { name: "Chat is running" })).toBeInTheDocument();

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
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    const [, usageInit] = usageCalls.at(-1)!;
    expect(typeof usageInit?.body).toBe("string");
    expect(JSON.parse(usageInit?.body as string)).toMatchObject({
      assistantDraft: "Still running.",
      assistantDraftReasoning: null,
      chatId: "chat-1",
      draftMessage: null,
      latestResponseUsage: {
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        inputTokens: 70000,
        outputTokens: 1000,
      },
    });

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

    await waitFor(() => {
      const usageCallsAfterComplete = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      );
      expect(usageCallsAfterComplete.length).toBeGreaterThan(
        usageCallCountBeforeComplete,
      );
      const [, completeUsageInit] = usageCallsAfterComplete.at(-1)!;
      expect(typeof completeUsageInit?.body).toBe("string");
      expect(JSON.parse(completeUsageInit?.body as string)).toMatchObject({
        assistantDraft: null,
        assistantDraftReasoning: null,
        chatId: "chat-1",
        draftMessage: null,
        latestResponseUsage: null,
      });
    });

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

    const choosePathButton = within(dialog).getByRole("button", {
      name: "Choose workspace path",
    });
    await waitFor(() => expect(choosePathButton).toBeEnabled());
    await userEvent.click(choosePathButton);

    await waitFor(() => {
      expect(pathInput).toHaveValue("C:/Users/fonla/Documents/Repos/NewWorkspace");
      expect(nameInput).toHaveValue("NewWorkspace");
    });

    await userEvent.upload(
      within(dialog).getByLabelText("Workspace icon file"),
      new File([new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])], "workspace-logo.png", {
        type: "image/png",
      }),
    );

    await waitFor(() => {
      expect(within(dialog).getByText("workspace-logo.png")).toBeInTheDocument();
    });

    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      const addWorkspaceCall = fetchMock.mock.calls.find(
        ([url, init]) => url === "/api/workspaces/add" && init?.method === "POST",
      );
      expect(addWorkspaceCall).toBeDefined();
      expect(JSON.parse(String(addWorkspaceCall?.[1]?.body))).toEqual({
        contentBase64: expect.any(String),
        name: "NewWorkspace",
        path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
      });
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

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));
    expect(screen.getByText("Prompt settings")).toBeInTheDocument();
    expect(screen.getByText("Prompt files")).toBeInTheDocument();
    expect(screen.getByText("No prompt files")).toBeInTheDocument();

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

  it("saves Windows auto start from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Start Foco when Windows starts",
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"autoStartEnabled":true'),
          method: "POST",
        }),
      );
    });
  });

  it("saves memory settings and manages manual memories", async () => {
    const fetchMock = vi.mocked(fetch);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);
    expect(await screen.findByText(memoryExtractionJob.errorMessage)).toBeInTheDocument();
    expect(screen.getAllByText("Preference").length).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));
    await waitFor(() => {
      const pageCall = [...fetchMock.mock.calls].find(([url]) => {
        const value = String(url);
        return (
          value.startsWith("/api/memory?") &&
          value.includes("page=2") &&
          value.includes("pageSize=20")
        );
      });
      expect(pageCall).toBeDefined();
    });

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.selectOptions(screen.getByLabelText("Extraction mode"), "automatic");
    await userEvent.selectOptions(screen.getByLabelText("Memory matching"), "llm");
    await userEvent.type(screen.getByLabelText("Retention days"), "30");
    await userEvent.selectOptions(screen.getByLabelText("Extraction model"), "gpt-test");
    await userEvent.selectOptions(screen.getByLabelText("Matching model"), "gpt-test");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/settings/memory",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        enabled: true,
        extractionMode: "automatic",
        retrievalMode: "llm",
        extractionModelId: "gpt-test",
        retrievalModelId: "gpt-test",
        retentionDays: 30,
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Create memory" }));
    const createDialog = await screen.findByRole("dialog", { name: "Create memory" });
    await userEvent.type(
      within(createDialog).getByLabelText("Memory fact"),
      "Remember local memory graph.",
    );
    await userEvent.click(within(createDialog).getByRole("button", { name: "Create memory" }));

    await waitFor(() => {
      const createCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/manual",
      );
      expect(createCall).toBeDefined();
      expect(JSON.parse(String(createCall?.[1]?.body))).toEqual({
        chatId: null,
        confidence: null,
        fact: "Remember local memory graph.",
        kind: "user_note",
        metadata: {},
        pinned: false,
        scope: "global",
        workspaceId: null,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory kind"), "preference");
    await waitFor(() => {
      const filteredListCall = [...fetchMock.mock.calls].find(([url]) =>
        String(url).startsWith("/api/memory?") && String(url).includes("kind=preference"),
      );
      expect(filteredListCall).toBeDefined();
    });

    const editButtons = screen.getAllByRole("button", { name: "Edit memory" });
    await userEvent.click(editButtons[0]);
    const editDialog = await screen.findByRole("dialog", { name: "Edit memory" });
    expect(within(editDialog).getByText("Memory details")).toBeInTheDocument();
    expect(await within(editDialog).findAllByText("Expand JSON")).toHaveLength(2);
    await userEvent.click(
      within(editDialog).getByRole("button", { name: "Expand JSON Source content" }),
    );
    expect(within(editDialog).getAllByLabelText("Source content")).toHaveLength(1);
    expect(within(editDialog).getAllByText(/"origin"/).length).toBeGreaterThan(0);
    const editFactInput = within(editDialog).getByLabelText("Memory fact");
    await userEvent.clear(editFactInput);
    await userEvent.type(editFactInput, "Updated memory preference.");
    await userEvent.click(within(editDialog).getByRole("button", { name: "Save memory" }));

    await waitFor(() => {
      const editCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/edit",
      );
      expect(editCall).toBeDefined();
      expect(JSON.parse(String(editCall?.[1]?.body))).toEqual({
        confidence: null,
        fact: "Updated memory preference.",
        kind: "preference",
        metadata: {},
        memoryId: activeMemory.id,
        pinned: true,
        scope: "global",
        sources: [
          {
            content: memorySource.content,
            id: memorySource.id,
            metadata: { source: "manual" },
            title: memorySource.title,
          },
        ],
        workspaceId: null,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory scope"), "workspace");
    expect(await screen.findByText(workspaceMemory.fact)).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Clear filtered workspace memories" }),
    );

    await waitFor(() => {
      const clearCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/clear",
      );
      expect(clearCall).toBeDefined();
      expect(JSON.parse(String(clearCall?.[1]?.body))).toEqual({
        chatId: null,
        kind: "preference",
        query: null,
        scope: "workspace",
        status: "active",
        workspaceId: workspace.id,
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Promote one level" }));

    await waitFor(() => {
      const promoteCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/promote",
      );
      expect(promoteCall).toBeDefined();
      expect(JSON.parse(String(promoteCall?.[1]?.body))).toEqual({
        memoryId: workspaceMemory.id,
        scope: "workspace",
        targetChatId: null,
        targetScope: "global",
        targetWorkspaceId: null,
        workspaceId: workspace.id,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory scope"), "global");
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    await userEvent.click(screen.getAllByRole("button", { name: "Delete memory" })[0]);
    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledWith("Delete memory confirmation");
      const forgetCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/forget",
      );
      expect(forgetCall).toBeDefined();
      expect(JSON.parse(String(forgetCall?.[1]?.body))).toEqual({
        memoryId: activeMemory.id,
        scope: "global",
        workspaceId: null,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory kind"), "");
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
    confirmSpy.mockRestore();
  }, 10000);

  it("keeps chat memory pagination requests tied to a chat id", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));
    expect(await screen.findByText("Memory settings")).toBeInTheDocument();

    const callCountBeforeChatScope = fetchMock.mock.calls.length;
    await userEvent.selectOptions(screen.getByLabelText("Memory scope"), "chat");

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Go to page 2" })).not.toBeInTheDocument();
    });
    const missingChatIdCall = fetchMock.mock.calls
      .slice(callCountBeforeChatScope)
      .find(([url]) => {
        const value = String(url);
        return value.startsWith("/api/memory?") && value.includes("scope=chat");
      });
    expect(missingChatIdCall).toBeUndefined();

    await userEvent.type(screen.getByLabelText("Chat ID"), "chat-test");
    expect(await screen.findByText(chatMemory.fact)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));

    await waitFor(() => {
      const pageCall = [...fetchMock.mock.calls].find(([url]) => {
        const value = String(url);
        return (
          value.startsWith("/api/memory?") &&
          value.includes("scope=chat") &&
          value.includes("chatId=chat-test") &&
          value.includes("page=2")
        );
      });
      expect(pageCall).toBeDefined();
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

  it("saves prompt files and extra prompt text", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const systemPromptInput = screen.getByLabelText("System prompt");
    expect(systemPromptInput).toHaveValue("You are Foco, a local coding agent.");
    await userEvent.clear(systemPromptInput);
    await userEvent.type(systemPromptInput, "Custom system prompt.");
    await userEvent.type(screen.getByPlaceholderText("Prompt name"), "Review");
    await userEvent.click(screen.getByRole("button", { name: "Add system prompt" }));
    expect(screen.getAllByText("Review").length).toBeGreaterThan(0);
    await userEvent.type(screen.getByLabelText("System prompt"), "Review as senior engineer.");
    await userEvent.type(
      screen.getByLabelText("Prompt file path"),
      "C:/Users/fonla/.codex/AGENTS.md",
    );
    await userEvent.click(screen.getByRole("button", { name: "Add prompt file" }));
    await userEvent.type(screen.getByLabelText("Extra prompt"), "Keep replies concise.");
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: JSON.stringify({
            extraText: "Keep replies concise.",
            files: ["C:/Users/fonla/.codex/AGENTS.md"],
            systemPrompts: [
              {
                content: "Custom system prompt.",
                name: "Default",
              },
              {
                name: "Review",
                content: "Review as senior engineer.",
              },
            ],
          }),
          method: "POST",
        }),
      );
    });
  });

  it("restores the default system prompt", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const systemPromptInput = screen.getByLabelText("System prompt");
    await userEvent.clear(systemPromptInput);
    await userEvent.type(systemPromptInput, "Custom system prompt.");
    await userEvent.click(screen.getByRole("button", { name: "Restore default system prompt" }));
    expect(systemPromptInput).toHaveValue("You are Foco, a local coding agent.");

    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: JSON.stringify({
            extraText: "",
            files: [],
            systemPrompts: [
              {
                content: "You are Foco, a local coding agent.",
                name: "Default",
              },
            ],
          }),
          method: "POST",
        }),
      );
    });
  });

  it("closes the model dialog from the backdrop without saving", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));

    expect(
      await screen.findByRole("form", { name: "Model configuration" }),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Close model configuration backdrop" }),
    );

    await waitFor(() => {
      expect(
        screen.queryByRole("form", { name: "Model configuration" }),
      ).not.toBeInTheDocument();
    });
    expect(fetchMock.mock.calls.some(([url]) => url === "/api/models/manual")).toBe(
      false,
    );
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
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/manual",
      expect.objectContaining({
        body: expect.stringContaining('"systemPromptName":"Default"'),
        method: "POST",
      }),
    );

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
            disabled: [],
            enabled: ["global:gitmemo"],
          }),
          method: "POST",
        }),
      );
    });
  }, 10000);

  it("toggles the context panel and opens the terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    render(<App />);

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Close context panel" }));
    expect(screen.queryByRole("tab", { name: "ToDo" })).not.toBeInTheDocument();
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

    render(<App />);

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
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        type: "todoGraphRefresh",
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

    const inlineDiffLine = await screen.findByText(/hello world/);
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

  it("deletes memories from the right panel memory tab", async () => {
    const fetchMock = vi.mocked(fetch);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<App />);

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
    render(<App />);

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
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: "Stats" }));
    expect(await screen.findByText("+12 / -3")).toBeInTheDocument();

    await user.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "edit the file",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        codeChangeStats: { additions: 5, deletions: 1 },
        type: "gitDiffRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByText("+17 / -4")).toBeInTheDocument();

    await act(async () => {
      activeChatStreamController?.close();
    });
  });

  it("opens the todo graph sidebar when a todo graph refresh arrives", async () => {
    render(<App />);

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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
      activeChatStreamController?.close();
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

    render(<App />);

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    await waitFor(() => expect(todoGraphRequests).toHaveLength(1));

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(activeChatStreamController).not.toBeNull());

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

  it("loads saved API request audit column settings", async () => {
    const { unmount } = render(<App />);

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
    render(<App />);

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    const reloadedTable = await screen.findByRole("table");
    expect(within(reloadedTable).queryByText("openai")).not.toBeInTheDocument();
    await userEvent.click(screen.getByText("Columns"));
    expect(screen.getByRole("checkbox", { name: "Provider" })).not.toBeChecked();
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
      workspaces: workspaceResponseWorkspaces,
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
        {
          chats: [],
          id: "new-workspace",
          logoUrl: "/api/workspaces/new-workspace/logo?v=1",
          name: "New Workspace",
          path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
          pinned: false,
          terminalShell: "powershell",
          commonCommands: [],
        },
        workspace,
        secondaryWorkspace,
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
        retrievalMode: "llm",
        extractionModelId: "gpt-test",
        retrievalModelId: "gpt-test",
        retentionDays: 30,
      },
    });
  }

  if (path === "/api/settings/prompts") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      extraText?: string;
      files?: string[];
      systemPrompts?: Array<{ content: string; name: string }>;
      systemPrompt?: string | null;
    };
    const systemPrompts =
      body.systemPrompts ??
      [
        {
          content: body.systemPrompt ?? settings.prompts.defaultSystemPrompt,
          name: "Default",
        },
      ];
    return jsonResponse({
      ...settings,
      prompts: {
        defaultSystemPrompt: settings.prompts.defaultSystemPrompt,
        extraText: body.extraText ?? "Keep replies concise.",
        files: body.files ?? ["C:/Users/fonla/.codex/AGENTS.md"],
        systemPrompt: null,
        systemPrompts,
      },
    });
  }

  if (path === "/api/memory") {
    const status = requestUrl.searchParams.get("status");
    const scope = requestUrl.searchParams.get("scope");
    const chatId = requestUrl.searchParams.get("chatId");
    const page = Number(requestUrl.searchParams.get("page") ?? "1");
    const pageSize = Number(requestUrl.searchParams.get("pageSize") ?? "20");
    const memories =
      status === "pending"
        ? [pendingMemory]
        : scope === "chat"
          ? [chatMemory]
        : scope === "workspace"
          ? [workspaceMemory]
          : [activeMemory];
    const totalCount =
      (scope === "global" && status !== "pending") || (scope === "chat" && chatId)
        ? 21
        : memories.length;
    return jsonResponse({
      extractionJobs: [memoryExtractionJob],
      memories,
      page,
      pageSize,
      totalCount,
      totalPages: totalCount ? Math.ceil(totalCount / pageSize) : 0,
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
    path === "/api/memory/clear" ||
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
      return jsonResponse(workspaceGitDiffResponse);
    }

    const file = workspaceGitDiffResponse.files.find(
      (summary) => summary.path === selectedPath,
    );
    const escapedPath = selectedPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const sectionMatch = workspaceGitDiffResponse.diff.match(
      new RegExp(`diff --git a/${escapedPath} b/${escapedPath}[\\s\\S]*?(?=diff --git a/|$)`),
    );

    return jsonResponse({
      ...workspaceGitDiffResponse,
      diff: sectionMatch?.[0] ?? "",
      files: workspaceGitDiffResponse.files,
      path: selectedPath,
      status: file
        ? `${file.indexStatus}${file.worktreeStatus} ${file.path}\n`
        : workspaceGitDiffResponse.status,
    });
  }

  if (path === "/api/workspaces/workspace-1/context-usage") {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as {
            chatId?: string | null;
            latestResponseUsage?: { inputTokens?: number | null };
          })
        : {};

    if (body.latestResponseUsage?.inputTokens === 70000) {
      return jsonResponse({
        ...contextUsage,
        usagePercent: 64,
        usedMessageTokens: 71000,
      });
    }

    return jsonResponse({
      ...contextUsage,
      usagePercent: body.chatId === "chat-2" ? 23 : contextUsage.usagePercent,
      usedMessageTokens:
        body.chatId === "chat-2" ? 25520 : contextUsage.usedMessageTokens,
    });
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
    return jsonResponse({ ...chatMessages, activeRun: null });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/todo-graph") {
    return jsonResponse(todoGraph);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/statistics") {
    return jsonResponse(chatStatistics);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
    return jsonResponse({ ...secondChatMessages, activeRun: null });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-2/todo-graph") {
    return jsonResponse({
      chatId: "chat-2",
      createdAt: null,
      exists: false,
      tasks: [],
      updatedAt: null,
    });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-2/statistics") {
    return jsonResponse({
      ...chatStatistics,
      chatId: "chat-2",
      messageCount: 2,
      totalTokens: 0,
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
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { chatId?: string | null })
        : {};
    return chatStreamResponse(body.chatId ?? "chat-1");
  }


  if (path === "/api/workspaces/workspace-1/chat/runs/request-stream/stream") {
    return chatStreamResponse("chat-1");
  }

  if (path === "/api/workspaces/workspace-1/chat/runs/request-stream/cancel") {
    return jsonResponse({ ok: true, runId: "request-stream" });
  }

  if (path === "/api/workspaces/workspace-1/chat/guidance") {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { message?: string })
        : {};
    return jsonResponse({
      content: body.message ?? "",
      id: "guidance-1",
      parts: [],
    });
  }

  if (path === "/api/workspaces/workspace-2/chat/stream") {
    return chatStreamResponse("side-chat-stream");
  }

  return jsonResponse({ error: `Unhandled test route: ${url}` }, { status: 404 });
}

function chatStreamResponse(chatId = "chat-1") {
  const encoder = new TextEncoder();
  chatStreamCounter += 1;
  const userMessageId =
    chatStreamCounter === 1
      ? "message-user-stream"
      : `message-user-stream-${chatStreamCounter}`;
  const assistantMessageId =
    chatStreamCounter === 1
      ? "message-assistant-stream"
      : `message-assistant-stream-${chatStreamCounter}`;
  const llmRequestId =
    chatStreamCounter === 1 ? "request-stream" : `request-stream-${chatStreamCounter}`;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      activeChatStreamController = controller;
      chatStreamControllers.set(llmRequestId, controller);
      controller.enqueue(
        encoder.encode(
          `data: ${JSON.stringify({
            type: "start",
            chatId,
            userMessageId,
            assistantMessageId,
            llmRequestId,
            memoriesUsed: [
              {
                chatId: null,
                fact: "Use memory before streaming.",
                id: "stream-fact-1",
                kind: "project_fact",
                pinned: false,
                scope: "workspace",
                source: "direct",
              },
            ],
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

function enqueueChatStreamEventForRun(runId: string, value: unknown) {
  const controller = chatStreamControllers.get(runId);
  if (!controller) {
    throw new Error(`chat stream is not active: ${runId}`);
  }

  const encoder = new TextEncoder();
  controller.enqueue(encoder.encode(`data: ${JSON.stringify(value)}\n\n`));
}

function jsonResponse(value: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(value), {
    headers: { "Content-Type": "application/json" },
    status: 200,
    ...init,
  });
}

type Deferred<T> = {
  promise: Promise<T>;
  reject: (reason?: unknown) => void;
  resolve: (value: T | PromiseLike<T>) => void;
};

function deferred<T>(): Deferred<T> {
  let reject: Deferred<T>["reject"] = () => undefined;
  let resolve: Deferred<T>["resolve"] = () => undefined;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  return { promise, reject, resolve };
}
