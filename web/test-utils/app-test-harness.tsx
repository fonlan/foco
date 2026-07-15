import { fireEvent, render, type RenderOptions } from "@testing-library/react";
import { vi } from "vitest";

import type {
  ConfiguredModelSummary,
  ConfiguredSkillSummary,
  ConfiguredWorkspaceSummary,
  GitBranchesResponse,
  MemoryDreamJobsResponse,
  MemoryFactRecord,
  ModelTestResponse,
  Plan,
  QuestionRequestSummary,
  RemoteServerSummary,
  SettingsWorkspaceSpecJobSummary,
  UpdateStatusSummary,
  WorkspaceSpecJobSummary,
  WorkspaceSpecResponse,
} from "../api/types";

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
export const defaultPlanModeSystemPrompt =
  "You are Foco Plan Mode, a planning partner for software work.";
export const defaultReviewSystemPrompt =
  "You are Foco's built-in code review agent.";

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
    queuedRun: null,
    title,
    updatedAt,
  };
}

export const workspaceChats = [
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
    queuedRun: null,
    title: `Older chat ${index + 1}`,
    updatedAt: `2026-06-04T${String(10 - index).padStart(2, "0")}:05:00Z`,
  })),
];

export const sideProjectChats = [
  chatSummary(
    "side-chat-1",
    "Side note",
    "2026-06-05T12:00:00Z",
    "2026-06-05T12:05:00Z",
  ),
];

export const workspace = {
  chatPagination: {
    hasMore: true,
    limit: 5,
    nextCursor: "workspace-page-2",
    total: workspaceChats.length,
  },
  chats: workspaceChats.slice(0, 5),
  commonCommands: [],
  connectionStatus: "local",
  displayPath: "C:\\Users\\fonla\\.foco\\workspace",
  id: "workspace-1",
  lastRemoteError: null,
  logoUrl: "/api/workspaces/workspace-1/logo/thumbnail?v=1",
  name: "Default",
  path: "C:\\Users\\fonla\\.foco\\workspace",
  remotePath: null,
  serverId: null,
  serverName: null,
  pinned: false,
  terminalShell: "powershell",
};

export const secondaryWorkspace = {
  chatPagination: {
    hasMore: false,
    limit: 5,
    nextCursor: null,
    total: sideProjectChats.length,
  },
  chats: sideProjectChats,
  commonCommands: [],
  connectionStatus: "local",
  displayPath: "C:\\Users\\fonla\\Documents\\Repos\\SideProject",
  id: "workspace-2",
  lastRemoteError: null,
  logoUrl: null,
  name: "Side project",
  path: "C:\\Users\\fonla\\Documents\\Repos\\SideProject",
  remotePath: null,
  serverId: null,
  serverName: null,
  pinned: false,
  terminalShell: "powershell",
};

export const settings = {
  about: {
    version: "0.1.8",
  },
  appVersion: "0.1.8",
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
      inputModalities: ["text"],
      maxOutputTokens: 4096,
      metadataKey: null,
      metadataRefreshedAt: null,
      metadataSourceUrl: null,
      missingLimits: [],
      providerIds: ["openai", "anthropic"],
      outputModalities: ["text"],
      supportsThinking: true,
      supportedThinkingLevels: ["low", "high"],
      systemPromptName: "Default",
      thinkingLevel: null,
      warnings: [],
    },
  ] as ConfiguredModelSummary[],
  general: {
    apiAudit: {
      requestDetailRetentionDays: 3,
      saveRequestResponseDetails: true,
    },
    autoStartEnabled: false,
    chatTitleGenerationModelId: "current_chat_model",
    defaultTeamModeEnabled: true,
    hookAuditEnabled: false,
    language: "en",
    llmRequestRetryCount: 3,
    maxLlmRequestRetryCount: 10,
    runtimeToolStateCompressionEnabled: false,
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
    extractionLlmTimeoutMs: 120000,
    retrievalLlmTimeoutMs: 60000,
    contextBudgetPercent: 12,
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
      workspaceThresholdFacts: 50,
      globalThresholdFacts: 50,
      llmTimeoutMs: 120000,
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
  plan: {
    mergeAutomationMode: "isolated_auto_once" as string,
    modeModelId: null as string | null,
    mergeAutomationModes: [
      { label: "Isolated worktree, auto-merge once", value: "isolated_auto_once" },
    ],
  },
  spec: {
    autoEnabled: true,
    generationModelId: null,
    generationSystemPrompt: null,
    updateSystemPrompt: null,
    llmTimeoutMs: 120000,
    defaultGenerationSystemPrompt:
      "Generate a concise Project Spec Markdown document from provided evidence.",
    defaultUpdateSystemPrompt:
      "Decide whether the Project Spec needs an update after the latest completed chat turn.",
  },
  prompts: {
    defaultSystemPrompt: "You are Foco, a local coding agent.",
    defaultPlanModeSystemPrompt,
    defaultReviewSystemPrompt,
    defaultContextCompressionSystemPrompt:
      "You are creating a context checkpoint handoff summary for a coding agent so work can continue after older conversation messages are replaced by this summary.",
    contextCompressionSystemPrompt: null as string | null,
    extraText: "",
    files: [] as string[],
    systemPrompt: null as string | null,
    systemPrompts: [
      {
        content: "You are Foco, a local coding agent.",
        name: "Default",
      },
      {
        content: defaultPlanModeSystemPrompt,
        name: "Plan Mode",
      },
      {
        content: defaultReviewSystemPrompt,
        name: "Review",
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
    {
      defaultBaseUrl: "https://api.anthropic.com/v1",
      kind: "anthropic",
      label: "Anthropic",
    },
    {
      defaultBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
      kind: "gemini",
      label: "Gemini",
    },
    {
      defaultBaseUrl: "https://api.x.ai/v1",
      kind: "xai",
      label: "xAI",
    },
    {
      defaultBaseUrl: "https://api.deepseek.com/v1",
      kind: "deepseek",
      label: "DeepSeek",
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
      autoSyncModels: true,
      id: "openai",
      kind: "openai-chat",
      kindLabel: "OpenAI Chat",
      modelSyncFilterRegex: "^gpt-4",
      modelRedirects: [] as { from: string; to: string }[],
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
      autoSyncModels: false,
      id: "anthropic",
      kind: "openai-chat",
      kindLabel: "OpenAI Chat",
      modelSyncFilterRegex: null,
      modelRedirects: [] as { from: string; to: string }[],
      name: "Anthropic",
      requestOverrides: [],
      warnings: [],
    },
  ],
  remoteServers: [] as RemoteServerSummary[],
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
    locations: [
      {
        enabled: true,
        id: "global:agents",
        path: "C:\\Users\\fonla\\.agents\\skills",
      },
    ],
    errors: [],
    translationModelId: null as string | null,
  },
  thinkingLevels: [
    { label: "Low", value: "low" },
    { label: "High", value: "high" },
  ],
  update: {
    assetDownloadUrl: null as string | null,
    assetName: null as string | null,
    autoCheckEnabled: false,
    checking: false,
    currentVersion: "0.1.8",
    error: null as string | null,
    lastCheckedAt: null as string | null,
    publishedAt: null as string | null,
    releaseName: null as string | null,
    releaseUrl: null as string | null,
    targetVersion: null as string | null,
    updateAvailable: false,
  } as UpdateStatusSummary,
  workspaces: [
    {
      id: workspace.id,
      isDefault: true,
      name: workspace.name,
      path: workspace.path,
      displayPath: workspace.displayPath,
      serverId: workspace.serverId,
      serverName: workspace.serverName,
      remotePath: workspace.remotePath,
      connectionStatus: workspace.connectionStatus,
      lastRemoteError: workspace.lastRemoteError,
      logoUrl: workspace.logoUrl,
      pinned: workspace.pinned,
      terminalShell: workspace.terminalShell,
      commonCommands: workspace.commonCommands,
    },
  ],
};

const skillStoreFiles = [
  {
    path: "SKILL.md",
    content:
      "---\nname: browser-scout\ndescription: Find useful web references.\n---\n\n# Browser Scout\n\nUse this skill to collect focused web references.",
  },
  {
    path: "README.md",
    content: "# Browser Scout\n\nA focused research helper for local agent work.",
  },
  {
    path: "scripts/search.md",
    content: "Run a short search and summarize the most useful references.",
  },
];

const skillStoreHotSkills = [
  {
    change: 7,
    description: "Find useful web references.",
    id: "browser-scout",
    installs: 42,
    installsYesterday: 35,
    name: "Browser Scout",
    official: true,
    source: "foco/browser-scout",
  },
  {
    change: -1,
    description: "Clean up Markdown notes.",
    id: "markdown-cleaner",
    installs: 12,
    installsYesterday: 13,
    name: "Markdown Cleaner",
    official: false,
    source: "foco/markdown-cleaner",
  },
  ...Array.from({ length: 19 }, (_, index) => {
    const skillNumber = index + 3;
    return {
      change: 0,
      description: `Registry skill ${skillNumber}.`,
      id: `registry-skill-${skillNumber}`,
      installs: 30 - skillNumber,
      installsYesterday: 30 - skillNumber,
      name: skillNumber === 21 ? "Page Two Skill" : `Registry Skill ${skillNumber}`,
      official: false,
      source: `foco/registry-skill-${skillNumber}`,
    };
  }),
];

const skillStoreSearchSkills = [skillStoreHotSkills[0]];

function skillStoreListPage<T>(items: T[], requestUrl: URL) {
  const page = Number(requestUrl.searchParams.get("page") ?? "1");
  const pageSize = Number(requestUrl.searchParams.get("pageSize") ?? "20");
  const safePage = Number.isFinite(page) && page > 0 ? page : 1;
  const safePageSize = Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 20;
  const start = (safePage - 1) * safePageSize;
  const totalPages = Math.ceil(items.length / safePageSize);
  return {
    hasMore: safePage < totalPages,
    page: safePage,
    pageSize: safePageSize,
    skills: items.slice(start, start + safePageSize),
    total: items.length,
    totalPages,
  };
}

function installedSkillFromStore(workspaceId: string | null): ConfiguredSkillSummary {
  return {
    canEnable: true,
    description: "Find useful web references.",
    enabled: true,
    id: "browser-scout",
    key: workspaceId ? `${workspaceId}:browser-scout` : "global:browser-scout",
    name: "browser-scout",
    path: workspaceId
      ? `C:\\Repos\\Default\\.agents\\skills\\browser-scout\\SKILL.md`
      : "C:\\Users\\fonla\\.agents\\skills\\browser-scout\\SKILL.md",
    scope: workspaceId ? "workspace" : "global",
    warnings: [],
    workspaceId,
    workspaceName: workspaceId ? "Default" : null,
  };
}

function skillStoreRefreshedSettings(workspaceId: string | null) {
  const installedSkill = installedSkillFromStore(workspaceId);
  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    skills: {
      ...appTestState.settingsResponse.skills,
      detected: [
        ...appTestState.settingsResponse.skills.detected.filter(
          (skill) => skill.id !== installedSkill.id,
        ),
        installedSkill as (typeof appTestState.settingsResponse.skills.detected)[number],
      ],
    },
  };
  return appTestState.settingsResponse;
}

export const agentDefinitions = {
  agentDefinitions: [
    {
      allowedExecutionWorkspaceModes: ["shared", "isolated_worktree"],
      allowedTools: ["read_file", "find_files", "search_text"],
      description: "Built-in default agent for chat and Team coordination.",
      id: "agent-definition-default",
      maxInstances: 1,
      modelId: "gpt-test",
      modelOptions: { maxOutputTokens: null, thinkingLevel: null },
      name: "Default agent",
      permissions: {
        allowedAgentDefinitionIds: [
          "agent-definition-review",
          "agent-definition-coordinator",
          "agent-definition-worker",
        ],
        canCreateInstances: true,
        canDelegate: true,
      },
      providerId: "openai",
      revision: 1,
      systemPrompt: "Default built-in prompt.",
    },
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
      allowedTools: ["read_file", "find_files", "search_text"],
      description: "Built-in agent for focused code review and verification.",
      id: "agent-definition-review",
      maxInstances: 1,
      modelId: "gpt-test",
      modelOptions: { maxOutputTokens: null, thinkingLevel: null },
      name: "Review",
      permissions: {
        allowedAgentDefinitionIds: [],
        canCreateInstances: false,
        canDelegate: false,
      },
      providerId: "openai",
      revision: 1,
      systemPrompt: defaultReviewSystemPrompt,
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
  defaultRolePrompts: {
    "agent-definition-default": "Default built-in prompt.",
    "agent-definition-review": defaultReviewSystemPrompt,
  },
};

const coordinatorAgentDefinition = agentDefinitions.agentDefinitions.find(
  (definition) => definition.id === "agent-definition-coordinator",
)!;
const workerAgentDefinition = agentDefinitions.agentDefinitions.find(
  (definition) => definition.id === "agent-definition-worker",
)!;

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
        ...coordinatorAgentDefinition,
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
        ...workerAgentDefinition,
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
  runEvents: [],
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
export const agentTranscriptResponse = {
  hasMore: false,
  items: [
    {
      author: "Coordinator",
      content: "Worker, inspect the current task.",
      createdAt: "2026-06-05T10:00:01Z",
      id: "message:agent-message-1",
      kind: "Message",
      metrics: null,
      parts: [],
      role: "user",
      status: null,
      taskStatus: "completed",
    },
    {
      author: "Worker",
      content: "Inspection complete.",
      createdAt: "2026-06-05T10:00:02Z",
      id: "task:agent-task-1:run",
      kind: "Task result",
      metrics: {
        firstTokenLatencyMs: 1,
        llmRequestIds: ["request-1"],
        modelId: "gpt-test",
        outputTokens: 3,
        providerId: "openai",
        totalLatencyMs: 10,
      },
      parts: [
        { type: "reasoning", text: "Checking workspace state." },
        { type: "text", text: "Inspection complete." },
        {
          type: "toolCall",
          toolCall: {
            completedAt: "2026-06-05T10:00:04Z",
            id: "tool-read-file",
            input: { path: "notes.md" },
            isError: false,
            name: "read_file",
            output: { content: "workspace notes" },
            startedAt: "2026-06-05T10:00:03Z",
            status: "completed",
          },
        },
      ],
      role: "assistant",
      status: null,
      taskStatus: "completed",
    },
    {
      author: "Worker",
      content: "Found the issue in the workspace notes.",
      createdAt: "2026-06-05T10:00:04Z",
      id: "message:agent-message-2",
      kind: "Reply",
      metrics: null,
      parts: [],
      role: "assistant",
      status: null,
      taskStatus: "completed",
    },
  ],
  page: 1,
  pageSize: 25,
  totalCount: 3,
  totalPages: 1,
};

export const activeMemory = {
  chatId: null,
  confidence: null,
  createdAt: "2026-06-09T02:00:00Z",
  enabled: true,
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

export const memoryDreamJob = {
  changeCounts: {
    added: 1,
    expired: 1,
    rejected: 0,
    superseded: 2,
    updated: 1,
  },
  completedAt: "2026-06-10T02:15:00Z",
  createdAt: "2026-06-10T02:00:00Z",
  errorMessage: null,
  id: "dream-job-1",
  mode: "llm",
  modelId: "gpt-test",
  scope: "workspace",
  startedAt: "2026-06-10T02:00:30Z",
  status: "completed",
  summary: "Merged duplicate workspace preferences.",
  transcriptChatId: "dream-transcript-chat-1",
  transcriptWorkspaceId: "workspace-1",
  triggerType: "manual",
  workspaceId: "workspace-1",
};

export const failedMemoryDreamJob = {
  ...memoryDreamJob,
  changeCounts: {
    added: 0,
    expired: 0,
    rejected: 0,
    superseded: 0,
    updated: 0,
  },
  completedAt: "2026-06-09T02:15:00Z",
  createdAt: "2026-06-09T02:00:00Z",
  errorMessage: "memory Dream model unavailable",
  id: "dream-job-failed",
  status: "failed",
  summary: null,
  transcriptChatId: null,
};

export const memoryDreamChange = {
  afterJson: { fact: "Prefer concise repo answers.", status: "active" },
  appliedAt: "2026-06-10T02:14:00Z",
  beforeJson: { fact: "Prefer concise answers.", status: "active" },
  confidence: 0.91,
  createdAt: "2026-06-10T02:12:00Z",
  errorMessage: null,
  evidence: [{ quote: "Use concise repo answers.", sourceId: "memory-active-1" }],
  id: "dream-change-1",
  jobId: "dream-job-1",
  newFactId: null,
  operation: "update",
  reason: "Refined duplicate preference wording.",
  riskLevel: "low",
  status: "applied",
  targetFactIds: ["memory-active-1"],
};

export const workspaceSpecJob: WorkspaceSpecJobSummary = {
  baseRevision: 3,
  chatId: null,
  completedAt: "2026-06-11T03:01:00Z",
  createdAt: "2026-06-11T03:00:00Z",
  errorMessage: null,
  hasRetry: false,
  id: "workspace-spec-job-1",
  inputSummary: {},
  modelId: "gpt-test",
  output: null,
  runId: null,
  startedAt: "2026-06-11T03:00:10Z",
  status: "completed",
  triggerType: "manual_refresh",
};

export const workspaceSpecQueuedJob: WorkspaceSpecJobSummary = {
  ...workspaceSpecJob,
  completedAt: null,
  createdAt: "2026-06-11T03:05:00Z",
  id: "workspace-spec-job-queued",
  startedAt: null,
  status: "queued",
};

export const settingsSpecCompletedJob: SettingsWorkspaceSpecJobSummary = {
  job: {
    ...workspaceSpecJob,
    output: { contentBytes: 512, revision: 4 },
  },
  workspaceId: "workspace-1",
  workspaceName: "Default",
  workspacePath: "/Users/fonla/Repos/Foco",
};

export const settingsSpecFailedJob: SettingsWorkspaceSpecJobSummary = {
  job: {
    ...workspaceSpecJob,
    completedAt: "2026-06-11T03:09:00Z",
    createdAt: "2026-06-11T03:08:00Z",
    errorMessage: "model timed out",
    id: "workspace-spec-job-failed",
    output: null,
    startedAt: "2026-06-11T03:08:10Z",
    status: "failed",
    triggerType: "chat_completed",
  },
  workspaceId: "workspace-2",
  workspaceName: "Side project",
  workspacePath: "/Users/fonla/Repos/SideProject",
};

export const settingsSpecRunningJob: SettingsWorkspaceSpecJobSummary = {
  job: {
    ...workspaceSpecQueuedJob,
    createdAt: "2026-06-11T03:07:00Z",
    id: "workspace-spec-job-running",
    startedAt: "2026-06-11T03:07:10Z",
    status: "running",
  },
  workspaceId: "workspace-1",
  workspaceName: "Default",
  workspacePath: "/Users/fonla/Repos/Foco",
};

export const settingsSpecFailedRetriedJob: SettingsWorkspaceSpecJobSummary = {
  job: {
    ...settingsSpecFailedJob.job,
    createdAt: "2026-06-11T03:06:00Z",
    errorMessage: "already retried timeout",
    hasRetry: true,
    id: "workspace-spec-job-failed-retried",
  },
  workspaceId: "workspace-1",
  workspaceName: "Default",
  workspacePath: "/Users/fonla/Repos/Foco",
};

function clonedSettingsSpecJobs(): SettingsWorkspaceSpecJobSummary[] {
  return JSON.parse(
    JSON.stringify([
      settingsSpecFailedJob,
      settingsSpecRunningJob,
      settingsSpecFailedRetriedJob,
      settingsSpecCompletedJob,
    ]),
  ) as SettingsWorkspaceSpecJobSummary[];
}

export const workspaceSpec: WorkspaceSpecResponse = {
  contentMarkdown:
    "# Project Spec\n\n## Purpose\n\nDescribe the current workspace.",
  generatedAt: "2026-06-11T03:01:00Z",
  latestJob: workspaceSpecJob,
  revision: 3,
  settings: {
    enabled: true,
    injectEnabled: true,
  },
  updatedAt: "2026-06-11T03:01:00Z",
};

function clonedWorkspaceSpec(): WorkspaceSpecResponse {
  return JSON.parse(JSON.stringify(workspaceSpec)) as WorkspaceSpecResponse;
}

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
      invalidatedAt: null,
      invalidatedReason: null,
      firstTokenAt: "2026-06-05T10:00:01Z",
      firstTokenLatencyMs: 1000,
      id: "request-1",
      inputTokens: 100,
      modelId: "gpt-test",
      outputTokens: 40,
      providerId: "openai",
      reasoningTokens: 4,
      requestKind: "chat completion",
      requestStartedAt: "2026-06-05T10:00:00Z",
      statusCode: 200,
      thinkingLevel: "high",
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
    requestKindBreakdown: [
      {
        averageLatencyMs: 2000,
        failedRequests: 1,
        requestCount: 120,
        requestKind: "chat completion",
        totalCacheReadTokens: 10,
        totalCacheWriteTokens: 2,
        totalInputTokens: 12000,
        totalLatencyMs: 240000,
        totalOutputTokens: 4800,
        totalReasoningTokens: 400,
        totalTokens: 16800,
      },
      {
        averageLatencyMs: 800,
        failedRequests: 0,
        requestCount: 5,
        requestKind: "contextCompression",
        totalCacheReadTokens: 0,
        totalCacheWriteTokens: 0,
        totalInputTokens: 500,
        totalLatencyMs: 4000,
        totalOutputTokens: 260,
        totalReasoningTokens: 0,
        totalTokens: 760,
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
    requestBody: {
      body: JSON.stringify({
        input: [{ content: "Hello", role: "user" }],
        model: "gpt-test",
      }),
      format: "provider_request_v1",
      headers: {
        accept: ["application/json"],
        authorization: ["********"],
        "content-type": ["application/json"],
        cookie: ["session=fixture-cookie"],
        "x-api-key": ["fixture-api-key"],
        "x-legacy-redacted": "[REDACTED]",
        "x-real-ip": ["203.0.113.42"],
      },
      method: "POST",
      url: "https://api.example.test/v1/responses",
      version: 1,
    },
    responseBody: {
      format: "provider_final_response_v1",
      http: {
        headers: {
          authorization: ["********"],
          "content-type": ["application/json"],
          "set-cookie": ["response-session=fixture-cookie"],
          "x-api-key": ["fixture-response-api-key"],
          "x-request-id": ["request-fixture-1"],
        },
        status: 200,
        version: "HTTP/2.0",
      },
      reasoning: "Finished reasoning.",
      responseId: "resp-test",
      state: "succeeded",
      stopReason: "stop",
      text: "Done.",
      toolCalls: [],
      usage: { inputTokens: 3, outputTokens: 2 },
      version: 1,
    },
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
        autoSyncModels: true,
        id: "test-provider",
        kind: "openai-chat",
        kindLabel: "OpenAI Chat",
        modelSyncFilterRegex: "^gpt-4",
        modelRedirects: [] as { from: string; to: string }[],
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
      inputModalities: ["text"],
      maxOutputTokens: 2048,
      metadataKey: null,
      metadataRefreshedAt: null,
      metadataSourceUrl: null,
      missingLimits: [],
      providerIds: ["openai"],
      outputModalities: ["text"],
      reasoning: false,
      supportsThinking: false,
      supportedThinkingLevels: [],
      systemPromptName: "Default",
      thinkingLevel: null,
      warnings: [],
    },
  ],
  fetchedAt: "2026-06-05T10:00:00Z",
  models: [
    {
      contextWindow: 32000,
      inputModalities: ["text"],
      key: "openai/created-model",
      maxOutputTokens: 2048,
      modelId: "created-model",
      name: "Created Model",
      outputModalities: ["text"],
      pricing: {
        cacheRead: null,
        cacheWrite: null,
        input: null,
        output: null,
        reasoning: null,
      },
      providerId: "openai",
      providerName: "OpenAI",
      reasoning: false,
      refreshedAt: "2026-06-05T10:00:00Z",
      sourceUrl: "https://models.dev/api.json",
      supportedThinkingLevels: [],
      supportsCache: false,
      supportsTools: true,
    },
    {
      contextWindow: null,
      inputModalities: ["text"],
      key: "openai/gpt-image-2",
      maxOutputTokens: null,
      modelId: "gpt-image-2",
      name: "GPT Image 2",
      outputModalities: ["image"],
      pricing: {
        cacheRead: null,
        cacheWrite: null,
        input: null,
        output: null,
        reasoning: null,
      },
      providerId: "openai",
      providerName: "OpenAI",
      reasoning: false,
      refreshedAt: "2026-06-05T10:00:00Z",
      sourceUrl: "https://models.dev/api.json",
      supportedThinkingLevels: [],
      supportsCache: false,
      supportsTools: false,
    },
  ],
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
  chat: {
    id: "chat-1",
    kind: null,
    readOnly: false,
    title: "Tool run",
  },
  pagination: { hasMoreBefore: false, nextBeforeSequence: null },
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
        llmRequestIds: ["request-1", "request-2"],
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

export const dreamTranscriptChatMessages = {
  chat: {
    id: "dream-transcript-chat-1",
    kind: "memory_dream",
    readOnly: true,
    title: "Memory Dream: workspace manual",
  },
  pagination: { hasMoreBefore: false, nextBeforeSequence: null },
  messages: [
    {
      content: "job started\n\nfinal status: completed",
      createdAt: "2026-06-10T02:00:30.000Z",
      extractedMemories: [],
      id: "dream-transcript-message-1",
      memoriesUsed: [],
      metrics: null,
      parts: [{ text: "job started\n\nfinal status: completed", type: "text" }],
      reasoning: null,
      role: "assistant",
      toolCalls: [],
    },
  ],
};

export const secondChatMessages = {
  chat: {
    id: "chat-2",
    kind: null,
    readOnly: false,
    title: "Second chat",
  },
  pagination: { hasMoreBefore: false, nextBeforeSequence: null },
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

export const planFixture: Plan = {
  activePhaseId: "phase-1",
  completedAt: null,
  completedByUserAt: null,
  createdAt: "2026-06-05T10:00:00Z",
  errorMessage: null,
  sharedMergeCommitId: null,
  id: "plan-1",
  overview: "Implement a focused settings regression.",
  pauseRequestedAt: null,
  phases: [
    {
      agentTaskId: null,
      agentTeamId: null,
      attempts: [],
      commitId: null,
      completedAt: null,
      createdAt: "2026-06-05T10:00:00Z",
      errorMessage: null,
      id: "phase-1",
      implementationChatId: null,
      mergeAttemptCount: 0,
      planId: "plan-1",
      sequence: 0,
      startedAt: null,
      status: "ready",
      steps: [
        {
          acceptance: ["Plan history renders"],
          checkedAt: null,
          createdAt: "2026-06-05T10:00:00Z",
          detail: "Use the settings page.",
          id: "step-1",
          phaseId: "phase-1",
          planId: "plan-1",
          sequence: 0,
          status: "pending",
          title: "Open settings",
          updatedAt: "2026-06-05T10:00:00Z",
        },
      ],
      summary: "Single phase fixture.",
      title: "Implement UI",
      updatedAt: "2026-06-05T10:00:00Z",
    },
  ],
  sortOrder: 0,
  sourceChatId: null,
  status: "ready",
  title: "Settings plan fixture",
  updatedAt: "2026-06-05T10:05:00Z",
};

export const contextUsage = {
  availableMessageTokens: 110960,
  assembledMessageTokens: 52340,
  assembledUsagePercent: 47,
  compressionSnapshotTokens: 2340,
  compressionTriggerPercent: 80,
  compressionTriggerTokens: 102400,
  contextWindow: 128000,
  historyTokens: 32000,
  hasLlmCompressionPlan: false,
  llmCompressionTriggerPercent: 95,
  llmCompressionTriggerTokens: 121600,
  maxOutputTokens: 20000,
  memoryBudgetTokens: 15360,
  memoryContextTokens: 120,
  segments: {
    compressionSnapshot: 2340,
    history: 32000,
    reservedOutput: 0,
    systemPrompt: 4200,
    toolSchema: 9800,
  },
  systemPromptTokens: 4200,
  packedMessageTokens: 52340,
  postCompressionMessageTokens: 52340,
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
      {
        compressibleTokens: 4000,
        optionalTokens: 5000,
        requiredTokens: 1000,
        source: "toolCalls",
        tokens: 6000,
      },
    ],
    compressibleTokens: 36120,
    optionalTokens: 37120,
    requiredTokens: 21220,
  },
  toolSchemaTokens: 9800,
  totalUsedContextTokens: 48340,
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
    runtimeToolStateSnapshotCount: 2,
    savedTokenCount: 6800,
    snapshotCount: 1,
    summaryTokenCount: 2200,
  },
  contextUsageTimeline: [
    {
      contextWindow: 128000,
      kind: "rule",
      maxOutputTokens: 20000,
      segments: {
        compressionSnapshot: 18000,
        history: 85000,
        reservedOutput: 0,
        systemPrompt: 4200,
        toolSchema: 9800,
      },
      sequence: 1,
      snapshotId: "snapshot-with-a-long-id-that-should-not-break-layout",
      totalUsedTokens: 117000,
      triggerTokens: 102400,
    },
    {
      contextWindow: 128000,
      kind: "llm",
      maxOutputTokens: 20000,
      segments: {
        compressionSnapshot: 12000,
        history: 64000,
        reservedOutput: 0,
        systemPrompt: 4200,
        toolSchema: 9800,
      },
      sequence: 2,
      snapshotId: "snapshot-2",
      totalUsedTokens: 80000,
      triggerTokens: 102400,
    },
  ],
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
        children: [],
        childrenLoaded: true,
        hasChildren: false,
        kind: "file",
        name: "logo.png",
        path: "assets/logo.png",
        sizeBytes: 2048,
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
        name: "index.html",
        path: "demo/index.html",
        sizeBytes: 256,
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
  '<div align="center">',
  "",
  '<img src="foco.svg" alt="Foco" width="96" />',
  "",
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
  "",
  "</div>",
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

export const scheduledTasks: {
  page: number;
  pageSize: number;
  statusCounts: Record<string, number>;
  tasks: ScheduledTaskFixture[];
  totalCount: number;
  totalPages: number;
} = {
  page: 1,
  pageSize: 25,
  statusCounts: { enabled: 1, paused: 0, completed: 0, archived: 0 },
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
  totalCount: 1,
  totalPages: 1,
};

function scheduledTasksEnvelope(
  tasks: ScheduledTaskFixture[],
  page = 1,
  pageSize = 25,
): typeof scheduledTasks {
  const statusCounts = tasks.reduce<Record<string, number>>(
    (counts, task) => ({ ...counts, [task.status]: (counts[task.status] ?? 0) + 1 }),
    { enabled: 0, paused: 0, completed: 0, archived: 0 },
  );
  return {
    page,
    pageSize,
    statusCounts,
    tasks,
    totalCount: tasks.length,
    totalPages: tasks.length ? Math.ceil(tasks.length / pageSize) : 0,
  };
}

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
  chatMessagesResponsesByChatKey: Record<string, typeof chatMessages>;
  chatStatisticsResponsesByChatKey: Record<string, typeof chatStatistics>;
  contextUsageResponseQueuesByChatKey: Record<string, Array<typeof contextUsage>>;
  terminalSessionCounter: number;
  chatStreamCounter: number;
  chatQueueCounter: number;
  scheduledTaskRunsByTaskId: Record<string, ScheduledTaskRunFixture[]>;
  scheduledTasksResponse: typeof scheduledTasks;
  settingsResponse: Omit<typeof settings, "skills" | "workspaces"> & {
    skills: {
      detected: ConfiguredSkillSummary[];
      directories: string[];
      locations?: { id: string; path: string; enabled: boolean }[];
      errors: { path: string; message: string }[];
      translationModelId: string | null;
    };
    workspaces: ConfiguredWorkspaceSummary[];
  };
  workspaceSpecResponse: typeof workspaceSpec;
  workspaceSpecResponsesByWorkspaceId: Record<string, typeof workspaceSpec>;
  workspaceSpecSettingsResponses: Array<Response | Promise<Response>>;
  settingsSpecJobsResponse: SettingsWorkspaceSpecJobSummary[];
  agentTeamSnapshotResponse: typeof agentTeamSnapshot;
  workspaceSpecSaveConflict: boolean;
  workspaceSpecGenerateCompletes: boolean;
  workspaceSpecJobPollCount: number;
  workspaceSpecJobPollFailuresRemaining: number;
  workspaceSpecJobPollsBeforeCompletion: number | null;
  workspaceGitBranchesResponses: GitBranchesResponse[];
  workspaceGitDiffResponse: typeof gitDiff;
  workspaceGitDiffResponsesByWorktreePath: Record<string, typeof gitDiff>;
  pendingQuestionsResponse: QuestionRequestSummary[];
  answeredQuestionIds: string[];
  workspaceResponseWorkspaces: unknown[];
  workspaceChatsByWorkspaceId: Record<string, Array<(typeof workspaceChats)[number]>>;
  workspaceChatSearchResponseWorkspaces: unknown[] | null;
  memoryDreamJobsResponses: MemoryDreamJobsResponse[];
  memoriesById: Record<string, MemoryFactRecord>;
  memoryListAdditional: MemoryFactRecord[];
  memoryEnabledResponses: Array<Response | Promise<Response>>;
  modelTestResponses: Array<Response | Promise<Response>>;
  updateHealthStatuses: number[];
  lastWorkspaceOrderRequest: string[] | null;
  lastManualWorkspaceRequest: Partial<ConfiguredWorkspaceSummary> | null;
  previewSessionFailNext: boolean | string;
  previewSessionCounter: number;
  activePreviewSessions: Array<{ path: string; token: string; workspaceId: string }>;
} = {
  activeChatStreamController: null,
  chatStreamControllers: new Map<string, ReadableStreamDefaultController<Uint8Array>>(),
  chatMessagesResponsesByChatKey: {},
  chatStatisticsResponsesByChatKey: {},
  contextUsageResponseQueuesByChatKey: {},
  terminalSessionCounter: 0,
  chatStreamCounter: 0,
  chatQueueCounter: 0,
  scheduledTaskRunsByTaskId,
  scheduledTasksResponse: scheduledTasks,
  settingsResponse: settings,
  workspaceSpecResponse: clonedWorkspaceSpec(),
  workspaceSpecResponsesByWorkspaceId: {
    [workspace.id]: clonedWorkspaceSpec(),
  },
  workspaceSpecSettingsResponses: [],
  settingsSpecJobsResponse: clonedSettingsSpecJobs(),
  agentTeamSnapshotResponse: agentTeamSnapshot,
  workspaceSpecSaveConflict: false,
  workspaceSpecGenerateCompletes: false,
  workspaceSpecJobPollCount: 0,
  workspaceSpecJobPollFailuresRemaining: 0,
  workspaceSpecJobPollsBeforeCompletion: null,
  workspaceGitBranchesResponses: [],
  workspaceGitDiffResponse: gitDiff,
  workspaceGitDiffResponsesByWorktreePath: {},
  pendingQuestionsResponse: [],
  answeredQuestionIds: [],
  workspaceResponseWorkspaces: [workspace, secondaryWorkspace],
  workspaceChatsByWorkspaceId: {},
  workspaceChatSearchResponseWorkspaces: null,
  memoryDreamJobsResponses: [],
  memoriesById: {
    [activeMemory.id]: { ...activeMemory },
    [chatMemory.id]: { ...chatMemory },
    [pendingMemory.id]: { ...pendingMemory },
    [workspaceMemory.id]: { ...workspaceMemory },
  },
  memoryListAdditional: [],
  memoryEnabledResponses: [],
  modelTestResponses: [],
  updateHealthStatuses: [],
  lastWorkspaceOrderRequest: null,
  lastManualWorkspaceRequest: null,
  previewSessionFailNext: false,
  previewSessionCounter: 0,
  activePreviewSessions: [],
};

function workspaceSpecResponseForWorkspace(workspaceId: string) {
  if (workspaceId === workspace.id) {
    return appTestState.workspaceSpecResponse;
  }

  const existing = appTestState.workspaceSpecResponsesByWorkspaceId[workspaceId];
  if (existing) {
    return existing;
  }

  const created = clonedWorkspaceSpec();
  appTestState.workspaceSpecResponsesByWorkspaceId = {
    ...appTestState.workspaceSpecResponsesByWorkspaceId,
    [workspaceId]: created,
  };
  return created;
}

function setWorkspaceSpecResponseForWorkspace(
  workspaceId: string,
  response: typeof workspaceSpec,
) {
  if (workspaceId === workspace.id) {
    appTestState.workspaceSpecResponse = response;
  }
  appTestState.workspaceSpecResponsesByWorkspaceId = {
    ...appTestState.workspaceSpecResponsesByWorkspaceId,
    [workspaceId]: response,
  };
}

function workspaceSettingsSummaryFromWorkspace(item: unknown): ConfiguredWorkspaceSummary {
  const workspaceSummary = item as ConfiguredWorkspaceSummary & { chats?: unknown[] };

  return {
    commonCommands: workspaceSummary.commonCommands ?? [],
    connectionStatus: workspaceSummary.connectionStatus ?? "local",
    displayPath: workspaceSummary.displayPath ?? workspaceSummary.path,
    id: workspaceSummary.id,
    isDefault: workspaceSummary.isDefault ?? workspaceSummary.id === workspace.id,
    lastRemoteError: workspaceSummary.lastRemoteError ?? null,
    logoUrl: workspaceSummary.logoUrl ?? null,
    name: workspaceSummary.name,
    path: workspaceSummary.path,
    remotePath: workspaceSummary.remotePath ?? null,
    serverId: workspaceSummary.serverId ?? null,
    serverName: workspaceSummary.serverName ?? null,
    pinned: Boolean(workspaceSummary.pinned),
    terminalShell: workspaceSummary.terminalShell ?? "powershell",
  };
}

function settingsWorkspacesFromWorkspaceResponse() {
  return appTestState.workspaceResponseWorkspaces.map(workspaceSettingsSummaryFromWorkspace);
}

function workspaceChatSearchWorkspaces(query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();

  if (normalizedQuery.length === 0) {
    return [];
  }

  const source =
    appTestState.workspaceChatSearchResponseWorkspaces ??
    appTestState.workspaceResponseWorkspaces;

  return source
    .map((item) => {
      const workspaceSummary = item as { id?: string; chats?: unknown[] };
      const availableChats =
        workspaceSummary.id === workspace.id
          ? workspaceChats
          : workspaceSummary.id === secondaryWorkspace.id
            ? sideProjectChats
            : (workspaceSummary.chats ?? []);
      const chats = availableChats.filter((chat) =>
        String((chat as { title?: unknown }).title ?? "")
          .toLocaleLowerCase()
          .includes(normalizedQuery),
      );

      return chats.length > 0
        ? {
          ...(item as object),
          chatPagination: {
            hasMore: false,
            limit: 5,
            nextCursor: null,
            total: chats.length,
          },
          chats,
        }
        : null;
    })
    .filter((item): item is NonNullable<typeof item> => item !== null);
}

function persistedWorkspaceChats(workspaceId: string) {
  const persisted = appTestState.workspaceChatsByWorkspaceId[workspaceId];
  if (persisted) {
    return persisted;
  }
  if (workspaceId === workspace.id) {
    return workspaceChats;
  }
  if (workspaceId === secondaryWorkspace.id) {
    return sideProjectChats;
  }

  const workspaceSummary = appTestState.workspaceResponseWorkspaces.find(
    (item) => (item as { id?: string }).id === workspaceId,
  ) as { chats?: Array<(typeof workspaceChats)[number]> } | undefined;
  return workspaceSummary?.chats ?? [];
}

function workspaceChatsPage(workspaceId: string, cursor: string | null, includeChatId: string | null) {
  const allChats = persistedWorkspaceChats(workspaceId);
  const limit = 5;
  const startIndex = cursor === "workspace-page-2" ? 5 : cursor === "workspace-page-3" ? 10 : 0;
  const pageChats = allChats.slice(startIndex, startIndex + limit);
  const chats = includeChatId && !pageChats.some((chat) => chat.id === includeChatId)
    ? [...pageChats, ...allChats.filter((chat) => chat.id === includeChatId)]
    : pageChats;
  const nextIndex = startIndex + limit;

  return {
    chats,
    hasMore: nextIndex < allChats.length,
    limit,
    nextCursor: nextIndex < allChats.length ? `workspace-page-${Math.floor(nextIndex / limit) + 1}` : null,
    total: allChats.length,
  };
}

function reorderUnknownWorkspacesByIds<T>(items: T[], itemIds: string[]) {
  const itemsById = new Map(
    items.map((item) => [(item as { id: string }).id, item]),
  );
  const next = itemIds
    .map((itemId) => itemsById.get(itemId))
    .filter((item): item is T => Boolean(item));

  return next.length === items.length ? next : items;
}

function groupPinnedUnknownWorkspaces<T>(items: T[]) {
  return [
    ...items.filter((item) => Boolean((item as { pinned?: boolean }).pinned)),
    ...items.filter((item) => !Boolean((item as { pinned?: boolean }).pinned)),
  ];
}

function promotePinnedUnknownWorkspace<T>(items: T[], itemId: string | undefined) {
  const index = items.findIndex(
    (item) => (item as { id?: string; pinned?: boolean }).id === itemId &&
      Boolean((item as { pinned?: boolean }).pinned),
  );
  if (index <= 0) {
    return items;
  }

  const next = [...items];
  const [item] = next.splice(index, 1);
  next.unshift(item);
  return next;
}

function workspaceOrderSettings(workspaceIds: string[]) {
  appTestState.lastWorkspaceOrderRequest = workspaceIds;
  appTestState.workspaceResponseWorkspaces = reorderUnknownWorkspacesByIds(
    appTestState.workspaceResponseWorkspaces,
    workspaceIds,
  );
  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    workspaces: settingsWorkspacesFromWorkspaceResponse(),
  };

  return appTestState.settingsResponse;
}

function saveManualWorkspaceSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as Partial<ConfiguredWorkspaceSummary>;
  appTestState.lastManualWorkspaceRequest = body;
  appTestState.workspaceResponseWorkspaces = promotePinnedUnknownWorkspace(
    groupPinnedUnknownWorkspaces(
      appTestState.workspaceResponseWorkspaces.map((item) => {
        const current = item as Record<string, unknown>;
        return current.id === body.id ? { ...current, ...body } : current;
      }),
    ),
    body.id,
  );
  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    workspaces: settingsWorkspacesFromWorkspaceResponse(),
  };

  return appTestState.settingsResponse;
}

function saveManualProviderSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as {
    apiProxy?: { enabled: boolean; proxyType: string; url: string };
    autoSyncModels?: boolean;
    baseUrl?: string | null;
    enabled?: boolean;
    id?: string;
    kind?: string;
    modelSyncFilterRegex?: string | null;
    name?: string;
  };
  const currentProvider = appTestState.settingsResponse.providers.find(
    (provider) => provider.id === body.id,
  );
  if (!currentProvider) {
    const providerKind = appTestState.settingsResponse.providerKinds.find(
      (kind) => kind.kind === body.kind,
    );
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      providers: [
        ...appTestState.settingsResponse.providers,
        {
          apiProxy: {
            enabled: body.apiProxy?.enabled ?? false,
            proxyType: body.apiProxy?.proxyType ?? "http",
            supportedTypes: settings.providers[0].apiProxy.supportedTypes,
            url: body.apiProxy?.url ?? "",
          },
          autoSyncModels: body.autoSyncModels ?? false,
          baseUrl: body.baseUrl ?? providerKind?.defaultBaseUrl ?? "",
          enabled: body.enabled ?? true,
          hasApiKey: false,
          id: body.id ?? "provider",
          kind: body.kind ?? "openai-chat",
          kindLabel: providerKind?.label ?? body.kind ?? "OpenAI Chat",
          modelRedirects: [],
          modelSyncFilterRegex: body.modelSyncFilterRegex ?? null,
          name: body.name ?? body.id ?? "Provider",
          requestOverrides: [],
          warnings: [],
        },
      ],
    };
    return appTestState.settingsResponse;
  }

  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    providers: appTestState.settingsResponse.providers.map((provider) => {
      if (provider.id !== body.id) {
        return provider;
      }

      return {
        ...provider,
        apiProxy: body.apiProxy
          ? { ...provider.apiProxy, ...body.apiProxy }
          : provider.apiProxy,
        autoSyncModels: body.autoSyncModels ?? provider.autoSyncModels,
        enabled: body.enabled ?? provider.enabled,
      };
    }),
  };

  return appTestState.settingsResponse;
}

function deleteProviderSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as { id?: string };
  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    providers: appTestState.settingsResponse.providers.filter(
      (provider) => provider.id !== body.id,
    ),
  };

  return appTestState.settingsResponse;
}

function savedSkillsSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as {
    disabled?: string[];
    enabled?: string[];
    translationModelId?: string | null;
    disabledLocationIds?: string[];
  };
  const enabled = new Set(body.enabled ?? []);
  const disabled = new Set(body.disabled ?? []);
  const disabledLocationIds = new Set(body.disabledLocationIds ?? []);
  const hasLocationUpdate = body.disabledLocationIds !== undefined;

  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    skills: {
      ...appTestState.settingsResponse.skills,
      detected: appTestState.settingsResponse.skills.detected
        .filter((skill) =>
          hasLocationUpdate
            ? !disabledLocationIds.has(
              skill.scope === "global"
                ? "global:agents"
                : `workspace:${skill.workspaceId}:agents`,
            )
            : true,
        )
        .map((skill) => ({
          ...skill,
          enabled: enabled.has(skill.key)
            ? true
            : disabled.has(skill.key)
              ? false
              : skill.enabled,
        })),
      locations: appTestState.settingsResponse.skills.locations?.map((location) => ({
        ...location,
        enabled: hasLocationUpdate
          ? !disabledLocationIds.has(location.id)
          : location.enabled,
      })),
      translationModelId:
        body.translationModelId === undefined
          ? appTestState.settingsResponse.skills.translationModelId
          : body.translationModelId,
    },
  };

  return appTestState.settingsResponse;
}

function deletedSkillSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as { id?: string };

  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    skills: {
      ...appTestState.settingsResponse.skills,
      detected: appTestState.settingsResponse.skills.detected.filter(
        (skill) => skill.key !== body.id,
      ),
    },
  };

  return appTestState.settingsResponse;
}

function skillStoreUpdateSettings(init?: RequestInit) {
  const body = JSON.parse(String(init?.body ?? "{}")) as { key?: string };
  return {
    results: [
      {
        error: null,
        key: body.key ?? "",
        ok: true,
        path: "C:\\Users\\fonla\\.agents\\skills\\gitmemo",
      },
    ],
    settings: appTestState.settingsResponse,
  };
}

function skillStoreUpdateAllSettings() {
  return {
    results: appTestState.settingsResponse.skills.detected
      .filter((skill) => (skill as ConfiguredSkillSummary).store?.updateable)
      .map((skill) => ({
        error: null,
        key: skill.key,
        ok: true,
        path: skill.path.replace(/\\SKILL\.md$/, ""),
      })),
    settings: appTestState.settingsResponse,
  };
}
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
      chatTitleGenerationModelId:
        typeof body.chatTitleGenerationModelId === "string"
          ? body.chatTitleGenerationModelId
          : settings.general.chatTitleGenerationModelId,
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
      runtimeToolStateCompressionEnabled:
        typeof body.runtimeToolStateCompressionEnabled === "boolean"
          ? body.runtimeToolStateCompressionEnabled
          : settings.general.runtimeToolStateCompressionEnabled,
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
  appTestState.chatMessagesResponsesByChatKey = {};
  appTestState.chatStatisticsResponsesByChatKey = {};
  appTestState.contextUsageResponseQueuesByChatKey = {};
  appTestState.terminalSessionCounter = 0;
  appTestState.chatStreamCounter = 0;
  appTestState.chatQueueCounter = 0;
  appTestState.scheduledTaskRunsByTaskId = {
    "scheduled-task-1": [...scheduledTaskRunsByTaskId["scheduled-task-1"]],
  };
  appTestState.scheduledTasksResponse = scheduledTasks;
  appTestState.settingsResponse = {
    ...settings,
    prompts: {
      ...settings.prompts,
    },
  };
  appTestState.workspaceSpecResponse = clonedWorkspaceSpec();
  appTestState.workspaceSpecResponsesByWorkspaceId = {
    [workspace.id]: clonedWorkspaceSpec(),
  };
  appTestState.workspaceSpecSettingsResponses = [];
  appTestState.settingsSpecJobsResponse = clonedSettingsSpecJobs();
  appTestState.agentTeamSnapshotResponse = agentTeamSnapshot;
  appTestState.workspaceSpecSaveConflict = false;
  appTestState.workspaceSpecGenerateCompletes = false;
  appTestState.workspaceSpecJobPollCount = 0;
  appTestState.workspaceSpecJobPollFailuresRemaining = 0;
  appTestState.workspaceSpecJobPollsBeforeCompletion = null;
  appTestState.workspaceGitBranchesResponses = [];
  appTestState.workspaceGitDiffResponse = gitDiff;
  appTestState.workspaceGitDiffResponsesByWorktreePath = {};
  appTestState.pendingQuestionsResponse = [];
  appTestState.answeredQuestionIds = [];
  appTestState.workspaceResponseWorkspaces = [workspace, secondaryWorkspace];
  appTestState.workspaceChatsByWorkspaceId = {};
  appTestState.workspaceChatSearchResponseWorkspaces = null;
  appTestState.memoryDreamJobsResponses = [];
  appTestState.memoriesById = {
    [activeMemory.id]: { ...activeMemory },
    [chatMemory.id]: { ...chatMemory },
    [pendingMemory.id]: { ...pendingMemory },
    [workspaceMemory.id]: { ...workspaceMemory },
  };
  appTestState.memoryListAdditional = [];
  appTestState.memoryEnabledResponses = [];
  appTestState.modelTestResponses = [];
  appTestState.updateHealthStatuses = [];
  appTestState.lastWorkspaceOrderRequest = null;
  appTestState.lastManualWorkspaceRequest = null;
  appTestState.previewSessionFailNext = false;
  appTestState.previewSessionCounter = 0;
  appTestState.activePreviewSessions = [];
  window.history.replaceState(null, "", "/");
  window.localStorage.clear();
  document.documentElement.removeAttribute("data-foco-theme");
  mermaidMock.initialize.mockClear();
  mermaidMock.render.mockClear();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      write: vi.fn().mockResolvedValue(undefined),
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

  if (path === "/api/chat/questions/pending") {
    return jsonResponse({ questions: appTestState.pendingQuestionsResponse });
  }

  const questionAnswerMatch = path.match(/^\/api\/chat\/questions\/([^/]+)\/answer$/);
  if (questionAnswerMatch) {
    appTestState.answeredQuestionIds.push(decodeURIComponent(questionAnswerMatch[1]));
    return jsonResponse({ ok: true, questionId: decodeURIComponent(questionAnswerMatch[1]) });
  }

  if (path === "/api/workspaces") {
    return jsonResponse({
      activeWorkspaceId: workspace.id,
      workspaces: appTestState.workspaceResponseWorkspaces,
    });
  }

  const workspaceChatsMatch = path.match(/^\/api\/workspaces\/([^/]+)\/chats$/);
  if (workspaceChatsMatch) {
    return jsonResponse(workspaceChatsPage(
      decodeURIComponent(workspaceChatsMatch[1] ?? ""),
      requestUrl.searchParams.get("cursor"),
      requestUrl.searchParams.get("includeChatId"),
    ));
  }

  const planAutoRunMatch = path.match(/^\/api\/workspaces\/([^/]+)\/plans\/auto-run$/);
  if (planAutoRunMatch) {
    const body = init?.body ? JSON.parse(String(init.body)) as { enabled?: boolean } : null;
    return jsonResponse({
      busy: false,
      enabled: body?.enabled ?? false,
    });
  }

  const plansMatch = path.match(/^\/api\/workspaces\/([^/]+)\/plans$/);
  if (plansMatch) {
    const workspaceId = decodeURIComponent(plansMatch[1] ?? "");
    const page = Math.max(1, Number(requestUrl.searchParams.get("page")) || 1);
    const pageSize = Math.min(
      100,
      Math.max(1, Number(requestUrl.searchParams.get("pageSize")) || 20),
    );
    const status = requestUrl.searchParams.get("status");
    const plans =
      status && status !== planFixture.status
        ? []
        : [{ ...planFixture, id: `${workspaceId}-plan-1` }];

    return jsonResponse({
      page,
      pageSize,
      plans,
      totalCount: plans.length,
      totalPages: plans.length ? 1 : 0,
    });
  }

  if (path === "/api/workspaces/search-chats") {
    return jsonResponse({
      activeWorkspaceId: workspace.id,
      workspaces: workspaceChatSearchWorkspaces(
        requestUrl.searchParams.get("query") ?? "",
      ),
    });
  }

  if (path === "/api/scheduled-tasks") {
    const page = Math.max(1, Number(requestUrl.searchParams.get("page")) || 1);
    const pageSize = Math.min(
      100,
      Math.max(1, Number(requestUrl.searchParams.get("pageSize")) || 25),
    );
    const status = requestUrl.searchParams.get("status");
    const workspaceId = requestUrl.searchParams.get("workspaceId");
    const query = (requestUrl.searchParams.get("q") ?? "").toLowerCase();
    const tasks = appTestState.scheduledTasksResponse.tasks.filter((task) => {
      if (status && task.status !== status) {
        return false;
      }
      if (workspaceId && task.workspaceId !== workspaceId) {
        return false;
      }
      if (!query) {
        return true;
      }
      return [task.id, task.title, task.description ?? "", task.workspaceName]
        .join(" ")
        .toLowerCase()
        .includes(query);
    });
    return jsonResponse(
      scheduledTasksEnvelope(tasks.slice((page - 1) * pageSize, page * pageSize), page, pageSize),
    );
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
    const runs = appTestState.scheduledTaskRunsByTaskId[taskId] ?? [];
    const page = Math.max(1, Number(requestUrl.searchParams.get("page")) || 1);
    const pageSize = Math.min(
      100,
      Math.max(1, Number(requestUrl.searchParams.get("pageSize")) || 20),
    );
    return jsonResponse({
      page,
      pageSize,
      runs: runs.slice((page - 1) * pageSize, page * pageSize),
      totalCount: runs.length,
      totalPages: runs.length ? Math.ceil(runs.length / pageSize) : 0,
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
    appTestState.scheduledTasksResponse = scheduledTasksEnvelope(
      appTestState.scheduledTasksResponse.tasks.map((task) =>
        task.id === taskId ? { ...task, lastRunAt: now, updatedAt: now } : task,
      ),
    );
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
      appTestState.scheduledTasksResponse = scheduledTasksEnvelope(
        appTestState.scheduledTasksResponse.tasks.map((task) =>
          task.id === taskId ? updatedTask! : task,
        ),
      );
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
    appTestState.scheduledTasksResponse = scheduledTasksEnvelope([
      task,
      ...appTestState.scheduledTasksResponse.tasks,
    ]);
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
      appTestState.scheduledTasksResponse = scheduledTasksEnvelope(
        appTestState.scheduledTasksResponse.tasks.filter((task) => task.id !== taskId),
      );
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
      appTestState.scheduledTasksResponse = scheduledTasksEnvelope(
        appTestState.scheduledTasksResponse.tasks.map((task) =>
          task.id === taskId ? updatedTask : task,
        ),
      );
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
    appTestState.scheduledTasksResponse = scheduledTasksEnvelope([
      task,
      ...appTestState.scheduledTasksResponse.tasks,
    ]);
    appTestState.scheduledTaskRunsByTaskId = {
      ...appTestState.scheduledTaskRunsByTaskId,
      [task.id]: [],
    };
    return jsonResponse({ task });
  }

  if (path === "/api/file-picker/list") {
    return jsonResponse({
      entries: [
        {
          disabled: false,
          isDirectory: true,
          modifiedMs: null,
          name: "NewWorkspace",
          path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
          sizeBytes: null,
        },
        {
          disabled: false,
          isDirectory: false,
          modifiedMs: null,
          name: "note.txt",
          path: "C:/Users/fonla/Desktop/note.txt",
          sizeBytes: 5,
        },
        {
          disabled: false,
          isDirectory: false,
          modifiedMs: null,
          name: "screen.png",
          path: "C:/Users/fonla/Desktop/screen.png",
          sizeBytes: 3,
        },
        {
          disabled: false,
          isDirectory: false,
          modifiedMs: null,
          name: "workspace-logo.png",
          path: "C:/Users/fonla/Desktop/workspace-logo.png",
          sizeBytes: 8,
        },
      ],
      parentPath: "C:/Users/fonla/Documents",
      path: "C:/Users/fonla/Documents/Repos",
      truncated: false,
      warnings: [],
    });
  }

  if (path === "/api/file-picker/read-files") {
    const body = JSON.parse(String(init?.body ?? "{}")) as { paths?: string[] };
    return jsonResponse({
      files: (body.paths ?? ["C:/Users/fonla/Desktop/note.txt"]).map((filePath) => {
        const name = filePath.split(/[\\/]/).pop() ?? "note.txt";
        if (name === "screen.png") {
          return {
            contentBase64: "cG5n",
            contentType: "image/png",
            name,
            path: filePath,
            sizeBytes: 3,
          };
        }
        if (name === "workspace-logo.png") {
          return {
            contentBase64: "iVBORw0KGgo=",
            contentType: "image/png",
            name,
            path: filePath,
            sizeBytes: 8,
          };
        }
        return {
          contentBase64: "SGVsbG8=",
          contentType: "text/plain",
          name,
          path: filePath,
          sizeBytes: 5,
        };
      }),
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
          : filePath.endsWith(".html") || filePath.endsWith(".htm")
            ? "<!DOCTYPE html><html><body><h1>demo</h1></body></html>"
            : `// ${filePath || "untitled"}`,
      path: filePath,
    });
  }

  const previewSessionCreateMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/preview\/sessions$/,
  );
  if (previewSessionCreateMatch && (init?.method === "POST" || !init?.method)) {
    const workspaceId = decodeURIComponent(previewSessionCreateMatch[1] ?? "");
    if (appTestState.previewSessionFailNext) {
      const message =
        typeof appTestState.previewSessionFailNext === "string"
          ? appTestState.previewSessionFailNext
          : "failed to create HTML preview session";
      appTestState.previewSessionFailNext = false;
      return jsonResponse({ message }, { status: 400 });
    }
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { path?: string })
        : {};
    const entryPath = body.path ?? "index.html";
    appTestState.previewSessionCounter += 1;
    const token = `previewtoken${String(appTestState.previewSessionCounter).padStart(20, "0")}`.slice(
      0,
      32,
    );
    appTestState.activePreviewSessions = [
      ...appTestState.activePreviewSessions.filter(
        (session) => !(session.workspaceId === workspaceId && session.path === entryPath),
      ),
      { path: entryPath, token, workspaceId },
    ];
    const rootPath = entryPath.includes("/")
      ? entryPath.slice(0, entryPath.lastIndexOf("/"))
      : "";
    const entryName = entryPath.includes("/")
      ? entryPath.slice(entryPath.lastIndexOf("/") + 1)
      : entryPath;
    return jsonResponse({
      entryPath,
      iframeSandbox: "allow-scripts allow-same-origin",
      previewOrigin: `http://${token}.preview.localhost:3210`,
      previewUrl: `http://${token}.preview.localhost:3210/${entryName}`,
      rootPath,
      token,
      workspaceId,
    });
  }

  const previewSessionDeleteMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/preview\/sessions\/([^/]+)$/,
  );
  if (previewSessionDeleteMatch && init?.method === "DELETE") {
    const workspaceId = decodeURIComponent(previewSessionDeleteMatch[1] ?? "");
    const token = decodeURIComponent(previewSessionDeleteMatch[2] ?? "");
    appTestState.activePreviewSessions = appTestState.activePreviewSessions.filter(
      (session) => !(session.workspaceId === workspaceId && session.token === token),
    );
    return jsonResponse({ released: true });
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
    const newWorkspace = {
      chatPagination: {
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 0,
      },
      chats: [],
      id: "new-workspace",
      connectionStatus: "local",
      displayPath: "C:/Users/fonla/Documents/Repos/NewWorkspace",
      lastRemoteError: null,
      logoUrl: "/api/workspaces/new-workspace/logo/thumbnail?v=1",
      name: "New Workspace",
      path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
      remotePath: null,
      serverId: null,
      serverName: null,
      pinned: false,
      terminalShell: "powershell",
      commonCommands: [],
    };
    appTestState.workspaceResponseWorkspaces = [
      newWorkspace,
      workspace,
      secondaryWorkspace,
    ];
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: settingsWorkspacesFromWorkspaceResponse(),
    };

    return jsonResponse({
      activeWorkspaceId: "new-workspace",
      workspaces: appTestState.workspaceResponseWorkspaces,
    });
  }

  if (path === "/api/health") {
    const status = appTestState.updateHealthStatuses.shift() ?? 200;
    return jsonResponse({ service: "foco" }, { status });
  }

  if (path === "/api/settings") {
    return jsonResponse(appTestState.settingsResponse);
  }

  if (path === "/api/models/test") {
    const response = appTestState.modelTestResponses.shift();
    if (response) {
      return response;
    }

    const body = JSON.parse(String(init?.body ?? "{}")) as { modelId?: string };
    const modelId = body.modelId ?? "gpt-test";
    return jsonResponse({
      message: `Model '${modelId}' responded successfully through provider 'openai'`,
      modelId,
      ok: true,
      providerId: "openai",
    } satisfies ModelTestResponse);
  }

  if (path === "/api/update/status") {
    return jsonResponse(appTestState.settingsResponse.update);
  }

  if (path === "/api/update/check") {
    const update = {
      ...appTestState.settingsResponse.update,
      assetDownloadUrl: "https://github.com/fonlan/foco/releases/download/v0.2.0/Foco-v0.2.0-macos-arm64.dmg",
      assetName: "Foco-v0.2.0-macos-arm64.dmg",
      error: null,
      releaseName: "Foco v0.2.0",
      releaseUrl: "https://github.com/fonlan/foco/releases/tag/v0.2.0",
      targetVersion: "0.2.0",
      updateAvailable: true,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      update,
    };
    return jsonResponse(update);
  }

  if (path === "/api/update/settings") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      autoCheckEnabled?: boolean;
    };
    const update = {
      ...appTestState.settingsResponse.update,
      autoCheckEnabled: Boolean(body.autoCheckEnabled),
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      update,
    };
    return jsonResponse(update);
  }

  if (path === "/api/update/install") {
    return jsonResponse(appTestState.settingsResponse.update);
  }

  if (path === "/api/settings/spec/jobs") {
    const page = Math.max(1, Number(requestUrl.searchParams.get("page") ?? "1") || 1);
    const pageSize = Math.min(
      100,
      Math.max(1, Number(requestUrl.searchParams.get("pageSize") ?? requestUrl.searchParams.get("limit") ?? "20") || 20),
    );
    const retryableOnly = requestUrl.searchParams.get("retryableOnly") === "true";
    const source = retryableOnly
      ? appTestState.settingsSpecJobsResponse.filter(
        (item) =>
          item.job.status === "queued" ||
          item.job.status === "running" ||
          (item.job.status === "failed" && !item.job.hasRetry),
      )
      : appTestState.settingsSpecJobsResponse;
    const totalCount = source.length;
    const totalPages = totalCount ? Math.ceil(totalCount / pageSize) : 0;
    const offset = (page - 1) * pageSize;
    return jsonResponse({
      errors: [],
      jobs: source.slice(offset, offset + pageSize),
      page,
      pageSize,
      totalCount,
      totalPages,
    });
  }

  if (path === "/api/skill-store/browse") {
    const page = skillStoreListPage(skillStoreHotSkills, requestUrl);
    return jsonResponse({
      ...page,
      source: "test-browse",
    });
  }

  if (path === "/api/skill-store/hot") {
    return jsonResponse({
      hasMore: false,
      skills: skillStoreHotSkills.slice(0, 20),
      source: "test-hot",
      total: skillStoreHotSkills.length,
    });
  }

  if (path === "/api/skill-store/search") {
    const page = skillStoreListPage(skillStoreSearchSkills, requestUrl);
    return jsonResponse({
      ...page,
      source: "test-search",
    });
  }

  const skillStoreDetailMatch = path.match(/^\/api\/skill-store\/skills\/([^/]+)$/);
  if (skillStoreDetailMatch) {
    const skillId = decodeURIComponent(skillStoreDetailMatch[1] ?? "");
    const source = requestUrl.searchParams.get("source");
    return jsonResponse({
      description: "Find useful web references.",
      files: skillStoreFiles,
      id: skillId,
      name: "Browser Scout",
      source,
    });
  }

  if (path === "/api/skill-store/import-preview") {
    return jsonResponse({
      description: "Create HTML presentations from notes.",
      files: [
        {
          path: "SKILL.md",
          content:
            "---\nname: html-ppt\ndescription: Create HTML presentations from notes.\n---\n\n# HTML PPT\n",
        },
        {
          path: "assets/logo.png",
          content: "iVBORwD/AA==",
          contentEncoding: "base64",
        },
      ],
      id: "html-ppt",
      name: "HTML PPT",
      source: "foco/html-ppt",
    });
  }

  if (path === "/api/skill-store/install") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      target?: string;
      workspaceId?: string;
    };
    return jsonResponse({
      detected: [installedSkillFromStore(body.workspaceId ?? null)],
      path:
        body.target === "workspace"
          ? "C:\\Repos\\Default\\.agents\\skills\\browser-scout"
          : "C:\\Users\\fonla\\.agents\\skills\\browser-scout",
      target: body.target ?? "global",
      workspaceId: body.workspaceId ?? null,
    });
  }

  if (path === "/api/skill-store/translate") {
    return jsonResponse({ translatedContent: "Translated SKILL.md summary" });
  }

  const specJobRetryMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/spec\/jobs\/([^/]+)\/retry$/,
  );
  if (specJobRetryMatch) {
    const workspaceId = decodeURIComponent(specJobRetryMatch[1] ?? "");
    const jobId = decodeURIComponent(specJobRetryMatch[2] ?? "");
    const source = appTestState.settingsSpecJobsResponse.find(
      (item) => item.workspaceId === workspaceId && item.job.id === jobId,
    );
    const retryJob: WorkspaceSpecJobSummary = {
      ...(source?.job ?? workspaceSpecQueuedJob),
      completedAt: null,
      createdAt: "2026-06-11T03:12:00Z",
      errorMessage: null,
      hasRetry: false,
      id: `${jobId}-retry`,
      output: null,
      startedAt: null,
      status: "queued",
    };
    appTestState.settingsSpecJobsResponse = source
      ? [
        { ...source, job: retryJob },
        ...appTestState.settingsSpecJobsResponse.map((item) =>
          item === source ? { ...item, job: { ...item.job, hasRetry: true } } : item,
        ),
      ]
      : appTestState.settingsSpecJobsResponse;
    return jsonResponse({ job: retryJob });
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
    return jsonResponse(appTestState.agentTeamSnapshotResponse);
  }

  const agentTranscriptMatch = path.match(
    /^\/api\/workspaces\/workspace-1\/agent-team\/instances\/([^/]+)\/transcript$/,
  );
  if (agentTranscriptMatch) {
    return jsonResponse(agentTranscriptResponse);
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

  if (path === "/api/workspaces/workspace-1/spec") {
    if (init?.method === "PUT") {
      if (appTestState.workspaceSpecSaveConflict) {
        return jsonResponse(
          { error: "workspace spec revision changed; reload and retry" },
          { status: 409 },
        );
      }
      const body = JSON.parse(String(init.body ?? "{}")) as {
        contentMarkdown: string;
        expectedRevision: number;
      };
      setWorkspaceSpecResponseForWorkspace(workspace.id, {
        ...appTestState.workspaceSpecResponse,
        contentMarkdown: body.contentMarkdown,
        latestJob: appTestState.workspaceSpecResponse.latestJob,
        revision: body.expectedRevision + 1,
        updatedAt: "2026-06-11T03:10:00Z",
      });
    }
    return jsonResponse(appTestState.workspaceSpecResponse);
  }

  const workspaceSpecMatch = path.match(/^\/api\/workspaces\/([^/]+)\/spec$/);
  if (workspaceSpecMatch) {
    const workspaceId = decodeURIComponent(workspaceSpecMatch[1] ?? "");
    return jsonResponse(workspaceSpecResponseForWorkspace(workspaceId));
  }

  const workspaceSpecSettingsMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/spec\/settings$/,
  );
  if (workspaceSpecSettingsMatch && init?.method === "PUT") {
    const queuedResponse = appTestState.workspaceSpecSettingsResponses.shift();
    if (queuedResponse) {
      return await queuedResponse;
    }

    const workspaceId = decodeURIComponent(workspaceSpecSettingsMatch[1] ?? "");
    const body = JSON.parse(String(init.body ?? "{}")) as {
      enabled: boolean;
      injectEnabled: boolean;
    };
    const response = {
      ...workspaceSpecResponseForWorkspace(workspaceId),
      settings: {
        enabled: body.enabled,
        injectEnabled: body.injectEnabled,
      },
    };
    setWorkspaceSpecResponseForWorkspace(workspaceId, response);
    return jsonResponse(response);
  }

  const workspaceSpecGenerateMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/spec\/generate$/,
  );
  if (workspaceSpecGenerateMatch) {
    const workspaceId = decodeURIComponent(workspaceSpecGenerateMatch[1] ?? "");
    const response = workspaceSpecResponseForWorkspace(workspaceId);
    const generatedJob =
      appTestState.workspaceSpecGenerateCompletes ||
      appTestState.workspaceSpecJobPollsBeforeCompletion !== null
        ? { ...workspaceSpecQueuedJob, status: "running" as const }
        : workspaceSpecQueuedJob;
    appTestState.workspaceSpecJobPollCount = 0;
    setWorkspaceSpecResponseForWorkspace(workspaceId, {
      ...response,
      latestJob: generatedJob,
    });
    return jsonResponse({ job: generatedJob });
  }

  const workspaceSpecJobsMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/spec\/jobs$/,
  );
  if (workspaceSpecJobsMatch) {
    const workspaceId = decodeURIComponent(workspaceSpecJobsMatch[1] ?? "");
    if (appTestState.workspaceSpecJobPollFailuresRemaining > 0) {
      appTestState.workspaceSpecJobPollFailuresRemaining -= 1;
      return jsonResponse(
        { error: "temporary remote spec proxy failure" },
        { status: 502 },
      );
    }

    appTestState.workspaceSpecJobPollCount += 1;
    const pollsBeforeCompletion = appTestState.workspaceSpecGenerateCompletes
      ? 0
      : appTestState.workspaceSpecJobPollsBeforeCompletion;
    const shouldComplete =
      pollsBeforeCompletion !== null &&
      appTestState.workspaceSpecJobPollCount > pollsBeforeCompletion;
    const response = workspaceSpecResponseForWorkspace(workspaceId);
    if (shouldComplete && response.latestJob?.status !== "completed") {
      setWorkspaceSpecResponseForWorkspace(workspaceId, {
        ...response,
        contentMarkdown:
          "# Project Spec\n\n## Purpose\n\nRegenerated by the LLM.",
        generatedAt: "2026-06-11T03:15:00Z",
        latestJob: { ...workspaceSpecQueuedJob, status: "completed" },
        revision: response.revision + 1,
        updatedAt: "2026-06-11T03:15:00Z",
      });
    }
    return jsonResponse({
      jobs: [workspaceSpecResponseForWorkspace(workspaceId).latestJob],
    });
  }

  if (path === "/api/settings/general") {
    const savedSettings = savedGeneralSettings(init);
    appTestState.settingsResponse = savedSettings;
    return jsonResponse(savedSettings);
  }

  if (path === "/api/settings/memory") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      contextBudgetPercent?: number;
      dream?: typeof settings.memory.dream;
      extractionLlmTimeoutMs?: number;
      extractionModelId?: string | null;
      retentionDays?: number | null;
      retrievalLlmTimeoutMs?: number;
      retrievalModelId?: string | null;
    };
    return jsonResponse({
      ...settings,
      memory: {
        ...settings.memory,
        enabled: true,
        extractionMode: "pending_review",
        retrievalMode: "llm",
        extractionModelId: body.extractionModelId ?? "gpt-test",
        retrievalModelId: body.retrievalModelId ?? "gpt-test",
        extractionLlmTimeoutMs:
          body.extractionLlmTimeoutMs ?? settings.memory.extractionLlmTimeoutMs,
        retrievalLlmTimeoutMs:
          body.retrievalLlmTimeoutMs ?? settings.memory.retrievalLlmTimeoutMs,
        contextBudgetPercent:
          body.contextBudgetPercent ?? settings.memory.contextBudgetPercent,
        retentionDays: body.retentionDays ?? 30,
        dream: body.dream ?? settings.memory.dream,
      },
    });
  }

  if (path === "/api/settings/spec") {
    const body = JSON.parse(String(init?.body ?? "{}")) as typeof settings.spec;
    return jsonResponse({
      ...settings,
      spec: {
        ...settings.spec,
        autoEnabled: body.autoEnabled,
        generationModelId: body.generationModelId ?? null,
        generationSystemPrompt: body.generationSystemPrompt ?? null,
        updateSystemPrompt: body.updateSystemPrompt ?? null,
        llmTimeoutMs: body.llmTimeoutMs,
      },
    });
  }

  if (path === "/api/settings/plan") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      mergeAutomationMode?: string;
      modeModelId?: string | null;
    };
    const nextSettings = {
      ...appTestState.settingsResponse,
      plan: {
        ...appTestState.settingsResponse.plan,
        mergeAutomationMode:
          typeof body.mergeAutomationMode === "string" && body.mergeAutomationMode.trim()
            ? body.mergeAutomationMode.trim()
            : appTestState.settingsResponse.plan.mergeAutomationMode,
        modeModelId:
          typeof body.modeModelId === "string" && body.modeModelId.trim()
            ? body.modeModelId.trim()
            : null,
      },
    };
    appTestState.settingsResponse = nextSettings;
    return jsonResponse(nextSettings);
  }

  if (path === "/api/settings/prompts") {
    const body = JSON.parse(String(init?.body ?? "{}")) as {
      contextCompressionSystemPrompt?: string | null;
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
    const hasCompressionField = Object.prototype.hasOwnProperty.call(
      body,
      "contextCompressionSystemPrompt",
    );
    const nextCompressionOverride = hasCompressionField
      ? typeof body.contextCompressionSystemPrompt === "string" &&
        body.contextCompressionSystemPrompt.trim()
        ? body.contextCompressionSystemPrompt
        : null
      : (appTestState.settingsResponse.prompts.contextCompressionSystemPrompt ?? null);
    const nextSettings = {
      ...appTestState.settingsResponse,
      prompts: {
        ...appTestState.settingsResponse.prompts,
        defaultContextCompressionSystemPrompt:
          appTestState.settingsResponse.prompts.defaultContextCompressionSystemPrompt ??
          settings.prompts.defaultContextCompressionSystemPrompt,
        contextCompressionSystemPrompt: nextCompressionOverride,
        extraText: body.extraText ?? "",
        files: body.files ?? ([] as string[]),
        systemPrompt: null as string | null,
        systemPrompts,
      },
    } satisfies typeof appTestState.settingsResponse;
    appTestState.settingsResponse = nextSettings;
    return jsonResponse(nextSettings);
  }

  if (path === "/api/memory") {
    const status = requestUrl.searchParams.get("status");
    const scope = requestUrl.searchParams.get("scope");
    const chatId = requestUrl.searchParams.get("chatId");
    const page = Number(requestUrl.searchParams.get("page") ?? "1");
    const pageSize = Number(requestUrl.searchParams.get("pageSize") ?? "20");
    const memories = [
      ...(status === "pending"
        ? [appTestState.memoriesById[pendingMemory.id]]
        : scope === "chat"
          ? [appTestState.memoriesById[chatMemory.id]]
        : scope === "workspace"
          ? [appTestState.memoriesById[workspaceMemory.id]]
          : [appTestState.memoriesById[activeMemory.id]]),
      ...appTestState.memoryListAdditional.filter(
        (memory) => memory.scope === scope && memory.status === status,
      ),
    ];
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

  if (path === "/api/memory/enabled") {
    const queuedResponse = appTestState.memoryEnabledResponses.shift();
    if (queuedResponse) {
      return await queuedResponse;
    }

    const body = JSON.parse(String(init?.body ?? "{}")) as {
      enabled?: boolean;
      factId?: string;
    };
    const memory = body.factId ? appTestState.memoriesById[body.factId] : undefined;
    if (!memory || typeof body.enabled !== "boolean") {
      return jsonResponse({ message: "memory fact was not found" }, { status: 404 });
    }
    const updatedMemory = { ...memory, enabled: body.enabled };
    appTestState.memoriesById[memory.id] = updatedMemory;
    return jsonResponse({ memory: updatedMemory });
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

  if (path === "/api/memory/dream/jobs") {
    const queuedResponse = appTestState.memoryDreamJobsResponses.shift();
    if (queuedResponse) {
      return jsonResponse(queuedResponse);
    }

    const page = Number(requestUrl.searchParams.get("page") ?? "1");
    const pageSize = Math.min(
      200,
      Math.max(1, Number(requestUrl.searchParams.get("pageSize") ?? "10") || 10),
    );
    const jobs = [memoryDreamJob, failedMemoryDreamJob];
    const totalCount = jobs.length;
    const offset = (page - 1) * pageSize;
    return jsonResponse({
      jobs: jobs.slice(offset, offset + pageSize),
      page,
      pageSize,
      totalCount,
      totalPages: totalCount ? Math.ceil(totalCount / pageSize) : 0,
    });
  }

  if (path === "/api/memory/dream/run") {
    return jsonResponse({
      job: { ...memoryDreamJob, completedAt: null, status: "running" },
      jobId: memoryDreamJob.id,
      status: "running",
      transcriptChatId: memoryDreamJob.transcriptChatId,
    });
  }

  if (path === `/api/memory/dream/jobs/${memoryDreamJob.id}/changes`) {
    return jsonResponse({ changes: [memoryDreamChange] });
  }

  if (path === `/api/memory/dream/jobs/${failedMemoryDreamJob.id}/changes`) {
    return jsonResponse({ changes: [] });
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

  if (path === "/api/workspaces/manual") {
    return jsonResponse(saveManualWorkspaceSettings(init));
  }

  if (path === "/api/workspaces/order") {
    const body = JSON.parse(String(init?.body ?? "{}")) as { workspaceIds?: string[] };
    return jsonResponse(workspaceOrderSettings(body.workspaceIds ?? []));
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
              : "/api/workspaces/workspace-1/logo/thumbnail?v=2",
        },
      ],
    });
  }

  if (path === "/api/model-metadata") {
    return jsonResponse({
      cachePath: "C:\\Users\\fonla\\.foco\\models.dev.json",
      configuredModels: appTestState.settingsResponse.configuredModels,
      fetchedAt: "2026-06-05T10:00:00Z",
      models: savedModelMetadata.models,
      sourceUrl: "https://models.dev/api.json",
    });
  }

  if (path === "/api/providers/manual") {
    return jsonResponse(saveManualProviderSettings(init));
  }

  if (path === "/api/providers/delete") {
    return jsonResponse(deleteProviderSettings(init));
  }

  if (path === "/api/providers/reveal-api-key") {
    return jsonResponse({ apiKey: "sk-saved" });
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
      models: ["gpt-4.1"],
    });
  }

  if (path === "/api/models/manual") {
    return jsonResponse(savedModelMetadata);
  }

  if (path === "/api/mcp/servers/manual") {
    return jsonResponse(savedSettings.mcp);
  }

  if (path === "/api/skills/manual") {
    return jsonResponse(savedSkillsSettings(init));
  }

  if (path === "/api/skills/delete") {
    return jsonResponse(deletedSkillSettings(init));
  }

  if (path === "/api/skills/refresh") {
    return jsonResponse(skillStoreRefreshedSettings(null));
  }

  if (path === "/api/skill-store/update") {
    return jsonResponse(skillStoreUpdateSettings(init));
  }

  if (path === "/api/skill-store/update-all") {
    return jsonResponse(skillStoreUpdateAllSettings());
  }

  if (path === "/api/ai-statistics") {
    const page = Number(requestUrl.searchParams.get("page") ?? aiStatistics.page);
    const pageSize = Number(
      requestUrl.searchParams.get("pageSize") ?? aiStatistics.pageSize,
    );
    return jsonResponse({
      ...aiStatistics,
      page: Number.isSafeInteger(page) && page > 0 ? page : aiStatistics.page,
      pageSize:
        Number.isSafeInteger(pageSize) && pageSize > 0
          ? pageSize
          : aiStatistics.pageSize,
    });
  }

  if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
    return jsonResponse(aiStatisticsDetail);
  }

  if (path === "/api/workspaces/workspace-1/git/branches") {
    return jsonResponse(
      appTestState.workspaceGitBranchesResponses.shift() ?? {
        branches: ["main"],
        currentBranch: "main",
        isGitRepository: true,
        worktrees: [
          {
            branch: "main",
            isCurrent: true,
            name: "workspace",
            path: "C:/Users/fonla/.foco/workspace",
          },
        ],
      },
    );
  }

  if (path === "/api/workspaces/workspace-1/git/diff") {
    const selectedPath = requestUrl.searchParams.get("path");
    const worktreePath = requestUrl.searchParams.get("worktreePath");
    const targetResponse =
      worktreePath && appTestState.workspaceGitDiffResponsesByWorktreePath[worktreePath]
        ? appTestState.workspaceGitDiffResponsesByWorktreePath[worktreePath]
        : appTestState.workspaceGitDiffResponse;
    if (!selectedPath) {
      return jsonResponse(targetResponse);
    }

    const file = targetResponse.files.find(
      (summary) => summary.path === selectedPath,
    );
    const escapedPath = selectedPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const sectionMatch = targetResponse.diff.match(
      new RegExp(`diff --git a/${escapedPath} b/${escapedPath}[\\s\\S]*?(?=diff --git a/|$)`),
    );

    return jsonResponse({
      ...targetResponse,
      diff: sectionMatch?.[0] ?? "",
      files: targetResponse.files,
      path: selectedPath,
      status: file
        ? `${file.indexStatus}${file.worktreeStatus} ${file.path}\n`
        : targetResponse.status,
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

  const contextUsageMatch = path.match(/^\/api\/workspaces\/([^/]+)\/context-usage$/);
  if (contextUsageMatch) {
    const workspaceId = decodeURIComponent(contextUsageMatch[1] ?? "");
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as {
            chatId?: string | null;
          })
        : {};
    const responses = appTestState.contextUsageResponseQueuesByChatKey[
      `${workspaceId}/${body.chatId ?? ""}`
    ];
    const configuredResponse = responses?.shift();
    if (configuredResponse) {
      return jsonResponse(configuredResponse);
    }

    const isSecondLocalChat = workspaceId === workspace.id && body.chatId === "chat-2";
    return jsonResponse({
      ...contextUsage,
      usagePercent: isSecondLocalChat ? 23 : contextUsage.usagePercent,
      usedMessageTokens: isSecondLocalChat ? 25520 : contextUsage.usedMessageTokens,
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

  if (path === "/api/workspaces/workspace-1/chats/chat-1/messages/message-user/edit") {
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { message?: string; sessionMode?: string | null })
        : {};
    return jsonResponse({
      assistantMessageId: "message-assistant-edited",
      assistantSequence: 1,
      cancelledAgentTaskIds: [],
      chatId: "chat-1",
      completedMemoriesPreserved: true,
      content: body.message ?? "",
      invalidatedRunIds: ["request-1", "request-2"],
      parts: [{ text: body.message ?? "", type: "text" }],
      removedMessageIds: ["message-assistant"],
      sessionMode: body.sessionMode ?? null,
      skippedMemoryExtractionJobIds: [],
      skippedWorkspaceSpecJobIds: [],
      userMessageId: "message-user",
    });
  }

  if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
    const override =
      appTestState.chatMessagesResponsesByChatKey["workspace-1/chat-1"];
    return jsonResponse({ ...(override ?? chatMessages), activeRun: null });
  }

  if (path === "/api/workspaces/workspace-1/chats/dream-transcript-chat-1/messages") {
    return jsonResponse({ ...dreamTranscriptChatMessages, activeRun: null });
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

  const configuredChatMessagesMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/chats\/([^/]+)\/messages$/,
  );
  if (configuredChatMessagesMatch) {
    const workspaceId = decodeURIComponent(configuredChatMessagesMatch[1] ?? "");
    const chatId = decodeURIComponent(configuredChatMessagesMatch[2] ?? "");
    const response = appTestState.chatMessagesResponsesByChatKey[`${workspaceId}/${chatId}`];
    if (response) {
      return jsonResponse({ ...response, activeRun: null });
    }
  }

  const configuredChatStatisticsMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/chats\/([^/]+)\/statistics$/,
  );
  if (configuredChatStatisticsMatch) {
    const workspaceId = decodeURIComponent(configuredChatStatisticsMatch[1] ?? "");
    const chatId = decodeURIComponent(configuredChatStatisticsMatch[2] ?? "");
    const response = appTestState.chatStatisticsResponsesByChatKey[`${workspaceId}/${chatId}`];
    if (response) {
      return jsonResponse(response);
    }
  }

  const workspaceChatDeleteMatch = path.match(
    /^\/api\/workspaces\/([^/]+)\/chats\/([^/]+)\/delete$/,
  );
  if (workspaceChatDeleteMatch && init?.method === "POST") {
    const workspaceId = decodeURIComponent(workspaceChatDeleteMatch[1] ?? "");
    const chatId = decodeURIComponent(workspaceChatDeleteMatch[2] ?? "");
    const nextChats = persistedWorkspaceChats(workspaceId).filter((chat) => chat.id !== chatId);
    appTestState.workspaceChatsByWorkspaceId = {
      ...appTestState.workspaceChatsByWorkspaceId,
      [workspaceId]: nextChats,
    };
    appTestState.workspaceResponseWorkspaces = appTestState.workspaceResponseWorkspaces.map(
      (item) => {
        const workspaceSummary = item as {
          chatPagination?: { limit?: number };
          chats?: Array<(typeof workspaceChats)[number]>;
          id?: string;
        };
        if (workspaceSummary.id !== workspaceId) {
          return item;
        }

        const limit = workspaceSummary.chatPagination?.limit ?? 5;
        return {
          ...(item as object),
          chatPagination: {
            hasMore: nextChats.length > limit,
            limit,
            nextCursor: nextChats.length > limit ? "workspace-page-2" : null,
            total: nextChats.length,
          },
          chats: nextChats.slice(0, limit),
        };
      },
    );
    return jsonResponse({ deleted: true, chatId });
  }

  const chatStreamMatch = path.match(/^\/api\/workspaces\/([^/]+)\/chat\/stream$/);
  if (chatStreamMatch) {
    const workspaceId = decodeURIComponent(chatStreamMatch[1] ?? "");
    const body =
      typeof init?.body === "string"
        ? (JSON.parse(init.body) as { chatId?: string | null })
        : {};
    return chatStreamResponse(
      body.chatId ?? (workspaceId === "workspace-2" ? "side-chat-stream" : "chat-1"),
    );
  }
  const chatQueueMatch = path.match(/^\/api\/workspaces\/([^/]+)\/chat\/queue$/);
  if (chatQueueMatch) {
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

  const chatRunCancelMatch = path.match(
    /^\/api\/workspaces\/[^/]+\/chat\/runs\/([^/]+)\/cancel$/,
  );
  if (chatRunCancelMatch) {
    return jsonResponse({
      ok: true,
      runId: decodeURIComponent(chatRunCancelMatch[1] ?? ""),
    });
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

export function enqueueChatStreamEventForRun(
  runId: string,
  value: unknown,
  options: { id?: number | string } = {},
) {
  const controller = appTestState.chatStreamControllers.get(runId);
  if (!controller) {
    throw new Error(`chat stream is not active: ${runId}`);
  }

  const encoder = new TextEncoder();
  const idLine = options.id === undefined ? "" : `id: ${options.id}\n`;
  controller.enqueue(encoder.encode(`${idLine}data: ${JSON.stringify(value)}\n\n`));
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
