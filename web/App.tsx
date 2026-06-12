import {
  Activity,
  ArrowDown,
  ArrowUp,
  BarChart3,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Code2,
  Copy,
  Download,
  Eye,
  EyeOff,
  FileText,
  Folder,
  FolderPlus,
  FolderSearch,
  GitBranch,
  GitCompare,
  Globe,
  GripVertical,
  Home,
  KeyRound,
  ListChecks,
  Lock,
  LogOut,
  LoaderCircle,
  MessageSquare,
  PanelRight,
  Pencil,
  Play,
  PlugZap,
  Plus,
  RefreshCw,
  Send,
  Server,
  Settings,
  SlidersHorizontal,
  SquareTerminal,
  ScrollText,
  SunMoon,
  Terminal,
  Trash2,
  Upload,
  User,
  Webhook,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  Children,
  CSSProperties,
  ChangeEvent as ReactChangeEvent,
  DragEvent as ReactDragEvent,
  FormEvent,
  ClipboardEvent as ReactClipboardEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  WheelEvent as ReactWheelEvent,
  createContext,
  isValidElement,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";

type ChatSummary = {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  codeChangeStats: GitDiffLineStats;
};

type WorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  logoUrl: string | null;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
  chats: ChatSummary[];
};

type WorkspaceCommonCommandSummary = {
  name: string;
  command: string;
};

type WorkspacesResponse = {
  activeWorkspaceId: string;
  workspaces: WorkspaceSummary[];
};

type ModelPricing = {
  input: number | null;
  output: number | null;
  reasoning: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
};

type ModelMetadataRecord = {
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

type ConfiguredModelSummary = {
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
  supportsThinking: boolean;
  warnings: string[];
};

type ModelMetadataResponse = {
  sourceUrl: string | null;
  fetchedAt: string | null;
  cachePath: string;
  models: ModelMetadataRecord[];
  configuredModels: ConfiguredModelSummary[];
};

type ModelFormState = {
  displayName: string;
  enabled: boolean;
  maxOutputTokens: string;
  modelId: string;
  contextWindow: string;
  providerIds: string[];
  activeProviderId: string;
  thinkingLevel: string;
};

type ProviderKindSummary = {
  kind: string;
  label: string;
  defaultBaseUrl: string;
};

type ThinkingLevelSummary = {
  value: string;
  label: string;
};

type ConfiguredProviderSummary = {
  apiProxy: ApiProxySettingsSummary;
  id: string;
  name: string;
  kind: string;
  kindLabel: string;
  enabled: boolean;
  baseUrl: string | null;
  hasApiKey: boolean;
  warnings: string[];
};

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
  ripgrep: RipgrepToolSummary;
};

type InstallRipgrepResponse = {
  ripgrep: RipgrepToolSummary;
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

type AppLanguageId = "en" | "zh-CN";
type AppThemeId = "light" | "dark";

type AppLanguageSummary = {
  id: AppLanguageId;
  name: string;
};

type AppThemeSummary = {
  id: AppThemeId;
  name: string;
};

type GeneralSettingsSummary = {
  hookAuditEnabled: boolean;
  language: AppLanguageId;
  supportedLanguages: AppLanguageSummary[];
  supportedThemes: AppThemeSummary[];
  theme: AppThemeId;
  webServer: WebServerSettingsSummary;
};

type MemoryExtractionModeSummary = {
  value: string;
  label: string;
};

type MemorySettingsSummary = {
  enabled: boolean;
  extractionMode: string;
  retrievalMode: string;
  retentionDays: number | null;
  extractionModelId: string | null;
  retrievalModelId: string | null;
  extractionModes: MemoryExtractionModeSummary[];
  retrievalModes: MemoryExtractionModeSummary[];
};

type PromptSettingsSummary = {
  files: string[];
  extraText: string;
};

type ConfiguredWorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  logoUrl: string | null;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
  isDefault: boolean;
};

type TerminalShellSummary = {
  shell: string;
  label: string;
};

type SettingsResponse = {
  general: GeneralSettingsSummary;
  nativeTools: NativeToolsSummary;
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

type MemoryFactRecord = {
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

type MemorySourceRecord = {
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

type MemoryExtractionJobSummary = {
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

type MemoryListResponse = {
  memories: MemoryFactRecord[];
  extractionJobs: MemoryExtractionJobSummary[];
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

type MemoryMutationResponse = {
  memory: MemoryFactRecord | null;
};

type ClearMemoriesResponse = {
  deletedCount: number;
};

type MemorySourcesResponse = {
  sources: MemorySourceRecord[];
};

type MemorySettingsFormState = {
  enabled: boolean;
  extractionMode: string;
  retrievalMode: string;
  retentionDays: string;
  extractionModelId: string;
  retrievalModelId: string;
};

type MemoryFilterState = {
  status: "active" | "pending";
  scope: "global" | "workspace" | "chat";
  kind: string;
  workspaceId: string;
  chatId: string;
  query: string;
  page: number;
  pageSize: number;
};

type MemoryListMeta = {
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

type ManualMemoryFormState = {
  scope: "global" | "workspace" | "chat";
  workspaceId: string;
  chatId: string;
  kind: string;
  fact: string;
  confidence: string;
  metadataText: string;
  pinned: boolean;
};

type MemorySourceFormState = {
  id: string;
  title: string;
  content: string;
  metadataText: string;
};

type MemoryDialogMode = "create" | "edit";

type ProviderFormState = {
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
};

type GeneralFormState = {
  hookAuditEnabled: boolean;
  language: string;
  listenHost: string;
  listenPort: string;
  password: string;
  theme: AppThemeId;
};

type PromptSettingsFormState = {
  files: string[];
  extraText: string;
  pendingFile: string;
};

type AuthStatusResponse = {
  authenticated: boolean;
  enabled: boolean;
};

type WorkspaceFormState = {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  terminalShell: string;
  commonCommands: WorkspaceCommonCommandSummary[];
};

type McpTransportSummary = {
  transport: string;
  label: string;
};

type ConfiguredMcpServerSummary = {
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

type McpServerFormState = {
  argsText: string;
  command: string;
  enabled: boolean;
  id: string;
  name: string;
  transport: string;
  url: string;
};

type SkillsSettingsSummary = {
  directories: string[];
  detected: ConfiguredSkillSummary[];
  errors: SkillDiscoveryErrorSummary[];
};

type ConfiguredSkillSummary = {
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

type HookHandlerType = "command" | "http" | "mcp_tool" | "prompt";

type HookConfig = {
  disableAllHooks?: boolean;
  [eventName: string]: boolean | HookMatcherGroup[] | undefined;
};

type HookMatcherGroup = {
  enabled?: boolean;
  matcher?: string | null;
  hooks: HookHandler[];
};

type HookHandler = {
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

type HookConfigScopeSummary = {
  source: string;
  path: string;
  workspaceId: string | null;
  config: HookConfig;
};

type EffectiveHookSummary = {
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

type HookRunSummaryRow = {
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

type HooksSettingsResponse = {
  supportedEvents: string[];
  unsupportedEvents: string[];
  global: HookConfigScopeSummary;
  workspace: HookConfigScopeSummary;
  effective: EffectiveHookSummary[];
  recentRuns: HookRunSummaryRow[];
};

type HookRunsResponse = {
  runs: HookRunSummaryRow[];
};

type ImportClaudeHooksResponse = {
  saved: boolean;
  target: "global" | "workspace" | string;
  path: string;
  importedFiles: string[];
  validationErrors: string[];
  config: HookConfig;
};

type HookDecision =
  | { type: "allow" }
  | { type: "ask"; reason: string }
  | { type: "block"; reason: string }
  | { type: "deny"; reason: string };

type HookRunSummary = {
  decisions: HookDecision[];
  additionalContext: string[];
  systemMessages: string[];
  errors: string[];
};

type HookRunDetail = HookRunSummaryRow & {
  input: JsonValue;
  output: JsonValue | null;
};

type HookRunDetailResponse = {
  run: HookRunDetail;
};

type HookScope = "global" | "workspace";

type HookHandlerFormState = {
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

type ProviderTestResponse = {
  ok: boolean;
  message: string;
  modelCount: number;
};

type ProviderTestState = {
  message: string;
  status: "error" | "ok" | "testing";
};

type JsonValue =
  | boolean
  | null
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

type AiRequestAuditSummary = {
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

type AiRequestAuditDetail = AiRequestAuditSummary & {
  requestBody: JsonValue | null;
  responseBody: JsonValue | null;
};

type AiStatisticsTrendPoint = {
  bucket: string;
  requestCount: number;
  totalTokens: number;
};

type AiStatisticsModelBreakdown = {
  modelId: string;
  requestCount: number;
  totalTokens: number;
};

type AiStatisticsProviderBreakdown = {
  averageLatencyMs: number | null;
  failedCount: number;
  providerId: string;
  requestCount: number;
  successCount: number;
  successRate: number | null;
  totalTokens: number;
};

type AiStatisticsSummary = {
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

type AiStatisticsResponse = {
  page: number;
  pageSize: number;
  requests: AiRequestAuditSummary[];
  summary: AiStatisticsSummary;
  totalCount: number;
  totalPages: number;
};

type AiRequestDetailResponse = {
  request: AiRequestAuditDetail;
};

type AiStatsFilterState = {
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

type AiStatsColumn = {
  cellClassName: string;
  headerClassName?: string;
  id: AiStatsColumnId;
  label: string;
  render: (request: AiRequestAuditSummary) => ReactNode;
};

type ChatToolCallSummary = {
  id: string;
  name: string;
  status: string;
  input: JsonValue;
  output: JsonValue | null;
  isError: boolean;
};

type ChatMessagePart =
  | { type: "text"; text: string }
  | { type: "error"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "attachment"; attachment: ChatAttachmentPartSummary }
  | { type: "toolCall"; toolCall: ChatToolCallSummary };

type ChatAttachmentPayload = {
  id: string;
  name: string;
  contentType: string;
  contentBase64?: string;
  path?: string;
  sizeBytes: number;
};

type ChatAttachmentPartSummary = {
  id: string;
  name: string;
  contentType: string;
  sizeBytes: number;
  path: string | null;
  previewDataUrl: string | null;
};

type ComposerAttachment = ChatAttachmentPayload & {
  previewDataUrl: string | null;
};

type NativeSelectedFile = {
  path: string;
  name: string;
  contentType: string;
  sizeBytes: number;
  contentBase64?: string | null;
};

type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
  createdAt: string;
  reasoning: string | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
};

type ActiveChatRunSummary = {
  runId: string;
  workspaceId: string;
  chatId: string;
  lastSequence: number | null;
};

type ChatMessagesResponse = {
  messages: ChatMessageSummary[];
  activeRun?: ActiveChatRunSummary | null;
};

type TaskStatus =
  | "pending"
  | "ready"
  | "running"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled";

type TodoGraphTask = {
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

type TodoGraphResponse = {
  chatId: string;
  exists: boolean;
  tasks: TodoGraphTask[];
  createdAt: string | null;
  updatedAt: string | null;
};

type ChatUsage = {
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
};

type ContextUsageResponse = {
  usedMessageTokens: number;
  availableMessageTokens: number;
  memoryContextTokens: number;
  memoryBudgetTokens: number;
  usagePercent: number;
  compressionTriggerTokens: number;
  compressionTriggerPercent: number;
  willCompressOnNextSend: boolean;
};

type ChatReplyMetrics = {
  modelId: string;
  providerId: string;
  totalLatencyMs: number | null;
  firstTokenLatencyMs: number | null;
  outputTokens: number | null;
};

type ChatMemoryUsedSummary = {
  id: string;
  scope: string;
  chatId: string | null;
  kind: string;
  fact: string;
  pinned: boolean;
  source: string;
};

type ChatExtractedMemorySummary = {
  id: string;
  scope: string;
  chatId: string | null;
  status: string;
  kind: string;
  fact: string;
};

type QuestionOptionSummary = {
  label: string;
  value: string;
  description: string | null;
};

type QuestionItemSummary = {
  id: string;
  question: string;
  options: QuestionOptionSummary[];
  allowFreeText: boolean;
};

type QuestionRequestSummary = {
  id: string;
  toolCallId: string;
  workspaceId: string;
  chatId: string;
  questions: QuestionItemSummary[];
};

type ChatStreamEvent =
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
    }
  | {
      type: "todoGraphRefresh";
      workspaceId: string;
      chatId: string;
    }
  | { type: "error"; message: string };

type HookNotificationSummary = {
  event: string;
  level: string;
  message: string;
};

type SettingsSection =
  | "general"
  | "prompts"
  | "hooks"
  | "memory"
  | "mcp"
  | "models"
  | "providers"
  | "skills"
  | "workspaces";
type ViewMode = "chat" | "settings" | "stats";
type ContextPanelTab = "todo" | "git" | "memory";
type BrowserRoute =
  | { viewMode: "chat"; workspaceId: string | null; chatId: string | null }
  | { viewMode: "settings"; section: SettingsSection }
  | { viewMode: "stats" };

const CREATE_BRANCH_OPTION_VALUE = "__create_branch__";
const CHAT_BOTTOM_LOCK_THRESHOLD_PX = 24;
const WORKSPACE_CHAT_HISTORY_PAGE_SIZE = 5;
const WORKSPACE_SIDEBAR_MIN_WIDTH = 232;
const WORKSPACE_SIDEBAR_MAX_WIDTH = 420;
const CONTEXT_PANEL_MIN_WIDTH = 280;
const CONTEXT_PANEL_MAX_WIDTH = 720;
const MAX_CHAT_ATTACHMENTS = 6;
const MAX_CHAT_ATTACHMENT_BYTES = 10 * 1024 * 1024;
const MAX_CHAT_ATTACHMENT_TOTAL_BYTES = 24 * 1024 * 1024;
const SAVED_PASSWORD_MASK = "********";
const SETTINGS_SECTION_IDS: SettingsSection[] = [
  "general",
  "prompts",
  "workspaces",
  "hooks",
  "memory",
  "providers",
  "models",
  "mcp",
  "skills",
];
const MEMORY_KIND_OPTIONS = [
  "user_note",
  "preference",
  "project_fact",
  "project_decision",
  "procedure",
  "constraint",
  "episode",
];
const AI_STATS_COLUMN_IDS = [
  "requestTime",
  "workspace",
  "chat",
  "provider",
  "model",
  "inputTokens",
  "outputTokens",
  "cacheRead",
  "cacheWrite",
  "cacheRatio",
  "latency",
  "firstToken",
  "statusCode",
  "status",
  "details",
] as const;
const AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY = "foco.aiStats.visibleColumns";
const DEFAULT_AI_STATS_COLUMN_IDS: AiStatsColumnId[] = [...AI_STATS_COLUMN_IDS];
const ANALYTICS_CHART_COLORS = [
  "#0f766e",
  "#2563eb",
  "#7c3aed",
  "#dc2626",
  "#ca8a04",
  "#16a34a",
  "#475569",
  "#db2777",
];
const chartTooltipStyle: CSSProperties = {
  backgroundColor: "#ffffff",
  border: "1px solid #e7e5e4",
  borderRadius: "10px",
  boxShadow: "0 12px 28px rgba(33, 31, 28, 0.14)",
  color: "#1c1917",
  fontSize: "12px",
};
const chartTooltipLabelStyle: CSSProperties = {
  color: "#57534e",
  fontWeight: 700,
  marginBottom: "4px",
};

type Translate = (key: string, values?: Record<string, string | number>) => string;
type AiStatsColumnId = (typeof AI_STATS_COLUMN_IDS)[number];

type MermaidRuntime = {
  initialize: (config: Record<string, unknown>) => void;
  render: (
    id: string,
    definition: string,
  ) => Promise<{
    bindFunctions?: (element: Element) => void;
    svg: string;
  }>;
};

const MERMAID_CONFIG: Record<string, unknown> = {
  flowchart: {
    curve: "basis",
  },
  htmlLabels: false,
  securityLevel: "strict",
  startOnLoad: false,
  theme: "base",
  themeVariables: {
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
    lineColor: "#78716c",
    primaryBorderColor: "#0f766e",
    primaryColor: "#f5f5f4",
    primaryTextColor: "#1c1917",
    secondaryBorderColor: "#a8a29e",
    secondaryColor: "#fafaf9",
    tertiaryColor: "#ccfbf1",
  },
};
let mermaidRuntimePromise: Promise<MermaidRuntime> | null = null;

const MARKDOWN_COMPONENTS: Components = {
  pre({ children, node: _node, ...props }) {
    const mermaidDefinition = mermaidDefinitionFromPreChildren(children);
    if (mermaidDefinition !== null) {
      return <MermaidDiagram definition={mermaidDefinition} />;
    }

    return <pre {...props}>{children}</pre>;
  },
};

const TRANSLATIONS: Record<AppLanguageId, Record<string, string>> = {
  en: {},
  "zh-CN": {
    "Local workspace": "本地工作区",
    "Refresh workspaces": "刷新工作区",
    "Add workspace": "添加工作区",
    "Collapse chat history": "收起聊天历史",
    "Expand chat history": "展开聊天历史",
    "Show {count} more chats": "继续展开 {count} 个会话",
    "Show {count} more chats in {name}": "在 {name} 中继续展开 {count} 个会话",
    "{count} hidden chats": "还有 {count} 个会话",
    "New chat": "新建聊天",
    "New chat in {name}": "在 {name} 中新建聊天",
    "Delete chat": "删除聊天",
    "Delete chat {title}": "删除聊天 {title}",
    "Delete this chat?": "删除此聊天？",
    "This will delete the saved chat history.": "这会删除已保存的聊天历史。",
    "Cancel chat deletion": "取消删除聊天",
    "Confirm delete chat": "确认删除聊天",
    "Close chat tab {title}": "关闭会话标签 {title}",
    "Scroll chat tabs left": "向左滚动会话标签",
    "Scroll chat tabs right": "向右滚动会话标签",
    "Chat is running": "会话运行中",
    "No chats": "暂无聊天",
    "No open chats": "暂无打开的会话",
    "Loading workspaces...": "正在加载工作区...",
    "No workspaces": "暂无工作区",
    Workspaces: "工作区",
    "Workspace settings": "工作区设置",
    "Workspace order and terminal shell": "工作区顺序与终端 Shell",
    "Workspace configuration": "工作区配置",
    "Close workspace configuration": "关闭工作区配置",
    "Edit workspace": "编辑工作区",
    "Workspace icon": "工作区图标",
    "Custom icon": "自定义图标",
    "Folder icon": "文件夹图标",
    "Clear workspace icon": "清除工作区图标",
    "Workspace icon file": "工作区图标文件",
    "Upload icon": "上传图标",
    "Workspace list": "工作区列表",
    "Terminal shell": "终端 Shell",
    "Common commands": "常用命令",
    "Command name": "命令名",
    "Add command": "添加命令",
    "Remove command": "移除命令",
    "Remove command {name}": "移除命令 {name}",
    "Pinned workspace": "置顶工作区",
    "Save workspace": "保存工作区",
    "Default workspace": "Default 工作区",
    pinned: "已置顶",
    "Pin workspace": "置顶工作区",
    "Unpin workspace": "取消置顶工作区",
    "Pin workspace {name}": "置顶工作区 {name}",
    "Unpin workspace {name}": "取消置顶工作区 {name}",
    "Edit workspace {name}": "编辑工作区 {name}",
    "Reorder workspace {name}": "调整工作区顺序 {name}",
    "All workspaces": "全部工作区",
    "All chats": "全部聊天",
    Workspace: "工作区",
    Chat: "聊天",
    Settings: "设置",
    Stats: "统计",
    "Close terminal": "关闭终端",
    "Close terminal {number}": "关闭终端 {number}",
    "Open terminal": "打开终端",
    "Close context panel": "关闭右侧面板",
    "Open context panel": "打开右侧面板",
    "Resize workspace sidebar": "调整工作区面板宽度",
    "Resize context panel": "调整右侧面板宽度",
    "Open git diff": "打开 Git diff",
    "Cancel the current run before deleting this chat.":
      "删除此聊天前请先取消当前运行。",
    "Select a workspace before creating a branch.":
      "创建分支前请先选择工作区。",
    "Git branch name must not be empty.": "Git 分支名不能为空。",
    "Select a workspace before sending.": "发送前请先选择工作区。",
    "Select an enabled model before sending.": "发送前请先选择已启用的模型。",
    "Add attachment": "添加附件",
    "Remove attachment {name}": "移除附件 {name}",
    "At most {count} attachments are allowed.": "最多允许 {count} 个附件。",
    "Attachment {name} exceeds the {size} limit.":
      "附件 {name} 超过 {size} 限制。",
    "Attachments exceed the {size} total limit.":
      "附件总大小超过 {size} 限制。",
    "Run cancelled.": "运行已取消。",
    "rg command was not found": "未找到 rg 命令",
    "Foco uses ripgrep for full-text search. Install it into {path} so the search_text tool can run.":
      "Foco 使用 ripgrep 执行全文搜索。请将它安装到 {path}，这样 search_text 工具才能运行。",
    "Download ripgrep": "下载 ripgrep",
    "Installing ripgrep...": "正在安装 ripgrep...",
    "Dismiss ripgrep warning": "关闭 ripgrep 提示",
    "ripgrep was installed.": "ripgrep 已安装。",
    "Foco needs your answer": "Foco 需要你的回答",
    "Waiting for your answer": "正在等待你的回答",
    "Custom answer": "手动输入",
    "Continue run": "继续运行",
    "Answer must not be empty.": "回答不能为空。",
    "Create or register a local folder.": "创建或注册本地文件夹。",
    "Close workspace dialog": "关闭工作区弹窗",
    Close: "关闭",
    Name: "名称",
    "Workspace name": "工作区名称",
    Path: "路径",
    "Choose workspace path": "选择工作区路径",
    Cancel: "取消",
    "Cancel workspace dialog": "取消工作区弹窗",
    "New branch": "新建分支",
    "Close branch dialog": "关闭分支弹窗",
    "Branch name": "分支名",
    "Cancel branch creation": "取消创建分支",
    "Create branch": "创建分支",
    "Remove skill": "移除技能",
    "Remove skill {name}": "移除技能 {name}",
    "Message Foco": "给 Foco 发送消息",
    "Ask Foco anything about {name}...": "询问 Foco 关于 {name} 的任何问题...",
    "Copy message": "复制消息",
    "Copied message": "已复制消息",
    "Select skill {name}": "选择技能 {name}",
    "Skill is disabled": "技能已禁用",
    "Skill locations": "技能位置",
    "Global skill": "全局技能",
    "Workspace skill": "工作区技能",
    "Workspace skill {name}": "工作区技能：{name}",
    disabled: "已禁用",
    "No matching skills": "没有匹配的技能",
    Model: "模型",
    "No enabled models": "没有已启用模型",
    Thinking: "思考",
    "Collapse thinking": "收起思考",
    "Expand thinking": "展开思考",
    "Thinking duration {duration}": "思考时长 {duration}",
    "Model default": "模型默认",
    "Retry last run": "重试上次运行",
    "Cancel run": "取消运行",
    "Send message": "发送消息",
    "Send guidance": "发送引导",
    "Send guidance. Ctrl+click queues.": "发送引导。Ctrl+点击进入队列。",
    "Send guidance. Ctrl+click queues. {count} queued.":
      "发送引导。Ctrl+点击进入队列。已排队 {count} 条。",
    "No active run is available for guidance.":
      "当前没有可引导的运行。",
    "Guidance pending": "引导待生效",
    Queued: "队列中",
    Send: "发送",
    "Context usage": "上下文使用量",
    "Context usage {percent}%": "上下文使用量 {percent}%",
    "Context compression may run on the next send":
      "下次发送可能会触发上下文压缩",
    "Git branch": "Git 分支",
    "Switch to branch {name}": "切换到分支 {name}",
    "No branches": "暂无分支",
    "Create git branch": "创建 Git 分支",
    Input: "输入",
    Output: "输出",
    "Mermaid diagram failed to render.": "Mermaid 图渲染失败。",
    error: "错误",
    connected: "已连接",
    connecting: "连接中",
    closed: "已关闭",
    "Terminal container was not mounted.": "终端容器尚未挂载。",
    "Terminal returned an unknown event.": "终端返回了未知事件。",
    "Terminal WebSocket failed.": "终端 WebSocket 失败。",
    "terminal exited: {status}": "终端已退出：{status}",
    "terminal error: {message}": "终端错误：{message}",
    "API details": "API 详情",
    "API overview": "API 概览",
    "API statistics": "API 详情",
    "Total requests": "总请求数",
    "Total tokens": "总 token 数",
    "Failed requests": "失败请求数",
    "Request trend": "请求数趋势",
    "Token trend": "Token 数趋势",
    "Tokens by model": "Token 按模型分布",
    "Requests by model": "请求数按模型分布",
    "Tokens by channel": "Token 按渠道分布",
    "Requests by channel": "请求数按渠道分布",
    "Channel success rate": "渠道请求成功率",
    "Channel response time": "渠道请求响应时间",
    "No statistics yet": "暂无统计数据",
    "No chart data": "暂无图表数据",
    "No workspace selected": "未选择工作区",
    "Workspace chats": "工作区聊天",
    "Enabled providers": "已启用供应商",
    "Runnable models": "可运行模型",
    "Configured models": "已配置模型",
    "Request audit": "请求审计",
    "Refresh request audit": "刷新请求审计",
    Columns: "列",
    "Recorded requests": "已记录请求",
    "Input tokens": "输入 token",
    "Output tokens": "输出 token",
    "Average latency": "平均延迟",
    "requests {count}": "请求 {count}",
    "All providers": "全部供应商",
    "All models": "全部模型",
    "All statuses": "全部状态",
    succeeded: "成功",
    failed: "失败",
    "Page size": "每页数量",
    "Showing {start}-{end} of {total}": "显示 {start}-{end}，共 {total}",
    "Page {page} of {totalPages}": "第 {page} 页，共 {totalPages} 页",
    "Previous page": "上一页",
    "Next page": "下一页",
    "Go to page {page}": "前往第 {page} 页",
    "Request audit pagination": "请求审计分页",
    "Started after": "开始时间不早于",
    "Started before": "开始时间不晚于",
    "Request time": "请求时间",
    Provider: "供应商",
    Channel: "渠道",
    "Total time": "总耗时",
    "tokens/s": "token/秒",
    Status: "状态",
    "Cache read": "缓存读取",
    "Cache write": "缓存写入",
    "Cache ratio": "缓存命中率",
    Latency: "延迟",
    "First token": "首 token",
    "First token latency": "首字延迟",
    "Status code": "状态码",
    Details: "详情",
    "View request details": "查看请求详情",
    "No recorded requests": "暂无已记录请求",
    "Request details": "请求详情",
    "Close request details": "关闭请求详情",
    "Request body": "请求正文",
    "Response body": "响应正文",
    Copy: "复制",
    Copied: "已复制",
    "Copy {label}": "复制 {label}",
    "Collapse all": "全部收起",
    "Expand all": "全部展开",
    "Collapse all {label}": "收起全部 {label}",
    "Expand all {label}": "展开全部 {label}",
    "Collapse JSON node": "收起 JSON 节点",
    "Expand JSON node": "展开 JSON 节点",
    "Resize git diff panel": "调整 Git diff 面板宽度",
    "Resize todo graph and git diff panels": "调整待办事项和 Git diff 面板高度",
    "Code changes +{additions} -{deletions}":
      "代码改动 +{additions} -{deletions}",
    "Resize terminal panel": "调整终端面板高度",
    "Run common command": "运行常用命令",
    "Run common command {name}": "运行常用命令 {name}",
    "Terminal is not connected.": "终端尚未连接。",
    "New terminal": "新建终端",
    "Terminal sessions": "终端列表",
    "Terminal {number}": "终端 {number}",
    "ToDo graph": "待办事项",
    "Updated {time}": "更新于 {time}",
    pending: "待处理",
    Active: "活跃",
    Default: "默认",
    ready: "就绪",
    running: "运行中",
    blocked: "阻塞",
    completed: "已完成",
    cancelled: "已取消",
    "Git diff": "Git diff",
    "Workspace changes": "工作区变更",
    "Refresh diff": "刷新 diff",
    "All changed files": "全部变更文件",
    "No changes": "无变更",
    "No diff": "无 diff",
    Staged: "已暂存",
    Unstaged: "未暂存",
    "Inline diff is unavailable for binary or non-text files.":
      "二进制或非文本文件无法展示 inline diff。",
    General: "常规",
    Prompts: "提示词",
    Providers: "供应商",
    Models: "模型",
    Skills: "技能",
    Memory: "记忆",
    "General settings": "常规设置",
    "Prompt settings": "提示词设置",
    "Provider settings": "供应商设置",
    "Model settings": "模型设置",
    "MCP settings": "MCP 设置",
    "Skill settings": "技能设置",
    "Memory settings": "记忆设置",
    "Memories saved": "已保存记忆",
    "Local memory graph and review queue": "本地记忆图与审核队列",
    "Memory controls": "记忆控制",
    "Enable memory": "启用记忆",
    "General memory control": "记忆总控",
    "Controls whether memory tools, retrieval, and extraction are available.":
      "控制记忆工具、检索和抽取是否可用。",
    "Memory extraction": "记忆抽取",
    "Controls how new facts are extracted and how long they are retained.":
      "控制新事实如何抽取，以及保留多久。",
    "Memory retrieval": "记忆匹配",
    "Controls how existing memory is matched into chat context.":
      "控制已有记忆如何匹配进聊天上下文。",
    "Extraction mode": "抽取模式",
    "Memory matching": "记忆匹配",
    "Retention days": "保留天数",
    "Extraction model": "抽取模型",
    "Matching model": "匹配模型",
    "SQLite FTS": "SQLite FTS",
    "Model matching": "大模型匹配",
    "Current chat model": "当前会话模型",
    "Save memory settings": "保存记忆设置",
    "Memory list": "记忆列表",
    "Create memory": "创建记忆",
    "Close memory dialog": "关闭记忆弹窗",
    "Delete memory": "删除记忆",
    "Delete memory confirmation": "确定要删除这条记忆吗？",
    "Clear filtered workspace memories": "清空当前筛选的工作区记忆",
    "Clear filtered chat memories": "清空当前筛选的会话记忆",
    "Clear filtered memories confirmation": "确定要清空当前筛选范围内的记忆吗？",
    "Memory pagination": "记忆分页",
    "Memory scope": "记忆范围",
    "Memory status": "记忆状态",
    "Search memories": "搜索记忆",
    "Refresh memories": "刷新记忆",
    "No memories": "暂无记忆",
    "All memory kinds": "全部记忆类型",
    "Pending review": "待审核",
    Rejected: "已拒绝",
    Expired: "已过期",
    Superseded: "已取代",
    "User note": "用户备注",
    Preference: "偏好",
    "Project fact": "项目事实",
    "Project decision": "项目决策",
    Procedure: "流程",
    Constraint: "约束",
    Episode: "片段",
    Automatic: "自动",
    "Manual": "手动",
    Disabled: "已禁用",
    "Workspace memory": "工作区记忆",
    "Chat memory": "会话记忆",
    "Chat ID": "会话 ID",
    "Global memory": "全局记忆",
    "Memory fact": "记忆事实",
    "Memory kind": "记忆类型",
    "Pinned memory": "置顶记忆",
    "Confidence": "置信度",
    "Memory metadata": "记忆元数据",
    "Memory details": "记忆详情",
    "Memory source details": "记忆来源详情",
    "Source title": "来源标题",
    "Source content": "来源内容",
    "Source metadata": "来源元数据",
    "Source type": "来源类型",
    "Source ID": "来源 ID",
    Latest: "最新",
    "Expires at": "过期时间",
    Created: "创建时间",
    Updated: "更新时间",
    Yes: "是",
    No: "否",
    "Expand JSON": "展开 JSON",
    "Collapse JSON": "折叠 JSON",
    "Approve memory": "批准记忆",
    "Reject memory": "拒绝记忆",
    "Forget memory": "遗忘记忆",
    "Promote memory": "提升记忆",
    "Promote one level": "提升一级",
    "Promote to workspace": "提升到工作区",
    "Promote to global": "提升到全局",
    "Memory sources": "记忆来源",
    "No memory sources": "暂无记忆来源",
    "Edit memory": "编辑记忆",
    "Save memory": "保存记忆",
    "Extraction failures": "抽取失败",
    "No extraction failures": "暂无抽取失败",
    "Memory extraction failed": "记忆抽取失败",
    "Web service listen address": "Web 服务监听地址",
    "Prompt files and extra instructions": "提示词文件与额外指令",
    "Provider credentials and connection checks": "供应商凭据与连接检查",
    "Workspace-scoped MCP server runtimes": "工作区级 MCP 服务运行时",
    "Skill discovery and enablement": "技能发现与启用",
    "Model metadata and runtime limits": "模型元数据与运行限制",
    "Fetched {time} from {source}": "已从 {source} 获取：{time}",
    "Model metadata has not been refreshed": "尚未刷新模型元数据",
    "Refresh model metadata": "刷新模型元数据",
    "Web service": "Web 服务",
    "Listen address": "监听地址",
    "Listen port": "监听端口",
    "Browser authentication": "浏览器认证",
    "Authentication password": "认证密码",
    "Password required": "需要密码",
    Password: "密码",
    "Show password": "显示密码",
    "Hide password": "隐藏密码",
    "Log in": "登录",
    "Log out": "退出登录",
    "Password is enabled": "已启用密码",
    "Password is disabled": "未启用密码",
    "New password is kept empty unless changed.":
      "不填写则保留当前密码。",
    "Saved password cannot be revealed; type a new password to preview it.":
      "已保存的密码无法显示；输入新密码后可预览。",
    "Set a password to require browser login.":
      "设置密码后，浏览器访问需要先登录。",
    "Clear browser password": "清除浏览器密码",
    Language: "语言",
    Theme: "主题",
    Light: "浅色",
    Dark: "深色",
    "Switch to light theme": "切换到浅色主题",
    "Switch to dark theme": "切换到深色主题",
    "Save general settings": "保存常规设置",
    Save: "保存",
    "Reload general settings": "重新加载常规设置",
    "Prompt files": "提示词文件",
    "Prompt file path": "提示词文件路径",
    "Add prompt file": "添加提示词文件",
    "Choose prompt file": "选择提示词文件",
    "Remove prompt file": "移除提示词文件",
    "Remove prompt file {path}": "移除提示词文件 {path}",
    "No prompt files": "暂无提示词文件",
    "Extra prompt": "额外提示词",
    "Save prompt settings": "保存提示词设置",
    "Reload prompt settings": "重新加载提示词设置",
    "Reload settings": "重新加载设置",
    Reload: "重新加载",
    "Saved bind": "已保存绑定",
    "restart required": "需要重启",
    "Loading...": "正在加载...",
    "Saved host and port are used the next time the backend starts.":
      "已保存的 host 和端口会在后端下次启动时生效。",
    "Language changes apply immediately after saving.":
      "语言设置保存后会立即生效。",
    "Theme changes apply immediately after saving.":
      "主题设置保存后会立即生效。",
    "Hook settings": "钩子设置",
    "Global and workspace lifecycle hooks": "全局与工作区生命周期钩子",
    "Hook run detail": "钩子运行详情",
    "Hook configuration": "钩子配置",
    "Add hook": "添加钩子",
    "Edit hook": "编辑钩子",
    Hooks: "钩子",
    Global: "全局",
    "Global hooks": "全局钩子",
    "Workspace hooks": "工作区钩子",
    Event: "事件",
    "Matcher": "匹配器",
    "Enable hook": "启用钩子",
    "Handler type": "处理器类型",
    HTTP: "HTTP",
    "MCP tool": "MCP 工具",
    "If filter": "if 过滤器",
    "Shell": "Shell",
    "Timeout ms": "超时 ms",
    "Async": "异步",
    "Async re-wake": "异步唤醒",
    "Status message": "状态消息",
    "Input override JSON": "输入覆盖 JSON",
    "Save hook": "保存钩子",
    "Hook rules": "钩子规则",
    "Disable all hooks": "禁用全部钩子",
    "Record hook run logs": "记录钩子运行日志",
    "Import Claude hooks": "导入 Claude 钩子",
    "Import to global hooks": "导入到全局钩子",
    "Import to workspace hooks": "导入到工作区钩子",
    "Global import reads user Claude settings; workspace import reads the selected workspace.":
      "全局导入读取用户级 Claude 设置；工作区导入读取当前选择的工作区。",
    "Import saved": "导入已保存",
    "Import not saved": "导入未保存",
    "Test hook": "测试钩子",
    "Match value": "匹配值",
    "Sample payload": "示例载荷",
    "Run hook test": "运行钩子测试",
    "Effective hooks": "生效的钩子",
    "Recent hook runs": "最近钩子运行",
    "No hook rules": "暂无钩子规则",
    "No effective hooks": "暂无生效钩子",
    "No hook runs": "暂无钩子运行",
    "Refresh hook runs": "刷新钩子运行",
    "Close hook configuration": "关闭钩子配置",
    "Close hook run detail": "关闭钩子运行详情",
    "Running hook": "正在运行钩子",
    "Session start": "会话开始",
    "Session end": "会话结束",
    "User prompt submit": "用户提交",
    "Pre tool use": "工具调用前",
    "Permission request": "权限请求",
    "Permission denied": "权限拒绝",
    "Post tool use": "工具调用后",
    "Post tool use failure": "工具调用失败后",
    "Post tool batch": "工具批次后",
    Stop: "停止",
    "Stop failure": "停止失败",
    "Pre compact": "压缩前",
    "Post compact": "压缩后",
    Elicitation: "询问",
    "Elicitation result": "询问结果",
    "Hook asks whether to allow tool '{toolName}': {reason}":
      "钩子询问是否允许工具 '{toolName}'：{reason}",
    "Return a JSON hook result.": "返回 JSON 格式的钩子结果。",
    "Select a workspace first.": "请先选择工作区。",
    "Prompt": "提示词",
    "MCP server id": "MCP 服务 ID",
    "MCP tool name": "MCP 工具名",
    "rules {count}": "规则 {count}",
    "handlers {count}": "处理器 {count}",
    "hooks {count}": "钩子 {count}",
    "last {status}": "最近 {status}",
    "Reload hooks": "重新加载钩子",
    "Move hook up": "上移钩子",
    "Move hook down": "下移钩子",
    "Move handler up": "上移处理器",
    "Move handler down": "下移处理器",
    "Delete hook": "删除钩子",
    "Enable hook group": "启用钩子组",
    "Failed to render.": "渲染失败。",
    "AI API proxy": "代理服务器",
    "Proxy enabled": "代理已启用",
    "Proxy disabled": "代理已禁用",
    "Enable AI API proxy": "启用代理服务器",
    "Proxy type": "代理类型",
    "Proxy server": "代理服务器",
    "Provider configuration": "供应商配置",
    "Edit provider": "编辑供应商",
    "Add provider": "添加供应商",
    "Enable provider": "启用供应商",
    "Delete provider": "删除供应商",
    "Close provider configuration": "关闭供应商配置",
    Protocol: "协议",
    "Base URL": "Base URL",
    "API key": "API key",
    "Saved key is kept unless replaced": "已保存的 key 会保留，除非填写新值",
    "Clear saved API key": "清除已保存的 API key",
    "Save provider": "保存供应商",
    "Configured providers": "已配置供应商",
    enabled: "已启用",
    "key saved": "已保存 key",
    "key missing": "缺少 key",
    "Edit provider {name}": "编辑供应商 {name}",
    "Test provider {name}": "测试供应商 {name}",
    "Test provider": "测试供应商",
    "No configured providers": "暂无已配置供应商",
    "Testing connection...": "正在测试连接...",
    "MCP server configuration": "MCP 服务配置",
    "Edit MCP server": "编辑 MCP 服务",
    "Add MCP server": "添加 MCP 服务",
    "Enable MCP server": "启用 MCP 服务",
    "Delete MCP server": "删除 MCP 服务",
    "Close MCP server configuration": "关闭 MCP 服务配置",
    Transport: "传输方式",
    Stdio: "标准输入输出",
    "Streamable HTTP": "Streamable HTTP",
    URL: "URL",
    Command: "命令",
    Args: "参数",
    "Save MCP server": "保存 MCP 服务",
    "MCP servers": "MCP 服务",
    "Reload MCP settings": "重新加载 MCP 设置",
    "tools {count}": "工具 {count}",
    "Edit MCP server {name}": "编辑 MCP 服务 {name}",
    "No configured MCP servers": "暂无已配置 MCP 服务",
    stopped: "已停止",
    "Refresh skill discovery": "刷新技能发现",
    Refresh: "刷新",
    "Detected skills": "已发现技能",
    "skills {count}": "技能 {count}",
    "Enable skill {name}": "启用技能 {name}",
    "No detected skills": "暂无已发现技能",
    "Edit model {name}": "编辑模型 {name}",
    "Reorder model {name}": "调整模型顺序 {name}",
    "Add model": "添加模型",
    "Edit model": "编辑模型",
    "Delete model": "删除模型",
    "Close model configuration": "关闭模型配置",
    "Model configuration": "模型配置",
    "Model id": "模型 ID",
    "Display name": "显示名称",
    "Context window": "上下文窗口",
    "Max output tokens": "最大输出 token",
    "Enable model": "启用模型",
    "limits ok": "limits 已就绪",
    "limits missing": "limits 缺失",
    "providers {count}": "供应商 {count}",
    "active {id}": "当前 {id}",
    "active missing": "缺少当前供应商",
    "No configured models": "暂无已配置模型",
    "No providers": "暂无供应商",
    "Active provider": "当前供应商",
    "Thinking level": "思考级别",
    None: "无",
    "Fill both limits before enabling.": "启用前请填写两个 limits。",
    "Save model": "保存模型",
    "pricing in/out:": "价格 输入/输出：",
    "Search model metadata": "搜索模型元数据",
    "Reload model metadata cache": "重新加载模型元数据缓存",
    "Reload cache": "重新加载缓存",
    "input n/a": "输入不可用",
    "Loading models...": "正在加载模型...",
    "No cached models": "暂无缓存模型",
    Minimal: "最小",
    Low: "低",
    Medium: "中",
    High: "高",
    "Extra High": "极高",
    "Listen port must be a positive whole number": "监听端口必须是正整数",
    Unknown: "未知错误",
    "Unknown error": "未知错误",
  },
};

const I18nContext = createContext<{
  language: AppLanguageId;
  t: Translate;
}>({
  language: "en",
  t: translate,
});

function useI18n() {
  return useContext(I18nContext);
}

function translate(
  key: string,
  values: Record<string, string | number> = {},
  language: AppLanguageId = "en",
) {
  const template = TRANSLATIONS[language][key] ?? key;

  return Object.entries(values).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    template,
  );
}

type GitStatusFileSummary = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
};

type GitDiffResponse = {
  path: string | null;
  status: string;
  diff: string;
  stagedDiff: string;
  files: GitStatusFileSummary[];
};

type GitDiffLineStats = {
  additions: number;
  deletions: number;
};

type GitBranchesResponse = {
  isGitRepository: boolean;
  currentBranch: string | null;
  branches: string[];
};

type TerminalSessionResponse = {
  id: string;
  name: string;
  workingDirectory: string;
};

type TerminalServerEvent =
  | { type: "started"; cwd: string }
  | { type: "output"; data: string }
  | { type: "cwd"; cwd: string }
  | { type: "exit"; status: string }
  | { type: "error"; message: string };

type TerminalPaneStatus = "closed" | "connected" | "connecting" | "error";

type TerminalCommandRun = {
  input: string;
};

type TerminalPanelSession = {
  clientId: string;
  cwd: string;
  error: string | null;
  number: number;
  pendingCommand: TerminalCommandRun | null;
  serverSessionId: string | null;
  status: TerminalPaneStatus;
};

type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  createdAt: string;
  reasoning: string | null;
  status?: "error" | "streaming";
  pendingMode?: "guidance" | "queued";
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
  memoriesUsed: ChatMemoryUsedSummary[];
  extractedMemories: ChatExtractedMemorySummary[];
};

type OpenChatTab = {
  workspaceId: string;
  chatId: string;
  fallbackTitle: string;
  fallbackWorkspaceName: string;
};

type ChatTabSummary = OpenChatTab & {
  title: string;
  workspaceLogoUrl: string | null;
  workspaceName: string;
};

type PendingDeleteChat = {
  workspaceId: string;
  chatId: string;
  title: string;
  workspaceName: string;
};

type RetryRunRequest = {
  workspaceId: string;
  chatId: string | null;
  content: string;
  attachments: ChatAttachmentPayload[];
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
  pendingUserMessageId?: string;
};

type ActiveRunInfo = {
  workspaceId: string;
  chatId: string | null;
  runId: string | null;
  chatKey: string;
  lastSequence?: number | null;
};

type ContextUsageRefreshRequest = {
  workspaceId: string;
  chatId: string | null;
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
  assistantDraft: string;
  assistantDraftReasoning: string;
  latestResponseUsage: ChatUsage | null;
};

type ContextMemoryState = {
  global: MemoryFactRecord[];
  workspace: MemoryFactRecord[];
};

type QuestionAnswerSubmission = {
  answers: {
    id: string;
    answer: string;
    selectedOptionValue: string | null;
  }[];
};

export function App() {
  const [initialBrowserRoute] = useState(() => currentBrowserRoute());
  const [authStatus, setAuthStatus] = useState<AuthStatusResponse | null>(null);
  const [authPassword, setAuthPassword] = useState("");
  const [isCheckingAuth, setIsCheckingAuth] = useState(true);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>("");
  const [expandedWorkspaceId, setExpandedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceChatVisibleCounts, setWorkspaceChatVisibleCounts] = useState<
    Record<string, number>
  >({});
  const [viewMode, setViewMode] = useState<ViewMode>(
    initialBrowserRoute.viewMode,
  );
  const [settingsSection, setSettingsSection] = useState<SettingsSection>(
    initialBrowserRoute.viewMode === "settings"
      ? initialBrowserRoute.section
      : "general",
  );
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceDialogRevision, setWorkspaceDialogRevision] = useState(0);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [draftMessage, setDraftMessage] = useState("");
  const [draftAttachments, setDraftAttachments] = useState<ComposerAttachment[]>(
    [],
  );
  const [messages, setMessages] = useState<ShellMessage[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [openChatTabs, setOpenChatTabs] = useState<OpenChatTab[]>([]);
  const [pendingDeleteChat, setPendingDeleteChat] =
    useState<PendingDeleteChat | null>(null);
  const [chatMessagesByKey, setChatMessagesByKey] = useState<
    Record<string, ShellMessage[]>
  >({});
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [selectedThinkingLevel, setSelectedThinkingLevel] = useState("");
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
  const [gitBranches, setGitBranches] = useState<GitBranchesResponse | null>(null);
  const [selectedGitBranch, setSelectedGitBranch] = useState("");
  const [isLoadingBranches, setIsLoadingBranches] = useState(false);
  const [branchError, setBranchError] = useState<string | null>(null);
  const [isBranchDialogOpen, setIsBranchDialogOpen] = useState(false);
  const [newBranchName, setNewBranchName] = useState("");
  const [isSavingBranch, setIsSavingBranch] = useState(false);
  const [isContextPanelOpen, setIsContextPanelOpen] = useState(
    () => typeof window !== "undefined" && window.innerWidth >= 768,
  );
  const [contextPanelTab, setContextPanelTab] =
    useState<ContextPanelTab>("todo");
  const [diffPanelWidth, setDiffPanelWidth] = useState(CONTEXT_PANEL_MIN_WIDTH);
  const [isResizingDiffPanel, setIsResizingDiffPanel] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(WORKSPACE_SIDEBAR_MIN_WIDTH);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [isMobileWorkspaceOpen, setIsMobileWorkspaceOpen] = useState(false);
  const [terminalOpenWorkspaceIds, setTerminalOpenWorkspaceIds] = useState<
    Set<string>
  >(() => new Set());
  const [gitDiff, setGitDiff] = useState<GitDiffResponse | null>(null);
  const [selectedDiffPath, setSelectedDiffPath] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [todoGraph, setTodoGraph] = useState<TodoGraphResponse | null>(null);
  const [isLoadingTodoGraph, setIsLoadingTodoGraph] = useState(false);
  const [todoGraphError, setTodoGraphError] = useState<string | null>(null);
  const [contextMemories, setContextMemories] = useState<ContextMemoryState>({
    global: [],
    workspace: [],
  });
  const [isLoadingContextMemories, setIsLoadingContextMemories] =
    useState(false);
  const [contextMemoryError, setContextMemoryError] = useState<string | null>(
    null,
  );
  const [deletingContextMemoryId, setDeletingContextMemoryId] = useState<
    string | null
  >(null);
  const [runningChatKeys, setRunningChatKeys] = useState<Set<string>>(
    () => new Set(),
  );
  const [failedChatKeySet, setFailedChatKeySet] = useState<Set<string>>(
    () => new Set(),
  );
  const [retryRunRequest, setRetryRunRequest] =
    useState<RetryRunRequest | null>(null);
  const [queuedRunRequestsByChatKey, setQueuedRunRequestsByChatKey] = useState<
    Record<string, RetryRunRequest[]>
  >({});
  const [activeRunInfoByChatKey, setActiveRunInfoByChatKey] = useState<
    Record<string, ActiveRunInfo>
  >({});
  const [contextUsage, setContextUsage] =
    useState<ContextUsageResponse | null>(null);
  const [isLoadingContextUsage, setIsLoadingContextUsage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingTheme, setIsSavingTheme] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSelectingWorkspacePath, setIsSelectingWorkspacePath] = useState(false);
  const [isSelectingAttachments, setIsSelectingAttachments] = useState(false);
  const [pendingQuestion, setPendingQuestion] =
    useState<QuestionRequestSummary | null>(null);
  const [isAnsweringQuestion, setIsAnsweringQuestion] = useState(false);
  const [questionError, setQuestionError] = useState<string | null>(null);
  const [isRipgrepDialogDismissed, setIsRipgrepDialogDismissed] = useState(false);
  const [isInstallingRipgrep, setIsInstallingRipgrep] = useState(false);
  const [ripgrepInstallError, setRipgrepInstallError] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const activeRunAbortByChatKeyRef = useRef<Map<string, AbortController>>(
    new Map(),
  );
  const contextUsageAbortRef = useRef<AbortController | null>(null);
  const contextUsageIdentityRef = useRef("");
  const contextUsageRequestIdRef = useRef(0);
  const activeChatKeyRef = useRef<string | null>(null);
  const queuedRunRequestsByChatKeyRef = useRef<
    Record<string, RetryRunRequest[]>
  >({});
  const pendingGuidanceMessageIdsRef = useRef<Map<string, string>>(new Map());
  const applyBrowserRouteRef = useRef<(route: BrowserRoute) => void>(() => {});
  const hasAppliedInitialBrowserRouteRef = useRef(false);
  const hasManuallySelectedModelRef = useRef(false);
  const workspaceSidebarRef = useRef<HTMLElement | null>(null);

  const activeWorkspace = useMemo(
    () =>
      workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
      workspaces[0],
    [activeWorkspaceId, workspaces],
  );
  const activeChatKey =
    activeChatId === null
      ? activeChatKeyRef.current
      : chatRunKey(activeWorkspaceId, activeChatId);
  const activeRunInfo = activeChatKey
    ? activeRunInfoByChatKey[activeChatKey] ?? null
    : null;
  const isSendingMessage =
    activeChatKey !== null && runningChatKeys.has(activeChatKey);
  const queuedRunRequests = activeChatKey
    ? queuedRunRequestsByChatKey[activeChatKey] ?? []
    : [];
  const chatTabs = useMemo(
    () => openChatTabs.map((tab) => hydrateChatTab(tab, workspaces)),
    [openChatTabs, workspaces],
  );
  const openChatKeySet = useMemo(
    () =>
      new Set(
        openChatTabs.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
      ),
    [openChatTabs],
  );
  const availableModels = useMemo(
    () =>
      (settings?.configuredModels ?? []).filter(
        (model) =>
          model.enabled &&
          model.canEnable &&
          model.activeProviderId !== null &&
          model.providerIds.length > 0,
      ),
    [settings],
  );
  const detectedSkills = useMemo(
    () => settings?.skills.detected ?? [],
    [settings],
  );
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const isTerminalOpen = activeWorkspace
    ? terminalOpenWorkspaceIds.has(activeWorkspace.id)
    : false;
  const isGlobalView = viewMode === "settings" || viewMode === "stats";
  const showContextPanel = !isGlobalView && isContextPanelOpen;
  const canUseApp = Boolean(
    authStatus && (!authStatus.enabled || authStatus.authenticated),
  );
  const canLogout = Boolean(settings?.general.webServer.passwordEnabled);
  const language = settings?.general.language ?? "en";
  const theme = settings?.general.theme ?? "light";
  const t = useCallback<Translate>(
    (key, values) => translate(key, values, language),
    [language],
  );
  const updateSidebarWidthFromClientX = useCallback((clientX: number) => {
    const sidebarLeft =
      workspaceSidebarRef.current?.getBoundingClientRect().left ?? 0;
    const nextWidth = clientX - sidebarLeft;

    setSidebarWidth(
      Math.min(
        Math.max(nextWidth, WORKSPACE_SIDEBAR_MIN_WIDTH),
        WORKSPACE_SIDEBAR_MAX_WIDTH,
      ),
    );
  }, []);
  const updateBrowserRoute = useCallback(
    (route: BrowserRoute, mode: "push" | "replace" = "push") => {
      if (typeof window === "undefined") {
        return;
      }

      const nextPath = browserPathForRoute(route);
      if (window.location.pathname === nextPath) {
        return;
      }

      if (mode === "replace") {
        window.history.replaceState(null, "", nextPath);
        return;
      }

      window.history.pushState(null, "", nextPath);
    },
    [],
  );

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  useEffect(() => {
    document.documentElement.dataset.focoTheme = theme;
  }, [theme]);

  useEffect(() => {
    activeChatKeyRef.current =
      activeChatId === null ? null : chatRunKey(activeWorkspaceId, activeChatId);
  }, [activeChatId, activeWorkspaceId]);

  useEffect(() => {
    const identity = [
      activeWorkspaceId,
      activeChatId ?? "",
      selectedModelId,
      selectedProviderId,
      selectedThinkingLevel,
    ].join("\u0000");

    if (contextUsageIdentityRef.current === identity) {
      return;
    }

    contextUsageIdentityRef.current = identity;
    if (isSendingMessage) {
      return;
    }

    contextUsageAbortRef.current?.abort();
    setContextUsage(null);
    setIsLoadingContextUsage(false);

    if (
      !activeWorkspaceId ||
      !activeChatId ||
      !selectedModelId ||
      !selectedProviderId
    ) {
      contextUsageRequestIdRef.current += 1;
      return;
    }

    void refreshContextUsage({
      assistantDraft: "",
      assistantDraftReasoning: "",
      chatId: activeChatId,
      latestResponseUsage: null,
      modelId: selectedModelId,
      providerId: selectedProviderId,
      skillIds: [],
      thinkingLevel: selectedThinkingLevel,
      workspaceId: activeWorkspaceId,
    });
  }, [
    activeChatId,
    activeWorkspaceId,
    isSendingMessage,
    selectedModelId,
    selectedProviderId,
    selectedThinkingLevel,
  ]);

  useEffect(
    () => () => {
      contextUsageAbortRef.current?.abort();
    },
    [],
  );

  const loadAuthStatus = useCallback(async () => {
    setIsCheckingAuth(true);
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/status");
      setAuthStatus(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsCheckingAuth(false);
    }
  }, []);

  const refreshWorkspaces = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>("/api/workspaces");
      setWorkspaces(data.workspaces);
      setActiveWorkspaceId((current) =>
        data.workspaces.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );
      setExpandedWorkspaceId((current) =>
        current !== null &&
        data.workspaces.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadSettings = useCallback(async () => {
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      setSettings(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, []);

  const handleInstallRipgrep = useCallback(async () => {
    setIsInstallingRipgrep(true);
    setRipgrepInstallError(null);

    try {
      const data = await requestJson<InstallRipgrepResponse>(
        "/api/native/install-ripgrep",
        {
          method: "POST",
        },
      );
      setSettings((current) =>
        current
          ? {
              ...current,
              nativeTools: {
                ...current.nativeTools,
                ripgrep: data.ripgrep,
              },
            }
          : current,
      );
      setIsRipgrepDialogDismissed(true);
    } catch (requestError) {
      setRipgrepInstallError(errorMessage(requestError));
    } finally {
      setIsInstallingRipgrep(false);
    }
  }, []);

  const loadGitDiff = useCallback(async (workspaceId: string, path: string | null) => {
    setIsLoadingDiff(true);
    setDiffError(null);

    try {
      const query = path ? `?path=${encodeURIComponent(path)}` : "";
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/diff${query}`,
      );
      setGitDiff(data);
      setSelectedDiffPath(path && data.files.some((file) => file.path === path) ? path : null);
      return data;
    } catch (requestError) {
      setGitDiff(null);
      setDiffError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingDiff(false);
    }
  }, []);

  const loadContextMemories = useCallback(async (workspaceId: string) => {
    setIsLoadingContextMemories(true);
    setContextMemoryError(null);

    try {
      const globalParams = new URLSearchParams({
        limit: "20",
        scope: "global",
        status: "active",
      });
      const workspaceParams = new URLSearchParams({
        limit: "20",
        scope: "workspace",
        status: "active",
        workspaceId,
      });
      const [globalData, workspaceData] = await Promise.all([
        requestJson<MemoryListResponse>(`/api/memory?${globalParams.toString()}`),
        requestJson<MemoryListResponse>(
          `/api/memory?${workspaceParams.toString()}`,
        ),
      ]);

      setContextMemories({
        global: globalData.memories,
        workspace: workspaceData.memories,
      });
    } catch (requestError) {
      setContextMemories({ global: [], workspace: [] });
      setContextMemoryError(errorMessage(requestError));
    } finally {
      setIsLoadingContextMemories(false);
    }
  }, []);

  const forgetContextMemory = useCallback(
    async (memory: MemoryFactRecord) => {
      if (!activeWorkspace?.id) {
        return;
      }
      if (!window.confirm(t("Delete memory confirmation"))) {
        return;
      }

      setDeletingContextMemoryId(memory.id);
      setContextMemoryError(null);

      try {
        await requestJson<MemoryMutationResponse>("/api/memory/forget", {
          body: JSON.stringify({
            memoryId: memory.id,
            scope: memory.scope,
            workspaceId:
              memory.scope === "global" ? null : activeWorkspace.id,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        });
        await loadContextMemories(activeWorkspace.id);
      } catch (requestError) {
        setContextMemoryError(errorMessage(requestError));
      } finally {
        setDeletingContextMemoryId((current) =>
          current === memory.id ? null : current,
        );
      }
    },
    [activeWorkspace?.id, loadContextMemories, t],
  );

  const loadTodoGraph = useCallback(async (workspaceId: string, chatId: string) => {
    const requestedChatKey = chatRunKey(workspaceId, chatId);
    setIsLoadingTodoGraph(true);
    setTodoGraphError(null);

    try {
      const data = await requestJson<TodoGraphResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/todo-graph`,
      );
      if (activeChatKeyRef.current === requestedChatKey) {
        setTodoGraph(data);
      }
    } catch (requestError) {
      if (activeChatKeyRef.current === requestedChatKey) {
        setTodoGraph(null);
        setTodoGraphError(errorMessage(requestError));
      }
    } finally {
      if (activeChatKeyRef.current === requestedChatKey) {
        setIsLoadingTodoGraph(false);
      }
    }
  }, []);

  const loadGitBranches = useCallback(async (workspaceId: string) => {
    setIsLoadingBranches(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/branches`,
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");
    } catch (requestError) {
      setGitBranches(null);
      setSelectedGitBranch("");
      setBranchError(errorMessage(requestError));
    } finally {
      setIsLoadingBranches(false);
    }
  }, []);

  useEffect(() => {
    void loadAuthStatus();
  }, [loadAuthStatus]);

  useEffect(() => {
    if (!canUseApp) {
      return;
    }

    void refreshWorkspaces();
    void loadSettings();
  }, [canUseApp, loadSettings, refreshWorkspaces]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setGitDiff(null);
      setSelectedDiffPath(null);
      setDiffError(null);
      return;
    }

    if (!isContextPanelOpen || contextPanelTab !== "git") {
      return;
    }

    void loadGitDiff(activeWorkspace.id, selectedDiffPath);
  }, [
    activeWorkspace?.id,
    activeChatId,
    contextPanelTab,
    isContextPanelOpen,
    loadGitDiff,
    selectedDiffPath,
  ]);

  useEffect(() => {
    if (!activeWorkspace?.id || !activeChatId) {
      setTodoGraph(null);
      setTodoGraphError(null);
      setIsLoadingTodoGraph(false);
      return;
    }

    setTodoGraph(null);
    setTodoGraphError(null);
    void loadTodoGraph(activeWorkspace.id, activeChatId);
  }, [activeChatId, activeWorkspace?.id, loadTodoGraph]);

  useEffect(() => {
    if (contextPanelTab !== "memory" || !activeWorkspace?.id) {
      return;
    }

    void loadContextMemories(activeWorkspace.id);
  }, [activeWorkspace?.id, contextPanelTab, loadContextMemories]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setGitBranches(null);
      setSelectedGitBranch("");
      setBranchError(null);
      return;
    }

    void loadGitBranches(activeWorkspace.id);
  }, [activeWorkspace?.id, loadGitBranches]);

  useEffect(() => {
    setOpenChatTabs((current) => {
      const next = current.filter((tab) => workspaceHasChat(workspaces, tab));
      return next.length === current.length ? current : next;
    });

    setPendingDeleteChat((current) =>
      current && workspaceHasChat(workspaces, current) ? current : null,
    );

    setFailedChatKeySet((current) => {
      const next = new Set(
        [...current].filter((chatKey) => {
          const parsed = parseChatRunKey(chatKey);
          return parsed ? workspaceHasChat(workspaces, parsed) : false;
        }),
      );
      return next.size === current.size ? current : next;
    });

    setRunningChatKeys((current) => {
      const next = new Set(
        [...current].filter((chatKey) => {
          if (chatKey.includes(":pending:")) {
            return true;
          }

          const parsed = parseChatRunKey(chatKey);
          return parsed ? workspaceHasChat(workspaces, parsed) : false;
        }),
      );
      return next.size === current.size ? current : next;
    });

    setActiveRunInfoByChatKey((current) => {
      const next = Object.fromEntries(
        Object.entries(current).filter(([chatKey]) => {
          if (chatKey.includes(":pending:")) {
            return true;
          }

          const parsed = parseChatRunKey(chatKey);
          return parsed ? workspaceHasChat(workspaces, parsed) : false;
        }),
      );

      return Object.keys(next).length === Object.keys(current).length
        ? current
        : next;
    });

    updateQueuedRunRequestsByWorkspaceList(workspaces);
  }, [workspaces]);

  useEffect(() => {
    if (!isResizingDiffPanel) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const nextWidth = window.innerWidth - event.clientX;
      setDiffPanelWidth(
        Math.min(
          Math.max(nextWidth, CONTEXT_PANEL_MIN_WIDTH),
          CONTEXT_PANEL_MAX_WIDTH,
        ),
      );
    }

    function handlePointerUp() {
      setIsResizingDiffPanel(false);
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizingDiffPanel]);

  useEffect(() => {
    if (!isResizingSidebar) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      updateSidebarWidthFromClientX(event.clientX);
    }

    function handlePointerUp() {
      setIsResizingSidebar(false);
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizingSidebar, updateSidebarWidthFromClientX]);

  useEffect(() => {
    if (!workspaces.length) {
      setExpandedWorkspaceId(null);
      return;
    }

    setExpandedWorkspaceId((current) => {
      if (
        current === null ||
        workspaces.some((workspace) => workspace.id === current)
      ) {
        return current;
      }

      return activeWorkspace?.id ?? workspaces[0]?.id ?? null;
    });
  }, [activeChatId, activeWorkspace?.id, activeWorkspaceId, workspaces]);

  useEffect(() => {
    setSelectedModelId((current) => {
      const highestPriorityModelId = availableModels[0]?.id ?? "";

      if (!highestPriorityModelId) {
        hasManuallySelectedModelRef.current = false;
        return "";
      }

      if (!hasManuallySelectedModelRef.current) {
        return highestPriorityModelId;
      }

      if (availableModels.some((model) => model.id === current)) {
        return current;
      }

      hasManuallySelectedModelRef.current = false;
      return highestPriorityModelId;
    });
  }, [availableModels]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );
    setSelectedThinkingLevel(selectedModel?.thinkingLevel ?? "");
  }, [availableModels, selectedModelId]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );

    setSelectedProviderId((current) => {
      if (!selectedModel?.providerIds.length) {
        return "";
      }

      if (current && selectedModel.providerIds.includes(current)) {
        return current;
      }

      if (
        selectedModel.activeProviderId &&
        selectedModel.providerIds.includes(selectedModel.activeProviderId)
      ) {
        return selectedModel.activeProviderId;
      }

      return selectedModel.providerIds[0] ?? "";
    });
  }, [availableModels, selectedModelId]);

  useEffect(() => {
    const enabledSkillIds = new Set(
      detectedSkills.filter((skill) => skill.enabled).map((skill) => skill.key),
    );

    setSelectedSkillIds((current) => {
      const next = current.filter((skillId) => enabledSkillIds.has(skillId));
      return next.length === current.length ? current : next;
    });
  }, [detectedSkills]);

  async function handleWorkspaceSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>("/api/workspaces/add", {
        body: JSON.stringify({
          name: workspaceName,
          path: workspacePath,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const createdWorkspace = data.workspaces[data.workspaces.length - 1];

      setWorkspaces(data.workspaces);
      setActiveWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      setExpandedWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      updateBrowserRoute({
        chatId: null,
        viewMode: "chat",
        workspaceId: createdWorkspace?.id ?? data.activeWorkspaceId,
      });
      setWorkspaceName("");
      setWorkspacePath("");
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  async function handleSelectWorkspacePath() {
    setIsSelectingWorkspacePath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        { method: "POST" },
      );

      if (data.path) {
        setWorkspacePath(data.path);
        setWorkspaceName((current) =>
          current.trim() ? current : workspaceNameFromPath(data.path ?? ""),
        );
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingWorkspacePath(false);
    }
  }

  function setMessagesForChatKey(
    chatKey: string | null,
    updater: ShellMessage[] | ((current: ShellMessage[]) => ShellMessage[]),
  ) {
    const resolveNext = (current: ShellMessage[]) =>
      typeof updater === "function" ? updater(current) : updater;

    if (!chatKey) {
      setMessages((current) => resolveNext(current));
      return;
    }

    setChatMessagesByKey((current) => {
      const next = resolveNext(current[chatKey] ?? []);
      return { ...current, [chatKey]: next };
    });

    if (activeChatKeyRef.current === chatKey) {
      setMessages((current) => resolveNext(current));
    }
  }

  function moveMessagesForChatKey(
    fromChatKey: string,
    toChatKey: string,
    updater: (current: ShellMessage[]) => ShellMessage[],
  ) {
    setChatMessagesByKey((current) => {
      const nextMessages = updater(current[fromChatKey] ?? []);
      const { [fromChatKey]: _removed, ...next } = current;
      return { ...next, [toChatKey]: nextMessages };
    });

    if (activeChatKeyRef.current === fromChatKey) {
      activeChatKeyRef.current = toChatKey;
      setMessages((current) => updater(current));
    }
  }

  function removeMessagesForChatKey(chatKey: string) {
    setChatMessagesByKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }

      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
  }

  function appendPendingUserMessage(
    chatKey: string,
    messageId: string,
    content: string,
    attachments: ChatAttachmentPayload[],
    pendingMode: "guidance" | "queued",
  ) {
    const createdAt = new Date().toISOString();
    setMessagesForChatKey(chatKey, (current) => [
      ...current,
      {
        id: messageId,
        role: "user",
        content,
        createdAt,
        reasoning: null,
        pendingMode,
        toolCalls: [],
        parts: userMessageParts(content, attachments),
        metrics: null,
        memoriesUsed: [],
        extractedMemories: [],
      },
    ]);
  }

  function removeMessageForChatKey(chatKey: string, messageId: string) {
    setMessagesForChatKey(chatKey, (current) =>
      current.filter((message) => message.id !== messageId),
    );
  }

  function setChatRunFailed(chatKey: string | null, failed: boolean) {
    if (!chatKey || chatKey.includes(":pending:")) {
      return;
    }

    setFailedChatKeySet((current) => {
      if (current.has(chatKey) === failed) {
        return current;
      }

      const next = new Set(current);
      if (failed) {
        next.add(chatKey);
      } else {
        next.delete(chatKey);
      }
      return next;
    });
  }

  function setChatRunning(chatKey: string, running: boolean) {
    setRunningChatKeys((current) => {
      if (current.has(chatKey) === running) {
        return current;
      }

      const next = new Set(current);
      if (running) {
        next.add(chatKey);
      } else {
        next.delete(chatKey);
      }
      return next;
    });
  }

  function setActiveRunInfoForChatKey(
    chatKey: string,
    runInfo: ActiveRunInfo | null,
  ) {
    setActiveRunInfoByChatKey((current) => {
      if (!runInfo) {
        if (!(chatKey in current)) {
          return current;
        }

        const { [chatKey]: _removed, ...next } = current;
        return next;
      }

      return { ...current, [chatKey]: runInfo };
    });
  }

  function updateQueuedRunRequestsForChatKey(
    chatKey: string,
    updater: (current: RetryRunRequest[]) => RetryRunRequest[],
  ) {
    const nextRequests = updater(
      queuedRunRequestsByChatKeyRef.current[chatKey] ?? [],
    );
    const next = { ...queuedRunRequestsByChatKeyRef.current };

    if (nextRequests.length) {
      next[chatKey] = nextRequests;
    } else {
      delete next[chatKey];
    }

    queuedRunRequestsByChatKeyRef.current = next;
    setQueuedRunRequestsByChatKey(next);
  }

  function updateQueuedRunRequestsByWorkspaceList(
    nextWorkspaces: WorkspaceSummary[],
  ) {
    const next: Record<string, RetryRunRequest[]> = {};

    for (const [chatKey, requests] of Object.entries(
      queuedRunRequestsByChatKeyRef.current,
    )) {
      if (chatKey.includes(":pending:")) {
        next[chatKey] = requests;
        continue;
      }

      const parsed = parseChatRunKey(chatKey);
      if (parsed && workspaceHasChat(nextWorkspaces, parsed)) {
        next[chatKey] = requests;
      }
    }

    if (
      Object.keys(next).length ===
      Object.keys(queuedRunRequestsByChatKeyRef.current).length
    ) {
      return;
    }

    queuedRunRequestsByChatKeyRef.current = next;
    setQueuedRunRequestsByChatKey(next);
  }

  async function loadChatMessages(
    workspaceId: string,
    chatId: string,
    options: { updateUrl?: boolean } = {},
  ) {
    setError(null);

    try {
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages`,
      );
      const chatKey = chatRunKey(workspaceId, chatId);
      const nextMessages = data.messages.map(normalizeChatMessageSummary);
      const activeRun = normalizeActiveChatRunSummary(data.activeRun);
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setExpandedWorkspaceId(workspaceId);
      openChatTab(workspaceId, chatId);
      activeChatKeyRef.current = chatKey;
      setMessages(nextMessages);
      setChatMessagesByKey((current) => ({ ...current, [chatKey]: nextMessages }));
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      if (activeRun) {
        void subscribeActiveChatRun(activeRun);
      } else {
        setChatRunning(chatKey, false);
        setActiveRunInfoForChatKey(chatKey, null);
      }
      if (options.updateUrl !== false) {
        updateBrowserRoute({ chatId, viewMode: "chat", workspaceId });
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function selectWorkspaceChat(
    workspaceId: string,
    chatId: string,
    options: { updateUrl?: boolean } = {},
  ) {
    const chatKey = chatRunKey(workspaceId, chatId);
    const cachedMessages = chatMessagesByKey[chatKey];

    if (!cachedMessages) {
      void loadChatMessages(workspaceId, chatId, options);
      return;
    }

    setActiveWorkspaceId(workspaceId);
    setActiveChatId(chatId);
    setExpandedWorkspaceId(workspaceId);
    openChatTab(workspaceId, chatId);
    activeChatKeyRef.current = chatKey;
    setMessages(cachedMessages);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      updateBrowserRoute({ chatId, viewMode: "chat", workspaceId });
    }
  }

  function startNewWorkspaceChat(
    workspaceId: string,
    options: { updateUrl?: boolean } = {},
  ) {
    setExpandedWorkspaceId(workspaceId);
    setActiveWorkspaceId(workspaceId);
    setActiveChatId(null);
    activeChatKeyRef.current = null;
    setMessages([]);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      updateBrowserRoute({ chatId: null, viewMode: "chat", workspaceId });
    }
  }

  function openChatTab(workspaceId: string, chatId: string) {
    const workspace = workspaces.find((workspace) => workspace.id === workspaceId);
    const chat = workspace?.chats.find((chat) => chat.id === chatId);

    setOpenChatTabs((current) =>
      upsertOpenChatTab(current, {
        workspaceId,
        chatId,
        fallbackTitle: chat?.title ?? t("Chat"),
        fallbackWorkspaceName: workspace?.name ?? t("Workspace"),
      }),
    );
  }

  function closeChatTab(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);
    if (runningChatKeys.has(chatKey)) {
      return;
    }

    const tabIndex = openChatTabs.findIndex(
      (tab) => tab.workspaceId === workspaceId && tab.chatId === chatId,
    );
    setOpenChatTabs((current) =>
      current.filter(
        (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
      ),
    );
    setChatRunFailed(chatKey, false);
    removeMessagesForChatKey(chatKey);

    if (activeWorkspaceId !== workspaceId || activeChatId !== chatId) {
      return;
    }

    const nextTabs = openChatTabs.filter(
      (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
    );
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);

    if (nextTab) {
      selectWorkspaceChat(nextTab.workspaceId, nextTab.chatId);
      return;
    }

    setActiveChatId(null);
    activeChatKeyRef.current = null;
    setMessages([]);
    updateBrowserRoute({
      chatId: null,
      viewMode: "chat",
      workspaceId: activeWorkspaceId || workspaceId,
    });
  }

  function requestDeleteWorkspaceChat(workspace: WorkspaceSummary, chat: ChatSummary) {
    if (runningChatKeys.has(chatRunKey(workspace.id, chat.id))) {
      setError(t("Cancel the current run before deleting this chat."));
      return;
    }

    setError(null);
    setPendingDeleteChat({
      workspaceId: workspace.id,
      chatId: chat.id,
      title: chat.title,
      workspaceName: workspace.name,
    });
  }

  async function confirmDeleteWorkspaceChat() {
    const target = pendingDeleteChat;
    if (!target) {
      return;
    }

    await deleteWorkspaceChat(target.workspaceId, target.chatId);
  }

  async function deleteWorkspaceChat(workspaceId: string, chatId: string) {
    if (runningChatKeys.has(chatRunKey(workspaceId, chatId))) {
      setError(t("Cancel the current run before deleting this chat."));
      return;
    }

    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/delete`,
        { method: "POST" },
      );
      setWorkspaces(data.workspaces);
      setActiveWorkspaceId((current) =>
        data.workspaces.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );

      if (activeChatId === chatId) {
        setActiveWorkspaceId(workspaceId);
        setActiveChatId(null);
        setMessages([]);
        updateBrowserRoute({
          chatId: null,
          viewMode: "chat",
          workspaceId,
        });
      }

      setRetryRunRequest((current) =>
        current?.chatId === chatId ? null : current,
      );
      setPendingDeleteChat(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function toggleWorkspaceTerminal() {
    if (!activeWorkspace) {
      return;
    }

    setTerminalOpenWorkspaceIds((current) => {
      const next = new Set(current);

      if (next.has(activeWorkspace.id)) {
        next.delete(activeWorkspace.id);
      } else {
        next.add(activeWorkspace.id);
      }

      return next;
    });
  }

  function toggleSelectedSkill(skillId: string) {
    setSelectedSkillIds((current) =>
      current.includes(skillId)
        ? current.filter((id) => id !== skillId)
        : [...current, skillId],
    );
  }

  function removeSelectedSkill(skillId: string) {
    setSelectedSkillIds((current) => current.filter((id) => id !== skillId));
  }

  async function handleGitBranchChange(branch: string) {
    if (branch === CREATE_BRANCH_OPTION_VALUE) {
      setNewBranchName("");
      setBranchError(null);
      setIsBranchDialogOpen(true);
      return;
    }

    if (!activeWorkspace || !gitBranches?.isGitRepository || !branch) {
      return;
    }

    if (branch === selectedGitBranch) {
      return;
    }

    setIsLoadingBranches(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/branches/switch`,
        {
          body: JSON.stringify({ name: branch }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");

      if (isContextPanelOpen && contextPanelTab === "git") {
        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
      }
    } catch (requestError) {
      setBranchError(errorMessage(requestError));
    } finally {
      setIsLoadingBranches(false);
    }
  }

  async function handleCreateGitBranch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!activeWorkspace) {
      setBranchError(t("Select a workspace before creating a branch."));
      return;
    }

    const branch = newBranchName.trim();
    if (!branch) {
      setBranchError(t("Git branch name must not be empty."));
      return;
    }

    setIsSavingBranch(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/branches/create`,
        {
          body: JSON.stringify({ name: branch }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");
      setNewBranchName("");
      setIsBranchDialogOpen(false);

      if (isContextPanelOpen && contextPanelTab === "git") {
        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
      }
    } catch (requestError) {
      setBranchError(errorMessage(requestError));
    } finally {
      setIsSavingBranch(false);
    }
  }

  async function handleQuestionSubmit(answer: QuestionAnswerSubmission) {
    if (!pendingQuestion || isAnsweringQuestion) {
      return;
    }

    setIsAnsweringQuestion(true);
    setQuestionError(null);

    try {
      await requestJson<{ ok: boolean; questionId: string }>(
        `/api/chat/questions/${encodeURIComponent(pendingQuestion.id)}/answer`,
        {
          body: JSON.stringify(answer),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setPendingQuestion((current) =>
        current?.id === pendingQuestion.id ? null : current,
      );
    } catch (requestError) {
      setQuestionError(errorMessage(requestError));
    } finally {
      setIsAnsweringQuestion(false);
    }
  }

  async function handleAddDraftAttachments(attachments: ComposerAttachment[]) {
    if (!attachments.length) {
      return;
    }

    const totalCount = draftAttachments.length + attachments.length;
    if (totalCount > MAX_CHAT_ATTACHMENTS) {
      setError(
        t("At most {count} attachments are allowed.", {
          count: MAX_CHAT_ATTACHMENTS,
        }),
      );
      return;
    }

    for (const attachment of attachments) {
      if (attachment.sizeBytes > MAX_CHAT_ATTACHMENT_BYTES) {
        setError(
          t("Attachment {name} exceeds the {size} limit.", {
            name: attachment.name,
            size: formatFileSize(MAX_CHAT_ATTACHMENT_BYTES),
          }),
        );
        return;
      }
    }

    const totalSize =
      draftAttachments.reduce((sum, attachment) => sum + attachment.sizeBytes, 0) +
      attachments.reduce((sum, attachment) => sum + attachment.sizeBytes, 0);
    if (totalSize > MAX_CHAT_ATTACHMENT_TOTAL_BYTES) {
      setError(
        t("Attachments exceed the {size} total limit.", {
          size: formatFileSize(MAX_CHAT_ATTACHMENT_TOTAL_BYTES),
        }),
      );
      return;
    }

    setDraftAttachments((current) => [...current, ...attachments]);
    setError(null);
  }

  async function handleAddPastedImageAttachments(files: File[]) {
    if (!files.length) {
      return;
    }

    try {
      const nextAttachments = await Promise.all(
        files.map(fileToComposerAttachment),
      );
      await handleAddDraftAttachments(nextAttachments);
    } catch (readError) {
      setError(errorMessage(readError));
    }
  }

  async function handleSelectDraftAttachments() {
    setIsSelectingAttachments(true);
    setError(null);

    try {
      const data = await requestJson<{ files: NativeSelectedFile[] }>(
        "/api/native/select-files",
        { method: "POST" },
      );
      const attachments = data.files.map(nativeSelectedFileToComposerAttachment);
      await handleAddDraftAttachments(attachments);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingAttachments(false);
    }
  }

  function handleRemoveDraftAttachment(attachmentId: string) {
    setDraftAttachments((current) =>
      current.filter((attachment) => attachment.id !== attachmentId),
    );
  }

  function currentDraftRunRequest(): RetryRunRequest | null {
    const content = draftMessage.trim();
    const attachments = draftAttachments.map(chatAttachmentPayload);
    if (!content && !attachments.length) {
      return null;
    }

    if (!activeWorkspace) {
      setError(t("Select a workspace before sending."));
      return null;
    }

    if (!selectedModelId) {
      setError(t("Select an enabled model before sending."));
      return null;
    }

    if (!selectedProviderId) {
      setError(t("Select a provider before sending."));
      return null;
    }

    const skillIds = [...selectedSkillIds];
    return {
      attachments,
      chatId: activeChatId,
      content,
      modelId: selectedModelId,
      providerId: selectedProviderId,
      skillIds,
      thinkingLevel: selectedThinkingLevel,
      workspaceId: activeWorkspace.id,
    };
  }

  async function handleSendMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }

    if (isSendingMessage) {
      await guideActiveRun(request);
      return;
    }

    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");

    await runChatMessage(request);
  }

  async function handleGuideActiveRun() {
    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }

    await guideActiveRun(request);
  }

  function handleQueueActiveRun() {
    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }
    const currentChatKey = activeChatKeyRef.current;
    const runInfo = activeRunInfo;
    if (
      !currentChatKey ||
      !runInfo?.chatKey ||
      runInfo.chatKey !== currentChatKey
    ) {
      setError(t("No active run is available for guidance."));
      return;
    }

    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");
    setError(null);

    const pendingUserMessageId = localUiId("pending-queued-user");
    appendPendingUserMessage(
      runInfo.chatKey,
      pendingUserMessageId,
      messageWithSelectedSkills(detectedSkills, request.skillIds, request.content),
      request.attachments,
      "queued",
    );

    const queuedRequest = {
      ...request,
      chatId: runInfo.chatId ?? request.chatId,
      pendingUserMessageId,
      workspaceId: runInfo.workspaceId ?? request.workspaceId,
    };
    updateQueuedRunRequestsForChatKey(runInfo.chatKey, (current) => [
      ...current,
      queuedRequest,
    ]);
  }

  async function handleRetryRun() {
    if (!retryRunRequest || isSendingMessage) {
      return;
    }

    const retryRequest = retryRunRequest;
    setActiveWorkspaceId(retryRequest.workspaceId);
    setActiveChatId(retryRequest.chatId);
    updateBrowserRoute({
      chatId: retryRequest.chatId,
      viewMode: "chat",
      workspaceId: retryRequest.workspaceId,
    });
    hasManuallySelectedModelRef.current = true;
    setSelectedModelId(retryRequest.modelId);
    setSelectedProviderId(retryRequest.providerId);
    setSelectedSkillIds(retryRequest.skillIds);
    setSelectedThinkingLevel(retryRequest.thinkingLevel);
    await runChatMessage(retryRequest);
  }

  function handleChatModelChange(modelId: string) {
    hasManuallySelectedModelRef.current = true;
    setSelectedModelId(modelId);
  }

  function currentChatBrowserRoute(): BrowserRoute {
    return {
      chatId: activeChatId,
      viewMode: "chat",
      workspaceId: activeWorkspace?.id ?? (activeWorkspaceId || null),
    };
  }

  function openSettingsSection(section: SettingsSection) {
    setSettingsSection(section);
    setViewMode("settings");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({ section, viewMode: "settings" });
  }

  function openStatsView() {
    setViewMode("stats");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({ viewMode: "stats" });
  }

  function openCurrentChatView() {
    setViewMode("chat");
    updateBrowserRoute(currentChatBrowserRoute());
  }

  function applyBrowserRoute(route: BrowserRoute) {
    if (route.viewMode === "settings") {
      setSettingsSection(route.section);
      setViewMode("settings");
      setIsMobileWorkspaceOpen(false);
      return;
    }

    if (route.viewMode === "stats") {
      setViewMode("stats");
      setIsMobileWorkspaceOpen(false);
      return;
    }

    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (!route.workspaceId) {
      setActiveChatId(null);
      activeChatKeyRef.current = null;
      setMessages([]);
      return;
    }

    if (!workspaces.some((workspace) => workspace.id === route.workspaceId)) {
      setError(`Workspace not found: ${route.workspaceId}`);
      return;
    }

    if (route.chatId) {
      selectWorkspaceChat(route.workspaceId, route.chatId, {
        updateUrl: false,
      });
      return;
    }

    startNewWorkspaceChat(route.workspaceId, { updateUrl: false });
  }

  applyBrowserRouteRef.current = applyBrowserRoute;

  useEffect(() => {
    if (
      !canUseApp ||
      isLoading ||
      hasAppliedInitialBrowserRouteRef.current
    ) {
      return;
    }

    hasAppliedInitialBrowserRouteRef.current = true;
    applyBrowserRoute(initialBrowserRoute);
    updateBrowserRoute(initialBrowserRoute, "replace");
  }, [canUseApp, initialBrowserRoute, isLoading, updateBrowserRoute]);

  useEffect(() => {
    function handlePopState() {
      applyBrowserRouteRef.current(currentBrowserRoute());
    }

    window.addEventListener("popstate", handlePopState);
    return () => {
      window.removeEventListener("popstate", handlePopState);
    };
  }, []);

  async function handleCancelRun() {
    const currentChatKey = activeChatKeyRef.current;
    if (!currentChatKey) {
      return;
    }

    const runInfo = activeRunInfoByChatKey[currentChatKey] ?? null;
    if (runInfo?.runId) {
      try {
        await requestJson<{ ok: boolean; runId: string }>(
          `/api/workspaces/${encodeURIComponent(runInfo.workspaceId)}/chat/runs/${encodeURIComponent(runInfo.runId)}/cancel`,
          { method: "POST" },
        );
      } catch (requestError) {
        setError(errorMessage(requestError));
        return;
      }
    }

    activeRunAbortByChatKeyRef.current.get(currentChatKey)?.abort();
    setPendingQuestion(null);
    setQuestionError(null);
    setIsAnsweringQuestion(false);
  }

  async function refreshContextUsage(request: ContextUsageRefreshRequest) {
    if (!request.chatId) {
      return;
    }

    const requestId = contextUsageRequestIdRef.current + 1;
    contextUsageRequestIdRef.current = requestId;
    contextUsageAbortRef.current?.abort();
    const abortController = new AbortController();
    contextUsageAbortRef.current = abortController;
    setIsLoadingContextUsage(true);

    try {
      const data = await requestJson<ContextUsageResponse>(
        `/api/workspaces/${encodeURIComponent(request.workspaceId)}/context-usage`,
        {
          body: JSON.stringify({
            chatId: request.chatId,
            modelId: request.modelId,
            providerId: request.providerId,
            thinkingLevel: request.thinkingLevel || null,
            skillIds: request.skillIds.length ? request.skillIds : null,
            draftMessage: null,
            assistantDraft: request.assistantDraft || null,
            assistantDraftReasoning: request.assistantDraftReasoning || null,
            latestResponseUsage: request.latestResponseUsage,
            attachments: [],
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (contextUsageRequestIdRef.current === requestId) {
        setContextUsage(data);
      }
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (!wasCancelled && contextUsageRequestIdRef.current === requestId) {
        setError(errorMessage(requestError));
      }
    } finally {
      if (contextUsageAbortRef.current === abortController) {
        contextUsageAbortRef.current = null;
      }
      if (contextUsageRequestIdRef.current === requestId) {
        setIsLoadingContextUsage(false);
      }
    }
  }

  function appendGuidanceMessage(
    chatKey: string,
    guidance: {
      id: string;
      content: string;
      parts: ChatMessagePart[];
      interruptedAssistantMetrics: ChatReplyMetrics | null;
    },
    assistantId: string,
    previousAssistantId: string,
  ) {
    const pendingGuidanceMessageId =
      pendingGuidanceMessageIdsRef.current.get(guidance.id) ?? null;
    pendingGuidanceMessageIdsRef.current.delete(guidance.id);
    setMessagesForChatKey(chatKey, (current) => {
      if (current.some((message) => message.id === assistantId)) {
        return current;
      }

      const matchingPendingGuidanceMessageId =
        pendingGuidanceMessageId ??
        current.find(
          (message) =>
            message.pendingMode === "guidance" &&
            message.content === guidance.content,
        )?.id ??
        null;
      let reusedGuidanceMessage = false;
      const nextMessages = current
        .filter(
          (message) =>
            message.id !== previousAssistantId ||
            !isEmptyStreamingAssistantMessage(message),
        )
        .map((message) => {
          if (
            message.id === matchingPendingGuidanceMessageId ||
            message.id === guidance.id
          ) {
            reusedGuidanceMessage = true;
            return {
              ...message,
              id: guidance.id,
              content: guidance.content,
              pendingMode: undefined,
              parts: guidance.parts.length
                ? [{ type: "text" as const, text: guidance.content }, ...guidance.parts]
                : [{ type: "text" as const, text: guidance.content }],
            };
          }

          if (message.id === previousAssistantId) {
            return {
              ...message,
              status: undefined,
              metrics: guidance.interruptedAssistantMetrics ?? message.metrics,
            };
          }

          return message;
        });
      const createdAt = new Date().toISOString();

      return [
        ...nextMessages,
        ...(reusedGuidanceMessage
          ? []
          : [
              {
                id: guidance.id,
                role: "user" as const,
                content: guidance.content,
                createdAt,
                reasoning: null,
                status: undefined,
                toolCalls: [],
                parts: guidance.parts.length
                  ? [{ type: "text" as const, text: guidance.content }, ...guidance.parts]
                  : [{ type: "text" as const, text: guidance.content }],
                metrics: null,
                memoriesUsed: [],
                extractedMemories: [],
              },
            ]),
        {
          id: assistantId,
          role: "assistant",
          content: "",
          createdAt,
          reasoning: null,
          status: "streaming",
          toolCalls: [],
          parts: [],
          metrics: null,
          memoriesUsed: [],
          extractedMemories: [],
        },
      ];
    });
  }

  async function guideActiveRun(request: RetryRunRequest) {
    const runInfo = activeRunInfo;
    if (
      !isSendingMessage ||
      !runInfo ||
      !runInfo.chatId ||
      !runInfo.runId ||
      runInfo.chatKey !== activeChatKeyRef.current
    ) {
      setError(t("No active run is available for guidance."));
      return;
    }

    const pendingUserMessageId = localUiId("pending-guidance-user");
    const visibleUserContent = messageWithSelectedSkills(
      detectedSkills,
      request.skillIds,
      request.content,
    );
    appendPendingUserMessage(
      runInfo.chatKey,
      pendingUserMessageId,
      visibleUserContent,
      request.attachments,
      "guidance",
    );
    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");
    setError(null);

    try {
      const guidance = await requestJson<{
        id: string;
        content: string;
        parts: ChatMessagePart[];
      }>(
        `/api/workspaces/${encodeURIComponent(runInfo.workspaceId)}/chat/guidance`,
        {
          body: JSON.stringify({
            attachments: request.attachments,
            chatId: runInfo.chatId,
            message: visibleUserContent,
            runId: runInfo.runId,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      pendingGuidanceMessageIdsRef.current.set(
        guidance.id,
        pendingUserMessageId,
      );
    } catch (requestError) {
      removeMessageForChatKey(runInfo.chatKey, pendingUserMessageId);
      setError(errorMessage(requestError));
    }
  }

  async function subscribeActiveChatRun(activeRun: ActiveChatRunSummary) {
    const chatKey = chatRunKey(activeRun.workspaceId, activeRun.chatId);
    if (activeRunAbortByChatKeyRef.current.has(chatKey)) {
      return;
    }

    const abortController = new AbortController();
    let assistantMessageId = `active-assistant-${activeRun.runId}`;
    let currentAssistantMessageId = assistantMessageId;
    let hasGuidanceTurns = false;
    let streamHadError = false;

    const ensureStreamingAssistantMessage = (
      nextAssistantMessageId: string,
      memoriesUsed: ChatMemoryUsedSummary[] = [],
    ) => {
      setMessagesForChatKey(chatKey, (current) => {
        if (current.some((message) => message.id === nextAssistantMessageId)) {
          return current.map((message) =>
            message.id === nextAssistantMessageId && message.role === "assistant"
              ? {
                  ...message,
                  memoriesUsed: message.memoriesUsed.length
                    ? message.memoriesUsed
                    : memoriesUsed,
                  status: "streaming",
                }
              : message,
          );
        }

        return [
          ...current,
          streamingAssistantMessage(nextAssistantMessageId, memoriesUsed),
        ];
      });
    };

    const isCurrentAssistantMessage = (
      message: ShellMessage,
      eventAssistantMessageId?: string,
    ) =>
      message.role === "assistant" &&
      (message.id === currentAssistantMessageId ||
        (eventAssistantMessageId !== undefined &&
          message.id === eventAssistantMessageId) ||
        message.id === assistantMessageId);

    setChatRunning(chatKey, true);
    setChatRunFailed(chatKey, false);
    setActiveRunInfoForChatKey(chatKey, {
      chatId: activeRun.chatId,
      chatKey,
      lastSequence: activeRun.lastSequence,
      runId: activeRun.runId,
      workspaceId: activeRun.workspaceId,
    });
    activeRunAbortByChatKeyRef.current.set(chatKey, abortController);

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(activeRun.workspaceId)}/chat/runs/${encodeURIComponent(activeRun.runId)}/stream?afterSequence=-1`,
        {
          cache: "no-store",
          credentials: "same-origin",
          signal: abortController.signal,
        },
      );

      if (!response.ok) {
        throw new Error(await responseErrorMessage(response));
      }

      await readChatStream(response, (streamEvent) => {
        if (streamEvent.type === "start") {
          assistantMessageId = streamEvent.assistantMessageId;
          currentAssistantMessageId = streamEvent.assistantMessageId;
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId,
            streamEvent.memoriesUsed,
          );
          setChatRunFailed(chatKey, false);
          setChatRunning(chatKey, true);
          setActiveRunInfoForChatKey(chatKey, {
            chatId: streamEvent.chatId,
            chatKey,
            lastSequence: activeRun.lastSequence,
            runId: streamEvent.llmRequestId ?? activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          return;
        }

        if (streamEvent.type === "textDelta") {
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId ?? currentAssistantMessageId,
          );
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    content: message.content + streamEvent.delta,
                    parts: appendTextPart(message.parts, streamEvent.delta),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "reasoningDelta") {
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId ?? currentAssistantMessageId,
          );
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    reasoning: `${message.reasoning ?? ""}${streamEvent.delta}`,
                    parts: appendReasoningPart(message.parts, streamEvent.delta),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "usage") {
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          hasGuidanceTurns = true;
          appendGuidanceMessage(
            chatKey,
            streamEvent,
            guidanceAssistantId,
            previousAssistantId,
          );
          return;
        }

        if (streamEvent.type === "complete") {
          setChatRunFailed(chatKey, false);
          setRetryRunRequest(null);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? hasGuidanceTurns
                  ? completedGuidanceAssistantMessage(message, streamEvent)
                  : completedAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          ensureStreamingAssistantMessage(streamEvent.assistantMessageId);
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    parts: upsertToolCallPart(message.parts, streamEvent.toolCall),
                    toolCalls: upsertToolCall(
                      message.toolCalls,
                      streamEvent.toolCall,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolResult") {
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    parts: applyToolResultToParts(
                      message.parts,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                    toolCalls: applyToolResult(
                      message.toolCalls,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "questionRequest") {
          setQuestionError(null);
          setPendingQuestion(streamEvent.request);
          return;
        }

        if (streamEvent.type === "hookNotification") {
          if (streamEvent.notification.level === "error") {
            setError(streamEvent.notification.message);
          }
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    parts: appendTextPart(
                      message.parts,
                      `\n\n[${streamEvent.notification.event}] ${streamEvent.notification.message}`,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          if (isContextPanelOpen && contextPanelTab === "git") {
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          }
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          const activeKey = activeChatKeyRef.current;
          if (activeKey === chatRunKey(streamEvent.workspaceId, streamEvent.chatId)) {
            setContextPanelTab("todo");
            setIsContextPanelOpen(true);
            void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId);
          }
          return;
        }

        if (streamEvent.type === "error") {
          streamHadError = true;
          setChatRunFailed(chatKey, true);
          setChatRunning(chatKey, false);
          setError(streamEvent.message);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message)
                ? assistantMessageWithAppendedError(message, streamEvent.message)
                : message,
            ),
          );
        }
      });

      await refreshWorkspaces();
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (!wasCancelled) {
        setChatRunFailed(chatKey, true);
        setError(errorMessage(requestError));
      }
    } finally {
      if (activeRunAbortByChatKeyRef.current.get(chatKey) === abortController) {
        activeRunAbortByChatKeyRef.current.delete(chatKey);
      }
      setChatRunning(chatKey, false);
      setActiveRunInfoForChatKey(chatKey, null);
      if (!streamHadError) {
        setPendingQuestion(null);
        setQuestionError(null);
        setIsAnsweringQuestion(false);
      }
    }
  }

  async function runChatMessage(request: RetryRunRequest) {
    const runKey =
      globalThis.crypto?.randomUUID?.() ??
      `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const pendingUserMessageId = request.pendingUserMessageId ?? null;
    const localUserId = pendingUserMessageId ?? `local-user-${runKey}`;
    const localAssistantId = `local-assistant-${runKey}`;
    const localCreatedAt = new Date().toISOString();
    const visibleUserContent = messageWithSelectedSkills(
      detectedSkills,
      request.skillIds,
      request.content,
    );
    const localUserParts = userMessageParts(
      visibleUserContent,
      request.attachments,
    );
    let assistantMessageId = localAssistantId;
    let currentAssistantMessageId = localAssistantId;
    let requestChatId = request.chatId;
    let runMessagesKey = requestChatId
      ? chatRunKey(request.workspaceId, requestChatId)
      : pendingChatRunKey(request.workspaceId, runKey);
    let currentRunningChatKey = runMessagesKey;
    let assistantDraft = "";
    let assistantDraftReasoning = "";
    let latestResponseUsage: ChatUsage | null = null;
    let runSucceeded = false;
    let streamHadError = false;
    let hasGuidanceTurns = false;
    const abortController = new AbortController();
    const refreshRunContextUsage = () => {
      void refreshContextUsage({
        assistantDraft,
        assistantDraftReasoning,
        chatId: requestChatId,
        latestResponseUsage,
        modelId: request.modelId,
        providerId: request.providerId,
        skillIds: request.skillIds,
        thinkingLevel: request.thinkingLevel,
        workspaceId: request.workspaceId,
      });
    };

    activeChatKeyRef.current = runMessagesKey;
    setChatRunFailed(runMessagesKey, false);
    setMessagesForChatKey(runMessagesKey, (current) => {
      const assistantMessage: ShellMessage = {
        id: localAssistantId,
        role: "assistant",
        content: "",
        createdAt: localCreatedAt,
        reasoning: null,
        status: "streaming",
        toolCalls: [],
        parts: [],
        metrics: null,
        memoriesUsed: [],
        extractedMemories: [],
      };

      if (pendingUserMessageId) {
        const pendingIndex = current.findIndex(
          (message) => message.id === pendingUserMessageId,
        );

        if (pendingIndex >= 0) {
          const next = current.map((message) =>
            message.id === pendingUserMessageId
              ? {
                  ...message,
                  content: visibleUserContent,
                  pendingMode: undefined,
                  parts: localUserParts,
                }
              : message,
          );
          next.splice(pendingIndex + 1, 0, assistantMessage);
          return next;
        }
      }

      return [
        ...current,
        {
          id: localUserId,
          role: "user",
          content: visibleUserContent,
          createdAt: localCreatedAt,
          reasoning: null,
          toolCalls: [],
          parts: localUserParts,
          metrics: null,
          memoriesUsed: [],
          extractedMemories: [],
        },
        assistantMessage,
      ];
    });
    setDraftMessage("");
    setChatRunning(currentRunningChatKey, true);
    setActiveRunInfoForChatKey(currentRunningChatKey, {
      chatId: requestChatId,
      chatKey: currentRunningChatKey,
      runId: null,
      workspaceId: request.workspaceId,
    });
    setRetryRunRequest(null);
    setError(null);
    contextUsageAbortRef.current?.abort();
    contextUsageRequestIdRef.current += 1;
    if (!request.chatId) {
      setContextUsage(null);
    }
    setIsLoadingContextUsage(false);
    activeRunAbortByChatKeyRef.current.set(
      currentRunningChatKey,
      abortController,
    );

    const isCurrentAssistantMessage = (
      message: ShellMessage,
      eventAssistantMessageId?: string,
    ) =>
      message.role === "assistant" &&
      (message.id === currentAssistantMessageId ||
        (currentAssistantMessageId === assistantMessageId &&
          eventAssistantMessageId !== undefined &&
          message.id === eventAssistantMessageId) ||
        (currentAssistantMessageId === localAssistantId &&
          message.id === localAssistantId));

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/stream`,
        {
          body: JSON.stringify({
            chatId: request.chatId,
            message: request.content,
            attachments: request.attachments,
            modelId: request.modelId,
            providerId: request.providerId,
            skillIds: request.skillIds.length ? request.skillIds : null,
            thinkingLevel: request.thinkingLevel || null,
          }),
          cache: "no-store",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (!response.ok) {
        throw new Error(await responseErrorMessage(response));
      }

      await readChatStream(response, (streamEvent) => {
        if (streamEvent.type === "start") {
          assistantMessageId = streamEvent.assistantMessageId;
          currentAssistantMessageId = streamEvent.assistantMessageId;
          requestChatId = streamEvent.chatId;
          currentRunningChatKey = chatRunKey(
            request.workspaceId,
            streamEvent.chatId,
          );
          setChatRunFailed(currentRunningChatKey, false);
          openChatTab(request.workspaceId, streamEvent.chatId);
          if (runMessagesKey !== currentRunningChatKey) {
            setChatRunning(runMessagesKey, false);
            setActiveRunInfoForChatKey(runMessagesKey, null);
            if (
              activeRunAbortByChatKeyRef.current.get(runMessagesKey) ===
              abortController
            ) {
              activeRunAbortByChatKeyRef.current.delete(runMessagesKey);
              activeRunAbortByChatKeyRef.current.set(
                currentRunningChatKey,
                abortController,
              );
            }
            const pendingQueuedRequests =
              queuedRunRequestsByChatKeyRef.current[runMessagesKey] ?? [];
            if (pendingQueuedRequests.length) {
              updateQueuedRunRequestsForChatKey(
                currentRunningChatKey,
                (current) => [
                  ...current,
                  ...pendingQueuedRequests.map((queuedRequest) => ({
                    ...queuedRequest,
                    chatId: streamEvent.chatId,
                    workspaceId: request.workspaceId,
                  })),
                ],
              );
              updateQueuedRunRequestsForChatKey(runMessagesKey, () => []);
            }
            moveMessagesForChatKey(runMessagesKey, currentRunningChatKey, (current) =>
              current.map((message) => {
                if (message.id === localUserId) {
                  return { ...message, id: streamEvent.userMessageId };
                }

                if (
                  message.role === "assistant" &&
                  message.id === localAssistantId
                ) {
                  return {
                    ...message,
                    id: streamEvent.assistantMessageId,
                    memoriesUsed: streamEvent.memoriesUsed,
                  };
                }

                return message;
              }),
            );
            runMessagesKey = currentRunningChatKey;
          } else {
            setMessagesForChatKey(currentRunningChatKey, (current) =>
              current.map((message) => {
                if (message.id === localUserId) {
                  return { ...message, id: streamEvent.userMessageId };
                }

                if (
                  message.role === "assistant" &&
                  message.id === localAssistantId
                ) {
                  return {
                    ...message,
                    id: streamEvent.assistantMessageId,
                    memoriesUsed: streamEvent.memoriesUsed,
                  };
                }

                return message;
              }),
            );
          }
          setChatRunning(currentRunningChatKey, true);
          setActiveRunInfoForChatKey(currentRunningChatKey, {
            chatId: streamEvent.chatId,
            chatKey: currentRunningChatKey,
            runId: streamEvent.llmRequestId ?? null,
            workspaceId: request.workspaceId,
          });
          if (
            activeChatKeyRef.current === currentRunningChatKey ||
            activeChatKeyRef.current === null ||
            request.chatId
          ) {
            setActiveChatId(streamEvent.chatId);
            activeChatKeyRef.current = currentRunningChatKey;
            updateBrowserRoute({
              chatId: streamEvent.chatId,
              viewMode: "chat",
              workspaceId: request.workspaceId,
            });
          }
          void refreshWorkspaces();
          return;
        }

        if (streamEvent.type === "textDelta") {
          assistantDraft += streamEvent.delta;
          refreshRunContextUsage();
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    content: message.content + streamEvent.delta,
                    parts: appendTextPart(message.parts, streamEvent.delta),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "reasoningDelta") {
          assistantDraftReasoning += streamEvent.delta;
          refreshRunContextUsage();
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                  ? {
                      ...message,
                      reasoning: `${message.reasoning ?? ""}${streamEvent.delta}`,
                      parts: appendReasoningPart(
                        message.parts,
                        streamEvent.delta,
                      ),
                    }
                  : message,
              ),
          );
          return;
        }

        if (streamEvent.type === "usage") {
          latestResponseUsage =
            streamEvent.usage &&
            streamEvent.usage.inputTokens !== null &&
            streamEvent.usage.outputTokens !== null
              ? streamEvent.usage
              : null;
          refreshRunContextUsage();
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          hasGuidanceTurns = true;
          appendGuidanceMessage(
            runMessagesKey,
            streamEvent,
            guidanceAssistantId,
            previousAssistantId,
          );
          return;
        }

        if (streamEvent.type === "complete") {
          assistantDraft = "";
          assistantDraftReasoning = "";
          refreshRunContextUsage();
          setChatRunFailed(runMessagesKey, false);
          setRetryRunRequest(null);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? hasGuidanceTurns
                  ? completedGuidanceAssistantMessage(message, streamEvent)
                  : completedAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          refreshRunContextUsage();
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    toolCalls: upsertToolCall(
                      message.toolCalls,
                      streamEvent.toolCall,
                    ),
                    parts: upsertToolCallPart(message.parts, streamEvent.toolCall),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolResult") {
          refreshRunContextUsage();
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    toolCalls: applyToolResult(
                      message.toolCalls,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                    parts: applyToolResultToParts(
                      message.parts,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "questionRequest") {
          setQuestionError(null);
          setPendingQuestion(streamEvent.request);
          return;
        }

        if (streamEvent.type === "hookNotification") {
          if (streamEvent.notification.level === "error") {
            setError(streamEvent.notification.message);
          }
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    parts: appendTextPart(
                      message.parts,
                      `\n\n[${streamEvent.notification.event}] ${streamEvent.notification.message}`,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          if (isContextPanelOpen && contextPanelTab === "git") {
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          }
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          const activeKey = activeChatKeyRef.current;
          if (
            activeKey ===
            chatRunKey(streamEvent.workspaceId, streamEvent.chatId)
          ) {
            setContextPanelTab("todo");
            setIsContextPanelOpen(true);
            void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId);
          }
          return;
        }

        if (streamEvent.type === "error") {
          streamHadError = true;
          setChatRunFailed(runMessagesKey, true);
          setChatRunning(currentRunningChatKey, false);
          setError(streamEvent.message);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message)
                ? assistantMessageWithAppendedError(message, streamEvent.message)
                : message,
            ),
          );
        }
      });

      await refreshWorkspaces();
      runSucceeded = !streamHadError;
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      const message = wasCancelled ? t("Run cancelled.") : errorMessage(requestError);
      if (!wasCancelled) {
        setChatRunFailed(runMessagesKey, true);
      }
      setError(message);
      setPendingQuestion(null);
      setQuestionError(null);
      setIsAnsweringQuestion(false);
      setRetryRunRequest({
        ...request,
        chatId: requestChatId,
      });
      setMessagesForChatKey(runMessagesKey, (current) =>
        current.map((item) =>
          isCurrentAssistantMessage(item)
            ? assistantMessageWithAppendedError(item, message)
            : item,
        ),
      );
    } finally {
      if (
        activeRunAbortByChatKeyRef.current.get(currentRunningChatKey) ===
        abortController
      ) {
        activeRunAbortByChatKeyRef.current.delete(currentRunningChatKey);
      }
      setChatRunning(currentRunningChatKey, false);
      setActiveRunInfoForChatKey(currentRunningChatKey, null);
    }

    if (runSucceeded) {
      const [queuedRequest] =
        queuedRunRequestsByChatKeyRef.current[currentRunningChatKey] ?? [];
      if (queuedRequest) {
        updateQueuedRunRequestsForChatKey(currentRunningChatKey, (current) =>
          current.slice(1),
        );
        await runChatMessage({
          ...queuedRequest,
          chatId: requestChatId,
        });
      }
    }
  }

  function toggleWorkspace(workspaceId: string) {
    setExpandedWorkspaceId((current) =>
      current === workspaceId ? null : workspaceId,
    );
  }

  function showMoreWorkspaceChats(workspaceId: string) {
    setWorkspaceChatVisibleCounts((current) => ({
      ...current,
      [workspaceId]:
        (current[workspaceId] ?? WORKSPACE_CHAT_HISTORY_PAGE_SIZE) +
        WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
    }));
  }

  function openWorkspaceDialog() {
    setWorkspaceName("");
    setWorkspacePath("");
    setError(null);
    setWorkspaceDialogRevision((current) => current + 1);
    setIsWorkspaceDialogOpen(true);
  }

  async function saveAppTheme(nextTheme: AppThemeId) {
    if (!settings || settings.general.theme === nextTheme) {
      return;
    }

    const previousTheme = settings.general.theme;
    setSettings((current) =>
      current
        ? { ...current, general: { ...current.general, theme: nextTheme } }
        : current,
    );
    setIsSavingTheme(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          language: settings.general.language,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          password: null,
          theme: nextTheme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setSettings((current) =>
        current
          ? { ...current, general: { ...current.general, theme: previousTheme } }
          : current,
      );
    } finally {
      setIsSavingTheme(false);
    }
  }

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsLoggingIn(true);
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/login", {
        body: JSON.stringify({ password: authPassword }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setAuthStatus(data);
      setAuthPassword("");
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoggingIn(false);
    }
  }

  async function handleLogout() {
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/logout", {
        method: "POST",
      });
      setAuthStatus(data);
      setWorkspaces([]);
      setSettings(null);
      setOpenChatTabs([]);
      setActiveChatId(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  if (isCheckingAuth) {
    return (
      <I18nContext.Provider value={{ language, t }}>
        <main className="app-root grid place-items-center bg-stone-100 text-stone-950">
          <LoaderCircle aria-hidden="true" className="size-6 animate-spin text-teal-700" />
        </main>
      </I18nContext.Provider>
    );
  }

  if (authStatus?.enabled && !authStatus.authenticated) {
    return (
      <I18nContext.Provider value={{ language, t }}>
        <LoginView
          error={error}
          isLoggingIn={isLoggingIn}
          onLogin={(event) => void handleLogin(event)}
          onPasswordChange={setAuthPassword}
          password={authPassword}
        />
      </I18nContext.Provider>
    );
  }

  return (
    <I18nContext.Provider value={{ language, t }}>
    <main className="app-root foco-workbench text-stone-950">
      {isGlobalView ? (
        <div className="global-shell">
          <FocoNavRail
            activeMode={viewMode}
            canLogout={canLogout}
            isSavingTheme={isSavingTheme}
            onAddWorkspace={openWorkspaceDialog}
            onLogout={handleLogout}
            onOpenSettings={() => openSettingsSection("general")}
            onOpenStats={openStatsView}
            onReturnHome={openCurrentChatView}
            onToggleTheme={() =>
              void saveAppTheme(theme === "dark" ? "light" : "dark")
            }
            theme={theme}
          />
          <section className="global-main-panel min-w-0">
            {viewMode === "settings" ? (
              <SettingsPanel
                canLogout={canLogout}
                activeSection={settingsSection}
                onAddWorkspace={openWorkspaceDialog}
                onActiveSectionChange={openSettingsSection}
                onLogout={handleLogout}
                onSettingsChange={setSettings}
                onWorkspacesChange={refreshWorkspaces}
                workspaceDialogRevision={workspaceDialogRevision}
              />
            ) : (
              <ApiStatsPanel
                settings={settings}
                workspaces={workspaces}
              />
            )}
          </section>
        </div>
      ) : (
        <div
          className={`app-shell ${showContextPanel ? "app-shell-with-context" : ""}`}
          style={
            {
              "--diff-panel-width": `${diffPanelWidth}px`,
              "--sidebar-width": `${sidebarWidth}px`,
            } as CSSProperties
          }
        >
        {isMobileWorkspaceOpen ? (
          <button
            aria-label={t("Close")}
            className="mobile-sidebar-backdrop"
            onClick={() => setIsMobileWorkspaceOpen(false)}
            type="button"
          />
        ) : null}
        <FocoNavRail
          activeMode={viewMode}
          canLogout={canLogout}
          isSavingTheme={isSavingTheme}
          onAddWorkspace={openWorkspaceDialog}
          onLogout={handleLogout}
          onOpenSettings={() => openSettingsSection("general")}
          onOpenStats={openStatsView}
          onReturnHome={openCurrentChatView}
          onToggleTheme={() =>
            void saveAppTheme(theme === "dark" ? "light" : "dark")
          }
          theme={theme}
        />
        <aside
          className={`workspace-sidebar relative border-stone-200/80 lg:border-r ${
            isMobileWorkspaceOpen ? "workspace-sidebar-mobile-open" : ""
          }`}
          ref={workspaceSidebarRef}
        >
          <div
            aria-label={t("Resize workspace sidebar")}
            aria-orientation="vertical"
            aria-valuemax={WORKSPACE_SIDEBAR_MAX_WIDTH}
            aria-valuemin={WORKSPACE_SIDEBAR_MIN_WIDTH}
            aria-valuenow={sidebarWidth}
            className={`workspace-sidebar-splitter cursor-col-resize ${
              isResizingSidebar ? "workspace-sidebar-splitter-active" : ""
            }`}
            onKeyDown={(event) => {
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                setSidebarWidth((current) =>
                  Math.max(current - 24, WORKSPACE_SIDEBAR_MIN_WIDTH),
                );
              }

              if (event.key === "ArrowRight") {
                event.preventDefault();
                setSidebarWidth((current) =>
                  Math.min(current + 24, WORKSPACE_SIDEBAR_MAX_WIDTH),
                );
              }
            }}
            onPointerDown={(event) => {
              event.preventDefault();
              updateSidebarWidthFromClientX(event.clientX);
              setIsResizingSidebar(true);
            }}
            role="separator"
            tabIndex={0}
          />
          <div className="flex h-full min-h-0 flex-col">
            <div className="workspace-sidebar-header flex items-center justify-between gap-2 border-b border-stone-200/80 px-4 py-2">
              <div className="min-w-0">
                <span className="workspace-sidebar-title">
                  {t("Workspaces")}
                </span>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <button
                  aria-label={t("Close")}
                  className="mobile-sidebar-close inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                  onClick={() => setIsMobileWorkspaceOpen(false)}
                  title={t("Close")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>

            {error ? (
              <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
                {error}
              </div>
            ) : null}

            <nav
              aria-label={t("Workspace list")}
              className="workspace-nav panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3"
            >
              {workspaces.length ? (
                workspaces.map((workspace) => {
                  const isExpanded = expandedWorkspaceId === workspace.id;
                  const isActive = workspace.id === activeWorkspace?.id;
                  const selectedChatIndex =
                    isActive && activeChatId
                      ? workspace.chats.findIndex(
                          (chat) => chat.id === activeChatId,
                        )
                      : -1;
                  const configuredVisibleChatCount =
                    workspaceChatVisibleCounts[workspace.id] ??
                    WORKSPACE_CHAT_HISTORY_PAGE_SIZE;
                  const visibleChatCount =
                    selectedChatIndex >= configuredVisibleChatCount
                      ? selectedChatIndex + 1
                      : configuredVisibleChatCount;
                  const visibleChats = workspace.chats.slice(0, visibleChatCount);
                  const hiddenChatCount = Math.max(
                    workspace.chats.length - visibleChats.length,
                    0,
                  );
                  const nextVisibleChatCount = Math.min(
                    WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
                    hiddenChatCount,
                  );

                  return (
                    <div className="mb-1.5" key={workspace.id}>
                      <div className={workspaceMenuClass(isActive)}>
                        <button
                          aria-expanded={isExpanded}
                          className={workspaceItemClass(isActive)}
                          onClick={() => toggleWorkspace(workspace.id)}
                          title={
                            isExpanded
                              ? t("Collapse chat history")
                              : t("Expand chat history")
                          }
                          type="button"
                        >
                          {isExpanded ? (
                            <ChevronDown
                              aria-hidden="true"
                              className="workspace-expand-icon"
                            />
                          ) : (
                            <ChevronRight
                              aria-hidden="true"
                              className="workspace-expand-icon"
                            />
                          )}
                          <WorkspaceIcon
                            className="size-4 shrink-0 rounded object-cover"
                            fallbackClassName="size-4 shrink-0"
                            logoUrl={workspace.logoUrl}
                          />
                          <span className="min-w-0 flex-1 truncate text-left">
                            {workspace.name}
                          </span>
                        </button>
                        <button
                          aria-label={t("New chat in {name}", {
                            name: workspace.name,
                          })}
                          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg text-stone-500 hover:text-teal-800"
                          onClick={() => startNewWorkspaceChat(workspace.id)}
                          title={t("New chat")}
                          type="button"
                        >
                          <Plus aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                      {isExpanded ? (
                        <div className="ml-4 mt-1 space-y-1 border-l border-stone-200/80 pl-2">
                          {workspace.chats.length > 0 ? (
                            <>
                              {visibleChats.map((chat) => {
                                const chatKey = chatRunKey(workspace.id, chat.id);
                                const isChatRunning =
                                  runningChatKeys.has(chatKey);
                                const isChatOpen = openChatKeySet.has(chatKey);
                                const isChatFailed =
                                  isChatOpen && failedChatKeySet.has(chatKey);
                                let statusDotClass = "session-status-dot-idle";
                                if (isChatRunning) {
                                  statusDotClass = "session-status-dot-running";
                                } else if (isChatFailed) {
                                  statusDotClass = "session-status-dot-error";
                                } else if (isChatOpen) {
                                  statusDotClass = "session-status-dot-open";
                                }
                                const isChatActive =
                                  activeWorkspace?.id === workspace.id &&
                                  activeChatId === chat.id;
                                const chatDiffStats = chat.codeChangeStats;

                                return (
                                  <div
                                    className="group flex min-w-0 items-center gap-1"
                                    key={chat.id}
                                  >
                                    <button
                                      aria-current={
                                        isChatActive ? "page" : undefined
                                      }
                                      className={chatItemClass(isChatActive)}
                                      onClick={() =>
                                        selectWorkspaceChat(workspace.id, chat.id)
                                      }
                                      type="button"
                                    >
                                      <span
                                        aria-hidden="true"
                                        className={`session-status-dot ${statusDotClass}`}
                                      />
                                      <span className="min-w-0 flex-1">
                                        <span className="block truncate">
                                          {chat.title}
                                        </span>
                                        <span className="mt-0.5 flex min-w-0 items-center justify-between gap-2 text-[0.68rem] font-normal leading-tight text-stone-400">
                                          <span className="min-w-0 truncate">
                                            {formatChatCreatedAt(chat.createdAt)}
                                          </span>
                                          {chatDiffStats &&
                                          hasGitDiffStats(chatDiffStats) ? (
                                            <span
                                              aria-label={t(
                                                "Code changes +{additions} -{deletions}",
                                                {
                                                  additions:
                                                    chatDiffStats.additions,
                                                  deletions:
                                                    chatDiffStats.deletions,
                                                },
                                              )}
                                              className="chat-diff-stats"
                                              title={t(
                                                "Code changes +{additions} -{deletions}",
                                                {
                                                  additions:
                                                    chatDiffStats.additions,
                                                  deletions:
                                                    chatDiffStats.deletions,
                                                },
                                              )}
                                            >
                                              <span className="chat-diff-add">
                                                +{chatDiffStats.additions}
                                              </span>
                                              <span className="chat-diff-delete">
                                                -{chatDiffStats.deletions}
                                              </span>
                                            </span>
                                          ) : null}
                                        </span>
                                      </span>
                                    </button>
                                    <button
                                      aria-label={t("Delete chat {title}", {
                                        title: chat.title,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100"
                                      onClick={() =>
                                        requestDeleteWorkspaceChat(workspace, chat)
                                      }
                                      title={t("Delete chat")}
                                      type="button"
                                    >
                                      <Trash2
                                        aria-hidden="true"
                                        className="size-3.5"
                                      />
                                    </button>
                                  </div>
                                );
                              })}
                              {hiddenChatCount > 0 ? (
                                <div className="group flex min-w-0 items-center gap-1">
                                  <button
                                    aria-label={t(
                                      "Show {count} more chats in {name}",
                                      {
                                        count: nextVisibleChatCount,
                                        name: workspace.name,
                                      },
                                    )}
                                    className="flex min-h-10 min-w-0 flex-1 items-center gap-2 rounded-lg border border-transparent px-2 py-1.5 text-left text-xs font-medium text-stone-500 hover:border-stone-200 hover:bg-white/80 hover:text-stone-950"
                                    onClick={() =>
                                      showMoreWorkspaceChats(workspace.id)
                                    }
                                    type="button"
                                  >
                                    <ChevronDown
                                      aria-hidden="true"
                                      className="size-3.5 shrink-0"
                                    />
                                    <span className="min-w-0 flex-1">
                                      <span className="block truncate">
                                        {t("Show {count} more chats", {
                                          count: nextVisibleChatCount,
                                        })}
                                      </span>
                                      <span className="mt-0.5 block truncate text-[0.68rem] font-normal leading-tight text-stone-400">
                                        {t("{count} hidden chats", {
                                          count: hiddenChatCount,
                                        })}
                                      </span>
                                    </span>
                                  </button>
                                  <span
                                    aria-hidden="true"
                                    className="inline-flex size-7 shrink-0"
                                  />
                                </div>
                              ) : null}
                            </>
                          ) : (
                            <div className="rounded-lg px-2 py-1.5 text-xs text-stone-500">
                              {t("No chats")}
                            </div>
                          )}
                        </div>
                      ) : null}
                    </div>
                  );
                })
              ) : (
                <div className="mx-2 rounded-lg border border-dashed border-stone-300 bg-white/60 px-3 py-4 text-sm text-stone-500">
                  {isLoading ? t("Loading workspaces...") : t("No workspaces")}
                </div>
              )}
            </nav>
          </div>
        </aside>

        <section className="app-main-panel flex min-w-0 flex-col">
              <header className="app-toolbar shrink-0 border-b border-stone-200/80 bg-white/80 backdrop-blur">
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <button
                    aria-label={t("Workspaces")}
                    className="mobile-workspace-button inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 md:hidden"
                    onClick={() => setIsMobileWorkspaceOpen(true)}
                    title={t("Workspaces")}
                    type="button"
                  >
                    <Folder aria-hidden="true" className="size-4" />
                  </button>
                  <ChatTabBar
                    activeChatId={activeChatId}
                    activeWorkspaceId={activeWorkspaceId}
                    onCloseTab={closeChatTab}
                    onSelectTab={selectWorkspaceChat}
                    runningChatKeys={runningChatKeys}
                    tabs={chatTabs}
                  />
                  <div className="chat-header-actions">
                    <button
                      aria-label={
                        isTerminalOpen ? t("Close terminal") : t("Open terminal")
                      }
                      className={`chat-toolbar-button terminal-toolbar-button ${
                        isTerminalOpen ? "chat-toolbar-button-active" : ""
                      } disabled:cursor-not-allowed disabled:text-stone-400`}
                      disabled={!activeWorkspace}
                      onClick={toggleWorkspaceTerminal}
                      title={
                        isTerminalOpen ? t("Terminal (1)") : t("Terminal")
                      }
                      type="button"
                    >
                      <span
                        aria-hidden="true"
                        className={`terminal-status-dot ${
                          isTerminalOpen ? "terminal-status-dot-running" : ""
                        }`}
                      />
                      <SquareTerminal aria-hidden="true" className="size-4" />
                    </button>
                    <button
                      aria-label={
                        isContextPanelOpen
                          ? t("Close context panel")
                          : t("Open context panel")
                      }
                      className={`chat-toolbar-button ${
                        isContextPanelOpen ? "chat-toolbar-button-active" : ""
                      }`}
                      onClick={() => {
                        setIsContextPanelOpen((current) => !current);
                      }}
                      title={
                        isContextPanelOpen
                          ? t("Close context panel")
                          : t("Open context panel")
                      }
                      type="button"
                    >
                      <PanelRight aria-hidden="true" className="size-4" />
                    </button>
                  </div>
                </div>
              </header>
          <ChatPanel
              activeWorkspaceName={activeWorkspace?.name ?? null}
              availableModels={availableModels}
              branchError={branchError}
              chatScrollKey={`${activeWorkspaceId}:${activeChatId ?? ""}`}
              canGuideActiveRun={
                activeRunInfo?.chatKey === activeChatKey &&
                activeRunInfo.runId !== null
              }
              draftAttachments={draftAttachments}
              draftMessage={draftMessage}
              gitBranches={gitBranches}
              contextUsage={contextUsage}
              isLoadingSettings={isLoadingSettings}
              isLoadingBranches={isLoadingBranches}
              isLoadingContextUsage={isLoadingContextUsage}
              isSendingMessage={isSendingMessage}
              isSelectingAttachments={isSelectingAttachments}
              messages={messages}
              onAddPastedImageAttachments={(files) =>
                void handleAddPastedImageAttachments(files)
              }
              onBranchChange={(branch) => void handleGitBranchChange(branch)}
              onDraftMessageChange={setDraftMessage}
              onSelectAttachments={() => void handleSelectDraftAttachments()}
              onCancelRun={() => void handleCancelRun()}
              onGuideActiveRun={() => void handleGuideActiveRun()}
              onQueueActiveRun={handleQueueActiveRun}
              onModelChange={handleChatModelChange}
              onProviderChange={setSelectedProviderId}
              onRemoveAttachment={handleRemoveDraftAttachment}
              onRemoveSkill={removeSelectedSkill}
              onRetryRun={() => void handleRetryRun()}
              onSubmit={handleSendMessage}
              onThinkingLevelChange={setSelectedThinkingLevel}
              onToggleSkill={toggleSelectedSkill}
              canRetryRun={retryRunRequest !== null && !isSendingMessage}
              queuedRunCount={queuedRunRequests.length}
              selectedGitBranch={selectedGitBranch}
              selectedModelId={selectedModelId}
              selectedProviderId={selectedProviderId}
              selectedSkillIds={selectedSkillIds}
              selectedThinkingLevel={selectedThinkingLevel}
              settings={settings}
              providers={settings?.providers ?? []}
              skills={detectedSkills}
              thinkingLevels={thinkingLevels}
              workspaces={workspaces}
            />
          {isTerminalOpen ? (
            <TerminalPanel
              onClose={() => {
                if (activeWorkspace) {
                  setTerminalOpenWorkspaceIds((current) => {
                    const next = new Set(current);
                    next.delete(activeWorkspace.id);
                    return next;
                  });
                }
              }}
              workspace={activeWorkspace}
            />
          ) : null}
        </section>

        {showContextPanel ? (
        <aside className="context-sidebar diff-sidebar min-w-0 border-stone-200/80 lg:border-l">
          <div className="relative flex h-full min-h-0 min-w-0 flex-col">
            <div
              aria-label={t("Resize context panel")}
              aria-orientation="vertical"
              className="context-sidebar-splitter absolute bottom-0 left-0 top-0 z-10 hidden w-1 cursor-col-resize bg-transparent hover:bg-teal-500/40 lg:block"
              onKeyDown={(event) => {
                if (event.key === "ArrowLeft") {
                  event.preventDefault();
                  setDiffPanelWidth((current) =>
                    Math.min(current + 24, CONTEXT_PANEL_MAX_WIDTH),
                  );
                }

                if (event.key === "ArrowRight") {
                  event.preventDefault();
                  setDiffPanelWidth((current) =>
                    Math.max(current - 24, CONTEXT_PANEL_MIN_WIDTH),
                  );
                }
              }}
              onPointerDown={(event) => {
                event.preventDefault();
                setIsResizingDiffPanel(true);
              }}
              role="separator"
              tabIndex={0}
            />
            <ContextPanel
              activeTab={contextPanelTab}
              contextMemories={contextMemories}
              deletingContextMemoryId={deletingContextMemoryId}
              contextMemoryError={contextMemoryError}
              diffError={diffError}
              diffResponse={gitDiff}
              files={gitDiff?.files ?? []}
              isLoadingDiff={isLoadingDiff}
              isLoadingContextMemories={isLoadingContextMemories}
              isLoadingTodoGraph={isLoadingTodoGraph}
              onRefreshDiff={() => {
                if (activeWorkspace?.id) {
                  void loadGitDiff(activeWorkspace.id, selectedDiffPath);
                }
              }}
              onForgetContextMemory={(memory) => void forgetContextMemory(memory)}
              onSelectDiffFile={setSelectedDiffPath}
              onTabChange={(tab) => {
                setContextPanelTab(tab);
                setIsContextPanelOpen(true);
              }}
              selectedPath={selectedDiffPath}
              todoGraph={todoGraph}
              todoGraphError={todoGraphError}
            />
          </div>
        </aside>
        ) : null}
      </div>
      )}
      {isWorkspaceDialogOpen ? (
        <WorkspaceDialog
          isSelectingPath={isSelectingWorkspacePath}
          isSaving={isSavingWorkspace}
          name={workspaceName}
          onClose={() => setIsWorkspaceDialogOpen(false)}
          onNameChange={setWorkspaceName}
          onPathChange={setWorkspacePath}
          onSelectPath={handleSelectWorkspacePath}
          onSubmit={handleWorkspaceSubmit}
          path={workspacePath}
        />
      ) : null}
      {isBranchDialogOpen ? (
        <GitBranchDialog
          branchName={newBranchName}
          error={branchError}
          isSaving={isSavingBranch}
          onBranchNameChange={setNewBranchName}
          onClose={() => setIsBranchDialogOpen(false)}
          onSubmit={handleCreateGitBranch}
        />
      ) : null}
      {pendingDeleteChat ? (
        <DeleteChatDialog
          chat={pendingDeleteChat}
          onClose={() => setPendingDeleteChat(null)}
          onConfirm={() => void confirmDeleteWorkspaceChat()}
        />
      ) : null}
      {pendingQuestion ? (
        <QuestionDialog
          error={questionError}
          isSaving={isAnsweringQuestion}
          onCancelRun={() => void handleCancelRun()}
          onSubmit={handleQuestionSubmit}
          question={pendingQuestion}
        />
      ) : null}
      {settings && !settings.nativeTools.ripgrep.available && !isRipgrepDialogDismissed ? (
        <RipgrepMissingDialog
          error={ripgrepInstallError}
          installDir={settings.nativeTools.ripgrep.installDir}
          isInstalling={isInstallingRipgrep}
          onClose={() => setIsRipgrepDialogDismissed(true)}
          onInstall={() => void handleInstallRipgrep()}
        />
      ) : null}
    </main>
    </I18nContext.Provider>
  );
}

function RipgrepMissingDialog({
  error,
  installDir,
  isInstalling,
  onClose,
  onInstall,
}: {
  error: string | null;
  installDir: string;
  isInstalling: boolean;
  onClose: () => void;
  onInstall: () => void;
}) {
  const { language, t } = useI18n();

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="ripgrep-dialog-title"
        aria-modal="true"
        className="w-full max-w-lg overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <CircleAlert
              aria-hidden="true"
              className="size-5 shrink-0 text-amber-600"
            />
            <div className="min-w-0">
              <h2
                className="truncate text-base font-semibold text-stone-950"
                id="ripgrep-dialog-title"
              >
                {t("rg command was not found")}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {installDir}
              </p>
            </div>
          </div>
          <button
            aria-label={t("Dismiss ripgrep warning")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <div className="space-y-4 px-4 py-4">
          <p className="text-sm leading-6 text-stone-700">
            {t(
              "Foco uses ripgrep for full-text search. Install it into {path} so the search_text tool can run.",
              { path: installDir },
            )}
          </p>
          {error ? (
            <p className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-medium text-rose-700">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Download ripgrep")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isInstalling}
              onClick={onInstall}
              title={isInstalling ? t("Installing ripgrep...") : t("Download ripgrep")}
              type="button"
            >
              {isInstalling ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : (
                <Download aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function WorkspaceDialog({
  isSelectingPath,
  isSaving,
  name,
  onClose,
  onNameChange,
  onPathChange,
  onSelectPath,
  onSubmit,
  path,
}: {
  isSelectingPath: boolean;
  isSaving: boolean;
  name: string;
  onClose: () => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSelectPath: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  path: string;
}) {
  const { language, t } = useI18n();
  const title = t("Add workspace");

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="workspace-dialog-title"
        aria-modal="true"
        className="w-full max-w-lg overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="min-w-0">
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="workspace-dialog-title"
            >
              {title}
            </h2>
            <p className="mt-1 truncate text-xs font-medium text-stone-500">
              {t("Create or register a local folder.")}
            </p>
          </div>
          <button
            aria-label={t("Close workspace dialog")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <form
          className="space-y-4 px-4 py-4"
          onSubmit={(event) => void onSubmit(event)}
        >
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Name")}
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="workspace-name"
              onChange={(event) => onNameChange(event.target.value)}
              placeholder={t("Workspace name")}
              value={name}
            />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Path")}
            </span>
            <div className="flex gap-2">
              <input
                autoComplete="off"
                className="h-11 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                name="workspace-path"
                onChange={(event) => onPathChange(event.target.value)}
                placeholder="C:/Users/name/workspace"
                value={path}
              />
              <button
                aria-label={t("Choose workspace path")}
                className="inline-flex size-11 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                disabled={isSelectingPath}
                onClick={onSelectPath}
                title={t("Choose workspace path")}
                type="button"
              >
                {isSelectingPath ? (
                  <LoaderCircle
                    aria-hidden="true"
                    className="size-4 animate-spin"
                  />
                ) : (
                  <FolderSearch aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>
          </label>
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel workspace dialog")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={title}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving}
              title={title}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : (
                <FolderPlus aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function GitBranchDialog({
  branchName,
  error,
  isSaving,
  onBranchNameChange,
  onClose,
  onSubmit,
}: {
  branchName: string;
  error: string | null;
  isSaving: boolean;
  onBranchNameChange: (value: string) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const { t } = useI18n();
  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="git-branch-dialog-title"
        aria-modal="true"
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <GitBranch aria-hidden="true" className="size-5 text-teal-700" />
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="git-branch-dialog-title"
            >
              {t("New branch")}
            </h2>
          </div>
          <button
            aria-label={t("Close branch dialog")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <form className="space-y-4 px-4 py-4" onSubmit={onSubmit}>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Branch name")}
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="git-branch-name"
              onChange={(event) => onBranchNameChange(event.target.value)}
              placeholder="feature/name"
              value={branchName}
            />
          </label>
          {error ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel branch creation")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Create branch")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving || !branchName.trim()}
              title={t("Create branch")}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : (
                <Plus aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function DeleteChatDialog({
  chat,
  onClose,
  onConfirm,
}: {
  chat: PendingDeleteChat;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="delete-chat-dialog-title"
        aria-modal="true"
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <Trash2 aria-hidden="true" className="size-5 text-rose-700" />
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="delete-chat-dialog-title"
            >
              {t("Delete this chat?")}
            </h2>
          </div>
          <button
            aria-label={t("Cancel chat deletion")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Cancel")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="space-y-4 px-4 py-4">
          <div>
            <p className="text-sm font-medium text-stone-950">{chat.title}</p>
            <p className="mt-1 text-xs font-medium text-stone-500">
              {chat.workspaceName}
            </p>
          </div>
          <p className="text-sm leading-6 text-stone-600">
            {t("This will delete the saved chat history.")}
          </p>
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel chat deletion")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-stone-300 hover:bg-stone-50"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Confirm delete chat")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-rose-700 text-white shadow-[0_12px_28px_rgba(190,18,60,0.22)] hover:bg-rose-800"
              onClick={onConfirm}
              title={t("Delete chat")}
              type="button"
            >
              <Trash2 aria-hidden="true" className="size-4" />
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function QuestionDialog({
  error,
  isSaving,
  onCancelRun,
  onSubmit,
  question,
}: {
  error: string | null;
  isSaving: boolean;
  onCancelRun: () => void;
  onSubmit: (answer: QuestionAnswerSubmission) => void;
  question: QuestionRequestSummary;
}) {
  const { t } = useI18n();
  const [draftAnswers, setDraftAnswers] = useState<
    Record<string, { manualAnswer: string; selectedOptionValue: string | null }>
  >({});
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    setDraftAnswers(
      Object.fromEntries(
        question.questions.map((item) => [
          item.id,
          {
            manualAnswer: "",
            selectedOptionValue: null,
          },
        ]),
      ),
    );
    setLocalError(null);
  }, [question.id, question.questions]);

  function submitAnswer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const answers = question.questions.map((item) => {
      const draft = draftAnswers[item.id] ?? {
        manualAnswer: "",
        selectedOptionValue: null,
      };

      if (draft.selectedOptionValue !== null) {
        return {
          id: item.id,
          answer: draft.selectedOptionValue,
          selectedOptionValue: draft.selectedOptionValue,
        };
      }

      return {
        id: item.id,
        answer: draft.manualAnswer.trim(),
        selectedOptionValue: null,
      };
    });

    if (answers.some((answer) => !answer.answer)) {
      setLocalError(t("Answer must not be empty."));
      return;
    }

    onSubmit({ answers });
  }

  const displayedError = localError ?? error;
  const canSubmit =
    question.questions.length > 0 &&
    question.questions.every((item) => {
      const draft = draftAnswers[item.id];
      if (!draft) {
        return false;
      }

      return (
        draft.selectedOptionValue !== null ||
        draft.manualAnswer.trim().length > 0
      );
    });

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="question-dialog-title"
        aria-modal="true"
        className="w-full max-w-xl overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <MessageSquare
              aria-hidden="true"
              className="size-5 shrink-0 text-teal-700"
            />
            <div className="min-w-0">
              <h2
                className="truncate text-base font-semibold text-stone-950"
                id="question-dialog-title"
              >
                {t("Foco needs your answer")}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {t("Waiting for your answer")}
              </p>
            </div>
          </div>
          <button
            aria-label={t("Cancel run")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onCancelRun}
            title={t("Cancel run")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <form
          className="max-h-[min(72vh,720px)] space-y-4 overflow-y-auto px-4 py-4"
          onSubmit={submitAnswer}
        >
          <div className="space-y-4">
            {question.questions.map((item, index) => {
              const draft = draftAnswers[item.id] ?? {
                manualAnswer: "",
                selectedOptionValue: null,
              };

              return (
                <section
                  className="space-y-3 rounded-lg border border-stone-200 bg-stone-50/60 p-3"
                  key={item.id}
                >
                  <p className="whitespace-pre-wrap text-sm font-semibold leading-6 text-stone-900">
                    {question.questions.length > 1
                      ? `${index + 1}. ${item.question}`
                      : item.question}
                  </p>

                  {item.options.length ? (
                    <div className="space-y-2">
                      {item.options.map((option) => {
                        const isSelected =
                          draft.selectedOptionValue === option.value;
                        return (
                          <label
                            className={`flex cursor-pointer gap-3 rounded-lg border px-3 py-2 text-sm transition ${
                              isSelected
                                ? "border-teal-700 bg-teal-50 text-teal-950"
                                : "border-stone-200 bg-white text-stone-800 hover:border-teal-200 hover:bg-teal-50/60"
                            }`}
                            key={option.value}
                          >
                            <input
                              checked={isSelected}
                              className="mt-1 size-4 accent-teal-800"
                              name={`question-option-${item.id}`}
                              onChange={() => {
                                setDraftAnswers((current) => ({
                                  ...current,
                                  [item.id]: {
                                    manualAnswer:
                                      current[item.id]?.manualAnswer ?? "",
                                    selectedOptionValue: option.value,
                                  },
                                }));
                                setLocalError(null);
                              }}
                              type="radio"
                            />
                            <span className="min-w-0">
                              <span className="block font-semibold">
                                {option.label}
                              </span>
                              {option.description ? (
                                <span className="mt-0.5 block text-xs leading-5 text-stone-500">
                                  {option.description}
                                </span>
                              ) : null}
                            </span>
                          </label>
                        );
                      })}
                    </div>
                  ) : null}

                  {item.allowFreeText ? (
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Custom answer")}
                      </span>
                      <textarea
                        className="min-h-24 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) => {
                          setDraftAnswers((current) => ({
                            ...current,
                            [item.id]: {
                              manualAnswer: event.target.value,
                              selectedOptionValue: null,
                            },
                          }));
                          setLocalError(null);
                        }}
                        value={draft.manualAnswer}
                      />
                    </label>
                  ) : null}
                </section>
              );
            })}
          </div>

          {displayedError ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {displayedError}
            </div>
          ) : null}

          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel run")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onCancelRun}
              title={t("Cancel run")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Continue run")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving || !canSubmit}
              title={t("Continue run")}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : (
                <CheckCircle2 aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function ChatTabBar({
  activeChatId,
  activeWorkspaceId,
  onCloseTab,
  onSelectTab,
  runningChatKeys,
  tabs,
}: {
  activeChatId: string | null;
  activeWorkspaceId: string;
  onCloseTab: (workspaceId: string, chatId: string) => void;
  onSelectTab: (workspaceId: string, chatId: string) => void;
  runningChatKeys: Set<string>;
  tabs: ChatTabSummary[];
}) {
  const { t } = useI18n();
  const tabsContainerRef = useRef<HTMLDivElement>(null);
  const tabListRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({
    canScrollLeft: false,
    canScrollRight: false,
    hasOverflow: false,
  });

  const updateScrollState = useCallback(() => {
    const element = tabListRef.current;
    if (!element) {
      setScrollState({
        canScrollLeft: false,
        canScrollRight: false,
        hasOverflow: false,
      });
      return;
    }

    const maxScrollLeft = Math.max(0, element.scrollWidth - element.clientWidth);
    const availableWidth = tabsContainerRef.current?.clientWidth ?? element.clientWidth;
    const hasOverflow = element.scrollWidth > availableWidth + 1;
    if (!hasOverflow && element.scrollLeft !== 0) {
      element.scrollLeft = 0;
    }

    const scrollLeft = hasOverflow ? element.scrollLeft : 0;
    const nextState = {
      canScrollLeft: scrollLeft > 1,
      canScrollRight: scrollLeft < maxScrollLeft - 1,
      hasOverflow,
    };

    setScrollState((current) =>
      current.canScrollLeft === nextState.canScrollLeft &&
      current.canScrollRight === nextState.canScrollRight &&
      current.hasOverflow === nextState.hasOverflow
        ? current
        : nextState,
    );
  }, []);

  useLayoutEffect(() => {
    updateScrollState();
  }, [tabs, updateScrollState]);

  useEffect(() => {
    const element = tabListRef.current;
    const container = tabsContainerRef.current;
    if (!element || !container) {
      return undefined;
    }

    const handleResize = () => updateScrollState();
    const resizeObserver =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(handleResize);

    resizeObserver?.observe(container);
    resizeObserver?.observe(element);
    window.addEventListener("resize", handleResize);
    return () => {
      resizeObserver?.disconnect();
      window.removeEventListener("resize", handleResize);
    };
  }, [updateScrollState]);

  function scrollTabs(direction: -1 | 1) {
    const element = tabListRef.current;
    if (!element) {
      return;
    }

    element.scrollBy({
      behavior: "smooth",
      left: direction * Math.max(180, Math.floor(element.clientWidth * 0.7)),
    });
  }

  function handleWheel(event: ReactWheelEvent<HTMLDivElement>) {
    const element = tabListRef.current;
    if (!element) {
      return;
    }

    const maxScrollLeft = Math.max(0, element.scrollWidth - element.clientWidth);
    if (maxScrollLeft <= 0) {
      return;
    }

    const rawDelta =
      Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (rawDelta === 0) {
      return;
    }

    const deltaUnit =
      event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? element.clientWidth : 1;
    const nextScrollLeft = Math.min(
      maxScrollLeft,
      Math.max(0, element.scrollLeft + rawDelta * deltaUnit),
    );

    if (nextScrollLeft === element.scrollLeft) {
      return;
    }

    event.preventDefault();
    element.scrollLeft = nextScrollLeft;
    updateScrollState();
  }

  return (
    <div
      className="chat-tabs flex min-w-0 flex-1 flex-nowrap overflow-hidden"
      ref={tabsContainerRef}
    >
      {scrollState.hasOverflow ? (
        <button
          aria-label={t("Scroll chat tabs left")}
          className="chat-tab-scroll-button"
          disabled={!scrollState.canScrollLeft}
          onClick={() => scrollTabs(-1)}
          title={t("Scroll chat tabs left")}
          type="button"
        >
          <ChevronLeft aria-hidden="true" className="size-4" />
        </button>
      ) : null}
      <div
        aria-label={t("Chat")}
        className="chat-tab-list panel-scroll flex min-w-0 flex-1 gap-1 overflow-x-auto"
        onScroll={updateScrollState}
        onWheel={handleWheel}
        ref={tabListRef}
        role="tablist"
      >
        {tabs.length ? (
          tabs.map((tab) => {
            const isActive =
              activeWorkspaceId === tab.workspaceId && activeChatId === tab.chatId;
            const isRunning = runningChatKeys.has(
              chatRunKey(tab.workspaceId, tab.chatId),
            );
            const title = tab.title || t("Chat");

            return (
              <div
                className={`chat-tab-item group flex h-12 min-w-36 max-w-64 shrink-0 items-center rounded-lg border px-2 py-1.5 transition-colors ${
                  isActive
                    ? "border-teal-200 bg-white text-stone-950 shadow-sm"
                    : "border-stone-200 bg-stone-50/80 text-stone-600 hover:border-stone-300 hover:bg-white"
                }`}
                key={chatRunKey(tab.workspaceId, tab.chatId)}
              >
                <button
                  aria-selected={isActive}
                  className="min-w-0 flex-1 text-left"
                  onClick={() => onSelectTab(tab.workspaceId, tab.chatId)}
                  role="tab"
                  title={`${title} · ${tab.workspaceName}`}
                  type="button"
                >
                  <span className="block truncate text-sm font-semibold leading-5">
                    {title}
                  </span>
                  <span className="flex min-w-0 items-center gap-1 text-[11px] font-medium leading-4 text-stone-400">
                    <WorkspaceIcon
                      className="size-3 shrink-0 rounded-sm object-cover"
                      fallbackClassName="size-3 shrink-0"
                      logoUrl={tab.workspaceLogoUrl}
                    />
                    <span className="min-w-0 truncate">{tab.workspaceName}</span>
                  </span>
                </button>
                <span className="ml-1 inline-flex size-7 shrink-0 items-center justify-center">
                  {isRunning ? (
                    <span aria-label={t("Chat is running")} role="status">
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin text-teal-700"
                      />
                    </span>
                  ) : (
                    <button
                      aria-label={t("Close chat tab {title}", { title })}
                      className="inline-flex size-7 items-center justify-center rounded-md text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100"
                      onClick={() => onCloseTab(tab.workspaceId, tab.chatId)}
                      title={t("Close")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                    </button>
                  )}
                </span>
              </div>
            );
          })
        ) : (
          <div className="flex h-12 min-w-0 items-center rounded-lg border border-dashed border-stone-300 bg-white/55 px-3 text-sm font-medium text-stone-500">
            {t("No open chats")}
          </div>
        )}
      </div>
      {scrollState.hasOverflow ? (
        <button
          aria-label={t("Scroll chat tabs right")}
          className="chat-tab-scroll-button"
          disabled={!scrollState.canScrollRight}
          onClick={() => scrollTabs(1)}
          title={t("Scroll chat tabs right")}
          type="button"
        >
          <ChevronRight aria-hidden="true" className="size-4" />
        </button>
      ) : null}
    </div>
  );
}

function FocoNavRail({
  activeMode,
  canLogout,
  isSavingTheme,
  onAddWorkspace,
  onLogout,
  onOpenSettings,
  onOpenStats,
  onReturnHome,
  onToggleTheme,
  theme,
}: {
  activeMode: ViewMode;
  canLogout: boolean;
  isSavingTheme: boolean;
  onAddWorkspace: () => void;
  onLogout: () => Promise<void>;
  onOpenSettings: () => void;
  onOpenStats: () => void;
  onReturnHome: () => void;
  onToggleTheme: () => void;
  theme: AppThemeId;
}) {
  const { t } = useI18n();
  const themeLabel =
    theme === "dark" ? t("Switch to light theme") : t("Switch to dark theme");

  return (
    <nav aria-label="Foco" className="foco-nav-rail">
      <div className="foco-nav-rail-main">
        <button
          aria-label="Foco"
          className="foco-nav-logo-button"
          onClick={onReturnHome}
          title="Foco"
          type="button"
        >
          <FocoLogoMark />
        </button>
        <NavRailButton
          active={activeMode === "chat"}
          icon={Home}
          label={t("Home")}
          onClick={onReturnHome}
        />
        <NavRailButton
          active={activeMode === "stats"}
          icon={Activity}
          label={t("API details")}
          onClick={onOpenStats}
        />
        <NavRailButton
          active={activeMode === "settings"}
          icon={Settings}
          label={t("Settings")}
          onClick={onOpenSettings}
        />
      </div>
      <div className="foco-nav-rail-bottom">
        <NavRailButton
          active={false}
          icon={FolderPlus}
          label={t("Add workspace")}
          onClick={onAddWorkspace}
        />
        <NavRailButton
          active={theme === "dark"}
          disabled={isSavingTheme}
          icon={SunMoon}
          label={themeLabel}
          onClick={onToggleTheme}
        />
        {canLogout ? (
          <NavRailButton
            active={false}
            icon={LogOut}
            label={t("Logout")}
            onClick={() => void onLogout()}
          />
        ) : null}
      </div>
    </nav>
  );
}

function NavRailButton({
  active,
  disabled = false,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  disabled?: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className={`foco-nav-rail-button ${active ? "foco-nav-rail-button-active" : ""}`}
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function ChatPanel({
  activeWorkspaceName,
  availableModels,
  branchError,
  chatScrollKey,
  canGuideActiveRun,
  canRetryRun,
  contextUsage,
  draftAttachments,
  draftMessage,
  gitBranches,
  queuedRunCount,
  isLoadingBranches,
  isLoadingContextUsage,
  isLoadingSettings,
  isSendingMessage,
  isSelectingAttachments,
  messages,
  onAddPastedImageAttachments,
  onBranchChange,
  onCancelRun,
  onDraftMessageChange,
  onGuideActiveRun,
  onModelChange,
  onProviderChange,
  onQueueActiveRun,
  onRemoveAttachment,
  onRemoveSkill,
  onRetryRun,
  onSelectAttachments,
  onSubmit,
  onThinkingLevelChange,
  onToggleSkill,
  selectedGitBranch,
  selectedModelId,
  selectedProviderId,
  selectedSkillIds,
  selectedThinkingLevel,
  settings,
  providers,
  skills,
  thinkingLevels,
  workspaces,
}: {
  activeWorkspaceName: string | null;
  availableModels: ConfiguredModelSummary[];
  branchError: string | null;
  chatScrollKey: string;
  canGuideActiveRun: boolean;
  canRetryRun: boolean;
  contextUsage: ContextUsageResponse | null;
  draftAttachments: ComposerAttachment[];
  draftMessage: string;
  gitBranches: GitBranchesResponse | null;
  queuedRunCount: number;
  isLoadingBranches: boolean;
  isLoadingContextUsage: boolean;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  isSelectingAttachments: boolean;
  messages: ShellMessage[];
  onAddPastedImageAttachments: (files: File[]) => void;
  onBranchChange: (value: string) => void;
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onGuideActiveRun: () => void;
  onModelChange: (value: string) => void;
  onProviderChange: (value: string) => void;
  onQueueActiveRun: () => void;
  onRemoveAttachment: (attachmentId: string) => void;
  onRemoveSkill: (skillId: string) => void;
  onRetryRun: () => void;
  onSelectAttachments: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onThinkingLevelChange: (value: string) => void;
  onToggleSkill: (skillId: string) => void;
  selectedGitBranch: string;
  selectedModelId: string;
  selectedProviderId: string;
  selectedSkillIds: string[];
  selectedThinkingLevel: string;
  settings: SettingsResponse | null;
  providers: ConfiguredProviderSummary[];
  skills: ConfiguredSkillSummary[];
  thinkingLevels: ThinkingLevelSummary[];
  workspaces: WorkspaceSummary[];
}) {
  const { t } = useI18n();
  const messageScrollRef = useRef<HTMLDivElement>(null);
  const messageScrollContentRef = useRef<HTMLDivElement>(null);
  const messageScrollEndRef = useRef<HTMLDivElement>(null);
  const messageTextareaRef = useRef<HTMLTextAreaElement>(null);
  const copiedMessageTimerRef = useRef<number | null>(null);
  const shouldLockMessageScrollRef = useRef(true);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = new Set(selectedSkillIds);
  const selectedSkills = selectedSkillIds
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill));
  const workspaceName = activeWorkspaceName?.trim();
  const composerPlaceholder = workspaceName
    ? t("Ask Foco anything about {name}...", { name: workspaceName })
    : t("Ask Foco anything...");
  const modelOptions = availableModels.map((model) => ({
    label: model.displayName,
    value: model.id,
  }));
  const selectedModel =
    availableModels.find((model) => model.id === selectedModelId) ?? null;
  const providersById = new Map(providers.map((provider) => [provider.id, provider]));
  const providerOptions = (selectedModel?.providerIds ?? []).map((providerId) => {
    const provider = providersById.get(providerId);
    return {
      label: provider?.name ?? providerId,
      value: providerId,
    };
  });
  const thinkingOptions = [
    { label: t("Model default"), value: "" },
    ...thinkingLevels.map((level) => ({
      label: t(level.label),
      value: level.value,
    })),
  ];
  const visibleSkills =
    skillQuery === null
      ? []
      : skills.filter((skill) => {
          const query = skillQuery.toLowerCase();
          return (
            skill.canEnable &&
            !selectedSkillSet.has(skill.key) &&
            (skill.name.toLowerCase().includes(query) ||
              skill.id.toLowerCase().includes(query) ||
              skill.key.toLowerCase().includes(query) ||
              skill.description.toLowerCase().includes(query))
          );
        });
  const hasComposerDraft = Boolean(draftMessage.trim() || draftAttachments.length);
  const runningButtonSendsMessage = isSendingMessage && hasComposerDraft;
  const runningButtonLabel = runningButtonSendsMessage
    ? t("Send guidance")
    : t("Cancel run");
  const runningButtonTitle = runningButtonSendsMessage
    ? queuedRunCount > 0
      ? t("Send guidance. Ctrl+click queues. {count} queued.", {
          count: queuedRunCount,
        })
      : t("Send guidance. Ctrl+click queues.")
    : t("Cancel run");

  function scrollMessageListToBottom() {
    messageScrollEndRef.current?.scrollIntoView({
      block: "end",
      inline: "nearest",
    });
  }

  useLayoutEffect(() => {
    const element = messageScrollRef.current;
    shouldLockMessageScrollRef.current = messages.length > 0;

    if (messages.length === 0) {
      if (element) {
        element.scrollTop = 0;
      }
      return;
    }

    scrollMessageListToBottom();
  }, [chatScrollKey, messages.length]);

  useLayoutEffect(() => {
    if (!shouldLockMessageScrollRef.current) {
      return;
    }

    scrollMessageListToBottom();
  }, [messages]);

  useLayoutEffect(() => {
    const container = messageScrollRef.current;
    const content = messageScrollContentRef.current;
    if (!container || !content) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldLockMessageScrollRef.current) {
        scrollMessageListToBottom();
      }
    });
    observer.observe(container);
    observer.observe(content);

    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    return () => {
      if (copiedMessageTimerRef.current !== null) {
        window.clearTimeout(copiedMessageTimerRef.current);
      }
    };
  }, []);

  function handleMessageScroll() {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }

    if (messages.length === 0) {
      shouldLockMessageScrollRef.current = false;
      return;
    }

    shouldLockMessageScrollRef.current =
      element.scrollHeight - element.scrollTop - element.clientHeight <=
      CHAT_BOTTOM_LOCK_THRESHOLD_PX;
  }

  function handleSkillSelect(skill: ConfiguredSkillSummary) {
    if (!skill.enabled) {
      return;
    }

    onDraftMessageChange(removeActiveSkillToken(draftMessage));
    onToggleSkill(skill.key);
  }

  function handleComposerSubmit(event: FormEvent<HTMLFormElement>) {
    onSubmit(event);
    window.requestAnimationFrame(() => messageTextareaRef.current?.focus());
  }

  function handleRunningRunButtonClick(
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    const hasDraft = Boolean(draftMessage.trim() || draftAttachments.length);
    if (!hasDraft) {
      onCancelRun();
      return;
    }

    if (event.ctrlKey) {
      onQueueActiveRun();
      return;
    }

    onGuideActiveRun();
  }

  function handlePaste(event: ReactClipboardEvent<HTMLTextAreaElement>) {
    const itemFiles = Array.from(event.clipboardData.items)
      .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null);
    const imageFiles = itemFiles.length
      ? itemFiles
      : Array.from(event.clipboardData.files).filter((file) =>
          file.type.startsWith("image/"),
        );
    if (!imageFiles.length) {
      return;
    }

    event.preventDefault();
    onAddPastedImageAttachments(imageFiles);
  }

  async function handleCopyMessage(messageId: string, text: string) {
    if (!text) {
      return;
    }

    try {
      await navigator.clipboard.writeText(text);
    } catch {
      return;
    }
    setCopiedMessageId(messageId);
    if (copiedMessageTimerRef.current !== null) {
      window.clearTimeout(copiedMessageTimerRef.current);
    }
    copiedMessageTimerRef.current = window.setTimeout(() => {
      setCopiedMessageId((current) => (current === messageId ? null : current));
      copiedMessageTimerRef.current = null;
    }, 1600);
  }

  return (
    <div className="chat-panel flex min-h-0 flex-1 flex-col overflow-hidden">
      <div
        className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4"
        onScroll={handleMessageScroll}
        ref={messageScrollRef}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${
            messages.length ? "max-w-5xl gap-4" : "max-w-6xl"
          }`}
          ref={messageScrollContentRef}
        >
          {messages.length ? (
            messages.map((message) => {
              const isUser = message.role === "user";
              const parts = message.parts.length
                ? message.parts
                : fallbackMessageParts(message);
              const authorLabel = isUser ? "You" : "Foco Agent";
              const createdAtLabel = formatChatCreatedAt(message.createdAt);
              const copyText = messageCopyText(message, parts);
              const copyLabel =
                copiedMessageId === message.id
                  ? t("Copied message")
                  : t("Copy message");
              const pendingLabel =
                message.pendingMode === "guidance"
                  ? t("Guidance pending")
                  : message.pendingMode === "queued"
                    ? t("Queued")
                    : null;
              const isPendingUserMessage = isUser && pendingLabel !== null;

              return (
                <div
                  className={`message-row flex ${isUser ? "message-row-user" : "message-row-agent"}`}
                  key={message.id}
                >
                  <div className="message-card-shell">
                    <div
                      className={`message-bubble flex max-w-[min(42rem,92%)] items-start gap-3 rounded-2xl border px-4 py-3 shadow-[0_18px_42px_rgba(75,63,42,0.08)] sm:max-w-[78%] ${
                        isUser
                          ? "message-bubble-user flex-row rounded-tr-md"
                          : "message-bubble-assistant flex-row rounded-tl-md"
                      } ${isPendingUserMessage ? "message-bubble-pending" : ""}`}
                      style={{
                        backgroundColor: isPendingUserMessage
                          ? "var(--foco-panel-soft)"
                          : isUser
                            ? "var(--foco-user-surface)"
                            : "var(--foco-panel)",
                        borderColor: isPendingUserMessage
                          ? "var(--foco-border)"
                          : isUser
                            ? "var(--foco-user-border)"
                            : "var(--foco-border)",
                      }}
                    >
                      <div
                        className={`message-avatar mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${
                          isUser
                            ? "bg-teal-950/45 text-white"
                            : "bg-stone-100 text-stone-700"
                        }`}
                      >
                        {isUser ? (
                          <User aria-hidden="true" className="size-4" />
                        ) : (
                          <Bot aria-hidden="true" className="size-4" />
                        )}
                      </div>
                      <div className="min-w-0 flex-1 space-y-3">
                        <div className="message-author-row">
                          <span className="message-author-meta">
                            <span>{authorLabel}</span>
                            {pendingLabel ? (
                              <span className="message-pending-badge">
                                {pendingLabel}
                              </span>
                            ) : null}
                            <time
                              className="message-created-at"
                              dateTime={message.createdAt}
                              title={message.createdAt}
                            >
                              {createdAtLabel}
                            </time>
                          </span>
                          <button
                            aria-label={copyLabel}
                            className="message-action-menu"
                            disabled={!copyText}
                            onClick={() =>
                              void handleCopyMessage(message.id, copyText)
                            }
                            title={copyLabel}
                            type="button"
                          >
                            {copiedMessageId === message.id ? (
                              <CheckCircle2
                                aria-hidden="true"
                                className="size-3.5"
                              />
                            ) : (
                              <Copy aria-hidden="true" className="size-3.5" />
                            )}
                          </button>
                        </div>
                        {!isUser ? (
                          <MemoriesUsedBlock memories={message.memoriesUsed} />
                        ) : null}
                        {parts.length ? (
                          parts.map((part, partIndex) => (
                            <MessagePartBlock
                              isError={message.status === "error"}
                              isStreaming={message.status === "streaming"}
                              isStreamingTail={partIndex === parts.length - 1}
                              isUser={isUser}
                              key={`${message.id}-part-${partIndex}`}
                              part={part}
                              reasoningDurationMs={message.metrics?.totalLatencyMs ?? null}
                            />
                          ))
                        ) : message.status === "streaming" ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : null}
                        {!isUser ? (
                          <ExtractedMemoriesBlock
                            memories={message.extractedMemories}
                          />
                        ) : null}
                        {!isUser && message.metrics ? (
                          <ChatReplyMetricsLine metrics={message.metrics} />
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })
          ) : (
            <ApiOverviewPanel settings={settings} workspaces={workspaces} />
          )}
        </div>
        <div aria-hidden="true" className="h-px" ref={messageScrollEndRef} />
      </div>

      <div className="composer-shell shrink-0 border-t border-stone-200/80 bg-transparent px-3 py-1.5 sm:px-5">
        <form className="mx-auto max-w-5xl" onSubmit={handleComposerSubmit}>
          <div className="composer-surface relative rounded-xl border border-stone-300 bg-white">
            {selectedSkills.length ? (
              <div className="flex flex-wrap gap-1.5 px-3 pt-2">
                {selectedSkills.map((skill) => (
                  <span
                    className="inline-flex max-w-full items-center gap-1 rounded-full border border-teal-200 bg-teal-50 px-2 py-1 text-xs font-semibold text-teal-900"
                    key={skill.key}
                  >
                    <span className="max-w-44 truncate">{skill.name}</span>
                    <button
                      aria-label={t("Remove skill {name}", {
                        name: skill.name,
                      })}
                      className="inline-flex size-4 items-center justify-center rounded-full text-teal-800 hover:bg-teal-100"
                      onClick={() => onRemoveSkill(skill.key)}
                      title={t("Remove skill")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
            ) : null}
            {draftAttachments.length ? (
              <div className="composer-attachment-list px-3 pt-2">
                {draftAttachments.map((attachment) => (
                  <ComposerAttachmentChip
                    attachment={attachment}
                    key={attachment.id}
                    onRemove={() => onRemoveAttachment(attachment.id)}
                  />
                ))}
              </div>
            ) : null}
            <textarea
              className="message-composer-textarea min-h-16 w-full resize-none border-0 bg-transparent px-3 py-1.5 text-sm leading-6 text-stone-900 outline-none placeholder:text-stone-400"
              name="message"
              onChange={(event) => onDraftMessageChange(event.target.value)}
              onKeyDown={(event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
                if (
                  event.key !== "Enter" ||
                  event.ctrlKey ||
                  event.shiftKey ||
                  event.nativeEvent.isComposing
                ) {
                  return;
                }

                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }}
              onPaste={handlePaste}
              placeholder={composerPlaceholder}
              ref={messageTextareaRef}
              value={draftMessage}
            />
            {skillQuery !== null ? (
              <div className="absolute bottom-full left-0 z-20 mb-2 w-full overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
                <div className="panel-scroll max-h-64 overflow-y-auto py-1">
                  {visibleSkills.length ? (
                    visibleSkills.map((skill) => (
                      <button
                        aria-label={t("Select skill {name}", {
                          name: skill.name,
                        })}
                        className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 px-3 py-2 text-left hover:bg-stone-50 disabled:cursor-not-allowed disabled:bg-stone-50 disabled:text-stone-400"
                        disabled={!skill.enabled}
                        key={skill.key}
                        onClick={() => handleSkillSelect(skill)}
                        title={
                          skill.enabled ? skill.description : t("Skill is disabled")
                        }
                        type="button"
                      >
                        <span className="min-w-0">
                          <span className="block truncate text-sm font-semibold text-stone-900">
                            {skill.name}
                          </span>
                          <span className="mt-0.5 block truncate text-xs text-stone-500">
                            {skill.description}
                          </span>
                        </span>
                        <span className="self-center rounded-md border border-stone-200 px-1.5 py-0.5 text-[11px] font-semibold text-stone-500">
                          {skill.enabled ? skillScopeLabel(skill, t) : t("disabled")}
                        </span>
                      </button>
                    ))
                  ) : (
                    <div className="px-3 py-3 text-sm text-stone-500">
                      {t("No matching skills")}
                    </div>
                  )}
                </div>
              </div>
            ) : null}
            <div
              className={`message-composer-control-row ${
                canRetryRun ? "message-composer-actions-with-retry" : ""
              }`}
            >
              <button
                aria-label={t("Add attachment")}
                className="composer-tool-button"
                disabled={isSelectingAttachments}
                onClick={onSelectAttachments}
                title={t("Add attachment")}
                type="button"
              >
                {isSelectingAttachments ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <Plus aria-hidden="true" className="size-4" />
                )}
              </button>
              <ComposerSelectMenu
                ariaLabel={t("Model")}
                className="composer-model-select max-w-full"
                disabled={isLoadingSettings || !modelOptions.length}
                emptyLabel={t("No enabled models")}
                icon={Bot}
                onChange={onModelChange}
                options={modelOptions}
                selectedValue={selectedModelId}
              />
              <ComposerSelectMenu
                ariaLabel={t("Provider")}
                className="composer-provider-select max-w-full"
                disabled={
                  isLoadingSettings ||
                  !selectedModelId ||
                  !providerOptions.length
                }
                emptyLabel={t("Provider")}
                icon={Server}
                onChange={onProviderChange}
                options={providerOptions}
                selectedValue={selectedProviderId}
              />
              <ComposerSelectMenu
                ariaLabel={t("Thinking")}
                className="composer-thinking-select max-w-full"
                disabled={isLoadingSettings}
                emptyLabel={t("Model default")}
                icon={SlidersHorizontal}
                onChange={onThinkingLevelChange}
                options={thinkingOptions}
                selectedValue={selectedThinkingLevel}
              />
              <BranchSelector
                branches={gitBranches?.branches ?? []}
                currentBranch={selectedGitBranch}
                disabled={isSendingMessage || isLoadingBranches}
                isGitRepository={gitBranches?.isGitRepository ?? false}
                isLoading={isLoadingBranches}
                onChange={onBranchChange}
              />
              {canRetryRun ? (
                <button
                  aria-label={t("Retry last run")}
                  className="composer-retry-button composer-run-button inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={onRetryRun}
                  title={t("Retry last run")}
                  type="button"
                >
                  <RefreshCw aria-hidden="true" className="size-4" />
                </button>
              ) : null}
              <span aria-hidden="true" className="composer-control-spacer" />
              <ContextUsageCircle
                isLoading={isLoadingContextUsage}
                usage={contextUsage}
              />
              {isSendingMessage ? (
                <button
                  aria-label={runningButtonLabel}
                  className={
                    runningButtonSendsMessage
                      ? "composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                      : "composer-run-button inline-flex size-8 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                  }
                  disabled={
                    runningButtonSendsMessage &&
                    (!canGuideActiveRun || !selectedModelId)
                  }
                  onClick={handleRunningRunButtonClick}
                  title={runningButtonTitle}
                  type="button"
                >
                  {runningButtonSendsMessage ? (
                    <Send aria-hidden="true" className="size-4" />
                  ) : (
                    <X aria-hidden="true" className="size-4" />
                  )}
                </button>
              ) : (
                <button
                  aria-label={t("Send message")}
                  className="composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                  disabled={
                    (!draftMessage.trim() && !draftAttachments.length) ||
                    !selectedModelId
                  }
                  title={t("Send")}
                  type="submit"
                >
                  <Send aria-hidden="true" className="size-4" />
                </button>
              )}
            </div>
          </div>
          {branchError ? (
            <div className="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {branchError}
            </div>
          ) : null}
        </form>
      </div>
    </div>
  );
}

function ContextUsageCircle({
  className = "",
  isLoading,
  usage,
}: {
  className?: string;
  isLoading: boolean;
  usage: ContextUsageResponse | null;
}) {
  const { t } = useI18n();
  const percent = usage?.usagePercent ?? 0;
  const clampedPercent = Math.min(Math.max(percent, 0), 100);
  const toneClass = usage?.willCompressOnNextSend
    ? "context-usage-circle-critical"
    : usage && percent >= usage.compressionTriggerPercent
      ? "context-usage-circle-warn"
      : "context-usage-circle-normal";
  const title = t("Context usage {percent}%", { percent });

  return (
    <div
      aria-label={title}
      className={`context-usage-circle ${toneClass} ${
        isLoading ? "context-usage-circle-loading" : ""
      } ${className}`}
      role="status"
      style={{
        "--context-usage-percent": `${clampedPercent}%`,
      } as CSSProperties}
      title={title}
    >
      {percent}%
    </div>
  );
}

type ComposerSelectOption = {
  label: string;
  value: string;
};

function ComposerSelectMenu({
  ariaLabel,
  className,
  disabled,
  emptyLabel,
  icon: Icon,
  onChange,
  options,
  selectedValue,
}: {
  ariaLabel: string;
  className: string;
  disabled: boolean;
  emptyLabel: string;
  icon: LucideIcon;
  onChange: (value: string) => void;
  options: ComposerSelectOption[];
  selectedValue: string;
}) {
  const selectedOption =
    options.find((option) => option.value === selectedValue) ?? null;
  const selectedLabel = selectedOption?.label ?? emptyLabel;
  const detailsRef = useCloseDetailsOnOutsidePointerDown();

  function handleSelect(value: string, event: ReactMouseEvent<HTMLButtonElement>) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onChange(value);
  }

  return (
    <details
      className={`composer-select-menu group relative ${className}`}
      ref={detailsRef}
    >
      <summary
        aria-disabled={disabled}
        aria-label={ariaLabel}
        className={`composer-select-summary flex h-8 w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${
          disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
        }`}
        title={selectedLabel}
      >
        <Icon aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="composer-select-label min-w-0 flex-1 truncate">
          {selectedLabel}
        </span>
        <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
      </summary>
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-52 overflow-y-auto py-1">
          {options.length ? (
            options.map((option) => (
              <button
                aria-label={`${ariaLabel}: ${option.label}`}
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${
                  option.value === selectedValue
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                }`}
                key={option.value}
                onClick={(event) => handleSelect(option.value, event)}
                type="button"
              >
                <Icon aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{option.label}</span>
                {option.value === selectedValue ? (
                  <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                ) : null}
              </button>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">{emptyLabel}</div>
          )}
        </div>
      </div>
    </details>
  );
}

function BranchSelector({
  branches,
  currentBranch,
  disabled,
  isGitRepository,
  isLoading,
  onChange,
}: {
  branches: string[];
  currentBranch: string;
  disabled: boolean;
  isGitRepository: boolean;
  isLoading: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useI18n();
  const detailsRef = useCloseDetailsOnOutsidePointerDown();
  if (!isGitRepository) {
    return (
      <div
        aria-label={t("Git branch")}
        className="composer-branch-select inline-flex h-8 max-w-full items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-400"
      >
        <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
        <span className="composer-select-label min-w-0 flex-1 truncate" />
      </div>
    );
  }

  function handleSelect(value: string, event: ReactMouseEvent<HTMLButtonElement>) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onChange(value);
  }

  return (
    <details
      className="composer-branch-select group relative max-w-full"
      ref={detailsRef}
    >
      <summary
        className={`composer-select-summary flex h-8 w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${
          disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
        }`}
        title={t("Git branch")}
      >
        <GitBranch aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="composer-select-label min-w-0 flex-1 truncate">
          {currentBranch}
        </span>
        {isLoading ? (
          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        ) : (
          <ChevronDown aria-hidden="true" className="size-3.5" />
        )}
      </summary>
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-52 overflow-y-auto py-1">
          {branches.length ? (
            branches.map((branch) => (
              <button
                aria-label={t("Switch to branch {name}", { name: branch })}
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${
                  branch === currentBranch
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                }`}
                key={branch}
                onClick={(event) => handleSelect(branch, event)}
                type="button"
              >
                <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{branch}</span>
                {branch === currentBranch ? (
                  <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                ) : null}
              </button>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">
              {t("No branches")}
            </div>
          )}
        </div>
        <div className="border-t border-stone-100 bg-white p-1.5">
          <button
            aria-label={t("Create git branch")}
            className="flex h-9 w-full items-center gap-2 rounded-lg px-2 text-sm font-semibold text-teal-800 hover:bg-teal-50"
            onClick={(event) => handleSelect(CREATE_BRANCH_OPTION_VALUE, event)}
            type="button"
          >
            <Plus aria-hidden="true" className="size-4" />
            <span className="min-w-0 flex-1 text-left">{t("New branch")}</span>
          </button>
        </div>
      </div>
    </details>
  );
}

function useCloseDetailsOnOutsidePointerDown() {
  const detailsRef = useRef<HTMLDetailsElement | null>(null);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      const details = detailsRef.current;
      if (!details?.open) {
        return;
      }
      const target = event.target;
      if (!(target instanceof Node) || details.contains(target)) {
        return;
      }
      details.removeAttribute("open");
    }

    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  return detailsRef;
}

function ReasoningBlock({
  durationMs,
  isStreaming,
  reasoning,
}: {
  durationMs: number | null;
  isStreaming: boolean;
  reasoning: string;
}) {
  const { language, t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(isStreaming);
  const preview = compactInlineText(reasoning);
  const durationLabel =
    durationMs === null ? null : formatNullableLatencySeconds(durationMs, language);
  const durationTitle = durationLabel
    ? t("Thinking duration {duration}", { duration: durationLabel })
    : null;

  useEffect(() => {
    setIsExpanded(isStreaming);
  }, [isStreaming]);

  const toggleLabel = isExpanded ? t("Collapse thinking") : t("Expand thinking");

  return (
    <div className="reasoning-block min-w-0 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
      <button
        aria-expanded={isExpanded}
        aria-label={toggleLabel}
        className="flex min-h-6 w-full min-w-0 items-center gap-2 text-left text-xs font-semibold text-stone-500 hover:text-stone-700"
        onClick={() => setIsExpanded((current) => !current)}
        title={toggleLabel}
        type="button"
      >
        {isExpanded ? (
          <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
        ) : (
          <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
        )}
        <span className="shrink-0">{t("Thinking")}</span>
        {isExpanded ? null : (
          <span
            className="min-w-0 flex-1 truncate font-normal text-stone-600"
            title={preview}
          >
            {preview}
          </span>
        )}
        {durationLabel && durationTitle ? (
          <span
            className="ml-auto shrink-0 tabular-nums text-[11px] font-semibold text-stone-500"
            title={durationTitle}
          >
            {durationLabel}
          </span>
        ) : null}
      </button>
      {isExpanded ? (
        <div className="mt-1.5">
          <MarkdownContent content={reasoning} isUser={false} variant="reasoning" />
        </div>
      ) : null}
    </div>
  );
}

function MessagePartBlock({
  isError,
  isStreaming,
  isStreamingTail,
  isUser,
  part,
  reasoningDurationMs,
}: {
  isError: boolean;
  isStreaming: boolean;
  isStreamingTail: boolean;
  isUser: boolean;
  part: ChatMessagePart;
  reasoningDurationMs: number | null;
}) {
  if (part.type === "reasoning") {
    return (
      <ReasoningBlock
        durationMs={reasoningDurationMs}
        isStreaming={isStreaming && isStreamingTail}
        reasoning={part.text}
      />
    );
  }

  if (part.type === "toolCall") {
    return <ToolCallBlock toolCall={part.toolCall} />;
  }

  if (part.type === "attachment") {
    return <AttachmentPartBlock attachment={part.attachment} isUser={isUser} />;
  }

  if (part.type === "error") {
    return <ErrorMessagePart text={part.text} />;
  }

  return (
    <MarkdownContent content={part.text} isError={isError} isUser={isUser} />
  );
}

function ErrorMessagePart({ text }: { text: string }) {
  return (
    <div className="whitespace-pre-wrap break-words rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm leading-6 text-rose-700">
      {text}
    </div>
  );
}

function ComposerAttachmentChip({
  attachment,
  onRemove,
}: {
  attachment: ComposerAttachment;
  onRemove: () => void;
}) {
  const { t } = useI18n();
  const title = attachment.path
    ? `${attachment.name} · ${attachment.path} · ${formatFileSize(attachment.sizeBytes)}`
    : `${attachment.name} · ${formatFileSize(attachment.sizeBytes)}`;

  return (
    <span
      className={`composer-attachment-chip ${
        attachment.previewDataUrl ? "composer-attachment-chip-image" : ""
      }`}
      title={title}
    >
      {attachment.previewDataUrl ? (
        <img alt={attachment.name} src={attachment.previewDataUrl} />
      ) : (
        <FileText aria-hidden="true" className="size-4 shrink-0" />
      )}
      <span className="min-w-0 truncate">{attachment.name}</span>
      <button
        aria-label={t("Remove attachment {name}", { name: attachment.name })}
        className="inline-flex size-5 shrink-0 items-center justify-center rounded-full text-stone-500 hover:bg-stone-200 hover:text-stone-900"
        onClick={onRemove}
        title={t("Remove attachment {name}", { name: attachment.name })}
        type="button"
      >
        <X aria-hidden="true" className="size-3" />
      </button>
    </span>
  );
}

function AttachmentPartBlock({
  attachment,
  isUser,
}: {
  attachment: ChatAttachmentPartSummary;
  isUser: boolean;
}) {
  const title = attachment.path
    ? `${attachment.name} · ${attachment.path} · ${formatFileSize(attachment.sizeBytes)}`
    : `${attachment.name} · ${formatFileSize(attachment.sizeBytes)}`;

  return (
    <div
      className={`message-attachment-part ${
        isUser ? "message-attachment-part-user" : ""
      }`}
      title={title}
    >
      {attachment.previewDataUrl ? (
        <img alt={attachment.name} src={attachment.previewDataUrl} />
      ) : (
        <span className="message-attachment-file-icon">
          <FileText aria-hidden="true" className="size-4" />
        </span>
      )}
      <span className="min-w-0 flex-1 truncate text-sm font-semibold">
        {attachment.name}
      </span>
      <span className="shrink-0 text-[11px] font-medium opacity-70">
        {formatFileSize(attachment.sizeBytes)}
      </span>
    </div>
  );
}

function ChatReplyMetricsLine({ metrics }: { metrics: ChatReplyMetrics }) {
  const { language, t } = useI18n();
  const values = [
    `${t("Model")}: ${metrics.modelId}`,
    `${t("Channel")}: ${metrics.providerId}`,
    `${t("Total time")}: ${formatNullableLatencySeconds(
      metrics.totalLatencyMs,
      language,
    )}`,
    `${t("tokens/s")}: ${formatTokensPerSecond(metrics, language)}`,
    `${t("First token latency")}: ${formatNullableLatencySeconds(
      metrics.firstTokenLatencyMs,
      language,
    )}`,
  ];

  return (
    <div className="flex flex-wrap gap-x-2 gap-y-1 border-t border-stone-100 pt-2 text-[11px] leading-4 text-stone-400">
      {values.map((value) => (
        <span className="min-w-0 break-words" key={value}>
          {value}
        </span>
      ))}
    </div>
  );
}

function MemoriesUsedBlock({ memories }: { memories: ChatMemoryUsedSummary[] }) {
  const { t } = useI18n();
  if (!memories.length) {
    return null;
  }

  return (
    <details className="rounded-lg border border-stone-100 bg-stone-50/70 px-3 py-2 text-xs text-stone-600">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-stone-600 marker:hidden">
        <Brain aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span>{t("Memories used")}</span>
        <span className="rounded-full bg-white px-1.5 py-0.5 text-[10px] text-stone-500">
          {memories.length}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {memories.map((memory) => (
          <div
            className="min-w-0 rounded-md border border-stone-100 bg-white px-2.5 py-2"
            key={`${memory.scope}-${memory.id}`}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[10px] font-semibold uppercase tracking-normal text-stone-400">
              <span>{memory.scope}</span>
              <span>{memory.kind}</span>
              <span>{memory.source}</span>
              {memory.pinned ? <span>{t("Pinned")}</span> : null}
            </div>
            <div className="mt-1 line-clamp-2 break-words text-xs leading-5 text-stone-700">
              {memory.fact}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}

function ExtractedMemoriesBlock({
  memories,
}: {
  memories: ChatExtractedMemorySummary[];
}) {
  const { t } = useI18n();
  if (!memories.length) {
    return null;
  }

  return (
    <details className="rounded-lg border border-stone-100 bg-stone-50/70 px-3 py-2 text-xs text-stone-600">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-stone-600 marker:hidden">
        <Brain aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span>{t("Memories saved")}</span>
        <span className="rounded-full bg-white px-1.5 py-0.5 text-[10px] text-stone-500">
          {memories.length}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {memories.map((memory) => (
          <div
            className="min-w-0 rounded-md border border-stone-100 bg-white px-2.5 py-2"
            key={`${memory.scope}-${memory.id}`}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[10px] font-semibold uppercase tracking-normal text-stone-400">
              <span>{memory.scope}</span>
              <span>{memory.kind}</span>
              <span>{memory.status}</span>
            </div>
            <div className="mt-1 line-clamp-2 break-words text-xs leading-5 text-stone-700">
              {memory.fact}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}

function MarkdownContent({
  content,
  isError = false,
  isUser,
  variant = "message",
}: {
  content: string;
  isError?: boolean;
  isUser: boolean;
  variant?: "message" | "reasoning";
}) {
  const skillPrefix = selectedSkillPrefix(content, isUser);
  const markdownContent = deferIncompleteMermaidBlocks(
    skillPrefix?.remaining ?? content,
  );

  return (
    <div
      className={`markdown-content min-w-0 break-words text-sm leading-6 ${
        isUser ? "markdown-content-user" : "markdown-content-assistant"
      } ${variant === "reasoning" ? "markdown-content-reasoning" : ""} ${
        isError ? "text-rose-700" : ""
      }`}
    >
      {skillPrefix ? (
        <div className="message-skill-chip-row">
          {skillPrefix.skills.map((skill) => (
            <span
              aria-label={skill.path}
              className="message-skill-chip"
              key={`${skill.name}-${skill.path}`}
              title={skill.path}
            >
              {skill.name}
            </span>
          ))}
        </div>
      ) : null}
      {markdownContent ? (
        <ReactMarkdown components={MARKDOWN_COMPONENTS} remarkPlugins={[remarkGfm]}>
          {markdownContent}
        </ReactMarkdown>
      ) : null}
    </div>
  );
}

function MermaidDiagram({ definition }: { definition: string }) {
  const { t } = useI18n();
  const reactId = useId();
  const baseRenderId = `foco-mermaid-${reactId.replaceAll(":", "")}`;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const renderCounterRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState("");

  useEffect(() => {
    let cancelled = false;
    renderCounterRef.current += 1;
    const renderId = `${baseRenderId}-${renderCounterRef.current}`;

    async function renderDiagram() {
      setError(null);
      setSvg("");

      try {
        const mermaid = await loadMermaidRuntime();
        if (cancelled) {
          return;
        }
        const result = await mermaid.render(renderId, definition);
        if (cancelled) {
          return;
        }
        setSvg(result.svg);
        window.setTimeout(() => {
          if (!cancelled && containerRef.current) {
            result.bindFunctions?.(containerRef.current);
          }
        }, 0);
      } catch (renderError) {
        if (!cancelled) {
          setError(errorMessage(renderError));
        }
      }
    }

    void renderDiagram();

    return () => {
      cancelled = true;
    };
  }, [definition, baseRenderId]);

  if (error !== null) {
    return (
      <div className="mermaid-diagram mermaid-diagram-error">
        <div className="mermaid-diagram-error-title">
          {t("Mermaid diagram failed to render.")}
        </div>
        <div className="mermaid-diagram-error-message">{error}</div>
        <pre>
          <code>{definition}</code>
        </pre>
      </div>
    );
  }

  return (
    <div
      aria-label="Mermaid diagram"
      className={`mermaid-diagram ${svg ? "" : "mermaid-diagram-loading"}`}
      dangerouslySetInnerHTML={svg ? { __html: svg } : undefined}
      ref={containerRef}
      role="img"
    />
  );
}

async function loadMermaidRuntime() {
  mermaidRuntimePromise ??= import("mermaid").then((module) => {
    module.default.initialize(MERMAID_CONFIG);
    return module.default;
  });

  return mermaidRuntimePromise;
}

function mermaidDefinitionFromPreChildren(children: ReactNode) {
  const childNodes = Children.toArray(children);
  if (childNodes.length !== 1) {
    return null;
  }

  const child = childNodes[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(child)) {
    return null;
  }

  const className = child.props.className ?? "";
  if (!/\blanguage-mermaid\b/i.test(className)) {
    return null;
  }

  const definition = Children.toArray(child.props.children).join("").trim();
  return definition ? definition : null;
}

function deferIncompleteMermaidBlocks(content: string) {
  const lines = content.match(/[^\r\n]*(?:\r\n|\n|\r|$)/g) ?? [];
  const nonEmptyLines = lines.filter((line) => line.length > 0);
  if (nonEmptyLines.length === 0) {
    return content;
  }

  let activeFence: MarkdownFence | null = null;
  for (let index = 0; index < nonEmptyLines.length; index += 1) {
    const line = nonEmptyLines[index];

    if (activeFence !== null) {
      if (isFenceClosingLine(line, activeFence)) {
        activeFence = null;
      }
      continue;
    }

    const fence = parseMarkdownFence(line);
    if (fence !== null) {
      activeFence = {
        ...fence,
        lineIndex: index,
      };
    }
  }

  if (activeFence?.language !== "mermaid") {
    return content;
  }

  const nextLines = [...nonEmptyLines];
  nextLines[activeFence.lineIndex] = neutralizeMermaidFenceLine(
    nextLines[activeFence.lineIndex],
  );
  return nextLines.join("");
}

type MarkdownFence = {
  char: "`" | "~";
  length: number;
  language: string | null;
  lineIndex: number;
};

function parseMarkdownFence(line: string) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return null;
  }

  const marker = match[2];
  const language = match[3].trim().split(/\s+/, 1)[0]?.toLowerCase() || null;
  return {
    char: marker[0] as "`" | "~",
    length: marker.length,
    language,
    lineIndex: -1,
  };
}

function isFenceClosingLine(line: string, fence: MarkdownFence) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const escapedChar = fence.char === "`" ? "`" : "~";
  return new RegExp(`^[ \\t]{0,3}${escapedChar}{${fence.length},}[ \\t]*$`).test(
    body,
  );
}

function neutralizeMermaidFenceLine(line: string) {
  const lineEnding = line.match(/(?:\r\n|\n|\r)$/)?.[0] ?? "";
  const body = line.slice(0, line.length - lineEnding.length);
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return line;
  }

  return `${match[1]}${match[2]}text${lineEnding}`;
}

function ToolCallBlock({ toolCall }: { toolCall: ChatToolCallSummary }) {
  const { t } = useI18n();
  const input = normalizedToolInput(toolCall.input);
  const detailText = toolCallDetailText(toolCall);

  return (
    <details className="tool-call-block group min-w-0">
      <summary className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-stone-700 marker:hidden">
        <Wrench aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="min-w-0 shrink-0 truncate">{toolCall.name}</span>
        {detailText ? (
          <span className="shrink-0 text-stone-300">·</span>
        ) : null}
        {detailText ? (
          <span
            className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-stone-500"
            title={detailText}
          >
            {detailText}
          </span>
        ) : null}
        <span
          className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-4 ${
            toolCall.isError
              ? "bg-rose-50 text-rose-700"
              : toolCall.status === "completed"
                ? "bg-emerald-50 text-emerald-700"
              : "bg-stone-100 text-stone-600"
          }`}
        >
          {toolStatusText(toolCall, t)}
        </span>
      </summary>
      <div className="mt-2 grid gap-2 text-xs text-stone-600">
        <div className="min-w-0">
          <div className="mb-1 font-semibold text-stone-500">{t("Input")}</div>
          <pre className="panel-scroll max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-stone-200 pl-3 font-mono text-[11px] leading-5">
            {formatJsonValue(input)}
          </pre>
        </div>
        {toolCall.output !== null ? (
          <div className="min-w-0">
            <div className="mb-1 font-semibold text-stone-500">{t("Output")}</div>
            <pre
              className={`panel-scroll max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${
                toolCall.isError
                  ? "border-rose-200 text-rose-700"
                  : "border-stone-200"
              }`}
            >
              {formatJsonValue(toolCall.output)}
            </pre>
          </div>
        ) : null}
      </div>
    </details>
  );
}

function TerminalCommandButton({
  commands,
  disabled,
  onRun,
}: {
  commands: WorkspaceCommonCommandSummary[];
  disabled: boolean;
  onRun: (command: WorkspaceCommonCommandSummary) => void;
}) {
  const { t } = useI18n();
  const detailsRef = useCloseDetailsOnOutsidePointerDown();

  if (!commands.length) {
    return null;
  }

  if (commands.length === 1) {
    const command = commands[0];
    return (
      <button
        aria-label={t("Run common command {name}", { name: command.name })}
        className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-stone-800 hover:text-stone-100 disabled:cursor-not-allowed disabled:text-stone-600 disabled:hover:bg-transparent"
        disabled={disabled}
        onClick={() => onRun(command)}
        title={t("Run common command {name}", { name: command.name })}
        type="button"
      >
        <Play aria-hidden="true" className="size-3.5 fill-current" />
      </button>
    );
  }

  function handleSelect(
    command: WorkspaceCommonCommandSummary,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onRun(command);
  }

  return (
    <details className="relative" ref={detailsRef}>
      <summary
        aria-disabled={disabled}
        aria-label={t("Run common command")}
        className={`inline-flex size-6 cursor-pointer list-none items-center justify-center rounded-md text-stone-400 outline-none marker:hidden focus-visible:ring-2 focus-visible:ring-teal-500/60 [&::-webkit-details-marker]:hidden ${
          disabled
            ? "pointer-events-none text-stone-600"
            : "hover:bg-stone-800 hover:text-stone-100"
        }`}
        title={t("Run common command")}
      >
        <Play aria-hidden="true" className="size-3.5 fill-current" />
      </summary>
      <div className="absolute right-0 top-full z-30 mt-2 w-56 overflow-hidden rounded-lg border border-stone-800 bg-stone-950 shadow-[0_18px_40px_rgba(0,0,0,0.28)]">
        <div className="panel-scroll max-h-56 overflow-y-auto py-1">
          {commands.map((command, index) => (
            <button
              aria-label={t("Run common command {name}", { name: command.name })}
              className="flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-xs font-semibold text-stone-200 hover:bg-stone-800"
              key={`${command.name}-${index}`}
              onClick={(event) => handleSelect(command, event)}
              type="button"
            >
              <Play
                aria-hidden="true"
                className="size-3.5 shrink-0 fill-current text-teal-400"
              />
              <span className="min-w-0 flex-1 truncate">{command.name}</span>
            </button>
          ))}
        </div>
      </div>
    </details>
  );
}

function TerminalPanel({
  onClose,
  workspace,
}: {
  onClose: () => void;
  workspace: WorkspaceSummary | undefined;
}) {
  const { t } = useI18n();
  const [panelHeight, setPanelHeight] = useState(256);
  const [isResizing, setIsResizing] = useState(false);
  const [activeClientId, setActiveClientId] = useState("");
  const [sessions, setSessions] = useState<TerminalPanelSession[]>(() => [
    createTerminalPanelSession(1),
  ]);
  const nextSessionNumberRef = useRef(2);
  const previousWorkspaceIdRef = useRef(workspace?.id ?? "");
  const workspaceId = workspace?.id ?? "";
  const workspacePath = workspace?.path ?? "";
  const commonCommands = workspace?.commonCommands ?? [];
  const activeSession =
    sessions.find((session) => session.clientId === activeClientId) ??
    sessions[0] ??
    null;

  useEffect(() => {
    if (previousWorkspaceIdRef.current === workspaceId) {
      return;
    }

    previousWorkspaceIdRef.current = workspaceId;
    const initialSession = createTerminalPanelSession(1);
    nextSessionNumberRef.current = 2;
    setSessions([initialSession]);
    setActiveClientId(initialSession.clientId);
  }, [workspaceId]);

  useEffect(() => {
    if (activeClientId || sessions.length === 0) {
      return;
    }

    setActiveClientId(sessions[0].clientId);
  }, [activeClientId, sessions]);

  useEffect(() => {
    if (!isResizing) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const nextHeight = window.innerHeight - event.clientY;
      setPanelHeight(Math.min(Math.max(nextHeight, 180), 520));
    }

    function handlePointerUp() {
      setIsResizing(false);
    }

    document.body.style.cursor = "row-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizing]);

  const updateSession = useCallback(
    (clientId: string, patch: Partial<Omit<TerminalPanelSession, "clientId">>) => {
      setSessions((current) =>
        current.map((session) =>
          session.clientId === clientId ? { ...session, ...patch } : session,
        ),
      );
    },
    [],
  );

  const markSessionClosed = useCallback((clientId: string) => {
    setSessions((current) =>
      current.map((session) =>
        session.clientId === clientId
          ? {
              ...session,
              status: session.status === "error" ? "error" : "closed",
            }
          : session,
      ),
    );
  }, []);

  function createSession() {
    const session = createTerminalPanelSession(nextSessionNumberRef.current);
    nextSessionNumberRef.current += 1;
    setSessions((current) => [...current, session]);
    setActiveClientId(session.clientId);
  }

  function closeSession(clientId: string) {
    if (sessions.length <= 1) {
      return;
    }

    const next = sessions.filter((session) => session.clientId !== clientId);
    setSessions(next);
    if (clientId === activeClientId) {
      setActiveClientId(next[0]?.clientId ?? "");
    }
  }

  function runWorkspaceCommonCommand(command: WorkspaceCommonCommandSummary) {
    if (!activeSession || !workspace) {
      return;
    }

    updateSession(activeSession.clientId, {
      pendingCommand: {
        input: terminalCommandInput(
          workspace.terminalShell,
          workspace.path,
          command.command,
        ),
      },
    });
  }

  return (
    <section
      className="terminal-panel relative shrink-0 border-t border-stone-800 bg-[#16130f]"
      style={{ "--terminal-panel-height": `${panelHeight}px` } as CSSProperties}
    >
      <div
        aria-label={t("Resize terminal panel")}
        aria-orientation="horizontal"
        className="absolute left-0 right-0 top-0 z-10 h-1 cursor-row-resize bg-transparent hover:bg-teal-500/50"
        onKeyDown={(event) => {
          if (event.key === "ArrowUp") {
            event.preventDefault();
            setPanelHeight((current) => Math.min(current + 24, 520));
          }

          if (event.key === "ArrowDown") {
            event.preventDefault();
            setPanelHeight((current) => Math.max(current - 24, 180));
          }
        }}
        onPointerDown={(event) => {
          event.preventDefault();
          setIsResizing(true);
        }}
        role="separator"
        tabIndex={0}
      />
      <div className="terminal-panel-body mx-auto flex h-[var(--terminal-panel-height)] w-full max-w-5xl min-w-0">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-8 items-center justify-between gap-3 px-3 text-xs text-stone-400">
            <span className="inline-flex min-w-0 items-center gap-2">
              <Terminal aria-hidden="true" className="size-4 shrink-0" />
              <span className={terminalStatusClass(activeSession?.status ?? "closed")}>
                {terminalStatusText(activeSession?.status ?? "closed", t)}
              </span>
              <span className="min-w-0 truncate">
                {activeSession?.cwd || workspacePath}
              </span>
            </span>
            <span className="flex min-w-0 shrink-0 items-center gap-2">
              {activeSession?.error ? (
                <span className="min-w-0 truncate text-rose-300">
                  {activeSession.error}
                </span>
              ) : null}
              <TerminalCommandButton
                commands={commonCommands}
                disabled={!activeSession || !isTerminalConnected(activeSession.status)}
                onRun={runWorkspaceCommonCommand}
              />
              <button
                aria-label={t("New terminal")}
                className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-stone-800 hover:text-stone-100"
                onClick={createSession}
                title={t("New terminal")}
                type="button"
              >
                <Plus aria-hidden="true" className="size-3.5" />
              </button>
              <button
                aria-label={t("Close terminal")}
                className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-rose-950/60 hover:text-rose-200"
                onClick={onClose}
                title={t("Close terminal")}
                type="button"
              >
                <X aria-hidden="true" className="size-3.5" />
              </button>
            </span>
          </div>
          <div className="relative min-h-0 flex-1">
            {sessions.map((session) => (
              <TerminalSessionPane
                isActive={session.clientId === activeSession?.clientId}
                key={session.clientId}
                markClosed={markSessionClosed}
                onUpdate={updateSession}
                session={session}
                workspaceId={workspaceId}
              />
            ))}
          </div>
        </div>
        {sessions.length > 1 ? (
          <aside
            aria-label={t("Terminal sessions")}
            className="terminal-session-list panel-scroll w-44 shrink-0 overflow-y-auto border-l border-stone-800 bg-stone-950/35 px-2 py-2"
          >
            {sessions.map((session) => (
              <div
                className={`flex w-full min-w-0 items-center gap-1 rounded-md text-xs ${
                  session.clientId === activeSession?.clientId
                    ? "bg-stone-800 text-stone-100"
                    : "text-stone-400 hover:bg-stone-900 hover:text-stone-100"
                }`}
                key={session.clientId}
              >
                <button
                  className="flex min-w-0 flex-1 items-center gap-2 px-2 py-2 text-left"
                  onClick={() => setActiveClientId(session.clientId)}
                  type="button"
                >
                  <span
                    aria-label={terminalStatusText(session.status, t)}
                    className={`size-2 shrink-0 rounded-full ${
                      isTerminalConnected(session.status)
                        ? "bg-emerald-400"
                        : "bg-rose-500"
                    }`}
                    title={terminalStatusText(session.status, t)}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-semibold">
                      {t("Terminal {number}", { number: session.number })}
                    </span>
                    <span
                      className="block truncate text-[11px] opacity-60"
                      title={session.cwd || workspacePath}
                    >
                      {session.cwd || workspacePath}
                    </span>
                  </span>
                </button>
                <button
                  aria-label={t("Close terminal {number}", {
                    number: session.number,
                  })}
                  className="mr-1 inline-flex size-6 shrink-0 items-center justify-center rounded-md text-stone-500 hover:bg-rose-950/60 hover:text-rose-200"
                  onClick={() => closeSession(session.clientId)}
                  title={t("Close terminal {number}", { number: session.number })}
                  type="button"
                >
                  <X aria-hidden="true" className="size-3.5" />
                </button>
              </div>
            ))}
          </aside>
        ) : null}
      </div>
    </section>
  );
}

function TerminalSessionPane({
  isActive,
  markClosed,
  onUpdate,
  session,
  workspaceId,
}: {
  isActive: boolean;
  markClosed: (clientId: string) => void;
  onUpdate: (
    clientId: string,
    patch: Partial<Omit<TerminalPanelSession, "clientId">>,
  ) => void;
  session: TerminalPanelSession;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const tRef = useRef(t);
  const isActiveRef = useRef(isActive);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const { clientId } = session;

  useEffect(() => {
    tRef.current = t;
  }, [t]);

  useEffect(() => {
    isActiveRef.current = isActive;
    if (isActive) {
      xtermRef.current?.focus();
    }
  }, [isActive]);

  useEffect(() => {
    if (!workspaceId) {
      return;
    }

    let cancelled = false;
    const terminal = new XTerm({
      allowProposedApi: false,
      convertEol: true,
      cursorBlink: true,
      fontFamily: "Cascadia Mono, Consolas, monospace",
      fontSize: 13,
      rows: 12,
      theme: {
        background: "#16130f",
        foreground: "#f7f3ea",
        cursor: "#14b8a6",
      },
    });
    const fitAddon = new FitAddon();
    let socket: WebSocket | null = null;

    xtermRef.current = terminal;
    fitAddonRef.current = fitAddon;
    terminal.loadAddon(fitAddon);
    onUpdate(clientId, { error: null, status: "connecting" });

    if (!containerRef.current) {
      onUpdate(clientId, {
        error: tRef.current("Terminal container was not mounted."),
        status: "error",
      });
      terminal.dispose();
      return;
    }

    terminal.open(containerRef.current);
    fitAddon.fit();

    const sendResize = () => {
      if (socket?.readyState !== WebSocket.OPEN) {
        return;
      }

      socket.send(
        JSON.stringify({
          type: "resize",
          cols: terminal.cols,
          rows: terminal.rows,
        }),
      );
    };

    const observer = new ResizeObserver(() => {
      fitAddon.fit();
      sendResize();
    });
    observer.observe(containerRef.current);
    resizeObserverRef.current = observer;

    const inputDisposable = terminal.onData((data) => {
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "input", data }));
      }
    });

    async function connectTerminal() {
      if (!workspaceId) {
        return;
      }

      try {
        const serverSession = await requestJson<TerminalSessionResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/terminal/session`,
          { method: "POST" },
        );
        if (cancelled) {
          return;
        }

        onUpdate(clientId, {
          cwd: serverSession.workingDirectory,
          serverSessionId: serverSession.id,
        });
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        socket = new WebSocket(
          `${protocol}//${window.location.host}/api/workspaces/${encodeURIComponent(
            workspaceId,
          )}/terminal/${encodeURIComponent(serverSession.id)}/ws?cols=${terminal.cols}&rows=${terminal.rows}`,
        );
        socketRef.current = socket;

        socket.onopen = () => {
          onUpdate(clientId, { status: "connected" });
          sendResize();
          if (isActiveRef.current) {
            terminal.focus();
          }
        };
        socket.onmessage = (event) => {
          const parsed = JSON.parse(event.data as string) as unknown;
          if (!isTerminalServerEvent(parsed)) {
            onUpdate(clientId, {
              error: tRef.current("Terminal returned an unknown event."),
              status: "error",
            });
            return;
          }

          if (parsed.type === "started" || parsed.type === "cwd") {
            onUpdate(clientId, { cwd: parsed.cwd });
            return;
          }

          if (parsed.type === "output") {
            terminal.write(parsed.data);
            return;
          }

          if (parsed.type === "exit") {
            onUpdate(clientId, { status: "closed" });
            terminal.writeln(
              `\r\n[${tRef.current("terminal exited: {status}", {
                status: parsed.status,
              })}]`,
            );
            return;
          }

          onUpdate(clientId, { error: parsed.message, status: "error" });
          terminal.writeln(
            `\r\n[${tRef.current("terminal error: {message}", {
              message: parsed.message,
            })}]`,
          );
        };
        socket.onerror = () => {
          onUpdate(clientId, {
            error: tRef.current("Terminal WebSocket failed."),
            status: "error",
          });
        };
        socket.onclose = () => {
          markClosed(clientId);
        };
      } catch (requestError) {
        if (!cancelled) {
          const message = errorMessage(requestError);
          onUpdate(clientId, { error: message, status: "error" });
          terminal.writeln(
            `[${tRef.current("terminal error: {message}", { message })}]`,
          );
        }
      }
    }

    void connectTerminal();

    return () => {
      cancelled = true;
      inputDisposable.dispose();
      observer.disconnect();
      socket?.close();
      terminal.dispose();
      socketRef.current = null;
      xtermRef.current = null;
      fitAddonRef.current = null;
      resizeObserverRef.current = null;
    };
  }, [clientId, markClosed, onUpdate, workspaceId]);

  useEffect(() => {
    const pendingCommand = session.pendingCommand;
    if (!pendingCommand) {
      return;
    }

    const socket = socketRef.current;
    if (socket?.readyState !== WebSocket.OPEN) {
      onUpdate(clientId, {
        error: tRef.current("Terminal is not connected."),
        pendingCommand: null,
      });
      return;
    }

    socket.send(JSON.stringify({ type: "input", data: pendingCommand.input }));
    onUpdate(clientId, { pendingCommand: null });
  }, [clientId, onUpdate, session.pendingCommand]);

  return (
    <div
      aria-hidden={!isActive}
      className={`terminal-session-pane absolute inset-0 min-h-0 min-w-0 p-2 ${
        isActive ? "" : "pointer-events-none opacity-0"
      }`}
    >
      <div ref={containerRef} className="terminal-xterm h-full min-w-0" />
    </div>
  );
}

function createTerminalPanelSession(number: number): TerminalPanelSession {
  return {
    clientId: `${Date.now().toString(36)}-${number}-${Math.random()
      .toString(36)
      .slice(2)}`,
    cwd: "",
    error: null,
    number,
    pendingCommand: null,
    serverSessionId: null,
    status: "closed",
  };
}

function terminalCommandInput(
  terminalShell: string,
  workspacePath: string,
  command: string,
) {
  const commandInput =
    command.endsWith("\n") || command.endsWith("\r") ? command : `${command}\r`;
  return `${terminalCdCommand(terminalShell, workspacePath)}\r${commandInput}`;
}

function terminalCdCommand(terminalShell: string, workspacePath: string) {
  if (terminalShell === "powershell") {
    return `Set-Location -LiteralPath '${quotePowerShellSingle(workspacePath)}'`;
  }

  if (terminalShell === "cmd") {
    return `cd /d "${workspacePath.replaceAll('"', '""')}"`;
  }

  return `cd -- '${quotePosixSingle(workspacePath)}'`;
}

function quotePowerShellSingle(value: string) {
  return value.replaceAll("'", "''");
}

function quotePosixSingle(value: string) {
  return value.replaceAll("'", "'\\''");
}

type ChartDatum = {
  displayValue?: string;
  id: string;
  label: string;
  value: number;
};

function ApiOverviewPanel({
  settings,
  workspaces,
}: {
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const [filters, setFilters] = useState({
    startedAfter: "",
    startedBefore: "",
    workspaceId: "",
  });
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const summary = stats?.summary ?? emptyAiStatisticsSummary();
  const providerLabels = useMemo(
    () =>
      new Map(
        (settings?.providers ?? []).map((provider) => [
          provider.id,
          provider.name,
        ]),
      ),
    [settings?.providers],
  );
  const modelLabels = useMemo(
    () =>
      new Map(
        (settings?.configuredModels ?? []).map((model) => [
          model.id,
          model.displayName,
        ]),
      ),
    [settings?.configuredModels],
  );
  const selectedWorkspace =
    workspaces.find((workspace) => workspace.id === filters.workspaceId) ?? null;
  const requestTrendData = summary.trend.map((point) => ({
    id: point.bucket,
    label: formatTrendBucket(point.bucket, language),
    value: point.requestCount,
  }));
  const tokenTrendData = summary.trend.map((point) => ({
    id: point.bucket,
    label: formatTrendBucket(point.bucket, language),
    value: point.totalTokens,
  }));
  const modelTokenData = summary.modelBreakdown.map((item) => ({
    id: item.modelId,
    label: modelLabels.get(item.modelId) ?? item.modelId,
    value: item.totalTokens,
  }));
  const modelRequestData = summary.modelBreakdown.map((item) => ({
    id: item.modelId,
    label: modelLabels.get(item.modelId) ?? item.modelId,
    value: item.requestCount,
  }));
  const providerTokenData = summary.providerBreakdown.map((item) => ({
    id: item.providerId,
    label: providerLabels.get(item.providerId) ?? item.providerId,
    value: item.totalTokens,
  }));
  const providerRequestData = summary.providerBreakdown.map((item) => ({
    id: item.providerId,
    label: providerLabels.get(item.providerId) ?? item.providerId,
    value: item.requestCount,
  }));
  const providerSuccessData = summary.providerBreakdown.map((item) => ({
    displayValue: formatPercent(item.successRate, language),
    id: item.providerId,
    label: providerLabels.get(item.providerId) ?? item.providerId,
    value: item.successRate ?? 0,
  }));
  const providerLatencyData = summary.providerBreakdown.map((item) => ({
    displayValue: formatNullableLatencySeconds(item.averageLatencyMs, language),
    id: item.providerId,
    label: providerLabels.get(item.providerId) ?? item.providerId,
    value: item.averageLatencyMs ?? 0,
  }));

  const loadOverview = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const query = aiOverviewQuery(filters);
      const data = await requestJson<AiStatisticsResponse>(
        `/api/ai-statistics${query ? `?${query}` : ""}`,
      );
      setStats(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  function updateOverviewFilters(update: Partial<typeof filters>) {
    setFilters((current) => ({
      ...current,
      ...update,
    }));
  }

  return (
    <div className="api-overview-panel flex w-full flex-col gap-4">
      <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="inline-flex size-10 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
              <BarChart3 aria-hidden="true" className="size-5" />
            </span>
            <div className="min-w-0">
              <h2 className="truncate text-lg font-semibold text-stone-950">
                {t("API overview")}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {selectedWorkspace?.name ?? t("All workspaces")}
              </p>
            </div>
          </div>
          <button
            aria-label={t("Refresh request audit")}
            className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
            disabled={isLoading}
            onClick={() => void loadOverview()}
            title={t("Refresh request audit")}
            type="button"
          >
            {isLoading ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <RefreshCw aria-hidden="true" className="size-4" />
            )}
          </button>
        </div>
        <div className="mt-4 grid gap-3 md:grid-cols-3">
          <FilterSelect
            label={t("Workspace")}
            onChange={(value) => updateOverviewFilters({ workspaceId: value })}
            options={workspaces.map((workspace) => ({
              label: workspace.name,
              value: workspace.id,
            }))}
            placeholder={t("All workspaces")}
            value={filters.workspaceId}
          />
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Started after")}
            </span>
            <input
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) =>
                updateOverviewFilters({ startedAfter: event.target.value })
              }
              type="datetime-local"
              value={filters.startedAfter}
            />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Started before")}
            </span>
            <input
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) =>
                updateOverviewFilters({ startedBefore: event.target.value })
              }
              type="datetime-local"
              value={filters.startedBefore}
            />
          </label>
        </div>
      </section>

      {error ? (
        <div className="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {error}
        </div>
      ) : null}

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <StatsCard
          icon={Activity}
          label={t("Total requests")}
          value={formatNumber(summary.totalRequests, language)}
        />
        <StatsCard
          icon={Bot}
          label={t("Total tokens")}
          value={formatCompactNumber(summary.totalTokens, language)}
        />
        <StatsCard
          icon={SlidersHorizontal}
          label={t("Average latency")}
          value={formatNullableLatencySeconds(summary.averageLatencyMs, language)}
        />
        <StatsCard
          icon={CircleAlert}
          label={t("Failed requests")}
          value={formatNumber(summary.failedRequests, language)}
        />
      </section>

      {summary.totalRequests === 0 ? (
        <section className="rounded-2xl border border-dashed border-stone-300 bg-white/65 px-4 py-8 text-center text-sm font-medium text-stone-500">
          {isLoading ? t("Loading...") : t("No statistics yet")}
        </section>
      ) : (
        <section className="grid gap-4 xl:grid-cols-2">
          <LineChartCard
            data={requestTrendData}
            title={t("Request trend")}
            valueFormatter={(value) => formatNumber(value, language)}
          />
          <LineChartCard
            data={tokenTrendData}
            title={t("Token trend")}
            valueFormatter={(value) => formatCompactNumber(value, language)}
          />
          <DonutChartCard
            data={modelTokenData}
            title={t("Tokens by model")}
            valueFormatter={(value) => formatCompactNumber(value, language)}
          />
          <DonutChartCard
            data={modelRequestData}
            title={t("Requests by model")}
            valueFormatter={(value) => formatNumber(value, language)}
          />
          <BarChartCard
            data={providerTokenData}
            title={t("Tokens by channel")}
            valueFormatter={(value) => formatCompactNumber(value, language)}
          />
          <DonutChartCard
            data={providerRequestData}
            title={t("Requests by channel")}
            valueFormatter={(value) => formatNumber(value, language)}
          />
          <BarChartCard
            data={providerSuccessData}
            maxValue={1}
            title={t("Channel success rate")}
            valueFormatter={(value) => formatPercent(value, language)}
          />
          <BarChartCard
            data={providerLatencyData}
            title={t("Channel response time")}
            valueFormatter={(value) => formatNullableLatencySeconds(value, language)}
          />
        </section>
      )}
    </div>
  );
}

function LineChartCard({
  data,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.slice(-12);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {chartData.length ? (
        <div className="mt-3 h-52 w-full">
          <ResponsiveContainer
            height="100%"
            initialDimension={{ height: 208, width: 720 }}
            width="100%"
          >
            <LineChart
              data={chartData}
              margin={{ bottom: 4, left: 0, right: 12, top: 10 }}
            >
              <CartesianGrid stroke="#f5f5f4" vertical={false} />
              <XAxis
                axisLine={false}
                dataKey="label"
                minTickGap={18}
                tick={{ fill: "#78716c", fontSize: 12 }}
                tickLine={false}
              />
              <YAxis
                axisLine={false}
                tick={{ fill: "#78716c", fontSize: 12 }}
                tickFormatter={compactChartTick}
                tickLine={false}
                width={46}
              />
              <Tooltip
                contentStyle={chartTooltipStyle}
                cursor={{ stroke: "#99f6e4", strokeWidth: 1 }}
                formatter={(value) => [valueFormatter(Number(value)), title]}
                labelStyle={chartTooltipLabelStyle}
              />
              <Line
                activeDot={{ r: 6, stroke: "#0f766e", strokeWidth: 2 }}
                dataKey="value"
                dot={{
                  fill: "#ffffff",
                  r: 3,
                  stroke: "#0f766e",
                  strokeWidth: 2,
                }}
                isAnimationActive
                name={title}
                stroke="#0f766e"
                strokeWidth={2.5}
                type="monotone"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

function DonutChartCard({
  data,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.filter((item) => item.value > 0).slice(0, 6);
  const total = chartData.reduce((sum, item) => sum + item.value, 0);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {total > 0 ? (
        <div className="mt-4 grid gap-4 sm:grid-cols-[12rem_1fr] sm:items-center">
          <div className="relative h-48 w-full min-w-0">
            <ResponsiveContainer
              height="100%"
              initialDimension={{ height: 192, width: 192 }}
              width="100%"
            >
              <PieChart>
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  formatter={(value, _name, item) => [
                    valueFormatter(Number(value)),
                    chartPayloadLabel(item.payload),
                  ]}
                  labelStyle={chartTooltipLabelStyle}
                />
                <Pie
                  animationDuration={450}
                  data={chartData}
                  dataKey="value"
                  innerRadius="58%"
                  nameKey="label"
                  outerRadius="82%"
                  paddingAngle={2}
                >
                  {chartData.map((item, index) => (
                    <Cell fill={chartColor(index)} key={item.id} />
                  ))}
                </Pie>
              </PieChart>
            </ResponsiveContainer>
            <div className="pointer-events-none absolute inset-0 grid place-items-center">
              <div className="rounded-full bg-white/80 px-2 py-1 text-center font-mono text-sm font-semibold text-stone-950 shadow-sm">
                {valueFormatter(total)}
              </div>
            </div>
          </div>
          <ChartLegend data={chartData} valueFormatter={valueFormatter} />
        </div>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

function BarChartCard({
  data,
  maxValue,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  maxValue?: number;
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.filter((item) => item.value > 0).slice(0, 8);
  const chartMax = Math.max(maxValue ?? 0, ...chartData.map((item) => item.value), 1);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {chartData.length ? (
        <>
          <div className="mt-4 h-64 w-full">
            <ResponsiveContainer
              height="100%"
              initialDimension={{ height: 256, width: 720 }}
              width="100%"
            >
              <BarChart
                data={chartData}
                layout="vertical"
                margin={{ bottom: 4, left: 6, right: 18, top: 4 }}
              >
                <CartesianGrid horizontal={false} stroke="#f5f5f4" />
                <XAxis domain={[0, chartMax]} hide type="number" />
                <YAxis
                  axisLine={false}
                  dataKey="label"
                  tick={{ fill: "#78716c", fontSize: 12 }}
                  tickFormatter={compactChartLabel}
                  tickLine={false}
                  type="category"
                  width={112}
                />
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  cursor={{ fill: "#f0fdfa" }}
                  formatter={(value, _name, item) => [
                    chartPayloadDisplayValue(
                      item.payload,
                      valueFormatter,
                      value,
                    ),
                    chartPayloadLabel(item.payload),
                  ]}
                  labelStyle={chartTooltipLabelStyle}
                />
                <Bar
                  animationDuration={450}
                  barSize={16}
                  dataKey="value"
                  radius={[0, 8, 8, 0]}
                >
                  {chartData.map((item, index) => (
                    <Cell fill={chartColor(index)} key={item.id} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
          <ChartLegend data={chartData} valueFormatter={valueFormatter} />
        </>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

function ChartLegend({
  data,
  valueFormatter,
}: {
  data: ChartDatum[];
  valueFormatter: (value: number) => string;
}) {
  return (
    <div className="grid gap-2">
      {data.map((item, index) => (
        <div className="flex min-w-0 items-center gap-2 text-xs" key={item.id}>
          <span
            aria-hidden="true"
            className="size-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: chartColor(index) }}
          />
          <span className="min-w-0 flex-1 truncate font-medium text-stone-600">
            {item.label}
          </span>
          <span className="shrink-0 font-mono text-stone-950">
            {item.displayValue ?? valueFormatter(item.value)}
          </span>
        </div>
      ))}
    </div>
  );
}

function ChartEmptyState({ label }: { label: string }) {
  return (
    <div className="mt-4 grid h-44 place-items-center rounded-xl border border-dashed border-stone-300 bg-stone-50/70 text-sm font-medium text-stone-500">
      {label}
    </div>
  );
}

function ApiStatsPanel({
  settings,
  workspaces,
}: {
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const [filters, setFilters] = useState<AiStatsFilterState>(
    emptyAiStatsFilters,
  );
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [detail, setDetail] = useState<AiRequestDetailResponse | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [visibleColumnIds, setVisibleColumnIds] = useState<
    Set<AiStatsColumnId>
  >(readAiStatsVisibleColumnIds);
  const requests = stats?.requests ?? [];
  const summary = stats?.summary ?? emptyAiStatisticsSummary();
  const totalCount = stats?.totalCount ?? summary.totalRequests;
  const currentPage = stats?.page ?? positiveIntegerText(filters.page, 1);
  const pageSize = stats?.pageSize ?? positiveIntegerText(filters.pageSize, 20);
  const totalPages =
    stats?.totalPages ?? (totalCount ? Math.ceil(totalCount / pageSize) : 0);
  const paginationItems = auditPaginationItems(currentPage, totalPages);
  const pageStart = requests.length ? (currentPage - 1) * pageSize + 1 : 0;
  const pageEnd = requests.length
    ? Math.min(totalCount, pageStart + requests.length - 1)
    : 0;
  const selectedWorkspace =
    workspaces.find((workspace) => workspace.id === filters.workspaceId) ?? null;
  const chatOptions = (selectedWorkspace ? [selectedWorkspace] : workspaces)
    .flatMap((workspace) =>
      workspace.chats.map((chat) => ({
        label: selectedWorkspace
          ? chat.title
          : `${workspace.name} / ${chat.title}`,
        value: chat.id,
      })),
    );
  const providerOptions = auditOptions(
    settings?.providers.map((provider) => ({
      label: provider.name,
      value: provider.id,
    })) ?? [],
    requests.map((request) => request.providerId),
  );
  const modelOptions = auditOptions(
    settings?.configuredModels.map((model) => ({
      label: model.displayName,
      value: model.id,
    })) ?? [],
    requests.map((request) => request.modelId),
  );
  const statusOptions = auditOptions(
    ["succeeded", "failed", "completed"].map((status) => ({
      label: auditStatusText(status, t),
      value: status,
    })),
    requests.map((request) => request.finalState),
    (status) => auditStatusText(status, t),
  );
  const totalInputTokens = summary.totalInputTokens;
  const totalOutputTokens = summary.totalOutputTokens;
  const aiStatsColumns: AiStatsColumn[] = [
    {
      cellClassName: "px-4 py-3 font-medium text-stone-900",
      id: "requestTime",
      label: t("Request time"),
      render: (request) =>
        formatAuditDate(request.requestStartedAt, language),
    },
    {
      cellClassName: "max-w-[10rem] truncate px-4 py-3 text-stone-700",
      id: "workspace",
      label: t("Workspace"),
      render: (request) => request.workspaceName,
    },
    {
      cellClassName: "max-w-[12rem] truncate px-4 py-3 text-stone-600",
      id: "chat",
      label: t("Chat"),
      render: (request) => request.chatTitle ?? request.chatId ?? "n/a",
    },
    {
      cellClassName:
        "max-w-[12rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "provider",
      label: t("Provider"),
      render: (request) => request.providerId,
    },
    {
      cellClassName:
        "max-w-[14rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "model",
      label: t("Model"),
      render: (request) => request.modelId,
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "inputTokens",
      label: t("Input tokens"),
      render: (request) =>
        formatNullableCompactNumber(request.inputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "outputTokens",
      label: t("Output tokens"),
      render: (request) =>
        formatNullableCompactNumber(request.outputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRead",
      label: t("Cache read"),
      render: (request) =>
        formatNullableCompactNumber(request.cacheReadTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheWrite",
      label: t("Cache write"),
      render: (request) =>
        formatNullableCompactNumber(request.cacheWriteTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRatio",
      label: t("Cache ratio"),
      render: (request) => formatPercent(request.cacheRatio, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "latency",
      label: t("Latency"),
      render: (request) =>
        formatNullableLatencySeconds(request.totalLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "firstToken",
      label: t("First token"),
      render: (request) =>
        formatNullableLatencySeconds(request.firstTokenLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "statusCode",
      label: t("Status code"),
      render: (request) => formatNullableNumber(request.statusCode, language),
    },
    {
      cellClassName: "px-4 py-3",
      id: "status",
      label: t("Status"),
      render: (request) => (
        <span className={auditStatusClass(request.finalState)}>
          {auditStatusText(request.finalState, t)}
        </span>
      ),
    },
    {
      cellClassName: "px-4 py-3 text-right",
      headerClassName: "text-right",
      id: "details",
      label: t("Details"),
      render: (request) => (
        <button
          aria-label={t("View request details")}
          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
          onClick={() => void openRequestDetail(request)}
          title={t("View request details")}
          type="button"
        >
          <Eye aria-hidden="true" className="size-4" />
        </button>
      ),
    },
  ];
  const visibleColumns = aiStatsColumns.filter((column) =>
    visibleColumnIds.has(column.id),
  );

  function updateAuditFilters(update: Partial<AiStatsFilterState>) {
    setFilters((current) => ({
      ...current,
      ...update,
      page: "1",
    }));
  }

  function goToAuditPage(page: number) {
    const maxPage = Math.max(1, totalPages);
    setFilters((current) => ({
      ...current,
      page: String(Math.min(maxPage, Math.max(1, page))),
    }));
  }

  function toggleAiStatsColumn(columnId: AiStatsColumnId) {
    setVisibleColumnIds((current) => {
      if (current.has(columnId) && current.size === 1) {
        return current;
      }

      const next = new Set(current);
      if (next.has(columnId)) {
        next.delete(columnId);
      } else {
        next.add(columnId);
      }

      return next;
    });
  }

  const loadStats = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const query = aiStatsQuery(filters);
      const data = await requestJson<AiStatisticsResponse>(
        `/api/ai-statistics${query ? `?${query}` : ""}`,
      );
      setStats(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void loadStats();
  }, [loadStats]);

  useEffect(() => {
    writeAiStatsVisibleColumnIds(visibleColumnIds);
  }, [visibleColumnIds]);

  async function openRequestDetail(request: AiRequestAuditSummary) {
    setSelectedRequestId(request.id);
    setDetail(null);
    setDetailError(null);
    setCopiedKey(null);
    setIsLoadingDetail(true);

    try {
      const data = await requestJson<AiRequestDetailResponse>(
        `/api/workspaces/${encodeURIComponent(
          request.workspaceId,
        )}/ai-statistics/${encodeURIComponent(request.id)}`,
      );
      setDetail(data);
    } catch (requestError) {
      setDetailError(errorMessage(requestError));
    } finally {
      setIsLoadingDetail(false);
    }
  }

  async function copyAuditText(key: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      window.setTimeout(() => {
        setCopiedKey((current) => (current === key ? null : current));
      }, 1600);
    } catch (copyError) {
      setDetailError(errorMessage(copyError));
    }
  }

  return (
    <div className="panel-scroll h-full min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="flex w-full min-w-0 flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/80 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className="inline-flex size-10 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
                <BarChart3 aria-hidden="true" className="size-5" />
              </span>
              <div className="min-w-0">
                <h2 className="truncate text-lg font-semibold text-stone-950">
                  {t("API details")}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {filters.workspaceId
                    ? selectedWorkspace?.name ?? filters.workspaceId
                    : t("All workspaces")}
                </p>
              </div>
            </div>
            <button
              aria-label={t("Refresh request audit")}
              className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
              disabled={isLoading}
              onClick={() => void loadStats()}
              title={t("Refresh request audit")}
              type="button"
            >
              {isLoading ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <RefreshCw aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <StatsCard
            icon={Activity}
            label={t("Total requests")}
            value={formatNumber(totalCount, language)}
          />
          <StatsCard
            icon={MessageSquare}
            label={t("Total tokens")}
            value={formatCompactNumber(summary.totalTokens, language)}
          />
          <StatsCard
            icon={Bot}
            label={t("Input tokens")}
            value={formatCompactNumber(totalInputTokens, language)}
          />
          <StatsCard
            icon={SlidersHorizontal}
            label={t("Average latency")}
            value={formatNullableLatencySeconds(summary.averageLatencyMs, language)}
          />
        </section>

        <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
            <div>
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Request audit")}
              </h3>
              <p className="mt-1 text-xs text-stone-500">
                {t("requests {count}", {
                  count: formatNumber(totalCount, language),
                })}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <div className="text-xs text-stone-500">
                {t("Output tokens")}:{" "}
                {formatCompactNumber(totalOutputTokens, language)}
              </div>
              <details className="relative">
                <summary className="inline-flex h-9 cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 [&::-webkit-details-marker]:hidden">
                  <SlidersHorizontal aria-hidden="true" className="size-4" />
                  {t("Columns")}
                </summary>
                <div className="absolute right-0 z-20 mt-2 w-56 rounded-xl border border-stone-200 bg-white p-2 shadow-[0_18px_42px_rgba(75,63,42,0.16)]">
                  {aiStatsColumns.map((column) => (
                    <label
                      className="flex min-h-9 cursor-pointer items-center gap-2 rounded-lg px-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                      key={column.id}
                    >
                      <input
                        checked={visibleColumnIds.has(column.id)}
                        className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                        disabled={
                          visibleColumnIds.has(column.id) &&
                          visibleColumnIds.size === 1
                        }
                        onChange={() => toggleAiStatsColumn(column.id)}
                        type="checkbox"
                      />
                      <span className="min-w-0 truncate">{column.label}</span>
                    </label>
                  ))}
                </div>
              </details>
            </div>
          </div>
          <div className="grid gap-3 border-b border-stone-200 bg-stone-50/70 px-4 py-4 md:grid-cols-2 xl:grid-cols-7">
            <FilterSelect
              label={t("Workspace")}
              onChange={(value) =>
                updateAuditFilters({
                  chatId: "",
                  workspaceId: value,
                })
              }
              options={workspaces.map((workspace) => ({
                label: workspace.name,
                value: workspace.id,
              }))}
              placeholder={t("All workspaces")}
              value={filters.workspaceId}
            />
            <FilterSelect
              label={t("Chat")}
              onChange={(value) => updateAuditFilters({ chatId: value })}
              options={chatOptions}
              placeholder={t("All chats")}
              value={filters.chatId}
            />
            <FilterSelect
              label={t("Provider")}
              onChange={(value) => updateAuditFilters({ providerId: value })}
              options={providerOptions}
              placeholder={t("All providers")}
              value={filters.providerId}
            />
            <FilterSelect
              label={t("Model")}
              onChange={(value) => updateAuditFilters({ modelId: value })}
              options={modelOptions}
              placeholder={t("All models")}
              value={filters.modelId}
            />
            <FilterSelect
              label={t("Status")}
              onChange={(value) => updateAuditFilters({ status: value })}
              options={statusOptions}
              placeholder={t("All statuses")}
              value={filters.status}
            />
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started after")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateAuditFilters({
                    startedAfter: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedAfter}
              />
            </label>
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started before")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateAuditFilters({
                    startedBefore: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedBefore}
              />
            </label>
          </div>
          {error ? (
            <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div className="panel-scroll min-w-0 overflow-x-auto">
            <table className="w-full min-w-max text-left text-sm">
              <thead className="border-b border-stone-200 bg-white text-xs font-semibold text-stone-500">
                <tr>
                  {visibleColumns.map((column) => (
                    <th
                      className={`whitespace-nowrap px-4 py-3 ${
                        column.headerClassName ?? ""
                      }`}
                      key={column.id}
                    >
                      {column.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {requests.length ? (
                  requests.map((request) => (
                    <tr key={request.id} className="align-top hover:bg-teal-50/40">
                      {visibleColumns.map((column) => (
                        <td
                          className={`whitespace-nowrap ${column.cellClassName}`}
                          key={column.id}
                        >
                          {column.render(request)}
                        </td>
                      ))}
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td
                      className="px-4 py-10 text-center text-sm text-stone-500"
                      colSpan={visibleColumns.length}
                    >
                      {isLoading ? t("Loading...") : t("No recorded requests")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 px-4 py-3 text-sm">
            <div className="text-stone-500">
              {t("Showing {start}-{end} of {total}", {
                end: formatNumber(pageEnd, language),
                start: formatNumber(pageStart, language),
                total: formatNumber(totalCount, language),
              })}
            </div>
            <div className="flex flex-wrap items-center justify-end gap-3">
              <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                <span>{t("Page size")}</span>
                <input
                  className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  max={500}
                  min={1}
                  onChange={(event) =>
                    updateAuditFilters({ pageSize: event.target.value })
                  }
                  type="number"
                  value={filters.pageSize}
                />
              </label>
              <nav
                aria-label={t("Request audit pagination")}
                className="flex items-center gap-1"
              >
              <button
                aria-label={t("Previous page")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                disabled={isLoading || currentPage <= 1}
                onClick={() => goToAuditPage(currentPage - 1)}
                title={t("Previous page")}
                type="button"
              >
                <ChevronLeft aria-hidden="true" className="size-4" />
              </button>
              {paginationItems.map((item, index) =>
                item === "ellipsis" ? (
                  <span
                    aria-hidden="true"
                    className="inline-flex size-9 items-center justify-center text-stone-400"
                    key={`ellipsis-${index}`}
                  >
                    ...
                  </span>
                ) : (
                  <button
                    aria-current={item === currentPage ? "page" : undefined}
                    aria-label={t("Go to page {page}", {
                      page: formatNumber(item, language),
                    })}
                    className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                      item === currentPage
                        ? "border-teal-700 bg-teal-700 text-white"
                        : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    }`}
                    disabled={isLoading}
                    key={item}
                    onClick={() => goToAuditPage(item)}
                    title={t("Go to page {page}", {
                      page: formatNumber(item, language),
                    })}
                    type="button"
                  >
                    {formatNumber(item, language)}
                  </button>
                ),
              )}
              <button
                aria-label={t("Next page")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                disabled={
                  isLoading || totalPages === 0 || currentPage >= totalPages
                }
                onClick={() => goToAuditPage(currentPage + 1)}
                title={t("Next page")}
                type="button"
              >
                <ChevronRight aria-hidden="true" className="size-4" />
              </button>
              </nav>
            </div>
          </div>
        </section>
      </div>
      {selectedRequestId ? (
        <AiRequestDetailDialog
          copiedKey={copiedKey}
          detail={detail}
          error={detailError}
          isLoading={isLoadingDetail}
          onClose={() => {
            setSelectedRequestId(null);
            setDetail(null);
            setDetailError(null);
            setCopiedKey(null);
          }}
          onCopy={(key, text) => void copyAuditText(key, text)}
        />
      ) : null}
    </div>
  );
}

function FilterSelect({
  label,
  onChange,
  options,
  placeholder,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: { label: string; value: string }[];
  placeholder: string;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <select
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        <option value="">{placeholder}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function AiRequestDetailDialog({
  copiedKey,
  detail,
  error,
  isLoading,
  onClose,
  onCopy,
}: {
  copiedKey: string | null;
  detail: AiRequestDetailResponse | null;
  error: string | null;
  isLoading: boolean;
  onClose: () => void;
  onCopy: (key: string, text: string) => void;
}) {
  const { language, t } = useI18n();
  const request = detail?.request ?? null;

  return (
    <div
      className="fixed inset-0 z-50 flex min-h-0 items-center justify-center overflow-y-auto bg-stone-950/35 p-4 backdrop-blur-sm"
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
      role="presentation"
    >
      <section
        aria-labelledby="ai-request-detail-title"
        aria-modal="true"
        className="flex h-[min(90dvh,56rem)] w-full max-w-6xl flex-col overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="min-w-0">
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="ai-request-detail-title"
            >
              {t("Request details")}
            </h2>
            <p className="mt-1 truncate text-xs font-medium text-stone-500">
              {request ? request.id : t("Loading...")}
            </p>
          </div>
          <button
            aria-label={t("Close request details")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          {error ? (
            <div className="mb-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          {isLoading ? (
            <div className="flex items-center gap-2 py-8 text-sm text-stone-500">
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              {t("Loading...")}
            </div>
          ) : null}
          {request ? (
            <div className="grid gap-4">
              <div className="grid gap-3 rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3 md:grid-cols-2 xl:grid-cols-4">
                <AuditMeta label={t("Workspace")} value={request.workspaceName} />
                <AuditMeta
                  label={t("Chat")}
                  value={request.chatTitle ?? request.chatId ?? "n/a"}
                />
                <AuditMeta label={t("Provider")} value={request.providerId} />
                <AuditMeta label={t("Model")} value={request.modelId} />
                <AuditMeta
                  label={t("Request time")}
                  value={formatAuditDate(request.requestStartedAt, language)}
                />
                <AuditMeta
                  label={t("Latency")}
                  value={formatNullableLatencySeconds(
                    request.totalLatencyMs,
                    language,
                  )}
                />
                <AuditMeta
                  label={t("First token")}
                  value={formatNullableLatencySeconds(
                    request.firstTokenLatencyMs,
                    language,
                  )}
                />
                <AuditMeta
                  label={t("Status")}
                  value={auditStatusText(request.finalState, t)}
                />
              </div>
              <div className="grid gap-4 xl:grid-cols-2">
                <AuditJsonBlock
                  copied={copiedKey === "request"}
                  label={t("Request body")}
                  onCopy={() =>
                    onCopy("request", auditJsonText(request.requestBody))
                  }
                  value={request.requestBody}
                />
                <AuditJsonBlock
                  copied={copiedKey === "response"}
                  label={t("Response body")}
                  onCopy={() =>
                    onCopy("response", auditJsonText(request.responseBody))
                  }
                  value={request.responseBody}
                />
              </div>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function AuditMeta({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs font-semibold text-stone-500">{label}</div>
      <div className="mt-1 truncate text-sm font-medium text-stone-950">
        {value}
      </div>
    </div>
  );
}

function AuditJsonBlock({
  copied,
  label,
  onCopy,
  value,
}: {
  copied: boolean;
  label: string;
  onCopy: () => void;
  value: JsonValue | null;
}) {
  const { t } = useI18n();
  const jsonText = auditJsonText(value);
  const jsonValue = useMemo(
    () => (value === null ? null : normalizedJsonValue(value)),
    [value],
  );
  const collapsiblePaths = useMemo(
    () => collectJsonContainerPaths(jsonValue, "root"),
    [jsonValue],
  );
  const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(new Set());

  useEffect(() => {
    setCollapsedPaths(new Set());
  }, [jsonText]);

  const collapseAll = useCallback(() => {
    setCollapsedPaths(new Set(collapsiblePaths));
  }, [collapsiblePaths]);

  const expandAll = useCallback(() => {
    setCollapsedPaths(new Set());
  }, []);

  const togglePath = useCallback((path: string) => {
    setCollapsedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  return (
    <section className="audit-json-block min-w-0">
      <div className="audit-json-header">
        <span className="audit-json-title">
          <Code2 aria-hidden="true" className="size-4" />
          <span>{label}</span>
        </span>
        <div className="audit-json-actions">
          <button
            aria-label={t("Collapse all {label}", { label })}
            className="audit-json-icon-button"
            disabled={collapsiblePaths.length === 0}
            onClick={collapseAll}
            title={t("Collapse all")}
            type="button"
          >
            <ArrowUp aria-hidden="true" className="size-3.5" />
          </button>
          <button
            aria-label={t("Expand all {label}", { label })}
            className="audit-json-icon-button"
            disabled={collapsedPaths.size === 0}
            onClick={expandAll}
            title={t("Expand all")}
            type="button"
          >
            <ArrowDown aria-hidden="true" className="size-3.5" />
          </button>
          <button
            aria-label={t("Copy {label}", { label })}
            className="audit-json-copy-button"
            onClick={onCopy}
            title={t("Copy {label}", { label })}
            type="button"
          >
            <Copy aria-hidden="true" className="size-3.5" />
            {copied ? t("Copied") : t("Copy")}
          </button>
        </div>
      </div>
      <div className="audit-json-code panel-scroll">
        <code>
          <JsonTreeNode
            collapsedPaths={collapsedPaths}
            depth={0}
            isLast
            onToggle={togglePath}
            path="root"
            value={jsonValue}
          />
        </code>
      </div>
    </section>
  );
}

function StatsCard({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <article className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-semibold text-stone-600">{label}</span>
        <Icon aria-hidden="true" className="size-4 text-teal-700" />
      </div>
      <div className="mt-4 font-mono text-3xl font-semibold text-stone-950">
        {value}
      </div>
    </article>
  );
}

function TodoGraphPanel({
  error,
  isLoading,
  todoGraph,
}: {
  error: string | null;
  isLoading: boolean;
  todoGraph: TodoGraphResponse;
}) {
  const { language, t } = useI18n();

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-800">
            <ListChecks aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">
              {t("ToDo graph")}
            </h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {todoGraph.updatedAt
                ? t("Updated {time}", {
                    time: formatTodoGraphDate(todoGraph.updatedAt, language),
                  })
                : todoGraph.chatId}
            </p>
          </div>
        </div>
        {isLoading ? (
          <LoaderCircle
            aria-hidden="true"
            className="size-4 shrink-0 animate-spin text-stone-500"
          />
        ) : null}
      </div>
      {error ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {error}
        </div>
      ) : null}
      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3">
        <div className="space-y-2">
          {todoGraph.tasks.map((task) => (
            <TodoGraphTaskItem key={task.id} level={0} task={task} />
          ))}
        </div>
      </div>
    </div>
  );
}

function ContextPanel({
  activeTab,
  contextMemories,
  deletingContextMemoryId,
  contextMemoryError,
  diffError,
  diffResponse,
  files,
  isLoadingContextMemories,
  isLoadingDiff,
  isLoadingTodoGraph,
  onForgetContextMemory,
  onRefreshDiff,
  onSelectDiffFile,
  onTabChange,
  selectedPath,
  todoGraph,
  todoGraphError,
}: {
  activeTab: ContextPanelTab;
  contextMemories: ContextMemoryState;
  deletingContextMemoryId: string | null;
  contextMemoryError: string | null;
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  isLoadingContextMemories: boolean;
  isLoadingDiff: boolean;
  isLoadingTodoGraph: boolean;
  onForgetContextMemory: (memory: MemoryFactRecord) => void;
  onRefreshDiff: () => void;
  onSelectDiffFile: (path: string | null) => void;
  onTabChange: (tab: ContextPanelTab) => void;
  selectedPath: string | null;
  todoGraph: TodoGraphResponse | null;
  todoGraphError: string | null;
}) {
  const { t } = useI18n();
  const tabs: { id: ContextPanelTab; label: string; icon: LucideIcon }[] = [
    { id: "todo", label: "ToDo", icon: ListChecks },
    { id: "git", label: "Git", icon: GitCompare },
    { id: "memory", label: "Memory", icon: Brain },
  ];

  return (
    <section className="context-panel flex h-full min-h-0 min-w-0 flex-col">
      <div className="context-panel-tabs panel-scroll" role="tablist">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;

          return (
            <button
              aria-selected={isActive}
              className={`context-panel-tab ${isActive ? "context-panel-tab-active" : ""}`}
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              role="tab"
              type="button"
            >
              <Icon aria-hidden="true" className="size-3.5" />
              <span>{t(tab.label)}</span>
            </button>
          );
        })}
      </div>

      <div className="flex min-h-0 flex-1 flex-col">
        {activeTab === "todo" ? (
          <ContextTodoGraphTab
            error={todoGraphError}
            isLoading={isLoadingTodoGraph}
            todoGraph={todoGraph}
          />
        ) : null}

        {activeTab === "git" ? (
          <div className="flex min-h-0 flex-1 flex-col">
            <GitDiffPanel
              diffError={diffError}
              diffResponse={diffResponse}
              files={files}
              isLoading={isLoadingDiff}
              onRefresh={onRefreshDiff}
              onSelectFile={onSelectDiffFile}
              selectedPath={selectedPath}
            />
          </div>
        ) : null}

        {activeTab === "memory" ? (
          <ContextMemoryTab
            deletingMemoryId={deletingContextMemoryId}
            error={contextMemoryError}
            isLoading={isLoadingContextMemories}
            memories={contextMemories}
            onForgetMemory={onForgetContextMemory}
          />
        ) : null}
      </div>
    </section>
  );
}

function ContextTodoGraphTab({
  error,
  isLoading,
  todoGraph,
}: {
  error: string | null;
  isLoading: boolean;
  todoGraph: TodoGraphResponse | null;
}) {
  const { t } = useI18n();

  if (todoGraph?.exists && todoGraph.tasks.length) {
    return (
      <TodoGraphPanel
        error={error}
        isLoading={isLoading}
        todoGraph={todoGraph}
      />
    );
  }

  return (
    <div className="context-empty-state">
      <ListChecks aria-hidden="true" className="size-5" />
      <h2>{t("ToDo graph")}</h2>
      <p>{t("No todo graph for the active session yet.")}</p>
    </div>
  );
}

function ContextMemoryTab({
  deletingMemoryId,
  error,
  isLoading,
  memories,
  onForgetMemory,
}: {
  deletingMemoryId: string | null;
  error: string | null;
  isLoading: boolean;
  memories: ContextMemoryState;
  onForgetMemory: (memory: MemoryFactRecord) => void;
}) {
  const { t } = useI18n();

  return (
    <div className="context-list-panel panel-scroll">
      {isLoading ? (
        <div className="context-empty-state">
          <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
          <h2>{t("Memory")}</h2>
          <p>{t("Loading...")}</p>
        </div>
      ) : error ? (
        <div className="context-empty-state">
          <Brain aria-hidden="true" className="size-5" />
          <h2>{t("Memory")}</h2>
          <p>{error}</p>
        </div>
      ) : (
        <>
          <ContextMemoryGroup
            deletingMemoryId={deletingMemoryId}
            emptyLabel={t("No memories")}
            label={t("Global memory")}
            memories={memories.global}
            onForgetMemory={onForgetMemory}
          />
          <ContextMemoryGroup
            deletingMemoryId={deletingMemoryId}
            emptyLabel={t("No memories")}
            label={t("Workspace memory")}
            memories={memories.workspace}
            onForgetMemory={onForgetMemory}
          />
        </>
      )}
    </div>
  );
}

function ContextMemoryGroup({
  deletingMemoryId,
  emptyLabel,
  label,
  memories,
  onForgetMemory,
}: {
  deletingMemoryId: string | null;
  emptyLabel: string;
  label: string;
  memories: MemoryFactRecord[];
  onForgetMemory: (memory: MemoryFactRecord) => void;
}) {
  const { t } = useI18n();

  return (
    <div className="context-memory-group">
      <div className="context-panel-section-title">{label}</div>
      {memories.length ? (
        memories.map((memory) => (
          <article className="context-memory-item" key={memory.id}>
            <div className="context-memory-item-header">
              <div className="context-memory-badges">
                <span className="context-memory-kind">{memory.kind}</span>
                {memory.pinned ? (
                  <span className="context-memory-pin">pinned</span>
                ) : null}
              </div>
              <button
                aria-label={t("Delete memory")}
                className="context-memory-delete-button"
                disabled={deletingMemoryId === memory.id}
                onClick={() => onForgetMemory(memory)}
                title={t("Delete memory")}
                type="button"
              >
                {deletingMemoryId === memory.id ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" />
                ) : (
                  <Trash2 aria-hidden="true" />
                )}
              </button>
            </div>
            <p>{memory.fact}</p>
            <small>
              {memory.scope} · {formatTodoGraphDate(memory.updatedAt)}
            </small>
          </article>
        ))
      ) : (
        <div className="context-empty-inline">{emptyLabel}</div>
      )}
    </div>
  );
}

function flattenTodoGraphTasks(tasks: TodoGraphTask[]): TodoGraphTask[] {
  return tasks.flatMap((task) => [task, ...flattenTodoGraphTasks(task.subtasks)]);
}

function TodoGraphTaskItem({
  level,
  task,
}: {
  level: number;
  task: TodoGraphTask;
}) {
  const { t } = useI18n();

  return (
    <div>
      <div
        className="rounded-lg border border-stone-200 bg-white px-3 py-2 shadow-sm"
        style={{ marginLeft: level ? Math.min(level * 14, 42) : 0 }}
      >
        <div className="flex min-w-0 items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] font-semibold text-stone-500">
                {task.id}
              </span>
              <span className={taskStatusClass(task.status)}>
                {t(task.status)}
              </span>
            </div>
            <h3 className="mt-1 break-words text-sm font-semibold leading-snug text-stone-950">
              {task.title}
            </h3>
          </div>
        </div>
        {task.summary ? (
          <p className="mt-2 break-words text-xs leading-5 text-stone-600">
            {task.summary}
          </p>
        ) : null}
        {task.dependsOn.length ? (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {task.dependsOn.map((dependencyId) => (
              <span
                className="rounded-md bg-stone-100 px-1.5 py-0.5 font-mono text-[11px] text-stone-600"
                key={dependencyId}
              >
                {dependencyId}
              </span>
            ))}
          </div>
        ) : null}
        {task.acceptance.length ? (
          <ul className="mt-2 space-y-1 text-xs leading-5 text-stone-600">
            {task.acceptance.map((item, index) => (
              <li className="flex gap-2" key={`${task.id}-acceptance-${index}`}>
                <CheckCircle2
                  aria-hidden="true"
                  className="mt-0.5 size-3.5 shrink-0 text-teal-700"
                />
                <span className="min-w-0 break-words">{item}</span>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
      {task.subtasks.length ? (
        <div className="mt-2 space-y-2">
          {task.subtasks.map((subtask) => (
            <TodoGraphTaskItem
              key={subtask.id}
              level={level + 1}
              task={subtask}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function GitDiffPanel({
  diffError,
  diffResponse,
  files,
  isLoading,
  onRefresh,
  onSelectFile,
  selectedPath,
}: {
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  isLoading: boolean;
  onRefresh: () => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();
  const diffSections = parseGitDiffSections(diffResponse);

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex min-h-[var(--foco-header-height)] items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-teal-50 text-teal-800">
            <GitCompare aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Git diff")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {selectedPath ?? t("Workspace changes")}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            aria-label={t("Refresh diff")}
            className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
            disabled={isLoading}
            onClick={onRefresh}
            title={t("Refresh diff")}
            type="button"
          >
            {isLoading ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <RefreshCw aria-hidden="true" className="size-4" />
            )}
          </button>
        </div>
      </div>

      {diffError ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {diffError}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3">
        <button
          className={diffFileButtonClass(selectedPath === null)}
          onClick={() => onSelectFile(null)}
          type="button"
        >
          <span className="truncate">{t("All changed files")}</span>
          <span className="text-xs text-stone-500">{files.length}</span>
        </button>
        <div className="mt-2 space-y-1">
          {files.length ? (
            files.map((file) => {
              const isExpanded = selectedPath === file.path;
              const label = statusLabel(file);

              return (
                <div key={file.path}>
                  <button
                    aria-label={`${file.path} ${label}`}
                    className={diffFileButtonClass(isExpanded)}
                    onClick={() => onSelectFile(isExpanded ? null : file.path)}
                    type="button"
                  >
                    {isExpanded ? (
                      <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
                    ) : (
                      <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
                    )}
                    <span className="min-w-0 flex-1 truncate text-left">
                      {file.path}
                    </span>
                    <span className="shrink-0 rounded-md bg-stone-100 px-1.5 py-0.5 font-mono text-[11px] text-stone-600">
                      {label}
                    </span>
                  </button>
                  {isExpanded ? (
                    <InlineGitDiff
                      isLoading={isLoading}
                      path={file.path}
                      sections={diffSections}
                    />
                  ) : null}
                </div>
              );
            })
          ) : (
            <div className="rounded-lg border border-dashed border-stone-300 bg-stone-50/80 px-3 py-3 text-sm text-stone-500">
              {t("No changes")}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function InlineGitDiff({
  isLoading,
  path,
  sections,
}: {
  isLoading: boolean;
  path: string;
  sections: GitDiffSection[];
}) {
  const { t } = useI18n();
  const matchingSections = sections
    .map((section) => ({
      ...section,
      files: section.files.filter((file) => file.path === path),
    }))
    .filter((section) => section.files.length > 0);

  if (isLoading) {
    return (
      <div className="ml-5 mt-1 flex items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 py-3 text-xs font-medium text-stone-500">
        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        {t("Loading...")}
      </div>
    );
  }

  if (!matchingSections.length) {
    return (
      <div className="ml-5 mt-1">
        <InlineGitDiffNotice>
          {t("Inline diff is unavailable for binary or non-text files.")}
        </InlineGitDiffNotice>
      </div>
    );
  }

  return (
    <div className="ml-5 mt-1 space-y-2">
      {matchingSections.map((section) => (
        <div key={section.kind} className="space-y-2">
          <div className="text-[11px] font-semibold uppercase text-stone-500">
            {t(section.kind === "staged" ? "Staged" : "Unstaged")}
          </div>
          {section.files.map((file) =>
            file.isBinary || file.lines.length === 0 ? (
              <InlineGitDiffNotice key={`${section.kind}-${file.path}`}>
                {t("Inline diff is unavailable for binary or non-text files.")}
              </InlineGitDiffNotice>
            ) : (
              <div
                className="panel-scroll max-h-[min(30rem,52dvh)] overflow-auto rounded-lg border border-stone-200 bg-white py-2 font-mono text-[11px] leading-5 shadow-sm"
                key={`${section.kind}-${file.path}`}
              >
                {file.lines.map((line, index) => (
                  <div
                    className={diffLineClass(line.kind)}
                    key={`${section.kind}-${file.path}-${index}`}
                  >
                    <span className="select-none pr-2 text-stone-400">
                      {line.prefix}
                    </span>
                    <span>{line.text || " "}</span>
                  </div>
                ))}
              </div>
            ),
          )}
        </div>
      ))}
    </div>
  );
}

function InlineGitDiffNotice({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-stone-200 bg-stone-50 px-3 py-3 text-xs font-medium text-stone-500">
      <FileText aria-hidden="true" className="size-3.5 shrink-0" />
      <span>{children}</span>
    </div>
  );
}

function SettingsPanel({
  activeSection,
  canLogout,
  onAddWorkspace,
  onActiveSectionChange,
  onLogout,
  onSettingsChange,
  onWorkspacesChange,
  workspaceDialogRevision,
}: {
  activeSection: SettingsSection;
  canLogout: boolean;
  onAddWorkspace: () => void;
  onActiveSectionChange: (section: SettingsSection) => void;
  onLogout: () => Promise<void>;
  onSettingsChange: (settings: SettingsResponse) => void;
  onWorkspacesChange: () => Promise<void>;
  workspaceDialogRevision: number;
}) {
  const { language, t } = useI18n();
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [isProviderDialogOpen, setIsProviderDialogOpen] = useState(false);
  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [isMcpDialogOpen, setIsMcpDialogOpen] = useState(false);
  const [metadata, setMetadata] = useState<ModelMetadataResponse | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedMetadataKey, setSelectedMetadataKey] = useState("");
  const [modelSearch, setModelSearch] = useState("");
  const [form, setForm] = useState<ModelFormState>(() => emptyModelForm());
  const [providerForm, setProviderForm] = useState<ProviderFormState>(() =>
    emptyProviderForm(),
  );
  const [generalForm, setGeneralForm] = useState<GeneralFormState>(() =>
    emptyGeneralForm(),
  );
  const [promptSettingsForm, setPromptSettingsForm] =
    useState<PromptSettingsFormState>(() => emptyPromptSettingsForm());
  const [memorySettingsForm, setMemorySettingsForm] =
    useState<MemorySettingsFormState>(() => emptyMemorySettingsForm());
  const [memoryFilter, setMemoryFilter] = useState<MemoryFilterState>(() =>
    emptyMemoryFilter(),
  );
  const [manualMemoryForm, setManualMemoryForm] =
    useState<ManualMemoryFormState>(() => emptyManualMemoryForm());
  const [memorySourceForms, setMemorySourceForms] = useState<MemorySourceFormState[]>(
    [],
  );
  const [expandedMemoryJsonIds, setExpandedMemoryJsonIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [memoryDialogMode, setMemoryDialogMode] =
    useState<MemoryDialogMode>("create");
  const [isMemoryDialogOpen, setIsMemoryDialogOpen] = useState(false);
  const [memories, setMemories] = useState<MemoryFactRecord[]>([]);
  const [memoryListMeta, setMemoryListMeta] = useState<MemoryListMeta>({
    page: 1,
    pageSize: 20,
    totalCount: 0,
    totalPages: 0,
  });
  const [memoryExtractionJobs, setMemoryExtractionJobs] = useState<
    MemoryExtractionJobSummary[]
  >([]);
  const [selectedMemoryId, setSelectedMemoryId] = useState<string | null>(null);
  const [memorySources, setMemorySources] = useState<MemorySourceRecord[]>([]);
  const [workspaceForm, setWorkspaceForm] = useState<WorkspaceFormState>(() =>
    emptyWorkspaceForm(),
  );
  const [mcpForm, setMcpForm] = useState<McpServerFormState>(() =>
    emptyMcpServerForm(),
  );
  const [hookSettings, setHookSettings] = useState<HooksSettingsResponse | null>(
    null,
  );
  const [hookScope, setHookScope] = useState<HookScope>("global");
  const [hookWorkspaceId, setHookWorkspaceId] = useState("");
  const [hookForm, setHookForm] = useState<HookHandlerFormState>(() =>
    emptyHookHandlerForm(),
  );
  const [isHookDialogOpen, setIsHookDialogOpen] = useState(false);
  const [isLoadingHooks, setIsLoadingHooks] = useState(false);
  const [isSavingHooks, setIsSavingHooks] = useState(false);
  const [isImportingHooks, setIsImportingHooks] = useState(false);
  const [isTestingHooks, setIsTestingHooks] = useState(false);
  const [isRefreshingHookRuns, setIsRefreshingHookRuns] = useState(false);
  const [hookImportResult, setHookImportResult] =
    useState<ImportClaudeHooksResponse | null>(null);
  const [hookTestResult, setHookTestResult] = useState<HookRunSummary | null>(
    null,
  );
  const [hookRunDetail, setHookRunDetail] = useState<HookRunDetail | null>(null);
  const [hookTestEvent, setHookTestEvent] = useState("PreToolUse");
  const [hookTestMatcher, setHookTestMatcher] = useState("run_command");
  const [hookTestPayload, setHookTestPayload] = useState(
    '{\n  "toolInput": {\n    "command": "git status"\n  }\n}',
  );
  const [enabledSkillIds, setEnabledSkillIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSavingGeneral, setIsSavingGeneral] = useState(false);
  const [isSavingPromptSettings, setIsSavingPromptSettings] = useState(false);
  const [isSelectingPromptFile, setIsSelectingPromptFile] = useState(false);
  const [isSavingMemorySettings, setIsSavingMemorySettings] = useState(false);
  const [isLoadingMemories, setIsLoadingMemories] = useState(false);
  const [isSavingMemory, setIsSavingMemory] = useState(false);
  const [isClearingPassword, setIsClearingPassword] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingTheme, setIsSavingTheme] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSavingWorkspaceOrder, setIsSavingWorkspaceOrder] = useState(false);
  const [isSavingWorkspaceLogo, setIsSavingWorkspaceLogo] = useState(false);
  const [isSelectingWorkspaceFormPath, setIsSelectingWorkspaceFormPath] =
    useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [isSavingMcpServer, setIsSavingMcpServer] = useState(false);
  const [isSavingModelOrder, setIsSavingModelOrder] = useState(false);
  const [isSavingSkills, setIsSavingSkills] = useState(false);
  const [isRefreshingSkills, setIsRefreshingSkills] = useState(false);
  const [draggedModelId, setDraggedModelId] = useState<string | null>(null);
  const [modelOrderPreview, setModelOrderPreview] = useState<string[] | null>(
    null,
  );
  const [draggedWorkspaceId, setDraggedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceOrderPreview, setWorkspaceOrderPreview] = useState<
    string[] | null
  >(null);
  const workspaceLogoInputRef = useRef<HTMLInputElement | null>(null);
  const [providerTests, setProviderTests] = useState<
    Record<string, ProviderTestState>
  >({});
  const [error, setError] = useState<string | null>(null);
  const [isGeneralPasswordVisible, setIsGeneralPasswordVisible] = useState(false);
  const [isEditingGeneralPassword, setIsEditingGeneralPassword] = useState(false);

  const selectedMetadata = useMemo(
    () =>
      metadata?.models.find((model) => model.key === selectedMetadataKey) ??
      null,
    [metadata, selectedMetadataKey],
  );
  const filteredModels = useMemo(() => {
    const query = modelSearch.trim().toLowerCase();
    const models = metadata?.models ?? [];

    if (!query) {
      return models.slice(0, 80);
    }

    return models
      .filter((model) =>
        [
          model.providerName,
          model.providerId,
          model.name,
          model.modelId,
          model.key,
        ]
          .join(" ")
          .toLowerCase()
          .includes(query),
      )
      .slice(0, 80);
  }, [metadata, modelSearch]);
  const enabledNeedsLimits =
    form.enabled &&
    (!form.contextWindow.trim() || !form.maxOutputTokens.trim());
  const providerKinds = settings?.providerKinds ?? [];
  const providers = settings?.providers ?? [];
  const workspaces = settings?.workspaces ?? [];
  const memoryWorkspace =
    workspaces.find((workspace) => workspace.id === memoryFilter.workspaceId) ??
    workspaces[0] ??
    null;
  const memoryDialogWorkspace =
    workspaces.find((workspace) => workspace.id === manualMemoryForm.workspaceId) ??
    workspaces[0] ??
    null;
  const selectedMemory =
    memories.find((memory) => memory.id === selectedMemoryId) ?? null;
  const memoryPaginationItems = auditPaginationItems(
    memoryListMeta.page,
    memoryListMeta.totalPages,
  );
  const memoryPageStart = memories.length
    ? (memoryListMeta.page - 1) * memoryListMeta.pageSize + 1
    : 0;
  const memoryPageEnd = memories.length
    ? Math.min(memoryListMeta.totalCount, memoryPageStart + memories.length - 1)
    : 0;
  const canClearFilteredMemories =
    memoryFilter.scope !== "global" &&
    (memoryFilter.scope !== "chat" || Boolean(memoryFilter.chatId.trim()));
  const isMemoryFilterReady =
    memoryFilter.scope !== "chat" || Boolean(memoryFilter.chatId.trim());
  const clearFilteredMemoryLabel =
    memoryFilter.scope === "chat"
      ? t("Clear filtered chat memories")
      : t("Clear filtered workspace memories");
  const selectedHookWorkspace =
    workspaces.find((workspace) => workspace.id === hookWorkspaceId) ??
    workspaces[0] ??
    null;
  const activeHookConfig =
    hookScope === "global"
      ? hookSettings?.global.config
      : hookSettings?.workspace.config;
  const activeHookPath =
    hookScope === "global"
      ? hookSettings?.global.path
      : hookSettings?.workspace.path;
  const activeHookGroups = hookConfigEntries(activeHookConfig);
  const terminalShells = settings?.terminalShells ?? [];
  const mcpTransports = settings?.mcpTransports ?? [];
  const mcpServers = settings?.mcpServers ?? [];
  const skills = settings?.skills;
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const configuredModels =
    settings?.configuredModels ?? metadata?.configuredModels ?? [];
  const passwordInputValue =
    generalForm.password ||
    (settings?.general.webServer.passwordEnabled && !isEditingGeneralPassword
      ? SAVED_PASSWORD_MASK
      : "");
  const orderedConfiguredModels = useMemo(() => {
    if (!modelOrderPreview) {
      return configuredModels;
    }

    const modelsById = new Map(
      configuredModels.map((model) => [model.id, model]),
    );
    const previewModels = modelOrderPreview
      .map((modelId) => modelsById.get(modelId))
      .filter((model): model is ConfiguredModelSummary => Boolean(model));

    return previewModels.length === configuredModels.length
      ? previewModels
      : configuredModels;
  }, [configuredModels, modelOrderPreview]);
  const orderedWorkspaces = useMemo(() => {
    if (!workspaceOrderPreview) {
      return workspaces;
    }

    const workspacesById = new Map(
      workspaces.map((workspace) => [workspace.id, workspace]),
    );
    const previewWorkspaces = workspaceOrderPreview
      .map((workspaceId) => workspacesById.get(workspaceId))
      .filter(
        (workspace): workspace is ConfiguredWorkspaceSummary =>
          Boolean(workspace),
      );

    return previewWorkspaces.length === workspaces.length
      ? previewWorkspaces
      : workspaces;
  }, [workspaceOrderPreview, workspaces]);
  const editingModel =
    configuredModels.find((model) => model.id === form.modelId) ?? null;
  const editingWorkspace =
    workspaces.find((workspace) => workspace.id === workspaceForm.id) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const apiProxyTypes = editingProvider?.apiProxy.supportedTypes ??
    providers[0]?.apiProxy.supportedTypes ?? [
      { label: "HTTP", proxyType: "http" },
      { label: "SOCKS", proxyType: "socks" },
    ];
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const selectedProviderIds = new Set(form.providerIds);

  function syncSkillsForm(data: SettingsResponse) {
    setEnabledSkillIds(
      new Set(
        data.skills.detected
          .filter((skill) => skill.enabled)
          .map((skill) => skill.key),
      ),
    );
  }

  function syncGeneralForm(data: SettingsResponse) {
    setIsEditingGeneralPassword(false);
    setIsGeneralPasswordVisible(false);
    setGeneralForm({
      hookAuditEnabled: data.general.hookAuditEnabled,
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
      password: "",
      theme: data.general.theme,
    });
  }

  function syncPromptSettingsForm(data: SettingsResponse) {
    setPromptSettingsForm({
      extraText: data.prompts.extraText,
      files: data.prompts.files,
      pendingFile: "",
    });
  }

  function syncMemorySettingsForm(data: SettingsResponse) {
    setMemorySettingsForm({
      enabled: data.memory.enabled,
      extractionMode: data.memory.extractionMode,
      retrievalMode: data.memory.retrievalMode,
      extractionModelId: data.memory.extractionModelId ?? "",
      retrievalModelId: data.memory.retrievalModelId ?? "",
      retentionDays:
        data.memory.retentionDays === null ? "" : String(data.memory.retentionDays),
    });
    setMemoryFilter((current) => ({
      ...current,
      workspaceId: current.workspaceId || data.workspaces[0]?.id || "",
    }));
  }

  const loadMetadata = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/model-metadata",
      );
      setMetadata(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadSettings = useCallback(async () => {
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      setSettings(data);
      onSettingsChange(data);
      setHookWorkspaceId((current) => current || data.workspaces[0]?.id || "");
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
      setDraggedModelId(null);
      setModelOrderPreview(null);
      syncGeneralForm(data);
      syncPromptSettingsForm(data);
      syncMemorySettingsForm(data);
      setProviderForm((current) => ({
        ...current,
        kind: current.kind || data.providerKinds[0]?.kind || "openai-responses",
      }));
      setMcpForm((current) => ({
        ...current,
        transport: current.transport || data.mcpTransports[0]?.transport || "stdio",
      }));
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, [onSettingsChange]);

  const loadHooks = useCallback(async (workspaceId: string) => {
    if (!workspaceId) {
      return;
    }

    setIsLoadingHooks(true);
    setError(null);

    try {
      const data = await requestJson<HooksSettingsResponse>(
        `/api/hooks?workspaceId=${encodeURIComponent(workspaceId)}`,
      );
      setHookSettings(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingHooks(false);
    }
  }, []);

  const loadMemories = useCallback(async () => {
    setIsLoadingMemories(true);
    setError(null);

    try {
      const chatId = memoryFilter.chatId.trim();
      if (memoryFilter.scope === "chat" && !chatId) {
        setMemories([]);
        setMemoryExtractionJobs([]);
        setMemoryListMeta({
          page: 1,
          pageSize: memoryFilter.pageSize,
          totalCount: 0,
          totalPages: 0,
        });
        setSelectedMemoryId(null);
        return;
      }

      const params = new URLSearchParams({
        page: String(memoryFilter.page),
        pageSize: String(memoryFilter.pageSize),
        scope: memoryFilter.scope,
        status: memoryFilter.status,
      });
      if (memoryFilter.workspaceId) {
        params.set("workspaceId", memoryFilter.workspaceId);
      }
      if (chatId) {
        params.set("chatId", chatId);
      }
      if (memoryFilter.kind) {
        params.set("kind", memoryFilter.kind);
      }
      if (memoryFilter.query.trim()) {
        params.set("query", memoryFilter.query.trim());
      }
      const data = await requestJson<MemoryListResponse>(
        `/api/memory?${params.toString()}`,
      );
      if (data.totalPages > 0 && data.page > data.totalPages) {
        setMemoryFilter((current) =>
          current.page === data.page ? { ...current, page: data.totalPages } : current,
        );
        return;
      }
      setMemories(data.memories);
      setMemoryExtractionJobs(data.extractionJobs ?? []);
      setMemoryListMeta({
        page: data.page,
        pageSize: data.pageSize,
        totalCount: data.totalCount,
        totalPages: data.totalPages,
      });
      setSelectedMemoryId((current) =>
        current && data.memories.some((memory) => memory.id === current)
          ? current
          : data.memories[0]?.id ?? null,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingMemories(false);
    }
  }, [memoryFilter]);

  useEffect(() => {
    void loadMetadata();
    void loadSettings();
  }, [loadMetadata, loadSettings]);

  useEffect(() => {
    if (hookWorkspaceId) {
      void loadHooks(hookWorkspaceId);
    }
  }, [hookWorkspaceId, loadHooks]);

  useEffect(() => {
    if (activeSection === "memory") {
      void loadMemories();
    }
  }, [activeSection, loadMemories]);

  useEffect(() => {
    if (activeSection !== "memory" || !selectedMemory) {
      setMemorySources([]);
      setMemorySourceForms([]);
      return;
    }
    const memoryForSources = selectedMemory;

    async function loadMemorySources() {
      try {
        const params = new URLSearchParams({
          memoryId: memoryForSources.id,
          scope: memoryFilter.scope,
        });
        if (memoryFilter.workspaceId) {
          params.set("workspaceId", memoryFilter.workspaceId);
        }
        const data = await requestJson<MemorySourcesResponse>(
          `/api/memory/sources?${params.toString()}`,
        );
        setMemorySources(data.sources);
        if (isMemoryDialogOpen && memoryDialogMode === "edit") {
          setMemorySourceForms(memorySourceRecordsToForm(data.sources));
        }
      } catch (requestError) {
        setError(errorMessage(requestError));
      }
    }

    void loadMemorySources();
  }, [
    activeSection,
    isMemoryDialogOpen,
    memoryFilter.scope,
    memoryFilter.workspaceId,
    memoryDialogMode,
    selectedMemory?.id,
  ]);

  useEffect(() => {
    if (workspaceDialogRevision > 0) {
      void loadSettings();
    }
  }, [loadSettings, workspaceDialogRevision]);

  function selectMetadataModel(key: string) {
    setSelectedMetadataKey(key);
    const model = metadata?.models.find((item) => item.key === key);

    if (!model) {
      return;
    }

    setForm({
      displayName: model.name,
      enabled: model.contextWindow !== null && model.maxOutputTokens !== null,
      modelId: model.modelId,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: [],
      activeProviderId: "",
      thinkingLevel: "",
    });
    setIsModelDialogOpen(true);
  }

  function modelMetadataForInput(modelId: string) {
    const normalizedModelId = modelId.trim();

    if (!normalizedModelId) {
      return null;
    }

    const models = metadata?.models ?? [];

    return (
      models.find((model) => model.key === normalizedModelId) ??
      models.find((model) => model.modelId === normalizedModelId) ??
      null
    );
  }

  function updateModelId(modelId: string) {
    const model = modelMetadataForInput(modelId);
    setSelectedMetadataKey(model?.key ?? "");

    setForm((current) => {
      if (!model) {
        return {
          ...current,
          modelId,
        };
      }

      return {
        ...current,
        displayName: model.name,
        enabled: model.contextWindow !== null && model.maxOutputTokens !== null,
        modelId: model.modelId,
        contextWindow: numberInputValue(model.contextWindow),
        maxOutputTokens: numberInputValue(model.maxOutputTokens),
      };
    });
  }

  function editConfiguredModel(model: ConfiguredModelSummary) {
    setSelectedMetadataKey(model.metadataKey ?? "");
    setForm({
      displayName: model.displayName,
      enabled: model.enabled,
      modelId: model.id,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: model.providerIds,
      activeProviderId: model.activeProviderId ?? "",
      thinkingLevel: model.thinkingLevel ?? "",
    });
    setIsModelDialogOpen(true);
  }

  function startAddingModel() {
    setSelectedMetadataKey("");
    setForm(emptyModelForm());
    setIsModelDialogOpen(true);
  }

  function startAddingProviderFromModel() {
    setIsModelDialogOpen(false);
    onActiveSectionChange("providers");
    startAddingProvider();
  }

  function editConfiguredProvider(provider: ConfiguredProviderSummary) {
    setProviderForm({
      apiKey: "",
      apiProxyEnabled: provider.apiProxy.enabled,
      apiProxyType:
        provider.apiProxy.proxyType ||
        provider.apiProxy.supportedTypes[0]?.proxyType ||
        "http",
      apiProxyUrl: provider.apiProxy.url,
      baseUrl: provider.baseUrl ?? "",
      clearApiKey: false,
      enabled: provider.enabled,
      id: provider.id,
      kind: provider.kind,
      name: provider.name,
    });
    setIsProviderDialogOpen(true);
  }

  function startAddingProvider() {
    setProviderForm({
      ...emptyProviderForm(),
      kind: providerKinds[0]?.kind || "openai-responses",
    });
    setIsProviderDialogOpen(true);
  }

  function editConfiguredMcpServer(server: ConfiguredMcpServerSummary) {
    setMcpForm({
      argsText: server.args.join("\n"),
      command: server.command ?? "",
      enabled: server.enabled,
      id: server.id,
      name: server.name,
      transport: server.transport,
      url: server.url ?? "",
    });
    setIsMcpDialogOpen(true);
  }

  function editConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setWorkspaceForm({
      commonCommands: workspace.commonCommands.map((command) => ({ ...command })),
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
      pinned: workspace.pinned,
      terminalShell: workspace.terminalShell,
    });
    setIsWorkspaceDialogOpen(true);
  }

  function startAddingMcpServer() {
    setMcpForm({
      ...emptyMcpServerForm(),
      transport: mcpTransports[0]?.transport || "stdio",
    });
    setIsMcpDialogOpen(true);
  }

  function startAddingHookHandler() {
    setHookForm({
      ...emptyHookHandlerForm(),
      event: hookSettings?.supportedEvents[0] ?? "PreToolUse",
    });
    setIsHookDialogOpen(true);
  }

  function editHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    group: HookMatcherGroup,
    handler: HookHandler,
  ) {
    setHookForm(hookHandlerFormFromConfig(event, groupIndex, handlerIndex, group, handler));
    setIsHookDialogOpen(true);
  }

  async function refreshMetadata() {
    setIsRefreshing(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/model-metadata/refresh",
        { method: "POST" },
      );
      setMetadata(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshing(false);
    }
  }

  async function saveGeneralSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingGeneral(true);
    setError(null);

    try {
      const password = generalForm.password;
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          listenHost: generalForm.listenHost,
          listenPort: optionalPositiveInteger(
            generalForm.listenPort,
            t("Listen port"),
          ),
          hookAuditEnabled: generalForm.hookAuditEnabled,
          language: generalForm.language,
          password: password.trim() ? password : null,
          theme: generalForm.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingGeneral(false);
    }
  }

  async function saveLanguageSetting(language: string) {
    setGeneralForm((current) => ({
      ...current,
      language,
    }));

    if (!settings) {
      return;
    }

    setIsSavingLanguage(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setGeneralForm((current) => ({
        ...current,
        language: data.general.language,
        theme: data.general.theme,
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        language: settings.general.language,
        theme: settings.general.theme,
      }));
    } finally {
      setIsSavingLanguage(false);
    }
  }

  async function saveThemeSetting(theme: AppThemeId) {
    setGeneralForm((current) => ({
      ...current,
      theme,
    }));

    if (!settings) {
      return;
    }

    setIsSavingTheme(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        theme: settings.general.theme,
      }));
    } finally {
      setIsSavingTheme(false);
    }
  }

  async function savePromptSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingPromptSettings(true);
    setError(null);

    try {
      const files = promptSettingsForm.files.map((file) => file.trim());
      const data = await requestJson<SettingsResponse>("/api/settings/prompts", {
        body: JSON.stringify({
          extraText: promptSettingsForm.extraText,
          files,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncPromptSettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingPromptSettings(false);
    }
  }

  function addPromptFilePath(path: string) {
    const nextPath = path.trim();
    if (!nextPath) {
      return;
    }

    setPromptSettingsForm((current) => {
      if (current.files.includes(nextPath)) {
        return {
          ...current,
          pendingFile: "",
        };
      }

      return {
        ...current,
        files: [...current.files, nextPath],
        pendingFile: "",
      };
    });
  }

  function removePromptFilePath(path: string) {
    setPromptSettingsForm((current) => ({
      ...current,
      files: current.files.filter((file) => file !== path),
    }));
  }

  async function selectPromptFile() {
    setIsSelectingPromptFile(true);
    setError(null);

    try {
      const data = await requestJson<{ files: NativeSelectedFile[] }>(
        "/api/native/select-files",
        { method: "POST" },
      );
      setPromptSettingsForm((current) => {
        const files = [...current.files];
        for (const file of data.files) {
          if (!files.includes(file.path)) {
            files.push(file.path);
          }
        }
        return {
          ...current,
          files,
        };
      });
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingPromptFile(false);
    }
  }

  async function clearBrowserPassword() {
    if (!settings?.general.webServer.passwordEnabled) {
      return;
    }

    setIsClearingPassword(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: true,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsClearingPassword(false);
    }
  }

  async function saveMemorySettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingMemorySettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/memory", {
        body: JSON.stringify({
          enabled: memorySettingsForm.enabled,
          extractionMode: memorySettingsForm.extractionMode,
          retrievalMode: memorySettingsForm.retrievalMode,
          extractionModelId: memorySettingsForm.extractionModelId.trim() || null,
          retrievalModelId: memorySettingsForm.retrievalModelId.trim() || null,
          retentionDays: optionalPositiveInteger(
            memorySettingsForm.retentionDays,
            t("Retention days"),
          ),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncMemorySettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemorySettings(false);
    }
  }

  function updateMemoryFilter(patch: Partial<MemoryFilterState>) {
    setMemoryFilter((current) => ({
      ...current,
      ...patch,
      page: 1,
    }));
  }

  function goToMemoryPage(page: number) {
    if (!isMemoryFilterReady) {
      return;
    }

    setMemoryFilter((current) => ({
      ...current,
      page,
    }));
  }

  function updateMemoryPageSize(value: string) {
    setMemoryFilter((current) => ({
      ...current,
      page: 1,
      pageSize: Math.min(200, positiveIntegerText(value, current.pageSize)),
    }));
  }

  function openCreateMemoryDialog() {
    setMemoryDialogMode("create");
    setMemorySourceForms([]);
    setExpandedMemoryJsonIds(new Set());
    setManualMemoryForm({
      ...emptyManualMemoryForm(),
      chatId: memoryFilter.chatId,
      scope: memoryFilter.scope,
      workspaceId: memoryFilter.workspaceId || workspaces[0]?.id || "",
    });
    setIsMemoryDialogOpen(true);
  }

  function openEditMemoryDialog(memory: MemoryFactRecord) {
    const isCurrentSelection = selectedMemoryId === memory.id;
    setSelectedMemoryId(memory.id);
    setMemoryDialogMode("edit");
    setMemorySourceForms(
      isCurrentSelection ? memorySourceRecordsToForm(memorySources) : [],
    );
    setExpandedMemoryJsonIds(new Set());
    setManualMemoryForm({
      chatId: memory.chatId ?? "",
      confidence: memory.confidence === null ? "" : String(memory.confidence),
      fact: memory.fact,
      kind: memory.kind,
      metadataText: prettyJsonText(memory.metadataJson),
      pinned: memory.pinned,
      scope: memory.scope as ManualMemoryFormState["scope"],
      workspaceId: memoryFilter.workspaceId || workspaces[0]?.id || "",
    });
    setIsMemoryDialogOpen(true);
  }

  function closeMemoryDialog() {
    setIsMemoryDialogOpen(false);
    setMemoryDialogMode("create");
    setManualMemoryForm(emptyManualMemoryForm());
    setMemorySourceForms([]);
    setExpandedMemoryJsonIds(new Set());
  }

  function updateMemorySourceForm(
    sourceId: string,
    field: keyof Omit<MemorySourceFormState, "id">,
    value: string,
  ) {
    setMemorySourceForms((current) =>
      current.map((source) =>
        source.id === sourceId ? { ...source, [field]: value } : source,
      ),
    );
  }

  function toggleMemoryJson(id: string) {
    setExpandedMemoryJsonIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function promoteMemoryOneLevel(memory: MemoryFactRecord) {
    if (memory.scope === "chat") {
      void promoteMemory(memory.id, "workspace");
    } else if (memory.scope === "workspace") {
      void promoteMemory(memory.id, "global");
    }
  }

  async function saveMemoryDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (memoryDialogMode === "edit" && !selectedMemory) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      const scope = manualMemoryForm.scope;
      const workspaceId =
        scope === "global" ? null : manualMemoryForm.workspaceId || memoryFilter.workspaceId;
      const metadata = parseJsonText(manualMemoryForm.metadataText || "{}", t("Memory metadata"));
      const payload =
        memoryDialogMode === "create"
          ? {
              chatId: scope === "chat" ? manualMemoryForm.chatId : null,
              confidence: optionalNumber(manualMemoryForm.confidence, t("Confidence")),
              fact: manualMemoryForm.fact,
              kind: manualMemoryForm.kind,
              metadata,
              pinned: manualMemoryForm.pinned,
              scope,
              workspaceId,
            }
          : {
              confidence: optionalNumber(manualMemoryForm.confidence, t("Confidence")),
              fact: manualMemoryForm.fact,
              kind: manualMemoryForm.kind,
              memoryId: selectedMemory?.id,
              metadata,
              pinned: manualMemoryForm.pinned,
              scope: memoryFilter.scope,
              sources: memorySourceForms.map((source) => ({
                content: source.content,
                id: source.id,
                metadata: parseJsonText(
                  source.metadataText || "{}",
                  `${t("Source metadata")} ${source.id}`,
                ),
                title: source.title,
              })),
              workspaceId:
                memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
            };
      const endpoint =
        memoryDialogMode === "create" ? "/api/memory/manual" : "/api/memory/edit";
      await requestJson<MemoryMutationResponse>(endpoint, {
        body: JSON.stringify(payload),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      closeMemoryDialog();
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function setMemoryStatus(memoryId: string, status: string) {
    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/status", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          status,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function forgetMemory(memoryId: string) {
    if (!window.confirm(t("Delete memory confirmation"))) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/forget", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function clearFilteredMemories() {
    if (!canClearFilteredMemories) {
      return;
    }
    if (!window.confirm(t("Clear filtered memories confirmation"))) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<ClearMemoriesResponse>("/api/memory/clear", {
        body: JSON.stringify({
          chatId: memoryFilter.scope === "chat" ? memoryFilter.chatId : null,
          kind: memoryFilter.kind || null,
          query: memoryFilter.query.trim() || null,
          scope: memoryFilter.scope,
          status: memoryFilter.status,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const shouldReload = memoryFilter.page === 1;
      setMemoryFilter((current) => ({
        ...current,
        page: 1,
      }));
      if (shouldReload) {
        await loadMemories();
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function promoteMemory(memoryId: string, targetScope: "workspace" | "global") {
    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/promote", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          targetChatId: null,
          targetScope,
          targetWorkspaceId:
            targetScope === "global" ? null : memoryFilter.workspaceId,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function saveModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/models/manual",
        {
          body: JSON.stringify({
            displayName: form.displayName,
            enabled: form.enabled,
            metadataKey: selectedMetadataKey || null,
            modelId: form.modelId,
            contextWindow: optionalPositiveInteger(
              form.contextWindow,
              "Context window",
            ),
            maxOutputTokens: optionalPositiveInteger(
              form.maxOutputTokens,
              "Max output tokens",
            ),
            providerIds: form.providerIds,
            activeProviderId: form.activeProviderId,
            thinkingLevel: form.thinkingLevel || null,
            clearThinkingLevel: !form.thinkingLevel,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setMetadata(data);
      await loadSettings();
      setIsModelDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  async function saveModelOrder(modelIds: string[]) {
    setIsSavingModelOrder(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/models/order", {
        body: JSON.stringify({ modelIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setDraggedModelId(null);
      setModelOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
    } finally {
      setIsSavingModelOrder(false);
    }
  }

  async function saveWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    const shouldSaveOrder = editingWorkspace?.pinned !== workspaceForm.pinned;
    const workspaceIds = shouldSaveOrder
      ? groupedWorkspaceIds(
          orderedWorkspaces.map((workspace) =>
            workspace.id === workspaceForm.id
              ? { ...workspace, pinned: workspaceForm.pinned }
              : workspace,
          ),
        )
      : null;

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/manual", {
        body: JSON.stringify({
          id: workspaceForm.id,
          name: workspaceForm.name,
          path: workspaceForm.path,
          pinned: workspaceForm.pinned,
          terminalShell: workspaceForm.terminalShell,
          commonCommands: workspaceForm.commonCommands,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const finalData = workspaceIds
        ? await requestJson<SettingsResponse>("/api/workspaces/order", {
            body: JSON.stringify({ workspaceIds }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          })
        : data;
      setSettings(finalData);
      onSettingsChange(finalData);
      await onWorkspacesChange();
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  function addWorkspaceCommonCommand() {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: [...current.commonCommands, { name: "", command: "" }],
    }));
  }

  function updateWorkspaceCommonCommand(
    index: number,
    field: keyof WorkspaceCommonCommandSummary,
    value: string,
  ) {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: current.commonCommands.map((command, commandIndex) =>
        commandIndex === index ? { ...command, [field]: value } : command,
      ),
    }));
  }

  function removeWorkspaceCommonCommand(index: number) {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: current.commonCommands.filter(
        (_command, commandIndex) => commandIndex !== index,
      ),
    }));
  }

  async function saveWorkspaceOrder(workspaceIds: string[]) {
    setIsSavingWorkspaceOrder(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/order", {
        body: JSON.stringify({ workspaceIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
      await onWorkspacesChange();
    } finally {
      setIsSavingWorkspaceOrder(false);
    }
  }

  async function toggleWorkspacePinned(
    workspace: ConfiguredWorkspaceSummary,
    pinned: boolean,
  ) {
    setIsSavingWorkspaceOrder(true);
    setError(null);

    const nextWorkspaces = orderedWorkspaces.map((item) =>
      item.id === workspace.id ? { ...item, pinned } : item,
    );
    const workspaceIds = groupedWorkspaceIds(nextWorkspaces);

    try {
      await requestJson<SettingsResponse>("/api/workspaces/manual", {
          body: JSON.stringify({
            id: workspace.id,
            name: workspace.name,
            path: workspace.path,
            pinned,
            terminalShell: workspace.terminalShell,
            commonCommands: workspace.commonCommands,
          }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const orderData = await requestJson<SettingsResponse>("/api/workspaces/order", {
        body: JSON.stringify({ workspaceIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(orderData);
      onSettingsChange(orderData);
      setWorkspaceForm((current) =>
        current.id === workspace.id ? { ...current, pinned } : current,
      );
      await onWorkspacesChange();
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
      await onWorkspacesChange();
    } finally {
      setIsSavingWorkspaceOrder(false);
    }
  }

  async function selectWorkspaceFormPath() {
    setIsSelectingWorkspaceFormPath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        { method: "POST" },
      );

      if (data.path) {
        setWorkspaceForm((current) => ({
          ...current,
          name: current.name.trim()
            ? current.name
            : workspaceNameFromPath(data.path ?? ""),
          path: data.path ?? current.path,
        }));
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingWorkspaceFormPath(false);
    }
  }

  async function saveWorkspaceLogo(contentBase64: string) {
    if (!editingWorkspace) {
      return;
    }

    setIsSavingWorkspaceLogo(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        `/api/workspaces/${encodeURIComponent(editingWorkspace.id)}/logo`,
        {
          body: JSON.stringify({ contentBase64 }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspaceLogo(false);
    }
  }

  async function clearWorkspaceLogo() {
    if (!editingWorkspace?.logoUrl) {
      return;
    }

    setIsSavingWorkspaceLogo(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        `/api/workspaces/${encodeURIComponent(editingWorkspace.id)}/logo`,
        { method: "DELETE" },
      );
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspaceLogo(false);
    }
  }

  async function uploadWorkspaceLogoFile(file: File) {
    try {
      const contentBase64 = await fileToBase64(file);
      await saveWorkspaceLogo(contentBase64);
    } catch (readError) {
      setError(errorMessage(readError));
    }
  }

  function handleWorkspaceLogoFileChange(
    event: ReactChangeEvent<HTMLInputElement>,
  ) {
    const file = event.target.files?.[0] ?? null;
    event.target.value = "";
    if (!file) {
      return;
    }

    void uploadWorkspaceLogoFile(file);
  }

  function handleWorkspaceDragStart(
    event: ReactDragEvent<HTMLElement>,
    workspaceId: string,
  ) {
    setDraggedWorkspaceId(workspaceId);
    setWorkspaceOrderPreview(orderedWorkspaces.map((workspace) => workspace.id));
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", workspaceId);
  }

  function handleWorkspaceDragOver(
    event: ReactDragEvent<HTMLDivElement>,
    targetWorkspaceId: string,
  ) {
    event.preventDefault();

    const sourceWorkspaceId = draggedWorkspaceId;
    if (!sourceWorkspaceId || sourceWorkspaceId === targetWorkspaceId) {
      return;
    }

    const sourceWorkspace = orderedWorkspaces.find(
      (workspace) => workspace.id === sourceWorkspaceId,
    );
    const targetWorkspace = orderedWorkspaces.find(
      (workspace) => workspace.id === targetWorkspaceId,
    );
    if (!sourceWorkspace || !targetWorkspace || sourceWorkspace.pinned !== targetWorkspace.pinned) {
      return;
    }

    const workspaceIds = moveItemId(
      workspaceOrderPreview ?? orderedWorkspaces.map((workspace) => workspace.id),
      sourceWorkspaceId,
      targetWorkspaceId,
    );
    setWorkspaceOrderPreview(workspaceIds);
  }

  async function handleWorkspaceDrop(event: ReactDragEvent<HTMLDivElement>) {
    event.preventDefault();

    const workspaceIds = workspaceOrderPreview;
    setDraggedWorkspaceId(null);

    if (!workspaceIds || sameStringList(workspaceIds, workspaces.map((workspace) => workspace.id))) {
      setWorkspaceOrderPreview(null);
      return;
    }

    await saveWorkspaceOrder(workspaceIds);
  }

  function handleWorkspaceDragEnd() {
    setDraggedWorkspaceId(null);
    setWorkspaceOrderPreview(null);
  }

  async function saveProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingProvider(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/providers/manual",
        {
          body: JSON.stringify({
            apiKey: providerForm.apiKey || null,
            apiProxy: {
              enabled: providerForm.apiProxyEnabled,
              proxyType: providerForm.apiProxyType,
              url: providerForm.apiProxyUrl,
            },
            baseUrl: providerForm.baseUrl || null,
            clearApiKey: providerForm.clearApiKey,
            enabled: providerForm.enabled,
            id:
              providerForm.id ||
              nextProviderId(providerForm.name, providerForm.kind, providers),
            kind: providerForm.kind,
            name: providerForm.name,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setProviderForm((current) => ({
        ...current,
        apiKey: "",
        clearApiKey: false,
      }));
      setIsProviderDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingProvider(false);
    }
  }

  async function saveMcpServer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingMcpServer(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/mcp/servers/manual",
        {
          body: JSON.stringify({
            args: mcpForm.argsText
              .split(/\r?\n/)
              .map((arg) => arg.trim())
              .filter(Boolean),
            command: mcpForm.command || null,
            enabled: mcpForm.enabled,
            id:
              mcpForm.id ||
              nextMcpServerId(mcpForm.name, mcpForm.transport, mcpServers),
            name: mcpForm.name,
            transport: mcpForm.transport,
            url: mcpForm.url || null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setIsMcpDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMcpServer(false);
    }
  }

  async function deleteProvider(providerId: string) {
    setIsSavingProvider(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/providers/delete", {
        body: JSON.stringify({ id: providerId }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setProviderForm({
        ...emptyProviderForm(),
        kind: data.providerKinds[0]?.kind || "openai-responses",
      });
      setIsProviderDialogOpen(false);
      setForm((current) => ({
        ...current,
        activeProviderId:
          current.activeProviderId === providerId
            ? ""
            : current.activeProviderId,
        providerIds: current.providerIds.filter((id) => id !== providerId),
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingProvider(false);
    }
  }

  async function deleteMcpServer(serverId: string) {
    setIsSavingMcpServer(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/mcp/servers/delete",
        {
          body: JSON.stringify({ id: serverId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setMcpForm({
        ...emptyMcpServerForm(),
        transport: data.mcpTransports[0]?.transport || "stdio",
      });
      setIsMcpDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMcpServer(false);
    }
  }

  async function deleteModel(modelId: string) {
    setIsSaving(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>("/api/models/delete", {
        body: JSON.stringify({ id: modelId }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setMetadata(data);
      await loadSettings();
      setSelectedMetadataKey("");
      setForm(emptyModelForm());
      setIsModelDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  async function saveSkills(nextEnabledSkillIds: Set<string>) {
    setIsSavingSkills(true);
    setError(null);

    try {
      const disabledSkillIds = (skills?.detected ?? [])
        .filter((skill) => !nextEnabledSkillIds.has(skill.key))
        .map((skill) => skill.key);
      const data = await requestJson<SettingsResponse>("/api/skills/manual", {
        body: JSON.stringify({
          disabled: disabledSkillIds,
          enabled: Array.from(nextEnabledSkillIds),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingSkills(false);
    }
  }

  async function refreshSkills() {
    setIsRefreshingSkills(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/skills/refresh", {
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingSkills(false);
    }
  }

  async function saveHookAuditEnabled(hookAuditEnabled: boolean) {
    if (!settings) {
      return;
    }

    setGeneralForm((current) => ({
      ...current,
      hookAuditEnabled,
    }));
    setIsSavingGeneral(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        hookAuditEnabled: settings.general.hookAuditEnabled,
      }));
    } finally {
      setIsSavingGeneral(false);
    }
  }

  async function refreshHookRuns() {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsRefreshingHookRuns(true);
    setError(null);

    try {
      const data = await requestJson<HookRunsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/hooks/runs?limit=50`,
      );
      setHookSettings((current) =>
        current ? { ...current, recentRuns: data.runs } : current,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingHookRuns(false);
    }
  }

  async function saveHookConfig(nextConfig: HookConfig) {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsSavingHooks(true);
    setError(null);
    setHookImportResult(null);

    try {
      const url =
        hookScope === "global" ? "/api/hooks/global" : "/api/hooks/workspace";
      const body =
        hookScope === "global"
          ? { config: nextConfig }
          : { workspaceId, config: nextConfig };
      const data = await requestJson<HooksSettingsResponse>(url, {
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setHookSettings(data);
      setIsHookDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingHooks(false);
    }
  }

  async function submitHookForm(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    try {
      const currentConfig = activeHookConfig ?? emptyHookConfig();
      const nextConfig = upsertHookHandlerInConfig(currentConfig, hookForm);
      await saveHookConfig(nextConfig);
    } catch (formError) {
      setError(errorMessage(formError));
    }
  }

  function updateHookConfig(nextConfig: HookConfig) {
    void saveHookConfig(nextConfig);
  }

  function deleteHookHandler(event: string, groupIndex: number, handlerIndex: number) {
    const nextConfig = deleteHookHandlerFromConfig(
      activeHookConfig ?? emptyHookConfig(),
      event,
      groupIndex,
      handlerIndex,
    );
    updateHookConfig(nextConfig);
  }

  function toggleHookGroup(event: string, groupIndex: number, enabled: boolean) {
    updateHookConfig(
      updateHookGroupInConfig(activeHookConfig ?? emptyHookConfig(), event, groupIndex, {
        enabled,
      }),
    );
  }

  function toggleHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    enabled: boolean,
  ) {
    updateHookConfig(
      updateHookHandlerInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        handlerIndex,
        { enabled },
      ),
    );
  }

  function moveHookGroup(event: string, groupIndex: number, direction: -1 | 1) {
    updateHookConfig(
      moveHookGroupInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        direction,
      ),
    );
  }

  function moveHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    direction: -1 | 1,
  ) {
    updateHookConfig(
      moveHookHandlerInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        handlerIndex,
        direction,
      ),
    );
  }

  async function importClaudeHooks(target: HookScope) {
    const workspaceId = selectedHookWorkspace?.id;
    if (target === "workspace" && !workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsImportingHooks(true);
    setError(null);
    setHookImportResult(null);

    try {
      const data = await requestJson<ImportClaudeHooksResponse>(
        "/api/hooks/import-claude",
        {
          body: JSON.stringify({
            target,
            workspaceId: target === "workspace" ? workspaceId : null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setHookImportResult(data);
      await loadHooks(workspaceId);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsImportingHooks(false);
    }
  }

  async function testHooks(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsTestingHooks(true);
    setError(null);
    setHookTestResult(null);

    try {
      const parsedPayload = parseJsonText(hookTestPayload, t("Sample payload"));
      const data = await requestJson<HookRunSummary>("/api/hooks/test", {
        body: JSON.stringify({
          event: hookTestEvent,
          matchValue: hookTestMatcher.trim() || null,
          payload: parsedPayload,
          workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setHookTestResult(data);
      await loadHooks(workspaceId);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsTestingHooks(false);
    }
  }

  async function openHookRunDetail(runId: string) {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      return;
    }

    setError(null);
    try {
      const data = await requestJson<HookRunDetailResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/hooks/runs/${encodeURIComponent(runId)}`,
      );
      setHookRunDetail(data.run);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  async function testProvider(providerId: string) {
    setProviderTests((current) => ({
      ...current,
      [providerId]: { message: t("Testing connection..."), status: "testing" },
    }));
    setError(null);

    try {
      const data = await requestJson<ProviderTestResponse>(
        "/api/providers/test",
        {
          body: JSON.stringify({ providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setProviderTests((current) => ({
        ...current,
        [providerId]: { message: data.message, status: data.ok ? "ok" : "error" },
      }));
    } catch (requestError) {
      setProviderTests((current) => ({
        ...current,
        [providerId]: {
          message: errorMessage(requestError),
          status: "error",
        },
      }));
    }
  }

  function toggleModelProvider(providerId: string, checked: boolean) {
    setForm((current) => {
      const providerIds = checked
        ? [...current.providerIds, providerId].filter(uniqueString)
        : current.providerIds.filter((id) => id !== providerId);
      const activeProviderId = providerIds.includes(current.activeProviderId)
        ? current.activeProviderId
        : providerIds[0] ?? "";

      return {
        ...current,
        activeProviderId,
        providerIds,
      };
    });
  }

  function toggleSkill(skillId: string, checked: boolean) {
    const next = new Set(enabledSkillIds);

    if (checked) {
      next.add(skillId);
    } else {
      next.delete(skillId);
    }

    setEnabledSkillIds(next);
    void saveSkills(next);
  }

  function handleModelDragStart(
    event: ReactDragEvent<HTMLDivElement>,
    modelId: string,
  ) {
    setDraggedModelId(modelId);
    setModelOrderPreview(orderedConfiguredModels.map((model) => model.id));
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", modelId);
  }

  function handleModelDragOver(
    event: ReactDragEvent<HTMLDivElement>,
    targetModelId: string,
  ) {
    event.preventDefault();

    const sourceModelId = draggedModelId;
    if (!sourceModelId || sourceModelId === targetModelId) {
      return;
    }

    const modelIds = moveItemId(
      modelOrderPreview ?? orderedConfiguredModels.map((model) => model.id),
      sourceModelId,
      targetModelId,
    );
    setModelOrderPreview(modelIds);
  }

  async function handleModelDrop(event: ReactDragEvent<HTMLDivElement>) {
    event.preventDefault();

    const modelIds = modelOrderPreview;
    setDraggedModelId(null);

    if (!modelIds || sameStringList(modelIds, configuredModels.map((model) => model.id))) {
      setModelOrderPreview(null);
      return;
    }

    await saveModelOrder(modelIds);
  }

  function handleModelDragEnd() {
    setDraggedModelId(null);
    setModelOrderPreview(null);
  }

  return (
    <div className="settings-shell panel-scroll min-h-0 flex-1 overflow-y-auto">
      <div className="settings-layout grid">
        <aside className="settings-section-nav-card flex min-h-0 flex-col border-stone-200 bg-white p-2">
          <div className="settings-sidebar-header workspace-sidebar-header flex items-center justify-between gap-2 border-b border-stone-200/80 px-4 py-2">
            <div className="min-w-0">
              <span className="workspace-sidebar-title">{t("Settings")}</span>
            </div>
          </div>
          <nav
            aria-label={t("Settings")}
            className="settings-section-nav flex flex-col gap-1.5"
          >
          <SettingsNavButton
            active={activeSection === "general"}
            icon={Globe}
            label={t("General")}
            onClick={() => onActiveSectionChange("general")}
          />
          <SettingsNavButton
            active={activeSection === "prompts"}
            icon={ScrollText}
            label={t("Prompts")}
            onClick={() => onActiveSectionChange("prompts")}
          />
          <SettingsNavButton
            active={activeSection === "workspaces"}
            icon={Folder}
            label={t("Workspaces")}
            onClick={() => onActiveSectionChange("workspaces")}
          />
          <SettingsNavButton
            active={activeSection === "hooks"}
            icon={Webhook}
            label={t("Hooks")}
            onClick={() => onActiveSectionChange("hooks")}
          />
          <SettingsNavButton
            active={activeSection === "memory"}
            icon={Brain}
            label={t("Memory")}
            onClick={() => onActiveSectionChange("memory")}
          />
          <SettingsNavButton
            active={activeSection === "providers"}
            icon={PlugZap}
            label={t("Providers")}
            onClick={() => onActiveSectionChange("providers")}
          />
          <SettingsNavButton
            active={activeSection === "models"}
            icon={SlidersHorizontal}
            label={t("Models")}
            onClick={() => onActiveSectionChange("models")}
          />
          <SettingsNavButton
            active={activeSection === "mcp"}
            icon={Server}
            label={t("MCP")}
            onClick={() => onActiveSectionChange("mcp")}
          />
          <SettingsNavButton
            active={activeSection === "skills"}
            icon={Wrench}
            label={t("Skills")}
            onClick={() => onActiveSectionChange("skills")}
          />
          </nav>
        </aside>

        <div className="min-w-0 flex flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/75 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-stone-950">
                {settingsSectionTitle(activeSection, t)}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {activeSection === "models"
                  ? metadata?.fetchedAt
                  ? t("Fetched {time} from {source}", {
                      time: metadata.fetchedAt,
                      source: metadata.sourceUrl ?? "",
                    })
                    : t("Model metadata has not been refreshed")
                  : settingsSectionSubtitle(activeSection, t)}
              </p>
            </div>
            {activeSection === "models" ? (
              <button
                aria-label={t("Refresh model metadata")}
                className="inline-flex size-10 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                disabled={isRefreshing}
                onClick={() => void refreshMetadata()}
                title={t("Refresh model metadata")}
                type="button"
              >
                {isRefreshing ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
              </button>
            ) : null}
          </div>
        </section>

        {error ? (
          <div className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        {activeSection === "general" ? (
        <section className="grid gap-4">
          <form
            className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
            onSubmit={(event) => void saveGeneralSettings(event)}
          >
            <div className="flex items-center gap-2">
              <Globe aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Web service")}
              </h3>
            </div>
            <div className="mt-4 grid gap-3">
              <TextField
                label={t("Listen address")}
                onChange={(value) =>
                  setGeneralForm((current) => ({
                    ...current,
                    listenHost: value,
                  }))
                }
                placeholder="127.0.0.1"
                value={generalForm.listenHost}
              />
              <TextField
                inputMode="numeric"
                label={t("Listen port")}
                onChange={(value) =>
                  setGeneralForm((current) => ({
                    ...current,
                    listenPort: value,
                  }))
                }
                placeholder="3210"
                value={generalForm.listenPort}
              />
            </div>
            <div className="mt-4 border-t border-stone-200 pt-4">
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2">
                  <Lock aria-hidden="true" className="size-4 text-teal-700" />
                  <h4 className="text-sm font-semibold text-stone-950">
                    {t("Browser authentication")}
                  </h4>
                </div>
                <CapabilityPill
                  label={
                    settings?.general.webServer.passwordEnabled
                      ? t("Password is enabled")
                      : t("Password is disabled")
                  }
                  ok={Boolean(settings?.general.webServer.passwordEnabled)}
                />
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                {canLogout ? (
                  <button
                    aria-label={t("Log out")}
                    className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                    onClick={() => void onLogout()}
                    title={t("Log out")}
                    type="button"
                  >
                    <Lock aria-hidden="true" className="size-4" />
                    {t("Log out")}
                  </button>
                ) : null}
                <button
                  aria-label={t("Clear browser password")}
                  className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-sm font-semibold text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:border-stone-200 disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={
                    isClearingPassword ||
                    !settings?.general.webServer.passwordEnabled
                  }
                  onClick={() => void clearBrowserPassword()}
                  title={t("Clear browser password")}
                  type="button"
                >
                  {isClearingPassword ? (
                    <LoaderCircle
                      aria-hidden="true"
                      className="size-4 animate-spin"
                    />
                  ) : (
                    <X aria-hidden="true" className="size-4" />
                  )}
                  {t("Clear browser password")}
                </button>
              </div>
              <div className="mt-3 grid gap-3">
                <div className="grid gap-2">
                  <label className="block min-w-0">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Authentication password")}
                    </span>
                    <span className="relative block">
                      <input
                        autoComplete="new-password"
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 pr-10 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setGeneralForm((current) => ({
                            ...current,
                            password: event.target.value,
                          }))
                        }
                        onBlur={() => {
                          if (!generalForm.password) {
                            setIsEditingGeneralPassword(false);
                          }
                        }}
                        onFocus={() => setIsEditingGeneralPassword(true)}
                        placeholder={
                          settings?.general.webServer.passwordEnabled
                            ? t("New password is kept empty unless changed.")
                            : t("Set a password to require browser login.")
                        }
                        type={isGeneralPasswordVisible ? "text" : "password"}
                        value={passwordInputValue}
                      />
                      <button
                        aria-label={
                          isGeneralPasswordVisible
                            ? t("Hide password")
                            : t("Show password")
                        }
                        className="absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                        disabled={!generalForm.password}
                        onClick={() =>
                          setIsGeneralPasswordVisible((current) => !current)
                        }
                        title={
                          isGeneralPasswordVisible
                            ? t("Hide password")
                            : t("Show password")
                        }
                        type="button"
                      >
                        {isGeneralPasswordVisible ? (
                          <EyeOff aria-hidden="true" className="size-4" />
                        ) : (
                          <Eye aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </span>
                    {settings?.general.webServer.passwordEnabled ? (
                      <span className="mt-1 block text-xs text-stone-500">
                        {t(
                          "Saved password cannot be revealed; type a new password to preview it.",
                        )}
                      </span>
                    ) : null}
                  </label>
                </div>
              </div>
            </div>
            <label className="mt-4 block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Language")}
              </span>
              <select
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                disabled={isSavingLanguage || isLoadingSettings}
                onChange={(event) => void saveLanguageSetting(event.target.value)}
                value={generalForm.language}
              >
                {(settings?.general.supportedLanguages ?? []).map((language) => (
                  <option key={language.id} value={language.id}>
                    {language.name}
                  </option>
                ))}
              </select>
              <span className="mt-1 block text-xs text-stone-500">
                {t("Language changes apply immediately after saving.")}
              </span>
            </label>
            <label className="mt-4 block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Theme")}
              </span>
              <select
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                disabled={isSavingTheme || isLoadingSettings}
                onChange={(event) =>
                  void saveThemeSetting(event.target.value as AppThemeId)
                }
                value={generalForm.theme}
              >
                {(settings?.general.supportedThemes ?? []).map((theme) => (
                  <option key={theme.id} value={theme.id}>
                    {t(theme.name)}
                  </option>
                ))}
              </select>
              <span className="mt-1 block text-xs text-stone-500">
                {t("Theme changes apply immediately after saving.")}
              </span>
            </label>
            <div className="mt-4 flex flex-wrap gap-2">
              <button
                aria-label={t("Save general settings")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingGeneral ||
                  !generalForm.listenHost.trim() ||
                  !generalForm.listenPort.trim()
                }
                title={t("Save general settings")}
                type="submit"
              >
                {isSavingGeneral ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
                {t("Save")}
              </button>
              <button
                aria-label={t("Reload general settings")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                disabled={isLoadingSettings}
                onClick={() => void loadSettings()}
                title={t("Reload settings")}
                type="button"
              >
                {isLoadingSettings ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
                {t("Reload")}
              </button>
            </div>
          </form>

          <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Saved bind")}
              </h3>
              <CapabilityPill label={t("restart required")} ok={false} />
            </div>
            <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
              <div className="break-all text-sm font-semibold text-stone-950">
                {settings
                  ? `${settings.general.webServer.listenHost}:${settings.general.webServer.listenPort}`
                  : t("Loading...")}
              </div>
              <div className="mt-2 text-xs text-stone-500">
                {t("Saved host and port are used the next time the backend starts.")}
              </div>
            </div>
          </section>
        </section>
        ) : null}

        {activeSection === "prompts" ? (
        <section className="grid gap-4">
          <form
            className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
            onSubmit={(event) => void savePromptSettings(event)}
          >
            <div className="flex items-center gap-2">
              <ScrollText aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Prompt files")}
              </h3>
            </div>
            <div className="mt-4 grid gap-3">
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("Prompt file path")}
                </span>
                <div className="flex gap-2">
                  <input
                    autoComplete="off"
                    className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    name="prompt-file-path"
                    onChange={(event) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        pendingFile: event.target.value,
                      }))
                    }
                    placeholder="C:/Users/name/.codex/AGENTS.md"
                    value={promptSettingsForm.pendingFile}
                  />
                  <button
                    aria-label={t("Add prompt file")}
                    className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                    disabled={!promptSettingsForm.pendingFile.trim()}
                    onClick={() => addPromptFilePath(promptSettingsForm.pendingFile)}
                    title={t("Add prompt file")}
                    type="button"
                  >
                    <Plus aria-hidden="true" className="size-4" />
                  </button>
                  <button
                    aria-label={t("Choose prompt file")}
                    className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                    disabled={isSelectingPromptFile}
                    onClick={() => void selectPromptFile()}
                    title={t("Choose prompt file")}
                    type="button"
                  >
                    {isSelectingPromptFile ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <FolderSearch aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>
              </label>
              <div className="rounded-xl border border-stone-200 bg-stone-50/80">
                {promptSettingsForm.files.length ? (
                  <div className="divide-y divide-stone-200">
                    {promptSettingsForm.files.map((file) => (
                      <div
                        className="flex min-w-0 items-center justify-between gap-3 px-3 py-2"
                        key={file}
                      >
                        <div className="min-w-0 break-all text-sm font-semibold text-stone-800">
                          {file}
                        </div>
                        <button
                          aria-label={t("Remove prompt file {path}", { path: file })}
                          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                          onClick={() => removePromptFilePath(file)}
                          title={t("Remove prompt file")}
                          type="button"
                        >
                          <Trash2 aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="px-3 py-6 text-center text-sm font-medium text-stone-500">
                    {t("No prompt files")}
                  </div>
                )}
              </div>
            </div>

            <label className="mt-4 block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Extra prompt")}
              </span>
              <textarea
                className="min-h-36 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  setPromptSettingsForm((current) => ({
                    ...current,
                    extraText: event.target.value,
                  }))
                }
                placeholder={t("Extra prompt")}
                value={promptSettingsForm.extraText}
              />
            </label>

            <div className="mt-4 flex flex-wrap gap-2">
              <button
                aria-label={t("Save prompt settings")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={isSavingPromptSettings}
                title={t("Save prompt settings")}
                type="submit"
              >
                {isSavingPromptSettings ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
                {t("Save")}
              </button>
              <button
                aria-label={t("Reload prompt settings")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                disabled={isLoadingSettings}
                onClick={() => void loadSettings()}
                title={t("Reload settings")}
                type="button"
              >
                {isLoadingSettings ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
                {t("Reload")}
              </button>
            </div>
          </form>
        </section>
        ) : null}

        {activeSection === "memory" ? (
        <section className="grid gap-4">
          {isMemoryDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={
                  memoryDialogMode === "create"
                    ? t("Create memory")
                    : t("Edit memory")
                }
                className={`fixed left-1/2 top-1/2 z-50 max-h-[88vh] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)] ${
                  memoryDialogMode === "edit" ? "w-[min(94vw,72rem)]" : "w-[min(92vw,34rem)]"
                }`}
                onSubmit={(event) => void saveMemoryDialog(event)}
                role="dialog"
              >
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      {memoryDialogMode === "create" ? (
                        <Plus aria-hidden="true" className="size-5 text-teal-700" />
                      ) : (
                        <Pencil aria-hidden="true" className="size-5 text-teal-700" />
                      )}
                      <h3 className="text-sm font-semibold text-stone-950">
                        {memoryDialogMode === "create"
                          ? t("Create memory")
                          : t("Edit memory")}
                      </h3>
                    </div>
                    <div className="mt-1 truncate text-xs text-stone-500">
                      {memoryScopeLabel(manualMemoryForm.scope, t)}
                    </div>
                  </div>
                  <button
                    aria-label={t("Close memory dialog")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                    onClick={closeMemoryDialog}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                </div>
                <div
                  className={
                    memoryDialogMode === "edit"
                      ? "grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]"
                      : "grid gap-3"
                  }
                >
                  <div className="grid min-w-0 gap-3">
                  {memoryDialogMode === "create" ? (
                    <>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Memory scope")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setManualMemoryForm((current) => ({
                              ...current,
                              scope: event.target.value as ManualMemoryFormState["scope"],
                            }))
                          }
                          value={manualMemoryForm.scope}
                        >
                          <option value="global">{t("Global memory")}</option>
                          <option value="workspace">{t("Workspace memory")}</option>
                          <option value="chat">{t("Chat memory")}</option>
                        </select>
                      </label>
                      {manualMemoryForm.scope !== "global" ? (
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Workspace")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setManualMemoryForm((current) => ({
                                ...current,
                                workspaceId: event.target.value,
                              }))
                            }
                            value={
                              manualMemoryForm.workspaceId ||
                              memoryDialogWorkspace?.id ||
                              ""
                            }
                          >
                            {workspaces.map((workspace) => (
                              <option key={workspace.id} value={workspace.id}>
                                {workspace.name}
                              </option>
                            ))}
                          </select>
                        </label>
                      ) : null}
                      {manualMemoryForm.scope === "chat" ? (
                        <TextField
                          label={t("Chat ID")}
                          onChange={(value) =>
                            setManualMemoryForm((current) => ({
                              ...current,
                              chatId: value,
                            }))
                          }
                          placeholder="chat-..."
                          value={manualMemoryForm.chatId}
                        />
                      ) : null}
                    </>
                  ) : null}
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory kind")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setManualMemoryForm((current) => ({
                          ...current,
                          kind: event.target.value,
                        }))
                      }
                      value={manualMemoryForm.kind}
                    >
                      {MEMORY_KIND_OPTIONS.map((kind) => (
                        <option key={kind} value={kind}>
                          {memoryKindLabel(kind, t)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <TextField
                      inputMode="numeric"
                      label={t("Confidence")}
                      onChange={(value) =>
                        setManualMemoryForm((current) => ({
                          ...current,
                          confidence: value,
                        }))
                      }
                      placeholder="0.8"
                      value={manualMemoryForm.confidence}
                    />
                    <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2 sm:mt-6">
                      <span className="text-sm font-semibold text-stone-700">
                        {t("Pinned memory")}
                      </span>
                      <input
                        checked={manualMemoryForm.pinned}
                        className="size-4 accent-teal-700"
                        onChange={(event) =>
                          setManualMemoryForm((current) => ({
                            ...current,
                            pinned: event.target.checked,
                          }))
                        }
                        type="checkbox"
                      />
                    </label>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory fact")}
                    </span>
                    <textarea
                      className="min-h-32 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setManualMemoryForm((current) => ({
                          ...current,
                          fact: event.target.value,
                        }))
                      }
                      value={manualMemoryForm.fact}
                    />
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory metadata")}
                    </span>
                    <textarea
                      className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setManualMemoryForm((current) => ({
                          ...current,
                          metadataText: event.target.value,
                        }))
                      }
                      spellCheck={false}
                      value={manualMemoryForm.metadataText}
                    />
                  </label>
                  {memoryDialogMode === "edit" && selectedMemory ? (
                    <div className="grid gap-2 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3 text-xs text-stone-600">
                      <div className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                        {t("Memory details")}
                      </div>
                      <div className="grid gap-2 sm:grid-cols-2">
                        <div className="break-all">
                          <span className="font-semibold text-stone-700">ID: </span>
                          {selectedMemory.id}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Memory status")}:{" "}
                          </span>
                          {memoryStatusLabel(selectedMemory.status, t)}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Memory scope")}:{" "}
                          </span>
                          {memoryScopeLabel(selectedMemory.scope, t)}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Chat ID")}:{" "}
                          </span>
                          {selectedMemory.chatId ?? "-"}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Latest")}:{" "}
                          </span>
                          {selectedMemory.isLatest ? t("Yes") : t("No")}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Expires at")}:{" "}
                          </span>
                          {selectedMemory.expiresAt ?? "-"}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Created")}:{" "}
                          </span>
                          {selectedMemory.createdAt}
                        </div>
                        <div>
                          <span className="font-semibold text-stone-700">
                            {t("Updated")}:{" "}
                          </span>
                          {selectedMemory.updatedAt}
                        </div>
                      </div>
                    </div>
                  ) : null}
                  </div>
                  {memoryDialogMode === "edit" ? (
                    <div className="grid min-w-0 gap-2 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-2">
                      <div className="flex items-center justify-between gap-2">
                        <h4 className="text-xs font-semibold text-stone-600">
                          {t("Memory source details")}
                        </h4>
                        <span className="font-mono text-[11px] font-semibold text-stone-400">
                          {memorySourceForms.length}
                        </span>
                      </div>
                      {memorySourceForms.length === 0 ? (
                        <div className="rounded-lg border border-dashed border-stone-300 bg-white px-3 py-6 text-center text-sm font-medium text-stone-500">
                          {t("No memory sources")}
                        </div>
                      ) : (
                        <div className="grid max-h-[58vh] gap-3 overflow-y-auto pr-1">
                          {memorySourceForms.map((source, index) => (
                            <div
                              className="grid gap-3 rounded-xl border border-stone-200 bg-white px-3 py-3"
                              key={source.id}
                            >
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <div className="min-w-0">
                                  <div className="text-xs font-semibold text-stone-500">
                                    {t("Memory sources")} #{index + 1}
                                  </div>
                                  <div className="mt-1 break-all font-mono text-[11px] text-stone-400">
                                    {source.id}
                                  </div>
                                </div>
                              </div>
                              <TextField
                                label={t("Source title")}
                                onChange={(value) =>
                                  updateMemorySourceForm(source.id, "title", value)
                                }
                                placeholder={t("Source title")}
                                value={source.title}
                              />
                              <MemorySourceReadonlyDetails
                                source={memorySources.find((item) => item.id === source.id)}
                                t={t}
                              />
                              <div className="grid gap-1.5">
                                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                  {t("Source content")}
                                </span>
                                <SourceValueEditor
                                  id={`${source.id}:content`}
                                  isExpanded={expandedMemoryJsonIds.has(`${source.id}:content`)}
                                  minHeightClass="min-h-28"
                                  onChange={(value) =>
                                    updateMemorySourceForm(source.id, "content", value)
                                  }
                                  onToggle={toggleMemoryJson}
                                  t={t}
                                  title={t("Source content")}
                                  value={source.content}
                                />
                              </div>
                              <div className="grid gap-1.5">
                                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                  {t("Source metadata")}
                                </span>
                                <SourceValueEditor
                                  id={`${source.id}:metadata`}
                                  isExpanded={expandedMemoryJsonIds.has(`${source.id}:metadata`)}
                                  minHeightClass="min-h-24"
                                  onChange={(value) =>
                                    updateMemorySourceForm(source.id, "metadataText", value)
                                  }
                                  onToggle={toggleMemoryJson}
                                  t={t}
                                  title={t("Source metadata")}
                                  value={source.metadataText}
                                />
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ) : null}
                  <button
                    aria-label={
                      memoryDialogMode === "create"
                        ? t("Create memory")
                        : t("Save memory")
                    }
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 xl:col-span-2"
                    disabled={
                      isSavingMemory ||
                      !manualMemoryForm.fact.trim() ||
                      (manualMemoryForm.scope === "chat" &&
                        !manualMemoryForm.chatId.trim())
                    }
                    title={
                      memoryDialogMode === "create"
                        ? t("Create memory")
                        : t("Save memory")
                    }
                    type="submit"
                  >
                    {isSavingMemory ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : memoryDialogMode === "create" ? (
                      <Plus aria-hidden="true" className="size-4" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {memoryDialogMode === "create"
                      ? t("Create memory")
                      : t("Save memory")}
                  </button>
                </div>
              </form>
            </>
          ) : null}

          <form
            className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
            onSubmit={(event) => void saveMemorySettings(event)}
          >
            <div className="flex items-center gap-2">
              <Bot aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Memory controls")}
              </h3>
            </div>
            <div className="mt-4 grid gap-3">
              <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                <legend className="px-1 text-xs font-semibold text-stone-600">
                  {t("General memory control")}
                </legend>
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <div>
                    <p className="text-sm font-semibold text-stone-800">
                      {t("Enable memory")}
                    </p>
                    <p className="mt-1 text-xs text-stone-500">
                      {t(
                        "Controls whether memory tools, retrieval, and extraction are available.",
                      )}
                    </p>
                  </div>
                  <label
                    aria-label={t("Enable memory")}
                    className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                  >
                    <input
                      checked={memorySettingsForm.enabled}
                      className="size-4 accent-teal-700"
                      onChange={(event) =>
                        setMemorySettingsForm((current) => ({
                          ...current,
                          enabled: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                  </label>
                </div>
              </fieldset>

              <div className="grid gap-3 xl:grid-cols-2">
                <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                  <legend className="px-1 text-xs font-semibold text-stone-600">
                    {t("Memory extraction")}
                  </legend>
                  <div className="mb-3 flex items-start gap-2">
                    <SlidersHorizontal
                      aria-hidden="true"
                      className="mt-0.5 size-4 shrink-0 text-teal-700"
                    />
                    <p className="text-xs text-stone-500">
                      {t(
                        "Controls how new facts are extracted and how long they are retained.",
                      )}
                    </p>
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Extraction mode")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setMemorySettingsForm((current) => ({
                            ...current,
                            extractionMode: event.target.value,
                          }))
                        }
                        value={memorySettingsForm.extractionMode}
                      >
                        {(settings?.memory.extractionModes ?? []).map((mode) => (
                          <option key={mode.value} value={mode.value}>
                            {t(mode.label)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Extraction model")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setMemorySettingsForm((current) => ({
                            ...current,
                            extractionModelId: event.target.value,
                          }))
                        }
                        value={memorySettingsForm.extractionModelId}
                      >
                        <option value="">{t("Current chat model")}</option>
                        {(settings?.configuredModels ?? []).map((model) => (
                          <option key={model.id} value={model.id}>
                            {model.displayName}
                          </option>
                        ))}
                      </select>
                    </label>
                    <div className="sm:col-span-2">
                      <TextField
                        inputMode="numeric"
                        label={t("Retention days")}
                        onChange={(value) =>
                          setMemorySettingsForm((current) => ({
                            ...current,
                            retentionDays: value,
                          }))
                        }
                        placeholder="90"
                        value={memorySettingsForm.retentionDays}
                      />
                    </div>
                  </div>
                </fieldset>

                <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                  <legend className="px-1 text-xs font-semibold text-stone-600">
                    {t("Memory retrieval")}
                  </legend>
                  <div className="mb-3 flex items-start gap-2">
                    <Brain
                      aria-hidden="true"
                      className="mt-0.5 size-4 shrink-0 text-teal-700"
                    />
                    <p className="text-xs text-stone-500">
                      {t("Controls how existing memory is matched into chat context.")}
                    </p>
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Memory matching")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setMemorySettingsForm((current) => ({
                            ...current,
                            retrievalMode: event.target.value,
                          }))
                        }
                        value={memorySettingsForm.retrievalMode}
                      >
                        {(settings?.memory.retrievalModes ?? []).map((mode) => (
                          <option key={mode.value} value={mode.value}>
                            {t(mode.label)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Matching model")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setMemorySettingsForm((current) => ({
                            ...current,
                            retrievalModelId: event.target.value,
                          }))
                        }
                        value={memorySettingsForm.retrievalModelId}
                      >
                        <option value="">{t("Current chat model")}</option>
                        {(settings?.configuredModels ?? []).map((model) => (
                          <option key={model.id} value={model.id}>
                            {model.displayName}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                </fieldset>
              </div>
            </div>
            <button
              aria-label={t("Save memory settings")}
              className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
              disabled={isSavingMemorySettings}
              title={t("Save memory settings")}
              type="submit"
            >
              {isSavingMemorySettings ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <CheckCircle2 aria-hidden="true" className="size-4" />
              )}
              {t("Save")}
            </button>
          </form>

          <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="min-w-0">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Memory list")}
                  </h3>
                  <p className="mt-1 truncate text-xs text-stone-500">
                    {memoryScopeLabel(memoryFilter.scope, t)}
                  </p>
                </div>
                <div className="flex flex-wrap items-center justify-end gap-2">
                  {memoryFilter.scope !== "global" ? (
                    <button
                      aria-label={clearFilteredMemoryLabel}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-sm font-semibold text-rose-700 hover:bg-rose-50 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={!canClearFilteredMemories || isSavingMemory}
                      onClick={() => void clearFilteredMemories()}
                      title={clearFilteredMemoryLabel}
                      type="button"
                    >
                      <Trash2 aria-hidden="true" className="size-4" />
                      {clearFilteredMemoryLabel}
                    </button>
                  ) : null}
                  <button
                    aria-label={t("Create memory")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900"
                    onClick={openCreateMemoryDialog}
                    title={t("Create memory")}
                    type="button"
                  >
                    <Plus aria-hidden="true" className="size-4" />
                    {t("Create memory")}
                  </button>
                </div>
              </div>
              <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(9rem,0.8fr)_minmax(9rem,0.8fr)_minmax(8rem,0.7fr)_minmax(0,1.4fr)_auto]">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Memory scope")}
                  </span>
                  <select
                    aria-label={t("Memory scope")}
                    className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) =>
                      updateMemoryFilter({
                        scope: event.target.value as MemoryFilterState["scope"],
                        workspaceId:
                          event.target.value === "global"
                            ? ""
                            : memoryFilter.workspaceId || memoryWorkspace?.id || "",
                      })
                    }
                    value={memoryFilter.scope}
                  >
                    <option value="global">{t("Global memory")}</option>
                    <option value="workspace">{t("Workspace memory")}</option>
                    <option value="chat">{t("Chat memory")}</option>
                  </select>
                </label>
                {memoryFilter.scope !== "global" ? (
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Workspace")}
                    </span>
                    <select
                      aria-label={t("Workspace")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateMemoryFilter({
                          workspaceId: event.target.value,
                        })
                      }
                      value={memoryFilter.workspaceId || memoryWorkspace?.id || ""}
                    >
                      {workspaces.map((workspace) => (
                        <option key={workspace.id} value={workspace.id}>
                          {workspace.name}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : null}
                {memoryFilter.scope === "chat" ? (
                  <TextField
                    label={t("Chat ID")}
                    onChange={(value) =>
                      updateMemoryFilter({
                        chatId: value,
                      })
                    }
                    placeholder="chat-..."
                    value={memoryFilter.chatId}
                  />
                ) : null}
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Memory kind")}
                  </span>
                  <select
                    aria-label={t("Memory kind")}
                    className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) =>
                      updateMemoryFilter({
                        kind: event.target.value,
                      })
                    }
                    value={memoryFilter.kind}
                  >
                    <option value="">{t("All memory kinds")}</option>
                    {MEMORY_KIND_OPTIONS.map((kind) => (
                      <option key={kind} value={kind}>
                        {memoryKindLabel(kind, t)}
                      </option>
                    ))}
                  </select>
                </label>
                <TextField
                  label={t("Search memories")}
                  onChange={(value) =>
                    updateMemoryFilter({
                      query: value,
                    })
                  }
                  placeholder={t("Search memories")}
                  value={memoryFilter.query}
                />
                <div className="flex items-end gap-2">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory status")}
                    </span>
                    <select
                      aria-label={t("Memory status")}
                      className="h-10 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateMemoryFilter({
                          status: event.target.value as MemoryFilterState["status"],
                        })
                      }
                      value={memoryFilter.status}
                    >
                      <option value="active">{t("Active")}</option>
                      <option value="pending">{t("Pending review")}</option>
                    </select>
                  </label>
                  <button
                    aria-label={t("Refresh memories")}
                    className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    onClick={() => void loadMemories()}
                    title={t("Refresh memories")}
                    type="button"
                  >
                    {isLoadingMemories ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <RefreshCw aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>
              </div>
              <div className="mt-4 grid gap-3">
                {memories.length === 0 ? (
                  <div className="rounded-xl border border-dashed border-stone-300 bg-stone-50 px-3 py-6 text-center text-sm font-medium text-stone-500">
                    {t("No memories")}
                  </div>
                ) : (
                  memories.map((memory) => (
                    <div
                      className={`grid gap-3 rounded-xl border px-3 py-3 sm:grid-cols-[minmax(0,1fr)_auto] ${
                        selectedMemoryId === memory.id
                          ? "border-teal-200 bg-teal-50/80"
                          : "border-stone-200 bg-white hover:border-teal-100 hover:bg-stone-50"
                      }`}
                      key={memory.id}
                    >
                      <button
                        className="min-w-0 text-left"
                        onClick={() => setSelectedMemoryId(memory.id)}
                        type="button"
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <CapabilityPill
                            label={memoryStatusLabel(memory.status, t)}
                            ok={memory.status === "active"}
                          />
                          <CapabilityPill
                            label={memoryKindLabel(memory.kind, t)}
                            ok={memory.pinned}
                          />
                          {memory.scope === "chat" && memory.chatId ? (
                            <span className="text-xs font-semibold text-stone-500">
                              {memory.chatId}
                            </span>
                          ) : null}
                        </div>
                        <div className="mt-2 break-words text-sm font-semibold text-stone-900">
                          {memory.fact}
                        </div>
                        <div className="mt-2 text-xs text-stone-500">
                          {memory.updatedAt}
                        </div>
	                      </button>
	                      <div className="flex items-start justify-end gap-2">
	                        {memory.scope !== "global" ? (
	                          <button
	                            aria-label={t("Promote one level")}
	                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
	                            onClick={() => promoteMemoryOneLevel(memory)}
	                            title={
	                              memory.scope === "chat"
	                                ? t("Promote to workspace")
	                                : t("Promote to global")
	                            }
	                            type="button"
	                          >
	                            <ArrowUp aria-hidden="true" className="size-4" />
	                          </button>
	                        ) : null}
	                        <button
	                          aria-label={t("Edit memory")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                          onClick={() => openEditMemoryDialog(memory)}
                          title={t("Edit memory")}
                          type="button"
                        >
                          <Pencil aria-hidden="true" className="size-4" />
                        </button>
                        <button
                          aria-label={t("Delete memory")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                          onClick={() => void forgetMemory(memory.id)}
                          title={t("Delete memory")}
                          type="button"
                        >
                          <Trash2 aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
              <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 pt-3 text-sm">
                <div className="text-stone-500">
                  {t("Showing {start}-{end} of {total}", {
                    end: formatNumber(memoryPageEnd, language),
                    start: formatNumber(memoryPageStart, language),
                    total: formatNumber(memoryListMeta.totalCount, language),
                  })}
                </div>
                <div className="flex flex-wrap items-center justify-end gap-3">
                  <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                    <span>{t("Page size")}</span>
                    <input
                      className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      max={200}
                      min={1}
                      onChange={(event) => updateMemoryPageSize(event.target.value)}
                      type="number"
                      value={memoryFilter.pageSize}
                    />
                  </label>
                  <nav
                    aria-label={t("Memory pagination")}
                    className="flex items-center gap-1"
                  >
                    <button
                      aria-label={t("Previous page")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={
                        !isMemoryFilterReady ||
                        isLoadingMemories ||
                        memoryListMeta.page <= 1
                      }
                      onClick={() => goToMemoryPage(memoryListMeta.page - 1)}
                      title={t("Previous page")}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" className="size-4" />
                    </button>
                    {memoryPaginationItems.map((item, index) =>
                      item === "ellipsis" ? (
                        <span
                          aria-hidden="true"
                          className="inline-flex size-9 items-center justify-center text-stone-400"
                          key={`memory-ellipsis-${index}`}
                        >
                          ...
                        </span>
                      ) : (
                        <button
                          aria-current={
                            item === memoryListMeta.page ? "page" : undefined
                          }
                          aria-label={t("Go to page {page}", {
                            page: formatNumber(item, language),
                          })}
                          className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                            item === memoryListMeta.page
                              ? "border-teal-700 bg-teal-700 text-white"
                              : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                          }`}
                          disabled={!isMemoryFilterReady || isLoadingMemories}
                          key={item}
                          onClick={() => goToMemoryPage(item)}
                          title={t("Go to page {page}", {
                            page: formatNumber(item, language),
                          })}
                          type="button"
                        >
                          {formatNumber(item, language)}
                        </button>
                      ),
                    )}
                    <button
                      aria-label={t("Next page")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={
                        isLoadingMemories ||
                        !isMemoryFilterReady ||
                        memoryListMeta.totalPages === 0 ||
                        memoryListMeta.page >= memoryListMeta.totalPages
                      }
                      onClick={() => goToMemoryPage(memoryListMeta.page + 1)}
                      title={t("Next page")}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" className="size-4" />
                    </button>
                  </nav>
                </div>
              </div>
              <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                <div className="flex items-center gap-2">
                  <CircleAlert aria-hidden="true" className="size-4 text-rose-700" />
                  <h4 className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                    {t("Extraction failures")}
                  </h4>
                </div>
                <div className="mt-2 grid gap-2">
                  {memoryExtractionJobs.length === 0 ? (
                    <div className="text-sm text-stone-500">
                      {t("No extraction failures")}
                    </div>
                  ) : (
                    memoryExtractionJobs.map((job) => (
                      <div
                        className="rounded-lg border border-rose-100 bg-white px-3 py-2"
                        key={job.id}
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <CapabilityPill label={job.status} ok={false} />
                          <CapabilityPill
                            label={job.modelId ?? t("Default")}
                            ok={false}
                          />
                          {job.chatId ? (
                            <span className="text-xs font-semibold text-stone-500">
                              {job.chatId}
                            </span>
                          ) : null}
                        </div>
                        <div className="mt-2 text-sm font-semibold text-rose-700">
                          {job.errorMessage ?? t("Memory extraction failed")}
                        </div>
                        <div className="mt-1 text-xs text-stone-500">
                          {job.completedAt ?? job.startedAt ?? job.createdAt}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
              {selectedMemory?.status === "pending" ? (
                <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                  <div className="flex flex-wrap gap-2">
                    <button
                      className="inline-flex h-9 items-center gap-2 rounded-lg bg-teal-800 px-3 text-xs font-semibold text-white hover:bg-teal-900"
                      onClick={() => void setMemoryStatus(selectedMemory.id, "active")}
                      type="button"
                    >
                      <CheckCircle2 aria-hidden="true" className="size-3.5" />
                      {t("Approve memory")}
                    </button>
                    <button
                      className="inline-flex h-9 items-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-xs font-semibold text-rose-700 hover:bg-rose-50"
                      onClick={() => void setMemoryStatus(selectedMemory.id, "rejected")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                      {t("Reject memory")}
                    </button>
                  </div>
                </div>
              ) : null}
          </section>
        </section>
        ) : null}

        {activeSection === "workspaces" ? (
        <section className="grid gap-4">
          {isWorkspaceDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={t("Workspace configuration")}
                className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                onSubmit={(event) => void saveWorkspace(event)}
              >
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Folder aria-hidden="true" className="size-5 text-teal-700" />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {t("Edit workspace")}
                      </h3>
                    </div>
                    {editingWorkspace ? (
                      <div className="mt-1 truncate text-xs text-stone-500">
                        {editingWorkspace.path}
                      </div>
                    ) : null}
                  </div>
                  <button
                    aria-label={t("Close workspace configuration")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                    onClick={() => setIsWorkspaceDialogOpen(false)}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                </div>
                <div className="space-y-3">
                  <TextField
                    label={t("Workspace name")}
                    onChange={(value) =>
                      setWorkspaceForm((current) => ({
                        ...current,
                        name: value,
                      }))
                    }
                    placeholder={t("Workspace name")}
                    value={workspaceForm.name}
                  />
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Path")}
                    </span>
                    <div className="flex gap-2">
                      <input
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        name="workspace-path"
                        onChange={(event) =>
                          setWorkspaceForm((current) => ({
                            ...current,
                            path: event.target.value,
                          }))
                        }
                        placeholder="C:/Users/name/workspace"
                        value={workspaceForm.path}
                      />
                      <button
                        aria-label={t("Choose workspace path")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={isSelectingWorkspaceFormPath}
                        onClick={() => void selectWorkspaceFormPath()}
                        title={t("Choose workspace path")}
                        type="button"
                      >
                        {isSelectingWorkspaceFormPath ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : (
                          <FolderSearch aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </div>
                  </label>
                  <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
                    <div className="mb-3 flex items-center justify-between gap-3">
                      <div className="flex min-w-0 items-center gap-2">
                        <WorkspaceIcon
                          className="size-10 rounded-lg border border-stone-200 bg-white object-cover p-1"
                          fallbackClassName="size-10 rounded-lg border border-stone-200 bg-white p-2 text-teal-700"
                          logoUrl={editingWorkspace?.logoUrl ?? null}
                        />
                        <div className="min-w-0">
                          <span className="block text-sm font-semibold text-stone-800">
                            {t("Workspace icon")}
                          </span>
                          <span className="block truncate text-xs text-stone-500">
                            {editingWorkspace?.logoUrl
                              ? t("Custom icon")
                              : t("Folder icon")}
                          </span>
                        </div>
                      </div>
                      <button
                        aria-label={t("Clear workspace icon")}
                        className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-600 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700 disabled:cursor-not-allowed disabled:text-stone-300"
                        disabled={isSavingWorkspaceLogo || !editingWorkspace?.logoUrl}
                        onClick={() => void clearWorkspaceLogo()}
                        title={t("Clear workspace icon")}
                        type="button"
                      >
                        {isSavingWorkspaceLogo ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : (
                          <Trash2 aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </div>
                    <input
                      aria-label={t("Workspace icon file")}
                      accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                      className="sr-only"
                      onChange={handleWorkspaceLogoFileChange}
                      ref={workspaceLogoInputRef}
                      type="file"
                    />
                    <button
                      aria-label={t("Upload icon")}
                      className="mt-2 inline-flex h-9 items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                      disabled={isSavingWorkspaceLogo}
                      onClick={() => workspaceLogoInputRef.current?.click()}
                      title={t("Upload icon")}
                      type="button"
                    >
                      <Upload aria-hidden="true" className="size-3.5" />
                      {t("Upload icon")}
                    </button>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Terminal shell")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setWorkspaceForm((current) => ({
                          ...current,
                          terminalShell: event.target.value,
                        }))
                      }
                      value={workspaceForm.terminalShell || terminalShells[0]?.shell || ""}
                    >
                      {terminalShells.map((shell) => (
                        <option key={shell.shell} value={shell.shell}>
                          {shell.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
                    <div className="mb-3 flex items-center justify-between gap-3">
                      <span className="text-sm font-semibold text-stone-700">
                        {t("Common commands")}
                      </span>
                      <button
                        aria-label={t("Add command")}
                        className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        onClick={addWorkspaceCommonCommand}
                        title={t("Add command")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    {workspaceForm.commonCommands.length ? (
                      <div className="space-y-2">
                        <div className="grid gap-2 pr-10 text-xs font-semibold text-stone-500 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)]">
                          <span>{t("Command name")}</span>
                          <span>{t("Command")}</span>
                        </div>
                        {workspaceForm.commonCommands.map((command, index) => (
                          <div
                            className="grid items-center gap-2 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)_2.25rem]"
                            key={index}
                          >
                            <input
                              aria-label={t("Command name")}
                              autoComplete="off"
                              className="h-9 min-w-0 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                updateWorkspaceCommonCommand(
                                  index,
                                  "name",
                                  event.target.value,
                                )
                              }
                              placeholder={t("Command name")}
                              value={command.name}
                            />
                            <input
                              aria-label={t("Command")}
                              autoComplete="off"
                              className="h-9 min-w-0 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                updateWorkspaceCommonCommand(
                                  index,
                                  "command",
                                  event.target.value,
                                )
                              }
                              placeholder="npm run dev"
                              value={command.command}
                            />
                            <button
                              aria-label={t("Remove command {name}", {
                                name: command.name || String(index + 1),
                              })}
                              className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-500 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                              onClick={() => removeWorkspaceCommonCommand(index)}
                              title={t("Remove command")}
                              type="button"
                            >
                              <Trash2 aria-hidden="true" className="size-4" />
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : null}
                  </div>
                  <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                    <span className="text-sm font-semibold text-stone-700">
                      {t("Pinned workspace")}
                    </span>
                    <input
                      checked={workspaceForm.pinned}
                      className="size-4 accent-teal-700"
                      onChange={(event) =>
                        setWorkspaceForm((current) => ({
                          ...current,
                          pinned: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                  </label>
                  <button
                    aria-label={t("Save workspace")}
                    className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={
                      isSavingWorkspace ||
                      !workspaceForm.name.trim() ||
                      !workspaceForm.path.trim()
                    }
                    title={t("Save workspace")}
                    type="submit"
                  >
                    {isSavingWorkspace ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                </div>
              </form>
            </>
          ) : null}

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Workspace list")}
              </h3>
              <button
                aria-label={t("Add workspace")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                onClick={onAddWorkspace}
                title={t("Add workspace")}
                type="button"
              >
                <Plus aria-hidden="true" className="size-4" />
              </button>
            </div>
            <div className="divide-y divide-stone-100">
              {orderedWorkspaces.length ? (
                orderedWorkspaces.map((workspace) => (
                  <div
                    className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${
                      draggedWorkspaceId === workspace.id
                        ? "bg-teal-50/70 opacity-80"
                        : "bg-white/0"
                    }`}
                    key={workspace.id}
                    onDragOver={(event) =>
                      handleWorkspaceDragOver(event, workspace.id)
                    }
                    onDrop={(event) => void handleWorkspaceDrop(event)}
                  >
                    <div className="flex items-center">
                      <span
                        aria-label={t("Reorder workspace {name}", {
                          name: workspace.name,
                        })}
                        className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${
                          isSavingWorkspaceOrder
                            ? "cursor-not-allowed opacity-60"
                            : "cursor-grab hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        }`}
                        title={t("Reorder workspace {name}", {
                          name: workspace.name,
                        })}
                        draggable={!isSavingWorkspaceOrder}
                        onDragEnd={handleWorkspaceDragEnd}
                        onDragStart={(event) =>
                          handleWorkspaceDragStart(event, workspace.id)
                        }
                      >
                        {isSavingWorkspaceOrder && draggedWorkspaceId === workspace.id ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : (
                          <GripVertical aria-hidden="true" className="size-4" />
                        )}
                      </span>
                    </div>
                    <div className="flex min-w-0 items-center gap-3 select-text">
                      <WorkspaceIcon
                        className="size-9 shrink-0 rounded-lg border border-stone-200 object-cover shadow-sm"
                        fallbackClassName="size-9 shrink-0 rounded-lg border border-stone-200 bg-stone-50 p-2 text-stone-500 shadow-sm"
                        logoUrl={workspace.logoUrl}
                      />
                      <div className="min-w-0">
                        <div className="flex min-w-0 items-center gap-2">
                          <span className="min-w-0 truncate text-sm font-semibold">
                            {workspace.name}
                          </span>
                          {workspace.isDefault ? (
                            <CapabilityPill label={t("Default workspace")} ok />
                          ) : null}
                          {workspace.pinned ? (
                            <CapabilityPill label={t("pinned")} ok />
                          ) : null}
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          <span className="font-medium">
                            {terminalShellLabel(terminalShells, workspace.terminalShell)}
                          </span>
                          <span className="text-stone-300"> / </span>
                          <span>{workspace.path}</span>
                        </div>
                      </div>
                    </div>
                    <div className="flex gap-2 justify-end">
                      <button
                        aria-label={t(
                          workspace.pinned
                            ? "Unpin workspace {name}"
                            : "Pin workspace {name}",
                          { name: workspace.name },
                        )}
                        className={`inline-flex size-9 items-center justify-center rounded-lg border shadow-sm ${
                          workspace.pinned
                            ? "border-teal-300 bg-teal-700 text-white shadow-[0_10px_22px_rgba(15,118,110,0.22)] hover:bg-teal-800"
                            : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        }`}
                        disabled={isSavingWorkspaceOrder}
                        onClick={() =>
                          void toggleWorkspacePinned(workspace, !workspace.pinned)
                        }
                        title={t(workspace.pinned ? "Unpin workspace" : "Pin workspace")}
                        type="button"
                      >
                        <Lock aria-hidden="true" className="size-4" />
                      </button>
                      <button
                        aria-label={t("Edit workspace {name}", {
                          name: workspace.name,
                        })}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        onClick={() => editConfiguredWorkspace(workspace)}
                        title={t("Edit workspace")}
                        type="button"
                      >
                        <Pencil aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No workspaces")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}

        {activeSection === "hooks" ? (
        <section className="grid gap-4">
          {hookRunDetail ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <div
                aria-label={t("Hook run detail")}
                className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,46rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                role="dialog"
              >
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Webhook aria-hidden="true" className="size-5 text-teal-700" />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {hookEventLabel(hookRunDetail.event, t)}
                      </h3>
                    </div>
                    <div className="mt-1 truncate text-xs text-stone-500">
                      {hookRunDetail.id}
                    </div>
                  </div>
                  <button
                    aria-label={t("Close hook run detail")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                    onClick={() => setHookRunDetail(null)}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                </div>
                <div className="grid gap-3">
                  <div className="grid gap-2 sm:grid-cols-3">
                    <CapabilityPill
                      label={hookRunStatusLabel(hookRunDetail.status, t)}
                      ok={hookRunDetail.status === "succeeded"}
                    />
                    <CapabilityPill
                      label={hookSourceLabel(hookRunDetail.hookSource, t)}
                      ok={hookRunDetail.hookSource === "global"}
                    />
                    <CapabilityPill
                      label={hookHandlerTypeLabel(hookRunDetail.handlerType, t)}
                      ok
                    />
                  </div>
                  {hookRunDetail.stdoutPreview ? (
                    <pre className="max-h-32 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                      {hookRunDetail.stdoutPreview}
                    </pre>
                  ) : null}
                  {hookRunDetail.stderrPreview ? (
                    <pre className="max-h-32 overflow-auto rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700">
                      {hookRunDetail.stderrPreview}
                    </pre>
                  ) : null}
                  <div className="grid gap-3 lg:grid-cols-2">
                    <pre className="max-h-80 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                      {JSON.stringify(hookRunDetail.input, null, 2)}
                    </pre>
                    <pre className="max-h-80 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                      {JSON.stringify(hookRunDetail.output, null, 2)}
                    </pre>
                  </div>
                </div>
              </div>
            </>
          ) : null}

          {isHookDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={t("Hook configuration")}
                className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,40rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                onSubmit={(event) => void submitHookForm(event)}
              >
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Webhook aria-hidden="true" className="size-5 text-teal-700" />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {hookForm.handlerIndex === null
                          ? t("Add hook")
                          : t("Edit hook")}
                      </h3>
                    </div>
                    <div className="mt-1 truncate text-xs text-stone-500">
                      {hookScope === "global" ? t("Global hooks") : selectedHookWorkspace?.name}
                    </div>
                  </div>
                  <button
                    aria-label={t("Close hook configuration")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                    onClick={() => setIsHookDialogOpen(false)}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                </div>

                <div className="grid gap-3">
                  <div className="grid gap-3 sm:grid-cols-2">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Event")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setHookForm((current) => ({
                            ...current,
                            event: event.target.value,
                            groupIndex:
                              current.handlerIndex === null ? null : current.groupIndex,
                            handlerIndex:
                              current.handlerIndex === null ? null : current.handlerIndex,
                          }))
                        }
                        value={hookForm.event}
                      >
                        {(hookSettings?.supportedEvents ?? []).map((eventName) => (
                          <option key={eventName} value={eventName}>
                            {hookEventLabel(eventName, t)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <TextField
                      label={t("Matcher")}
                      onChange={(value) =>
                        setHookForm((current) => ({ ...current, matcher: value }))
                      }
                      placeholder="run_command|write_file"
                      value={hookForm.matcher}
                    />
                  </div>
                  <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                    <span className="text-sm font-semibold text-stone-700">
                      {t("Enable hook")}
                    </span>
                    <input
                      checked={hookForm.enabled}
                      className="size-4 accent-teal-700"
                      onChange={(event) =>
                        setHookForm((current) => ({
                          ...current,
                          enabled: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                  </label>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Handler type")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setHookForm((current) => ({
                            ...current,
                            type: event.target.value as HookHandlerType,
                          }))
                        }
                        value={hookForm.type}
                      >
                        {["command", "http", "mcp_tool", "prompt"].map((type) => (
                          <option key={type} value={type}>
                            {hookHandlerTypeLabel(type, t)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <TextField
                      label={t("If filter")}
                      onChange={(value) =>
                        setHookForm((current) => ({ ...current, ifFilter: value }))
                      }
                      placeholder="run_command(git *)"
                      value={hookForm.ifFilter}
                    />
                  </div>

                  {hookForm.type === "command" ? (
                    <>
                      <TextField
                        label={t("Command")}
                        onChange={(value) =>
                          setHookForm((current) => ({ ...current, command: value }))
                        }
                        placeholder="node scripts/hook.js"
                        value={hookForm.command}
                      />
                      <div className="grid gap-3 sm:grid-cols-2">
                        <TextField
                          label={t("Shell")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, shell: value }))
                          }
                          placeholder="powershell"
                          value={hookForm.shell}
                        />
                        <TextField
                          inputMode="numeric"
                          label={t("Timeout ms")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, timeout: value }))
                          }
                          placeholder="30000"
                          value={hookForm.timeout}
                        />
                      </div>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Args")}
                        </span>
                        <textarea
                          className="min-h-20 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setHookForm((current) => ({
                              ...current,
                              argsText: event.target.value,
                            }))
                          }
                          placeholder={"scripts/hook.js\n--check"}
                          value={hookForm.argsText}
                        />
                      </label>
                    </>
                  ) : null}

                  {hookForm.type === "http" ? (
                    <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem]">
                      <TextField
                        label={t("URL")}
                        onChange={(value) =>
                          setHookForm((current) => ({ ...current, url: value }))
                        }
                        placeholder="http://127.0.0.1:8787/hook"
                        value={hookForm.url}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Timeout ms")}
                        onChange={(value) =>
                          setHookForm((current) => ({ ...current, timeout: value }))
                        }
                        placeholder="30000"
                        value={hookForm.timeout}
                      />
                    </div>
                  ) : null}

                  {hookForm.type === "mcp_tool" ? (
                    <div className="grid gap-3 sm:grid-cols-2">
                      <TextField
                        label={t("MCP server id")}
                        onChange={(value) =>
                          setHookForm((current) => ({ ...current, serverId: value }))
                        }
                        placeholder="server"
                        value={hookForm.serverId}
                      />
                      <TextField
                        label={t("MCP tool name")}
                        onChange={(value) =>
                          setHookForm((current) => ({ ...current, toolName: value }))
                        }
                        placeholder="validate"
                        value={hookForm.toolName}
                      />
                    </div>
                  ) : null}

                  {hookForm.type === "prompt" ? (
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Prompt")}
                      </span>
                      <textarea
                        className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setHookForm((current) => ({
                            ...current,
                            prompt: event.target.value,
                          }))
                        }
                        placeholder={t("Return a JSON hook result.")}
                        value={hookForm.prompt}
                      />
                    </label>
                  ) : null}

                  <div className="grid gap-3 sm:grid-cols-2">
                    <TextField
                      label={t("Status message")}
                      onChange={(value) =>
                        setHookForm((current) => ({
                          ...current,
                          statusMessage: value,
                        }))
                      }
                      placeholder={t("Running hook")}
                      value={hookForm.statusMessage}
                    />
                    <TextField
                      inputMode="numeric"
                      label={t("Timeout ms")}
                      onChange={(value) =>
                        setHookForm((current) => ({ ...current, timeout: value }))
                      }
                      placeholder="60000"
                      value={hookForm.timeout}
                    />
                  </div>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                      <span className="text-sm font-semibold text-stone-700">
                        {t("Async")}
                      </span>
                      <input
                        checked={hookForm.asyncHook}
                        className="size-4 accent-teal-700"
                        onChange={(event) =>
                          setHookForm((current) => ({
                            ...current,
                            asyncHook: event.target.checked,
                          }))
                        }
                        type="checkbox"
                      />
                    </label>
                    <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                      <span className="text-sm font-semibold text-stone-700">
                        {t("Async re-wake")}
                      </span>
                      <input
                        checked={hookForm.asyncRewake}
                        className="size-4 accent-teal-700"
                        onChange={(event) =>
                          setHookForm((current) => ({
                            ...current,
                            asyncRewake: event.target.checked,
                          }))
                        }
                        type="checkbox"
                      />
                    </label>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Input override JSON")}
                    </span>
                    <textarea
                      className="min-h-20 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setHookForm((current) => ({
                          ...current,
                          inputText: event.target.value,
                        }))
                      }
                      placeholder="{ }"
                      value={hookForm.inputText}
                    />
                  </label>
                  <button
                    aria-label={t("Save hook")}
                    className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={isSavingHooks || !hookForm.event || !hookForm.type}
                    title={t("Save hook")}
                    type="submit"
                  >
                    {isSavingHooks ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                </div>
              </form>
            </>
          ) : null}

          <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="grid gap-3 lg:grid-cols-[auto_minmax(0,1fr)_auto]">
              <div className="inline-flex h-10 rounded-lg border border-stone-200 bg-stone-100 p-1">
                {(["global", "workspace"] as HookScope[]).map((scope) => (
                  <button
                    className={`rounded-md px-3 text-sm font-semibold ${
                      hookScope === scope
                        ? "bg-white text-teal-900 shadow-sm"
                        : "text-stone-600 hover:text-stone-950"
                    }`}
                    key={scope}
                    onClick={() => setHookScope(scope)}
                    type="button"
                  >
                    {scope === "global" ? t("Global") : t("Workspace")}
                  </button>
                ))}
              </div>
              <label className="min-w-0">
                <span className="sr-only">{t("Workspace")}</span>
                <select
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => {
                    setHookWorkspaceId(event.target.value);
                    setHookRunDetail(null);
                  }}
                  value={hookWorkspaceId}
                >
                  {workspaces.map((workspace) => (
                    <option key={workspace.id} value={workspace.id}>
                      {workspace.name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="flex gap-2">
                <button
                  aria-label={t("Add hook")}
                  className="inline-flex size-10 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={!hookSettings}
                  onClick={startAddingHookHandler}
                  title={t("Add hook")}
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Reload hooks")}
                  className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                  disabled={isLoadingHooks || !selectedHookWorkspace}
                  onClick={() =>
                    selectedHookWorkspace
                      ? void loadHooks(selectedHookWorkspace.id)
                      : undefined
                  }
                  title={t("Reload hooks")}
                  type="button"
                >
                  {isLoadingHooks ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
            </div>
            <div className="mt-3 break-all rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
              {activeHookPath ?? t("Loading...")}
            </div>
            <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
              <span className="text-sm font-semibold text-stone-700">
                {t("Disable all hooks")}
              </span>
              <input
                checked={Boolean(activeHookConfig?.disableAllHooks)}
                className="size-4 accent-teal-700"
                disabled={isSavingHooks || !activeHookConfig}
                onChange={(event) =>
                  updateHookConfig({
                    ...(activeHookConfig ?? emptyHookConfig()),
                    disableAllHooks: event.target.checked,
                  })
                }
                type="checkbox"
              />
            </label>
            <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
              <span className="text-sm font-semibold text-stone-700">
                {t("Record hook run logs")}
              </span>
              <input
                checked={generalForm.hookAuditEnabled}
                className="size-4 accent-teal-700"
                disabled={isSavingGeneral || !settings}
                onChange={(event) => void saveHookAuditEnabled(event.target.checked)}
                type="checkbox"
              />
            </label>
          </section>

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Hook rules")}
              </h3>
              <CapabilityPill
                label={t("rules {count}", { count: activeHookGroups.length })}
                ok={activeHookGroups.length > 0}
              />
            </div>
            <div className="divide-y divide-stone-100">
              {activeHookGroups.length ? (
                activeHookGroups.map((entry) => (
                  <div className="px-4 py-3" key={`${entry.event}-${entry.groupIndex}`}>
                    <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-sm font-semibold text-stone-950">
                            {entry.event}
                          </span>
                          <CapabilityPill
                            label={entry.group.enabled === false ? t("disabled") : t("enabled")}
                            ok={entry.group.enabled !== false}
                          />
                          <CapabilityPill
                            label={entry.group.matcher || "*"}
                            ok={Boolean(entry.group.matcher)}
                          />
                        </div>
                        <div className="mt-1 text-xs text-stone-500">
                          {t("handlers {count}", { count: entry.group.hooks.length })}
                        </div>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <button
                          aria-label={t("Move hook up")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                          disabled={entry.groupIndex === 0 || isSavingHooks}
                          onClick={() => moveHookGroup(entry.event, entry.groupIndex, -1)}
                          title={t("Move hook up")}
                          type="button"
                        >
                          <ArrowUp aria-hidden="true" className="size-4" />
                        </button>
                        <button
                          aria-label={t("Move hook down")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                          disabled={
                            entry.groupIndex >=
                              hookGroupsForEvent(activeHookConfig, entry.event).length - 1 ||
                            isSavingHooks
                          }
                          onClick={() => moveHookGroup(entry.event, entry.groupIndex, 1)}
                          title={t("Move hook down")}
                          type="button"
                        >
                          <ArrowDown aria-hidden="true" className="size-4" />
                        </button>
                        <label className="relative inline-flex cursor-pointer items-center">
                          <input
                            aria-label={t("Enable hook group")}
                            checked={entry.group.enabled !== false}
                            className="peer sr-only"
                            disabled={isSavingHooks}
                            onChange={(event) =>
                              toggleHookGroup(
                                entry.event,
                                entry.groupIndex,
                                event.target.checked,
                              )
                            }
                            type="checkbox"
                          />
                          <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                          <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                        </label>
                      </div>
                    </div>
                    <div className="mt-3 space-y-2">
                      {entry.group.hooks.map((handler, handlerIndex) => (
                        <div
                          className="grid gap-3 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
                          key={`${entry.event}-${entry.groupIndex}-${handlerIndex}`}
                        >
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-mono text-xs font-semibold text-stone-800">
                                {handler.type}
                              </span>
                              <CapabilityPill
                                label={handler.enabled === false ? t("disabled") : t("enabled")}
                                ok={handler.enabled !== false}
                              />
                              {handler.if ? (
                                <CapabilityPill label={handler.if} ok />
                              ) : null}
                              {handler.async ? (
                                <CapabilityPill label={t("async")} ok />
                              ) : null}
                            </div>
                            <div className="mt-1 truncate text-xs text-stone-500">
                              {hookHandlerSummary(handler)}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <button
                              aria-label={t("Move handler up")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                              disabled={handlerIndex === 0 || isSavingHooks}
                              onClick={() =>
                                moveHookHandler(
                                  entry.event,
                                  entry.groupIndex,
                                  handlerIndex,
                                  -1,
                                )
                              }
                              title={t("Move handler up")}
                              type="button"
                            >
                              <ArrowUp aria-hidden="true" className="size-4" />
                            </button>
                            <button
                              aria-label={t("Move handler down")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                              disabled={
                                handlerIndex >= entry.group.hooks.length - 1 ||
                                isSavingHooks
                              }
                              onClick={() =>
                                moveHookHandler(
                                  entry.event,
                                  entry.groupIndex,
                                  handlerIndex,
                                  1,
                                )
                              }
                              title={t("Move handler down")}
                              type="button"
                            >
                              <ArrowDown aria-hidden="true" className="size-4" />
                            </button>
                            <button
                              aria-label={t("Edit hook")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              onClick={() =>
                                editHookHandler(
                                  entry.event,
                                  entry.groupIndex,
                                  handlerIndex,
                                  entry.group,
                                  handler,
                                )
                              }
                              title={t("Edit hook")}
                              type="button"
                            >
                              <Pencil aria-hidden="true" className="size-4" />
                            </button>
                            <button
                              aria-label={t("Delete hook")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                              disabled={isSavingHooks}
                              onClick={() =>
                                deleteHookHandler(
                                  entry.event,
                                  entry.groupIndex,
                                  handlerIndex,
                                )
                              }
                              title={t("Delete hook")}
                              type="button"
                            >
                              <Trash2 aria-hidden="true" className="size-4" />
                            </button>
                            <label className="relative inline-flex cursor-pointer items-center">
                              <input
                                aria-label={t("Enable hook")}
                                checked={handler.enabled !== false}
                                className="peer sr-only"
                                disabled={isSavingHooks}
                                onChange={(event) =>
                                  toggleHookHandler(
                                    entry.event,
                                    entry.groupIndex,
                                    handlerIndex,
                                    event.target.checked,
                                  )
                                }
                                type="checkbox"
                              />
                              <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                              <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                            </label>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No hook rules")}
                </div>
              )}
            </div>
          </section>

          <div className="grid gap-4 xl:grid-cols-2">
            <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-semibold text-stone-950">
                  {t("Import Claude hooks")}
                </h3>
                <div className="flex gap-2">
                  <button
                    aria-label={t("Import to global hooks")}
                    className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isImportingHooks}
                    onClick={() => void importClaudeHooks("global")}
                    title={t("Import to global hooks")}
                    type="button"
                  >
                    {isImportingHooks ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <Globe aria-hidden="true" className="size-4" />
                    )}
                    {t("Global")}
                  </button>
                  <button
                    aria-label={t("Import to workspace hooks")}
                    className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isImportingHooks || !selectedHookWorkspace}
                    onClick={() => void importClaudeHooks("workspace")}
                    title={t("Import to workspace hooks")}
                    type="button"
                  >
                    <Folder aria-hidden="true" className="size-4" />
                    {t("Workspace")}
                  </button>
                </div>
              </div>
              <p className="mt-2 text-xs text-stone-500">
                {t("Global import reads user Claude settings; workspace import reads the selected workspace.")}
              </p>
              {hookImportResult ? (
                <div
                  className={`mt-3 rounded-lg border px-3 py-2 text-sm ${
                    hookImportResult.saved
                      ? "border-teal-200 bg-teal-50 text-teal-800"
                      : "border-amber-200 bg-amber-50 text-amber-800"
                  }`}
                >
                  <div className="font-semibold">
                    {hookImportResult.saved ? t("Import saved") : t("Import not saved")}
                  </div>
                  <div className="mt-1 break-all text-xs">{hookImportResult.path}</div>
                  {hookImportResult.importedFiles.length ? (
                    <div className="mt-2 space-y-1">
                      {hookImportResult.importedFiles.map((path) => (
                        <div className="break-all text-xs" key={path}>
                          {path}
                        </div>
                      ))}
                    </div>
                  ) : null}
                  {hookImportResult.validationErrors.length ? (
                    <div className="mt-2 space-y-1">
                      {hookImportResult.validationErrors.map((message) => (
                        <div className="break-words text-xs" key={message}>
                          {message}
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </section>

            <form
              className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
              onSubmit={(event) => void testHooks(event)}
            >
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Test hook")}
              </h3>
              <div className="mt-3 grid gap-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Event")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) => setHookTestEvent(event.target.value)}
                      value={hookTestEvent}
                    >
                      {(hookSettings?.supportedEvents ?? []).map((eventName) => (
                        <option key={eventName} value={eventName}>
                          {hookEventLabel(eventName, t)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <TextField
                    label={t("Match value")}
                    onChange={setHookTestMatcher}
                    placeholder="run_command"
                    value={hookTestMatcher}
                  />
                </div>
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Sample payload")}
                  </span>
                  <textarea
                    className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) => setHookTestPayload(event.target.value)}
                    value={hookTestPayload}
                  />
                </label>
                <button
                  aria-label={t("Run hook test")}
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={isTestingHooks || !selectedHookWorkspace}
                  title={t("Run hook test")}
                  type="submit"
                >
                  {isTestingHooks ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                  {t("Run")}
                </button>
              </div>
              {hookTestResult ? (
                <pre className="mt-3 max-h-48 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                  {JSON.stringify(hookTestResult, null, 2)}
                </pre>
              ) : null}
            </form>
          </div>

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Effective hooks")}
              </h3>
              <CapabilityPill
                label={t("hooks {count}", { count: hookSettings?.effective.length ?? 0 })}
                ok={(hookSettings?.effective.length ?? 0) > 0}
              />
            </div>
            <div className="divide-y divide-stone-100">
              {hookSettings?.effective.length ? (
                hookSettings.effective.map((hook, index) => {
                  const lastRun = latestHookRunForSummary(
                    hook,
                    hookSettings.recentRuns,
                  );

                  return (
                    <div className="grid gap-3 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto]" key={`${hook.source}-${hook.event}-${index}`}>
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-sm font-semibold text-stone-950">
                            {hookEventLabel(hook.event, t)}
                          </span>
                          <CapabilityPill
                            label={hookSourceLabel(hook.source, t)}
                            ok={hook.source === "global"}
                          />
                          <CapabilityPill
                            label={hookHandlerTypeLabel(hook.handlerType, t)}
                            ok
                          />
                          {hook.asyncHook ? <CapabilityPill label={t("async")} ok /> : null}
                          {lastRun ? (
                            <CapabilityPill
                              label={t("last {status}", {
                                status: hookRunStatusLabel(lastRun.status, t),
                              })}
                              ok={lastRun.status === "succeeded"}
                            />
                          ) : null}
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          {[hook.matcher || "*", hook.command, hook.url, hook.serverId, hook.toolName]
                            .filter(Boolean)
                            .join(" / ")}
                        </div>
                      </div>
                      <div className="text-xs text-stone-500">
                        {lastRun?.startedAt ?? hook.statusMessage ?? t("ready")}
                      </div>
                    </div>
                  );
                })
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No effective hooks")}
                </div>
              )}
            </div>
          </section>

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Recent hook runs")}
              </h3>
              <button
                aria-label={t("Refresh hook runs")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                disabled={isRefreshingHookRuns || !selectedHookWorkspace}
                onClick={() => void refreshHookRuns()}
                title={t("Refresh hook runs")}
                type="button"
              >
                {isRefreshingHookRuns ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>
            <div className="divide-y divide-stone-100">
              {hookSettings?.recentRuns.length ? (
                hookSettings.recentRuns.map((run) => (
                  <button
                    className="grid w-full gap-3 px-4 py-3 text-left hover:bg-stone-50 md:grid-cols-[minmax(0,1fr)_auto]"
                    key={run.id}
                    onClick={() => void openHookRunDetail(run.id)}
                    type="button"
                  >
                    <span className="min-w-0">
                      <span className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-semibold text-stone-950">
                          {hookEventLabel(run.event, t)}
                        </span>
                        <CapabilityPill
                          label={hookRunStatusLabel(run.status, t)}
                          ok={run.status === "succeeded"}
                        />
                        <CapabilityPill
                          label={hookHandlerTypeLabel(run.handlerType, t)}
                          ok
                        />
                      </span>
                      <span className="mt-1 block truncate text-xs text-stone-500">
                        {run.id}
                      </span>
                    </span>
                    <span className="text-xs text-stone-500">{run.startedAt}</span>
                  </button>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No hook runs")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}

        {activeSection === "providers" ? (
        <section className="grid gap-4">
          {isProviderDialogOpen ? (
          <>
          <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
          <form
            aria-label={t("Provider configuration")}
            className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
            onSubmit={(event) => void saveProvider(event)}
          >
            <div className="mb-4 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <PlugZap aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {providerForm.id ? t("Edit provider") : t("Add provider")}
                  </h3>
                </div>
                {providerForm.id ? (
                  <div className="mt-1 truncate text-xs text-stone-500">
                    {providerForm.id}
                  </div>
                ) : null}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <label className="relative inline-flex cursor-pointer items-center">
                  <input
                    aria-label={t("Enable provider")}
                    checked={providerForm.enabled}
                    className="peer sr-only"
                    onChange={(event) =>
                      setProviderForm((current) => ({
                        ...current,
                        enabled: event.target.checked,
                      }))
                    }
                    type="checkbox"
                  />
                  <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                  <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                </label>
                {providerForm.id ? (
                <button
                  aria-label={t("Delete provider")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                  disabled={isSavingProvider}
                  onClick={() => void deleteProvider(providerForm.id)}
                  title={t("Delete provider")}
                  type="button"
                >
                  <Trash2 aria-hidden="true" className="size-4" />
                </button>
                ) : null}
                <button
                  aria-label={t("Close provider configuration")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                  onClick={() => setIsProviderDialogOpen(false)}
                  title={t("Close")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
            <div className="space-y-3">
              <TextField
                label={t("Name")}
                onChange={(value) =>
                  setProviderForm((current) => ({
                    ...current,
                    name: value,
                  }))
                }
                placeholder="OpenAI"
                value={providerForm.name}
              />
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("Protocol")}
                </span>
                <select
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) =>
                    setProviderForm((current) => ({
                      ...current,
                      kind: event.target.value,
                    }))
                  }
                  value={providerForm.kind || providerKinds[0]?.kind || ""}
                >
                  {providerKinds.map((kind) => (
                    <option key={kind.kind} value={kind.kind}>
                      {kind.label}
                    </option>
                  ))}
                </select>
              </label>
              <TextField
                label={t("Base URL")}
                onChange={(value) =>
                  setProviderForm((current) => ({
                    ...current,
                    baseUrl: value,
                  }))
                }
                placeholder={selectedProviderKind?.defaultBaseUrl ?? ""}
                value={providerForm.baseUrl}
              />
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("API key")}
                </span>
                <span className="relative block">
                <input
                  autoComplete="off"
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 pr-11 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  name="api-key"
                  onChange={(event) =>
                    setProviderForm((current) => ({
                      ...current,
                      apiKey: event.target.value,
                      clearApiKey: false,
                    }))
                  }
                  placeholder={
                    hasSavedProviderKey
                      ? t("Saved key is kept unless replaced")
                      : t("API key")
                  }
                  type="password"
                  value={providerForm.apiKey}
                />
                {hasSavedProviderKey || providerForm.clearApiKey ? (
                  <button
                    aria-label={t("Clear saved API key")}
                    className={`absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md ${
                      providerForm.clearApiKey
                        ? "bg-rose-50 text-rose-700"
                        : "text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                    }`}
                    onClick={() =>
                      setProviderForm((current) => ({
                        ...current,
                        apiKey: "",
                        clearApiKey: true,
                      }))
                    }
                    title={t("Clear saved API key")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                ) : null}
                </span>
              </label>
              <div className="rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <PlugZap aria-hidden="true" className="size-4 text-teal-700" />
                    <h4 className="text-sm font-semibold text-stone-950">
                      {t("AI API proxy")}
                    </h4>
                  </div>
                  <CapabilityPill
                    label={
                      providerForm.apiProxyEnabled
                        ? t("Proxy enabled")
                        : t("Proxy disabled")
                    }
                    ok={providerForm.apiProxyEnabled}
                  />
                </div>
                <div className="mt-3 grid gap-3">
                  <label className="inline-flex items-center gap-2 text-sm font-semibold text-stone-700">
                    <input
                      aria-label={t("Enable AI API proxy")}
                      checked={providerForm.apiProxyEnabled}
                      className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                      onChange={(event) =>
                        setProviderForm((current) => ({
                          ...current,
                          apiProxyEnabled: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                    {t("Enable AI API proxy")}
                  </label>
                  <div className="grid gap-3 sm:grid-cols-[12rem_minmax(0,1fr)]">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Proxy type")}
                      </span>
                      <select
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setProviderForm((current) => ({
                            ...current,
                            apiProxyType: event.target.value,
                          }))
                        }
                        value={providerForm.apiProxyType}
                      >
                        {apiProxyTypes.map((proxyType) => (
                          <option
                            key={proxyType.proxyType}
                            value={proxyType.proxyType}
                          >
                            {proxyType.label}
                          </option>
                        ))}
                      </select>
                    </label>
                    <TextField
                      label={t("Proxy server")}
                      onChange={(value) =>
                        setProviderForm((current) => ({
                          ...current,
                          apiProxyUrl: value,
                        }))
                      }
                      placeholder="127.0.0.1:7890"
                      value={providerForm.apiProxyUrl}
                    />
                  </div>
                </div>
              </div>
              <button
                aria-label={t("Save provider")}
                className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingProvider ||
                  !providerForm.name.trim() ||
                  !providerForm.kind.trim()
                }
                title={t("Save provider")}
                type="submit"
              >
                {isSavingProvider ? (
                  <LoaderCircle
                    aria-hidden="true"
                    className="size-4 animate-spin"
                  />
                ) : (
                  <KeyRound aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>
          </form>
          </>
          ) : null}

          <section className="order-1 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Configured providers")}
              </h3>
              <div className="flex gap-2">
                <button
                  aria-label={t("Add provider")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={startAddingProvider}
                  title={t("Add provider")}
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Reload settings")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoadingSettings}
                  onClick={() => void loadSettings()}
                  title={t("Reload settings")}
                  type="button"
                >
                  {isLoadingSettings ? (
                    <LoaderCircle
                      aria-hidden="true"
                      className="size-4 animate-spin"
                    />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
            </div>
            <div className="divide-y divide-stone-100">
              {providers.length ? (
                providers.map((provider) => {
                  const test = providerTests[provider.id];

                  return (
                    <div className="px-4 py-3" key={provider.id}>
                      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {provider.name}
                            </span>
                            <CapabilityPill
                              label={
                                provider.enabled ? t("enabled") : t("disabled")
                              }
                              ok={provider.enabled}
                            />
                            <CapabilityPill
                              label={
                                provider.hasApiKey
                                  ? t("key saved")
                                  : t("key missing")
                              }
                              ok={provider.hasApiKey}
                            />
                          </div>
                          <div className="mt-1 truncate text-xs font-medium text-stone-500">
                            {provider.id} / {provider.kindLabel}
                          </div>
                          {provider.baseUrl ? (
                            <div className="mt-1 truncate text-xs text-stone-500">
                              {provider.baseUrl}
                            </div>
                          ) : null}
                        </div>
                        <div className="flex flex-wrap gap-2">
                          <button
                            aria-label={t("Edit provider {name}", {
                              name: provider.name,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={() => editConfiguredProvider(provider)}
                            title={t("Edit provider")}
                            type="button"
                          >
                            <SlidersHorizontal aria-hidden="true" className="size-4" />
                          </button>
                          <button
                            aria-label={t("Test provider {name}", {
                              name: provider.name,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                            disabled={test?.status === "testing"}
                            onClick={() => void testProvider(provider.id)}
                            title={t("Test provider")}
                            type="button"
                          >
                            {test?.status === "testing" ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="size-4 animate-spin"
                              />
                            ) : (
                              <PlugZap aria-hidden="true" className="size-4" />
                            )}
                          </button>
                        </div>
                      </div>
                      {test ? (
                        <div
                          className={`mt-3 rounded-lg border px-3 py-2 text-sm ${
                            test.status === "ok"
                              ? "border-teal-200 bg-teal-50 text-teal-800"
                              : test.status === "testing"
                                ? "border-stone-200 bg-stone-50 text-stone-600"
                                : "border-rose-200 bg-rose-50 text-rose-700"
                          }`}
                        >
                          {test.message}
                        </div>
                      ) : null}
                      <Warnings warnings={provider.warnings} />
                    </div>
                  );
                })
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No configured providers")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}

        {activeSection === "mcp" ? (
        <section className="grid gap-4">
          {isMcpDialogOpen ? (
          <>
          <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
          <form
            aria-label={t("MCP server configuration")}
            className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
            onSubmit={(event) => void saveMcpServer(event)}
          >
            <div className="mb-4 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <Server aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {mcpForm.id ? t("Edit MCP server") : t("Add MCP server")}
                  </h3>
                </div>
                {mcpForm.id ? (
                  <div className="mt-1 truncate text-xs text-stone-500">
                    {mcpForm.id}
                  </div>
                ) : null}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <label className="relative inline-flex cursor-pointer items-center">
                  <input
                    aria-label={t("Enable MCP server")}
                    checked={mcpForm.enabled}
                    className="peer sr-only"
                    onChange={(event) =>
                      setMcpForm((current) => ({
                        ...current,
                        enabled: event.target.checked,
                      }))
                    }
                    type="checkbox"
                  />
                  <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                  <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                </label>
                {mcpForm.id ? (
                <button
                  aria-label={t("Delete MCP server")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                  disabled={isSavingMcpServer}
                  onClick={() => void deleteMcpServer(mcpForm.id)}
                  title={t("Delete MCP server")}
                  type="button"
                >
                  <Trash2 aria-hidden="true" className="size-4" />
                </button>
                ) : null}
                <button
                  aria-label={t("Close MCP server configuration")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                  onClick={() => setIsMcpDialogOpen(false)}
                  title={t("Close")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
            <div className="space-y-3">
              <TextField
                label={t("Name")}
                onChange={(value) =>
                  setMcpForm((current) => ({
                    ...current,
                    name: value,
                  }))
                }
                placeholder="CodeGraph"
                value={mcpForm.name}
              />
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("Transport")}
                </span>
                <select
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) =>
                    setMcpForm((current) => ({
                      ...current,
                      transport: event.target.value,
                    }))
                  }
                  value={mcpForm.transport || mcpTransports[0]?.transport || ""}
                >
                  {mcpTransports.map((transport) => (
                    <option
                      key={transport.transport}
                      value={transport.transport}
                    >
                      {t(transport.label)}
                    </option>
                  ))}
                </select>
              </label>
              {mcpForm.transport === "streamable-http" ? (
                <TextField
                  label={t("URL")}
                  onChange={(value) =>
                    setMcpForm((current) => ({
                      ...current,
                      url: value,
                    }))
                  }
                  placeholder="http://127.0.0.1:8000/mcp"
                  value={mcpForm.url}
                />
              ) : (
                <>
                  <TextField
                    label={t("Command")}
                    onChange={(value) =>
                      setMcpForm((current) => ({
                        ...current,
                        command: value,
                      }))
                    }
                    placeholder="codegraph"
                    value={mcpForm.command}
                  />
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Args")}
                    </span>
                    <textarea
                      className="min-h-24 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setMcpForm((current) => ({
                          ...current,
                          argsText: event.target.value,
                        }))
                      }
                      placeholder={"serve\n--stdio"}
                      value={mcpForm.argsText}
                    />
                  </label>
                </>
              )}
              <button
                aria-label={t("Save MCP server")}
                className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingMcpServer ||
                  !mcpForm.name.trim() ||
                  !mcpForm.transport.trim() ||
                  (mcpForm.transport === "streamable-http"
                    ? !mcpForm.url.trim()
                    : !mcpForm.command.trim())
                }
                title={t("Save MCP server")}
                type="submit"
              >
                {isSavingMcpServer ? (
                  <LoaderCircle
                    aria-hidden="true"
                    className="size-4 animate-spin"
                  />
                ) : mcpForm.transport === "streamable-http" ? (
                  <Globe aria-hidden="true" className="size-4" />
                ) : (
                  <Terminal aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>
          </form>
          </>
          ) : null}

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("MCP servers")}
              </h3>
              <div className="flex gap-2">
                <button
                  aria-label={t("Add MCP server")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={startAddingMcpServer}
                  title={t("Add MCP server")}
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Reload MCP settings")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoadingSettings}
                  onClick={() => void loadSettings()}
                  title={t("Reload settings")}
                  type="button"
                >
                  {isLoadingSettings ? (
                    <LoaderCircle
                      aria-hidden="true"
                      className="size-4 animate-spin"
                    />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
            </div>
            <div className="divide-y divide-stone-100">
              {mcpServers.length ? (
                mcpServers.map((server) => (
                  <div className="px-4 py-3" key={server.id}>
                    <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="truncate text-sm font-medium">
                            {server.name}
                          </span>
                          <CapabilityPill
                            label={server.enabled ? t("enabled") : t("disabled")}
                            ok={server.enabled}
                          />
                          <CapabilityPill
                            label={t(server.state)}
                            ok={server.state === "connected"}
                          />
                          <CapabilityPill
                            label={t("tools {count}", {
                              count: server.toolCount,
                            })}
                            ok={server.toolCount > 0}
                          />
                        </div>
                        <div className="mt-1 truncate text-xs font-medium text-stone-500">
                          {server.id} / {server.transportLabel}
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          {server.transport === "streamable-http"
                            ? server.url
                            : [server.command, ...server.args].filter(Boolean).join(" ")}
                        </div>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <button
                          aria-label={t("Edit MCP server {name}", {
                            name: server.name,
                          })}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                          onClick={() => editConfiguredMcpServer(server)}
                          title={t("Edit MCP server")}
                          type="button"
                        >
                          <SlidersHorizontal aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                    </div>
                    {server.error ? (
                      <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                        {server.error}
                      </div>
                    ) : null}
                    <Warnings warnings={server.warnings} />
                  </div>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No configured MCP servers")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}

        {activeSection === "skills" ? (
        <section className="grid gap-4">
          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Detected skills")}
              </h3>
              <div className="flex items-center gap-2">
                <CapabilityPill
                  label={t("skills {count}", {
                    count: skills?.detected.length ?? 0,
                  })}
                  ok={(skills?.detected.length ?? 0) > 0}
                />
                <button
                  aria-label={t("Refresh skill discovery")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={isRefreshingSkills}
                  onClick={() => void refreshSkills()}
                  title={t("Refresh skill discovery")}
                  type="button"
                >
                  {isRefreshingSkills ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
            </div>
            <div className="divide-y divide-stone-100">
              {skills?.detected.length ? (
                skills.detected.map((skill) => {
                  const enabled = enabledSkillIds.has(skill.key);

                  return (
                    <div className="px-4 py-3" key={skill.key}>
                      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {skill.name}
                            </span>
                            <CapabilityPill
                              label={enabled ? t("enabled") : t("disabled")}
                              ok={enabled}
                            />
                            <CapabilityPill
                              label={skillScopeLabel(skill, t)}
                              ok={skill.scope === "global"}
                            />
                          </div>
                          <div className="mt-1 truncate text-xs font-medium text-stone-500">
                            {skill.key}
                          </div>
                          <div className="mt-1 break-words text-xs text-stone-500">
                            {skill.description}
                          </div>
                          <div className="mt-1 break-all text-xs text-stone-400">
                            {skill.path}
                          </div>
                        </div>
                        <label className="relative inline-flex cursor-pointer items-center justify-self-start md:justify-self-end">
                          <input
                            aria-label={t("Enable skill {name}", {
                              name: skill.name,
                            })}
                            checked={enabled}
                            className="peer sr-only"
                            disabled={isSavingSkills || !skill.canEnable}
                            onChange={(event) =>
                              toggleSkill(skill.key, event.target.checked)
                            }
                            type="checkbox"
                          />
                          <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                          <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                        </label>
                      </div>
                      <Warnings warnings={skill.warnings} />
                    </div>
                  );
                })
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No detected skills")}
                </div>
              )}
            </div>
          </section>

          <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center gap-2">
              <Wrench aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Skill locations")}
              </h3>
            </div>
            <div className="mt-4 grid gap-2">
              {skills?.directories.length ? (
                skills.directories.map((directory) => (
                  <div
                    className="break-all rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs font-medium text-stone-600"
                    key={directory}
                  >
                    {directory}
                  </div>
                ))
              ) : (
                <div className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-sm text-stone-500">
                  {t("Loading...")}
                </div>
              )}
            </div>
            {skills?.errors.length ? (
              <div className="mt-4 space-y-2">
                {skills.errors.map((skillError) => (
                  <div
                    className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700"
                    key={`${skillError.path}-${skillError.message}`}
                  >
                    <div className="break-all font-medium">{skillError.path}</div>
                    <div className="mt-1 break-words">{skillError.message}</div>
                  </div>
                ))}
              </div>
            ) : null}
          </section>
        </section>
        ) : null}

        {activeSection === "models" ? (
        <section className="grid gap-4">
          <div className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Models")}
              </h3>
              <div className="flex gap-2">
                <button
                  aria-label={t("Add model")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={startAddingModel}
                  title={t("Add model")}
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
            <div className="divide-y divide-stone-100">
              {orderedConfiguredModels.length ? (
                orderedConfiguredModels.map((model) => (
                <div
                  className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${
                    draggedModelId === model.id
                      ? "bg-teal-50/70 opacity-80"
                      : "bg-white/0"
                  }`}
                  draggable={!isSavingModelOrder}
                  key={model.id}
                  onDragEnd={handleModelDragEnd}
                  onDragOver={(event) => handleModelDragOver(event, model.id)}
                  onDragStart={(event) => handleModelDragStart(event, model.id)}
                  onDrop={(event) => void handleModelDrop(event)}
                >
                  <div className="flex items-center">
                    <span
                      aria-label={t("Reorder model {name}", {
                        name: model.displayName,
                      })}
                      className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${
                        isSavingModelOrder
                          ? "cursor-not-allowed opacity-60"
                          : "cursor-grab hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      }`}
                      title={t("Reorder model {name}", {
                        name: model.displayName,
                      })}
                    >
                      {isSavingModelOrder && draggedModelId === model.id ? (
                        <LoaderCircle
                          aria-hidden="true"
                          className="size-4 animate-spin"
                        />
                      ) : (
                        <GripVertical aria-hidden="true" className="size-4" />
                      )}
                    </span>
                  </div>
                  <div className="min-w-0">
                    <div className="flex min-w-0 items-center gap-2 overflow-hidden">
                      <span
                        className="min-w-0 truncate text-sm font-semibold"
                        title={model.displayName}
                      >
                        {model.displayName}
                      </span>
                      <CapabilityPill
                        className="shrink-0"
                        label={model.enabled ? t("enabled") : t("disabled")}
                        ok={model.enabled}
                      />
                      <CapabilityPill
                        className="shrink-0"
                        label={
                          model.canEnable
                            ? t("limits ok")
                            : t("limits missing")
                        }
                        ok={model.canEnable}
                      />
                    </div>
                    <div className="mt-1 flex min-w-0 items-center gap-2 overflow-hidden">
                      <span
                        className="min-w-0 truncate text-xs font-medium text-stone-500"
                        title={model.id}
                      >
                        {model.id}
                      </span>
                      <CapabilityPill
                        className="shrink-0"
                        label={t("providers {count}", {
                          count: model.providerIds.length,
                        })}
                        ok={model.providerIds.length > 0}
                      />
                      <CapabilityPill
                        className="min-w-0"
                        label={
                          model.activeProviderId
                            ? t("active {id}", { id: model.activeProviderId })
                            : t("active missing")
                        }
                        ok={model.activeProviderId !== null}
                        title={model.activeProviderId ?? undefined}
                      />
                    </div>
                  </div>
                  <button
                    aria-label={t("Edit model {name}", {
                      name: model.displayName,
                    })}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    onClick={() => editConfiguredModel(model)}
                    title={t("Edit model")}
                    type="button"
                  >
                    <SlidersHorizontal aria-hidden="true" className="size-4" />
                  </button>
                </div>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  {t("No configured models")}
                </div>
              )}
            </div>
          </div>

          {isModelDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={t("Model configuration")}
                className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,38rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                onSubmit={(event) => void saveModel(event)}
              >
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <SlidersHorizontal
                        aria-hidden="true"
                        className="size-5 text-teal-700"
                      />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {editingModel ? t("Edit model") : t("Add model")}
                      </h3>
                    </div>
                    {selectedMetadata ? (
                      <div className="mt-1 truncate text-xs text-stone-500">
                        {selectedMetadata.key}
                      </div>
                    ) : null}
                  </div>
                  <div className="flex shrink-0 gap-2">
                    {editingModel ? (
                      <button
                        aria-label={t("Delete model")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={isSaving}
                        onClick={() => void deleteModel(editingModel.id)}
                        title={t("Delete model")}
                        type="button"
                      >
                        <Trash2 aria-hidden="true" className="size-4" />
                      </button>
                    ) : null}
                    <button
                      aria-label={t("Close model configuration")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                      onClick={() => setIsModelDialogOpen(false)}
                      title={t("Close")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-4" />
                    </button>
                  </div>
                </div>
                <div className="space-y-3">
                  <TextField
                    label={t("Model id")}
                    onChange={updateModelId}
                    placeholder="gpt-5.5"
                    value={form.modelId}
                  />
                  <TextField
                    label={t("Display name")}
                    onChange={(value) =>
                      setForm((current) => ({
                        ...current,
                        displayName: value,
                      }))
                    }
                    placeholder="GPT 5.5"
                    value={form.displayName}
                  />
                  <div className="grid gap-3 sm:grid-cols-2">
                    <TextField
                      inputMode="numeric"
                      label={t("Context window")}
                      onChange={(value) =>
                        setForm((current) => ({
                          ...current,
                          contextWindow: value,
                        }))
                      }
                      placeholder="128000"
                      value={form.contextWindow}
                    />
                    <TextField
                      inputMode="numeric"
                      label={t("Max output tokens")}
                      onChange={(value) =>
                        setForm((current) => ({
                          ...current,
                          maxOutputTokens: value,
                        }))
                      }
                      placeholder="16384"
                      value={form.maxOutputTokens}
                    />
                  </div>
                  <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                    <span className="text-sm font-semibold text-stone-700">
                      {t("Enable model")}
                    </span>
                    <input
                      checked={form.enabled}
                      className="size-4 accent-teal-700"
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          enabled: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                  </label>
                  <div className="rounded-xl border border-stone-200 px-3 py-3">
                    <div className="mb-2 flex items-center justify-between gap-2">
                      <div className="text-xs font-semibold text-stone-600">
                        {t("Providers")}
                      </div>
                      <button
                        aria-label={t("Add provider")}
                        className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        onClick={startAddingProviderFromModel}
                        title={t("Add provider")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    <div className="space-y-2">
                      {providers.length ? (
                        providers.map((provider) => (
                          <label
                            className="flex items-center justify-between gap-3 rounded-lg bg-stone-50/80 px-3 py-2"
                            key={provider.id}
                          >
                            <span className="min-w-0">
                              <span className="block truncate text-sm font-semibold text-stone-700">
                                {provider.name}
                              </span>
                              <span className="block truncate text-xs text-stone-500">
                                {provider.kindLabel}
                              </span>
                            </span>
                            <input
                              checked={selectedProviderIds.has(provider.id)}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                toggleModelProvider(
                                  provider.id,
                                  event.target.checked,
                                )
                              }
                              type="checkbox"
                            />
                          </label>
                        ))
                      ) : (
                        <button
                          className="flex w-full items-center justify-between rounded-lg border border-dashed border-stone-300 bg-stone-50 px-3 py-3 text-left text-sm text-stone-500 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                          onClick={startAddingProviderFromModel}
                          type="button"
                        >
                          <span>{t("No providers")}</span>
                          <Plus aria-hidden="true" className="size-4" />
                        </button>
                      )}
                    </div>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Active provider")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      disabled={!form.providerIds.length}
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          activeProviderId: event.target.value,
                        }))
                      }
                      value={form.activeProviderId}
                    >
                      <option value="">{t("None")}</option>
                      {form.providerIds.map((providerId) => {
                        const provider = providers.find(
                          (item) => item.id === providerId,
                        );

                        return (
                          <option key={providerId} value={providerId}>
                            {provider?.name ?? providerId}
                          </option>
                        );
                      })}
                    </select>
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Thinking level")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          thinkingLevel: event.target.value,
                        }))
                      }
                      value={form.thinkingLevel}
                    >
                      <option value="">{t("None")}</option>
                      {thinkingLevels.map((level) => (
                        <option key={level.value} value={level.value}>
                          {t(level.label)}
                        </option>
                      ))}
                    </select>
                  </label>
                  {enabledNeedsLimits ? (
                    <div className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                      <CircleAlert
                        aria-hidden="true"
                        className="size-4 shrink-0"
                      />
                      {t("Fill both limits before enabling.")}
                    </div>
                  ) : null}
                  <button
                    aria-label={t("Save model")}
                    className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={
                      isSaving ||
                      enabledNeedsLimits ||
                      !form.modelId.trim() ||
                      !form.displayName.trim()
                    }
                    title={t("Save model")}
                    type="submit"
                  >
                    {isSaving ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin"
                      />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>

                {selectedMetadata ? (
                  <div className="mt-4 border-t border-stone-200 pt-4 text-xs text-stone-500">
                    <div className="truncate">{selectedMetadata.key}</div>
                    <div className="mt-1">
                      {t("pricing in/out:")}{" "}
                      {priceText(selectedMetadata.pricing.input)} /{" "}
                      {priceText(selectedMetadata.pricing.output)}
                    </div>
                  </div>
                ) : null}
              </form>
            </>
          ) : null}

          <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="border-b border-stone-200 px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <input
                  className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => setModelSearch(event.target.value)}
                  placeholder={t("Search model metadata")}
                  value={modelSearch}
                />
                <button
                  aria-label={t("Reload model metadata cache")}
                  className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoading}
                  onClick={() => void loadMetadata()}
                  title={t("Reload cache")}
                  type="button"
                >
                  {isLoading ? (
                    <LoaderCircle
                      aria-hidden="true"
                      className="size-4 animate-spin"
                    />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
            </div>
            <div className="panel-scroll max-h-80 overflow-y-auto">
              {filteredModels.length > 0 ? (
                filteredModels.map((model) => (
                  <button
                    className={`grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-stone-100 px-4 py-3 text-left hover:bg-stone-50 ${
                      selectedMetadataKey === model.key ? "bg-teal-50" : "bg-white/70"
                    }`}
                    key={model.key}
                    onClick={() => selectMetadataModel(model.key)}
                    type="button"
                  >
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-semibold text-stone-950">
                        {model.name}
                      </span>
                      <span className="mt-1 block truncate text-xs font-medium text-stone-500">
                        {model.providerName} / {model.modelId}
                      </span>
                    </span>
                    <span className="text-right text-xs font-medium text-stone-500">
                      {model.inputModalities.join(", ") || t("input n/a")}
                    </span>
                  </button>
                ))
              ) : (
                <div className="px-4 py-8 text-sm text-stone-500">
                  {isLoading ? t("Loading models...") : t("No cached models")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}
        </div>
      </div>
    </div>
  );
}

function FocoLogoMark() {
  return (
    <span className="foco-logo-mark inline-flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-white shadow-[0_10px_24px_rgba(15,118,110,0.2)] ring-1 ring-stone-200/80">
      <img
        alt=""
        aria-hidden="true"
        className="size-full object-cover"
        src="/foco.svg"
      />
    </span>
  );
}

function WorkspaceIcon({
  className = "size-4 shrink-0 rounded object-cover",
  fallbackClassName = "size-4 shrink-0",
  logoUrl,
}: {
  className?: string;
  fallbackClassName?: string;
  logoUrl: string | null | undefined;
}) {
  const [failedLogoUrl, setFailedLogoUrl] = useState<string | null>(null);
  const shouldShowLogo = Boolean(logoUrl && failedLogoUrl !== logoUrl);

  if (shouldShowLogo && logoUrl) {
    return (
      <img
        alt=""
        aria-hidden="true"
        className={className}
        onError={() => setFailedLogoUrl(logoUrl)}
        src={logoUrl}
      />
    );
  }

  return <Folder aria-hidden="true" className={fallbackClassName} />;
}

function workspaceItemClass(active: boolean) {
  return `workspace-item flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg px-2 text-sm font-semibold ${
    active ? "workspace-item-active text-teal-950" : "text-stone-700"
  }`;
}

function workspaceNameFromPath(path: string) {
  const trimmedPath = path.trim().replace(/[\\/]+$/g, "");
  const parts = trimmedPath.split(/[\\/]+/);

  return parts.at(-1) ?? "";
}

function workspaceMenuClass(active: boolean) {
  return `workspace-menu flex min-w-0 items-center gap-1 rounded-xl border px-1.5 py-1 transition-colors ${
    active
      ? "workspace-menu-active border-teal-200 bg-teal-50 text-teal-950 shadow-sm"
      : "border-transparent bg-stone-100/60 text-stone-700 hover:border-stone-200 hover:bg-white/90 hover:text-stone-950"
  }`;
}

function chatItemClass(active: boolean) {
  return `chat-item flex min-h-11 min-w-0 flex-1 items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-xs font-medium ${
    active
      ? "chat-item-active border-teal-100 bg-white text-stone-950 shadow-sm"
      : "border-transparent text-stone-600 hover:border-stone-200 hover:bg-white/80 hover:text-stone-950"
  }`;
}

function hydrateChatTab(
  tab: OpenChatTab,
  workspaces: WorkspaceSummary[],
): ChatTabSummary {
  const workspace = workspaces.find((workspace) => workspace.id === tab.workspaceId);
  const chat = workspace?.chats.find((chat) => chat.id === tab.chatId);

  return {
    ...tab,
    title: chat?.title ?? tab.fallbackTitle,
    workspaceLogoUrl: workspace?.logoUrl ?? null,
    workspaceName: workspace?.name ?? tab.fallbackWorkspaceName,
  };
}

function upsertOpenChatTab(tabs: OpenChatTab[], nextTab: OpenChatTab) {
  if (
    tabs.some(
      (tab) =>
        tab.workspaceId === nextTab.workspaceId && tab.chatId === nextTab.chatId,
    )
  ) {
    return tabs;
  }

  return [...tabs, nextTab];
}

function workspaceHasChat(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
) {
  return workspaces.some(
    (workspace) =>
      workspace.id === tab.workspaceId &&
      workspace.chats.some((chat) => chat.id === tab.chatId),
  );
}

function diffFileButtonClass(active: boolean) {
  return `diff-file-button flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-sm ${
    active
      ? "diff-file-button-active bg-teal-50 text-teal-950 shadow-sm"
      : "text-stone-700 hover:bg-stone-50 hover:text-stone-950"
  }`;
}

function settingsSectionTitle(section: SettingsSection, t: Translate) {
  if (section === "general") {
    return t("General settings");
  }

  if (section === "workspaces") {
    return t("Workspace settings");
  }

  if (section === "prompts") {
    return t("Prompt settings");
  }

  if (section === "hooks") {
    return t("Hook settings");
  }

  if (section === "memory") {
    return t("Memory settings");
  }

  if (section === "providers") {
    return t("Provider settings");
  }

  if (section === "models") {
    return t("Model settings");
  }

  if (section === "mcp") {
    return t("MCP settings");
  }

  return t("Skill settings");
}

function settingsSectionSubtitle(section: SettingsSection, t: Translate) {
  if (section === "general") {
    return t("Web service listen address");
  }

  if (section === "workspaces") {
    return t("Workspace order and terminal shell");
  }

  if (section === "prompts") {
    return t("Prompt files and extra instructions");
  }

  if (section === "hooks") {
    return t("Global and workspace lifecycle hooks");
  }

  if (section === "memory") {
    return t("Local memory graph and review queue");
  }

  if (section === "providers") {
    return t("Provider credentials and connection checks");
  }

  if (section === "mcp") {
    return t("Workspace-scoped MCP server runtimes");
  }

  if (section === "skills") {
    return t("Skill discovery and enablement");
  }

  return t("Model metadata and runtime limits");
}

function SettingsNavButton({
  active,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      aria-current={active ? "page" : undefined}
      className={`inline-flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left text-sm font-semibold ${
        active
          ? "bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)]"
          : "text-stone-600 hover:bg-stone-100 hover:text-stone-950"
      }`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      <span className="min-w-0 truncate">{label}</span>
    </button>
  );
}

function LoginView({
  error,
  isLoggingIn,
  onLogin,
  onPasswordChange,
  password,
}: {
  error: string | null;
  isLoggingIn: boolean;
  onLogin: (event: FormEvent<HTMLFormElement>) => void;
  onPasswordChange: (value: string) => void;
  password: string;
}) {
  const { t } = useI18n();

  return (
    <main className="app-root grid place-items-center bg-stone-100 px-4 text-stone-950">
      <form
        aria-label={t("Foco authentication")}
        className="w-full max-w-sm rounded-2xl border border-stone-200 bg-white/90 px-4 py-5 shadow-[0_24px_70px_rgba(33,31,28,0.16)]"
        onSubmit={onLogin}
      >
        <div className="flex items-center gap-3">
          <FocoLogoMark />
          <div className="min-w-0">
            <h1 className="text-lg font-semibold text-stone-950">Foco</h1>
            <p className="mt-1 text-xs font-medium text-stone-500">
              {t("Password required")}
            </p>
          </div>
        </div>
        <label className="mt-5 block">
          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
            {t("Password")}
          </span>
          <input
            autoComplete="current-password"
            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
            onChange={(event) => onPasswordChange(event.target.value)}
            type="password"
            value={password}
          />
        </label>
        {error ? (
          <div className="mt-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}
        <button
          aria-label={t("Log in")}
          className="mt-4 inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
          disabled={isLoggingIn || !password.trim()}
          type="submit"
        >
          {isLoggingIn ? (
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          ) : (
            <Lock aria-hidden="true" className="size-4" />
          )}
          {t("Log in")}
        </button>
      </form>
    </main>
  );
}

function TextField({
  inputMode,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
}: {
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  type?: "password" | "text";
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <input
        autoComplete="off"
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        inputMode={inputMode}
        name={label.toLowerCase().replace(/\s+/g, "-")}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        type={type}
        value={value}
      />
    </label>
  );
}

function SourceValueEditor({
  id,
  isExpanded,
  minHeightClass,
  onChange,
  onToggle,
  title,
  value,
  t,
}: {
  id: string;
  isExpanded: boolean;
  minHeightClass: string;
  onChange: (value: string) => void;
  onToggle: (id: string) => void;
  title: string;
  value: string;
  t: Translate;
}) {
  const parsed = parseDisplayJson(value);

  if (!parsed) {
    return (
      <textarea
        aria-label={title}
        className={`${minHeightClass} w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100`}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        value={value}
      />
    );
  }

  return (
    <div className="rounded-lg border border-stone-200 bg-stone-950 text-stone-100">
      <button
        aria-label={`${isExpanded ? t("Collapse JSON") : t("Expand JSON")} ${title}`}
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-xs font-semibold text-stone-200"
        onClick={() => onToggle(id)}
        type="button"
      >
        <span className="inline-flex min-w-0 items-center gap-2">
          <Code2 aria-hidden="true" className="size-3.5 shrink-0 text-teal-300" />
          <span className="truncate">{title}</span>
        </span>
        <span className="shrink-0 text-stone-400">
          {isExpanded ? t("Collapse JSON") : t("Expand JSON")}
        </span>
      </button>
      {isExpanded ? (
        <textarea
          aria-label={title}
          className={`${minHeightClass} w-full resize-y border-0 border-t border-stone-800 bg-stone-950 px-3 py-3 font-mono text-xs leading-relaxed text-stone-100 outline-none focus:ring-2 focus:ring-inset focus:ring-teal-500/40`}
          onChange={(event) => onChange(event.target.value)}
          spellCheck={false}
          value={parsed.pretty}
        />
      ) : (
        <div className="border-t border-stone-800 px-3 py-2 font-mono text-xs text-stone-400">
          <code>{jsonSyntaxNodes(compactToolText(parsed.pretty))}</code>
        </div>
      )}
    </div>
  );
}

function MemorySourceReadonlyDetails({
  source,
  t,
}: {
  source: MemorySourceRecord | undefined;
  t: Translate;
}) {
  if (!source) {
    return null;
  }

  return (
    <div className="grid gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3 text-xs text-stone-600 sm:grid-cols-2">
      <div>
        <span className="font-semibold text-stone-700">{t("Memory scope")}: </span>
        {memoryScopeLabel(source.scope, t)}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Chat ID")}: </span>
        {source.chatId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Source type")}: </span>
        {source.sourceType}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Source ID")}: </span>
        {source.sourceId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Created")}: </span>
        {source.createdAt}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Updated")}: </span>
        {source.updatedAt}
      </div>
    </div>
  );
}

function memoryKindLabel(kind: string, t: Translate) {
  switch (kind) {
    case "constraint":
      return t("Constraint");
    case "episode":
      return t("Episode");
    case "preference":
      return t("Preference");
    case "procedure":
      return t("Procedure");
    case "project_decision":
      return t("Project decision");
    case "project_fact":
      return t("Project fact");
    case "user_note":
      return t("User note");
    default:
      return kind;
  }
}

function memoryScopeLabel(scope: string, t: Translate) {
  switch (scope) {
    case "chat":
      return t("Chat memory");
    case "global":
      return t("Global memory");
    case "workspace":
      return t("Workspace memory");
    default:
      return scope;
  }
}

function memoryStatusLabel(status: string, t: Translate) {
  switch (status) {
    case "active":
      return t("Active");
    case "expired":
      return t("Expired");
    case "pending":
      return t("Pending review");
    case "rejected":
      return t("Rejected");
    case "superseded":
      return t("Superseded");
    default:
      return status;
  }
}

function CapabilityPill({
  className,
  label,
  ok,
  title,
}: {
  className?: string;
  label: string;
  ok: boolean;
  title?: string;
}) {
  return (
    <span
      className={`inline-flex min-h-6 max-w-full items-center rounded-md border px-2 py-0.5 text-xs font-semibold ${
        ok
          ? "border-teal-200 bg-teal-50 text-teal-800"
          : "border-stone-200 bg-stone-50 text-stone-500"
      } ${className ?? ""}`}
      title={title}
    >
      <span className="min-w-0 truncate">{label}</span>
    </span>
  );
}

function Warnings({ warnings }: { warnings: string[] }) {
  if (!warnings.length) {
    return null;
  }

  return (
    <div className="mt-3 space-y-1">
      {warnings.map((warning) => (
        <div
          className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800"
          key={warning}
        >
          <CircleAlert aria-hidden="true" className="size-4 shrink-0" />
          <span className="min-w-0 break-words">{warning}</span>
        </div>
      ))}
    </div>
  );
}

function emptyModelForm(): ModelFormState {
  return {
    displayName: "",
    enabled: false,
    maxOutputTokens: "",
    modelId: "",
    contextWindow: "",
    providerIds: [],
    activeProviderId: "",
    thinkingLevel: "",
  };
}

function emptyProviderForm(): ProviderFormState {
  return {
    apiKey: "",
    apiProxyEnabled: false,
    apiProxyType: "http",
    apiProxyUrl: "",
    baseUrl: "",
    clearApiKey: false,
    enabled: true,
    id: "",
    kind: "",
    name: "",
  };
}

function emptyGeneralForm(): GeneralFormState {
  return {
    hookAuditEnabled: false,
    language: "en",
    listenHost: "127.0.0.1",
    listenPort: "3210",
    password: "",
    theme: "light",
  };
}

function emptyPromptSettingsForm(): PromptSettingsFormState {
  return {
    extraText: "",
    files: [],
    pendingFile: "",
  };
}

function emptyMemorySettingsForm(): MemorySettingsFormState {
  return {
    enabled: false,
    extractionMode: "manual",
    retrievalMode: "fts",
    extractionModelId: "",
    retrievalModelId: "",
    retentionDays: "",
  };
}

function emptyMemoryFilter(): MemoryFilterState {
  return {
    chatId: "",
    kind: "",
    page: 1,
    pageSize: 20,
    query: "",
    scope: "global",
    status: "active",
    workspaceId: "",
  };
}

function emptyManualMemoryForm(): ManualMemoryFormState {
  return {
    chatId: "",
    confidence: "",
    fact: "",
    kind: "user_note",
    metadataText: "{}",
    pinned: false,
    scope: "global",
    workspaceId: "",
  };
}

function emptyWorkspaceForm(): WorkspaceFormState {
  return {
    commonCommands: [],
    id: "",
    name: "",
    path: "",
    pinned: false,
    terminalShell: "",
  };
}

function emptyMcpServerForm(): McpServerFormState {
  return {
    argsText: "",
    command: "",
    enabled: true,
    id: "",
    name: "",
    transport: "",
    url: "",
  };
}

function emptyHookConfig(): HookConfig {
  return { disableAllHooks: false };
}

function emptyHookHandlerForm(): HookHandlerFormState {
  return {
    argsText: "",
    asyncHook: false,
    asyncRewake: false,
    command: "",
    enabled: true,
    event: "PreToolUse",
    groupIndex: null,
    handlerIndex: null,
    ifFilter: "",
    inputText: "",
    matcher: "",
    prompt: "",
    serverId: "",
    shell: "",
    statusMessage: "",
    timeout: "",
    toolName: "",
    type: "command",
    url: "",
  };
}

function hookConfigEntries(config: HookConfig | null | undefined) {
  if (!config) {
    return [];
  }

  return Object.entries(config).flatMap(([event, value]) => {
    if (event === "disableAllHooks" || !Array.isArray(value)) {
      return [];
    }

    return value.map((group, groupIndex) => ({
      event,
      group,
      groupIndex,
    }));
  });
}

function hookGroupsForEvent(config: HookConfig | null | undefined, event: string) {
  const value = config?.[event];
  return Array.isArray(value) ? value : [];
}

function hookHandlerFormFromConfig(
  event: string,
  groupIndex: number,
  handlerIndex: number,
  group: HookMatcherGroup,
  handler: HookHandler,
): HookHandlerFormState {
  return {
    argsText: (handler.args ?? []).join("\n"),
    asyncHook: Boolean(handler.async),
    asyncRewake: Boolean(handler.asyncRewake),
    command: handler.command ?? "",
    enabled: handler.enabled !== false,
    event,
    groupIndex,
    handlerIndex,
    ifFilter: handler.if ?? "",
    inputText:
      typeof handler.input === "undefined" || handler.input === null
        ? ""
        : JSON.stringify(handler.input, null, 2),
    matcher: group.matcher ?? "",
    prompt: handler.prompt ?? "",
    serverId: handler.serverId ?? "",
    shell: handler.shell ?? "",
    statusMessage: handler.statusMessage ?? "",
    timeout: numberInputValue(handler.timeout ?? null),
    toolName: handler.toolName ?? "",
    type: hookHandlerType(handler.type),
    url: handler.url ?? "",
  };
}

function hookHandlerType(type: string): HookHandlerType {
  return type === "http" || type === "mcp_tool" || type === "prompt"
    ? type
    : "command";
}

function upsertHookHandlerInConfig(
  config: HookConfig,
  form: HookHandlerFormState,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const event = form.event;
  const nextHandler = hookHandlerFromForm(form);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  const existingGroupIndex =
    form.groupIndex !== null && form.event === event ? form.groupIndex : null;
  const groupIndex =
    existingGroupIndex !== null && groups[existingGroupIndex]
      ? existingGroupIndex
      : groups.findIndex((group) => (group.matcher ?? "") === form.matcher);

  if (groupIndex >= 0) {
    const group = groups[groupIndex];
    group.enabled = form.enabled;
    group.matcher = optionalText(form.matcher);
    if (form.handlerIndex !== null && group.hooks[form.handlerIndex]) {
      group.hooks[form.handlerIndex] = nextHandler;
    } else {
      group.hooks = [...group.hooks, nextHandler];
    }
  } else {
    groups.push({
      enabled: form.enabled,
      hooks: [nextHandler],
      matcher: optionalText(form.matcher),
    });
  }

  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function hookHandlerFromForm(form: HookHandlerFormState): HookHandler {
  const timeout = optionalPositiveInteger(form.timeout, "Timeout");
  const input = form.inputText.trim()
    ? parseJsonText(form.inputText, "Input override JSON")
    : null;
  const base: HookHandler = {
    enabled: form.enabled,
    type: form.type,
    async: form.asyncHook,
    asyncRewake: form.asyncRewake,
    if: optionalText(form.ifFilter),
    input,
    statusMessage: optionalText(form.statusMessage),
    timeout,
  };

  if (form.type === "command") {
    return {
      ...base,
      args: form.argsText
        .split(/\r?\n/)
        .map((arg) => arg.trim())
        .filter(Boolean),
      command: form.command.trim(),
      shell: optionalText(form.shell),
    };
  }

  if (form.type === "http") {
    return {
      ...base,
      url: form.url.trim(),
    };
  }

  if (form.type === "mcp_tool") {
    return {
      ...base,
      serverId: form.serverId.trim(),
      toolName: form.toolName.trim(),
    };
  }

  return {
    ...base,
    prompt: form.prompt.trim(),
  };
}

function deleteHookHandlerFromConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  const group = groups[groupIndex];
  if (!group) {
    return nextConfig;
  }

  group.hooks = group.hooks.filter((_, index) => index !== handlerIndex);
  if (!group.hooks.length) {
    groups.splice(groupIndex, 1);
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function updateHookGroupInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  patch: Partial<HookMatcherGroup>,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  if (groups[groupIndex]) {
    groups[groupIndex] = { ...groups[groupIndex], ...patch };
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function updateHookHandlerInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
  patch: Partial<HookHandler>,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  const handler = groups[groupIndex]?.hooks[handlerIndex];
  if (handler) {
    groups[groupIndex].hooks[handlerIndex] = { ...handler, ...patch };
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function moveHookGroupInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  direction: -1 | 1,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  const targetIndex = groupIndex + direction;
  if (!groups[groupIndex] || targetIndex < 0 || targetIndex >= groups.length) {
    return nextConfig;
  }
  [groups[groupIndex], groups[targetIndex]] = [
    groups[targetIndex],
    groups[groupIndex],
  ];
  nextConfig[event] = groups;
  return nextConfig;
}

function moveHookHandlerInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
  direction: -1 | 1,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event).map(cloneHookGroup);
  const handlers = groups[groupIndex]?.hooks;
  const targetIndex = handlerIndex + direction;
  if (!handlers || !handlers[handlerIndex] || targetIndex < 0 || targetIndex >= handlers.length) {
    return nextConfig;
  }
  [handlers[handlerIndex], handlers[targetIndex]] = [
    handlers[targetIndex],
    handlers[handlerIndex],
  ];
  nextConfig[event] = groups;
  return nextConfig;
}

function cloneHookConfig(config: HookConfig): HookConfig {
  const nextConfig: HookConfig = {
    disableAllHooks: Boolean(config.disableAllHooks),
  };

  for (const [event, value] of Object.entries(config)) {
    if (event === "disableAllHooks" || !Array.isArray(value)) {
      continue;
    }
    nextConfig[event] = value.map(cloneHookGroup);
  }

  return nextConfig;
}

function cloneHookGroup(group: HookMatcherGroup): HookMatcherGroup {
  return {
    ...group,
    hooks: group.hooks.map((handler) => ({ ...handler })),
  };
}

function compactHookConfig(config: HookConfig): HookConfig {
  const nextConfig: HookConfig = {
    disableAllHooks: Boolean(config.disableAllHooks),
  };

  for (const [event, value] of Object.entries(config)) {
    if (event === "disableAllHooks" || !Array.isArray(value) || !value.length) {
      continue;
    }
    nextConfig[event] = value;
  }

  return nextConfig;
}

function hookHandlerSummary(handler: HookHandler) {
  return (
    handler.command ||
    handler.url ||
    [handler.serverId, handler.toolName].filter(Boolean).join(" / ") ||
    handler.prompt ||
    ""
  );
}

function hookEventLabel(event: string, t: Translate) {
  const labels: Record<string, string> = {
    SessionStart: "Session start",
    SessionEnd: "Session end",
    UserPromptSubmit: "User prompt submit",
    PreToolUse: "Pre tool use",
    PermissionRequest: "Permission request",
    PermissionDenied: "Permission denied",
    PostToolUse: "Post tool use",
    PostToolUseFailure: "Post tool use failure",
    PostToolBatch: "Post tool batch",
    Stop: "Stop",
    StopFailure: "Stop failure",
    PreCompact: "Pre compact",
    PostCompact: "Post compact",
    Elicitation: "Elicitation",
    ElicitationResult: "Elicitation result",
  };

  return t(labels[event] ?? event);
}

function hookHandlerTypeLabel(type: string, t: Translate) {
  switch (type) {
    case "command":
      return t("Command");
    case "http":
      return t("HTTP");
    case "mcp_tool":
      return t("MCP tool");
    case "prompt":
      return t("Prompt");
    default:
      return type;
  }
}

function hookSourceLabel(source: string, t: Translate) {
  switch (source) {
    case "global":
      return t("Global");
    case "workspace":
      return t("Workspace");
    default:
      return source;
  }
}

function hookRunStatusLabel(status: string, t: Translate) {
  switch (status) {
    case "succeeded":
      return t("succeeded");
    case "failed":
      return t("failed");
    case "error":
      return t("error");
    case "blocked":
      return t("blocked");
    case "running":
      return t("running");
    case "cancelled":
      return t("cancelled");
    default:
      return status;
  }
}

function latestHookRunForSummary(
  hook: EffectiveHookSummary,
  runs: HookRunSummaryRow[],
) {
  return runs.find(
    (run) =>
      run.event === hook.event &&
      run.hookSource === hook.source &&
      run.handlerType === hook.handlerType,
  );
}

function parseJsonText(value: string, label: string): JsonValue {
  const parsed = JSON.parse(value) as unknown;
  if (!isJsonValue(parsed)) {
    throw new Error(`${label} must be JSON-compatible`);
  }
  return parsed;
}

function prettyJsonText(value: string) {
  try {
    return JSON.stringify(JSON.parse(value) as JsonValue, null, 2);
  } catch {
    return value || "{}";
  }
}

function parseDisplayJson(value: string) {
  const normalized = normalizedJsonValue(value);
  if (normalized === value) {
    return null;
  }
  return { pretty: formatJsonValue(normalized) };
}

function jsonSyntaxNodes(value: string) {
  const tokenPattern =
    /("(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"(?=\s*:)|"(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"|true|false|null|-?\d+(?:\.\d*)?(?:[eE][+-]?\d+)?)/g;
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenPattern.exec(value)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(value.slice(lastIndex, match.index));
    }
    const token = match[0];
    nodes.push(
      <span className={jsonTokenClass(token)} key={`${match.index}-${token}`}>
        {token}
      </span>,
    );
    lastIndex = match.index + token.length;
  }

  if (lastIndex < value.length) {
    nodes.push(value.slice(lastIndex));
  }

  return nodes;
}

function jsonTokenClass(token: string) {
  if (token.startsWith('"')) {
    return token.endsWith('":') ? "text-sky-300" : "text-emerald-300";
  }
  if (token === "true" || token === "false") {
    return "text-amber-300";
  }
  if (token === "null") {
    return "text-stone-500";
  }
  return "text-violet-300";
}

function memorySourceRecordsToForm(sources: MemorySourceRecord[]): MemorySourceFormState[] {
  return sources.map((source) => ({
    content: source.content,
    id: source.id,
    metadataText: prettyJsonText(source.metadataJson),
    title: source.title,
  }));
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function nextProviderId(
  name: string,
  kind: string,
  providers: ConfiguredProviderSummary[],
) {
  const base = slugId(name) || slugId(kind);
  const existingIds = new Set(providers.map((provider) => provider.id));

  if (!existingIds.has(base)) {
    return base;
  }

  let index = 2;
  while (existingIds.has(`${base}-${index}`)) {
    index += 1;
  }

  return `${base}-${index}`;
}

function nextMcpServerId(
  name: string,
  transport: string,
  servers: ConfiguredMcpServerSummary[],
) {
  const base = slugId(name) || slugId(transport);
  const existingIds = new Set(servers.map((server) => server.id));

  if (!existingIds.has(base)) {
    return base;
  }

  let index = 2;
  while (existingIds.has(`${base}-${index}`)) {
    index += 1;
  }

  return `${base}-${index}`;
}

function moveItemId(
  itemIds: string[],
  sourceItemId: string,
  targetItemId: string,
) {
  const sourceIndex = itemIds.indexOf(sourceItemId);
  const targetIndex = itemIds.indexOf(targetItemId);

  if (sourceIndex === -1 || targetIndex === -1 || sourceIndex === targetIndex) {
    return itemIds;
  }

  const next = [...itemIds];
  const [source] = next.splice(sourceIndex, 1);
  next.splice(targetIndex, 0, source);

  return next;
}

function groupedWorkspaceIds(workspaces: ConfiguredWorkspaceSummary[]) {
  return [
    ...workspaces
      .filter((workspace) => workspace.pinned)
      .map((workspace) => workspace.id),
    ...workspaces
      .filter((workspace) => !workspace.pinned)
      .map((workspace) => workspace.id),
  ];
}

function terminalShellLabel(
  terminalShells: TerminalShellSummary[],
  terminalShell: string,
) {
  return (
    terminalShells.find((shell) => shell.shell === terminalShell)?.label ??
    terminalShell
  );
}

function sameStringList(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function slugId(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function activeSkillQuery(value: string) {
  const match = /(^|\s)\/([^\s/]*)$/.exec(value);
  return match ? match[2] : null;
}

function removeActiveSkillToken(value: string) {
  return value.replace(/(^|\s)\/[^\s/]*$/, (_match, prefix: string) => prefix);
}

function selectedSkillPrefix(content: string, isUser: boolean) {
  if (!isUser) {
    return null;
  }

  let remaining = content.trimStart();
  const skills: Array<{ name: string; path: string }> = [];

  while (true) {
    const match = /^\[\$([^\]\n]+)\]\(([^)\n]+)\)(?:\s+|$)/.exec(remaining);
    if (!match) {
      break;
    }

    const path = decodeMarkdownHref(match[2].trim());
    if (!path.replaceAll("\\", "/").endsWith("SKILL.md")) {
      break;
    }

    skills.push({
      name: match[1].trim(),
      path,
    });
    remaining = remaining.slice(match[0].length);
  }

  if (!skills.length) {
    return null;
  }

  return {
    remaining,
    skills,
  };
}

function decodeMarkdownHref(value: string) {
  try {
    return decodeURI(value);
  } catch {
    return value;
  }
}

function messageWithSelectedSkills(
  skills: ConfiguredSkillSummary[],
  skillIds: string[],
  message: string,
) {
  const links = skillIds
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill))
    .map((skill) => `[$${skill.name}](${skill.path})`);

  return links.length ? `${links.join(" ")} ${message}` : message;
}

async function fileToBase64(file: File): Promise<string> {
  return arrayBufferToBase64(await file.arrayBuffer());
}

async function fileToComposerAttachment(file: File): Promise<ComposerAttachment> {
  const name = file.name.trim();
  const contentType = fileContentType(file);

  if (!name) {
    throw new Error("attachment name must not be empty");
  }

  if (!contentType) {
    throw new Error(`attachment ${name} content type is missing`);
  }

  const contentBase64 = arrayBufferToBase64(await file.arrayBuffer());
  const previewDataUrl = contentType.startsWith("image/")
    ? `data:${contentType};base64,${contentBase64}`
    : null;

  return {
    id: localChatAttachmentId(),
    name,
    contentType,
    contentBase64,
    path: undefined,
    previewDataUrl,
    sizeBytes: file.size,
  };
}

function nativeSelectedFileToComposerAttachment(
  file: NativeSelectedFile,
): ComposerAttachment {
  const name = file.name.trim();
  const contentType = file.contentType.trim();
  const path = file.path.trim();

  if (!name) {
    throw new Error("attachment name must not be empty");
  }

  if (!contentType) {
    throw new Error(`attachment ${name} content type is missing`);
  }

  if (!path) {
    throw new Error(`attachment ${name} path is missing`);
  }

  const isImage = contentType.startsWith("image/");
  const contentBase64 = isImage ? file.contentBase64?.trim() : undefined;
  if (isImage && !contentBase64) {
    throw new Error(`attachment ${name} image content is missing`);
  }

  return {
    id: localChatAttachmentId(),
    name,
    contentBase64,
    contentType,
    path: isImage ? undefined : path,
    previewDataUrl: isImage
      ? `data:${contentType};base64,${contentBase64}`
      : null,
    sizeBytes: file.sizeBytes,
  };
}

function fileContentType(file: File) {
  const explicitType = file.type.trim();
  if (explicitType) {
    return explicitType;
  }

  const extension = file.name.trim().toLowerCase().split(".").pop() ?? "";
  const extensionTypes: Record<string, string> = {
    bat: "text/plain",
    c: "text/plain",
    cmd: "text/plain",
    cpp: "text/plain",
    cs: "text/plain",
    css: "text/css",
    csv: "text/csv",
    go: "text/plain",
    h: "text/plain",
    hpp: "text/plain",
    htm: "text/html",
    html: "text/html",
    java: "text/plain",
    js: "text/javascript",
    json: "application/json",
    jsx: "text/javascript",
    md: "text/markdown",
    pdf: "application/pdf",
    ps1: "text/plain",
    py: "text/x-python",
    rs: "text/plain",
    sh: "text/x-shellscript",
    toml: "application/toml",
    ts: "text/typescript",
    tsx: "text/typescript",
    txt: "text/plain",
    xml: "application/xml",
    yaml: "application/yaml",
    yml: "application/yaml",
  };

  return extensionTypes[extension] ?? "";
}

function localChatAttachmentId() {
  return (
    globalThis.crypto?.randomUUID?.() ??
    `attachment-${Date.now()}-${Math.random().toString(36).slice(2)}`
  );
}

function arrayBufferToBase64(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";

  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }

  return btoa(binary);
}

function chatAttachmentPayload(
  attachment: ComposerAttachment,
): ChatAttachmentPayload {
  const payload: ChatAttachmentPayload = {
    id: attachment.id,
    name: attachment.name,
    contentType: attachment.contentType,
    sizeBytes: attachment.sizeBytes,
  };
  if (attachment.contentBase64) {
    payload.contentBase64 = attachment.contentBase64;
  }
  if (attachment.path) {
    payload.path = attachment.path;
  }

  return payload;
}

function userMessageParts(
  content: string,
  attachments: ChatAttachmentPayload[],
): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [];
  if (content) {
    parts.push({ type: "text", text: content });
  }
  parts.push(
    ...attachments.map((attachment) => ({
      type: "attachment" as const,
      attachment: attachmentPartFromPayload(attachment),
    })),
  );
  return parts;
}

function attachmentPartFromPayload(
  attachment: ChatAttachmentPayload,
): ChatAttachmentPartSummary {
  return {
    id: attachment.id,
    name: attachment.name,
    contentType: attachment.contentType,
    path: attachment.path ?? null,
    previewDataUrl: attachment.contentType.startsWith("image/") &&
      attachment.contentBase64
      ? `data:${attachment.contentType};base64,${attachment.contentBase64}`
      : null,
    sizeBytes: attachment.sizeBytes,
  };
}

function formatFileSize(sizeBytes: number) {
  const units = ["B", "KB", "MB", "GB"];
  let value = sizeBytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const formatted =
    unitIndex === 0 || value >= 10 ? value.toFixed(0) : value.toFixed(1);
  return `${formatted} ${units[unitIndex]}`;
}

function skillScopeLabel(skill: ConfiguredSkillSummary, t: Translate) {
  if (skill.scope === "global") {
    return t("Global skill");
  }

  return skill.workspaceName
    ? t("Workspace skill {name}", { name: skill.workspaceName })
    : t("Workspace skill");
}

function uniqueString(value: string, index: number, values: string[]) {
  return values.indexOf(value) === index;
}

function numberInputValue(value: number | null) {
  return value === null ? "" : String(value);
}

function optionalPositiveInteger(value: string, label: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`${label} must be a positive whole number`);
  }

  const numberValue = Number(trimmed);

  if (!Number.isSafeInteger(numberValue) || numberValue <= 0) {
    throw new Error(`${label} must be a positive whole number`);
  }

  return numberValue;
}

function optionalNumber(value: string, label: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  const numberValue = Number(trimmed);

  if (!Number.isFinite(numberValue)) {
    throw new Error(`${label} must be a number`);
  }

  return numberValue;
}

function upsertToolCall(
  toolCalls: ChatToolCallSummary[],
  nextToolCall: ChatToolCallSummary,
) {
  const normalizedToolCall = normalizedToolCallSummary(nextToolCall);
  const existingIndex = toolCalls.findIndex(
    (toolCall) => toolCall.id === normalizedToolCall.id,
  );

  if (existingIndex === -1) {
    return [...toolCalls, normalizedToolCall];
  }

  return toolCalls.map((toolCall, index) =>
    index === existingIndex ? normalizedToolCall : toolCall,
  );
}

function applyToolResult(
  toolCalls: ChatToolCallSummary[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
) {
  return toolCalls.map((toolCall) =>
    toolCall.id === toolCallId
      ? {
          ...toolCall,
          output,
          isError,
          status: isError ? "error" : "completed",
        }
    : toolCall,
  );
}

function completedAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "complete" }>,
): ShellMessage {
  let parts = message.parts;
  const nextReasoning = streamEvent.reasoning ?? null;
  const reasoningDelta = missingFinalSuffix(message.reasoning ?? "", nextReasoning ?? "");
  if (reasoningDelta) {
    parts = appendReasoningPart(parts, reasoningDelta);
  }
  const textDelta = missingFinalSuffix(message.content, streamEvent.text);
  if (textDelta) {
    parts = appendTextPart(parts, textDelta);
  }

  return {
    ...message,
    content: streamEvent.text,
    metrics: streamEvent.metrics,
    memoriesUsed: streamEvent.memoriesUsed,
    extractedMemories: message.extractedMemories,
    reasoning: nextReasoning,
    status: undefined,
    parts: parts.length
      ? parts
      : fallbackMessageParts({
          ...message,
          content: streamEvent.text,
          reasoning: nextReasoning,
          status: undefined,
        }),
  };
}

function completedGuidanceAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "complete" }>,
): ShellMessage {
  return {
    ...message,
    metrics: streamEvent.metrics,
    memoriesUsed: streamEvent.memoriesUsed,
    extractedMemories: message.extractedMemories,
    status: undefined,
    parts: message.parts.length ? message.parts : fallbackMessageParts(message),
  };
}

function assistantMessageWithAppendedError(
  message: ShellMessage,
  errorText: string,
): ShellMessage {
  const hasVisibleContent =
    Boolean(message.content || message.reasoning || message.parts.length) ||
    message.toolCalls.length > 0;
  const separator = hasVisibleContent ? "\n\n" : "";
  const existingParts = message.parts.length
    ? message.parts
    : fallbackMessageParts(message);

  return {
    ...message,
    content: message.content
      ? `${message.content}${separator}${errorText}`
      : errorText,
    parts: appendErrorPart(existingParts, errorText),
    metrics: null,
    memoriesUsed: [],
    extractedMemories: [],
    status: hasVisibleContent ? undefined : "error",
  };
}

function isEmptyStreamingAssistantMessage(message: ShellMessage) {
  return (
    message.role === "assistant" &&
    message.status === "streaming" &&
    !message.content &&
    !message.reasoning &&
    message.parts.length === 0 &&
    message.toolCalls.length === 0
  );
}

function missingFinalSuffix(current: string, next: string) {
  if (!next || current === next) {
    return "";
  }

  return next.startsWith(current) ? next.slice(current.length) : "";
}

function compactInlineText(value: string) {
  return value.replace(/\s+/g, " ").trim();
}

function appendTextPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "text") {
    return [...parts, { type: "text", text }];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function appendErrorPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "error") {
    return [...parts, { type: "error", text }];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function appendReasoningPart(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "reasoning") {
    return [...parts, { type: "reasoning", text }];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function upsertToolCallPart(
  parts: ChatMessagePart[],
  nextToolCall: ChatToolCallSummary,
): ChatMessagePart[] {
  const normalizedToolCall = normalizedToolCallSummary(nextToolCall);
  const nextPart: ChatMessagePart = {
    type: "toolCall",
    toolCall: normalizedToolCall,
  };
  const existingIndex = parts.findIndex(
    (part) =>
      part.type === "toolCall" && part.toolCall.id === normalizedToolCall.id,
  );

  if (existingIndex === -1) {
    return [...parts, nextPart];
  }

  return parts.map((part, index) =>
    index === existingIndex ? nextPart : part,
  );
}

function applyToolResultToParts(
  parts: ChatMessagePart[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
): ChatMessagePart[] {
  return parts.map((part) =>
    part.type === "toolCall" && part.toolCall.id === toolCallId
      ? ({
          type: "toolCall",
          toolCall: {
            ...part.toolCall,
            output,
            isError,
            status: isError ? "error" : "completed",
          },
        } satisfies ChatMessagePart)
      : part,
  );
}

function fallbackMessageParts(
  message: ShellMessage | ChatMessageSummary,
): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [];
  if (message.reasoning) {
    parts.push({ type: "reasoning", text: message.reasoning });
  }
  if (message.content) {
    parts.push({ type: "text", text: message.content });
  }
  parts.push(
    ...message.toolCalls.map((toolCall) => ({
      type: "toolCall" as const,
      toolCall: normalizedToolCallSummary(toolCall),
    })),
  );
  return parts;
}

function messageCopyText(
  message: ShellMessage,
  parts: ChatMessagePart[],
): string {
  const content = message.content.trim();
  if (content) {
    return message.content;
  }

  return parts
    .map((part) => {
      if (
        part.type === "text" ||
        part.type === "reasoning" ||
        part.type === "error"
      ) {
        return part.text;
      }
      if (part.type === "attachment") {
        return part.attachment.path ?? part.attachment.name;
      }
      return `${part.toolCall.name} ${part.toolCall.status}`.trim();
    })
    .map((partText) => partText.trim())
    .filter(Boolean)
    .join("\n\n");
}

function normalizedToolCallSummary(
  toolCall: ChatToolCallSummary,
): ChatToolCallSummary {
  return {
    ...toolCall,
    input: normalizedToolInput(toolCall.input),
    output:
      toolCall.output === null ? null : normalizedJsonValue(toolCall.output),
  };
}

function toolStatusText(toolCall: ChatToolCallSummary, t: Translate) {
  if (toolCall.isError) {
    return t("error");
  }

  if (toolCall.status === "completed") {
    return t("completed");
  }

  return toolCall.status;
}

function toolCallDetailText(toolCall: ChatToolCallSummary) {
  const input = normalizedToolInput(toolCall.input);

  if (!isObjectRecord(input)) {
    return compactToolJson(input);
  }

  if (toolCall.name === "run_command") {
    const command = textField(input, "command");
    const args = stringArrayField(input, "args") ?? [];
    const cwd = textField(input, "cwd");

    if (command) {
      const fullCommand = [command, ...args].map(formatCommandPart).join(" ");
      return compactToolText(cwd && cwd !== "." ? `${fullCommand} | cwd: ${cwd}` : fullCommand);
    }
  }

  if (toolCall.name === "memory_search") {
    const scope = textField(input, "scope");
    const query = textField(input, "query");
    return compactToolText([scope, query].filter(Boolean).join(" | "));
  }

  if (toolCall.name === "memory_write") {
    const scope = textField(input, "scope");
    const kind = textField(input, "kind");
    const fact = textField(input, "fact");
    return compactToolText([scope, kind, fact].filter(Boolean).join(" | "));
  }

  const parts = [
    textField(input, "path"),
    textField(input, "query"),
    textField(input, "symbol"),
    numberTextField(input, "symbolId", "symbol_id"),
    numberTextField(input, "durationMs", "duration_ms"),
  ].filter(Boolean);
  const pathIndex = parts.findIndex((part) => part === textField(input, "path"));
  const startLine = numberTextField(input, "startLine", "start_line");
  const endLine = numberTextField(input, "endLine", "end_line");

  if (pathIndex !== -1 && startLine && endLine) {
    parts[pathIndex] = `${parts[pathIndex]}:${startLine}-${endLine}`;
  }

  return parts.length ? compactToolText(parts.join(" | ")) : compactToolJson(input);
}

function normalizedToolInput(value: JsonValue): JsonValue {
  const normalized = normalizedJsonValue(value);
  if (!isObjectRecord(normalized)) {
    return normalized;
  }

  for (const fieldName of ["arguments", "args", "input"]) {
    const nested = normalized[fieldName];
    if (!isJsonValue(nested)) {
      continue;
    }

    const normalizedNested = normalizedJsonValue(nested);
    if (isObjectRecord(normalizedNested)) {
      return normalizedNested;
    }
  }

  return normalized;
}

function textField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "string" ? field : null;
}

function numberTextField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "number" ? String(field) : null;
}

function stringArrayField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);

  if (field === null || typeof field === "undefined") {
    return null;
  }

  return Array.isArray(field) && field.every((item) => typeof item === "string")
    ? field
    : null;
}

function formatCommandPart(value: string) {
  if (value === "") {
    return '""';
  }

  return /^[A-Za-z0-9_./:=@%+,\-\\]+$/.test(value) ? value : JSON.stringify(value);
}

function compactToolJson(value: JsonValue) {
  return compactToolText(JSON.stringify(value));
}

function compactToolText(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 240 ? `${normalized.slice(0, 237)}...` : normalized;
}

function formatJsonValue(value: JsonValue) {
  return JSON.stringify(normalizedJsonValue(value), null, 2);
}

function normalizedJsonValue(value: JsonValue): JsonValue {
  let current = value;

  for (let index = 0; index < 4; index += 1) {
    if (typeof current !== "string") {
      return current;
    }

    const trimmed = current.trim();
    const looksLikeJson =
      trimmed.startsWith("{") ||
      trimmed.startsWith("[") ||
      trimmed.startsWith('"{') ||
      trimmed.startsWith('"[');
    if (!looksLikeJson) {
      return current;
    }

    try {
      const parsed = JSON.parse(trimmed);
      if (!isJsonValue(parsed)) {
        return current;
      }
      current = parsed;
    } catch {
      return current;
    }
  }

  return current;
}

function formatLimit(value: number | null, label: string, language: AppLanguageId = "en") {
  return value === null
    ? `${label} missing`
    : `${label} ${formatNumber(value, language)}`;
}

function emptyAiStatsFilters(): AiStatsFilterState {
  return {
    chatId: "",
    modelId: "",
    page: "1",
    pageSize: "20",
    providerId: "",
    startedAfter: "",
    startedBefore: "",
    status: "",
    workspaceId: "",
  };
}

function readAiStatsVisibleColumnIds(): Set<AiStatsColumnId> {
  const savedValue = window.localStorage.getItem(AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY);
  if (!savedValue) {
    return new Set(DEFAULT_AI_STATS_COLUMN_IDS);
  }

  const savedIds = JSON.parse(savedValue);
  if (!Array.isArray(savedIds)) {
    return new Set(DEFAULT_AI_STATS_COLUMN_IDS);
  }

  const visibleIds = savedIds.filter(isAiStatsColumnId);
  return new Set(visibleIds.length ? visibleIds : DEFAULT_AI_STATS_COLUMN_IDS);
}

function writeAiStatsVisibleColumnIds(visibleColumnIds: Set<AiStatsColumnId>) {
  const savedIds = AI_STATS_COLUMN_IDS.filter((columnId) =>
    visibleColumnIds.has(columnId),
  );
  window.localStorage.setItem(
    AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY,
    JSON.stringify(savedIds),
  );
}

function isAiStatsColumnId(value: unknown): value is AiStatsColumnId {
  return (
    typeof value === "string" &&
    (AI_STATS_COLUMN_IDS as readonly string[]).includes(value)
  );
}

function emptyAiStatisticsSummary(): AiStatisticsSummary {
  return {
    averageLatencyMs: null,
    failedRequests: 0,
    modelBreakdown: [],
    providerBreakdown: [],
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalRequests: 0,
    totalTokens: 0,
    trend: [],
  };
}

function aiOverviewQuery(filters: {
  startedAfter: string;
  startedBefore: string;
  workspaceId: string;
}) {
  const params = new URLSearchParams();
  if (filters.workspaceId) {
    params.set("workspaceId", filters.workspaceId);
  }
  const startedAfter = datetimeLocalToRfc3339(filters.startedAfter);
  if (startedAfter) {
    params.set("startedAfter", startedAfter);
  }
  const startedBefore = datetimeLocalToRfc3339(filters.startedBefore);
  if (startedBefore) {
    params.set("startedBefore", startedBefore);
  }
  params.set("page", "1");
  params.set("pageSize", "1");

  return params.toString();
}

function aiStatsQuery(filters: AiStatsFilterState) {
  const params = new URLSearchParams();
  const entries: [keyof AiStatsFilterState, string][] = [
    ["workspaceId", filters.workspaceId],
    ["chatId", filters.chatId],
    ["providerId", filters.providerId],
    ["modelId", filters.modelId],
    ["status", filters.status],
    ["startedAfter", datetimeLocalToRfc3339(filters.startedAfter)],
    ["startedBefore", datetimeLocalToRfc3339(filters.startedBefore)],
    ["page", filters.page.trim()],
    ["pageSize", filters.pageSize.trim()],
  ];

  for (const [key, value] of entries) {
    if (value) {
      params.set(key, value);
    }
  }

  return params.toString();
}

function chartColor(index: number) {
  return ANALYTICS_CHART_COLORS[index % ANALYTICS_CHART_COLORS.length];
}

function chartPayloadLabel(payload: unknown) {
  return isObjectRecord(payload) && typeof payload.label === "string"
    ? payload.label
    : "";
}

function chartPayloadDisplayValue(
  payload: unknown,
  valueFormatter: (value: number) => string,
  fallbackValue?: unknown,
) {
  if (isObjectRecord(payload) && typeof payload.displayValue === "string") {
    return payload.displayValue;
  }

  if (isObjectRecord(payload) && typeof payload.value === "number") {
    return valueFormatter(payload.value);
  }

  const numberValue = Number(fallbackValue);
  return Number.isFinite(numberValue) ? valueFormatter(numberValue) : "n/a";
}

function compactChartTick(value: unknown) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) {
    return String(value);
  }

  return formatCompactNumber(numberValue, "en");
}

function compactChartLabel(value: unknown) {
  const label = String(value);
  return label.length > 16 ? `${label.slice(0, 15)}...` : label;
}

function formatTrendBucket(bucket: string, language: AppLanguageId = "en") {
  const date = new Date(`${bucket}T00:00:00`);

  if (Number.isNaN(date.getTime())) {
    return bucket;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    month: "short",
  }).format(date);
}

function positiveIntegerText(value: string, fallback: number) {
  const parsed = Number(value);

  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function auditPaginationItems(
  currentPage: number,
  totalPages: number,
): Array<number | "ellipsis"> {
  if (totalPages <= 0) {
    return [];
  }

  const pages = new Set<number>([1, totalPages]);
  for (
    let page = Math.max(1, currentPage - 2);
    page <= Math.min(totalPages, currentPage + 2);
    page += 1
  ) {
    pages.add(page);
  }

  const sortedPages = Array.from(pages).sort((left, right) => left - right);
  const items: Array<number | "ellipsis"> = [];

  for (const page of sortedPages) {
    const previous = items[items.length - 1];
    if (typeof previous === "number" && page - previous > 1) {
      items.push("ellipsis");
    }
    items.push(page);
  }

  return items;
}

function datetimeLocalToRfc3339(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "";
  }

  const date = new Date(trimmed);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`invalid date time: ${value}`);
  }

  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function auditOptions(
  configuredOptions: { label: string; value: string }[],
  observedValues: string[],
  labelForObserved: (value: string) => string = (value) => value,
) {
  const optionsByValue = new Map<string, { label: string; value: string }>();

  for (const option of configuredOptions) {
    if (option.value) {
      optionsByValue.set(option.value, option);
    }
  }

  for (const value of observedValues) {
    if (value && !optionsByValue.has(value)) {
      optionsByValue.set(value, { label: labelForObserved(value), value });
    }
  }

  return Array.from(optionsByValue.values()).sort((left, right) =>
    left.label.localeCompare(right.label),
  );
}

function auditStatusText(status: string, t: Translate) {
  if (status === "succeeded" || status === "completed") {
    return t("succeeded");
  }

  if (status === "failed") {
    return t("failed");
  }

  return status;
}

function auditStatusClass(status: string) {
  const base = "inline-flex rounded-md px-2 py-1 text-xs font-semibold";

  if (status === "succeeded" || status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "failed") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function taskStatusClass(status: TaskStatus) {
  const base = "inline-flex rounded-md px-2 py-0.5 text-[11px] font-semibold";

  if (status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "running" || status === "ready") {
    return `${base} bg-sky-100 text-sky-800`;
  }

  if (status === "blocked") {
    return `${base} bg-amber-100 text-amber-800`;
  }

  if (status === "failed" || status === "cancelled") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function auditJsonText(value: JsonValue | null) {
  return value === null ? "null" : formatJsonValue(value);
}

function JsonTreeNode({
  collapsedPaths,
  depth,
  isLast,
  name,
  onToggle,
  path,
  value,
}: {
  collapsedPaths: Set<string>;
  depth: number;
  isLast: boolean;
  name?: string;
  onToggle: (path: string) => void;
  path: string;
  value: JsonValue | null;
}) {
  if (Array.isArray(value)) {
    return (
      <JsonContainerNode
        collapsedPaths={collapsedPaths}
        closeToken="]"
        depth={depth}
        entries={value.map((item, index) => [String(index), item] as [
          string,
          JsonValue,
        ])}
        isLast={isLast}
        name={name}
        onToggle={onToggle}
        openToken="["
        path={path}
        valueKind="array"
      />
    );
  }

  if (isObjectRecord(value)) {
    return (
      <JsonContainerNode
        collapsedPaths={collapsedPaths}
        closeToken="}"
        depth={depth}
        entries={Object.entries(value) as [string, JsonValue][]}
        isLast={isLast}
        name={name}
        onToggle={onToggle}
        openToken="{"
        path={path}
        valueKind="object"
      />
    );
  }

  return (
    <JsonLine depth={depth}>
      <JsonKey name={name} />
      <JsonPrimitive value={value} />
      {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
    </JsonLine>
  );
}

function JsonContainerNode({
  closeToken,
  collapsedPaths,
  depth,
  entries,
  isLast,
  name,
  onToggle,
  openToken,
  path,
  valueKind,
}: {
  closeToken: "]" | "}";
  collapsedPaths: Set<string>;
  depth: number;
  entries: [string, JsonValue][];
  isLast: boolean;
  name?: string;
  onToggle: (path: string) => void;
  openToken: "[" | "{";
  path: string;
  valueKind: "array" | "object";
}) {
  const { t } = useI18n();
  const isCollapsible = entries.length > 0;
  const isCollapsed = collapsedPaths.has(path);

  if (!isCollapsible) {
    return (
      <JsonLine depth={depth}>
        <JsonTogglePlaceholder />
        <JsonKey name={name} />
        <JsonPunctuation>{openToken}</JsonPunctuation>
        <JsonPunctuation>{closeToken}</JsonPunctuation>
        {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
      </JsonLine>
    );
  }

  return (
    <>
      <JsonLine depth={depth}>
        <button
          aria-label={
            isCollapsed ? t("Expand JSON node") : t("Collapse JSON node")
          }
          className="audit-json-node-toggle"
          onClick={() => onToggle(path)}
          type="button"
        >
          <ChevronRight
            aria-hidden="true"
            className={
              isCollapsed
                ? "audit-json-node-toggle-icon"
                : "audit-json-node-toggle-icon audit-json-node-toggle-icon-open"
            }
          />
        </button>
        <JsonKey name={name} />
        <JsonPunctuation>{openToken}</JsonPunctuation>
        {isCollapsed ? (
          <>
            <span className="audit-json-collapsed-marker">
              {jsonContainerSummary(valueKind, entries.length)}
            </span>
            <JsonPunctuation>{closeToken}</JsonPunctuation>
            {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
          </>
        ) : null}
      </JsonLine>
      {isCollapsed
        ? null
        : entries.map(([entryName, entryValue], index) => (
            <JsonTreeNode
              collapsedPaths={collapsedPaths}
              depth={depth + 1}
              isLast={index === entries.length - 1}
              key={jsonChildPath(path, entryName)}
              name={valueKind === "object" ? entryName : undefined}
              onToggle={onToggle}
              path={jsonChildPath(path, entryName)}
              value={entryValue}
            />
          ))}
      {isCollapsed ? null : (
        <JsonLine depth={depth}>
          <JsonTogglePlaceholder />
          <JsonPunctuation>{closeToken}</JsonPunctuation>
          {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
        </JsonLine>
      )}
    </>
  );
}

function JsonLine({
  children,
  depth,
}: {
  children: ReactNode;
  depth: number;
}) {
  return (
    <span
      className="audit-json-line"
      style={{ "--audit-json-depth": depth } as CSSProperties}
    >
      {children}
    </span>
  );
}

function JsonKey({ name }: { name?: string }) {
  if (typeof name === "undefined") {
    return null;
  }

  return (
    <>
      <span className="audit-json-token audit-json-token-key">
        {JSON.stringify(name)}
      </span>
      <JsonPunctuation>: </JsonPunctuation>
    </>
  );
}

function JsonPrimitive({ value }: { value: JsonValue | null }) {
  if (typeof value === "string") {
    return (
      <span className="audit-json-token audit-json-token-string">
        {JSON.stringify(value)}
      </span>
    );
  }

  if (typeof value === "number") {
    return (
      <span className="audit-json-token audit-json-token-number">
        {String(value)}
      </span>
    );
  }

  return (
    <span className="audit-json-token audit-json-token-literal">
      {value === null ? "null" : String(value)}
    </span>
  );
}

function JsonPunctuation({ children }: { children: ReactNode }) {
  return (
    <span className="audit-json-token audit-json-token-punctuation">
      {children}
    </span>
  );
}

function JsonTogglePlaceholder() {
  return <span aria-hidden="true" className="audit-json-node-toggle-spacer" />;
}

function collectJsonContainerPaths(value: JsonValue | null, path: string) {
  const paths: string[] = [];

  if (Array.isArray(value)) {
    if (value.length > 0) {
      paths.push(path);
    }
    value.forEach((item, index) => {
      paths.push(...collectJsonContainerPaths(item, jsonChildPath(path, index)));
    });
    return paths;
  }

  if (isObjectRecord(value)) {
    const entries = Object.entries(value) as [string, JsonValue][];
    if (entries.length > 0) {
      paths.push(path);
    }
    entries.forEach(([key, item]) => {
      paths.push(...collectJsonContainerPaths(item, jsonChildPath(path, key)));
    });
  }

  return paths;
}

function jsonChildPath(path: string, segment: string | number) {
  return `${path}/${encodeURIComponent(String(segment))}`;
}

function jsonContainerSummary(kind: "array" | "object", count: number) {
  return kind === "array" ? `... ${count} items ` : `... ${count} keys `;
}

function formatAuditDate(value: string, language: AppLanguageId = "en") {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    second: "2-digit",
    year: "numeric",
  }).format(date);
}

function formatTodoGraphDate(value: string, language: AppLanguageId = "en") {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
  }).format(date);
}

function formatNullableNumber(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : formatNumber(value, language);
}

function formatNullableCompactNumber(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : formatCompactNumber(value, language);
}

function formatNullableLatencySeconds(
  value: number | null,
  language: AppLanguageId = "en",
) {
  if (value === null) {
    return "n/a";
  }

  return `${new Intl.NumberFormat(language, {
    maximumFractionDigits: 2,
  }).format(value / 1000)} s`;
}

function formatTokensPerSecond(
  metrics: ChatReplyMetrics,
  language: AppLanguageId = "en",
) {
  if (
    metrics.outputTokens === null ||
    metrics.totalLatencyMs === null ||
    metrics.totalLatencyMs <= 0
  ) {
    return "n/a";
  }

  return new Intl.NumberFormat(language, {
    maximumFractionDigits: 2,
  }).format(metrics.outputTokens / (metrics.totalLatencyMs / 1000));
}

function formatPercent(value: number | null, language: AppLanguageId = "en") {
  if (value === null) {
    return "n/a";
  }

  return new Intl.NumberFormat(language, {
    maximumFractionDigits: 1,
    style: "percent",
  }).format(value);
}

function formatNumber(value: number, language: AppLanguageId = "en") {
  return new Intl.NumberFormat(language).format(value);
}

function formatCompactNumber(value: number, _language: AppLanguageId = "en") {
  return new Intl.NumberFormat("en", {
    maximumFractionDigits: 1,
    notation: "compact",
  }).format(value);
}

function formatChatCreatedAt(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(date);
}

function currentBrowserRoute(): BrowserRoute {
  if (typeof window === "undefined") {
    return { chatId: null, viewMode: "chat", workspaceId: null };
  }

  return browserRouteFromPathname(window.location.pathname);
}

function browserRouteFromPathname(pathname: string): BrowserRoute {
  const segments = pathname
    .split("/")
    .filter(Boolean)
    .map(decodePathSegment);

  if (segments[0] === "settings") {
    const section = settingsSectionFromPathSegment(segments[1]);
    return { section, viewMode: "settings" };
  }

  if (segments[0] === "stats") {
    return { viewMode: "stats" };
  }

  if (segments.length >= 2) {
    return {
      chatId: segments[1],
      viewMode: "chat",
      workspaceId: segments[0],
    };
  }

  if (segments.length === 1) {
    return { chatId: null, viewMode: "chat", workspaceId: segments[0] };
  }

  return { chatId: null, viewMode: "chat", workspaceId: null };
}

function browserPathForRoute(route: BrowserRoute) {
  if (route.viewMode === "settings") {
    return `/settings/${route.section}`;
  }

  if (route.viewMode === "stats") {
    return "/stats";
  }

  if (route.workspaceId && route.chatId) {
    return `/${encodeURIComponent(route.workspaceId)}/${encodeURIComponent(
      route.chatId,
    )}`;
  }

  if (route.workspaceId) {
    return `/${encodeURIComponent(route.workspaceId)}`;
  }

  return "/";
}

function decodePathSegment(segment: string) {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

function settingsSectionFromPathSegment(
  segment: string | undefined,
): SettingsSection {
  return SETTINGS_SECTION_IDS.includes(segment as SettingsSection)
    ? (segment as SettingsSection)
    : "general";
}

function chatRunKey(workspaceId: string, chatId: string) {
  return `${workspaceId}:${chatId}`;
}

function parseChatRunKey(chatKey: string) {
  const separatorIndex = chatKey.indexOf(":");
  if (separatorIndex <= 0 || separatorIndex === chatKey.length - 1) {
    return null;
  }

  return {
    workspaceId: chatKey.slice(0, separatorIndex),
    chatId: chatKey.slice(separatorIndex + 1),
  };
}

function pendingChatRunKey(workspaceId: string, runKey: string) {
  return `${workspaceId}:pending:${runKey}`;
}

function localUiId(prefix: string) {
  const suffix =
    globalThis.crypto?.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}-${suffix}`;
}

function priceText(value: number | null) {
  return value === null ? "n/a" : `$${value}`;
}

function statusLabel(file: GitStatusFileSummary) {
  const statuses = [file.indexStatus, file.worktreeStatus]
    .map(normalizeGitStatus)
    .filter(Boolean);
  const uniqueStatuses = [...new Set(statuses)];

  return uniqueStatuses.length ? uniqueStatuses.join("") : ".";
}

function normalizeGitStatus(status: string) {
  const trimmed = status.trim();
  if (!trimmed) {
    return "";
  }

  return trimmed === "?" ? "U" : trimmed;
}

type GitDiffSection = {
  kind: "staged" | "unstaged";
  files: GitDiffFile[];
};

type GitDiffFile = {
  isBinary: boolean;
  lines: GitDiffLine[];
  path: string;
};

type GitDiffLine = {
  kind: "add" | "context" | "hunk" | "meta" | "remove";
  prefix: string;
  text: string;
};

function parseGitDiffSections(diff: GitDiffResponse | null): GitDiffSection[] {
  if (!diff) {
    return [];
  }

  return [
    { kind: "staged" as const, text: diff.stagedDiff },
    { kind: "unstaged" as const, text: diff.diff },
  ]
    .map(({ kind, text }) => ({
      kind,
      files: parseGitDiffFiles(text),
    }))
    .filter((section) => section.files.length > 0);
}

function hasGitDiffStats(stats: GitDiffLineStats) {
  return stats.additions > 0 || stats.deletions > 0;
}

function parseGitDiffFiles(diffText: string): GitDiffFile[] {
  const files: GitDiffFile[] = [];
  let current: GitDiffFile | null = null;

  for (const line of diffText.split("\n")) {
    if (line.startsWith("diff --git ")) {
      if (current) {
        files.push(current);
      }
      current = {
        isBinary: false,
        lines: [],
        path: pathFromDiffHeader(line) ?? "",
      };
      continue;
    }

    if (!current) {
      continue;
    }

    if (line.startsWith("Binary files ")) {
      current.isBinary = true;
      continue;
    }

    if (line.startsWith("--- ") || line.startsWith("+++ ")) {
      const path = pathFromDiffMarker(line);
      if (path) {
        current.path = path;
      }
      continue;
    }

    if (line.startsWith("@@")) {
      current.lines.push({ kind: "hunk", prefix: "", text: line });
      continue;
    }

    if (line.startsWith("+")) {
      current.lines.push({ kind: "add", prefix: "+", text: line.slice(1) });
      continue;
    }

    if (line.startsWith("-")) {
      current.lines.push({ kind: "remove", prefix: "-", text: line.slice(1) });
      continue;
    }

    if (line.startsWith(" ")) {
      current.lines.push({ kind: "context", prefix: " ", text: line.slice(1) });
      continue;
    }

    if (line.startsWith("\\")) {
      current.lines.push({ kind: "meta", prefix: "", text: line });
    }
  }

  if (current) {
    files.push(current);
  }

  return files.filter((file) => file.path);
}

function pathFromDiffHeader(line: string) {
  const prefix = "diff --git a/";
  if (!line.startsWith(prefix)) {
    return null;
  }

  const rest = line.slice(prefix.length);
  const nextMarker = rest.indexOf(" b/");
  return nextMarker >= 0 ? rest.slice(0, nextMarker) : rest;
}

function pathFromDiffMarker(line: string) {
  const marker = line.slice(4);
  if (marker === "/dev/null") {
    return null;
  }

  if (marker.startsWith("a/") || marker.startsWith("b/")) {
    return marker.slice(2);
  }

  return marker;
}

function diffLineClass(kind: GitDiffLine["kind"]) {
  const base = "flex min-w-max px-3";

  if (kind === "add") {
    return `${base} bg-emerald-50 text-emerald-950`;
  }

  if (kind === "remove") {
    return `${base} bg-rose-50 text-rose-950`;
  }

  if (kind === "hunk") {
    return `${base} bg-sky-50 text-sky-900`;
  }

  if (kind === "meta") {
    return `${base} text-stone-500`;
  }

  return `${base} text-stone-700`;
}

function terminalStatusText(
  status: "closed" | "connected" | "connecting" | "error",
  t: Translate,
) {
  if (status === "connected") {
    return t("connected");
  }

  if (status === "connecting") {
    return t("connecting");
  }

  if (status === "error") {
    return t("error");
  }

  return t("closed");
}

function isTerminalConnected(status: TerminalPaneStatus) {
  return status === "connected";
}

function terminalStatusClass(status: "closed" | "connected" | "connecting" | "error") {
  const base = "rounded-md px-1.5 py-0.5 text-[11px] font-semibold";

  if (status === "connected") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "connecting") {
    return `${base} bg-stone-200 text-stone-700`;
  }

  if (status === "error") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-500`;
}

async function readChatStream(
  response: Response,
  onEvent: (event: ChatStreamEvent) => void,
) {
  if (!response.body) {
    throw new Error("chat stream response has no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    buffer += decoder.decode(value, { stream: true });
    buffer = readSseFrames(buffer, onEvent);
  }

  buffer += decoder.decode();
  readSseFrames(`${buffer}\n\n`, onEvent);
}

function readSseFrames(
  buffer: string,
  onEvent: (event: ChatStreamEvent) => void,
) {
  const normalized = buffer.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const frames = normalized.split("\n\n");
  const remaining = frames.pop() ?? "";

  for (const frame of frames) {
    const data = frame
      .split("\n")
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).trimStart())
      .join("\n");

    if (!data) {
      continue;
    }

    const parsed = JSON.parse(data) as unknown;
    const event = parseChatStreamEvent(parsed);
    if (!event) {
      throw new Error(
        `chat stream returned an unknown event: ${describeChatStreamEvent(parsed)}`,
      );
    }

    onEvent(event);
  }

  return remaining;
}

function parseChatStreamEvent(value: unknown): ChatStreamEvent | null {
  if (!isObjectRecord(value) || typeof value.type !== "string") {
    return null;
  }

  if (value.type === "start") {
    const chatId = stringField(value, "chatId", "chat_id");
    const userMessageId = stringField(value, "userMessageId", "user_message_id");
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const llmRequestId = optionalStringField(
      value,
      "llmRequestId",
      "llm_request_id",
    );
    const memoriesUsed = parseChatMemoriesUsed(
      fieldValue(value, "memoriesUsed", "memories_used"),
    );

    if (
      !chatId ||
      !userMessageId ||
      !assistantMessageId ||
      llmRequestId === null ||
      memoriesUsed === false
    ) {
      return null;
    }

    return {
      type: "start",
      chatId,
      userMessageId,
      assistantMessageId,
      llmRequestId,
      memoriesUsed,
    };
  }

  if (value.type === "textDelta" || value.type === "text_delta") {
    const assistantMessageId = optionalStringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const delta = stringField(value, "delta");

    if (assistantMessageId === null || delta === null) {
      return null;
    }

    return { type: "textDelta", assistantMessageId, delta };
  }

  if (value.type === "reasoningDelta" || value.type === "reasoning_delta") {
    const assistantMessageId = optionalStringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const delta = stringField(value, "delta");

    if (assistantMessageId === null || delta === null) {
      return null;
    }

    return { type: "reasoningDelta", assistantMessageId, delta };
  }

  if (value.type === "usage") {
    const usage = parseChatUsage(value.usage);

    if (usage === false) {
      return null;
    }

    return { type: "usage", usage };
  }

  if (value.type === "complete") {
    const chatId = stringField(value, "chatId", "chat_id");
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const text = stringField(value, "text");
    const reasoning = optionalNullableStringField(value, "reasoning");
    const usage = parseNullableChatUsage(fieldValue(value, "usage"));
    const stopReason = optionalNullableStringField(
      value,
      "stopReason",
      "stop_reason",
    );
    const metrics = parseRequiredChatReplyMetrics(fieldValue(value, "metrics"));
    const memoriesUsed = parseChatMemoriesUsed(
      fieldValue(value, "memoriesUsed", "memories_used"),
    );

    if (!chatId || !assistantMessageId || text === null) {
      return null;
    }

    if (
      reasoning === false ||
      usage === false ||
      stopReason === false ||
      metrics === false ||
      memoriesUsed === false
    ) {
      return null;
    }

    return {
      type: "complete",
      chatId,
      assistantMessageId,
      text,
      reasoning,
      usage,
      stopReason,
      metrics,
      memoriesUsed,
    };
  }

  if (value.type === "toolCall" || value.type === "tool_call") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const toolCall = parseChatToolCallSummary(
      fieldValue(value, "toolCall", "tool_call"),
    );

    if (!assistantMessageId || !toolCall) {
      return null;
    }

    return { type: "toolCall", assistantMessageId, toolCall };
  }

  if (value.type === "toolResult" || value.type === "tool_result") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const toolCallId = stringField(value, "toolCallId", "tool_call_id");
    const output = fieldValue(value, "output");
    const isError = fieldValue(value, "isError", "is_error");

    if (
      !assistantMessageId ||
      !toolCallId ||
      !isJsonValue(output) ||
      typeof isError !== "boolean"
    ) {
      return null;
    }

    return { type: "toolResult", assistantMessageId, toolCallId, output, isError };
  }

  if (value.type === "questionRequest" || value.type === "question_request") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const request = parseQuestionRequestSummary(fieldValue(value, "request"));

    if (!assistantMessageId || !request) {
      return null;
    }

    return { type: "questionRequest", assistantMessageId, request };
  }

  if (
    value.type === "hookNotification" ||
    value.type === "hook_notification"
  ) {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const notification = parseHookNotificationSummary(
      fieldValue(value, "notification"),
    );

    if (!assistantMessageId || !notification) {
      return null;
    }

    return { type: "hookNotification", assistantMessageId, notification };
  }

  if (
    value.type === "guidanceApplied" ||
    value.type === "guidance_applied"
  ) {
    const id = stringField(value, "id");
    const content = stringField(value, "content");
    const partsValue = fieldValue(value, "parts");
    const interruptedAssistantMetrics = parseOptionalChatReplyMetrics(
      fieldValue(
        value,
        "interruptedAssistantMetrics",
        "interrupted_assistant_metrics",
      ),
    );

    if (
      !id ||
      content === null ||
      !Array.isArray(partsValue) ||
      interruptedAssistantMetrics === false
    ) {
      return null;
    }

    const parts = partsValue.map(normalizeChatMessagePart);
    if (parts.some((part) => part === null)) {
      return null;
    }

    return {
      type: "guidanceApplied",
      id,
      content,
      parts: parts as ChatMessagePart[],
      interruptedAssistantMetrics,
    };
  }

  if (value.type === "gitDiffRefresh" || value.type === "git_diff_refresh") {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");

    if (!workspaceId) {
      return null;
    }

    return { type: "gitDiffRefresh", workspaceId };
  }

  if (
    value.type === "todoGraphRefresh" ||
    value.type === "todo_graph_refresh"
  ) {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");
    const chatId = stringField(value, "chatId", "chat_id");

    if (!workspaceId || !chatId) {
      return null;
    }

    return { type: "todoGraphRefresh", workspaceId, chatId };
  }

  if (value.type === "error") {
    const message = stringField(value, "message");

    if (!message) {
      return null;
    }

    return { type: "error", message };
  }

  return null;
}

function parseHookNotificationSummary(
  value: unknown,
): HookNotificationSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const event = stringField(value, "event");
  const level = stringField(value, "level");
  const message = stringField(value, "message");

  if (!event || !level || !message) {
    return null;
  }

  return { event, level, message };
}

function parseQuestionRequestSummary(value: unknown): QuestionRequestSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const toolCallId = stringField(value, "toolCallId", "tool_call_id");
  const workspaceId = stringField(value, "workspaceId", "workspace_id");
  const chatId = stringField(value, "chatId", "chat_id");
  const questions = fieldValue(value, "questions");

  if (
    !id ||
    !toolCallId ||
    !workspaceId ||
    !chatId ||
    !Array.isArray(questions) ||
    questions.length === 0
  ) {
    return null;
  }

  const parsedQuestions = questions.map(parseQuestionItemSummary);
  if (parsedQuestions.some((question) => question === null)) {
    return null;
  }

  return {
    chatId,
    id,
    questions: parsedQuestions as QuestionItemSummary[],
    toolCallId,
    workspaceId,
  };
}

function parseQuestionItemSummary(value: unknown): QuestionItemSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const question = stringField(value, "question");
  const options = fieldValue(value, "options");
  const allowFreeText = fieldValue(value, "allowFreeText", "allow_free_text");

  if (
    !id ||
    !question ||
    !Array.isArray(options) ||
    typeof allowFreeText !== "boolean"
  ) {
    return null;
  }

  const parsedOptions = options.map(parseQuestionOptionSummary);
  if (parsedOptions.some((option) => option === null)) {
    return null;
  }

  return {
    allowFreeText,
    id,
    options: parsedOptions as QuestionOptionSummary[],
    question,
  };
}

function parseQuestionOptionSummary(value: unknown): QuestionOptionSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const label = stringField(value, "label");
  const optionValue = stringField(value, "value");
  const description = optionalNullableStringField(value, "description");

  if (!label || !optionValue || description === false) {
    return null;
  }

  return {
    description: description ?? null,
    label,
    value: optionValue,
  };
}

function describeChatStreamEvent(value: unknown) {
  const summary = isObjectRecord(value) ? { type: value.type, value } : value;

  try {
    return JSON.stringify(summary).slice(0, 600);
  } catch {
    return String(value);
  }
}

function parseChatToolCallSummary(value: unknown): ChatToolCallSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const name = stringField(value, "name");
  const status = stringField(value, "status");
  const input = fieldValue(value, "input");
  const output = fieldValue(value, "output");
  const isError = fieldValue(value, "isError", "is_error");

  if (
    !id ||
    !name ||
    !status ||
    !isJsonValue(input) ||
    !isJsonValue(output) ||
    typeof isError !== "boolean"
  ) {
    return null;
  }

  return normalizedToolCallSummary({ id, name, status, input, output, isError });
}

function parseChatMemoriesUsed(
  value: unknown,
): ChatMemoryUsedSummary[] | false {
  if (typeof value === "undefined" || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    return false;
  }

  const memories = value.map(parseChatMemoryUsedSummary);
  return memories.some((memory) => memory === null)
    ? false
    : (memories as ChatMemoryUsedSummary[]);
}

function parseChatMemoryUsedSummary(
  value: unknown,
): ChatMemoryUsedSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const scope = stringField(value, "scope");
  const chatId = optionalNullableStringField(value, "chatId", "chat_id");
  const kind = stringField(value, "kind");
  const fact = stringField(value, "fact");
  const pinned = fieldValue(value, "pinned");
  const source = stringField(value, "source");

  if (
    !id ||
    !scope ||
    chatId === false ||
    !kind ||
    !fact ||
    typeof pinned !== "boolean" ||
    !source
  ) {
    return null;
  }

  return {
    chatId: chatId ?? null,
    fact,
    id,
    kind,
    pinned,
    scope,
    source,
  };
}

function parseChatExtractedMemories(
  value: unknown,
): ChatExtractedMemorySummary[] | false {
  if (typeof value === "undefined" || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    return false;
  }

  const memories = value.map(parseChatExtractedMemorySummary);
  return memories.some((memory) => memory === null)
    ? false
    : (memories as ChatExtractedMemorySummary[]);
}

function parseChatExtractedMemorySummary(
  value: unknown,
): ChatExtractedMemorySummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const scope = stringField(value, "scope");
  const chatId = optionalNullableStringField(value, "chatId", "chat_id");
  const status = stringField(value, "status");
  const kind = stringField(value, "kind");
  const fact = stringField(value, "fact");

  if (!id || !scope || chatId === false || !status || !kind || !fact) {
    return null;
  }

  return {
    chatId: chatId ?? null,
    fact,
    id,
    kind,
    scope,
    status,
  };
}

function streamingAssistantMessage(
  id: string,
  memoriesUsed: ChatMemoryUsedSummary[] = [],
): ShellMessage {
  return {
    id,
    role: "assistant",
    content: "",
    createdAt: new Date().toISOString(),
    reasoning: null,
    status: "streaming",
    toolCalls: [],
    parts: [],
    metrics: null,
    memoriesUsed,
    extractedMemories: [],
  };
}

function normalizeActiveChatRunSummary(
  value: unknown,
): ActiveChatRunSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const runId = stringField(value, "runId", "run_id");
  const workspaceId = stringField(value, "workspaceId", "workspace_id");
  const chatId = stringField(value, "chatId", "chat_id");
  const lastSequenceValue = fieldValue(value, "lastSequence", "last_sequence");

  if (!runId || !workspaceId || !chatId) {
    return null;
  }

  return {
    runId,
    workspaceId,
    chatId,
    lastSequence:
      typeof lastSequenceValue === "number" ? lastSequenceValue : null,
  };
}

function normalizeChatMessageSummary(
  message: ChatMessageSummary,
): ChatMessageSummary {
  const metrics = parseOptionalChatReplyMetrics(message.metrics);
  if (metrics === false) {
    throw new Error("chat message metrics are invalid");
  }
  const memoriesUsed = parseChatMemoriesUsed(message.memoriesUsed);
  if (memoriesUsed === false) {
    throw new Error("chat message memoriesUsed are invalid");
  }
  const extractedMemories = parseChatExtractedMemories(
    fieldValue(message, "extractedMemories", "extracted_memories"),
  );
  if (extractedMemories === false) {
    throw new Error("chat message extractedMemories are invalid");
  }

  const toolCalls = Array.isArray(message.toolCalls)
    ? message.toolCalls.map(normalizedToolCallSummary)
    : [];
  const partsSource = Array.isArray(message.parts) ? message.parts : [];
  const parts = partsSource
    .map((part) => normalizeChatMessagePart(part))
    .filter((part): part is ChatMessagePart => part !== null);
  const normalizedMessage = {
    ...message,
    extractedMemories,
    metrics,
    memoriesUsed,
    toolCalls,
    parts,
  };

  return {
    ...normalizedMessage,
    parts: parts.length ? parts : fallbackMessageParts(normalizedMessage),
  };
}

function normalizeChatMessagePart(part: unknown): ChatMessagePart | null {
  if (!isObjectRecord(part)) {
    return null;
  }

  if (part.type === "text") {
    const text = fieldValue(part, "text");
    return typeof text === "string" ? { type: "text", text } : null;
  }

  if (part.type === "error") {
    const text = fieldValue(part, "text");
    return typeof text === "string" ? { type: "error", text } : null;
  }

  if (part.type === "reasoning") {
    const text = fieldValue(part, "text");
    return typeof text === "string" ? { type: "reasoning", text } : null;
  }

  if (part.type === "attachment") {
    const attachment = parseChatAttachmentPartSummary(
      fieldValue(part, "attachment"),
    );
    return attachment ? { type: "attachment", attachment } : null;
  }

  if (part.type === "toolCall" || part.type === "tool_call") {
    const toolCall = parseChatToolCallSummary(
      fieldValue(part, "toolCall", "tool_call"),
    );
    return toolCall ? { type: "toolCall", toolCall } : null;
  }

  return null;
}

function parseChatAttachmentPartSummary(
  value: unknown,
): ChatAttachmentPartSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const name = stringField(value, "name");
  const contentType = stringField(value, "contentType", "content_type");
  const previewDataUrl = optionalNullableStringField(
    value,
    "previewDataUrl",
    "preview_data_url",
  );
  const path = optionalNullableStringField(value, "path");
  const sizeBytes = fieldValue(value, "sizeBytes", "size_bytes");

  if (
    !id ||
    !name ||
    !contentType ||
    previewDataUrl === false ||
    path === false ||
    typeof sizeBytes !== "number"
  ) {
    return null;
  }

  return {
    contentType,
    id,
    name,
    path: path ?? null,
    previewDataUrl: previewDataUrl ?? null,
    sizeBytes,
  };
}

function parseNullableChatUsage(value: unknown): ChatUsage | null | undefined | false {
  if (value === null) {
    return null;
  }

  return parseChatUsage(value);
}

function parseRequiredChatReplyMetrics(value: unknown): ChatReplyMetrics | false {
  const metrics = parseChatReplyMetrics(value);

  if (metrics === undefined || metrics === null) {
    return false;
  }

  return metrics;
}

function parseOptionalChatReplyMetrics(
  value: unknown,
): ChatReplyMetrics | null | false {
  if (typeof value === "undefined" || value === null) {
    return null;
  }

  const metrics = parseChatReplyMetrics(value);

  return metrics === undefined ? false : metrics;
}

function parseChatReplyMetrics(
  value: unknown,
): ChatReplyMetrics | undefined | false {
  if (typeof value === "undefined") {
    return undefined;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  const modelId = stringField(value, "modelId", "model_id");
  const providerId = stringField(value, "providerId", "provider_id");
  const totalLatencyMs = fieldValue(
    value,
    "totalLatencyMs",
    "total_latency_ms",
  );
  const firstTokenLatencyMs = fieldValue(
    value,
    "firstTokenLatencyMs",
    "first_token_latency_ms",
  );
  const outputTokens = fieldValue(value, "outputTokens", "output_tokens");

  if (
    !modelId ||
    !providerId ||
    !isNullableNumber(totalLatencyMs) ||
    !isNullableNumber(firstTokenLatencyMs) ||
    !isNullableNumber(outputTokens)
  ) {
    return false;
  }

  return {
    firstTokenLatencyMs,
    modelId,
    outputTokens,
    providerId,
    totalLatencyMs,
  };
}

function parseChatUsage(value: unknown): ChatUsage | undefined | false {
  if (typeof value === "undefined") {
    return undefined;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  const inputTokens = fieldValue(value, "inputTokens", "input_tokens");
  const outputTokens = fieldValue(value, "outputTokens", "output_tokens");
  const cacheReadTokens = fieldValue(value, "cacheReadTokens", "cache_read_tokens");
  const cacheWriteTokens = fieldValue(
    value,
    "cacheWriteTokens",
    "cache_write_tokens",
  );

  if (
    !isNullableNumber(inputTokens) ||
    !isNullableNumber(outputTokens) ||
    !isNullableNumber(cacheReadTokens) ||
    !isNullableNumber(cacheWriteTokens)
  ) {
    return false;
  }

  return { inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens };
}

function isNullableNumber(value: unknown) {
  return typeof value === "number" || value === null;
}

function stringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "string" ? field : null;
}

function optionalStringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "undefined" || typeof field === "string" ? field : null;
}

function optionalNullableStringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);

  if (
    typeof field === "undefined" ||
    field === null ||
    typeof field === "string"
  ) {
    return field;
  }

  return false;
}

function fieldValue(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  if (typeof value[camelName] !== "undefined") {
    return value[camelName];
  }

  if (snakeName && typeof value[snakeName] !== "undefined") {
    return value[snakeName];
  }

  return undefined;
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string"
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  if (isObjectRecord(value)) {
    return Object.values(value).every(isJsonValue);
  }

  return false;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isTerminalServerEvent(value: unknown): value is TerminalServerEvent {
  if (
    typeof value !== "object" ||
    value === null ||
    !("type" in value) ||
    typeof value.type !== "string"
  ) {
    return false;
  }

  if (value.type === "started" || value.type === "cwd") {
    return "cwd" in value && typeof value.cwd === "string";
  }

  if (value.type === "output") {
    return "data" in value && typeof value.data === "string";
  }

  if (value.type === "exit") {
    return "status" in value && typeof value.status === "string";
  }

  if (value.type === "error") {
    return "message" in value && typeof value.message === "string";
  }

  return false;
}

async function responseErrorMessage(response: Response) {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    const data = (await response.json()) as unknown;

    if (isErrorResponse(data)) {
      return data.error;
    }
  }

  const text = await response.text();
  return text || `request returned ${response.status}`;
}

async function requestJson<T>(
  url: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(url, {
    cache: "no-store",
    credentials: "same-origin",
    ...init,
  });
  const contentType = response.headers.get("content-type") ?? "";
  const data = contentType.includes("application/json")
    ? ((await response.json()) as unknown)
    : null;

  if (!response.ok) {
    if (isErrorResponse(data)) {
      throw new Error(data.error);
    }

    throw new Error(`${url} returned ${response.status}`);
  }

  return data as T;
}

function isErrorResponse(value: unknown): value is { error: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "error" in value &&
    typeof value.error === "string"
  );
}

function errorMessage(value: unknown) {
  return value instanceof Error ? value.message : "Unknown error";
}
