export type AppLanguageId = "en" | "zh-CN";

export type AppThemeId = "light" | "dark";

export type SettingsSection =
  | "general"
  | "agents"
  | "prompts"
  | "web-search"
  | "hooks"
  | "memory"
  | "mcp"
  | "models"
  | "providers"
  | "skills"
  | "workspaces";

export type BrowserRouteChatTab = {
  workspaceId: string;
  chatId: string;
};

export type BrowserRouteFileTab = {
  workspaceId: string;
  path: string;
};

export type BrowserRoute =
  | {
      viewMode: "chat";
      workspaceId: string | null;
      chatId: string | null;
      tabs?: BrowserRouteChatTab[];
      files?: BrowserRouteFileTab[];
      activeFile?: BrowserRouteFileTab;
    }
  | { viewMode: "settings"; section: SettingsSection }
  | { viewMode: "stats"; page: number }
  | { viewMode: "scheduled" };

export type Translate = (
  key: string,
  values?: Record<string, string | number>,
) => string;

// Git types

export type GitStatusFileSummary = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
};

export type GitDiffResponse = {
  path: string | null;
  status: string;
  diff: string;
  stagedDiff: string;
  files: GitStatusFileSummary[];
  stagedFiles: GitStatusFileSummary[];
};

export type GitCommitMessageResponse = {
  message: string;
};

export type GitDiffLineStats = {
  additions: number;
  deletions: number;
};

export type GitBranchesResponse = {
  isGitRepository: boolean;
  currentBranch: string | null;
  branches: string[];
};

export type WorkspaceFileTreeNode = {
  name: string;
  path: string;
  kind: "directory" | "file";
  sizeBytes: number;
  hasChildren: boolean;
  childrenLoaded: boolean;
  children: WorkspaceFileTreeNode[];
};

export type WorkspaceFileContentResponse = {
  content: string;
  path: string;
};

export type WorkspaceFileSaveResponse = {
  content: string;
  path: string;
};

export type WorkspaceFilesResponse = {
  root: WorkspaceFileTreeNode;
};

export type WorkspaceFileChildrenResponse = {
  path: string;
  children: WorkspaceFileTreeNode[];
};

// JSON types

export type JsonValue =
  | boolean
  | null
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

// Chat types

export type QueuedRunSummary = {
  status: "queued" | "running" | string;
  userMessageId: string;
  assistantMessageId: string | null;
  assistantSequence?: number | null;
  modelId?: string | null;
  providerId: string | null;
  thinkingLevel: string | null;
  skillIds: string[];
  content?: string | null;
};

export type ActiveChatRunSummary = {
  runId: string;
  workspaceId: string;
  chatId: string;
  lastSequence: number | null;
  acceptingGuidance: boolean;
};

export type ChatSummary = {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  codeChangeStats: GitDiffLineStats;
  activeRun: ActiveChatRunSummary | null;
  queuedRun: QueuedRunSummary | null;
};

export type ChatUsage = {
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
};

export type ChatReplyMetrics = {
  modelId: string;
  providerId: string;
  totalLatencyMs: number | null;
  firstTokenLatencyMs: number | null;
  outputTokens: number | null;
};

export type ChatMemoryUsedSummary = {
  id: string;
  scope: string;
  chatId: string | null;
  kind: string;
  fact: string;
  pinned: boolean;
  source: string;
};

export type ChatExtractedMemorySummary = {
  id: string;
  scope: string;
  chatId: string | null;
  status: string;
  kind: string;
  fact: string;
};

export type ChatToolLiveOutput = {
  stdout: string;
  stderr: string;
};

export type ChatToolCallSummary = {
  id: string;
  name: string;
  status: string;
  input: JsonValue;
  output: JsonValue | null;
  isError: boolean;
  liveOutput?: ChatToolLiveOutput;
};

export type ChatAttachmentPartSummary = {
  id: string;
  name: string;
  contentType: string;
  sizeBytes: number;
  path: string | null;
  previewDataUrl: string | null;
};

export type ChatMessagePart =
  | { type: "text"; text: string }
  | { type: "error"; text: string }
  | {
      type: "reasoning";
      text: string;
      durationMs?: number;
      liveDurationMs?: number;
      startedAtMs?: number;
    }
  | { type: "attachment"; attachment: ChatAttachmentPartSummary }
  | { type: "toolCall"; toolCall: ChatToolCallSummary };

export type ChatAttachmentPayload = {
  id: string;
  name: string;
  contentType: string;
  contentBase64?: string;
  path?: string;
  sizeBytes: number;
};

export type ComposerAttachment = ChatAttachmentPayload & {
  previewDataUrl: string | null;
};

export type NativeSelectedFile = {
  path: string;
  name: string;
  contentType: string;
  sizeBytes: number;
  contentBase64?: string | null;
};

export type QueuedMessageRunSummary = {
  status: "queued" | "running" | string;
  modelId: string;
  providerId: string | null;
  thinkingLevel: string | null;
  skillIds: string[];
  assistantMessageId: string | null;
  assistantSequence?: number | null;
};

export type ChatRunBadge =
  | "contextCompressionRule"
  | "contextCompressionLlm"
  | "llmReconnect";

export type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
  createdAt: string;
  reasoning: string | null;
  pendingMode?: "guidance" | "queued";
  queuedRun?: QueuedMessageRunSummary | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
};

export type ChatMessagesChatSummary = {
  id: string;
  title: string;
  kind?: string | null;
  readOnly: boolean;
};

export type QueueChatMessageResponse = {
  chatId: string;
  chatTitle: string;
  createdAt: string;
  updatedAt: string;
  userMessageId: string;
  assistantMessageId: string;
  content: string;
  parts: ChatMessagePart[];
  agentTeamId?: string;
  agentTaskId?: string;
};

export type ChatMessagesResponse = {
  chat?: ChatMessagesChatSummary | null;
  messages: ChatMessageSummary[];
  activeRun?: ActiveChatRunSummary | null;
};

export type QuestionOptionSummary = {
  label: string;
  value: string;
  description: string | null;
};

export type QuestionItemSummary = {
  id: string;
  question: string;
  options: QuestionOptionSummary[];
  allowFreeText: boolean;
};

export type QuestionRequestSummary = {
  id: string;
  toolCallId: string;
  workspaceId: string;
  chatId: string;
  questions: QuestionItemSummary[];
};

export type QuestionAnswerSubmission = {
  answers: {
    id: string;
    answer: string;
    selectedOptionValue: string | null;
  }[];
};

export type HookNotificationSummary = {
  event: string;
  level: string;
  message: string;
};

export type ChatStreamEvent =
  | {
    type: "start";
    chatId: string;
    userMessageId: string;
    assistantMessageId: string;
    llmRequestId?: string;
    memoriesUsed: ChatMemoryUsedSummary[];
  }
  | { type: "textDelta"; assistantMessageId?: string; delta: string }
  | { type: "reasoningDelta"; assistantMessageId?: string; delta: string }
  | {
    type: "streamAttemptStart";
    assistantMessageId: string;
    llmRequestId: string;
  }
  | {
    type: "streamReset";
    assistantMessageId: string;
    reason: string;
    text: string;
    reasoning: string | null;
    toolCalls: ChatToolCallSummary[];
  }
  | {
    type: "contextCompression";
    assistantMessageId: string;
    snapshotId: string;
    kind: "rule" | "llm";
  }
  | { type: "usage"; usage?: ChatUsage }
  | {
    type: "complete";
    chatId: string;
    assistantMessageId: string;
    text: string;
    reasoning?: string | null;
    usage?: ChatUsage | null;
    stopReason?: string | null;
    metrics: ChatReplyMetrics;
    memoriesUsed: ChatMemoryUsedSummary[];
  }
  | { type: "streamEnd" }
  | {
    type: "toolCall";
    assistantMessageId: string;
    toolCall: ChatToolCallSummary;
  }
  | {
    type: "toolResult";
    assistantMessageId: string;
    toolCallId: string;
    output: JsonValue;
    isError: boolean;
  }
  | {
    type: "toolOutputDelta";
    assistantMessageId: string;
    toolCallId: string;
    stream: "stdout" | "stderr";
    delta: string;
  }
  | {
    type: "questionRequest";
    assistantMessageId: string;
    request: QuestionRequestSummary;
  }
  | {
    type: "hookNotification";
    assistantMessageId: string;
    notification: HookNotificationSummary;
  }
  | {
    type: "guidanceApplied";
    id: string;
    content: string;
    parts: ChatMessagePart[];
    interruptedAssistantMetrics: ChatReplyMetrics | null;
  }
  | {
    type: "gitDiffRefresh";
    workspaceId: string;
    codeChangeStats: GitDiffLineStats;
  }
  | {
    type: "todoGraphRefresh";
    workspaceId: string;
    chatId: string;
  }
  | {
    type: "agentTeamRefresh";
    workspaceId: string;
    chatId: string;
    teamId: string;
    instanceId?: string;
    reason: string;
    revealPanel: boolean;
  }
  | {
    type: "memoryExtractionComplete";
    assistantMessageId: string;
    extractedMemories: ChatExtractedMemorySummary[];
  }
  | {
    type: "memoryResolved";
    assistantMessageId: string;
    memoriesUsed: ChatMemoryUsedSummary[];
    agentTeamId?: string;
    agentInstanceId?: string;
    agentTaskId?: string;
  }
  | { type: "error"; message: string };

export type ChatToolBreakdown = {
  toolName: string;
  callCount: number;
};

export type ChatCompressionStatistics = {
  snapshotCount: number;
  ruleSnapshotCount: number;
  llmSnapshotCount: number;
  originalTokenCount: number;
  summaryTokenCount: number;
  savedTokenCount: number;
};

export type AiStatisticsModelBreakdown = {
  modelId: string;
  requestCount: number;
  totalTokens: number;
};

export type AiStatisticsProviderBreakdown = {
  averageLatencyMs: number | null;
  failedCount: number;
  providerId: string;
  requestCount: number;
  successCount: number;
  successRate: number | null;
  totalTokens: number;
};

export type ChatStatisticsResponse = {
  workspaceId: string;
  chatId: string;
  messageCount: number;
  userMessageCount: number;
  assistantMessageCount: number;
  toolMessageCount: number;
  totalRequests: number;
  failedRequests: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheWriteTokens: number;
  totalTokens: number;
  totalLatencyMs: number;
  averageLatencyMs: number | null;
  memoryReferences: number;
  createdMemories: number;
  codeChangeStats: GitDiffLineStats;
  modelBreakdown: AiStatisticsModelBreakdown[];
  providerBreakdown: AiStatisticsProviderBreakdown[];
  toolBreakdown: ChatToolBreakdown[];
  compression: ChatCompressionStatistics;
};

export type LiveChatStatistics = {
  usage: ChatUsage | null;
  modelId: string;
  providerId: string;
  startedAtMs: number;
  codeChangeStats?: GitDiffLineStats;
};

// Context types

type ContextTokenBreakdown = {
  requiredTokens: number;
  optionalTokens: number;
  compressibleTokens: number;
  bySource: ContextSourceTokenBreakdown[];
};

type ContextSourceTokenBreakdown = {
  source: string;
  tokens: number;
  requiredTokens: number;
  optionalTokens: number;
  compressibleTokens: number;
};

export type ContextUsageResponse = {
  usedMessageTokens: number;
  availableMessageTokens: number;
  memoryContextTokens: number;
  memoryBudgetTokens: number;
  usagePercent: number;
  compressionTriggerTokens: number;
  compressionTriggerPercent: number;
  willCompressOnNextSend: boolean;
  tokenBreakdown: ContextTokenBreakdown;
};

export type ContextUsageRefreshRequest = {
  workspaceId: string;
  chatId: string | null;
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
  latestResponseUsage: ChatUsage;
};

export type ContextMemoryScopeState = {
  memories: MemoryFactRecord[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

export type ContextMemoryState = {
  global: ContextMemoryScopeState;
  workspace: ContextMemoryScopeState;
};

// Workspace types

export type WorkspaceCommonCommandSummary = {
  name: string;
  command: string;
};

export type WorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  logoUrl: string | null;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
  chats: ChatSummary[];
};

export type WorkspaceChatListItem = ChatSummary & {
  scheduledChatKey?: string;
  scheduledRunId?: string;
  scheduledStatus?: ScheduledWorkspaceRun["status"];
};

export type WorkspacesResponse = {
  activeWorkspaceId: string;
  workspaces: WorkspaceSummary[];
};

export type ConfiguredWorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  logoUrl: string | null;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
  isDefault: boolean;
};

export type WorkspaceFormState = {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
};

export type WorkspaceIconDraft = {
  contentBase64: string;
  dataUrl?: string;
  name: string;
  previewUrl: string;
};

// Model types

type ModelPricing = {
  input: number | null;
  output: number | null;
  reasoning: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
};

export type ModelMetadataRecord = {
  key: string;
  providerId: string;
  providerName: string;
  modelId: string;
  name: string;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  pricing: ModelPricing;
  inputModalities: string[];
  outputModalities: string[];
  supportsTools: boolean;
  supportsCache: boolean;
  sourceUrl: string;
  refreshedAt: string;
};

export type ConfiguredModelSummary = {
  id: string;
  displayName: string;
  enabled: boolean;
  metadataKey: string | null;
  metadataSourceUrl: string | null;
  metadataRefreshedAt: string | null;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  canEnable: boolean;
  missingLimits: string[];
  providerIds: string[];
  activeProviderId: string | null;
  thinkingLevel: string | null;
  systemPromptName: string;
  supportsThinking: boolean;
  warnings: string[];
};

export type ModelMetadataResponse = {
  sourceUrl: string | null;
  fetchedAt: string | null;
  cachePath: string;
  models: ModelMetadataRecord[];
  configuredModels: ConfiguredModelSummary[];
};

export type ModelFormState = {
  displayName: string;
  enabled: boolean;
  maxOutputTokens: string;
  modelId: string;
  contextWindow: string;
  providerIds: string[];
  activeProviderId: string;
  thinkingLevel: string;
  systemPromptName: string;
};

export type ThinkingLevelSummary = {
  value: string;
  label: string;
};

// Agent types

export type AgentModelOptions = {
  thinkingLevel?: string | null;
  maxOutputTokens?: number | null;
};

export type AgentPermissions = {
  canCreateInstances: boolean;
  canDelegate: boolean;
  allowedAgentDefinitionIds: string[];
};

export type AgentDefinitionInput = {
  name: string;
  description: string;
  providerId: string;
  modelId: string;
  modelOptions: AgentModelOptions;
  systemPrompt: string;
  allowedTools: string[];
  maxInstances: number;
  allowedExecutionWorkspaceModes: AgentExecutionWorkspaceMode[];
  permissions: AgentPermissions;
};

export type AgentDefinitionSettings = AgentDefinitionInput & {
  id: string;
  revision: number;
};

export type AgentDefinitionsResponse = {
  agentDefinitions: AgentDefinitionSettings[];
};

export type AgentDefinitionRuntimeView = Omit<
  AgentDefinitionSettings,
  "systemPrompt"
>;

export type AgentTeamView = {
  id: string;
  chatId: string;
  coordinatorInstanceId: string;
  status: string;
  maxConcurrentRuns: number;
  createdAt: string;
  updatedAt: string;
};

export type AgentWorkload = {
  queuedTasks: number;
  runningTasks: number;
  waitingTasks: number;
};

export type AgentExecutionWorkspaceMode = "shared" | "isolated_worktree";

export type AgentInstanceView = {
  id: string;
  teamId: string;
  definitionId: string;
  definitionRevision: number;
  definitionSnapshot: AgentDefinitionRuntimeView;
  role: string;
  status: string;
  nextTaskSequence: number;
  contextGeneration: number;
  executionWorkspaceMode: AgentExecutionWorkspaceMode;
  executionRootPath: string | null;
  worktreeBaseRevision: string | null;
  worktreeBranch: string | null;
  worktreeStatus: string | null;
  lastScheduledAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type AgentAttemptView = {
  id: string;
  sequence: number;
  status: string;
  startedAt: string;
  completedAt: string | null;
  interruptionReason: string | null;
};

export type AgentTaskView = {
  id: string;
  teamId: string;
  ownerInstanceId: string;
  originInstanceId: string | null;
  parentTaskId: string | null;
  sequence: number;
  status: string;
  input: JsonValue;
  result: JsonValue | null;
  error: JsonValue | null;
  attempts: AgentAttemptView[];
  createdAt: string;
  updatedAt: string;
  startedAt: string | null;
  completedAt: string | null;
};

export type AgentTaskDependencyView = {
  teamId: string;
  waitingTaskId: string;
  dependencyTaskId: string;
  waitMode: string;
  pendingToolCallId: string | null;
  deadlineAt: string | null;
  createdAt: string;
};

export type AgentMessageView = {
  id: string;
  teamId: string;
  senderInstanceId: string | null;
  receiverInstanceId: string;
  relatedTaskId: string | null;
  replyToMessageId: string | null;
  kind: string;
  content: string;
  sequence: number;
  createdAt: string;
  consumedAt: string | null;
};

export type AgentEventView = {
  teamId: string;
  sequence: number;
  eventType: string;
  instanceId: string | null;
  taskId: string | null;
  attemptId: string | null;
  messageId: string | null;
  payload: JsonValue;
  createdAt: string;
};

export type AgentMutationLeaseOwnerView = {
  instanceId: string | null;
  taskId: string | null;
  toolCallId: string | null;
  toolName: string | null;
  activeMs: number;
  waitMs: number;
};

export type AgentMetricSummaryView = {
  count: number;
  max: number | null;
  average: number | null;
};

export type AgentFailureClassView = {
  kind: string;
  count: number;
};

export type AgentObservabilityView = {
  queueLength: number;
  queueWaitMs: AgentMetricSummaryView;
  runDurationMs: AgentMetricSummaryView;
  schedulerLatencyMs: AgentMetricSummaryView;
  mutationLeaseWaitMs: AgentMetricSummaryView;
  failedTasks: number;
  cancelledTasks: number;
  interruptedTasks: number;
  failuresByType: AgentFailureClassView[];
};

export type AgentTeamSnapshotResponse = {
  team: AgentTeamView;
  workload: AgentWorkload;
  observability: AgentObservabilityView;
  instances: AgentInstanceView[];
  tasks: AgentTaskView[];
  dependencies: AgentTaskDependencyView[];
  messages: AgentMessageView[];
  events: AgentEventView[];
  mutationLeaseOwners: AgentMutationLeaseOwnerView[];
  worktreeAction?: JsonValue | null;
};

// Provider types

type ProviderKindSummary = {
  kind: string;
  label: string;
  defaultBaseUrl: string;
};

type ApiProxyTypeSummary = {
  proxyType: string;
  label: string;
};

type ApiProxySettingsSummary = {
  enabled: boolean;
  proxyType: string;
  supportedTypes: ApiProxyTypeSummary[];
  url: string;
};

export type ProviderRequestOverrideValueType = "boolean" | "number" | "string";

export type ProviderRequestOverrideTarget = "body" | "header";

export type ProviderRequestOverride = {
  target: ProviderRequestOverrideTarget;
  name: string;
  valueType: ProviderRequestOverrideValueType;
  value: boolean | number | string;
};

export type ProviderRequestOverrideFormState = {
  target: ProviderRequestOverrideTarget;
  name: string;
  valueType: ProviderRequestOverrideValueType;
  value: boolean | string;
};

export type ConfiguredProviderSummary = {
  apiProxy: ApiProxySettingsSummary;
  id: string;
  name: string;
  kind: string;
  kindLabel: string;
  enabled: boolean;
  baseUrl: string | null;
  hasApiKey: boolean;
  requestOverrides: ProviderRequestOverride[];
  warnings: string[];
};

export type ProviderFormState = {
  apiKey: string;
  apiProxyEnabled: boolean;
  apiProxyType: string;
  apiProxyUrl: string;
  baseUrl: string;
  clearApiKey: boolean;
  enabled: boolean;
  id: string;
  kind: string;
  name: string;
  requestOverrides: ProviderRequestOverrideFormState[];
  serviceId: string;
};

export type ProviderTestResponse = {
  ok: boolean;
  message: string;
  modelCount: number;
};

export type ProviderModelsResponse = {
  providerId: string;
  models: string[];
};

export type ProviderModelsRefreshResponse = {
  settings: SettingsResponse;
  providers: ProviderModelsResponse[];
};

export type ProviderTestState = {
  message: string;
  status: "error" | "ok" | "testing";
};

// Settings types

type WebServerSettingsSummary = {
  listenHost: string;
  listenPort: number;
  passwordEnabled: boolean;
};

type RipgrepToolSummary = {
  available: boolean;
  path: string | null;
  installDir: string;
};

type NativeToolsSummary = {
  browserProbePort: number;
  ripgrep: RipgrepToolSummary;
};

export type InstallRipgrepResponse = {
  ripgrep: RipgrepToolSummary;
};

type AppLanguageSummary = {
  id: AppLanguageId;
  name: string;
};

type AppThemeSummary = {
  id: AppThemeId;
  name: string;
};

type ApiAuditSettingsSummary = {
  requestDetailRetentionDays: number;
  saveRequestResponseDetails: boolean;
};

export type GeneralSettingsSummary = {
  apiAudit: ApiAuditSettingsSummary;
  autoStartEnabled: boolean;
  defaultTeamModeEnabled: boolean;
  hookAuditEnabled: boolean;
  language: AppLanguageId;
  llmRequestRetryCount: number;
  maxLlmRequestRetryCount: number;
  supportedLanguages: AppLanguageSummary[];
  supportedThemes: AppThemeSummary[];
  theme: AppThemeId;
  webServer: WebServerSettingsSummary;
};

export type GeneralFormState = {
  apiRequestDetailRetentionDays: string;
  apiSaveRequestResponseDetails: boolean;
  autoStartEnabled: boolean;
  hookAuditEnabled: boolean;
  language: string;
  listenHost: string;
  listenPort: string;
  llmRequestRetryCount: string;
  password: string;
  theme: AppThemeId;
};

type WebSearchProviderSummary = {
  provider: string;
  label: string;
  hasApiKey: boolean;
};

type WebSearchSettingsSummary = {
  enabled: boolean;
  activeProvider: string;
  providers: WebSearchProviderSummary[];
  apiProxy: ApiProxySettingsSummary;
};

export type WebSearchFormState = {
  activeProvider: string;
  apiProxyEnabled: boolean;
  apiProxyType: string;
  apiProxyUrl: string;
  braveApiKey: string;
  clearBraveApiKey: boolean;
  clearTavilyApiKey: boolean;
  enabled: boolean;
  tavilyApiKey: string;
};

export type SystemPromptSummary = {
  name: string;
  content: string;
};

export type PromptSettingsSummary = {
  systemPrompt: string | null;
  defaultSystemPrompt: string;
  systemPrompts?: SystemPromptSummary[];
  files: string[];
  extraText: string;
};

export type PromptSettingsFormState = {
  activeSystemPromptName: string;
  systemPrompts: SystemPromptSummary[];
  files: string[];
  extraText: string;
  pendingFile: string;
  pendingSystemPromptName: string;
  pendingSystemPromptRename: string;
  renamingSystemPromptName: string | null;
};

export type TerminalShellSummary = {
  shell: string;
  label: string;
};

export type AuthStatusResponse = {
  authenticated: boolean;
  enabled: boolean;
};

// Memory types

type MemoryExtractionModeSummary = {
  value: string;
  label: string;
};

export type MemoryDreamRunMode = "deterministic_only" | "llm";

export type MemoryDreamScope = "global" | "workspace";

export type MemoryDreamTriggerType =
  | "manual"
  | "auto_interval"
  | "auto_threshold";

export type MemoryDreamSettingsSummary = {
  enabled: boolean;
  autoEnabled: boolean;
  mode: MemoryDreamRunMode;
  modelId: string | null;
  workspaceIntervalDays: number;
  globalIntervalDays: number;
  createTranscriptChat: boolean;
  maxFactsPerRun: number;
  maxChangesPerRun: number;
  schedulerScanMinutes: number;
  workspaceThresholdFacts: number;
  globalThresholdFacts: number;
};

type MemorySettingsSummary = {
  enabled: boolean;
  extractionMode: string;
  retrievalMode: string;
  retentionDays: number | null;
  extractionModelId: string | null;
  retrievalModelId: string | null;
  dream: MemoryDreamSettingsSummary;
  extractionModes: MemoryExtractionModeSummary[];
  retrievalModes: MemoryExtractionModeSummary[];
};

export type MemoryFactRecord = {
  id: string;
  scope: string;
  chatId: string | null;
  status: string;
  kind: string;
  fact: string;
  confidence: number | null;
  pinned: boolean;
  isLatest: boolean;
  expiresAt: string | null;
  metadataJson: string;
  createdAt: string;
  updatedAt: string;
};

export type MemorySourceRecord = {
  id: string;
  scope: string;
  chatId: string | null;
  sourceType: string;
  sourceId: string | null;
  title: string;
  content: string;
  metadataJson: string;
  createdAt: string;
  updatedAt: string;
};

export type MemoryExtractionJobSummary = {
  id: string;
  scope: string;
  chatId: string | null;
  status: string;
  modelId: string | null;
  errorMessage: string | null;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
};

export type MemoryListResponse = {
  memories: MemoryFactRecord[];
  extractionJobs: MemoryExtractionJobSummary[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

export type MemoryMutationResponse = {
  memory: MemoryFactRecord | null;
};

export type ClearMemoriesResponse = {
  deletedCount: number;
};

export type MemorySourcesResponse = {
  sources: MemorySourceRecord[];
};

export type MemorySettingsFormState = {
  enabled: boolean;
  extractionMode: string;
  retrievalMode: string;
  retentionDays: string;
  extractionModelId: string;
  retrievalModelId: string;
  dream: {
    enabled: boolean;
    autoEnabled: boolean;
    mode: MemoryDreamRunMode;
    modelId: string;
    workspaceIntervalDays: string;
    globalIntervalDays: string;
    createTranscriptChat: boolean;
    maxFactsPerRun: string;
    maxChangesPerRun: string;
    schedulerScanMinutes: string;
  };
};

export type MemoryDreamJobStatus =
  | "queued"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"
  | "skipped"
  | string;

export type MemoryDreamChangeCounts = {
  added: number;
  updated: number;
  superseded: number;
  expired: number;
  rejected: number;
};

export type MemoryDreamJobSummary = {
  id: string;
  scope: MemoryDreamScope;
  workspaceId: string | null;
  triggerType: MemoryDreamTriggerType;
  mode: MemoryDreamRunMode;
  status: MemoryDreamJobStatus;
  modelId: string | null;
  transcriptChatId: string | null;
  transcriptWorkspaceId?: string | null;
  errorMessage: string | null;
  summary: string | null;
  changeCounts: MemoryDreamChangeCounts;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
};

export type MemoryDreamJobsResponse = {
  jobs: MemoryDreamJobSummary[];
};

export type MemoryDreamRunResponse = {
  jobId: string;
  status: MemoryDreamJobStatus;
  transcriptChatId: string | null;
};

export type MemoryDreamChangeSummary = {
  id: string;
  jobId: string;
  operation: string;
  targetFactIds: string[];
  newFactId: string | null;
  beforeJson: JsonValue | null;
  afterJson: JsonValue | null;
  reason: string;
  confidence: number | null;
  riskLevel: string;
  status: string;
  evidence: JsonValue;
  errorMessage: string | null;
  createdAt: string;
  appliedAt: string | null;
};

export type MemoryDreamChangesResponse = {
  changes: MemoryDreamChangeSummary[];
};

export type MemoryFilterState = {
  status: "active" | "pending";
  scope: "global" | "workspace" | "chat";
  kind: string;
  workspaceId: string;
  chatId: string;
  query: string;
  page: number;
  pageSize: number;
};

export type MemoryListMeta = {
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

export type ManualMemoryFormState = {
  scope: "global" | "workspace" | "chat";
  workspaceId: string;
  chatId: string;
  kind: string;
  fact: string;
  confidence: string;
  metadataText: string;
  pinned: boolean;
};

export type MemorySourceFormState = {
  id: string;
  title: string;
  content: string;
  metadataText: string;
};

export type MemoryDialogMode = "create" | "edit";

// MCP types

type McpTransportSummary = {
  transport: string;
  label: string;
};

export type ConfiguredMcpServerSummary = {
  id: string;
  name: string;
  enabled: boolean;
  transport: string;
  transportLabel: string;
  command: string | null;
  args: string[];
  url: string | null;
  state: string;
  error: string | null;
  toolCount: number;
  warnings: string[];
};

export type McpServerFormState = {
  argsText: string;
  command: string;
  enabled: boolean;
  id: string;
  name: string;
  transport: string;
  url: string;
};

// Skills types

type SkillsSettingsSummary = {
  directories: string[];
  detected: ConfiguredSkillSummary[];
  errors: SkillDiscoveryErrorSummary[];
};

export type ConfiguredSkillSummary = {
  key: string;
  id: string;
  name: string;
  description: string;
  path: string;
  scope: string;
  workspaceId: string | null;
  workspaceName: string | null;
  enabled: boolean;
  canEnable: boolean;
  warnings: string[];
};

type SkillDiscoveryErrorSummary = {
  path: string;
  message: string;
};

// Hooks types

export type HookHandlerType = "command" | "http" | "mcp_tool" | "prompt";

export type HookHandler = {
  enabled?: boolean;
  type: HookHandlerType | string;
  if?: string | null;
  command?: string | null;
  args?: string[];
  shell?: string | null;
  url?: string | null;
  serverId?: string | null;
  toolName?: string | null;
  prompt?: string | null;
  timeout?: number | null;
  async?: boolean;
  asyncRewake?: boolean;
  statusMessage?: string | null;
  input?: JsonValue | null;
};

export type HookMatcherGroup = {
  enabled?: boolean;
  matcher?: string | null;
  hooks: HookHandler[];
};

export type HookConfig = {
  disableAllHooks?: boolean;
  [eventName: string]: boolean | HookMatcherGroup[] | undefined;
};

export type HookConfigScopeSummary = {
  source: string;
  path: string;
  workspaceId: string | null;
  config: HookConfig;
};

export type EffectiveHookSummary = {
  source: string;
  event: string;
  matcher: string | null;
  handlerType: string;
  command: string | null;
  url: string | null;
  serverId: string | null;
  toolName: string | null;
  asyncHook: boolean;
  statusMessage: string | null;
};

export type HookRunSummaryRow = {
  id: string;
  workspaceId: string;
  chatId: string | null;
  runId: string | null;
  toolCallId: string | null;
  event: string;
  hookSource: string;
  handlerType: string;
  status: string;
  exitCode: number | null;
  stdoutPreview: string | null;
  stderrPreview: string | null;
  startedAt: string;
  completedAt: string;
};

export type HooksSettingsResponse = {
  supportedEvents: string[];
  unsupportedEvents: string[];
  global: HookConfigScopeSummary;
  workspace: HookConfigScopeSummary;
  effective: EffectiveHookSummary[];
  recentRuns: HookRunSummaryRow[];
};

export type HookRunsResponse = {
  runs: HookRunSummaryRow[];
};

export type ImportClaudeHooksResponse = {
  saved: boolean;
  target: "global" | "workspace" | string;
  path: string;
  importedFiles: string[];
  validationErrors: string[];
  config: HookConfig;
};

export type HookDecision =
  | { type: "allow" }
  | { type: "ask"; reason: string }
  | { type: "block"; reason: string }
  | { type: "deny"; reason: string };

export type HookRunSummary = {
  decisions: HookDecision[];
  additionalContext: string[];
  systemMessages: string[];
  errors: string[];
};

export type HookRunDetail = HookRunSummaryRow & {
  input: JsonValue;
  output: JsonValue | null;
};

export type HookRunDetailResponse = {
  run: HookRunDetail;
};

export type HookScope = "global" | "workspace";

export type HookHandlerFormState = {
  argsText: string;
  asyncHook: boolean;
  asyncRewake: boolean;
  command: string;
  enabled: boolean;
  event: string;
  groupIndex: number | null;
  handlerIndex: number | null;
  ifFilter: string;
  inputText: string;
  matcher: string;
  prompt: string;
  serverId: string;
  shell: string;
  statusMessage: string;
  timeout: string;
  toolName: string;
  type: HookHandlerType;
  url: string;
};

// AI Statistics types

export type AiRequestAuditSummary = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  chatId: string | null;
  chatTitle: string | null;
  providerId: string;
  modelId: string;
  requestStartedAt: string;
  firstTokenAt: string | null;
  completedAt: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  cacheRatio: number | null;
  firstTokenLatencyMs: number | null;
  totalLatencyMs: number | null;
  statusCode: number | null;
  finalState: string;
};

export type AiRequestAuditDetail = AiRequestAuditSummary & {
  requestBody: JsonValue | null;
  responseBody: JsonValue | null;
};

type AiStatisticsTrendPoint = {
  bucket: string;
  requestCount: number;
  totalTokens: number;
};

export type AiStatisticsSummary = {
  averageLatencyMs: number | null;
  failedRequests: number;
  modelBreakdown: AiStatisticsModelBreakdown[];
  providerBreakdown: AiStatisticsProviderBreakdown[];
  totalCacheReadTokens: number;
  totalCacheWriteTokens: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalRequests: number;
  totalTokens: number;
  trend: AiStatisticsTrendPoint[];
};

export type AiStatisticsResponse = {
  page: number;
  pageSize: number;
  requests: AiRequestAuditSummary[];
  summary: AiStatisticsSummary;
  totalCount: number;
  totalPages: number;
};

export type AiRequestDetailResponse = {
  request: AiRequestAuditDetail;
};

export type AiStatsFilterState = {
  workspaceId: string;
  chatId: string;
  providerId: string;
  modelId: string;
  status: string;
  startedAfter: string;
  startedBefore: string;
  page: string;
  pageSize: string;
};

// Todo Graph types

export type TaskStatus =
  | "pending"
  | "ready"
  | "running"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled";

export type TodoGraphTask = {
  id: string;
  title: string;
  status: TaskStatus;
  dependsOn: string[];
  acceptance: string[];
  summary: string | null;
  createdAt: string;
  updatedAt: string;
  subtasks: TodoGraphTask[];
};

export type TodoGraphResponse = {
  chatId: string;
  exists: boolean;
  tasks: TodoGraphTask[];
  createdAt: string | null;
  updatedAt: string | null;
};

// Terminal types

export type TerminalSessionResponse = {
  id: string;
  name: string;
  workingDirectory: string;
};

export type TerminalServerEvent =
  | { type: "started"; cwd: string }
  | { type: "output"; data: string }
  | { type: "cwd"; cwd: string }
  | { type: "exit"; status: string }
  | { type: "error"; message: string };

export type TerminalPaneStatus = "closed" | "connected" | "connecting" | "error";

export type TerminalCommandRun = {
  input: string;
};

export type TerminalPanelSession = {
  clientId: string;
  cwd: string;
  error: string | null;
  number: number;
  pendingCommand: TerminalCommandRun | null;
  serverSessionId: string | null;
  status: TerminalPaneStatus;
};

// Shell Message type (UI-specific variant of ChatMessageSummary)

export type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  createdAt: string;
  reasoning: string | null;
  status?: "error" | "streaming";
  pendingMode?: "guidance" | "queued";
  queuedRun?: QueuedMessageRunSummary | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
  runBadges?: ChatRunBadge[];
};

// Chat Tab types

export type OpenChatTab = {
  workspaceId: string;
  chatId: string;
  fallbackTitle: string;
  fallbackWorkspaceName: string;
};

export type ChatTabSummary = OpenChatTab & {
  title: string;
  workspaceLogoUrl: string | null;
  workspaceName: string;
};

export type PendingDeleteChat = {
  workspaceId: string;
  chatId: string;
  title: string;
  workspaceName: string;
};

// Run scheduling types

export type RetryRunRequest = {
  workspaceId: string;
  chatId: string | null;
  content: string;
  attachments: ChatAttachmentPayload[];
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
  teamModeEnabled?: boolean;
  localChatKey?: string;
  pendingUserMessageId?: string;
  queuedUserMessageId?: string;
  assistantMessageId?: string;
};

export type ScheduledWorkspaceRun = {
  id: string;
  workspaceId: string;
  chatId: string;
  chatKey: string;
  createdChatId?: string;
  title: string;
  createdAt: string;
  pendingUserMessageId: string;
  request: RetryRunRequest;
  status: "queued" | "starting";
};

export type ScheduledTaskStatus =
  | "enabled"
  | "paused"
  | "completed"
  | "archived";

export type ScheduledTaskSchedule =
  | { type: "one_shot_at"; run_at: string }
  | { type: "interval"; every_seconds: number; start_at?: string | null }
  | { type: "cron"; expression: string; timezone?: string | null };

export type ScheduledSessionMode =
  | "create_new_chat"
  | { reuse_chat: { chat_id: string } };

export type ScheduledTaskAction = {
  type: "agent_prompt";
  prompt: string;
  session_mode: ScheduledSessionMode;
  agent_definition_id?: string | null;
  model_id?: string | null;
  provider_id?: string | null;
  thinking_level?: string | null;
  skill_ids: string[];
  collaboration_tools_enabled: boolean;
};

export type ScheduledTaskUsageSummary = {
  totalRequests: number;
  failedRequests: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheWriteTokens: number;
  totalTokens: number;
  totalLatencyMs: number;
  averageLatencyMs: number | null;
};

export type ScheduledTaskView = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  title: string;
  description: string | null;
  schedule: JsonValue;
  action: JsonValue;
  status: ScheduledTaskStatus;
  nextRunAt: string | null;
  lastRunAt: string | null;
  createdAt: string;
  updatedAt: string;
  metadata: JsonValue;
  usage: ScheduledTaskUsageSummary;
};

export type ScheduledTasksResponse = {
  tasks: ScheduledTaskView[];
};

export type ScheduledTaskPreviewNextRunResponse = {
  nextRunAt: string | null;
  nextRuns: string[];
};

export type ScheduledTaskRunStatus =
  | "pending"
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "skipped";

export type ScheduledTaskRunView = {
  id: string;
  workspaceId: string;
  taskId: string;
  triggerReason: "scheduled" | "manual" | "retry" | "misfire_catch_up" | string;
  status: ScheduledTaskRunStatus;
  scheduledAt: string;
  queuedAt: string | null;
  startedAt: string | null;
  completedAt: string | null;
  chatId: string | null;
  userMessageId: string | null;
  assistantMessageId: string | null;
  agentTeamId: string | null;
  agentTaskId: string | null;
  agentAttemptId: string | null;
  activeRunId: string | null;
  errorMessage: string | null;
  outputSummary: string | null;
  createdAt: string;
  updatedAt: string;
  metadata: JsonValue;
};

export type ScheduledTaskRunsResponse = {
  runs: ScheduledTaskRunView[];
};

export type ScheduledTaskRunResponse = {
  run: ScheduledTaskRunView;
};

export type ActiveRunInfo = {
  workspaceId: string;
  chatId: string | null;
  // Backend active-run registry id. Do not replace it with per-provider llmRequestId attempts.
  runId: string | null;
  chatKey: string;
  lastSequence?: number | null;
  acceptingGuidance: boolean;
};

// Settings response (aggregate type)

export type SettingsResponse = {
  general: GeneralSettingsSummary;
  agentTools: string[];
  nativeTools: NativeToolsSummary;
  webSearch: WebSearchSettingsSummary;
  memory: MemorySettingsSummary;
  prompts: PromptSettingsSummary;
  workspaces: ConfiguredWorkspaceSummary[];
  terminalShells: TerminalShellSummary[];
  providerKinds: ProviderKindSummary[];
  thinkingLevels: ThinkingLevelSummary[];
  providers: ConfiguredProviderSummary[];
  configuredModels: ConfiguredModelSummary[];
  mcpTransports: McpTransportSummary[];
  mcpServers: ConfiguredMcpServerSummary[];
  skills: SkillsSettingsSummary;
};
