import { fireEvent, render, type RenderOptions } from "@testing-library/react";
import { vi } from "vitest";

export const mermaidMock = {
  initialize: vi.fn(),
  render: vi.fn(async () => ({
    bindFunctions: vi.fn(),
    diagramType: "flowchart",
    svg: '<svg data-testid="mermaid-svg"><text>Rendered Mermaid</text></svg>',
  })),
};

vi.mock("mermaid", () => ({
  default: mermaidMock,
}));


export const defaultComposerPlaceholder = "Ask Foco anything about Default...";
export const sideProjectComposerPlaceholder = "Ask Foco anything about Side project...";

export function chatSummary(
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

export const workspace = {
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

export const secondaryWorkspace = {
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

export const settings = {
  agentTools: [
    "ask_question",
    "edit_file",
    "find_files",
    "read_file",
    "run_command",
    "search_text",
    "write_file",
  ],
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
    defaultTeamModeEnabled: false,
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
    dream: {
      enabled: false,
      autoEnabled: false,
      mode: "llm",
      modelId: null,
      workspaceIntervalDays: 7,
      globalIntervalDays: 30,
      createTranscriptChat: true,
      maxFactsPerRun: 200,
      maxChangesPerRun: 50,
      schedulerScanMinutes: 60,
    },
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
      requestOverrides: [],
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
      requestOverrides: [],
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

export const agentDefinitions = {
  agentDefinitions: [
    {
      allowedExecutionWorkspaceModes: ["shared", "isolated_worktree"],
      allowedTools: ["read_file", "send_message", "delegate_task"],
      description: "Coordinates the Agent team.",
      id: "agent-definition-coordinator",
      maxInstances: 1,
      modelId: "gpt-test",
      modelOptions: { maxOutputTokens: null, thinkingLevel: null },
      name: "Coordinator",
      permissions: {
        allowedAgentDefinitionIds: ["agent-definition-worker"],
        canCreateInstances: true,
        canDelegate: true,
      },
      providerId: "openai",
      revision: 1,
      systemPrompt: "Coordinate the team.",
    },
    {
      allowedExecutionWorkspaceModes: ["shared", "isolated_worktree"],
      allowedTools: ["read_file"],
      description: "Handles delegated implementation tasks.",
      id: "agent-definition-worker",
      maxInstances: 4,
      modelId: "gpt-test",
      modelOptions: { maxOutputTokens: null, thinkingLevel: null },
      name: "Worker",
      permissions: {
        allowedAgentDefinitionIds: [],
        canCreateInstances: false,
        canDelegate: false,
      },
      providerId: "openai",
      revision: 1,
      systemPrompt: "Do focused implementation work.",
    },
  ],
};

export const agentTeamSnapshot = {
  dependencies: [],
  events: [
    {
      attemptId: null,
      createdAt: "2026-06-05T10:00:00Z",
      eventType: "team_created",
      instanceId: "agent-instance-coordinator",
      messageId: null,
      payload: { coordinatorDefinitionId: "agent-definition-coordinator" },
      sequence: 1,
      taskId: null,
      teamId: "agent-team-1",
    },
  ],
  instances: [
    {
      contextGeneration: 0,
      createdAt: "2026-06-05T10:00:00Z",
      definitionId: "agent-definition-coordinator",
      definitionRevision: 1,
      definitionSnapshot: {
        ...agentDefinitions.agentDefinitions[0],
        systemPrompt: undefined,
      },
      executionRootPath: "C:\\Users\\fonla\\.foco\\workspace",
      executionWorkspaceMode: "shared",
      id: "agent-instance-coordinator",
      lastScheduledAt: null,
      nextTaskSequence: 2,
      role: "coordinator",
      status: "active",
      teamId: "agent-team-1",
      updatedAt: "2026-06-05T10:00:00Z",
      worktreeBaseRevision: null,
      worktreeBranch: null,
      worktreeStatus: null,
    },
    {
      contextGeneration: 0,
      createdAt: "2026-06-05T10:00:03Z",
      definitionId: "agent-definition-worker",
      definitionRevision: 1,
      definitionSnapshot: {
        ...agentDefinitions.agentDefinitions[1],
        systemPrompt: undefined,
      },
      executionRootPath:
        "C:\\Users\\fonla\\.foco\\workspace\\.foco\\agent-worktrees\\agent-instance-worker",
      executionWorkspaceMode: "isolated_worktree",
      id: "agent-instance-worker",
      lastScheduledAt: null,
      nextTaskSequence: 1,
      role: "worker",
      status: "active",
      teamId: "agent-team-1",
      updatedAt: "2026-06-05T10:00:03Z",
      worktreeBaseRevision: "base-revision",
      worktreeBranch: "foco/agent-instance-worker",
      worktreeStatus: "clean",
    },
  ],
  messages: [
    {
      consumedAt: null,
      content: "Worker, inspect the current task.",
      createdAt: "2026-06-05T10:00:01Z",
      id: "agent-message-1",
      kind: "notification",
      receiverInstanceId: "agent-instance-worker",
      relatedTaskId: "agent-task-1",
      replyToMessageId: null,
      senderInstanceId: "agent-instance-coordinator",
      sequence: 1,
      teamId: "agent-team-1",
    },
    {
      consumedAt: null,
      content: "Found the issue in the workspace notes.",
      createdAt: "2026-06-05T10:00:04Z",
      id: "agent-message-2",
      kind: "reply",
      receiverInstanceId: "agent-instance-coordinator",
      relatedTaskId: "agent-task-1",
      replyToMessageId: "agent-message-1",
      senderInstanceId: "agent-instance-worker",
      sequence: 1,
      teamId: "agent-team-1",
    },
  ],
  mutationLeaseOwners: [],
  observability: {
    cancelledTasks: 0,
    failedTasks: 0,
    failuresByType: [],
    interruptedTasks: 0,
    mutationLeaseWaitMs: { average: null, count: 0, max: null },
    queueLength: 0,
    queueWaitMs: { average: 1000, count: 1, max: 1000 },
    runDurationMs: { average: null, count: 0, max: null },
    schedulerLatencyMs: { average: 500, count: 1, max: 500 },
  },
  tasks: [
    {
      attempts: [],
      completedAt: "2026-06-05T10:00:05Z",
      createdAt: "2026-06-05T10:00:01Z",
      error: null,
      id: "agent-task-1",
      input: { message: "Inspect current task" },
      originInstanceId: "agent-instance-coordinator",
      ownerInstanceId: "agent-instance-worker",
      parentTaskId: "agent-task-root",
      result: { text: "Inspection complete." },
      sequence: 1,
      startedAt: "2026-06-05T10:00:02Z",
      status: "completed",
      teamId: "agent-team-1",
      updatedAt: "2026-06-05T10:00:05Z",
    },
  ],
  team: {
    chatId: "chat-1",
    coordinatorInstanceId: "agent-instance-coordinator",
    createdAt: "2026-06-05T10:00:00Z",
    id: "agent-team-1",
    maxConcurrentRuns: 1,
    status: "active",
    updatedAt: "2026-06-05T10:00:00Z",
  },
  workload: { queuedTasks: 0, runningTasks: 1, waitingTasks: 0 },
};

export const activeMemory = {
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

export const workspaceMemory = {
  ...activeMemory,
  fact: "Workspace scoped memory",
  id: "memory-workspace-1",
  scope: "workspace",
};

export const chatMemory = {
  ...activeMemory,
  chatId: "chat-test",
  fact: "Chat scoped memory",
  id: "memory-chat-1",
  scope: "chat",
};

export const pendingMemory = {
  ...activeMemory,
  fact: "Pending extracted memory",
  id: "memory-pending-1",
  pinned: false,
  status: "pending",
};

export const memorySource = {
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

export const memoryExtractionJob = {
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

export const aiStatistics = {
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

export const aiStatisticsDetail = {
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

export const savedSettings = {
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
        requestOverrides: [],
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

export const savedModelMetadata = {
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

export const gitDiff = {
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
  stagedFiles: [
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
  ],
  status: " M README.md\n?? new-note.txt\n M asset.bin\n",
};

export const emptyGitDiff = {
  diff: "",
  files: [],
  path: null,
  stagedDiff: "",
  stagedFiles: [],
  status: "",
};

export const generatedGitDiff = {
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
  stagedFiles: [],
  status: " M web/App.tsx\n M app/main.rs\n",
};

export const chatMessages = {
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

export const secondChatMessages = {
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

export const todoGraph = {
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

export const contextUsage = {
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

export const chatStatistics = {
  assistantMessageCount: 1,
  averageLatencyMs: 6200,
  chatId: "chat-1",
  codeChangeStats: { additions: 12, deletions: 3 },
  compression: {
    llmSnapshotCount: 0,
    originalTokenCount: 9000,
    ruleSnapshotCount: 1,
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

export const hookSettings = {
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

export const hookRunDetail = {
  run: {
    ...hookSettings.recentRuns[0],
    input: { payload: { toolInput: { command: "git status" } } },
    output: { systemMessage: "ok" },
  },
};


export const workspaceFilesResponse = {
  root: {
    children: [
      {
        children: [],
        childrenLoaded: true,
        hasChildren: false,
        kind: "file",
        name: "README.md",
        path: "README.md",
        sizeBytes: 512,
      },
      {
        children: [
          {
            children: [],
            childrenLoaded: true,
            hasChildren: false,
            kind: "file",
            name: "button.tsx",
            path: "src/components/button.tsx",
            sizeBytes: 512,
          },
        ],
        childrenLoaded: true,
        hasChildren: true,
        kind: "directory",
        name: "components",
        path: "src/components",
        sizeBytes: 0,
      },
      {
        children: [],
        childrenLoaded: false,
        hasChildren: true,
        kind: "directory",
        name: "pages",
        path: "src/pages",
        sizeBytes: 0,
      },
      {
        children: [],
        childrenLoaded: true,
        hasChildren: false,
        kind: "file",
        name: "main.ts",
        path: "src/main.ts",
        sizeBytes: 1024,
      },
    ],
    childrenLoaded: true,
    hasChildren: true,
    kind: "directory",
    name: "workspace",
    path: "",
    sizeBytes: 0,
  },
};

export const markdownFileContent = [
  "# Preview title",
  "",
  "![Remote asset](https://example.com/asset.png)",
  "",
  "![Inline asset](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=)",
  "",
  "Inline math $E=mc^2$.",
  "",
  "$$\\int_0^1 x^2 dx$$",
  "",
  "```mermaid",
  "flowchart TD",
  "  A --> B",
  "```",
].join("\n");

export const importedHooks = {
  config: { disableAllHooks: false },
  importedFiles: ["C:\\Users\\fonla\\.claude\\settings.json"],
  path: "C:\\Users\\fonla\\.foco\\config.json",
  saved: true,
  target: "global",
  validationErrors: [],
};

type ScheduledTaskFixture = {
  action: Record<string, unknown>;
  createdAt: string;
  description: string | null;
  id: string;
  lastRunAt: string | null;
  metadata: Record<string, unknown>;
  nextRunAt: string | null;
  schedule: Record<string, unknown>;
  status: string;
  title: string;
  updatedAt: string;
  usage: Record<string, number | null>;
  workspaceId: string;
  workspaceName: string;
};

const emptyScheduledTaskUsage = {
  averageLatencyMs: null,
  failedRequests: 0,
  totalCacheReadTokens: 0,
  totalCacheWriteTokens: 0,
  totalInputTokens: 0,
  totalLatencyMs: 0,
  totalOutputTokens: 0,
  totalRequests: 0,
  totalTokens: 0,
};

export const scheduledTasks: { tasks: ScheduledTaskFixture[] } = {
  tasks: [
    {
      action: {
        prompt: "Summarize workspace changes.",
        session_mode: "create_new_chat",
        type: "agent_prompt",
      },
      createdAt: "2026-06-22T08:00:00Z",
      description: "Daily repository summary",
      id: "scheduled-task-1",
      lastRunAt: "2026-06-22T08:00:00Z",
      metadata: {
        concurrencyPolicy: "skip_if_running",
        misfirePolicy: "catch_up_once",
        workspaceId: "workspace-1",
      },
      nextRunAt: "2026-06-23T08:00:00Z",
      schedule: {
        every_seconds: 86400,
        start_at: "2026-06-22T08:00:00Z",
        type: "interval",
      },
      status: "enabled",
      title: "Daily workspace summary",
      updatedAt: "2026-06-22T08:00:00Z",
      usage: {
        ...emptyScheduledTaskUsage,
        averageLatencyMs: 2000,
        failedRequests: 0,
        totalInputTokens: 100,
        totalLatencyMs: 2000,
        totalOutputTokens: 20,
        totalRequests: 1,
        totalTokens: 120,
      },
      workspaceId: "workspace-1",
      workspaceName: "Default",
    },
  ],
};

type ScheduledTaskRunFixture = {
  activeRunId: string | null;
  agentAttemptId: string | null;
  agentTaskId: string | null;
  agentTeamId: string | null;
  assistantMessageId: string | null;
  chatId: string | null;
  completedAt: string | null;
  createdAt: string;
  errorMessage: string | null;
  id: string;
  metadata: Record<string, unknown>;
  outputSummary: string | null;
  queuedAt: string | null;
  scheduledAt: string;
  startedAt: string | null;
  status: string;
  taskId: string;
  triggerReason: string;
  updatedAt: string;
  userMessageId: string | null;
  workspaceId: string;
};

export const scheduledTaskRunsByTaskId: Record<string, ScheduledTaskRunFixture[]> = {
  "scheduled-task-1": [
    {
      activeRunId: "agent-task-scheduled-1",
      agentAttemptId: null,
      agentTaskId: "agent-task-scheduled-1",
      agentTeamId: "agent-team-1",
      assistantMessageId: "message-assistant-1",
      chatId: "chat-1",
      completedAt: "2026-06-22T08:02:00Z",
      createdAt: "2026-06-22T08:00:00Z",
      errorMessage: null,
      id: "scheduled-run-1",
      metadata: {},
      outputSummary: null,
      queuedAt: "2026-06-22T08:00:01Z",
      scheduledAt: "2026-06-22T08:00:00Z",
      startedAt: "2026-06-22T08:00:03Z",
      status: "succeeded",
      taskId: "scheduled-task-1",
      triggerReason: "scheduled",
      updatedAt: "2026-06-22T08:02:00Z",
      userMessageId: "message-user-1",
      workspaceId: "workspace-1",
    },
  ],
};

export const appTestState: {
  activeChatStreamController: ReadableStreamDefaultController<Uint8Array> | null;
  chatStreamControllers: Map<string, ReadableStreamDefaultController<Uint8Array>>;
  terminalSessionCounter: number;
  chatStreamCounter: number;
  chatQueueCounter: number;
  scheduledTaskRunsByTaskId: Record<string, ScheduledTaskRunFixture[]>;
  scheduledTasksResponse: typeof scheduledTasks;
  workspaceGitDiffResponse: typeof gitDiff;
  workspaceResponseWorkspaces: unknown[];
} = {
  activeChatStreamController: null,
  chatStreamControllers: new Map<string, ReadableStreamDefaultController<Uint8Array>>(),
  terminalSessionCounter: 0,
  chatStreamCounter: 0,
  chatQueueCounter: 0,
  scheduledTaskRunsByTaskId,
  scheduledTasksResponse: scheduledTasks,
  workspaceGitDiffResponse: gitDiff,
  workspaceResponseWorkspaces: [workspace, secondaryWorkspace],
};

export function savedGeneralSettings(init?: RequestInit) {
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
      defaultTeamModeEnabled:
        typeof body.defaultTeamModeEnabled === "boolean"
          ? body.defaultTeamModeEnabled
          : settings.general.defaultTeamModeEnabled,
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

import { App } from "../App";

export function resetAppTestEnvironment() {
  appTestState.activeChatStreamController = null;
  appTestState.chatStreamControllers = new Map();
  appTestState.terminalSessionCounter = 0;
  appTestState.chatStreamCounter = 0;
  appTestState.chatQueueCounter = 0;
  appTestState.scheduledTaskRunsByTaskId = {
    "scheduled-task-1": [...scheduledTaskRunsByTaskId["scheduled-task-1"]],
  };
  appTestState.scheduledTasksResponse = scheduledTasks;
  appTestState.workspaceGitDiffResponse = gitDiff;
  appTestState.workspaceResponseWorkspaces = [workspace, secondaryWorkspace];
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
}

export function renderApp(options?: RenderOptions) {
  return render(<App />, options);
}

export function changeInput(element: Element, value: string) {
  fireEvent.change(element, { target: { value } });
}

export async function mockFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
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
      workspaces: appTestState.workspaceResponseWorkspaces,
    });
  }

  if (path === "/api/scheduled-tasks") {
    return jsonResponse(appTestState.scheduledTasksResponse);
  }

  if (path === "/api/scheduled-tasks/preview-next-run") {
    return jsonResponse({
      nextRunAt: "2026-06-22T09:00:00.000Z",
      nextRuns: [
        "2026-06-22T09:00:00.000Z",
        "2026-06-23T09:00:00.000Z",
        "2026-06-24T09:00:00.000Z",
        "2026-06-25T09:00:00.000Z",
        "2026-06-26T09:00:00.000Z",
      ],
    });
  }

  const scheduledRunsMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks\/([^/]+)\/runs$/,
  );
  if (scheduledRunsMatch) {
    const taskId = decodeURIComponent(scheduledRunsMatch[2] ?? "");
    return jsonResponse({
      runs: appTestState.scheduledTaskRunsByTaskId[taskId] ?? [],
    });
  }

  const scheduledRunNowMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks\/([^/]+)\/run-now$/,
  );
  if (scheduledRunNowMatch) {
    const workspaceId = decodeURIComponent(scheduledRunNowMatch[1] ?? "");
    const taskId = decodeURIComponent(scheduledRunNowMatch[2] ?? "");
    const now = "2026-06-22T09:00:00Z";
    const run: ScheduledTaskRunFixture = {
      activeRunId: "agent-task-manual-1",
      agentAttemptId: null,
      agentTaskId: "agent-task-manual-1",
      agentTeamId: "agent-team-1",
      assistantMessageId: "message-assistant-1",
      chatId: "chat-1",
      completedAt: null,
      createdAt: now,
      errorMessage: null,
      id: `scheduled-run-${(appTestState.scheduledTaskRunsByTaskId[taskId] ?? []).length + 1}`,
      metadata: {},
      outputSummary: null,
      queuedAt: now,
      scheduledAt: now,
      startedAt: null,
      status: "queued",
      taskId,
      triggerReason: "manual",
      updatedAt: now,
      userMessageId: "message-user-1",
      workspaceId,
    };
    appTestState.scheduledTaskRunsByTaskId = {
      ...appTestState.scheduledTaskRunsByTaskId,
      [taskId]: [run, ...(appTestState.scheduledTaskRunsByTaskId[taskId] ?? [])],
    };
    appTestState.scheduledTasksResponse = {
      tasks: appTestState.scheduledTasksResponse.tasks.map((task) =>
        task.id === taskId ? { ...task, lastRunAt: now, updatedAt: now } : task,
      ),
    };
    return jsonResponse({ run });
  }

  const scheduledTaskActionMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks\/([^/]+)\/(pause|resume|archive)$/,
  );
  if (scheduledTaskActionMatch) {
    const taskId = decodeURIComponent(scheduledTaskActionMatch[2] ?? "");
    const action = scheduledTaskActionMatch[3];
    const status =
      action === "pause" ? "paused" : action === "resume" ? "enabled" : "archived";
    let updatedTask = appTestState.scheduledTasksResponse.tasks.find(
      (task) => task.id === taskId,
    );
    if (updatedTask) {
      updatedTask = {
        ...updatedTask,
        nextRunAt: status === "enabled" ? updatedTask.nextRunAt : null,
        status,
        updatedAt: "2026-06-22T09:00:00Z",
      };
      appTestState.scheduledTasksResponse = {
        tasks: appTestState.scheduledTasksResponse.tasks.map((task) =>
          task.id === taskId ? updatedTask! : task,
        ),
      };
    }
    return jsonResponse({ task: updatedTask });
  }

  const scheduledTaskDuplicateMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks\/([^/]+)\/duplicate$/,
  );
  if (scheduledTaskDuplicateMatch) {
    const taskId = decodeURIComponent(scheduledTaskDuplicateMatch[2] ?? "");
    const existingTask = appTestState.scheduledTasksResponse.tasks.find(
      (task) => task.id === taskId,
    );
    if (!existingTask) {
      return jsonResponse({ message: "scheduled task was not found" }, { status: 404 });
    }
    const now = "2026-06-22T09:00:00Z";
    const task = {
      ...existingTask,
      createdAt: now,
      id: `scheduled-task-${appTestState.scheduledTasksResponse.tasks.length + 1}`,
      nextRunAt: null,
      status: "paused",
      title: `${existingTask.title} copy`,
      updatedAt: now,
    };
    appTestState.scheduledTasksResponse = {
      tasks: [task, ...appTestState.scheduledTasksResponse.tasks],
    };
    appTestState.scheduledTaskRunsByTaskId = {
      ...appTestState.scheduledTaskRunsByTaskId,
      [task.id]: [],
    };
    return jsonResponse({ task });
  }

  const scheduledTaskItemMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks\/([^/]+)$/,
  );
  if (scheduledTaskItemMatch) {
    const taskId = decodeURIComponent(scheduledTaskItemMatch[2] ?? "");
    const existingTask = appTestState.scheduledTasksResponse.tasks.find(
      (task) => task.id === taskId,
    );
    if (init?.method === "DELETE") {
      appTestState.scheduledTasksResponse = {
        tasks: appTestState.scheduledTasksResponse.tasks.filter(
          (task) => task.id !== taskId,
        ),
      };
      return jsonResponse({ task: existingTask });
    }
    if (init?.method === "PATCH" && existingTask) {
      const body = JSON.parse(String(init.body ?? "{}")) as Record<string, unknown>;
      const updatedTask = {
        ...existingTask,
        action:
          (body.action as Record<string, unknown> | undefined) ??
          existingTask.action,
        description:
          "description" in body
            ? (body.description as string | null)
            : existingTask.description,
        metadata: {
          ...(existingTask.metadata as Record<string, unknown>),
          concurrencyPolicy:
            body.concurrencyPolicy ??
            (existingTask.metadata as Record<string, unknown>).concurrencyPolicy,
          misfirePolicy:
            body.misfirePolicy ??
            (existingTask.metadata as Record<string, unknown>).misfirePolicy,
        },
        schedule:
          (body.schedule as Record<string, unknown> | undefined) ??
          existingTask.schedule,
        status: (body.status as string | undefined) ?? existingTask.status,
        title: (body.title as string | undefined) ?? existingTask.title,
        updatedAt: "2026-06-22T09:00:00Z",
      };
      appTestState.scheduledTasksResponse = {
        tasks: appTestState.scheduledTasksResponse.tasks.map((task) =>
          task.id === taskId ? updatedTask : task,
        ),
      };
      return jsonResponse({ task: updatedTask });
    }
  }

  const scheduledTaskCreateMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/scheduled-tasks$/,
  );
  if (scheduledTaskCreateMatch && init?.method === "POST") {
    const workspaceId = decodeURIComponent(scheduledTaskCreateMatch[1] ?? "");
    const body = JSON.parse(String(init.body ?? "{}")) as Record<string, unknown>;
    const workspaceName =
      (
        appTestState.workspaceResponseWorkspaces.find(
          (item) =>
            typeof item === "object" &&
            item !== null &&
            "id" in item &&
            item.id === workspaceId,
        ) as { name?: string } | undefined
      )?.name ?? workspaceId;
    const now = "2026-06-22T09:00:00Z";
    const task = {
      action: (body.action as Record<string, unknown> | undefined) ?? {
        prompt: "",
        session_mode: "create_new_chat",
        type: "agent_prompt",
      },
      createdAt: now,
      description: (body.description as string | null | undefined) ?? null,
      id: `scheduled-task-${appTestState.scheduledTasksResponse.tasks.length + 1}`,
      lastRunAt: null,
      metadata: {
        concurrencyPolicy: body.concurrencyPolicy ?? "skip_if_running",
        misfirePolicy: body.misfirePolicy ?? "catch_up_once",
        workspaceId,
      },
      nextRunAt: now,
      schedule: (body.schedule as Record<string, unknown> | undefined) ?? {
        every_seconds: 86400,
        type: "interval",
      },
      status: (body.status as string | undefined) ?? "enabled",
      title: (body.title as string | undefined) ?? "New scheduled task",
      updatedAt: now,
      usage: emptyScheduledTaskUsage,
      workspaceId,
      workspaceName,
    };
    appTestState.scheduledTasksResponse = {
      tasks: [task, ...appTestState.scheduledTasksResponse.tasks],
    };
    appTestState.scheduledTaskRunsByTaskId = {
      ...appTestState.scheduledTaskRunsByTaskId,
      [task.id]: [],
    };
    return jsonResponse({ task });
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

  if (path === "/api/workspaces/workspace-1/files") {
    return jsonResponse(workspaceFilesResponse);
  }

  if (path === "/api/workspaces/workspace-1/files/children") {
    const childPath = requestUrl.searchParams.get("path");
    if (childPath === "src/pages") {
      return jsonResponse({
        children: [
          {
            children: [],
            childrenLoaded: true,
            hasChildren: false,
            kind: "file",
            name: "index.tsx",
            path: "src/pages/index.tsx",
            sizeBytes: 256,
          },
        ],
        path: "src/pages",
      });
    }

    return jsonResponse({ children: [], path: childPath ?? "" });
  }

  if (path === "/api/workspaces/workspace-1/files/content") {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { path?: string })
        : {};
    const filePath = body.path ?? "";
    return jsonResponse({
      content:
        filePath === "README.md"
          ? markdownFileContent
          : `// ${filePath || "untitled"}`,
      path: filePath,
    });
  }

  if (path === "/api/workspaces/workspace-1/files/save") {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { content?: string; path?: string })
        : {};
    return jsonResponse({
      content: body.content ?? "",
      path: body.path ?? "",
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

  if (path === "/api/agent-definitions") {
    return jsonResponse(agentDefinitions);
  }

  if (
    path === "/api/agent-definitions/create" ||
    path === "/api/agent-definitions/update" ||
    path === "/api/agent-definitions/delete"
  ) {
    return jsonResponse(agentDefinitions);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team") {
    return jsonResponse(agentTeamSnapshot);
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/agent-team/enable") {
    return jsonResponse(agentTeamSnapshot);
  }

  if (
    path === "/api/workspaces/workspace-1/chats/chat-1/agent-team/action" ||
    path === "/api/workspaces/workspace-1/chats/chat-1/agent-team/instances/create" ||
    path === "/api/workspaces/workspace-1/agent-tasks/agent-task-1/action"
  ) {
    return jsonResponse(agentTeamSnapshot);
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

  if (path === "/api/providers/models/refresh") {
    return jsonResponse({
      providers: [
        { providerId: "openai", models: ["gpt-4.1-refresh", "gpt-4.1-mini"] },
        { providerId: "anthropic", models: [] },
      ],
      settings: {
        ...settings,
        providers: settings.providers.map((provider) =>
          provider.id === "anthropic" ? { ...provider, enabled: false } : provider,
        ),
      },
    });
  }

  if (path === "/api/providers/models") {
    return jsonResponse({
      providerId: "openai",
      models: ["gpt-4.1", "gpt-4.1-mini"],
    });
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
      return jsonResponse(appTestState.workspaceGitDiffResponse);
    }

    const file = appTestState.workspaceGitDiffResponse.files.find(
      (summary) => summary.path === selectedPath,
    );
    const escapedPath = selectedPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const sectionMatch = appTestState.workspaceGitDiffResponse.diff.match(
      new RegExp(`diff --git a/${escapedPath} b/${escapedPath}[\\s\\S]*?(?=diff --git a/|$)`),
    );

    return jsonResponse({
      ...appTestState.workspaceGitDiffResponse,
      diff: sectionMatch?.[0] ?? "",
      files: appTestState.workspaceGitDiffResponse.files,
      path: selectedPath,
      status: file
        ? `${file.indexStatus}${file.worktreeStatus} ${file.path}\n`
        : appTestState.workspaceGitDiffResponse.status,
    });
  }

  if (
    path === "/api/workspaces/workspace-1/git/stage" ||
    path === "/api/workspaces/workspace-1/git/unstage" ||
    path === "/api/workspaces/workspace-1/git/discard" ||
    path === "/api/workspaces/workspace-1/git/commit"
  ) {
    return jsonResponse(appTestState.workspaceGitDiffResponse);
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
    appTestState.terminalSessionCounter += 1;
    return jsonResponse({
      id: `terminal-${appTestState.terminalSessionCounter}`,
      name: `Terminal ${appTestState.terminalSessionCounter}`,
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
  if (
    path === "/api/workspaces/workspace-1/chat/queue" ||
    path === "/api/workspaces/workspace-2/chat/queue"
  ) {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { chatId?: string | null; message?: string })
        : {};
    const content = body.message ?? "";
    appTestState.chatQueueCounter += 1;
    const chatId = body.chatId ?? `queued-chat-${appTestState.chatQueueCounter}`;
    const assistantMessageId =
      appTestState.chatQueueCounter === 1
        ? "message-assistant-stream"
        : `message-assistant-stream-${appTestState.chatQueueCounter}`;
    return jsonResponse({
      chatId,
      chatTitle: content || "Queued chat",
      content,
      createdAt: "2026-06-05T12:00:00Z",
      parts: content ? [{ text: content, type: "text" }] : [],
      updatedAt: "2026-06-05T12:00:00Z",
      userMessageId: `queued-user-${appTestState.chatQueueCounter}`,
      assistantMessageId,
    });
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
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { chatId?: string | null })
        : {};
    return chatStreamResponse(body.chatId ?? "side-chat-stream");
  }

  return jsonResponse({ error: `Unhandled test route: ${url}` }, { status: 404 });
}

export function chatStreamResponse(chatId = "chat-1") {
  const encoder = new TextEncoder();
  appTestState.chatStreamCounter += 1;
  const userMessageId =
    appTestState.chatStreamCounter === 1
      ? "message-user-stream"
      : `message-user-stream-${appTestState.chatStreamCounter}`;
  const assistantMessageId =
    appTestState.chatStreamCounter === 1
      ? "message-assistant-stream"
      : `message-assistant-stream-${appTestState.chatStreamCounter}`;
  const llmRequestId =
    appTestState.chatStreamCounter === 1 ? "request-stream" : `request-stream-${appTestState.chatStreamCounter}`;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      appTestState.activeChatStreamController = controller;
      appTestState.chatStreamControllers.set(llmRequestId, controller);
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

export function enqueueChatStreamEvent(value: unknown) {
  if (!appTestState.activeChatStreamController) {
    throw new Error("chat stream is not active");
  }

  const encoder = new TextEncoder();
  appTestState.activeChatStreamController.enqueue(
    encoder.encode(`data: ${JSON.stringify(value)}\n\n`),
  );
}

export function enqueueChatStreamEventForRun(runId: string, value: unknown) {
  const controller = appTestState.chatStreamControllers.get(runId);
  if (!controller) {
    throw new Error(`chat stream is not active: ${runId}`);
  }

  const encoder = new TextEncoder();
  controller.enqueue(encoder.encode(`data: ${JSON.stringify(value)}\n\n`));
}

export function jsonResponse(value: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(value), {
    headers: { "Content-Type": "application/json" },
    status: 200,
    ...init,
  });
}

export type Deferred<T> = {
  promise: Promise<T>;
  reject: (reason?: unknown) => void;
  resolve: (value: T | PromiseLike<T>) => void;
};

export function deferred<T>(): Deferred<T> {
  let reject: Deferred<T>["reject"] = () => undefined;
  let resolve: Deferred<T>["resolve"] = () => undefined;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  return { promise, reject, resolve };
}
