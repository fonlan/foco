export type AppLanguageId = "en" | "zh-CN";

export type AppThemeId = "light" | "dark";

export type SettingsSection =
  | "general"
  | "agents"
  | "prompts"
  | "spec"
  | "plan"
  | "web-search"
  | "remote-servers"
  | "hooks"
  | "memory"
  | "mcp"
  | "models"
  | "providers"
  | "skills"
  | "workspaces"
  | "about";

export type BrowserRouteChatTab = {
  workspaceId: string;
  chatId: string;
};

export type BrowserRouteFileTab = {
  workspaceId: string;
  path: string;
};

/** Workspace HTML preview tab identity persisted in the URL (no capability token). */
export type BrowserRouteHtmlPreviewTab = {
  workspaceId: string;
  path: string;
};

export type PreviewSessionResponse = {
  token: string;
  workspaceId: string;
  entryPath: string;
  rootPath: string;
  previewUrl: string;
  previewOrigin: string;
  iframeSandbox: string;
};

export type BrowserRoute =
  | {
      viewMode: "chat";
      workspaceId: string | null;
      chatId: string | null;
      tabs?: BrowserRouteChatTab[];
      files?: BrowserRouteFileTab[];
      activeFile?: BrowserRouteFileTab;
      previews?: BrowserRouteHtmlPreviewTab[];
      activePreview?: BrowserRouteHtmlPreviewTab;
    }
  | { viewMode: "settings"; section: SettingsSection }
  | { viewMode: "stats"; page: number; filters?: Partial<AiStatsFilterState> }
  | { viewMode: "scheduled" }
  | { viewMode: "skill-store" };

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
  worktrees: GitWorktreeSummary[];
};

export type GitWorktreeSummary = {
  name: string;
  path: string;
  branch: string | null;
  isCurrent: boolean;
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

// Project Spec types

export type WorkspaceSpecSettings = {
  enabled: boolean;
  injectEnabled: boolean;
};

export type WorkspaceSpecJobSummary = {
  id: string;
  triggerType: string;
  status: string;
  chatId: string | null;
  runId: string | null;
  modelId: string | null;
  baseRevision: number | null;
  inputSummary: JsonValue;
  output: JsonValue | null;
  errorMessage: string | null;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
  hasRetry: boolean;
};

export type WorkspaceSpecResponse = {
  settings: WorkspaceSpecSettings;
  contentMarkdown: string;
  revision: number;
  generatedAt: string | null;
  updatedAt: string | null;
  latestJob: WorkspaceSpecJobSummary | null;
};

export type GenerateWorkspaceSpecResponse = {
  job: WorkspaceSpecJobSummary;
};

export type WorkspaceSpecJobsResponse = {
  jobs: WorkspaceSpecJobSummary[];
};

export type SettingsWorkspaceSpecJobSummary = {
  job: WorkspaceSpecJobSummary;
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
  chatTitle: string | null;
};

export type SettingsWorkspaceSpecJobError = {
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
  error: string;
};

export type SettingsWorkspaceSpecJobsResponse = {
  jobs: SettingsWorkspaceSpecJobSummary[];
  errors: SettingsWorkspaceSpecJobError[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

export type RetryWorkspaceSpecJobResponse = {
  job: WorkspaceSpecJobSummary;
};

export type DeleteFailedWorkspaceSpecJobResponse = {
  deleted: boolean;
  jobId: string;
};

// Plan types

export type PlanStatus =
  | "draft"
  | "ready"
  | "running"
  | "paused"
  | "implemented"
  | "completed"
  | "blocked"
  | "failed"
  | "cancelled";

export type PlanStepStatus =
  "pending" | "running" | "completed" | "failed" | "cancelled";

export type PlanStep = {
  id: string;
  planId: string;
  phaseId: string;
  sequence: number;
  title: string;
  detail: string;
  acceptance: string[];
  status: PlanStepStatus;
  checkedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type PlanPhaseAttempt = {
  id: string;
  planId: string;
  phaseId: string;
  sequence: number;
  trigger: string;
  status: string;
  providerId: string | null;
  modelId: string | null;
  thinkingLevel: string | null;
  implementationChatId: string | null;
  agentTeamId: string | null;
  agentTaskId: string | null;
  commitId: string | null;
  errorMessage: string | null;
  startedAt: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type PlanPhase = {
  id: string;
  planId: string;
  sequence: number;
  title: string;
  summary: string;
  status: string;
  implementationChatId: string | null;
  agentTeamId: string | null;
  agentTaskId: string | null;
  commitId: string | null;
  mergeAttemptCount: number;
  errorMessage: string | null;
  startedAt: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
  steps: PlanStep[];
  attempts: PlanPhaseAttempt[];
};

export type Plan = {
  id: string;
  title: string;
  overview: string;
  status: PlanStatus;
  sortOrder: number;
  sourceChatId: string | null;
  activePhaseId: string | null;
  pauseRequestedAt: string | null;
  completedAt: string | null;
  completedByUserAt: string | null;
  errorMessage: string | null;
  sharedMergeCommitId: string | null;
  createdAt: string;
  updatedAt: string;
  phases: PlanPhase[];
};

export type PlansResponse = {
  plans: Plan[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

export type PlanResponse = {
  plan: Plan;
};

export type PlanAutoRunResponse = {
  /** Backward-compatible effective state: desired and not runtime-blocked. */
  enabled: boolean;
  desiredEnabled: boolean;
  busy: boolean;
  blockedReason?:
    | "waiting_for_ready"
    | "waiting_for_retry"
    | "cancelled_phase"
    | "merge_blocked"
    | "scheduler_error";
  blockedPlanId?: string;
  blockedPhaseId?: string;
};

export type PlanWorktreeAuditItem = {
  planId: string;
  planStatus: string;
  phaseId: string;
  phaseStatus: string;
  implementationChatId: string | null;
  agentTaskId: string | null;
  agentTaskStatus: string | null;
  agentInstanceId: string;
  worktreePath: string;
  baseRevision: string | null;
  branch: string | null;
  refName: string | null;
  worktreeStatus: string | null;
  commitId: string | null;
  headCommitId: string | null;
  headCommitShort: string | null;
  errorMessage: string | null;
  cleanupAllowed: boolean;
};

export type PlanWorktreeAuditResponse = {
  items: PlanWorktreeAuditItem[];
  recoveryNote: string;
};

export type PlanWorktreeCleanupResponse = {
  deleted: boolean;
  item: PlanWorktreeAuditItem;
};

// JSON types

export type JsonValue =
  boolean | null | number | string | JsonValue[] | { [key: string]: JsonValue };

// Chat types

export type QueuedRunSummary = {
  status: "queued" | "running" | string;
  userMessageId: string;
  assistantMessageId: string | null;
  assistantSequence?: number | null;
  modelId?: string | null;
  providerId: string | null;
  thinkingLevel: string | null;
  latencyMode?: LatencyMode | null;
  skillIds: string[];
  sessionMode?: "plan" | null;
  content?: string | null;
};

export type ActiveChatRunSummary = {
  runId: string;
  workspaceId: string;
  chatId: string;
  // Optional for compatibility with sidecars that predate durable assistant identity.
  assistantMessageId?: string | null;
  assistantSequence?: number | null;
  queuedUserMessageId?: string | null;
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
  llmRequestIds: string[];
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

export type ChatSpecUpdateDiffLine = {
  kind: "added" | "removed";
  text: string;
};

export type ChatSpecUpdateSummary = {
  id: string;
  jobId: string;
  baseRevision: number;
  revision: number;
  completedAt: string;
  lines: ChatSpecUpdateDiffLine[];
  truncated: boolean;
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
  startedAt?: string | null;
  completedAt?: string | null;
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

export type ChatContextCompressionKind = "rule" | "llm" | "runtimeToolState";

export type ChatContextCompressionDetail = {
  status?: string;
  kind?: ChatContextCompressionKind;
  compressionId?: string | null;
  snapshotId?: string | null;
  originalTokenCount?: number | null;
  summaryTokenCount?: number | null;
  startedAt?: string | null;
  completedAt?: string | null;
  providerId?: string | null;
  modelId?: string | null;
  providerRequestId?: string | null;
  compressionMode?: "normal" | "required_overflow" | null;
  attemptIndex?: number | null;
  outcome?: string | null;
  action?: string | null;
  errorMessage?: string | null;
};

export type ChatContextCompressionPart = {
  type: "contextCompression";
  id: string;
  status: string;
  kind: ChatContextCompressionKind;
  detail: ChatContextCompressionDetail;
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
  | { type: "toolCall"; toolCall: ChatToolCallSummary }
  | ChatContextCompressionPart
  | {
      type: "userInterruption";
      id: string;
      content: string;
      source?: string;
      interruptedAssistantMetrics?: ChatReplyMetrics | null;
    };

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

export type FilePickerTarget =
  | { kind: "local" }
  | { kind: "remoteServer"; serverId: string }
  | { kind: "workspace"; workspaceId: string };

export type FilePickerMode = "file" | "directory";

export type FilePickerEntry = {
  name: string;
  path: string;
  isDirectory: boolean;
  sizeBytes?: number | null;
  modifiedAt?: string | null;
  disabled?: boolean;
};

export type FilePickerListResponse = {
  path: string;
  parentPath?: string | null;
  entries: FilePickerEntry[];
  truncated: boolean;
  warnings: string[];
};

export type QueuedMessageRunSummary = {
  status: "queued" | "running" | string;
  modelId: string;
  providerId: string | null;
  thinkingLevel: string | null;
  latencyMode?: LatencyMode | null;
  skillIds: string[];
  sessionMode?: "plan" | null;
  assistantMessageId: string | null;
  assistantSequence?: number | null;
};

export type ChatRunBadge =
  | "contextCompressionRule"
  | "contextCompressionLlm"
  | "contextCompressionRuntime"
  | "llmReconnect";

export type ChatMessageRunConfigSummary = {
  modelId: string;
  providerId: string | null;
  thinkingLevel: string | null;
  latencyMode?: LatencyMode | null;
  selectedSkillIds: string[];
  sessionMode?: "plan" | null;
  teamModeEnabled: boolean;
};

export type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
  createdAt: string;
  reasoning: string | null;
  status?: "error" | "streaming";
  sessionMode?: "plan" | null;
  runConfig?: ChatMessageRunConfigSummary | null;
  pendingMode?: "guidance" | "queued";
  queuedRun?: QueuedMessageRunSummary | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
  specUpdates: ChatSpecUpdateSummary[];
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
  sessionMode?: "plan" | null;
  agentTeamId?: string;
  agentTaskId?: string;
};

export type EditChatUserMessageResponse = {
  chatId: string;
  userMessageId: string;
  assistantMessageId: string;
  assistantSequence: number;
  content: string;
  parts: ChatMessagePart[];
  sessionMode?: "plan" | null;
  removedMessageIds: string[];
};

export type ChatMessagesResponse = {
  chat?: ChatMessagesChatSummary | null;
  messages: ChatMessageSummary[];
  pagination: {
    hasMoreBefore: boolean;
    nextBeforeSequence: number | null;
  };
  activeRun?: ActiveChatRunSummary | null;
  pendingQuestion?: QuestionRequestSummary | null;
  latestResponseUsage?: ChatUsage | null;
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

export type PendingQuestionsResponse = {
  questions: QuestionRequestSummary[];
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
      // Backend active-run registry id. This stays stable across provider retries and tool follow-ups.
      runId?: string;
      // Legacy logical-run id for local streams. Never use per-attempt ids from streamAttemptStart.
      llmRequestId?: string;
      memoriesUsed: ChatMemoryUsedSummary[];
    }
  | { type: "connecting"; message?: string }
  | {
      type: "textDelta";
      assistantMessageId?: string;
      delta: string;
      reasoningDurationMs?: number | null;
    }
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
      compressionId?: string;
      snapshotId?: string;
      kind: ChatContextCompressionKind;
      status: string;
      detail?: ChatContextCompressionDetail | null;
    }
  | { type: "usage"; usage?: ChatUsage }
  | {
      type: "complete";
      chatId: string;
      assistantMessageId: string;
      text: string;
      reasoning?: string | null;
      reasoningDurationMs?: number | null;
      usage?: ChatUsage | null;
      stopReason?: string | null;
      metrics: ChatReplyMetrics;
      memoriesUsed: ChatMemoryUsedSummary[];
    }
  | { type: "streamEnd" }
  | {
      type: "toolCall";
      assistantMessageId: string;
      reasoningDurationMs?: number | null;
      toolCall: ChatToolCallSummary;
    }
  | {
      type: "toolResult";
      assistantMessageId: string;
      toolCallId: string;
      output: JsonValue;
      isError: boolean;
      /** Missing on historical events and therefore treated as terminal. */
      terminal?: boolean;
      startedAt?: string | null;
      completedAt?: string | null;
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
      source?: string;
      interruptedAssistantId?: string | null;
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
      type: "planRefresh";
      workspaceId: string;
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
  runtimeToolStateSnapshotCount: number;
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
  contextUsageTimeline: ContextUsageTimelineEntry[];
};

export type ContextUsageSegments = {
  systemPrompt: number;
  toolSchema: number;
  compressionSnapshot: number;
  history: number;
  reservedOutput: number;
};

export type ContextUsageTimelineEntry = {
  snapshotId: string;
  sequence: number;
  kind: string;
  contextWindow: number;
  maxOutputTokens: number;
  triggerTokens: number;
  totalUsedTokens: number;
  segments: ContextUsageSegments;
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
  assembledMessageTokens: number;
  assembledUsagePercent: number;
  postCompressionMessageTokens: number;
  packedMessageTokens: number;
  availableMessageTokens: number;
  contextWindow: number;
  maxOutputTokens: number;
  systemPromptTokens: number;
  toolSchemaTokens: number;
  historyTokens: number;
  compressionSnapshotTokens: number;
  totalUsedContextTokens: number;
  memoryContextTokens: number;
  memoryBudgetTokens: number;
  usagePercent: number;
  compressionTriggerTokens: number;
  compressionTriggerPercent: number;
  llmCompressionTriggerTokens: number;
  llmCompressionTriggerPercent: number;
  hasLlmCompressionPlan: boolean;
  willCompressOnNextSend: boolean;
  segments: ContextUsageSegments;
  tokenBreakdown: ContextTokenBreakdown;
};

export type ContextUsageRefreshRequest = {
  workspaceId: string;
  chatId: string | null;
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
  assistantDraft?: string;
  assistantDraftReasoning?: string;
  sessionMode?: "plan";
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

export type WorkspaceChatPagination = {
  total: number;
  limit: number;
  hasMore: boolean;
  nextCursor: string | null;
};

export type RemoteAuthMethod = "key" | "password";

export type RemoteServerSummary = {
  id: string;
  name: string;
  hostAlias: string;
  user: string | null;
  port: number | null;
  identityFile: string | null;
  authMethod: RemoteAuthMethod;
  passwordConfigured: boolean;
  defaultRemoteRoot: string | null;
  focoCommand: string | null;
  terminalShell: string | null;
  connectTimeoutMs: number;
  status: string;
  lastError: string | null;
  lastKnownTarget: string | null;
  sidecarVersion: string | null;
  sidecarInstallState: string;
  workspaceCount: number;
  lastCheckedAt: string | null;
};

/** Create/update remote server request body. Password is never returned by the API. */
export type RemoteServerInput = {
  id?: string;
  name: string;
  hostAlias: string;
  user?: string | null;
  port?: number | null;
  identityFile?: string | null;
  authMethod?: RemoteAuthMethod;
  /** Empty/omitted on update keeps existing password when authMethod is password. */
  password?: string | null;
  defaultRemoteRoot?: string | null;
  focoCommand?: string | null;
  terminalShell?: string | null;
  connectTimeoutMs?: number;
};

export type RemoteServerDiagnosticStage = {
  stage: string;
  status: string;
  errorKind: string | null;
  message: string;
  details: string | null;
};

/** Structured SSH host key from diagnostics (no raw key material). */
export type RemoteServerHostKeyInfo = {
  host: string;
  port: number;
  algorithm: string;
  fingerprintSha256: string;
};

export type RemoteServerDiagnosticResult = {
  ok: boolean;
  errorKind: string | null;
  message: string | null;
  stages: RemoteServerDiagnosticStage[];
  hostKey?: RemoteServerHostKeyInfo | null;
  /** True when the UI may prompt the user to trust an unknown host key. */
  hostKeyVerificationRequired?: boolean;
};

export type RemoteServerResponse = {
  server: RemoteServerSummary;
};

export type RemoteServerDiagnosticResponse = RemoteServerResponse & {
  result: RemoteServerDiagnosticResult;
};

export type TrustHostKeyResponse = {
  trusted: boolean;
  server: RemoteServerSummary;
};

export type RemoteServerWorkspaceReference = {
  id: string;
  name: string;
  remotePath: string;
};

export type DeleteRemoteServerResponse = {
  deleted: boolean;
  references: RemoteServerWorkspaceReference[];
};

export type WorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  displayPath: string;
  serverId: string | null;
  serverName: string | null;
  remotePath: string | null;
  connectionStatus: string;
  lastRemoteError: string | null;
  logoUrl: string | null;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
  chats: ChatSummary[];
  chatPagination: WorkspaceChatPagination;
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

export type WorkspaceChatSearchResponse = WorkspacesResponse;

export type WorkspaceChatsResponse = WorkspaceChatPagination & {
  chats: ChatSummary[];
};

export type ConfiguredWorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  displayPath?: string;
  serverId?: string | null;
  serverName?: string | null;
  remotePath?: string | null;
  connectionStatus?: string;
  lastRemoteError?: string | null;
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
  serverId?: string | null;
  remotePath?: string | null;
  pinned: boolean;
  specEnabled: boolean;
  specInjectEnabled: boolean;
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
  supportedThinkingLevels: string[];
  supportsTools: boolean;
  supportsCache: boolean;
  reasoning: boolean;
  sourceUrl: string;
  refreshedAt: string;
};
export type WebSearchMode = "auto" | "native" | "function" | "disabled";

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
  inputModalities: string[];
  outputModalities: string[];
  thinkingLevel: string | null;
  /** Per-model web search preference; defaults to auto when omitted by older servers. */
  webSearchMode?: WebSearchMode;
  systemPromptName: string;
  supportsThinking: boolean;
  /** Server-resolved Fast eligibility for the active provider and upstream model. */
  supportsFast?: boolean;
  /** Model-level Fast preference, effective only while the active route supports it. */
  fastModeEnabled?: boolean;
  supportedThinkingLevels: string[];
  warnings: string[];
};

export type ModelTestResponse = {
  ok: boolean;
  message: string;
  modelId: string;
  providerId: string | null;
};

export type ModelTestState = {
  testing: boolean;
};

export type ModelMetadataResponse = {
  sourceUrl: string | null;
  fetchedAt: string | null;
  cachePath: string;
  models: ModelMetadataRecord[];
  configuredModels: ConfiguredModelSummary[];
};

/** Lightweight success body for POST /api/models/route (no models.dev catalog). */
export type UpdateModelRouteResponse = {
  modelId: string;
  activeProviderId: string;
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
  inputModalities: string[];
  outputModalities: string[];
  thinkingLevel: string;
  webSearchMode: WebSearchMode;
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
  /** Legacy/compatible field; new flows omit and resolve provider via model routing. */
  providerId?: string;
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
  /** Server may still return a derived/compatible provider id. */
  providerId: string;
};

export type AgentDefinitionsResponse = {
  agentDefinitions: AgentDefinitionSettings[];
  defaultRolePrompts?: Record<string, string>;
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

export type AgentRunEventView = {
  runId: string;
  sequence: number;
  eventType: string;
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
export type AgentTranscriptItemView = {
  id: string;
  author: string;
  role: "assistant" | "user";
  kind: string;
  createdAt: string;
  taskStatus: string | null;
  content: string;
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  status?: "error" | "streaming" | null;
};

export type AgentTranscriptResponse = {
  items: AgentTranscriptItemView[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
  hasMore: boolean;
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
  runEvents: AgentRunEventView[];
  mutationLeaseOwners: AgentMutationLeaseOwnerView[];
  worktreeAction?: JsonValue | null;
};

// Provider types

type ProviderKindSummary = {
  kind: string;
  label: string;
  defaultBaseUrl: string;
  usesWebsocket?: boolean;
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

export type ProviderModelRedirect = {
  from: string;
  to: string;
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
  autoSyncModels: boolean;
  modelSyncFilterRegex: string | null;
  modelRedirects: ProviderModelRedirect[];
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
  autoSyncModels: boolean;
  modelSyncFilterRegex: string;
  modelRedirects: ProviderModelRedirect[];
  name: string;
  requestOverrides: ProviderRequestOverrideFormState[];
  serviceId: string;
};

export type ProviderApiKeyResponse = {
  apiKey: string;
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
  chatTitleGenerationModelId: string | null;
  defaultTeamModeEnabled: boolean;
  hookAuditEnabled: boolean;
  language: AppLanguageId;
  llmRequestRetryCount: number;
  maxLlmRequestRetryCount: number;
  runtimeToolStateCompressionEnabled: boolean;
  supportedLanguages: AppLanguageSummary[];
  supportedThemes: AppThemeSummary[];
  theme: AppThemeId;
  webServer: WebServerSettingsSummary;
};

export type GeneralFormState = {
  apiRequestDetailRetentionDays: string;
  apiSaveRequestResponseDetails: boolean;
  autoStartEnabled: boolean;
  chatTitleGenerationModelId: string;
  hookAuditEnabled: boolean;
  language: string;
  listenHost: string;
  listenPort: string;
  llmRequestRetryCount: string;
  password: string;
  runtimeToolStateCompressionEnabled: boolean;
  theme: AppThemeId;
};

type WebSearchProviderSummary = {
  provider: string;
  label: string;
  hasApiKey: boolean;
};

type WebSearchSettingsSummary = {
  enabled: boolean;
  /** Whether the active Tavily/Brave provider has a usable API key for function fallback. */
  fallbackAvailable?: boolean;
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
  defaultImageGenerationSystemPrompt?: string | null;
  defaultPlanModeSystemPrompt?: string | null;
  defaultReviewSystemPrompt?: string | null;
  systemPrompts?: SystemPromptSummary[];
  files: string[];
  extraText: string;
  /** Stored override only; null means use built-in default. */
  contextCompressionSystemPrompt?: string | null;
  /** Built-in default for internal contextCompression requests. */
  defaultContextCompressionSystemPrompt?: string;
  /** Stored override only; null means use built-in default. */
  memoryRetrievalSystemPrompt?: string | null;
  defaultMemoryRetrievalSystemPrompt?: string;
  /** Stored override only; null means use built-in default. */
  memoryExtractionSystemPrompt?: string | null;
  defaultMemoryExtractionSystemPrompt?: string;
  /** Stored override only; null means use built-in default. */
  memoryDreamSystemPrompt?: string | null;
  defaultMemoryDreamSystemPrompt?: string;
};

/** Shared override-editor field: display value + whether user customized it. */
export type PromptOverrideFieldState = {
  /** Editor display text (stored override or built-in default). */
  value: string;
  /**
   * When false, save submits null so the built-in default is used.
   * Distinguishes “restored default” from an override that happens to match.
   */
  custom: boolean;
};

export type PromptSettingsFormState = {
  activeSystemPromptName: string;
  systemPrompts: SystemPromptSummary[];
  files: string[];
  extraText: string;
  contextCompression: PromptOverrideFieldState;
  generationSystemPrompt: PromptOverrideFieldState;
  updateSystemPrompt: PromptOverrideFieldState;
  memoryRetrieval: PromptOverrideFieldState;
  memoryExtraction: PromptOverrideFieldState;
  memoryDream: PromptOverrideFieldState;
  pendingFile: string;
  pendingSystemPromptName: string;
};

type SpecSettingsSummary = {
  autoEnabled: boolean;
  generationModelId: string | null;
  generationSystemPrompt: string | null;
  updateSystemPrompt: string | null;
  llmTimeoutMs: number;
  defaultGenerationSystemPrompt: string;
  defaultUpdateSystemPrompt: string;
};

export type PlanMergeAutomationModeSummary = {
  value: string;
  label: string;
};

export type PlanSettingsSummary = {
  mergeAutomationMode: string;
  modeModelId: string | null;
  mergeAutomationModes: PlanMergeAutomationModeSummary[];
};

/** Spec automation-only form (prompts live on the Prompts page). */
export type SpecSettingsFormState = {
  autoEnabled: boolean;
  generationModelId: string;
  llmTimeoutMs: string;
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
  "manual" | "auto_interval" | "auto_threshold";

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
  llmTimeoutMs: number;
};

type MemorySettingsSummary = {
  enabled: boolean;
  extractionMode: string;
  retrievalMode: string;
  retentionDays: number | null;
  extractionModelId: string | null;
  retrievalModelId: string | null;
  extractionLlmTimeoutMs: number;
  retrievalLlmTimeoutMs: number;
  /** Percent of available message tokens for matched memories (1–100, default 12). */
  contextBudgetPercent: number;
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
  enabled: boolean;
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
  extractionLlmTimeoutMs: string;
  retrievalLlmTimeoutMs: string;
  /** Form string for 1–100 integer percent; default `"12"`. */
  contextBudgetPercent: string;
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
    llmTimeoutMs: string;
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

export type MemoryDreamPartialUnavailableReason =
  "notConnected" | "requestFailed" | "invalidResponse";

export type MemoryDreamPartialUnavailable = {
  workspaceId: string;
  reason: MemoryDreamPartialUnavailableReason;
  message: string;
};

export type MemoryDreamJobsResponse = {
  jobs: MemoryDreamJobSummary[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
  partialUnavailable?: MemoryDreamPartialUnavailable[];
};

export type MemoryDreamRunResponse = {
  jobId: string;
  status: MemoryDreamJobStatus;
  transcriptChatId: string | null;
  job?: MemoryDreamJobSummary;
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
  executionHost: "auto" | "local" | "workspace";
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
  executionHost: "auto" | "local" | "workspace";
  url: string;
};

// Skills types

export type SkillLocationSummary = {
  id: string;
  path: string;
  enabled: boolean;
};

type SkillsSettingsSummary = {
  directories: string[];
  locations?: SkillLocationSummary[];
  detected: ConfiguredSkillSummary[];
  errors: SkillDiscoveryErrorSummary[];
  translationModelId: string | null;
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
  store?: ConfiguredSkillStoreSummary | null;
};

type ConfiguredSkillStoreSummary = {
  skillId: string;
  source: string;
  updateable: boolean;
};

export type SkillDiscoveryErrorSummary = {
  path: string;
  message: string;
};

/** Workspace-scoped skill menu catalog from GET /api/workspaces/{id}/skills */
export type WorkspaceSkillsDiscoveryResponse = {
  skills: ConfiguredSkillSummary[];
  errors: SkillDiscoveryErrorSummary[];
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

/** Wire-derived LLM transport for audit list/detail (never from live Provider config). */
export type AiRequestTransport = "http" | "websocket" | "unknown";

export type AiRequestAuditSummary = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  chatId: string | null;
  chatTitle: string | null;
  requestKind: string;
  providerId: string;
  modelId: string;
  thinkingLevel: string | null;
  requestStartedAt: string;
  firstTokenAt: string | null;
  completedAt: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  reasoningTokens: number | null;
  cacheRatio: number | null;
  firstTokenLatencyMs: number | null;
  totalLatencyMs: number | null;
  statusCode: number | null;
  finalState: string;
  invalidatedAt: string | null;
  invalidatedReason: string | null;
  /** Derived from versioned request_body_json wire. */
  transport: AiRequestTransport;
};

export type ProviderHttpHeadersDump = Record<string, string | string[]>;

export type ProviderWireRequestDump = {
  body?: string;
  bodyEncoding?: string;
  format: "provider_request_v1";
  headers: ProviderHttpHeadersDump;
  method: string;
  url: string;
  version: number;
};

export type ProviderWebSocketHandshakeDump = {
  headers: ProviderHttpHeadersDump;
  status: number;
  version: string;
};

/** Real OpenAI Responses WebSocket request dump (`response.create` frame). */
export type ProviderWebSocketRequestDump = {
  connectionReused: boolean;
  createFrame?: string;
  createFrameEncoding?: string;
  /** True only after response.create was written to the socket. */
  frameSent?: boolean;
  format: "provider_websocket_request_v1";
  handshake?: ProviderWebSocketHandshakeDump | null;
  headers: ProviderHttpHeadersDump;
  url: string;
  version: number;
};

export type ProviderAuditRequestDump =
  ProviderWireRequestDump | ProviderWebSocketRequestDump;

export type ProviderHttpResponseHeadDump = {
  headers: ProviderHttpHeadersDump;
  status: number;
  version: string;
};

export type ProviderStreamDiagnosticPayloadDump =
  | {
      kind: "json";
      value: JsonValue;
    }
  | {
      kind: "utf8_excerpt";
      value: string;
    };

/**
 * A bounded, failure-only diagnostic captured by the OpenAI Responses stream
 * decoder. All fields remain optional so historical v1 response envelopes and
 * future decoder additions continue to load in the audit detail view.
 */
export type ProviderStreamDiagnosticDump = {
  eventType?: string | null;
  kind?: string;
  payload?: ProviderStreamDiagnosticPayloadDump;
  previousEventSequence?: number | null;
  previousEventType?: string | null;
  providerError?: {
    code?: string | null;
    errorType?: string | null;
    message?: string | null;
    param?: string | null;
    type?: string | null;
  };
  payloadTruncated?: boolean;
  rawPayloadBytes?: number;
  rawPayloadSha256?: string;
  transport?: string;
  transportError?: string;
};

/**
 * The audit writer may replace an oversized stream diagnostic with this bounded
 * sentinel. Its bytes and hash describe the stored diagnostic JSON, not the
 * original provider frame.
 */
export type ProviderCompactedStreamDiagnosticDump = {
  originalBytes?: number;
  sha256?: string;
  truncated: true;
};

export type ProviderFinalResponseDump =
  | {
      format: "provider_final_response_v1";
      http?: ProviderHttpResponseHeadDump | null;
      reasoning: string | null;
      responseId: string | null;
      state: "succeeded";
      stopReason: string | null;
      text: string;
      toolCalls: JsonValue[];
      usage: JsonValue | null;
      version: number;
    }
  | {
      error: string;
      format: "provider_final_response_v1";
      http?: ProviderHttpResponseHeadDump | null;
      partial: boolean;
      state: "failed";
      statusCode: number | null;
      streamDiagnostic?:
        | ProviderCompactedStreamDiagnosticDump
        | ProviderStreamDiagnosticDump
        | null;
      version: number;
    };

export type AiRequestAuditDetail = AiRequestAuditSummary & {
  requestDetailStatus?:
    "captured" | "failed" | "malformed" | "partial" | "pending" | "unavailable";
  responseDetailStatus?:
    "captured" | "failed" | "malformed" | "partial" | "pending" | "unavailable";
  requestBody: JsonValue | ProviderAuditRequestDump | null;
  responseBody: JsonValue | ProviderFinalResponseDump | null;
};

type AiStatisticsTrendPoint = {
  bucket: string;
  requestCount: number;
  totalTokens: number;
};

export type AiStatisticsRequestKindBreakdown = {
  requestKind: string;
  requestCount: number;
  failedRequests: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheWriteTokens: number;
  totalReasoningTokens: number;
  totalTokens: number;
  totalLatencyMs: number;
  averageLatencyMs: number | null;
};

export type AiStatisticsSummary = {
  averageLatencyMs: number | null;
  failedRequests: number;
  modelBreakdown: AiStatisticsModelBreakdown[];
  providerBreakdown: AiStatisticsProviderBreakdown[];
  requestKindBreakdown: AiStatisticsRequestKindBreakdown[];
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
  requestIds: string;
  chatId: string;
  providerId: string;
  modelId: string;
  requestKind: string;
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

export type TerminalPaneStatus =
  "closed" | "connected" | "connecting" | "error";

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
  sessionMode?: "plan" | null;
  runConfig?: ChatMessageRunConfigSummary | null;
  pendingMode?: "guidance" | "queued";
  queuedRun?: QueuedMessageRunSummary | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
  specUpdates: ChatSpecUpdateSummary[];
  runBadges?: ChatRunBadge[];
  /**
   * Synthetic user bubble source (e.g. reasoningLoopGuard / toolCallLoopGuard /
   * expanded history interruption). Marks non-editable virtual messages; layout
   * stays normal.
   */
  syntheticSource?: string;
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
  latencyMode?: LatencyMode;
  skillIds: string[];
  sessionMode?: "plan";
  teamModeEnabled?: boolean;
  idempotencyKey?: string;
  localChatKey?: string;
  pendingUserMessageId?: string;
  queuedUserMessageId?: string;
  assistantMessageId?: string;
};

export type LatencyMode = "standard" | "fast";

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
  "enabled" | "paused" | "completed" | "archived";

export type ScheduledTaskSchedule =
  | { type: "one_shot_at"; run_at: string }
  | { type: "interval"; every_seconds: number; start_at?: string | null }
  | { type: "cron"; expression: string; timezone?: string | null };

export type ScheduledSessionMode =
  "create_new_chat" | { reuse_chat: { chat_id: string } };

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
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
  statusCounts: Record<ScheduledTaskStatus, number> & Record<string, number>;
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
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
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
  // Durable assistant identity from the active-run owner. Optional while a
  // local POST is still waiting for its first start event.
  assistantMessageId?: string | null;
  assistantSequence?: number | null;
  queuedUserMessageId?: string | null;
  lastSequence?: number | null;
  acceptingGuidance: boolean;
};

// Settings response (aggregate type)

export type SettingsResponse = {
  appVersion: string;
  general: GeneralSettingsSummary;
  agentTools: string[];
  nativeTools: NativeToolsSummary;
  webSearch: WebSearchSettingsSummary;
  memory: MemorySettingsSummary;
  spec: SpecSettingsSummary;
  plan: PlanSettingsSummary;
  prompts: PromptSettingsSummary;
  workspaces: ConfiguredWorkspaceSummary[];
  remoteServers: RemoteServerSummary[];
  terminalShells: TerminalShellSummary[];
  providerKinds: ProviderKindSummary[];
  thinkingLevels: ThinkingLevelSummary[];
  providers: ConfiguredProviderSummary[];
  configuredModels: ConfiguredModelSummary[];
  mcpTransports: McpTransportSummary[];
  mcpServers: ConfiguredMcpServerSummary[];
  skills: SkillsSettingsSummary;
  about: AboutSettingsSummary;
  update: UpdateStatusSummary;
};

export type UpdateStatusSummary = {
  currentVersion: string;
  autoCheckEnabled: boolean;
  checking: boolean;
  lastCheckedAt: string | null;
  updateAvailable: boolean;
  targetVersion: string | null;
  releaseName: string | null;
  publishedAt: string | null;
  releaseUrl: string | null;
  assetName: string | null;
  assetDownloadUrl: string | null;
  error: string | null;
};

export type UpdateStatusResponse = UpdateStatusSummary;

export type AboutSettingsSummary = {
  version: string;
};
