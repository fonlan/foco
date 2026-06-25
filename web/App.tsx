import focoLogoSvg from "../foco.svg?raw";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  BarChart3,
  Bot,
  Brain,
  CalendarClock,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Code2,
  Copy,
  ClipboardPaste,
  Database,
  Download,
  Eye,
  EyeOff,
  Files,
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
  Minus,
  PanelBottom,
  PanelRight,
  Pencil,
  Play,
  PlugZap,
  Plus,
  RefreshCw,
  Redo2,
  Save,
  Scissors,
  Search,
  Send,
  Server,
  Settings,
  SlidersHorizontal,
  Sparkles,
  SquareTerminal,
  ScrollText,
  SunMoon,
  WrapText,
  Terminal,
  Trash2,
  Undo2,
  Upload,
  User,
  Webhook,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  CSSProperties,
  ChangeEvent as ReactChangeEvent,
  DragEvent as ReactDragEvent,
  FormEvent,
  ClipboardEvent as ReactClipboardEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  WheelEvent as ReactWheelEvent,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type * as Monaco from "monaco-editor";
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
import type {
  ActiveChatRunSummary,
  ActiveRunInfo,
  AiRequestAuditDetail,
  AiRequestAuditSummary,
  AiRequestDetailResponse,
  AiStatisticsModelBreakdown,
  AiStatisticsProviderBreakdown,
  AiStatisticsResponse,
  AiStatisticsSummary,
  AiStatsFilterState,
  AgentDefinitionInput,
  AgentDefinitionSettings,
  AgentDefinitionsResponse,
  AgentInstanceView,
  AgentTeamSnapshotResponse,
  AppLanguageId,
  AppThemeId,
  AuthStatusResponse,
  BrowserRoute,
  BrowserRouteChatTab,
  BrowserRouteFileTab,
  ChatAttachmentPartSummary,
  ChatAttachmentPayload,
  ChatCompressionStatistics,
  ChatExtractedMemorySummary,
  ChatMemoryUsedSummary,
  ChatMessagePart,
  ChatMessageSummary,
  ChatMessagesResponse,
  ChatReplyMetrics,
  ChatRunBadge,
  ChatSpecUpdateSummary,
  ChatStatisticsResponse,
  ChatStreamEvent,
  ChatSummary,
  ChatToolBreakdown,
  ChatToolCallSummary,
  ChatToolLiveOutput,
  ChatTabSummary,
  ChatUsage,
  ClearMemoriesResponse,
  ComposerAttachment,
  ConfiguredMcpServerSummary,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  ConfiguredWorkspaceSummary,
  ContextMemoryScopeState,
  ContextMemoryState,
  ContextUsageRefreshRequest,
  ContextUsageResponse,
  EffectiveHookSummary,
  GeneralFormState,
  GeneralSettingsSummary,
  GenerateWorkspaceSpecResponse,
  GitBranchesResponse,
  GitCommitMessageResponse,
  GitDiffLineStats,
  GitDiffResponse,
  GitStatusFileSummary,
  HookConfig,
  HookConfigScopeSummary,
  HookDecision,
  HookHandler,
  HookHandlerFormState,
  HookHandlerType,
  HookMatcherGroup,
  HookNotificationSummary,
  HookRunDetail,
  HookRunDetailResponse,
  HookRunSummary,
  HookRunSummaryRow,
  HookRunsResponse,
  HookScope,
  HooksSettingsResponse,
  ImportClaudeHooksResponse,
  InstallRipgrepResponse,
  JsonValue,
  LiveChatStatistics,
  ManualMemoryFormState,
  McpServerFormState,
  MemoryDialogMode,
  MemoryDreamChangeSummary,
  MemoryDreamChangesResponse,
  MemoryDreamJobSummary,
  MemoryDreamJobsResponse,
  MemoryDreamRunMode,
  MemoryDreamRunResponse,
  MemoryDreamScope,
  MemoryExtractionJobSummary,
  MemoryFactRecord,
  MemoryFilterState,
  MemoryListMeta,
  MemoryListResponse,
  MemoryMutationResponse,
  MemorySettingsFormState,
  MemorySourceFormState,
  MemorySourceRecord,
  MemorySourcesResponse,
  ModelFormState,
  ModelMetadataRecord,
  ModelMetadataResponse,
  NativeSelectedFile,
  OpenChatTab,
  PendingDeleteChat,
  PromptSettingsFormState,
  PromptSettingsSummary,
  ProviderFormState,
  ProviderModelsResponse,
  ProviderModelsRefreshResponse,
  ProviderRequestOverrideFormState,
  ProviderRequestOverrideTarget,
  ProviderRequestOverrideValueType,
  ProviderTestResponse,
  ProviderTestState,
  QueueChatMessageResponse,
  QueuedMessageRunSummary,
  QueuedRunSummary,
  QuestionAnswerSubmission,
  QuestionItemSummary,
  QuestionOptionSummary,
  QuestionRequestSummary,
  RetryRunRequest,
  ScheduledWorkspaceRun,
  SettingsResponse,
  SettingsSection,
  ShellMessage,
  SpecSettingsFormState,
  SystemPromptSummary,
  TaskStatus,
  TerminalCommandRun,
  TerminalPaneStatus,
  TerminalPanelSession,
  TerminalServerEvent,
  TerminalSessionResponse,
  TerminalShellSummary,
  ThinkingLevelSummary,
  TodoGraphResponse,
  TodoGraphTask,
  Translate,
  WebSearchFormState,
  WorkspaceChatListItem,
  WorkspaceCommonCommandSummary,
  WorkspaceFileChildrenResponse,
  WorkspaceFileContentResponse,
  WorkspaceFileSaveResponse,
  WorkspaceFilesResponse,
  WorkspaceFileTreeNode,
  WorkspaceFormState,
  WorkspaceIconDraft,
  WorkspaceSpecJobsResponse,
  WorkspaceSpecResponse,
  WorkspaceSummary,
  WorkspacesResponse,
} from "./api/types";
import {
  diffLineClass,
  hasGitDiffStats,
  parseGitDiffLineStats,
  parseGitDiffSections,
  type GitDiffFile,
  type GitDiffLine,
  type GitDiffSection,
} from "./features/git/diff-parser";
import {
  ANALYTICS_CHART_COLORS,
  CHAT_BOTTOM_LOCK_THRESHOLD_PX,
  chartTooltipLabelStyle,
  chartTooltipStyle,
  CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT,
  CONTEXT_PANEL_DEFAULT_WIDTH,
  CONTEXT_PANEL_MAX_HEIGHT_RATIO,
  CONTEXT_PANEL_MAX_WIDTH,
  CONTEXT_PANEL_MIN_HEIGHT,
  CONTEXT_PANEL_MIN_WIDTH,
  CREATE_BRANCH_OPTION_VALUE,
  DEFAULT_SYSTEM_PROMPT_NAME,
  IMAGE_AGENT_SYSTEM_PROMPT_NAME,
  MAX_CHAT_ATTACHMENTS,
  MAX_CHAT_ATTACHMENT_BYTES,
  MAX_CHAT_ATTACHMENT_TOTAL_BYTES,
  MEMORY_KIND_OPTIONS,
  MOBILE_BREAKPOINT_PX,
  SAVED_PASSWORD_MASK,
  WORKSPACE_CHAT_CONTEXT_MENU_LONG_PRESS_MS,
  WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
  WORKSPACE_SIDEBAR_MAX_WIDTH,
  WORKSPACE_SIDEBAR_MIN_WIDTH,
  type AiStatsColumnId,
} from "./app/constants";
import {
  useBrowserPopState,
  useDocumentLanguage,
  useDocumentTheme,
  useInitialBrowserRouteEffect,
  useRightPanelResizeEffect,
  useSidebarResizeEffect,
} from "./app/app-effects";
import { useAppRouting } from "./app/app-routing";
import {
  browserPathForRoute,
  currentBrowserRoute,
} from "./shared/browser-route";
import { I18nContext, translate, useI18n } from "./shared/i18n";
import { TerminalPanel } from "./features/terminal/TerminalPanel";
import { WorkspaceIcon } from "./features/workspaces/WorkspaceIcon";
import { chatItemClass, workspaceItemClass, workspaceMenuClass, workspaceNameFromPath } from "./features/workspaces/workspace-helpers";
import { WorkspaceDialog } from "./features/workspaces/WorkspaceDialog";
import { GitBranchDialog } from "./features/git/GitBranchDialog";
import { DeleteChatDialog } from "./features/chat/DeleteChatDialog";
import { ChatPanel, type ChatPanelHelpers } from "./features/chat/ChatPanel";
import { MarkdownContent } from "./features/chat/MarkdownContent";
import { AgentsRuntimePanel } from "./features/agents/AgentsRuntimePanel";
import { AgentTranscriptPanel } from "./features/agents/AgentTranscriptPanel";
import { AgentsSettingsPanel } from "./features/agents/AgentsSettingsPanel";
import { errorMessage, requestJson, responseErrorMessage } from "./shared/api-client";
import {
  readAiStatsVisibleColumnIds,
  writeAiStatsVisibleColumnIds,
} from "./features/stats/ai-stats-preferences";
import {
  emptyAiStatsFilters,
  useAiStatisticsData,
} from "./features/stats/use-ai-statistics-data";
import { ScheduledTasksPage } from "./features/scheduled-tasks/ScheduledTasksPage";

type ViewMode = BrowserRoute["viewMode"];
type ContextPanelTab = "todo" | "files" | "git" | "memory" | "stats" | "agents" | "spec";
type ProviderModelListState = {
  message: string | null;
  models: string[];
  status: "error" | "loading" | "ok";
};
type WorkspaceChatContextMenuState = {
  chat: WorkspaceChatListItem;
  left: number;
  top: number;
  workspace: WorkspaceSummary;
};

const OPENAI_RESPONSES_PROVIDER_KIND = "openai-responses";

function saveWorkspaceSpecSettingsRequest(
  workspaceId: string,
  enabled: boolean,
  injectEnabled: boolean,
) {
  return requestJson<WorkspaceSpecResponse>(
    `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/settings`,
    {
      body: JSON.stringify({ enabled, injectEnabled }),
      headers: { "Content-Type": "application/json" },
      method: "PUT",
    },
  );
}

type ProviderServicePreset = {
  id: string;
  label: string;
  kindIds: string[];
  defaultKindId: string;
};

const PROVIDER_SERVICE_PRESETS: ProviderServicePreset[] = [
  {
    id: "openai",
    label: "OpenAI",
    kindIds: [OPENAI_RESPONSES_PROVIDER_KIND, "openai-chat"],
    defaultKindId: OPENAI_RESPONSES_PROVIDER_KIND,
  },
  {
    id: "anthropic",
    label: "Anthropic",
    kindIds: ["anthropic"],
    defaultKindId: "anthropic",
  },
  { id: "gemini", label: "Gemini", kindIds: ["gemini"], defaultKindId: "gemini" },
  { id: "xai", label: "xAI", kindIds: ["xai"], defaultKindId: "xai" },
  {
    id: "deepseek",
    label: "DeepSeek",
    kindIds: ["deepseek"],
    defaultKindId: "deepseek",
  },
  { id: "groq", label: "Groq", kindIds: ["groq"], defaultKindId: "groq" },
  {
    id: "open-router",
    label: "OpenRouter",
    kindIds: ["open-router"],
    defaultKindId: "open-router",
  },
  {
    id: "fireworks",
    label: "Fireworks",
    kindIds: ["fireworks"],
    defaultKindId: "fireworks",
  },
  {
    id: "together",
    label: "Together",
    kindIds: ["together"],
    defaultKindId: "together",
  },
  {
    id: "moonshot",
    label: "Moonshot",
    kindIds: ["moonshot"],
    defaultKindId: "moonshot",
  },
  { id: "zai", label: "ZAI", kindIds: ["zai"], defaultKindId: "zai" },
  {
    id: "bigmodel",
    label: "BigModel",
    kindIds: ["bigmodel"],
    defaultKindId: "bigmodel",
  },
  { id: "aliyun", label: "Aliyun", kindIds: ["aliyun"], defaultKindId: "aliyun" },
  { id: "baidu", label: "Baidu", kindIds: ["baidu"], defaultKindId: "baidu" },
  { id: "cohere", label: "Cohere", kindIds: ["cohere"], defaultKindId: "cohere" },
  { id: "ollama", label: "Ollama", kindIds: ["ollama"], defaultKindId: "ollama" },
  {
    id: "ollama-cloud",
    label: "Ollama Cloud",
    kindIds: ["ollama-cloud"],
    defaultKindId: "ollama-cloud",
  },
  { id: "vertex", label: "Vertex AI", kindIds: ["vertex"], defaultKindId: "vertex" },
  {
    id: "github-copilot",
    label: "GitHub Copilot",
    kindIds: ["github-copilot"],
    defaultKindId: "github-copilot",
  },
  {
    id: "opencode-go",
    label: "OpenCode Go",
    kindIds: ["opencode-go"],
    defaultKindId: "opencode-go",
  },
  {
    id: "bedrock-api",
    label: "Bedrock API",
    kindIds: ["bedrock-api"],
    defaultKindId: "bedrock-api",
  },
  {
    id: "aihubmix",
    label: "AIHubMix",
    kindIds: ["aihubmix"],
    defaultKindId: "aihubmix",
  },
  { id: "mimo", label: "Mimo", kindIds: ["mimo"], defaultKindId: "mimo" },
  { id: "nebius", label: "Nebius", kindIds: ["nebius"], defaultKindId: "nebius" },
  { id: "minimax", label: "MiniMax", kindIds: ["minimax"], defaultKindId: "minimax" },
];

type OpenFileTab = {
  workspaceId: string;
  path: string;
  name: string;
  workspaceName: string;
  workspaceLogoUrl: string | null;
};

type OpenAgentTab = {
  workspaceId: string;
  chatId: string;
  teamId: string;
  instanceId: string;
  fallbackTitle: string;
  fallbackWorkspaceName: string;
};

type ActiveMainTab =
  | { type: "chat"; workspaceId: string; chatId: string | null }
  | { type: "file"; workspaceId: string; path: string }
  | {
      type: "agent";
      workspaceId: string;
      chatId: string;
      teamId: string;
      instanceId: string;
    };

type MainTabSummary =
  | (ChatTabSummary & { type: "chat" })
  | (OpenFileTab & { type: "file"; title: string })
  | (OpenAgentTab & {
      type: "agent";
      title: string;
      workspaceName: string;
      workspaceLogoUrl: string | null;
    });

type WorkspaceFileEditorState = {
  content: string;
  error: string | null;
  isDirty: boolean;
  isLoading: boolean;
  isSaving: boolean;
  lastSavedContent: string;
};

type WorkspaceFileContextMenuState = {
  left: number;
  node: WorkspaceFileTreeNode;
  top: number;
  workspacePath: string;
};

type ChartDatum = {
  displayValue?: string;
  id: string;
  label: string;
  value: number;
};

type AiStatsColumn = {
  cellClassName: string;
  headerClassName?: string;
  id: AiStatsColumnId;
  label: string;
  render: (request: AiRequestAuditSummary) => ReactNode;
};

const LIVE_REASONING_DURATION_REFRESH_MS = 250;
const AGENT_TEAM_RUNNING_REFRESH_MS = 1000;
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-default";
const MEMORY_DREAM_DEFAULT_PAGE_SIZE = 10;
const MEMORY_DREAM_MAX_PAGE_SIZE = 200;

type ComposerDefaultSelection = {
  modelId: string;
  providerId: string;
  thinkingLevel: string;
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
  const [statsRoutePage, setStatsRoutePage] = useState(
    initialBrowserRoute.viewMode === "stats" ? initialBrowserRoute.page : 1,
  );
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceDialogRevision, setWorkspaceDialogRevision] = useState(0);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [workspaceSpecEnabled, setWorkspaceSpecEnabled] = useState(false);
  const [workspaceIconDraft, setWorkspaceIconDraft] =
    useState<WorkspaceIconDraft | null>(null);
  const workspaceIconInputRef = useRef<HTMLInputElement | null>(null);
  const [draftMessage, setDraftMessage] = useState("");
  const [draftAttachments, setDraftAttachments] = useState<ComposerAttachment[]>(
    [],
  );
  const [messages, setMessages] = useState<ShellMessage[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [isPreparingChatRun, setIsPreparingChatRun] = useState(false);
  const [activeMainTab, setActiveMainTab] = useState<ActiveMainTab>({
    chatId: null,
    type: "chat",
    workspaceId: "",
  });
  const [openChatTabs, setOpenChatTabs] = useState<OpenChatTab[]>([]);
  const openChatTabsRef = useRef<OpenChatTab[]>([]);
  const [openAgentTabs, setOpenAgentTabs] = useState<OpenAgentTab[]>([]);
  const [loadingChatKeys, setLoadingChatKeys] = useState<Set<string>>(() => new Set());
  const [openFileTabs, setOpenFileTabs] = useState<OpenFileTab[]>([]);
  const openFileTabsRef = useRef<OpenFileTab[]>([]);
  const [workspaceFileEditors, setWorkspaceFileEditors] = useState<
    Record<string, WorkspaceFileEditorState>
  >({});
  const [pendingDeleteChat, setPendingDeleteChat] =
    useState<PendingDeleteChat | null>(null);
  const [workspaceChatContextMenu, setWorkspaceChatContextMenu] =
    useState<WorkspaceChatContextMenuState | null>(null);
  const [workspaceFileContextMenu, setWorkspaceFileContextMenu] =
    useState<WorkspaceFileContextMenuState | null>(null);
  const [chatMessagesByKey, setChatMessagesByKey] = useState<
    Record<string, ShellMessage[]>
  >({});
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [agentDefinitions, setAgentDefinitions] = useState<AgentDefinitionSettings[]>([]);
  const [isTeamModeEnabled, setIsTeamModeEnabled] = useState(false);
  const [isLoadingAgentDefinitions, setIsLoadingAgentDefinitions] = useState(false);
  const [agentDefinitionsError, setAgentDefinitionsError] = useState<string | null>(null);
  const [agentDefinitionOperationKey, setAgentDefinitionOperationKey] = useState<string | null>(null);
  const [agentTeamSnapshot, setAgentTeamSnapshot] = useState<AgentTeamSnapshotResponse | null>(null);
  const [isLoadingAgentTeam, setIsLoadingAgentTeam] = useState(false);
  const [agentTeamError, setAgentTeamError] = useState<string | null>(null);
  const [nativeBrowserToken, setNativeBrowserToken] = useState<string | null>(null);
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
  const [diffPanelWidth, setDiffPanelWidth] = useState(CONTEXT_PANEL_DEFAULT_WIDTH);
  const [contextPanelMobileHeight, setContextPanelMobileHeight] = useState(
    CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT,
  );
  const [isResizingDiffPanel, setIsResizingDiffPanel] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(WORKSPACE_SIDEBAR_MIN_WIDTH);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [isMobileWorkspaceOpen, setIsMobileWorkspaceOpen] = useState(false);
  const [isWorkspaceSidebarOpen, setIsWorkspaceSidebarOpen] = useState(true);
  const [terminalOpenWorkspaceIds, setTerminalOpenWorkspaceIds] = useState<
    Set<string>
  >(() => new Set());
  const [gitDiff, setGitDiff] = useState<GitDiffResponse | null>(null);
  const [selectedDiffPath, setSelectedDiffPath] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [gitCommitMessage, setGitCommitMessage] = useState("");
  const [gitOperationKey, setGitOperationKey] = useState<string | null>(null);
  const [workspaceFiles, setWorkspaceFiles] = useState<WorkspaceFilesResponse | null>(null);
  const [expandedFileTreePaths, setExpandedFileTreePaths] = useState<Set<string>>(
    () => new Set([""]),
  );
  const [loadingWorkspaceDirectoryPaths, setLoadingWorkspaceDirectoryPaths] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoadingWorkspaceFiles, setIsLoadingWorkspaceFiles] = useState(false);
  const [workspaceFilesError, setWorkspaceFilesError] = useState<string | null>(null);
  const [workspaceFileOperationKey, setWorkspaceFileOperationKey] = useState<string | null>(null);
  const [todoGraph, setTodoGraph] = useState<TodoGraphResponse | null>(null);
  const [isLoadingTodoGraph, setIsLoadingTodoGraph] = useState(false);
  const [todoGraphError, setTodoGraphError] = useState<string | null>(null);
  const [chatStatistics, setChatStatistics] =
    useState<ChatStatisticsResponse | null>(null);
  const [isLoadingChatStatistics, setIsLoadingChatStatistics] = useState(false);
  const [chatStatisticsError, setChatStatisticsError] = useState<string | null>(
    null,
  );
  const [liveChatStatisticsByKey, setLiveChatStatisticsByKey] = useState<
    Record<string, LiveChatStatistics>
  >({});
  const [contextMemories, setContextMemories] = useState<ContextMemoryState>({
    global: { memories: [], page: 1, pageSize: 10, totalCount: 0, totalPages: 0 },
    workspace: { memories: [], page: 1, pageSize: 10, totalCount: 0, totalPages: 0 },
  });
  const [contextMemoryPages, setContextMemoryPages] = useState<{
    global: { page: number; pageSize: number };
    workspace: { page: number; pageSize: number };
  }>({
    global: { page: 1, pageSize: 10 },
    workspace: { page: 1, pageSize: 10 },
  });
  const [isLoadingContextMemories, setIsLoadingContextMemories] =
    useState(false);
  const [contextMemoryError, setContextMemoryError] = useState<string | null>(
    null,
  );
  const [deletingContextMemoryId, setDeletingContextMemoryId] = useState<
    string | null
  >(null);
  const [workspaceSpec, setWorkspaceSpec] = useState<WorkspaceSpecResponse | null>(null);
  const [workspaceSpecDraft, setWorkspaceSpecDraft] = useState("");
  const [isLoadingWorkspaceSpec, setIsLoadingWorkspaceSpec] = useState(false);
  const [workspaceSpecError, setWorkspaceSpecError] = useState<string | null>(null);
  const [workspaceSpecConflictMessage, setWorkspaceSpecConflictMessage] = useState<string | null>(null);
  const [workspaceSpecPreviewEnabled, setWorkspaceSpecPreviewEnabled] = useState(false);
  const [workspaceSpecOperationKey, setWorkspaceSpecOperationKey] = useState<
    "generate" | "save" | "settings" | null
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
  const [scheduledWorkspaceRuns, setScheduledWorkspaceRuns] = useState<
    ScheduledWorkspaceRun[]
  >([]);
  const [activeRunInfoByChatKey, setActiveRunInfoByChatKey] = useState<
    Record<string, ActiveRunInfo>
  >({});
  const [readOnlyChatKeys, setReadOnlyChatKeys] = useState<Record<string, boolean>>({});
  const [contextUsageByChatKey, setContextUsageByChatKey] = useState<
    Record<string, ContextUsageResponse>
  >({});
  const [contextUsageLoadingByChatKey, setContextUsageLoadingByChatKey] =
    useState<Record<string, boolean>>({});
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
  const contextUsageAbortByChatKeyRef = useRef<Map<string, AbortController>>(
    new Map(),
  );
  const contextUsageIdentityByChatKeyRef = useRef<Map<string, string>>(
    new Map(),
  );
  const contextUsageRequestIdByChatKeyRef = useRef<Map<string, number>>(
    new Map(),
  );
  const todoGraphRequestIdRef = useRef(0);
  const selectedModelIdRef = useRef("");
  const selectedProviderIdRef = useRef("");
  const selectedThinkingLevelRef = useRef("");
  const activeChatKeyRef = useRef<string | null>(null);
  const activeWorkspaceIdRef = useRef("");
  const activeChatIdRef = useRef<string | null>(null);
  const loadingChatKeysRef = useRef<Set<string>>(new Set());
  const loadingChatControllersRef = useRef<Map<string, AbortController>>(new Map());
  const runningChatKeysRef = useRef<Set<string>>(new Set());
  const activeRunInfoByChatKeyRef = useRef<Record<string, ActiveRunInfo>>({});
  const queuedRunRequestsByChatKeyRef = useRef<
    Record<string, RetryRunRequest[]>
  >({});
  const scheduledWorkspaceRunsRef = useRef<ScheduledWorkspaceRun[]>([]);
  const pendingGuidanceMessageIdsRef = useRef<Map<string, string>>(new Map());
  const applyBrowserRouteRef = useRef<(route: BrowserRoute) => void>(() => { });
  const hasAppliedInitialBrowserRouteRef = useRef(false);
  const hasManuallySelectedModelRef = useRef(false);
  const hasManuallySelectedThinkingLevelRef = useRef(false);
  const workspaceSidebarRef = useRef<HTMLElement | null>(null);
  const workspaceChatLongPressTimeoutRef = useRef<number | null>(null);
  const suppressNextWorkspaceChatClickRef = useRef(false);

  const activeWorkspace = useMemo(
    () =>
      workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
      workspaces[0],
    [activeWorkspaceId, workspaces],
  );
  const activeChatKey =
    activeChatId === null || isPendingChatId(activeChatId)
      ? activeChatKeyRef.current
      : chatRunKey(activeWorkspaceId, activeChatId);
  const isLoadingActiveChatMessages =
    activeChatKey !== null && loadingChatKeys.has(activeChatKey);
  const activeContextUsageKey =
    activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)
      ? chatRunKey(activeWorkspaceId, activeChatId)
      : null;
  const contextUsage = activeContextUsageKey
    ? contextUsageByChatKey[activeContextUsageKey] ?? null
    : null;
  const liveChatStatistics = activeChatKey
    ? liveChatStatisticsByKey[activeChatKey] ?? null
    : null;
  const displayedChatStatistics = liveChatStatistics
    ? withLiveChatStatistics(
      chatStatistics,
      liveChatStatistics,
      messages,
      activeWorkspaceId,
      activeChatId,
    )
    : chatStatistics;
  const isLoadingContextUsage = activeContextUsageKey
    ? contextUsageLoadingByChatKey[activeContextUsageKey] ?? false
    : false;
  const activeRunInfo = activeChatKey
    ? activeRunInfoByChatKey[activeChatKey] ?? null
    : null;
  const activeChatReadOnly = activeChatKey
    ? readOnlyChatKeys[activeChatKey] === true
    : false;
  const canUseTeamMode = agentDefinitions.length > 1;
  const isSendingMessage =
    activeChatKey !== null && runningChatKeys.has(activeChatKey);
  const queuedRunRequests = activeChatKey
    ? queuedRunRequestsByChatKey[activeChatKey] ?? []
    : [];
  const queuedMessageIds = useMemo(
    () =>
      new Set(
        queuedRunRequests.flatMap((request) => request.pendingUserMessageId ?? []),
      ),
    [queuedRunRequests],
  );
  const mainTabs = useMemo<MainTabSummary[]>(
    () => [
      ...openChatTabs.map((tab) => ({
        ...hydrateChatTab(tab, workspaces),
        type: "chat" as const,
      })),
      ...openAgentTabs.map((tab) => ({
        ...hydrateAgentTab(tab, workspaces),
        type: "agent" as const,
      })),
      ...openFileTabs.map((tab) => ({
        ...tab,
        title: tab.name,
        type: "file" as const,
      })),
    ],
    [openAgentTabs, openChatTabs, openFileTabs, workspaces],
  );
  const activeFileEditorKey =
    activeMainTab.type === "file"
      ? workspaceFileEditorKey(activeMainTab.workspaceId, activeMainTab.path)
      : null;
  const activeFileTab =
    activeMainTab.type === "file"
      ? openFileTabs.find(
        (tab) =>
          tab.workspaceId === activeMainTab.workspaceId &&
          tab.path === activeMainTab.path,
      ) ?? null
      : null;
  const activeAgentTab =
    activeMainTab.type === "agent"
      ? mainTabs.find(
        (tab): tab is Extract<MainTabSummary, { type: "agent" }> =>
          tab.type === "agent" &&
          tab.workspaceId === activeMainTab.workspaceId &&
          tab.chatId === activeMainTab.chatId &&
          tab.instanceId === activeMainTab.instanceId,
      ) ?? null
      : null;
  const activeFileEditor = activeFileEditorKey
    ? workspaceFileEditors[activeFileEditorKey] ?? null
    : null;
  const openChatKeySet = useMemo(
    () =>
      new Set(
        openChatTabs.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
      ),
    [openChatTabs],
  );
  const configuredModelsByName = useMemo(
    () =>
      [...(settings?.configuredModels ?? [])].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    [settings?.configuredModels],
  );
  const availableModels = useMemo(
    () =>
      configuredModelsByName.filter(
        (model) =>
          model.enabled &&
          model.canEnable &&
          model.activeProviderId !== null &&
          model.providerIds.length > 0,
      ),
    [configuredModelsByName],
  );
  const defaultAgentDefinition = useMemo(
    () =>
      agentDefinitions.find(
        (definition) => definition.id === DEFAULT_AGENT_DEFINITION_ID,
      ) ?? null,
    [agentDefinitions],
  );
  const defaultComposerSelection = useMemo<ComposerDefaultSelection>(() => {
    if (defaultAgentDefinition) {
      const agentModel = availableModels.find(
        (model) =>
          model.id === defaultAgentDefinition.modelId &&
          model.providerIds.includes(defaultAgentDefinition.providerId),
      );
      if (agentModel) {
        return {
          modelId: agentModel.id,
          providerId: defaultAgentDefinition.providerId,
          thinkingLevel:
            defaultAgentDefinition.modelOptions.thinkingLevel ?? "",
        };
      }
    }

    const model = availableModels[0];
    if (!model) {
      return { modelId: "", providerId: "", thinkingLevel: "" };
    }

    const providerId =
      model.activeProviderId && model.providerIds.includes(model.activeProviderId)
        ? model.activeProviderId
        : model.providerIds[0] ?? "";

    return {
      modelId: model.id,
      providerId,
      thinkingLevel: model.thinkingLevel ?? "",
    };
  }, [availableModels, defaultAgentDefinition]);
  const detectedSkills = useMemo(
    () => settings?.skills.detected ?? [],
    [settings],
  );
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const isTerminalOpen = activeWorkspace
    ? terminalOpenWorkspaceIds.has(activeWorkspace.id)
    : false;
  const isGlobalView =
    viewMode === "settings" || viewMode === "stats" || viewMode === "scheduled";
  const showContextPanel = !isGlobalView && isContextPanelOpen;
  const canUseApp = Boolean(
    authStatus && (!authStatus.enabled || authStatus.authenticated),
  );
  const canLogout = Boolean(settings?.general.webServer.passwordEnabled);
  const canUseNativePicker = nativeBrowserToken !== null;
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

      const routeWithTabs = route.viewMode === "chat"
        ? browserRouteWithOpenTabs(
          route,
          openChatTabsRef.current,
          openFileTabsRef.current,
        )
        : route;
      const nextPath = browserPathForRoute(routeWithTabs);
      const currentPath = `${window.location.pathname}${window.location.search}`;
      if (currentPath === nextPath) {
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

  useDocumentLanguage(language);
  useDocumentTheme(theme);

  useEffect(() => {
    openChatTabsRef.current = openChatTabs;
  }, [openChatTabs]);

  useEffect(() => {
    openFileTabsRef.current = openFileTabs;
  }, [openFileTabs]);

  useEffect(() => {
    activeWorkspaceIdRef.current = activeWorkspaceId;
    activeChatIdRef.current = activeChatId;
    activeChatKeyRef.current =
      activeChatId === null || isPendingChatId(activeChatId)
        ? activeChatKeyRef.current
        : chatRunKey(activeWorkspaceId, activeChatId);
  }, [activeChatId, activeWorkspaceId]);

  useEffect(() => {
    const chatKey =
      activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)
        ? chatRunKey(activeWorkspaceId, activeChatId)
        : null;
    const identity = [
      activeWorkspaceId,
      activeChatId ?? "",
      selectedModelId,
      selectedProviderId,
      selectedThinkingLevel,
    ].join("\u0000");

    if (!chatKey) {
      return;
    }

    if (contextUsageIdentityByChatKeyRef.current.get(chatKey) === identity) {
      return;
    }

    if (isSendingMessage) {
      return;
    }

    contextUsageIdentityByChatKeyRef.current.set(chatKey, identity);
    contextUsageAbortByChatKeyRef.current.get(chatKey)?.abort();
    contextUsageAbortByChatKeyRef.current.delete(chatKey);
    setContextUsageByChatKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }

      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
    setContextUsageLoadingByChatKey((current) => ({
      ...current,
      [chatKey]: false,
    }));

    contextUsageRequestIdByChatKeyRef.current.set(
      chatKey,
      (contextUsageRequestIdByChatKeyRef.current.get(chatKey) ?? 0) + 1,
    );
  }, [
    activeChatId,
    activeWorkspaceId,
    isSendingMessage,
    selectedModelId,
    selectedProviderId,
    selectedThinkingLevel,
  ]);

  useLayoutEffect(() => {
    selectedModelIdRef.current = selectedModelId;
  }, [selectedModelId]);

  useLayoutEffect(() => {
    selectedProviderIdRef.current = selectedProviderId;
  }, [selectedProviderId]);

  useLayoutEffect(() => {
    selectedThinkingLevelRef.current = selectedThinkingLevel;
  }, [selectedThinkingLevel]);

  useEffect(
    () => () => {
      for (const abortController of contextUsageAbortByChatKeyRef.current.values()) {
        abortController.abort();
      }
      contextUsageAbortByChatKeyRef.current.clear();
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
      setIsTeamModeEnabled(data.general.defaultTeamModeEnabled);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, []);

  const loadAgentDefinitions = useCallback(async () => {
    setIsLoadingAgentDefinitions(true);
    setAgentDefinitionsError(null);

    try {
      const data = await requestJson<AgentDefinitionsResponse>(
        "/api/agent-definitions",
      );
      setAgentDefinitions(data.agentDefinitions);
      return data.agentDefinitions;
    } catch (requestError) {
      setAgentDefinitionsError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingAgentDefinitions(false);
    }
  }, []);

  const loadAgentTeamSnapshot = useCallback(
    async (workspaceId: string, chatId: string) => {
      setIsLoadingAgentTeam(true);
      setAgentTeamError(null);

      try {
        const data = await requestJson<AgentTeamSnapshotResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/agent-team`,
        );
        setAgentTeamSnapshot(data);
        return data;
      } catch (requestError) {
        const message = errorMessage(requestError);
        if (message.includes("has no Agent team")) {
          setAgentTeamSnapshot(null);
          return null;
        }
        setAgentTeamSnapshot(null);
        setAgentTeamError(message);
        return null;
      } finally {
        setIsLoadingAgentTeam(false);
      }
    },
    [],
  );

  const handleAgentTeamRefresh = useCallback(
    (event: Extract<ChatStreamEvent, { type: "agentTeamRefresh" }>) => {
      if (activeChatKeyRef.current !== chatRunKey(event.workspaceId, event.chatId)) {
        return;
      }

      if (event.revealPanel) {
        setContextPanelTab("agents");
        setIsContextPanelOpen(true);
      }
      void loadAgentTeamSnapshot(event.workspaceId, event.chatId);
    },
    [loadAgentTeamSnapshot],
  );

  const refreshActiveAgentTeamSnapshot = useCallback(
    (workspaceId: string, chatId: string) => {
      if (activeChatKeyRef.current !== chatRunKey(workspaceId, chatId)) {
        return;
      }

      void loadAgentTeamSnapshot(workspaceId, chatId);
    },
    [loadAgentTeamSnapshot],
  );

  useEffect(() => {
    if (!canUseApp) {
      return;
    }

    void loadAgentDefinitions();
  }, [canUseApp, loadAgentDefinitions]);

  useEffect(() => {
    if (
      !canUseApp ||
      (activeMainTab.type !== "chat" && activeMainTab.type !== "agent") ||
      !activeWorkspaceId ||
      !activeChatId ||
      isPendingChatId(activeChatId)
    ) {
      setAgentTeamSnapshot(null);
      setAgentTeamError(null);
      return;
    }

    void loadAgentTeamSnapshot(activeWorkspaceId, activeChatId);
  }, [activeChatId, activeMainTab.type, activeWorkspaceId, canUseApp, loadAgentTeamSnapshot]);

  const visibleAgentSnapshotTarget = useMemo(() => {
    if (activeMainTab.type === "agent" && activeAgentTab) {
      return {
        chatId: activeAgentTab.chatId,
        workspaceId: activeAgentTab.workspaceId,
      };
    }

    if (
      isContextPanelOpen &&
      contextPanelTab === "agents" &&
      activeWorkspaceId &&
      activeChatId &&
      !isPendingChatId(activeChatId)
    ) {
      return { chatId: activeChatId, workspaceId: activeWorkspaceId };
    }

    return null;
  }, [
    activeAgentTab,
    activeChatId,
    activeMainTab.type,
    activeWorkspaceId,
    contextPanelTab,
    isContextPanelOpen,
  ]);

  const visibleAgentSnapshotHasRunningTask = Boolean(
    visibleAgentSnapshotTarget &&
      agentTeamSnapshot?.team.chatId === visibleAgentSnapshotTarget.chatId &&
      agentTeamSnapshot.tasks.some((task) => task.status === "running"),
  );

  useEffect(() => {
    if (
      !canUseApp ||
      !visibleAgentSnapshotTarget ||
      !visibleAgentSnapshotHasRunningTask ||
      isLoadingAgentTeam
    ) {
      return;
    }

    const refreshTimer = window.setTimeout(() => {
      void loadAgentTeamSnapshot(
        visibleAgentSnapshotTarget.workspaceId,
        visibleAgentSnapshotTarget.chatId,
      );
    }, AGENT_TEAM_RUNNING_REFRESH_MS);

    return () => window.clearTimeout(refreshTimer);
  }, [
    canUseApp,
    isLoadingAgentTeam,
    loadAgentTeamSnapshot,
    visibleAgentSnapshotHasRunningTask,
    visibleAgentSnapshotTarget,
  ]);

  async function createAgentDefinition(definition: AgentDefinitionInput) {
    setAgentDefinitionOperationKey("agent-definition-save");
    setAgentDefinitionsError(null);

    try {
      const data = await requestJson<AgentDefinitionsResponse>(
        "/api/agent-definitions/create",
        {
          body: JSON.stringify({ definition }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setAgentDefinitions(data.agentDefinitions);
      return true;
    } catch (requestError) {
      setAgentDefinitionsError(errorMessage(requestError));
      return false;
    } finally {
      setAgentDefinitionOperationKey(null);
    }
  }

  async function updateAgentDefinition(
    id: string,
    definition: AgentDefinitionInput,
  ) {
    setAgentDefinitionOperationKey("agent-definition-save");
    setAgentDefinitionsError(null);

    try {
      const data = await requestJson<AgentDefinitionsResponse>(
        "/api/agent-definitions/update",
        {
          body: JSON.stringify({ definition, id }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setAgentDefinitions(data.agentDefinitions);
      return true;
    } catch (requestError) {
      setAgentDefinitionsError(errorMessage(requestError));
      return false;
    } finally {
      setAgentDefinitionOperationKey(null);
    }
  }

  async function deleteAgentDefinition(id: string) {
    setAgentDefinitionOperationKey("agent-definition-delete");
    setAgentDefinitionsError(null);

    try {
      const data = await requestJson<AgentDefinitionsResponse>(
        "/api/agent-definitions/delete",
        {
          body: JSON.stringify({ id }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setAgentDefinitions(data.agentDefinitions);
    } catch (requestError) {
      setAgentDefinitionsError(errorMessage(requestError));
    } finally {
      setAgentDefinitionOperationKey(null);
    }
  }

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

  const loadWorkspaceFiles = useCallback(async (workspaceId: string) => {
    setIsLoadingWorkspaceFiles(true);
    setWorkspaceFilesError(null);

    try {
      const data = await requestJson<WorkspaceFilesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/files`,
      );
      setWorkspaceFiles(data);
      setLoadingWorkspaceDirectoryPaths(new Set());
      return data;
    } catch (requestError) {
      setWorkspaceFiles(null);
      setWorkspaceFilesError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingWorkspaceFiles(false);
    }
  }, []);

  const loadWorkspaceDirectoryChildren = useCallback(
    async (workspaceId: string, path: string) => {
      setLoadingWorkspaceDirectoryPaths((current) => new Set(current).add(path));
      setWorkspaceFilesError(null);

      try {
        const query = new URLSearchParams({ path });
        const data = await requestJson<WorkspaceFileChildrenResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/files/children?${query.toString()}`,
        );
        setWorkspaceFiles((current) =>
          current
            ? {
                ...current,
                root: replaceWorkspaceFileNodeChildren(current.root, data.path, data.children),
              }
            : current,
        );
        return data;
      } catch (requestError) {
        setWorkspaceFilesError(errorMessage(requestError));
        return null;
      } finally {
        setLoadingWorkspaceDirectoryPaths((current) => {
          const next = new Set(current);
          next.delete(path);
          return next;
        });
      }
    },
    [],
  );

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
        page: String(contextMemoryPages.global.page),
        pageSize: String(contextMemoryPages.global.pageSize),
        scope: "global",
        status: "active",
      });
      const workspaceParams = new URLSearchParams({
        page: String(contextMemoryPages.workspace.page),
        pageSize: String(contextMemoryPages.workspace.pageSize),
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
        global: {
          memories: globalData.memories,
          page: globalData.page,
          pageSize: globalData.pageSize,
          totalCount: globalData.totalCount,
          totalPages: globalData.totalPages,
        },
        workspace: {
          memories: workspaceData.memories,
          page: workspaceData.page,
          pageSize: workspaceData.pageSize,
          totalCount: workspaceData.totalCount,
          totalPages: workspaceData.totalPages,
        },
      });
    } catch (requestError) {
      setContextMemories({
        global: { memories: [], page: 1, pageSize: 10, totalCount: 0, totalPages: 0 },
        workspace: { memories: [], page: 1, pageSize: 10, totalCount: 0, totalPages: 0 },
      });
      setContextMemoryError(errorMessage(requestError));
    } finally {
      setIsLoadingContextMemories(false);
    }
  }, [contextMemoryPages]);

  const loadWorkspaceSpec = useCallback(async (workspaceId: string) => {
    setIsLoadingWorkspaceSpec(true);
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);

    try {
      const data = await requestJson<WorkspaceSpecResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec`,
      );
      if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
        return null;
      }
      setWorkspaceSpec(data);
      setWorkspaceSpecDraft(data.contentMarkdown);
      return data;
    } catch (requestError) {
      setWorkspaceSpec(null);
      setWorkspaceSpecDraft("");
      setWorkspaceSpecError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingWorkspaceSpec(false);
    }
  }, []);

  // ponytail: poll a queued spec job until it settles, then reload spec content.
  // Ceiling: fixed backoff schedule (~165s total); upgrade path is an SSE/job push.
  const pollWorkspaceSpecJobUntilSettled = useCallback(
    async (workspaceId: string, jobId: string) => {
      const attempts = [1000, 2000, 4000, 8000, 15000, 30000, 45000, 60000];
      try {
        for (const delayMs of attempts) {
          await new Promise<void>((resolve) => {
            window.setTimeout(resolve, delayMs);
          });
          if (activeWorkspaceIdRef.current !== workspaceId) {
            return;
          }
          const jobsResponse = await requestJson<WorkspaceSpecJobsResponse>(
            `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs?limit=24`,
          );
          const job = jobsResponse.jobs.find(
            (candidate) => candidate.id === jobId,
          );
          if (!job) {
            continue;
          }
          if (job.status === "queued" || job.status === "running") {
            continue;
          }
          if (job.status === "completed" && activeWorkspaceIdRef.current === workspaceId) {
            await loadWorkspaceSpec(workspaceId);
          }
          return;
        }
      } catch {
        return;
      }
    },
    [loadWorkspaceSpec],
  );

  const saveWorkspaceSpecSettings = useCallback(
    async (
      workspaceId: string,
      enabled: boolean,
      injectEnabled: boolean,
    ) => {
      const hasUnsavedDraft =
        workspaceSpec !== null &&
        workspaceSpecDraft !== workspaceSpec.contentMarkdown;
      setWorkspaceSpecOperationKey("settings");
      setWorkspaceSpecError(null);
      setWorkspaceSpecConflictMessage(null);

      try {
        const data = await saveWorkspaceSpecSettingsRequest(
          workspaceId,
          enabled,
          injectEnabled,
        );
        setWorkspaceSpec(data);
        if (!hasUnsavedDraft) {
          setWorkspaceSpecDraft(data.contentMarkdown);
        }
        return true;
      } catch (requestError) {
        setWorkspaceSpecError(errorMessage(requestError));
        return false;
      } finally {
        setWorkspaceSpecOperationKey((current) =>
          current === "settings" ? null : current,
        );
      }
    },
    [workspaceSpec, workspaceSpecDraft],
  );

  const saveWorkspaceSpecContent = useCallback(async () => {
    if (!activeWorkspace?.id || !workspaceSpec) {
      return false;
    }

    setWorkspaceSpecOperationKey("save");
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/spec`,
        {
          body: JSON.stringify({
            contentMarkdown: workspaceSpecDraft,
            expectedRevision: workspaceSpec.revision,
          }),
          cache: "no-store",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json" },
          method: "PUT",
        },
      );
      if (response.status === 409) {
        setWorkspaceSpecConflictMessage(await responseErrorMessage(response));
        return false;
      }
      if (!response.ok) {
        throw new Error(await responseErrorMessage(response));
      }

      const data = (await response.json()) as WorkspaceSpecResponse;
      setWorkspaceSpec(data);
      setWorkspaceSpecDraft(data.contentMarkdown);
      return true;
    } catch (requestError) {
      setWorkspaceSpecError(errorMessage(requestError));
      return false;
    } finally {
      setWorkspaceSpecOperationKey((current) =>
        current === "save" ? null : current,
      );
    }
  }, [activeWorkspace?.id, workspaceSpec, workspaceSpecDraft]);

  const generateWorkspaceSpec = useCallback(async () => {
    if (!activeWorkspace?.id) {
      return false;
    }
    const workspaceId = activeWorkspace.id;

    setWorkspaceSpecOperationKey("generate");
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);

    try {
      const data = await requestJson<GenerateWorkspaceSpecResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/generate`,
        {
          body: JSON.stringify({ modelId: null }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setWorkspaceSpec((current) =>
        current ? { ...current, latestJob: data.job } : current,
      );
      // ponytail: poll the queued job to completion, then reload spec content so
      // the panel updates without a manual refresh. Ceiling: fixed backoff
      // schedule (~165s total); upgrade path is an SSE/job-event push.
      void pollWorkspaceSpecJobUntilSettled(workspaceId, data.job.id);
      return true;
    } catch (requestError) {
      setWorkspaceSpecError(errorMessage(requestError));
      return false;
    } finally {
      setWorkspaceSpecOperationKey((current) =>
        current === "generate" ? null : current,
      );
    }
  }, [activeWorkspace?.id, pollWorkspaceSpecJobUntilSettled]);

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

  const goToContextMemoryPage = useCallback(
    (scope: "global" | "workspace", page: number) => {
      setContextMemoryPages((current) => ({
        ...current,
        [scope]: { ...current[scope], page },
      }));
    },
    [],
  );

  const loadTodoGraph = useCallback(
    async (
      workspaceId: string,
      chatId: string,
      options: { ignoreRequestInvalidation?: boolean } = {},
    ) => {
      const requestedChatKey = chatRunKey(workspaceId, chatId);
      const requestId = todoGraphRequestIdRef.current + 1;
      todoGraphRequestIdRef.current = requestId;
      const isCurrentRequest = () =>
        activeChatKeyRef.current === requestedChatKey &&
        (options.ignoreRequestInvalidation ||
          todoGraphRequestIdRef.current === requestId);
      setIsLoadingTodoGraph(true);
      setTodoGraphError(null);

      try {
        const data = await requestJson<TodoGraphResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/todo-graph`,
        );
        if (isCurrentRequest()) {
          setTodoGraph(data);
          setTodoGraphError(null);
        }
      } catch (requestError) {
        if (isCurrentRequest()) {
          setTodoGraph(null);
          setTodoGraphError(errorMessage(requestError));
        }
      } finally {
        if (isCurrentRequest()) {
          setIsLoadingTodoGraph(false);
        }
      }
    },
    [],
  );

  const loadChatStatistics = useCallback(
    async (workspaceId: string, chatId: string) => {
      const requestedChatKey = chatRunKey(workspaceId, chatId);
      setIsLoadingChatStatistics(true);
      setChatStatisticsError(null);

      try {
        const data = await requestJson<ChatStatisticsResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/statistics`,
        );
        if (activeChatKeyRef.current === requestedChatKey) {
          setChatStatistics(data);
          if (!runningChatKeysRef.current.has(requestedChatKey)) {
            clearLiveChatStatistics(requestedChatKey);
          }
        }
      } catch (requestError) {
        if (activeChatKeyRef.current === requestedChatKey) {
          setChatStatistics(null);
          setChatStatisticsError(errorMessage(requestError));
        }
      } finally {
        if (activeChatKeyRef.current === requestedChatKey) {
          setIsLoadingChatStatistics(false);
        }
      }
    },
    [],
  );

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
    const probePort = settings?.nativeTools.browserProbePort;
    if (!canUseApp || typeof probePort !== "number") {
      setNativeBrowserToken(null);
      return;
    }

    let isCurrent = true;
    let token: string;
    try {
      token = createNativeBrowserToken();
    } catch (error) {
      console.error(error);
      setNativeBrowserToken(null);
      return;
    }
    setNativeBrowserToken(null);

    void probeNativeBrowser(probePort, token).then((available) => {
      if (isCurrent) {
        setNativeBrowserToken(available ? token : null);
      }
    });

    return () => {
      isCurrent = false;
    };
  }, [canUseApp, settings?.nativeTools.browserProbePort]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setWorkspaceFiles(null);
      setWorkspaceFilesError(null);
      setIsLoadingWorkspaceFiles(false);
      return;
    }

    if (!isContextPanelOpen || contextPanelTab !== "files") {
      return;
    }

    void loadWorkspaceFiles(activeWorkspace.id);
  }, [
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    loadWorkspaceFiles,
  ]);

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
    const todoGraphChatTarget =
      activeWorkspace?.id && activeChatKey ? parseChatRunKey(activeChatKey) : null;

    if (
      !activeWorkspace?.id ||
      !todoGraphChatTarget ||
      todoGraphChatTarget.workspaceId !== activeWorkspace.id ||
      isPendingChatId(todoGraphChatTarget.chatId)
    ) {
      todoGraphRequestIdRef.current += 1;
      setTodoGraph(null);
      setTodoGraphError(null);
      setIsLoadingTodoGraph(false);
      return;
    }

    setTodoGraph(null);
    setTodoGraphError(null);
    void loadTodoGraph(
      todoGraphChatTarget.workspaceId,
      todoGraphChatTarget.chatId,
    );
  }, [activeChatKey, activeWorkspace?.id, loadTodoGraph]);

  useEffect(() => {
    if (
      !activeWorkspace?.id ||
      !activeChatId ||
      isPendingChatId(activeChatId)
    ) {
      setChatStatistics(null);
      setChatStatisticsError(null);
      setIsLoadingChatStatistics(false);
      return;
    }

    const requestedChatKey = chatRunKey(activeWorkspace.id, activeChatId);
    setChatStatistics(null);
    setChatStatisticsError(null);
    if (!runningChatKeysRef.current.has(requestedChatKey)) {
      clearLiveChatStatistics(requestedChatKey);
    }
    void loadChatStatistics(activeWorkspace.id, activeChatId);
  }, [activeChatId, activeWorkspace?.id, loadChatStatistics]);

  useEffect(() => {
    if (
      contextPanelTab !== "stats" ||
      !activeWorkspace?.id ||
      !activeChatId ||
      isPendingChatId(activeChatId)
    ) {
      return;
    }

    void loadChatStatistics(activeWorkspace.id, activeChatId);
  }, [
    activeChatId,
    activeWorkspace?.id,
    contextPanelTab,
    loadChatStatistics,
  ]);

  useEffect(() => {
    if (contextPanelTab !== "memory" || !activeWorkspace?.id) {
      return;
    }

    void loadContextMemories(activeWorkspace.id);
  }, [activeWorkspace?.id, contextPanelTab, loadContextMemories]);

  useEffect(() => {
    if (contextPanelTab !== "spec" || !activeWorkspace?.id) {
      return;
    }

    void loadWorkspaceSpec(activeWorkspace.id);
  }, [activeWorkspace?.id, contextPanelTab, loadWorkspaceSpec]);

  useEffect(() => {
    setContextMemoryPages({
      global: { page: 1, pageSize: 10 },
      workspace: { page: 1, pageSize: 10 },
    });
  }, [activeWorkspace?.id]);

  useEffect(() => {
    setWorkspaceSpec(null);
    setWorkspaceSpecDraft("");
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);
  }, [activeWorkspace?.id]);

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
    const nextRun = scheduledWorkspaceRuns.find(
      (run) =>
        run.status === "queued" &&
        !workspaceHasRunningOrStartingRun(run.workspaceId),
    );
    if (!nextRun) {
      return;
    }

    updateScheduledWorkspaceRuns((current) =>
      current.map((run) =>
        run.id === nextRun.id ? { ...run, status: "starting" } : run,
      ),
    );

    void (async () => {
      const createdChatId = await runChatMessage(nextRun.request);
      if (createdChatId) {
        updateScheduledWorkspaceRuns((current) =>
          current.map((run) =>
            run.id === nextRun.id ? { ...run, createdChatId } : run,
          ),
        );
      } else {
        updateScheduledWorkspaceRuns((current) =>
          current.filter((run) => run.id !== nextRun.id),
        );
      }
    })();
  }, [runningChatKeys, scheduledWorkspaceRuns, workspaces]);

  useEffect(() => {
    setOpenChatTabs((current) => {
      const next = current.filter(
        (tab) =>
          (isPendingChatId(tab.chatId) &&
            workspaces.some((workspace) => workspace.id === tab.workspaceId)) ||
          workspaceHasChatTab(workspaces, tab) ||
          scheduledWorkspaceRunsRef.current.some(
            (run) => run.workspaceId === tab.workspaceId && run.chatId === tab.chatId,
          ),
      );
      return next.length === current.length ? current : next;
    });

    setOpenFileTabs((current) => {
      const next = current.filter((tab) =>
        workspaces.some((workspace) => workspace.id === tab.workspaceId),
      );
      return next.length === current.length ? current : next;
    });

    setOpenAgentTabs((current) => {
      const next = current.filter((tab) => workspaceHasChatTab(workspaces, tab));
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
    updateScheduledWorkspaceRunsByWorkspaceList(workspaces);
  }, [workspaces]);

  useRightPanelResizeEffect({
    isResizing: isResizingDiffPanel,
    maxHeightRatio: CONTEXT_PANEL_MAX_HEIGHT_RATIO,
    maxWidth: CONTEXT_PANEL_MAX_WIDTH,
    minHeight: CONTEXT_PANEL_MIN_HEIGHT,
    minWidth: CONTEXT_PANEL_MIN_WIDTH,
    mobileBreakpoint: MOBILE_BREAKPOINT_PX,
    onResizeEnd: () => setIsResizingDiffPanel(false),
    setHeight: setContextPanelMobileHeight,
    setWidth: setDiffPanelWidth,
  });

  useSidebarResizeEffect({
    isResizing: isResizingSidebar,
    onPointerMove: updateSidebarWidthFromClientX,
    onResizeEnd: () => setIsResizingSidebar(false),
  });

  useEffect(() => {
    if (!workspaceChatContextMenu) {
      return;
    }

    function closeWorkspaceChatContextMenuForPointer(event: PointerEvent) {
      const target = event.target;
      if (
        target instanceof Element &&
        target.closest(".workspace-chat-context-menu")
      ) {
        return;
      }
      setWorkspaceChatContextMenu(null);
    }

    function closeWorkspaceChatContextMenuForKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setWorkspaceChatContextMenu(null);
      }
    }

    function closeWorkspaceChatContextMenu() {
      setWorkspaceChatContextMenu(null);
    }

    window.addEventListener("pointerdown", closeWorkspaceChatContextMenuForPointer);
    window.addEventListener("keydown", closeWorkspaceChatContextMenuForKey);
    window.addEventListener("resize", closeWorkspaceChatContextMenu);
    window.addEventListener("scroll", closeWorkspaceChatContextMenu, true);

    return () => {
      window.removeEventListener("pointerdown", closeWorkspaceChatContextMenuForPointer);
      window.removeEventListener("keydown", closeWorkspaceChatContextMenuForKey);
      window.removeEventListener("resize", closeWorkspaceChatContextMenu);
      window.removeEventListener("scroll", closeWorkspaceChatContextMenu, true);
    };
  }, [workspaceChatContextMenu]);

  useEffect(() => {
    return () => {
      if (workspaceChatLongPressTimeoutRef.current !== null) {
        window.clearTimeout(workspaceChatLongPressTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!workspaceFileContextMenu) {
      return;
    }

    function closeWorkspaceFileContextMenuForPointer(event: PointerEvent) {
      const target = event.target;
      if (
        target instanceof Element &&
        target.closest(".workspace-file-context-menu")
      ) {
        return;
      }
      setWorkspaceFileContextMenu(null);
    }

    function closeWorkspaceFileContextMenuForKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setWorkspaceFileContextMenu(null);
      }
    }

    function closeWorkspaceFileContextMenu() {
      setWorkspaceFileContextMenu(null);
    }

    window.addEventListener("pointerdown", closeWorkspaceFileContextMenuForPointer);
    window.addEventListener("keydown", closeWorkspaceFileContextMenuForKey);
    window.addEventListener("resize", closeWorkspaceFileContextMenu);
    window.addEventListener("scroll", closeWorkspaceFileContextMenu, true);

    return () => {
      window.removeEventListener("pointerdown", closeWorkspaceFileContextMenuForPointer);
      window.removeEventListener("keydown", closeWorkspaceFileContextMenuForKey);
      window.removeEventListener("resize", closeWorkspaceFileContextMenu);
      window.removeEventListener("scroll", closeWorkspaceFileContextMenu, true);
    };
  }, [workspaceFileContextMenu]);

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
      if (!defaultComposerSelection.modelId) {
        hasManuallySelectedModelRef.current = false;
        return "";
      }

      if (!hasManuallySelectedModelRef.current) {
        return defaultComposerSelection.modelId;
      }

      if (availableModels.some((model) => model.id === current)) {
        return current;
      }

      hasManuallySelectedModelRef.current = false;
      return defaultComposerSelection.modelId;
    });
  }, [availableModels, defaultComposerSelection.modelId]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );
    const supportedThinkingValues = new Set([
      "",
      ...thinkingLevels.map((level) => level.value),
    ]);

    setSelectedThinkingLevel((current) => {
      if (!selectedModel) {
        hasManuallySelectedThinkingLevelRef.current = false;
        return "";
      }

      const defaultThinkingLevel =
        !hasManuallySelectedModelRef.current &&
          selectedModel.id === defaultComposerSelection.modelId
          ? defaultComposerSelection.thinkingLevel
          : selectedModel.thinkingLevel ?? "";

      if (!hasManuallySelectedThinkingLevelRef.current) {
        return defaultThinkingLevel;
      }

      if (supportedThinkingValues.has(current)) {
        return current;
      }

      hasManuallySelectedThinkingLevelRef.current = false;
      return defaultThinkingLevel;
    });
  }, [
    availableModels,
    defaultComposerSelection.modelId,
    defaultComposerSelection.thinkingLevel,
    selectedModelId,
    thinkingLevels,
  ]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );

    setSelectedProviderId((current) => {
      if (!selectedModel?.providerIds.length) {
        return "";
      }

      if (
        !hasManuallySelectedModelRef.current &&
        selectedModel.id === defaultComposerSelection.modelId &&
        defaultComposerSelection.providerId &&
        selectedModel.providerIds.includes(defaultComposerSelection.providerId)
      ) {
        return defaultComposerSelection.providerId;
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
  }, [
    availableModels,
    defaultComposerSelection.modelId,
    defaultComposerSelection.providerId,
    selectedModelId,
  ]);

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
          contentBase64: workspaceIconDraft?.contentBase64 ?? null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const createdWorkspace =
        data.workspaces.find(
          (workspace) => workspace.id === data.activeWorkspaceId,
        ) ?? data.workspaces[0];

      setWorkspaces(data.workspaces);
      setActiveWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      setExpandedWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      updateBrowserRoute({
        chatId: null,
        viewMode: "chat",
        workspaceId: createdWorkspace?.id ?? data.activeWorkspaceId,
      });
      if (workspaceSpecEnabled && createdWorkspace?.id) {
        try {
          await saveWorkspaceSpecSettingsRequest(createdWorkspace.id, true, false);
        } catch (specError) {
          setError(errorMessage(specError));
        }
      }
      setWorkspaceName("");
      setWorkspacePath("");
      setWorkspaceSpecEnabled(false);
      closeWorkspaceDialog();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  function clearWorkspaceIconDraft() {
    setWorkspaceIconDraft(null);
  }

  async function handleWorkspaceIconFileChange(
    event: ReactChangeEvent<HTMLInputElement>,
  ) {
    const file = event.target.files?.[0] ?? null;
    event.target.value = "";
    if (!file) {
      return;
    }

    try {
      const contentBase64 = await fileToBase64(file);
      setWorkspaceIconDraft({
        contentBase64,
        name: file.name,
        previewUrl: file.type
          ? `data:${file.type};base64,${contentBase64}`
          : "",
      });
    } catch (readError) {
      setError(errorMessage(readError));
    }
  }

  async function handleSelectWorkspacePath() {
    if (!nativeBrowserToken) {
      setError(
        t(
          "Native file browsing is only available from a browser running on the Foco computer.",
        ),
      );
      return;
    }

    setIsSelectingWorkspacePath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        nativePickerRequestInit(nativeBrowserToken),
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

    moveContextUsageForChatKey(fromChatKey, toChatKey);
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

  function moveContextUsageForChatKey(fromChatKey: string, toChatKey: string) {
    setContextUsageByChatKey((current) => {
      if (!(fromChatKey in current)) {
        return current;
      }

      const { [fromChatKey]: movedUsage, ...next } = current;
      return { ...next, [toChatKey]: movedUsage };
    });
    setContextUsageLoadingByChatKey((current) => {
      if (!(fromChatKey in current)) {
        return current;
      }

      const { [fromChatKey]: movedLoading, ...next } = current;
      return { ...next, [toChatKey]: movedLoading };
    });

    const abortController =
      contextUsageAbortByChatKeyRef.current.get(fromChatKey);
    if (abortController) {
      contextUsageAbortByChatKeyRef.current.delete(fromChatKey);
      contextUsageAbortByChatKeyRef.current.set(toChatKey, abortController);
    }

    const requestId =
      contextUsageRequestIdByChatKeyRef.current.get(fromChatKey);
    if (requestId !== undefined) {
      contextUsageRequestIdByChatKeyRef.current.delete(fromChatKey);
      contextUsageRequestIdByChatKeyRef.current.set(toChatKey, requestId);
    }

    const identity = contextUsageIdentityByChatKeyRef.current.get(fromChatKey);
    if (identity !== undefined) {
      contextUsageIdentityByChatKeyRef.current.delete(fromChatKey);
      contextUsageIdentityByChatKeyRef.current.set(toChatKey, identity);
    }

  }

  function cancelContextUsageRequestForChatKey(chatKey: string) {
    contextUsageAbortByChatKeyRef.current.get(chatKey)?.abort();
    contextUsageAbortByChatKeyRef.current.delete(chatKey);
    contextUsageRequestIdByChatKeyRef.current.set(
      chatKey,
      (contextUsageRequestIdByChatKeyRef.current.get(chatKey) ?? 0) + 1,
    );
    setContextUsageLoadingByChatKey((current) => ({
      ...current,
      [chatKey]: false,
    }));
  }

  function removeContextUsageForChatKey(chatKey: string) {
    cancelContextUsageRequestForChatKey(chatKey);
    contextUsageIdentityByChatKeyRef.current.delete(chatKey);
    contextUsageRequestIdByChatKeyRef.current.delete(chatKey);
    setContextUsageByChatKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }

      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
    setContextUsageLoadingByChatKey((current) => {
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
        specUpdates: [],
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

  function clearLiveChatStatistics(chatKey: string) {
    setLiveChatStatisticsByKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }

      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
  }

  function updateLiveChatStatistics(
    chatKey: string,
    updater: LiveChatStatistics | null,
  ) {
    if (updater === null) {
      clearLiveChatStatistics(chatKey);
      return;
    }

    setLiveChatStatisticsByKey((current) => ({
      ...current,
      [chatKey]: {
        ...updater,
        codeChangeStats:
          updater.codeChangeStats ??
          current[chatKey]?.codeChangeStats ??
          emptyGitDiffLineStats(),
      },
    }));
  }

  function setChatRunning(chatKey: string, running: boolean) {
    if (running) {
      runningChatKeysRef.current.add(chatKey);
    } else {
      runningChatKeysRef.current.delete(chatKey);
    }
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
    const nextRef = { ...activeRunInfoByChatKeyRef.current };
    if (runInfo) {
      nextRef[chatKey] = runInfo;
    } else {
      delete nextRef[chatKey];
    }
    activeRunInfoByChatKeyRef.current = nextRef;
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

  function clearWorkspaceChatActiveRun(workspaceId: string, chatId: string) {
    setWorkspaces((current) => {
      let changed = false;
      const nextWorkspaces = current.map((workspace) => {
        if (workspace.id !== workspaceId) {
          return workspace;
        }

        let workspaceChanged = false;
        const nextChats = workspace.chats.map((chat) => {
          if (chat.id !== chatId || chat.activeRun === null) {
            return chat;
          }

          workspaceChanged = true;
          return { ...chat, activeRun: null };
        });

        if (!workspaceChanged) {
          return workspace;
        }

        changed = true;
        return { ...workspace, chats: nextChats };
      });

      return changed ? nextWorkspaces : current;
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

  function updateScheduledWorkspaceRuns(
    updater: (current: ScheduledWorkspaceRun[]) => ScheduledWorkspaceRun[],
  ) {
    const next = updater(scheduledWorkspaceRunsRef.current);
    scheduledWorkspaceRunsRef.current = next;
    setScheduledWorkspaceRuns(next);
  }

  function updateScheduledWorkspaceRunsByWorkspaceList(
    nextWorkspaces: WorkspaceSummary[],
  ) {
    updateScheduledWorkspaceRuns((current) => {
      const currentByChatKey = new Map(
        current.map((run) => [run.chatKey, run]),
      );
      const nextRuns = current.filter((run) =>
        nextWorkspaces.some(
          (workspace) =>
            workspace.id === run.workspaceId &&
            (!run.createdChatId ||
              !workspace.chats.some((chat) => chat.id === run.createdChatId)),
        ),
      );
      const nextRunChatKeys = new Set(nextRuns.map((run) => run.chatKey));

      for (const workspace of nextWorkspaces) {
        for (const chat of workspace.chats) {
          if (chat.queuedRun?.status !== "queued") {
            continue;
          }

          const chatKey = chatRunKey(workspace.id, chat.id);
          if (
            nextRunChatKeys.has(chatKey) ||
            !chat.queuedRun.modelId ||
            !chat.queuedRun.providerId ||
            !chat.queuedRun.content
          ) {
            continue;
          }

          const queuedRequest: RetryRunRequest = {
            workspaceId: workspace.id,
            chatId: chat.id,
            content: chat.queuedRun.content,
            attachments: [],
            modelId: chat.queuedRun.modelId,
            providerId: chat.queuedRun.providerId,
            thinkingLevel: chat.queuedRun.thinkingLevel ?? "",
            skillIds: chat.queuedRun.skillIds,
            localChatKey: chatKey,
            pendingUserMessageId: chat.queuedRun.userMessageId,
            queuedUserMessageId: chat.queuedRun.userMessageId,
            assistantMessageId: chat.queuedRun.assistantMessageId ?? undefined,
          };
          const existingRun = currentByChatKey.get(chatKey);
          const scheduledRun: ScheduledWorkspaceRun = existingRun
            ? { ...existingRun, request: queuedRequest, status: "queued" }
            : {
              id: chat.id,
              workspaceId: workspace.id,
              chatId: chat.id,
              chatKey,
              title: chat.title,
              createdAt: chat.createdAt,
              pendingUserMessageId: chat.queuedRun.userMessageId,
              request: queuedRequest,
              status: "queued",
            };

          nextRuns.push(scheduledRun);
          nextRunChatKeys.add(chatKey);
        }
      }

      return nextRuns;
    });
  }
  function restoreQueuedRunRequestsForChatKey(
    workspaceId: string,
    chatId: string,
    chatMessages: ShellMessage[],
  ) {
    const chatKey = chatRunKey(workspaceId, chatId);
    const queuedRequests = chatMessages
      .filter(
        (message) =>
          message.role === "user" &&
          message.pendingMode === "queued" &&
          message.queuedRun?.status === "queued",
      )
      .map((message) => ({
        workspaceId,
        chatId,
        content: message.content,
        attachments: [],
        modelId: message.queuedRun?.modelId ?? "",
        providerId: message.queuedRun?.providerId ?? "",
        thinkingLevel: message.queuedRun?.thinkingLevel ?? "",
        skillIds: message.queuedRun?.skillIds ?? [],
        localChatKey: chatKey,
        pendingUserMessageId: message.id,
        queuedUserMessageId: message.id,
        assistantMessageId: message.queuedRun?.assistantMessageId ?? undefined,
      }))
      .filter(
        (request) => request.modelId.trim() && request.providerId.trim(),
      );

    updateQueuedRunRequestsForChatKey(chatKey, () => queuedRequests);
    const [queuedRequest] = queuedRequests;
    if (!queuedRequest) {
      return;
    }

    const workspaceChat = workspaces
      .find((workspace) => workspace.id === workspaceId)
      ?.chats.find((chat) => chat.id === chatId);
    if (!workspaceChat) {
      return;
    }

    updateScheduledWorkspaceRuns((current) => {
      if (current.some((run) => run.chatKey === chatKey)) {
        return current;
      }

      return [
        ...current,
        {
          id: chatId,
          workspaceId,
          chatId,
          chatKey,
          title: workspaceChat.title,
          createdAt: workspaceChat.createdAt,
          pendingUserMessageId:
            queuedRequest.pendingUserMessageId ?? queuedRequest.queuedUserMessageId,
          request: queuedRequest,
          status: "queued",
        },
      ];
    });
  }

  function compareWorkspaceChatListItemsByCreatedAtDesc(
    left: WorkspaceChatListItem,
    right: WorkspaceChatListItem,
  ) {
    return Date.parse(right.createdAt) - Date.parse(left.createdAt);
  }

  function scheduledWorkspaceRunsFor(workspaceId: string) {
    return scheduledWorkspaceRuns.filter((run) => run.workspaceId === workspaceId);
  }

  function setActiveWorkspaceChatRefs(workspaceId: string, chatId: string | null) {
    activeWorkspaceIdRef.current = workspaceId;
    activeChatIdRef.current = chatId;
    activeChatKeyRef.current = chatId ? chatRunKey(workspaceId, chatId) : null;
  }

  function workspaceHasRunningOrStartingRun(workspaceId: string) {
    return (
      [...runningChatKeys].some(
        (chatKey) => chatKeyWorkspaceId(chatKey) === workspaceId,
      ) ||
      workspaces.some(
        (workspace) =>
          workspace.id === workspaceId &&
          workspace.chats.some((chat) => Boolean(chat.activeRun)),
      ) ||
      scheduledWorkspaceRunsRef.current.some(
        (run) => run.workspaceId === workspaceId && run.status === "starting",
      )
    );
  }
  async function refreshMessagesAfterSpecJobSettles(
    workspaceId: string,
    chatId: string,
    runId: string | null,
  ) {
    if (!runId) {
      return;
    }

    const attempts = [1000, 2000, 4000, 8000, 15000, 30000, 45000, 60000];
    try {
      for (const delayMs of attempts) {
        await delay(delayMs);
        const jobsResponse = await requestJson<WorkspaceSpecJobsResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs?limit=24`,
        );
        const job = jobsResponse.jobs.find(
          (candidate) =>
            candidate.chatId === chatId &&
            candidate.runId === runId &&
            candidate.triggerType === "chat_completed",
        );
        if (!job) {
          continue;
        }
        if (job.status === "queued" || job.status === "running") {
          continue;
        }
        if (job.status === "completed") {
          await loadChatMessages(workspaceId, chatId);
          if (activeWorkspaceIdRef.current === workspaceId) {
            await loadWorkspaceSpec(workspaceId);
          }
        }
        return;
      }
    } catch {
      return;
    }
  }

  function delay(durationMs: number) {
    return new Promise<void>((resolve) => {
      window.setTimeout(resolve, durationMs);
    });
  }

  function selectScheduledWorkspaceRun(run: ScheduledWorkspaceRun) {
    const cachedMessages = chatMessagesByKey[run.chatKey] ?? [];
    setActiveWorkspaceId(run.workspaceId);
    setActiveChatId(run.chatId);
    setActiveMainTab({ chatId: run.chatId, type: "chat", workspaceId: run.workspaceId });
    setExpandedWorkspaceId(run.workspaceId);
    activeWorkspaceIdRef.current = run.workspaceId;
    activeChatIdRef.current = run.chatId;
    activeChatKeyRef.current = run.chatKey;
    openChatTab(run.workspaceId, run.chatId);
    setMessages(cachedMessages);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({
      chatId: null,
      viewMode: "chat",
      workspaceId: run.workspaceId,
    });
  }

  async function loadChatMessages(
    workspaceId: string,
    chatId: string,
  ) {
    setError(null);
    const chatKey = chatRunKey(workspaceId, chatId);
    const existingController = loadingChatControllersRef.current.get(chatKey);
    if (existingController && !existingController.signal.aborted) {
      return;
    }
    loadingChatKeysRef.current.add(chatKey);
    const controller = new AbortController();
    loadingChatControllersRef.current.set(chatKey, controller);
    setLoadingChatKeys((current) => new Set(current).add(chatKey));

    try {
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages`,
        { signal: controller.signal },
      );
      const nextMessages = data.messages.map(normalizeChatMessageSummary);
      const activeRun = normalizeActiveChatRunSummary(data.activeRun);
      updateOpenChatTabTitle(workspaceId, chatId, data.chat?.title ?? null);
      setReadOnlyChatKeys((current) => {
        const readOnly = data.chat?.readOnly === true;
        if ((current[chatKey] === true) === readOnly && (readOnly || !(chatKey in current))) {
          return current;
        }
        const next = { ...current };
        if (readOnly) {
          next[chatKey] = true;
        } else {
          delete next[chatKey];
        }
        return next;
      });
      setChatMessagesByKey((current) => ({ ...current, [chatKey]: nextMessages }));
      restoreQueuedRunRequestsForChatKey(workspaceId, chatId, nextMessages);
      if (activeChatKeyRef.current === chatKey) {
        setMessages(nextMessages);
      }
      if (activeRun) {
        void subscribeActiveChatRun(activeRun);
      } else {
        setChatRunning(chatKey, false);
        setActiveRunInfoForChatKey(chatKey, null);
        clearWorkspaceChatActiveRun(workspaceId, chatId);
      }
    } catch (requestError) {
      if (activeChatKeyRef.current === chatKey) {
        setError(errorMessage(requestError));
      }
    } finally {
      if (loadingChatControllersRef.current.get(chatKey) === controller) {
        loadingChatControllersRef.current.delete(chatKey);
        loadingChatKeysRef.current.delete(chatKey);
        setLoadingChatKeys((current) => {
          const next = new Set(current);
          next.delete(chatKey);
          return next;
        });
      }
    }
  }

  function selectWorkspaceChat(
    workspaceId: string,
    chatId: string,
    options: { updateUrl?: boolean } = {},
  ) {
    if (isPendingChatId(chatId)) {
      const scheduledRun = scheduledWorkspaceRuns.find(
        (run) => run.workspaceId === workspaceId && run.chatId === chatId,
      );
      if (scheduledRun) {
        selectScheduledWorkspaceRun(scheduledRun);
        return;
      }
      const chatKey = chatRunKey(workspaceId, chatId);
      const cachedMessages = chatMessagesByKey[chatKey] ?? [];
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setActiveMainTab({ chatId, type: "chat", workspaceId });
      setExpandedWorkspaceId(workspaceId);
      setActiveWorkspaceChatRefs(workspaceId, chatId);
      setMessages(cachedMessages);
      setSelectedDiffPath(null);
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      if (options.updateUrl !== false) {
        updateBrowserRoute({ chatId: null, viewMode: "chat", workspaceId });
      }
      return;
    }

    const chatKey = chatRunKey(workspaceId, chatId);
    for (const [loadingChatKey, controller] of loadingChatControllersRef.current) {
      if (loadingChatKey !== chatKey) {
        controller.abort();
      }
    }
    const cachedMessages = chatMessagesByKey[chatKey];

    if (!cachedMessages) {
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setActiveMainTab({ chatId, type: "chat", workspaceId });
      setExpandedWorkspaceId(workspaceId);
      openChatTab(workspaceId, chatId);
      setActiveWorkspaceChatRefs(workspaceId, chatId);
      setMessages([]);
      setSelectedDiffPath(null);
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      if (options.updateUrl !== false) {
        updateBrowserRoute({ chatId, viewMode: "chat", workspaceId });
      }
      void loadChatMessages(workspaceId, chatId);
      return;
    }

    setActiveWorkspaceId(workspaceId);
    setActiveChatId(chatId);
    setActiveMainTab({ chatId, type: "chat", workspaceId });
    setExpandedWorkspaceId(workspaceId);
    openChatTab(workspaceId, chatId);
    setActiveWorkspaceChatRefs(workspaceId, chatId);
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
    resetComposerDefaultsForNewChat();
    setExpandedWorkspaceId(workspaceId);
    setActiveWorkspaceChatRefs(workspaceId, null);
    setActiveWorkspaceId(workspaceId);
    setActiveChatId(null);
    setActiveMainTab({ chatId: null, type: "chat", workspaceId });
    setIsTeamModeEnabled(settings?.general.defaultTeamModeEnabled ?? false);
    setMessages([]);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      updateBrowserRoute({ chatId: null, viewMode: "chat", workspaceId });
    }
  }

  function resetComposerDefaultsForNewChat() {
    hasManuallySelectedModelRef.current = false;
    hasManuallySelectedThinkingLevelRef.current = false;
    setSelectedModelId(defaultComposerSelection.modelId);
    setSelectedProviderId(defaultComposerSelection.providerId);
    setSelectedThinkingLevel(defaultComposerSelection.thinkingLevel);
  }

  function openChatTab(workspaceId: string, chatId: string) {
    const workspace = workspaces.find((workspace) => workspace.id === workspaceId);
    const chat = workspace?.chats.find((chat) => chat.id === chatId);
    const nextTabs = upsertOpenChatTab(openChatTabsRef.current, {
      workspaceId,
      chatId,
      fallbackTitle: chat?.title ?? t("Chat"),
      fallbackWorkspaceName: workspace?.name ?? t("Workspace"),
    });

    openChatTabsRef.current = nextTabs;
    setOpenChatTabs(nextTabs);
  }

  function updateOpenChatTabTitle(
    workspaceId: string,
    chatId: string,
    title: string | null,
  ) {
    const fallbackTitle = title?.trim();
    if (!fallbackTitle) {
      return;
    }

    setOpenChatTabs((current) => {
      let changed = false;
      const nextTabs = current.map((tab) => {
        if (tab.workspaceId !== workspaceId || tab.chatId !== chatId) {
          return tab;
        }
        if (tab.fallbackTitle === fallbackTitle) {
          return tab;
        }
        changed = true;
        return { ...tab, fallbackTitle };
      });
      if (!changed) {
        return current;
      }
      openChatTabsRef.current = nextTabs;
      return nextTabs;
    });
  }

  function restoreWorkspaceChatTabs(tabs: BrowserRouteChatTab[]) {
    const nextTabs = tabs.flatMap((tab) => {
      const workspace = workspaces.find((workspace) => workspace.id === tab.workspaceId);
      const chat = workspace?.chats.find((chat) => chat.id === tab.chatId);
      if (!workspace || !chat) {
        return [];
      }

      return [{
        chatId: tab.chatId,
        fallbackTitle: chat.title,
        fallbackWorkspaceName: workspace.name,
        workspaceId: tab.workspaceId,
      } satisfies OpenChatTab];
    });

    openChatTabsRef.current = nextTabs;
    setOpenChatTabs(nextTabs);
  }

  function selectAgentTab(tab: OpenAgentTab) {
    const chatKey = chatRunKey(tab.workspaceId, tab.chatId);
    const cachedMessages = chatMessagesByKey[chatKey];

    setActiveWorkspaceId(tab.workspaceId);
    setActiveChatId(tab.chatId);
    setActiveMainTab({
      chatId: tab.chatId,
      instanceId: tab.instanceId,
      teamId: tab.teamId,
      type: "agent",
      workspaceId: tab.workspaceId,
    });
    setExpandedWorkspaceId(tab.workspaceId);
    setActiveWorkspaceChatRefs(tab.workspaceId, tab.chatId);
    setMessages(cachedMessages ?? []);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({
      chatId: tab.chatId,
      viewMode: "chat",
      workspaceId: tab.workspaceId,
    });

    if (!cachedMessages) {
      void loadChatMessages(tab.workspaceId, tab.chatId);
    }
  }

  function openAgentInstanceTab(instance: AgentInstanceView) {
    if (!agentTeamSnapshot || !activeWorkspaceId || !activeChatId) {
      return;
    }

    if (instance.id === agentTeamSnapshot.team.coordinatorInstanceId) {
      selectWorkspaceChat(activeWorkspaceId, activeChatId);
      return;
    }

    const workspace = workspaces.find((workspace) => workspace.id === activeWorkspaceId);
    const nextTab: OpenAgentTab = {
      chatId: activeChatId,
      fallbackTitle: instance.definitionSnapshot.name,
      fallbackWorkspaceName: workspace?.name ?? t("Workspace"),
      instanceId: instance.id,
      teamId: agentTeamSnapshot.team.id,
      workspaceId: activeWorkspaceId,
    };

    setOpenAgentTabs((current) => upsertOpenAgentTab(current, nextTab));
    selectAgentTab(nextTab);
  }

  async function openWorkspaceFileTab(node: WorkspaceFileTreeNode) {
    if (!activeWorkspace) {
      setWorkspaceFilesError(t("Select a workspace before using file actions."));
      return;
    }
    if (node.kind !== "file" || !node.path) {
      return;
    }

    const file: OpenFileTab = {
      name: node.name,
      path: node.path,
      workspaceId: activeWorkspace.id,
      workspaceLogoUrl: activeWorkspace.logoUrl ?? null,
      workspaceName: activeWorkspace.name,
    };

    selectWorkspaceFileTab(file);
    await loadWorkspaceFileEditor(file);
  }

  function restoreWorkspaceFileTabs(
    files: BrowserRouteFileTab[],
    activeFile: BrowserRouteFileTab | null,
  ) {
    const nextTabs = files.flatMap((file) => {
      const workspace = workspaces.find((workspace) => workspace.id === file.workspaceId);
      if (!workspace) {
        return [];
      }

      return [browserRouteFileTabToOpenFileTab(file, workspace)];
    });

    openFileTabsRef.current = nextTabs;
    setOpenFileTabs(nextTabs);

    const selectedFile = activeFile
      ? nextTabs.find(
        (tab) =>
          tab.workspaceId === activeFile.workspaceId && tab.path === activeFile.path,
      ) ?? null
      : null;
    if (!selectedFile) {
      return false;
    }

    selectWorkspaceFileTab(selectedFile, { updateUrl: false });
    void loadWorkspaceFileEditor(selectedFile);
    return true;
  }

  function selectWorkspaceFileTab(
    file: OpenFileTab,
    options: { updateUrl?: boolean } = {},
  ) {
    const nextTabs = upsertOpenFileTab(openFileTabsRef.current, file);
    openFileTabsRef.current = nextTabs;
    setOpenFileTabs(nextTabs);
    setActiveWorkspaceId(file.workspaceId);
    setExpandedWorkspaceId(file.workspaceId);
    setActiveMainTab({ path: file.path, type: "file", workspaceId: file.workspaceId });
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    initWorkspaceFileEditor(file.workspaceId, file.path);
    if (options.updateUrl !== false) {
      updateBrowserRoute(browserRouteForActiveFile(file));
    }
  }

  function initWorkspaceFileEditor(workspaceId: string, path: string) {
    const editorKey = workspaceFileEditorKey(workspaceId, path);
    setWorkspaceFileEditors((current) => ({
      ...current,
      [editorKey]: current[editorKey] ?? {
        content: "",
        error: null,
        isDirty: false,
        isLoading: true,
        isSaving: false,
        lastSavedContent: "",
      },
    }));
  }

  async function loadWorkspaceFileEditor(file: OpenFileTab) {
    const editorKey = workspaceFileEditorKey(file.workspaceId, file.path);

    try {
      const response = await requestJson<WorkspaceFileContentResponse>(
        `/api/workspaces/${encodeURIComponent(file.workspaceId)}/files/content`,
        {
          body: JSON.stringify({ path: file.path }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setWorkspaceFileEditors((current) => ({
        ...current,
        [editorKey]: {
          content: response.content,
          error: null,
          isDirty: false,
          isLoading: false,
          isSaving: false,
          lastSavedContent: response.content,
        },
      }));
    } catch (requestError) {
      setWorkspaceFileEditors((current) => ({
        ...current,
        [editorKey]: {
          content: current[editorKey]?.content ?? "",
          error: errorMessage(requestError),
          isDirty: current[editorKey]?.isDirty ?? false,
          isLoading: false,
          isSaving: false,
          lastSavedContent: current[editorKey]?.lastSavedContent ?? "",
        },
      }));
    }
  }

  function browserRouteForActiveFile(file: OpenFileTab): BrowserRoute {
    return {
      activeFile: { path: file.path, workspaceId: file.workspaceId },
      chatId: activeWorkspaceIdRef.current === file.workspaceId
        ? activeChatIdRef.current
        : null,
      viewMode: "chat",
      workspaceId: file.workspaceId,
    };
  }

  const reloadWorkspaceFileEditor = useCallback(async (file: OpenFileTab) => {
    const editorKey = workspaceFileEditorKey(file.workspaceId, file.path);
    setWorkspaceFileEditors((current) => {
      const editor = current[editorKey];
      if (!editor) {
        return current;
      }

      return {
        ...current,
        [editorKey]: {
          ...editor,
          error: null,
          isLoading: true,
        },
      };
    });

    try {
      const response = await requestJson<WorkspaceFileContentResponse>(
        `/api/workspaces/${encodeURIComponent(file.workspaceId)}/files/content`,
        {
          body: JSON.stringify({ path: file.path }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setWorkspaceFileEditors((current) => ({
        ...current,
        [editorKey]: {
          content: response.content,
          error: null,
          isDirty: false,
          isLoading: false,
          isSaving: false,
          lastSavedContent: response.content,
        },
      }));
    } catch (requestError) {
      setWorkspaceFileEditors((current) => {
        const editor = current[editorKey];
        if (!editor) {
          return current;
        }

        return {
          ...current,
          [editorKey]: {
            ...editor,
            error: errorMessage(requestError),
            isLoading: false,
          },
        };
      });
    }
  }, []);

  const updateWorkspaceFileEditorContent = useCallback(
    (workspaceId: string, path: string, content: string) => {
      const editorKey = workspaceFileEditorKey(workspaceId, path);
      setWorkspaceFileEditors((current) => {
        const editor = current[editorKey];
        if (!editor || editor.content === content) {
          return current;
        }

        return {
          ...current,
          [editorKey]: {
            ...editor,
            content,
            isDirty: content !== editor.lastSavedContent,
          },
        };
      });
    },
    [],
  );

  const saveWorkspaceFileEditor = useCallback(
    async (file: OpenFileTab, content: string) => {
      const editorKey = workspaceFileEditorKey(file.workspaceId, file.path);
      setWorkspaceFileEditors((current) => {
        const editor = current[editorKey];
        if (!editor) {
          return current;
        }

        return {
          ...current,
          [editorKey]: {
            ...editor,
            content,
            error: null,
            isSaving: true,
          },
        };
      });

      try {
        const response = await requestJson<WorkspaceFileSaveResponse>(
          `/api/workspaces/${encodeURIComponent(file.workspaceId)}/files/save`,
          {
            body: JSON.stringify({ content, path: file.path }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        setWorkspaceFileEditors((current) => {
          const editor = current[editorKey];
          if (!editor) {
            return current;
          }

          return {
            ...current,
            [editorKey]: {
              ...editor,
              content: response.content,
              error: null,
              isDirty: false,
              isSaving: false,
              lastSavedContent: response.content,
            },
          };
        });
        return true;
      } catch (requestError) {
        setWorkspaceFileEditors((current) => {
          const editor = current[editorKey];
          if (!editor) {
            return current;
          }

          return {
            ...current,
            [editorKey]: {
              ...editor,
              error: errorMessage(requestError),
              isSaving: false,
            },
          };
        });
        return false;
      }
    },
    [],
  );

  function openPendingChatTab(
    workspaceId: string,
    chatId: string,
    fallbackTitle: string,
  ) {
    const workspace = workspaces.find((workspace) => workspace.id === workspaceId);
    const nextTabs = upsertOpenChatTab(openChatTabsRef.current, {
      workspaceId,
      chatId,
      fallbackTitle,
      fallbackWorkspaceName: workspace?.name ?? t("Workspace"),
    });

    openChatTabsRef.current = nextTabs;
    setOpenChatTabs(nextTabs);
  }

  function replacePendingChatTab(
    workspaceId: string,
    pendingChatId: string,
    chatId: string,
  ) {
    const workspace = workspaces.find((workspace) => workspace.id === workspaceId);
    const chat = workspace?.chats.find((chat) => chat.id === chatId);

    setOpenChatTabs((current) => {
      const pendingTab = current.find(
        (tab) => tab.workspaceId === workspaceId && tab.chatId === pendingChatId,
      );
      const nextTab: OpenChatTab = {
        workspaceId,
        chatId,
        fallbackTitle: chat?.title ?? pendingTab?.fallbackTitle ?? t("Chat"),
        fallbackWorkspaceName:
          workspace?.name ?? pendingTab?.fallbackWorkspaceName ?? t("Workspace"),
      };
      const withoutOldTabs = current.filter(
        (tab) =>
          tab.workspaceId !== workspaceId ||
          (tab.chatId !== pendingChatId && tab.chatId !== chatId),
      );
      const pendingIndex = current.findIndex(
        (tab) => tab.workspaceId === workspaceId && tab.chatId === pendingChatId,
      );

      if (pendingIndex < 0) {
        return upsertOpenChatTab(withoutOldTabs, nextTab);
      }

      const insertIndex = Math.min(pendingIndex, withoutOldTabs.length);
      return [
        ...withoutOldTabs.slice(0, insertIndex),
        nextTab,
        ...withoutOldTabs.slice(insertIndex),
      ];
    });
  }

  function selectMainTab(tab: MainTabSummary) {
    if (tab.type === "chat") {
      selectWorkspaceChat(tab.workspaceId, tab.chatId);
      return;
    }

    if (tab.type === "agent") {
      selectAgentTab(tab);
      return;
    }

    selectWorkspaceFileTab(tab);
    const editorKey = workspaceFileEditorKey(tab.workspaceId, tab.path);
    if (!workspaceFileEditors[editorKey]) {
      void loadWorkspaceFileEditor(tab);
    }
  }

  function closeMainTab(tab: MainTabSummary) {
    if (tab.type === "chat") {
      closeChatTab(tab.workspaceId, tab.chatId);
      return;
    }

    if (tab.type === "agent") {
      closeAgentTab(tab);
      return;
    }

    const tabIndex = mainTabs.findIndex(
      (current) => current.type === "file" && current.workspaceId === tab.workspaceId && current.path === tab.path,
    );
    const nextOpenFileTabs = openFileTabsRef.current.filter(
      (current) => current.workspaceId !== tab.workspaceId || current.path !== tab.path,
    );
    openFileTabsRef.current = nextOpenFileTabs;
    setOpenFileTabs(nextOpenFileTabs);
    setWorkspaceFileEditors((current) => {
      const next = { ...current };
      delete next[workspaceFileEditorKey(tab.workspaceId, tab.path)];
      return next;
    });

    if (
      activeMainTab.type !== "file" ||
      activeMainTab.workspaceId !== tab.workspaceId ||
      activeMainTab.path !== tab.path
    ) {
      if (activeMainTab.type === "file" && activeFileTab) {
        updateBrowserRoute(browserRouteForActiveFile(activeFileTab), "replace");
      } else {
        updateBrowserRoute({
          chatId: activeChatId,
          viewMode: "chat",
          workspaceId: activeWorkspaceId || tab.workspaceId,
        }, "replace");
      }
      return;
    }

    const nextTabs = mainTabs.filter(
      (current) => !(current.type === "file" && current.workspaceId === tab.workspaceId && current.path === tab.path),
    );
    const nextTab = nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveMainTab({ chatId: null, type: "chat", workspaceId: activeWorkspaceId || tab.workspaceId });
    updateBrowserRoute({
      chatId: activeChatId,
      viewMode: "chat",
      workspaceId: activeWorkspaceId || tab.workspaceId,
    }, "replace");
  }

  function closeAgentTab(tab: OpenAgentTab) {
    const tabIndex = mainTabs.findIndex(
      (current) =>
        current.type === "agent" &&
        current.workspaceId === tab.workspaceId &&
        current.chatId === tab.chatId &&
        current.instanceId === tab.instanceId,
    );
    setOpenAgentTabs((current) =>
      current.filter(
        (current) =>
          current.workspaceId !== tab.workspaceId ||
          current.chatId !== tab.chatId ||
          current.instanceId !== tab.instanceId,
      ),
    );

    if (
      activeMainTab.type !== "agent" ||
      activeMainTab.workspaceId !== tab.workspaceId ||
      activeMainTab.chatId !== tab.chatId ||
      activeMainTab.instanceId !== tab.instanceId
    ) {
      return;
    }

    const nextTabs = mainTabs.filter(
      (current) =>
        !(
          current.type === "agent" &&
          current.workspaceId === tab.workspaceId &&
          current.chatId === tab.chatId &&
          current.instanceId === tab.instanceId
        ),
    );
    const nextTab = nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveMainTab({ chatId: tab.chatId, type: "chat", workspaceId: tab.workspaceId });
  }

  function closeChatTab(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);
    if (runningChatKeys.has(chatKey)) {
      return;
    }

    const tabIndex = mainTabs.findIndex(
      (tab) => tab.type === "chat" && tab.workspaceId === workspaceId && tab.chatId === chatId,
    );
    const nextOpenChatTabs = openChatTabsRef.current.filter(
      (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
    );
    openChatTabsRef.current = nextOpenChatTabs;
    setOpenChatTabs(nextOpenChatTabs);
    setChatRunFailed(chatKey, false);
    removeMessagesForChatKey(chatKey);
    removeContextUsageForChatKey(chatKey);

    if (
      activeMainTab.type !== "chat" ||
      activeMainTab.workspaceId !== workspaceId ||
      activeMainTab.chatId !== chatId
    ) {
      updateBrowserRoute({
        chatId: activeChatId,
        tabs: openChatTabsToBrowserRouteTabs(nextOpenChatTabs),
        viewMode: "chat",
        workspaceId: activeWorkspaceId || workspaceId,
      }, "replace");
      return;
    }

    const nextTabs = mainTabs.filter(
      (tab) => !(tab.type === "chat" && tab.workspaceId === workspaceId && tab.chatId === chatId),
    );
    const nextTab = nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);

    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveWorkspaceChatRefs(activeWorkspaceId || workspaceId, null);
    setActiveChatId(null);
    setMessages([]);
    setActiveMainTab({ chatId: null, type: "chat", workspaceId: activeWorkspaceId || workspaceId });
    updateBrowserRoute({
      chatId: null,
      viewMode: "chat",
      workspaceId: activeWorkspaceId || workspaceId,
    });
  }

  function openWorkspaceChatContextMenu(
    event: Pick<ReactMouseEvent<HTMLElement> | ReactPointerEvent<HTMLElement>, "clientX" | "clientY" | "preventDefault" | "stopPropagation">,
    workspace: WorkspaceSummary,
    chat: WorkspaceChatListItem,
  ) {
    event.preventDefault();
    event.stopPropagation();
    setWorkspaceChatContextMenu({
      chat,
      left: event.clientX,
      top: event.clientY,
      workspace,
    });
  }

  function cancelWorkspaceChatLongPress() {
    if (workspaceChatLongPressTimeoutRef.current === null) {
      return;
    }

    window.clearTimeout(workspaceChatLongPressTimeoutRef.current);
    workspaceChatLongPressTimeoutRef.current = null;
  }

  function startWorkspaceChatLongPress(
    event: ReactPointerEvent<HTMLButtonElement>,
    workspace: WorkspaceSummary,
    chat: WorkspaceChatListItem,
  ) {
    cancelWorkspaceChatLongPress();

    if (
      event.pointerType === "mouse" ||
      typeof window === "undefined" ||
      window.innerWidth >= MOBILE_BREAKPOINT_PX
    ) {
      return;
    }

    const { clientX, clientY } = event;
    workspaceChatLongPressTimeoutRef.current = window.setTimeout(() => {
      workspaceChatLongPressTimeoutRef.current = null;
      suppressNextWorkspaceChatClickRef.current = true;
      setWorkspaceChatContextMenu({
        chat,
        left: clientX,
        top: clientY,
        workspace,
      });
    }, WORKSPACE_CHAT_CONTEXT_MENU_LONG_PRESS_MS);
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
        setActiveWorkspaceChatRefs(workspaceId, null);
        setActiveWorkspaceId(workspaceId);
        setActiveChatId(null);
        setMessages([]);
        updateBrowserRoute({
          chatId: null,
          viewMode: "chat",
          workspaceId,
        });
      }

      removeMessagesForChatKey(chatRunKey(workspaceId, chatId));
      removeContextUsageForChatKey(chatRunKey(workspaceId, chatId));
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
  function renameWorkspaceFileTab(workspaceId: string, path: string, newName: string) {
    setOpenFileTabs((current) =>
      current.map((tab) => {
        if (tab.workspaceId !== workspaceId || tab.path !== path) {
          return tab;
        }
        const nextPath = workspaceRenamedFilePath(path, newName);
        return {
          ...tab,
          name: newName,
          path: nextPath,
        };
      }),
    );
    setWorkspaceFileEditors((current) => {
      const oldKey = workspaceFileEditorKey(workspaceId, path);
      const nextPath = workspaceRenamedFilePath(path, newName);
      const newKey = workspaceFileEditorKey(workspaceId, nextPath);
      if (!(oldKey in current)) {
        return current;
      }
      const next = { ...current, [newKey]: current[oldKey] };
      delete next[oldKey];
      return next;
    });
    setActiveMainTab((current) =>
      current.type === "file" && current.workspaceId === workspaceId && current.path === path
        ? { path: workspaceRenamedFilePath(path, newName), type: "file", workspaceId }
        : current,
    );
  }

  function closeWorkspaceFileTabsForPath(workspaceId: string, path: string) {
    setOpenFileTabs((current) =>
      current.filter(
        (tab) =>
          tab.workspaceId !== workspaceId ||
          (tab.path !== path && !tab.path.startsWith(`${path}/`)),
      ),
    );
    setWorkspaceFileEditors((current) => {
      const next = { ...current };
      for (const key of Object.keys(next)) {
        const prefix = `${workspaceId}:`;
        if (!key.startsWith(prefix)) {
          continue;
        }
        const filePath = key.slice(prefix.length);
        if (filePath === path || filePath.startsWith(`${path}/`)) {
          delete next[key];
        }
      }
      return next;
    });
    if (
      activeMainTab.type === "file" &&
      activeMainTab.workspaceId === workspaceId &&
      (activeMainTab.path === path || activeMainTab.path.startsWith(`${path}/`))
    ) {
      setActiveMainTab({ chatId: activeChatId, type: "chat", workspaceId });
    }
  }

  async function handleWorkspaceFileOperation(
    action: "delete" | "rename",
    path: string,
    newName?: string,
  ) {
    if (!activeWorkspace) {
      setWorkspaceFilesError(t("Select a workspace before using file actions."));
      return;
    }

    const operationKey = `${action}:${path}`;
    setWorkspaceFileOperationKey(operationKey);
    setWorkspaceFilesError(null);

    try {
      const data = await requestJson<WorkspaceFileChildrenResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/files/${action}`,
        {
          body: JSON.stringify(action === "rename" ? { path, newName } : { path }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      if (action === "delete") {
        closeWorkspaceFileTabsForPath(activeWorkspace.id, path);
        setExpandedFileTreePaths((current) => {
          const next = new Set([...current, ""]);
          for (const expandedPath of current) {
            if (expandedPath === path || expandedPath.startsWith(`${path}/`)) {
              next.delete(expandedPath);
            }
          }
          return next;
        });
      }
      if (action === "rename" && newName) {
        renameWorkspaceFileTab(activeWorkspace.id, path, newName);
      }
      setWorkspaceFiles((current) =>
        current
          ? {
              ...current,
              root: replaceWorkspaceFileNodeChildren(current.root, data.path, data.children),
            }
          : current,
      );
      if (isContextPanelOpen && contextPanelTab === "git") {
        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
      }
    } catch (requestError) {
      setWorkspaceFilesError(errorMessage(requestError));
    } finally {
      setWorkspaceFileOperationKey(null);
    }
  }

  async function toggleWorkspaceFileTreePath(node: WorkspaceFileTreeNode) {
    const isExpanded = expandedFileTreePaths.has(node.path);
    if (isExpanded) {
      setExpandedFileTreePaths((current) => {
        const next = new Set(current);
        next.delete(node.path);
        return next;
      });
      return;
    }

    if (
      activeWorkspace?.id &&
      node.kind === "directory" &&
      node.hasChildren &&
      !node.childrenLoaded
    ) {
      const loaded = await loadWorkspaceDirectoryChildren(activeWorkspace.id, node.path);
      if (!loaded) {
        return;
      }
    }

    setExpandedFileTreePaths((current) => new Set([...current, node.path]));
  }

  async function copyWorkspaceFileText(text: string) {
    setWorkspaceFilesError(null);
    try {
      await navigator.clipboard.writeText(text);
    } catch (copyError) {
      setWorkspaceFilesError(errorMessage(copyError));
    }
  }

  function workspaceFileAbsolutePath(workspacePath: string, relativePath: string) {
    const separator = workspacePath.includes("\\") ? "\\" : "/";
    const root = workspacePath.replace(/[\\/]+$/, "");
    const normalizedRelativePath = relativePath.replace(/[\\/]+/g, separator);
    return root ? `${root}${separator}${normalizedRelativePath}` : `${separator}${normalizedRelativePath}`;
  }

  async function handleGitFileOperation(
    action: "stage" | "unstage" | "discard",
    path: string,
  ) {
    if (!activeWorkspace) {
      setDiffError(t("Select a workspace before using Git actions."));
      return;
    }

    const operationKey = `${action}:${path}`;
    setGitOperationKey(operationKey);
    setDiffError(null);

    try {
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/${action}`,
        {
          body: JSON.stringify({ path }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitDiff(data);
      setSelectedDiffPath(
        selectedDiffPath && data.files.some((file) => file.path === selectedDiffPath)
          ? selectedDiffPath
          : null,
      );
    } catch (requestError) {
      setDiffError(errorMessage(requestError));
    } finally {
      setGitOperationKey(null);
    }
  }

  async function handleGitCommit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!activeWorkspace) {
      setDiffError(t("Select a workspace before committing changes."));
      return;
    }

    const message = gitCommitMessage.trim();
    if (!message) {
      setDiffError(t("Commit message must not be empty."));
      return;
    }

    setGitOperationKey("commit");
    setDiffError(null);

    try {
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/commit`,
        {
          body: JSON.stringify({ message }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitDiff(data);
      setGitCommitMessage("");
      setSelectedDiffPath(null);
    } catch (requestError) {
      setDiffError(errorMessage(requestError));
    } finally {
      setGitOperationKey(null);
    }
  }

  async function handleGenerateGitCommitMessage() {
    if (!activeWorkspace) {
      setDiffError(t("Select a workspace before using Git actions."));
      return;
    }
    if (!gitDiff?.stagedFiles.length) {
      return;
    }
    if (!selectedModelId || !selectedProviderId) {
      setDiffError(t("Select an enabled model before generating a commit message."));
      return;
    }

    setGitOperationKey("generate-commit-message");
    setDiffError(null);

    try {
      const data = await requestJson<GitCommitMessageResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/commit-message`,
        {
          body: JSON.stringify({
            modelId: selectedModelId,
            providerId: selectedProviderId,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitCommitMessage(data.message);
    } catch (requestError) {
      setDiffError(errorMessage(requestError));
    } finally {
      setGitOperationKey(null);
    }
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

  async function handleSelectDraftAttachments(files: File[]) {
    if (!files.length) {
      return;
    }

    setIsSelectingAttachments(true);
    setError(null);

    try {
      const attachments = await Promise.all(files.map(fileToComposerAttachment));
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

    const currentWorkspaceId = activeWorkspaceIdRef.current || activeWorkspace?.id || "";
    const currentWorkspace =
      workspaces.find((workspace) => workspace.id === currentWorkspaceId) ??
      activeWorkspace;
    const currentChatId = activeChatIdRef.current;

    if (!currentWorkspace) {
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
      chatId:
        currentChatId && !isPendingChatId(currentChatId)
          ? currentChatId
          : null,
      content,
      modelId: selectedModelId,
      providerId: selectedProviderId,
      skillIds,
      teamModeEnabled: canUseTeamMode && isTeamModeEnabled,
      thinkingLevel: selectedThinkingLevel,
      workspaceId: currentWorkspace.id,
    };
  }

  function activeRunForRequest(request: RetryRunRequest): ActiveRunInfo | null {
    if (!request.chatId) {
      return null;
    }

    const currentWorkspaceId = activeWorkspaceIdRef.current;
    const currentChatId = activeChatIdRef.current;
    if (
      currentWorkspaceId !== request.workspaceId ||
      currentChatId !== request.chatId
    ) {
      return null;
    }

    const currentChatKey = activeChatKeyRef.current;
    const requestChatKey = chatRunKey(request.workspaceId, request.chatId);
    if (currentChatKey !== requestChatKey) {
      return null;
    }

    const runInfo = activeRunInfoByChatKeyRef.current[requestChatKey] ?? null;
    if (
      !runInfo ||
      runInfo.chatKey !== requestChatKey ||
      runInfo.workspaceId !== request.workspaceId ||
      runInfo.chatId !== request.chatId ||
      !runInfo.runId ||
      !runInfo.acceptingGuidance ||
      !runningChatKeysRef.current.has(requestChatKey)
    ) {
      return null;
    }

    return runInfo;
  }

  async function persistQueuedRunRequest(
    request: RetryRunRequest,
    options: { deferStart?: boolean } = {},
  ): Promise<QueueChatMessageResponse> {
    return requestJson<QueueChatMessageResponse>(
      `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/queue`,
      {
        body: JSON.stringify({
          chatId: request.chatId,
          message: request.content,
          attachments: request.attachments,
          modelId: request.modelId,
          providerId: request.providerId,
          skillIds: request.skillIds.length ? request.skillIds : null,
          teamModeEnabled: request.teamModeEnabled ?? false,
          deferStart: options.deferStart ?? false,
          thinkingLevel: request.thinkingLevel || null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      },
    );
  }

  async function handleSendMessage(
    event: FormEvent<HTMLFormElement>,
    options: { schedule?: boolean } = {},
  ) {
    event.preventDefault();

    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }

    if (request.chatId && readOnlyChatKeys[chatRunKey(request.workspaceId, request.chatId)]) {
      setError(t("This transcript is read-only."));
      return;
    }

    const requestActiveRun = activeRunForRequest(request);
    if (requestActiveRun) {
      if (options.schedule) {
        await handleQueueActiveRunWithRequest(request, requestActiveRun);
        return;
      }

      await guideActiveRun(request, requestActiveRun);
      return;
    }

    if (options.schedule) {
      await handleScheduleMessage(request);
      return;
    }

    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");

    await runChatMessage(request);
  }

  async function handleScheduleMessage(request: RetryRunRequest) {
    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");
    setError(null);

    try {
      const queued = await persistQueuedRunRequest(request, { deferStart: true });
      const chatKey = chatRunKey(request.workspaceId, queued.chatId);
      const createdAt = queued.createdAt;

      setActiveWorkspaceId(request.workspaceId);
      setActiveChatId(queued.chatId);
      setActiveMainTab({
        chatId: queued.chatId,
        type: "chat",
        workspaceId: request.workspaceId,
      });
      openPendingChatTab(request.workspaceId, queued.chatId, queued.chatTitle);
      setExpandedWorkspaceId(request.workspaceId);
      activeWorkspaceIdRef.current = request.workspaceId;
      activeChatIdRef.current = queued.chatId;
      activeChatKeyRef.current = chatKey;
      setSelectedDiffPath(null);
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      updateBrowserRoute({
        chatId: queued.chatId,
        viewMode: "chat",
        workspaceId: request.workspaceId,
      });
      setMessagesForChatKey(chatKey, (current) => [
        ...current,
        {
          id: queued.userMessageId,
          role: "user",
          content: queued.content,
          createdAt,
          reasoning: null,
          pendingMode: "queued",
          queuedRun: null,
          toolCalls: [],
          parts: queued.parts,
          metrics: null,
          memoriesUsed: [],
          extractedMemories: [],
        specUpdates: [],
        },
      ]);

      const scheduledRun: ScheduledWorkspaceRun = {
        id: queued.chatId,
        workspaceId: request.workspaceId,
        chatId: queued.chatId,
        chatKey,
        title: queued.chatTitle,
        createdAt,
        pendingUserMessageId: queued.userMessageId,
        request: {
          ...request,
          chatId: queued.chatId,
          localChatKey: chatKey,
          pendingUserMessageId: queued.userMessageId,
          queuedUserMessageId: queued.userMessageId,
          assistantMessageId: queued.assistantMessageId,
        },
        status: "queued",
      };

      updateScheduledWorkspaceRuns((current) => [...current, scheduledRun]);
      void refreshWorkspaces();
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  async function handleGuideActiveRun() {
    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }

    const runInfo = activeRunForRequest(request);
    if (!runInfo) {
      setError(t("No active run is available for guidance."));
      return;
    }

    await guideActiveRun(request, runInfo);
  }

  async function handleQueueActiveRun() {
    const request = currentDraftRunRequest();
    if (!request) {
      return;
    }
    const runInfo = activeRunForRequest(request);
    if (!runInfo) {
      setError(t("No active run is available for guidance."));
      return;
    }

    await handleQueueActiveRunWithRequest(request, runInfo);
  }

  async function handleQueueActiveRunWithRequest(
    request: RetryRunRequest,
    runInfo: ActiveRunInfo,
  ) {
    setSelectedSkillIds([]);
    setDraftAttachments([]);
    setDraftMessage("");
    setError(null);

    try {
      const queued = await persistQueuedRunRequest({
        ...request,
        chatId: runInfo.chatId ?? request.chatId,
        workspaceId: runInfo.workspaceId ?? request.workspaceId,
      });
      setMessagesForChatKey(runInfo.chatKey, (current) => [
        ...current,
        {
          id: queued.userMessageId,
          role: "user",
          content: queued.content,
          createdAt: queued.createdAt,
          reasoning: null,
          pendingMode: "queued",
          toolCalls: [],
          parts: queued.parts,
          metrics: null,
          memoriesUsed: [],
          extractedMemories: [],
        specUpdates: [],
        },
      ]);

      const queuedRequest = {
        ...request,
        chatId: runInfo.chatId ?? request.chatId,
        pendingUserMessageId: queued.userMessageId,
        queuedUserMessageId: queued.userMessageId,
        assistantMessageId: queued.assistantMessageId,
        workspaceId: runInfo.workspaceId ?? request.workspaceId,
      };
      updateQueuedRunRequestsForChatKey(runInfo.chatKey, (current) => [
        ...current,
        queuedRequest,
      ]);
      void refreshWorkspaces();
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function handleWithdrawQueuedMessage(messageId: string) {
    const chatKey = activeChatKeyRef.current;
    if (!chatKey) {
      return;
    }

    const queuedRequests = queuedRunRequestsByChatKeyRef.current[chatKey] ?? [];
    if (
      !queuedRequests.some(
        (request) => request.pendingUserMessageId === messageId,
      )
    ) {
      setError(t("Queued message is no longer available."));
      return;
    }

    updateQueuedRunRequestsForChatKey(chatKey, (current) =>
      current.filter((request) => request.pendingUserMessageId !== messageId),
    );
    removeMessageForChatKey(chatKey, messageId);
    setError(null);
  }

  async function handleGuideQueuedMessage(messageId: string) {
    const chatKey = activeChatKeyRef.current;
    const runInfo = chatKey
      ? activeRunInfoByChatKeyRef.current[chatKey] ?? null
      : null;
    if (
      !chatKey ||
      !runInfo ||
      !runInfo.chatId ||
      !runInfo.runId ||
      !runInfo.acceptingGuidance ||
      runInfo.chatKey !== chatKey ||
      !runningChatKeysRef.current.has(chatKey)
    ) {
      setError(t("No active run is available for guidance."));
      return;
    }

    const queuedRequests = queuedRunRequestsByChatKeyRef.current[chatKey] ?? [];
    const queuedIndex = queuedRequests.findIndex(
      (request) => request.pendingUserMessageId === messageId,
    );
    if (queuedIndex < 0) {
      setError(t("Queued message is no longer available."));
      return;
    }

    const queuedRequest = queuedRequests[queuedIndex];
    const visibleUserContent = messageWithSelectedSkills(
      detectedSkills,
      queuedRequest.skillIds,
      queuedRequest.content,
    );
    const visibleParts = userMessageParts(
      visibleUserContent,
      queuedRequest.attachments,
    );

    updateQueuedRunRequestsForChatKey(chatKey, (current) =>
      current.filter((request) => request.pendingUserMessageId !== messageId),
    );
    setMessagesForChatKey(chatKey, (current) =>
      current.map((message) =>
        message.id === messageId && message.pendingMode === "queued"
          ? {
            ...message,
            content: visibleUserContent,
            pendingMode: "guidance",
            parts: visibleParts,
          }
          : message,
      ),
    );
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
            attachments: queuedRequest.attachments,
            chatId: runInfo.chatId,
            message: visibleUserContent,
            runId: runInfo.runId,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      pendingGuidanceMessageIdsRef.current.set(guidance.id, messageId);
    } catch (requestError) {
      updateQueuedRunRequestsForChatKey(chatKey, (current) => {
        const next = current.filter(
          (request) => request.pendingUserMessageId !== messageId,
        );
        next.splice(Math.min(queuedIndex, next.length), 0, queuedRequest);
        return next;
      });
      setMessagesForChatKey(chatKey, (current) =>
        current.map((message) =>
          message.id === messageId
            ? { ...message, pendingMode: "queued" }
            : message,
        ),
      );
      setError(errorMessage(requestError));
    }
  }

  async function handleRetryRun() {
    if (!retryRunRequest || isSendingMessage) {
      return;
    }

    const retryRequest = retryRunRequest;
    activeWorkspaceIdRef.current = retryRequest.workspaceId;
    activeChatIdRef.current = retryRequest.chatId;
    setActiveWorkspaceId(retryRequest.workspaceId);
    setActiveChatId(retryRequest.chatId);
    updateBrowserRoute({
      chatId: retryRequest.chatId,
      viewMode: "chat",
      workspaceId: retryRequest.workspaceId,
    });
    hasManuallySelectedModelRef.current = true;
    hasManuallySelectedThinkingLevelRef.current = true;
    setSelectedModelId(retryRequest.modelId);
    setSelectedProviderId(retryRequest.providerId);
    setSelectedSkillIds(retryRequest.skillIds);
    setSelectedThinkingLevel(retryRequest.thinkingLevel);
    await runChatMessage(retryRequest);
  }

  function handleChatModelChange(modelId: string) {
    hasManuallySelectedModelRef.current = true;
    hasManuallySelectedThinkingLevelRef.current = false;
    setSelectedModelId(modelId);
  }

  function handleChatProviderChange(providerId: string) {
    hasManuallySelectedModelRef.current = true;
    setSelectedProviderId(providerId);
  }

  function handleChatThinkingLevelChange(thinkingLevel: string) {
    hasManuallySelectedThinkingLevelRef.current = true;
    setSelectedThinkingLevel(thinkingLevel);
  }
  const {
    applyBrowserRoute,
    openCurrentChatView,
    openScheduledTasksView,
    openSettingsSection,
    openStatsView,
  } = useAppRouting({
    activeChatId,
    activeChatKeyRef,
    activeWorkspaceIdOrNull: activeWorkspace?.id ?? (activeWorkspaceId || null),
    onMissingWorkspace: setError,
    onRestoreWorkspaceChatTabs: restoreWorkspaceChatTabs,
    onRestoreWorkspaceFileTabs: restoreWorkspaceFileTabs,
    onSelectWorkspaceChat: selectWorkspaceChat,
    onStartNewWorkspaceChat: startNewWorkspaceChat,
    setActiveChatId,
    setIsMobileWorkspaceOpen,
    setMessages,
    setSettingsSection,
    setStatsRoutePage,
    setViewMode,
    updateBrowserRoute,
    workspaces,
  });

  const updateStatsRoutePage = useCallback(
    (page: number) => {
      setStatsRoutePage((current) => (current === page ? current : page));
      updateBrowserRoute({ page, viewMode: "stats" });
    },
    [updateBrowserRoute],
  );

  function handleHomeNavClick() {
    if (viewMode !== "chat") {
      openCurrentChatView();
      return;
    }

    if (typeof window !== "undefined" && window.innerWidth < 768) {
      setIsMobileWorkspaceOpen((current) => !current);
      return;
    }

    setIsWorkspaceSidebarOpen((current) => !current);
  }

  applyBrowserRouteRef.current = applyBrowserRoute;

  useInitialBrowserRouteEffect({
    canUseApp,
    hasAppliedInitialBrowserRouteRef,
    initialBrowserRoute,
    isLoading,
    onApplyRoute: applyBrowserRoute,
    onReplaceRoute: (route) => updateBrowserRoute(route, "replace"),
  });

  useBrowserPopState(applyBrowserRouteRef);

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

    const chatKey = chatRunKey(request.workspaceId, request.chatId);
    contextUsageIdentityByChatKeyRef.current.set(
      chatKey,
      [
        request.workspaceId,
        request.chatId,
        request.modelId,
        request.providerId,
        request.thinkingLevel,
      ].join("\u0000"),
    );
    const requestId =
      (contextUsageRequestIdByChatKeyRef.current.get(chatKey) ?? 0) + 1;
    contextUsageRequestIdByChatKeyRef.current.set(chatKey, requestId);
    contextUsageAbortByChatKeyRef.current.get(chatKey)?.abort();
    const abortController = new AbortController();
    contextUsageAbortByChatKeyRef.current.set(chatKey, abortController);
    setContextUsageLoadingByChatKey((current) => ({
      ...current,
      [chatKey]: true,
    }));

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
            latestResponseUsage: request.latestResponseUsage,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId) {
        setContextUsageByChatKey((current) => ({ ...current, [chatKey]: data }));
      }
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (
        !wasCancelled &&
        contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId
      ) {
        setError(errorMessage(requestError));
      }
    } finally {
      if (contextUsageAbortByChatKeyRef.current.get(chatKey) === abortController) {
        contextUsageAbortByChatKeyRef.current.delete(chatKey);
      }
      if (contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId) {
        setContextUsageLoadingByChatKey((current) => ({
          ...current,
          [chatKey]: false,
        }));
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
        specUpdates: [],
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
        specUpdates: [],
        },
      ];
    });
  }

  async function guideActiveRun(
    request: RetryRunRequest,
    runInfo: ActiveRunInfo,
  ) {
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
    const placeholderAssistantMessageId = assistantMessageId;
    let currentAssistantMessageId = assistantMessageId;
    // Once a guidance message is applied, the backend keeps emitting subsequent
    // stream events under the original (now-interrupted) assistant message id,
    // but they belong to the new post-guidance bubble. Tracking the interrupted
    // id lets us route those events to `currentAssistantMessageId` instead of
    // the stale bubble that the event id would otherwise match.
    let interruptedAssistantMessageId: string | null = null;
    let latestResponseUsage: ChatUsage | null = null;
    let liveStartedAtMs = Date.now();
    let hasGuidanceTurns = false;
    let streamHadError = false;
    const refreshRunContextUsage = () => {
      const modelId = selectedModelIdRef.current;
      const providerId = selectedProviderIdRef.current;
      if (!modelId || !providerId || !latestResponseUsage) {
        return;
      }

      void refreshContextUsage({
        chatId: activeRun.chatId,
        latestResponseUsage,
        modelId,
        providerId,
        skillIds: [],
        thinkingLevel: selectedThinkingLevelRef.current,
        workspaceId: activeRun.workspaceId,
      });
    };

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
    const finishStreamingAssistantMessage = (finishedAssistantMessageId: string) => {
      setMessagesForChatKey(chatKey, (current) =>
        current.map((message) =>
          message.role === "assistant" &&
          message.id === finishedAssistantMessageId &&
          message.status === "streaming"
            ? { ...message, status: undefined }
            : message,
        ),
      );
    };

    const isCurrentAssistantMessage = (
      message: ShellMessage,
      eventAssistantMessageId?: string,
    ) => {
      // After a guidance boundary the backend keeps emitting events under the
      // interrupted assistant message id, but they must land in the new bubble.
      // Ignore the event-carried id (and the original `assistantMessageId`,
      // which equals the interrupted id) in that case and match only the current
      // bubble.
      const ignoreInterruptedId =
        interruptedAssistantMessageId !== null &&
        (eventAssistantMessageId === undefined ||
          eventAssistantMessageId === interruptedAssistantMessageId);
      return (
        message.role === "assistant" &&
        (message.id === currentAssistantMessageId ||
          (!ignoreInterruptedId &&
            eventAssistantMessageId !== undefined &&
            message.id === eventAssistantMessageId) ||
          (!ignoreInterruptedId && message.id === assistantMessageId))
      );
    };

    let activeReasoningStartedAtMs: number | null = null;
    let liveReasoningDurationTimer: ReturnType<typeof setInterval> | null = null;
    const updateLiveReasoningDuration = (startedAtMs: number) => {
      setMessagesForChatKey(chatKey, (current) =>
        current.map((message) =>
          isCurrentAssistantMessage(message) && message.status === "streaming"
            ? {
              ...message,
              parts: updateActiveReasoningPartDuration(
                message.parts,
                startedAtMs,
                Date.now(),
              ),
            }
            : message,
        ),
      );
    };
    const startLiveReasoningDuration = () => {
      if (activeReasoningStartedAtMs !== null) {
        return activeReasoningStartedAtMs;
      }
      const startedAtMs = Date.now();
      activeReasoningStartedAtMs = startedAtMs;
      if (liveReasoningDurationTimer !== null) {
        clearInterval(liveReasoningDurationTimer);
      }
      updateLiveReasoningDuration(startedAtMs);
      liveReasoningDurationTimer = setInterval(
        () => updateLiveReasoningDuration(startedAtMs),
        LIVE_REASONING_DURATION_REFRESH_MS,
      );
      return startedAtMs;
    };
    const stopLiveReasoningDuration = () => {
      if (liveReasoningDurationTimer !== null) {
        clearInterval(liveReasoningDurationTimer);
        liveReasoningDurationTimer = null;
      }
    };
    const finishLiveReasoningDuration = (eventAssistantMessageId?: string) => {
      const startedAtMs = activeReasoningStartedAtMs;
      if (startedAtMs === null) {
        return;
      }
      activeReasoningStartedAtMs = null;
      stopLiveReasoningDuration();
      const endedAtMs = Date.now();
      setMessagesForChatKey(chatKey, (current) =>
        current.map((message) =>
          isCurrentAssistantMessage(message, eventAssistantMessageId)
            ? {
              ...message,
              parts: finishActiveReasoningPart(
                message.parts,
                startedAtMs,
                endedAtMs,
              ),
            }
            : message,
        ),
      );
    };
    // Resolve which assistant bubble a post-guidance event targets: once a
    // guidance boundary is crossed, events keep carrying the interrupted id but
    // must target the new bubble (`currentAssistantMessageId`).
    const resolvedAssistantMessageId = (
      eventAssistantMessageId?: string,
    ): string => {
      if (
        interruptedAssistantMessageId !== null &&
        (eventAssistantMessageId === undefined ||
          eventAssistantMessageId === interruptedAssistantMessageId)
      ) {
        return currentAssistantMessageId;
      }
      return eventAssistantMessageId ?? currentAssistantMessageId;
    };

    setChatRunning(chatKey, true);
    setChatRunFailed(chatKey, false);
    setActiveRunInfoForChatKey(chatKey, {
      acceptingGuidance: activeRun.acceptingGuidance,
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
          const previousAssistantMessageId = currentAssistantMessageId;
          const startsNewAssistantBubble =
            previousAssistantMessageId !== streamEvent.assistantMessageId &&
            previousAssistantMessageId !== placeholderAssistantMessageId;
          assistantMessageId = streamEvent.assistantMessageId;
          currentAssistantMessageId = streamEvent.assistantMessageId;
          if (startsNewAssistantBubble) {
            finishStreamingAssistantMessage(previousAssistantMessageId);
          }
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              message.role === "assistant" && message.id === streamEvent.assistantMessageId
                ? {
                  ...message,
                  content: "",
                  reasoning: null,
                  toolCalls: [],
                  parts: [],
                  metrics: null,
                  status: "streaming",
                }
                : message,
            ),
          );
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId,
            streamEvent.memoriesUsed,
          );
          setChatRunFailed(chatKey, false);
          setChatRunning(chatKey, true);
          setActiveRunInfoForChatKey(chatKey, {
            acceptingGuidance: true,
            chatId: streamEvent.chatId,
            chatKey,
            lastSequence: activeRun.lastSequence,
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          liveStartedAtMs = Date.now();
          updateLiveChatStatistics(chatKey, {
            modelId: selectedModelIdRef.current,
            providerId: selectedProviderIdRef.current,
            startedAtMs: liveStartedAtMs,
            usage: null,
          });
          refreshActiveAgentTeamSnapshot(activeRun.workspaceId, streamEvent.chatId);
          return;
        }

        if (streamEvent.type === "textDelta") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
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
          const reasoningStartedAtMs = startLiveReasoningDuration();
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                  ...message,
                  reasoning: `${message.reasoning ?? ""}${streamEvent.delta}`,
                  parts: appendReasoningPart(
                    message.parts,
                    streamEvent.delta,
                    reasoningStartedAtMs,
                  ),
                }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "streamAttemptStart") {
          // A post-guidance turn still emits streamAttemptStart under the
          // interrupted id; keep targeting the new bubble in that case.
          if (interruptedAssistantMessageId === null) {
            currentAssistantMessageId = streamEvent.assistantMessageId;
          }
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setActiveRunInfoForChatKey(chatKey, {
            acceptingGuidance: true,
            chatId: activeRun.chatId,
            chatKey,
            lastSequence: activeRun.lastSequence,
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          return;
        }

        if (streamEvent.type === "streamReset") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          latestResponseUsage = null;
          updateLiveChatStatistics(chatKey, {
            modelId: selectedModelIdRef.current,
            providerId: selectedProviderIdRef.current,
            startedAtMs: liveStartedAtMs,
            usage: null,
          });
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? resetStreamingAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "contextCompression") {
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? addChatRunBadge(message, contextCompressionBadge(streamEvent.kind))
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
          updateLiveChatStatistics(chatKey, {
            modelId: selectedModelIdRef.current,
            providerId: selectedProviderIdRef.current,
            startedAtMs: liveStartedAtMs,
            usage: latestResponseUsage,
          });
          refreshRunContextUsage();
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          finishLiveReasoningDuration(currentAssistantMessageId);
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          interruptedAssistantMessageId = previousAssistantId;
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
          const completedAtMs = Date.now();
          const completedReasoningStartedAtMs = activeReasoningStartedAtMs;
          activeReasoningStartedAtMs = null;
          stopLiveReasoningDuration();
          const liveStatisticsUsage = streamEvent.usage ?? latestResponseUsage;
          updateLiveChatStatistics(chatKey, {
            modelId: streamEvent.metrics.modelId,
            providerId: streamEvent.metrics.providerId,
            startedAtMs: liveStartedAtMs,
            usage: liveStatisticsUsage,
          });
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          setChatRunFailed(chatKey, false);
          setChatRunning(chatKey, false);
          setActiveRunInfoForChatKey(chatKey, null);
          setRetryRunRequest(null);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? hasGuidanceTurns
                  ? completedGuidanceAssistantMessage(
                    message,
                    streamEvent,
                    completedReasoningStartedAtMs,
                    completedAtMs,
                  )
                  : completedAssistantMessage(
                    message,
                    streamEvent,
                    completedReasoningStartedAtMs,
                    completedAtMs,
                  )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCall.id);
          setMessagesForChatKey(chatKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
                ? {
                  ...message,
                  parts: upsertToolCallPart(message.parts, streamEvent.toolCall),
                  toolCalls: upsertToolCall(
                    message.toolCalls,
                    streamEvent.toolCall,
                  ),
                }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "toolResult") {
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          setMessagesForChatKey(chatKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
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
            );
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          setMessagesForChatKey(chatKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
                ? {
                  ...message,
                  parts: applyToolOutputDeltaToParts(
                    message.parts,
                    streamEvent.toolCallId,
                    streamEvent.stream,
                    streamEvent.delta,
                  ),
                  toolCalls: applyToolOutputDelta(
                    message.toolCalls,
                    streamEvent.toolCallId,
                    streamEvent.stream,
                    streamEvent.delta,
                  ),
                }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "questionRequest") {
          setQuestionError(null);
          setPendingQuestion(streamEvent.request);
          return;
        }

        if (streamEvent.type === "hookNotification") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
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
          updateLiveChatStatistics(chatKey, {
            codeChangeStats: streamEvent.codeChangeStats,
            modelId: selectedModelIdRef.current,
            providerId: selectedProviderIdRef.current,
            startedAtMs: liveStartedAtMs,
            usage: latestResponseUsage,
          });
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          setActiveWorkspaceChatRefs(streamEvent.workspaceId, streamEvent.chatId);
          setContextPanelTab("todo");
          setIsContextPanelOpen(true);
          void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId, {
            ignoreRequestInvalidation: true,
          });
          return;
        }

        if (streamEvent.type === "agentTeamRefresh") {
          handleAgentTeamRefresh(streamEvent);
          return;
        }

        if (streamEvent.type === "memoryExtractionComplete") {
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? assistantMessageWithExtractedMemories(
                  message,
                  streamEvent.extractedMemories,
                )
                : message,
            ),
          );
          return;
        }
        if (streamEvent.type === "memoryResolved") {
          setMessagesForChatKey(chatKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? assistantMessageWithMemoriesUsed(
                  message,
                  streamEvent.memoriesUsed,
                )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "streamEnd") {
          finishLiveReasoningDuration();
          stopLiveReasoningDuration();
          refreshActiveAgentTeamSnapshot(activeRun.workspaceId, activeRun.chatId);
          void refreshMessagesAfterSpecJobSettles(
            activeRun.workspaceId,
            activeRun.chatId,
            activeRun.runId,
          );
          return;
        }

        if (streamEvent.type === "error") {
          finishLiveReasoningDuration();
          stopLiveReasoningDuration();
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
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (!wasCancelled) {
        setChatRunFailed(chatKey, true);
        setError(errorMessage(requestError));
      }
    } finally {
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      if (activeRunAbortByChatKeyRef.current.get(chatKey) === abortController) {
        activeRunAbortByChatKeyRef.current.delete(chatKey);
        setChatRunning(chatKey, false);
        setActiveRunInfoForChatKey(chatKey, null);
        clearLiveChatStatistics(chatKey);
      }
      if (!streamHadError) {
        setPendingQuestion(null);
        setQuestionError(null);
        setIsAnsweringQuestion(false);
      }
    }
  }

  async function runChatMessage(initialRequest: RetryRunRequest): Promise<string | null> {
    let request = initialRequest;
    if (!request.queuedUserMessageId) {
      setIsPreparingChatRun(true);
      try {
        const queued = await persistQueuedRunRequest(request);
        request = {
          ...request,
          chatId: queued.chatId,
          pendingUserMessageId: queued.userMessageId,
          queuedUserMessageId: queued.userMessageId,
          assistantMessageId: queued.assistantMessageId,
        };
        const queuedChatKey = chatRunKey(request.workspaceId, queued.chatId);
        setActiveWorkspaceId(request.workspaceId);
        setActiveChatId(queued.chatId);
        setActiveMainTab({
          chatId: queued.chatId,
          type: "chat",
          workspaceId: request.workspaceId,
        });
        openPendingChatTab(request.workspaceId, queued.chatId, queued.chatTitle);
        setExpandedWorkspaceId(request.workspaceId);
        activeWorkspaceIdRef.current = request.workspaceId;
        activeChatIdRef.current = queued.chatId;
        activeChatKeyRef.current = queuedChatKey;
        setSelectedDiffPath(null);
        setViewMode("chat");
        setIsMobileWorkspaceOpen(false);
        updateBrowserRoute({
          chatId: queued.chatId,
          viewMode: "chat",
          workspaceId: request.workspaceId,
        });
      } catch (requestError) {
        setError(errorMessage(requestError));
        return null;
      } finally {
        setIsPreparingChatRun(false);
      }
    }
    const runKey = localRandomId();
    const pendingUserMessageId = request.pendingUserMessageId ?? null;
    const localUserId = pendingUserMessageId ?? `local-user-${runKey}`;
    const localAssistantId = request.assistantMessageId ?? `local-assistant-${runKey}`;
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
    // See subscribeActiveChatRun: post-guidance events keep carrying the
    // interrupted assistant message id but must target the new bubble.
    let interruptedAssistantMessageId: string | null = null;
    let requestChatId = request.chatId;
    const pendingChatId =
      request.chatId || request.localChatKey ? null : `pending:${runKey}`;
    let runMessagesKey = request.localChatKey ?? (requestChatId
      ? chatRunKey(request.workspaceId, requestChatId)
      : pendingChatRunKey(request.workspaceId, runKey));
    let currentRunningChatKey = runMessagesKey;
    let latestResponseUsage: ChatUsage | null = null;
    let liveStartedAtMs = Date.now();
    let runSucceeded = false;
    let streamHadError = false;
    let hasGuidanceTurns = false;
    let activeRunId: string | null = null;
    const abortController = new AbortController();
    const refreshRunContextUsage = () => {
      if (!latestResponseUsage) {
        return;
      }

      void refreshContextUsage({
        chatId: requestChatId,
        latestResponseUsage,
        modelId: request.modelId,
        providerId: request.providerId,
        skillIds: request.skillIds,
        thinkingLevel: request.thinkingLevel,
        workspaceId: request.workspaceId,
      });
    };

    const shouldActivateRun =
      !request.localChatKey || activeChatKeyRef.current === request.localChatKey;

    if (shouldActivateRun) {
      activeChatKeyRef.current = runMessagesKey;
    }
    if (pendingChatId) {
      setActiveWorkspaceId(request.workspaceId);
      setActiveChatId(pendingChatId);
      setActiveMainTab({
        chatId: pendingChatId,
        type: "chat",
        workspaceId: request.workspaceId,
      });
      setExpandedWorkspaceId(request.workspaceId);
      activeWorkspaceIdRef.current = request.workspaceId;
      activeChatIdRef.current = pendingChatId;
      openPendingChatTab(
        request.workspaceId,
        pendingChatId,
        chatTitleForDraft(request.content, request.attachments),
      );
      setSelectedDiffPath(null);
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      updateBrowserRoute({
        chatId: null,
        viewMode: "chat",
        workspaceId: request.workspaceId,
      });
    }
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
        specUpdates: [],
        runBadges: [],
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
        specUpdates: [],
        },
        assistantMessage,
      ];
    });
    setDraftMessage("");
    setChatRunning(currentRunningChatKey, true);
    setActiveRunInfoForChatKey(currentRunningChatKey, {
      acceptingGuidance: false,
      chatId: requestChatId,
      chatKey: currentRunningChatKey,
      runId: null,
      workspaceId: request.workspaceId,
    });
    setRetryRunRequest(null);
    setError(null);
    if (request.chatId) {
      cancelContextUsageRequestForChatKey(currentRunningChatKey);
    }
    if (request.queuedUserMessageId) {
      updateQueuedRunRequestsForChatKey(currentRunningChatKey, (current) =>
        current.filter(
          (queuedRequest) =>
            queuedRequest.queuedUserMessageId !== request.queuedUserMessageId &&
            queuedRequest.pendingUserMessageId !== request.queuedUserMessageId,
        ),
      );
    }
    activeRunAbortByChatKeyRef.current.set(
      currentRunningChatKey,
      abortController,
    );

    const ensureStreamingAssistantMessage = (
      nextAssistantMessageId: string,
      memoriesUsed: ChatMemoryUsedSummary[] = [],
    ) => {
      setMessagesForChatKey(runMessagesKey, (current) => {
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
    const finishStreamingAssistantMessage = (finishedAssistantMessageId: string) => {
      setMessagesForChatKey(runMessagesKey, (current) =>
        current.map((message) =>
          message.role === "assistant" &&
          message.id === finishedAssistantMessageId &&
          message.status === "streaming"
            ? { ...message, status: undefined }
            : message,
        ),
      );
    };
    const isCurrentAssistantMessage = (
      message: ShellMessage,
      eventAssistantMessageId?: string,
    ) => {
      const ignoreInterruptedId =
        interruptedAssistantMessageId !== null &&
        (eventAssistantMessageId === undefined ||
          eventAssistantMessageId === interruptedAssistantMessageId);
      return (
        message.role === "assistant" &&
        (message.id === currentAssistantMessageId ||
          (!ignoreInterruptedId &&
            eventAssistantMessageId !== undefined &&
            message.id === eventAssistantMessageId) ||
          (!ignoreInterruptedId && message.id === assistantMessageId) ||
          (currentAssistantMessageId === localAssistantId &&
            message.id === localAssistantId))
      );
    };
    const resolvedAssistantMessageId = (
      eventAssistantMessageId?: string,
    ): string => {
      if (
        interruptedAssistantMessageId !== null &&
        (eventAssistantMessageId === undefined ||
          eventAssistantMessageId === interruptedAssistantMessageId)
      ) {
        return currentAssistantMessageId;
      }
      return eventAssistantMessageId ?? currentAssistantMessageId;
    };
    let activeReasoningStartedAtMs: number | null = null;
    let liveReasoningDurationTimer: ReturnType<typeof setInterval> | null = null;
    const updateLiveReasoningDuration = (startedAtMs: number) => {
      setMessagesForChatKey(runMessagesKey, (current) =>
        current.map((message) =>
          isCurrentAssistantMessage(message) && message.status === "streaming"
            ? {
              ...message,
              parts: updateActiveReasoningPartDuration(
                message.parts,
                startedAtMs,
                Date.now(),
              ),
            }
            : message,
        ),
      );
    };
    const startLiveReasoningDuration = () => {
      if (activeReasoningStartedAtMs !== null) {
        return activeReasoningStartedAtMs;
      }
      const startedAtMs = Date.now();
      activeReasoningStartedAtMs = startedAtMs;
      if (liveReasoningDurationTimer !== null) {
        clearInterval(liveReasoningDurationTimer);
      }
      updateLiveReasoningDuration(startedAtMs);
      liveReasoningDurationTimer = setInterval(
        () => updateLiveReasoningDuration(startedAtMs),
        LIVE_REASONING_DURATION_REFRESH_MS,
      );
      return startedAtMs;
    };
    const stopLiveReasoningDuration = () => {
      if (liveReasoningDurationTimer !== null) {
        clearInterval(liveReasoningDurationTimer);
        liveReasoningDurationTimer = null;
      }
    };
    const finishLiveReasoningDuration = (eventAssistantMessageId?: string) => {
      const startedAtMs = activeReasoningStartedAtMs;
      if (startedAtMs === null) {
        return;
      }
      activeReasoningStartedAtMs = null;
      stopLiveReasoningDuration();
      const endedAtMs = Date.now();
      setMessagesForChatKey(runMessagesKey, (current) =>
        current.map((message) =>
          isCurrentAssistantMessage(message, eventAssistantMessageId)
            ? {
              ...message,
              parts: finishActiveReasoningPart(
                message.parts,
                startedAtMs,
                endedAtMs,
              ),
            }
            : message,
        ),
      );
    };
    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/stream`,
        {
          body: JSON.stringify({
            chatId: request.chatId,
            queuedUserMessageId: request.queuedUserMessageId ?? null,
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
          const previousAssistantMessageId = currentAssistantMessageId;
          const startsNewAssistantBubble =
            previousAssistantMessageId !== streamEvent.assistantMessageId &&
            previousAssistantMessageId !== localAssistantId;
          assistantMessageId = streamEvent.assistantMessageId;
          currentAssistantMessageId = streamEvent.assistantMessageId;
          requestChatId = streamEvent.chatId;
          currentRunningChatKey = chatRunKey(
            request.workspaceId,
            streamEvent.chatId,
          );
          setChatRunFailed(currentRunningChatKey, false);
          if (pendingChatId) {
            replacePendingChatTab(
              request.workspaceId,
              pendingChatId,
              streamEvent.chatId,
            );
          } else if (request.localChatKey) {
            openPendingChatTab(
              request.workspaceId,
              streamEvent.chatId,
              request.content,
            );
          } else {
            openChatTab(request.workspaceId, streamEvent.chatId);
          }

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
          if (startsNewAssistantBubble) {
            finishStreamingAssistantMessage(previousAssistantMessageId);
          }
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId,
            streamEvent.memoriesUsed,
          );
          setChatRunning(currentRunningChatKey, true);
          activeRunId = streamEvent.llmRequestId ?? activeRunId;
          setActiveRunInfoForChatKey(currentRunningChatKey, {
            acceptingGuidance: activeRunId !== null,
            chatId: streamEvent.chatId,
            chatKey: currentRunningChatKey,
            runId: activeRunId,
            workspaceId: request.workspaceId,
          });
          liveStartedAtMs = Date.now();
          updateLiveChatStatistics(currentRunningChatKey, {
            modelId: request.modelId,
            providerId: request.providerId,
            startedAtMs: liveStartedAtMs,
            usage: null,
          });
          refreshActiveAgentTeamSnapshot(request.workspaceId, streamEvent.chatId);
          const shouldActivateStartedChat =
            shouldActivateRun ||
            activeChatKeyRef.current === currentRunningChatKey ||
            activeChatKeyRef.current === request.localChatKey ||
            activeChatKeyRef.current === null ||
            Boolean(request.chatId && !request.localChatKey);
          if (
            shouldActivateStartedChat
          ) {
            setActiveWorkspaceChatRefs(request.workspaceId, streamEvent.chatId);
            setActiveChatId(streamEvent.chatId);
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
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
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
          const reasoningStartedAtMs = startLiveReasoningDuration();
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                  ...message,
                  reasoning: `${message.reasoning ?? ""}${streamEvent.delta}`,
                  parts: appendReasoningPart(
                    message.parts,
                    streamEvent.delta,
                    reasoningStartedAtMs,
                  ),
                }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "streamAttemptStart") {
          // A post-guidance turn still emits streamAttemptStart under the
          // interrupted id; keep targeting the new bubble in that case.
          if (interruptedAssistantMessageId === null) {
            currentAssistantMessageId = streamEvent.assistantMessageId;
          }
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setActiveRunInfoForChatKey(runMessagesKey, {
            acceptingGuidance: activeRunId !== null,
            chatId: requestChatId,
            chatKey: runMessagesKey,
            runId: activeRunId,
            workspaceId: request.workspaceId,
          });
          return;
        }

        if (streamEvent.type === "streamReset") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          latestResponseUsage = null;
          updateLiveChatStatistics(runMessagesKey, {
            modelId: request.modelId,
            providerId: request.providerId,
            startedAtMs: liveStartedAtMs,
            usage: null,
          });
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? resetStreamingAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "contextCompression") {
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? addChatRunBadge(message, contextCompressionBadge(streamEvent.kind))
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
          updateLiveChatStatistics(runMessagesKey, {
            modelId: request.modelId,
            providerId: request.providerId,
            startedAtMs: liveStartedAtMs,
            usage: latestResponseUsage,
          });
          refreshRunContextUsage();
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          finishLiveReasoningDuration(currentAssistantMessageId);
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          interruptedAssistantMessageId = previousAssistantId;
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
          const completedAtMs = Date.now();
          const completedReasoningStartedAtMs = activeReasoningStartedAtMs;
          activeReasoningStartedAtMs = null;
          stopLiveReasoningDuration();
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const liveStatisticsUsage = streamEvent.usage ?? latestResponseUsage;
          updateLiveChatStatistics(runMessagesKey, {
            modelId: streamEvent.metrics.modelId,
            providerId: streamEvent.metrics.providerId,
            startedAtMs: liveStartedAtMs,
            usage: liveStatisticsUsage,
          });
          setActiveRunInfoForChatKey(runMessagesKey, null);
          if (requestChatId) {
            void loadChatStatistics(request.workspaceId, requestChatId);
          }
          setChatRunFailed(runMessagesKey, false);
          setChatRunning(runMessagesKey, false);
          setRetryRunRequest(null);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? hasGuidanceTurns
                  ? completedGuidanceAssistantMessage(
                    message,
                    streamEvent,
                    completedReasoningStartedAtMs,
                    completedAtMs,
                  )
                  : completedAssistantMessage(
                    message,
                    streamEvent,
                    completedReasoningStartedAtMs,
                    completedAtMs,
                  )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCall.id);
          setMessagesForChatKey(runMessagesKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
                ? {
                  ...message,
                  toolCalls: upsertToolCall(
                    message.toolCalls,
                    streamEvent.toolCall,
                  ),
                  parts: upsertToolCallPart(message.parts, streamEvent.toolCall),
                }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "toolResult") {
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          setMessagesForChatKey(runMessagesKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
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
            );
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          setMessagesForChatKey(runMessagesKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (updateExistingToolCall
                ? messageOwnsToolCall(message)
                : isCurrentAssistantMessage(message, streamEvent.assistantMessageId))
                ? {
                  ...message,
                  toolCalls: applyToolOutputDelta(
                    message.toolCalls,
                    streamEvent.toolCallId,
                    streamEvent.stream,
                    streamEvent.delta,
                  ),
                  parts: applyToolOutputDeltaToParts(
                    message.parts,
                    streamEvent.toolCallId,
                    streamEvent.stream,
                    streamEvent.delta,
                  ),
                }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "questionRequest") {
          setQuestionError(null);
          setPendingQuestion(streamEvent.request);
          return;
        }

        if (streamEvent.type === "hookNotification") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
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
          updateLiveChatStatistics(runMessagesKey, {
            codeChangeStats: streamEvent.codeChangeStats,
            modelId: request.modelId,
            providerId: request.providerId,
            startedAtMs: liveStartedAtMs,
            usage: latestResponseUsage,
          });
          if (requestChatId) {
            void loadChatStatistics(request.workspaceId, requestChatId);
          }
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          setActiveWorkspaceChatRefs(streamEvent.workspaceId, streamEvent.chatId);
          setContextPanelTab("todo");
          setIsContextPanelOpen(true);
          void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId, {
            ignoreRequestInvalidation: true,
          });
          return;
        }

        if (streamEvent.type === "agentTeamRefresh") {
          handleAgentTeamRefresh(streamEvent);
          return;
        }

        if (streamEvent.type === "memoryExtractionComplete") {
          if (requestChatId) {
            void loadChatStatistics(request.workspaceId, requestChatId);
          }
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? assistantMessageWithExtractedMemories(
                  message,
                  streamEvent.extractedMemories,
                )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "memoryResolved") {
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? assistantMessageWithMemoriesUsed(
                  message,
                  streamEvent.memoriesUsed,
                )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "streamEnd") {
          finishLiveReasoningDuration();
          stopLiveReasoningDuration();
          if (requestChatId) {
            refreshActiveAgentTeamSnapshot(request.workspaceId, requestChatId);
            void refreshMessagesAfterSpecJobSettles(
              request.workspaceId,
              requestChatId,
              activeRunId,
            );
          }
          return;
        }

        if (streamEvent.type === "error") {
          finishLiveReasoningDuration();
          stopLiveReasoningDuration();
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
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
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
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      if (
        activeRunAbortByChatKeyRef.current.get(currentRunningChatKey) ===
        abortController
      ) {
        activeRunAbortByChatKeyRef.current.delete(currentRunningChatKey);
        setChatRunning(currentRunningChatKey, false);
        setActiveRunInfoForChatKey(currentRunningChatKey, null);
        clearLiveChatStatistics(currentRunningChatKey);
      }
    }

    if (request.localChatKey) {
      updateScheduledWorkspaceRuns((current) =>
        current.filter((run) => run.chatKey !== request.localChatKey),
      );
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

    return runSucceeded ? requestChatId : null;
  }

  function toggleWorkspace(workspaceId: string) {
    const isCollapsingWorkspace = expandedWorkspaceId === workspaceId;
    setExpandedWorkspaceId(isCollapsingWorkspace ? null : workspaceId);
    if (isCollapsingWorkspace) {
      setWorkspaceChatVisibleCounts((current) => {
        const next = { ...current };
        delete next[workspaceId];
        return next;
      });
    }
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
    setWorkspaceIconDraft(null);
    setError(null);
    setWorkspaceDialogRevision((current) => current + 1);
    setIsWorkspaceDialogOpen(true);
  }

  function closeWorkspaceDialog() {
    setWorkspaceIconDraft(null);
    setIsWorkspaceDialogOpen(false);
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

  const handleSettingsPanelSettingsChange = useCallback((data: SettingsResponse) => {
    setSettings(data);
    setIsTeamModeEnabled(data.general.defaultTeamModeEnabled);
  }, []);

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

  const chatPanelHelpers: ChatPanelHelpers = {
    activeSkillQuery,
    compactInlineText,
    compactToolJson,
    fallbackMessageParts,
    formatChatCreatedAt,
    formatFileSize,
    formatJsonValue,
    formatNullableLatencySeconds: (value, nextLanguage) =>
      formatNullableLatencySeconds(value, nextLanguage as AppLanguageId),
    formatTokensPerSecond: (metrics, nextLanguage) =>
      formatTokensPerSecond(metrics, nextLanguage as AppLanguageId),
    messageCopyText,
    normalizedToolInput,
    removeActiveSkillToken,
    selectedSkillPrefix,
    skillScopeLabel,
    toolCallChangeStats,
    toolCallDetailText,
    toolLiveOutputText,
    toolStatusText,
  };

  return (
    <I18nContext.Provider value={{ language, t }}>
      <main className="app-root foco-workbench text-stone-950">
        {error ? (
          <section
            aria-live="assertive"
            className="app-error-toast"
            role="alert"
          >
            <CircleAlert aria-hidden="true" className="app-error-toast-icon" />
            <div className="app-error-toast-message">{error}</div>
            <button
              aria-label={t("Close error message")}
              className="app-error-toast-close"
              onClick={() => setError(null)}
              title={t("Close error message")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
          </section>
        ) : null}
        {isGlobalView ? (
          <div className="global-shell">
            <FocoNavRail
              activeMode={viewMode}
              canLogout={canLogout}
              contextPanelButton={null}
              isSavingTheme={isSavingTheme}
              onAddWorkspace={openWorkspaceDialog}
              onLogout={handleLogout}
              onHomeClick={handleHomeNavClick}
              onOpenScheduledTasks={openScheduledTasksView}
              onOpenSettings={() => openSettingsSection("general")}
              onOpenStats={openStatsView}
              onReturnHome={openCurrentChatView}
              onToggleTheme={() =>
                void saveAppTheme(theme === "dark" ? "light" : "dark")
              }
              terminalButton={null}
              theme={theme}
            />
            <section className="global-main-panel min-w-0">
              {viewMode === "settings" ? (
                <SettingsPanel
                  agentDefinitionOperationKey={agentDefinitionOperationKey}
                  agentDefinitions={agentDefinitions}
                  agentDefinitionsError={agentDefinitionsError}
                  canLogout={canLogout}
                  canUseNativePicker={canUseNativePicker}
                  activeSection={settingsSection}
                  isLoadingAgentDefinitions={isLoadingAgentDefinitions}
                  nativeBrowserToken={nativeBrowserToken}
                  onAddWorkspace={openWorkspaceDialog}
                  onActiveSectionChange={openSettingsSection}
                  onCreateAgentDefinition={createAgentDefinition}
                  onDeleteAgentDefinition={deleteAgentDefinition}
                  onUpdateAgentDefinition={updateAgentDefinition}
                  onLogout={handleLogout}
                  onOpenChat={selectWorkspaceChat}
                  onSettingsChange={handleSettingsPanelSettingsChange}
                  onWorkspacesChange={refreshWorkspaces}
                  workspaceDialogRevision={workspaceDialogRevision}
                />
              ) : viewMode === "scheduled" ? (
                <ScheduledTasksPage
                  agentDefinitions={agentDefinitions}
                  onOpenChat={selectWorkspaceChat}
                  settings={settings}
                  workspaces={workspaces}
                />
              ) : (
                <ApiStatsPanel
                  onRoutePageChange={updateStatsRoutePage}
                  routePage={statsRoutePage}
                  settings={settings}
                  workspaces={workspaces}
                />
              )}
            </section>
          </div>
        ) : (
          <div
            className={`app-shell ${showContextPanel ? "app-shell-with-context" : ""} ${isWorkspaceSidebarOpen ? "" : "app-shell-workspace-closed"
              }`}
            style={
              {
                "--diff-panel-width": `${diffPanelWidth}px`,
                "--context-panel-min-height": `${CONTEXT_PANEL_MIN_HEIGHT}px`,
                "--context-panel-mobile-height": `${contextPanelMobileHeight}px`,
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
              contextPanelButton={{
                active: isContextPanelOpen,
                icon: ResponsiveContextPanelIcon,
                label: isContextPanelOpen
                  ? t("Close context panel")
                  : t("Open context panel"),
                onClick: () => setIsContextPanelOpen((current) => !current),
              }}
              terminalButton={{
                active: isTerminalOpen,
                disabled: !activeWorkspace,
                icon: SquareTerminal,
                label: isTerminalOpen ? t("Close terminal") : t("Open terminal"),
                onClick: toggleWorkspaceTerminal,
              }}
              onLogout={handleLogout}
              onOpenScheduledTasks={openScheduledTasksView}
              onOpenSettings={() => openSettingsSection("general")}
              onOpenStats={openStatsView}
              onHomeClick={handleHomeNavClick}
              onReturnHome={openCurrentChatView}
              onToggleTheme={() =>
                void saveAppTheme(theme === "dark" ? "light" : "dark")
              }
              theme={theme}
            />
            <aside
              className={`workspace-sidebar relative border-stone-200/80 lg:border-r ${isMobileWorkspaceOpen ? "workspace-sidebar-mobile-open" : ""
                }`}
              ref={workspaceSidebarRef}
            >
              <div
                aria-label={t("Resize workspace sidebar")}
                aria-orientation="vertical"
                aria-valuemax={WORKSPACE_SIDEBAR_MAX_WIDTH}
                aria-valuemin={WORKSPACE_SIDEBAR_MIN_WIDTH}
                aria-valuenow={sidebarWidth}
                className={`workspace-sidebar-splitter cursor-col-resize ${isResizingSidebar ? "workspace-sidebar-splitter-active" : ""
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
                      const persistedWorkspaceChatIds = new Set(
                        workspace.chats.map((chat) => chat.id),
                      );
                      const scheduledChats = scheduledWorkspaceRunsFor(
                        workspace.id,
                      )
                        .filter((run) => !persistedWorkspaceChatIds.has(run.chatId))
                        .map(
                          (run): WorkspaceChatListItem => ({
                            activeRun: null,
                            codeChangeStats: { additions: 0, deletions: 0 },
                            createdAt: run.createdAt,
                            id: run.chatId,
                            queuedRun: null,
                            scheduledChatKey: run.chatKey,
                            scheduledRunId: run.id,
                            scheduledStatus: run.status,
                            title: run.title,
                            updatedAt: run.createdAt,
                          }),
                        );
                      const persistedWorkspaceChats: WorkspaceChatListItem[] = workspace.chats.map(
                        (chat) => ({
                          ...chat,
                          scheduledStatus:
                            chat.queuedRun?.status === "queued" ? "queued" : undefined,
                        }),
                      );
                      const workspaceChats: WorkspaceChatListItem[] = [
                        ...scheduledChats,
                        ...persistedWorkspaceChats,
                      ].sort(compareWorkspaceChatListItemsByCreatedAtDesc);
                      const visibleChatCount =
                        selectedChatIndex >= configuredVisibleChatCount
                          ? selectedChatIndex + 1
                          : configuredVisibleChatCount;
                      const visibleChats = workspaceChats.slice(0, visibleChatCount);
                      const hiddenChatCount = Math.max(
                        workspaceChats.length - visibleChats.length,
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
                            <div className="mt-1 space-y-1 border-l border-stone-200/80 pl-3 pr-1.5">
                              {workspaceChats.length > 0 ? (
                                <>
                                  {visibleChats.map((chat) => {
                                    const chatKey = chatRunKey(workspace.id, chat.id);
                                    const scheduledChatKey =
                                      chat.scheduledChatKey ?? null;
                                    const isChatRunning =
                                      runningChatKeys.has(chatKey) ||
                                      Boolean(chat.activeRun) ||
                                      Boolean(
                                        scheduledChatKey &&
                                        runningChatKeys.has(scheduledChatKey),
                                      );
                                    const isChatScheduled =
                                      chat.scheduledStatus === "queued" ||
                                      chat.scheduledStatus === "starting";
                                    const isChatOpen =
                                      openChatKeySet.has(chatKey) ||
                                      Boolean(
                                        scheduledChatKey &&
                                        activeChatKey === scheduledChatKey,
                                      );
                                    const isChatFailed =
                                      isChatOpen && failedChatKeySet.has(chatKey);
                                    let statusDotClass = "session-status-dot-idle";
                                    if (isChatRunning) {
                                      statusDotClass = "session-status-dot-running";
                                    } else if (isChatScheduled) {
                                      statusDotClass = "session-status-dot-scheduled";
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
                                      <button
                                        aria-current={
                                          isChatActive ? "page" : undefined
                                        }
                                        className={chatItemClass(isChatActive)}
                                        key={chat.id}
                                        onClick={() => {
                                          if (suppressNextWorkspaceChatClickRef.current) {
                                            suppressNextWorkspaceChatClickRef.current =
                                              false;
                                            return;
                                          }

                                          selectWorkspaceChat(workspace.id, chat.id);
                                        }}
                                        onContextMenu={(event) =>
                                          openWorkspaceChatContextMenu(
                                            event,
                                            workspace,
                                            chat,
                                          )
                                        }
                                        onPointerCancel={cancelWorkspaceChatLongPress}
                                        onPointerDown={(event) =>
                                          startWorkspaceChatLongPress(
                                            event,
                                            workspace,
                                            chat,
                                          )
                                        }
                                        onPointerLeave={cancelWorkspaceChatLongPress}
                                        onPointerUp={cancelWorkspaceChatLongPress}
                                        title={chat.title}
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
                                    );
                                  })}
                                  {hiddenChatCount > 0 ? (
                                    <button
                                      aria-label={t(
                                        "Show {count} more chats in {name}",
                                        {
                                          count: nextVisibleChatCount,
                                          name: workspace.name,
                                        },
                                      )}
                                      className="flex min-h-10 min-w-0 w-full items-center gap-2 rounded-lg border border-transparent px-2 py-1.5 text-left text-xs font-medium text-stone-500 hover:border-stone-200 hover:bg-white/80 hover:text-stone-950"
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

            {workspaceChatContextMenu ? (
              <div
                aria-label={workspaceChatContextMenu.chat.title}
                className="workspace-chat-context-menu"
                role="menu"
                style={{
                  left: workspaceChatContextMenu.left,
                  top: workspaceChatContextMenu.top,
                }}
              >
                <button
                  className="workspace-chat-context-menu-item workspace-chat-context-menu-item-danger"
                  disabled={Boolean(workspaceChatContextMenu.chat.scheduledRunId)}
                  onClick={() => {
                    const { chat, workspace } = workspaceChatContextMenu;
                    setWorkspaceChatContextMenu(null);
                    requestDeleteWorkspaceChat(workspace, chat);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Trash2 aria-hidden="true" className="size-3.5" />
                  <span>{t("Delete chat")}</span>
                </button>
              </div>
            ) : null}

            {workspaceFileContextMenu ? (
              <div
                aria-label={workspaceFileContextMenu.node.name}
                className="workspace-chat-context-menu workspace-file-context-menu"
                role="menu"
                style={{
                  left: workspaceFileContextMenu.left,
                  top: workspaceFileContextMenu.top,
                }}
              >
                <button
                  className="workspace-chat-context-menu-item"
                  onClick={() => {
                    const { node } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    if (node.kind === "directory") {
                      void toggleWorkspaceFileTreePath(node);
                      return;
                    }
                    void openWorkspaceFileTab(node);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <FileText aria-hidden="true" className="size-3.5" />
                  <span>{t("Open")}</span>
                </button>
                <button
                  className="workspace-chat-context-menu-item"
                  onClick={() => {
                    const { node } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    const nextName = window.prompt(t("Rename file"), node.name);
                    if (nextName === null) {
                      return;
                    }
                    const trimmedName = nextName.trim();
                    if (!trimmedName || trimmedName === node.name) {
                      return;
                    }
                    void handleWorkspaceFileOperation("rename", node.path, trimmedName);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Pencil aria-hidden="true" className="size-3.5" />
                  <span>{t("Rename")}</span>
                </button>
                <button
                  className="workspace-chat-context-menu-item workspace-chat-context-menu-item-danger"
                  onClick={() => {
                    const { node } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    if (!window.confirm(t("Delete file confirmation"))) {
                      return;
                    }
                    void handleWorkspaceFileOperation("delete", node.path);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Trash2 aria-hidden="true" className="size-3.5" />
                  <span>{t("Delete")}</span>
                </button>
                <button
                  className="workspace-chat-context-menu-item"
                  onClick={() => {
                    const { node } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    void copyWorkspaceFileText(node.name);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Copy aria-hidden="true" className="size-3.5" />
                  <span>{t("Copy file name")}</span>
                </button>
                <button
                  className="workspace-chat-context-menu-item"
                  onClick={() => {
                    const { node } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    void copyWorkspaceFileText(node.path);
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Copy aria-hidden="true" className="size-3.5" />
                  <span>{t("Copy relative path")}</span>
                </button>
                <button
                  className="workspace-chat-context-menu-item"
                  onClick={() => {
                    const { node, workspacePath } = workspaceFileContextMenu;
                    setWorkspaceFileContextMenu(null);
                    void copyWorkspaceFileText(
                      workspaceFileAbsolutePath(workspacePath, node.path),
                    );
                  }}
                  role="menuitem"
                  type="button"
                >
                  <Copy aria-hidden="true" className="size-3.5" />
                  <span>{t("Copy absolute path")}</span>
                </button>
              </div>
            ) : null}

            <section className="app-main-panel flex min-w-0 flex-col">
              <header className="app-toolbar shrink-0 border-b border-stone-200/80 bg-white/80 backdrop-blur">
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <MainTabBar
                    activeTab={activeMainTab}
                    onCloseTab={closeMainTab}
                    onSelectTab={selectMainTab}
                    runningChatKeys={runningChatKeys}
                    tabs={mainTabs}
                  />
                </div>
              </header>
              {activeMainTab.type === "file" && activeFileTab ? (
                <WorkspaceFileEditorPanel
                  editor={activeFileEditor}
                  file={activeFileTab}
                  onChangeContent={updateWorkspaceFileEditorContent}
                  onReload={reloadWorkspaceFileEditor}
                  onSave={saveWorkspaceFileEditor}
                />
              ) : activeMainTab.type === "agent" && activeAgentTab ? (
                <AgentTranscriptPanel
                  error={agentTeamError}
                  helpers={chatPanelHelpers}
                  instanceId={activeAgentTab.instanceId}
                  isLoading={isLoadingAgentTeam}
                  onOpenMainChat={() =>
                    selectWorkspaceChat(activeAgentTab.workspaceId, activeAgentTab.chatId)
                  }
                  onRefresh={async () => {
                    await loadAgentTeamSnapshot(
                      activeAgentTab.workspaceId,
                      activeAgentTab.chatId,
                    );
                  }}
                  snapshot={agentTeamSnapshot}
                  workspaceId={activeAgentTab.workspaceId}
                />
              ) : (
                <ChatPanel
                  activeWorkspaceName={activeWorkspace?.name ?? null}
                  helpers={chatPanelHelpers}
                  availableModels={availableModels}
                  branchError={branchError}
                  chatScrollKey={`${activeWorkspaceId}:${activeChatId ?? ""}`}
                  canGuideActiveRun={
                    activeRunInfo?.chatKey === activeChatKey &&
                    activeRunInfo.runId !== null &&
                    activeRunInfo.acceptingGuidance
                  }
                  canUseNativePicker={canUseNativePicker}
                  draftAttachments={draftAttachments}
                  draftMessage={draftMessage}
                  gitBranches={gitBranches}
                  contextUsage={contextUsage}
                  isLoadingSettings={isLoadingSettings}
                  isLoadingBranches={isLoadingBranches}
                  isLoadingContextUsage={isLoadingContextUsage}
                  isLoadingMessages={isLoadingActiveChatMessages}
                  isSendingMessage={isSendingMessage}
                  isSelectingAttachments={isSelectingAttachments}
                  isTeamModeEnabled={canUseTeamMode && isTeamModeEnabled}
                  messages={messages}
                  readOnly={activeChatReadOnly}
                  overviewRenderer={() => (
                    <ApiOverviewPanel
                      activeWorkspaceId={activeWorkspaceId}
                      autoLoadEnabled={!isPreparingChatRun}
                      settings={settings}
                      workspaces={workspaces}
                    />
                  )}
                  onAddPastedImageAttachments={(files) =>
                    void handleAddPastedImageAttachments(files)
                  }
                  onBranchChange={(branch) => void handleGitBranchChange(branch)}
                  onDraftMessageChange={setDraftMessage}
                  onGuideQueuedMessage={(messageId) =>
                    void handleGuideQueuedMessage(messageId)
                  }
                  onSelectAttachments={(files) =>
                    void handleSelectDraftAttachments(files)
                  }
                  onCancelRun={() => void handleCancelRun()}
                  onGuideActiveRun={() => void handleGuideActiveRun()}
                  onQueueActiveRun={handleQueueActiveRun}
                  onModelChange={handleChatModelChange}
                  onProviderChange={handleChatProviderChange}
                  onRemoveAttachment={handleRemoveDraftAttachment}
                  onRemoveSkill={removeSelectedSkill}
                  onRetryRun={() => void handleRetryRun()}
                  onSubmit={(event, options) =>
                    void handleSendMessage(event, options)
                  }
                  onTeamModeEnabledChange={setIsTeamModeEnabled}
                  onThinkingLevelChange={handleChatThinkingLevelChange}
                  onToggleSkill={toggleSelectedSkill}
                  onWithdrawQueuedMessage={handleWithdrawQueuedMessage}
                  canRetryRun={retryRunRequest !== null && !isSendingMessage}
                  queuedRunCount={queuedRunRequests.length}
                  queuedMessageIds={queuedMessageIds}
                  selectedGitBranch={selectedGitBranch}
                  selectedModelId={selectedModelId}
                  selectedProviderId={selectedProviderId}
                  selectedSkillIds={selectedSkillIds}
                  selectedThinkingLevel={selectedThinkingLevel}
                  settings={settings}
                  showTeamModeToggle={canUseTeamMode}
                  providers={settings?.providers ?? []}
                  skills={detectedSkills}
                  thinkingLevels={thinkingLevels}
                  workspaces={workspaces}
                  workspaceId={activeWorkspace?.id ?? (activeWorkspaceId || null)}
                />
              )}
              {workspaces
                .filter((workspace) => terminalOpenWorkspaceIds.has(workspace.id))
                .map((workspace) => (
                  <TerminalPanel
                    errorMessage={errorMessage}
                    isVisible={workspace.id === activeWorkspace?.id}
                    key={workspace.id}
                    onClose={() => {
                      setTerminalOpenWorkspaceIds((current) => {
                        const next = new Set(current);
                        next.delete(workspace.id);
                        return next;
                      });
                    }}
                    requestJson={requestJson}
                    workspace={workspace}
                  />
                ))}
            </section>

            {showContextPanel ? (
              <aside className="context-sidebar diff-sidebar min-w-0 border-stone-200/80 lg:border-l">
                <div className="relative flex h-full min-h-0 min-w-0 flex-col">
                  <div
                    aria-label={t("Resize context panel")}
                    aria-orientation="vertical"
                    aria-valuemax={CONTEXT_PANEL_MAX_WIDTH}
                    aria-valuemin={CONTEXT_PANEL_MIN_WIDTH}
                    aria-valuenow={diffPanelWidth}
                    className={`context-sidebar-splitter ${isResizingDiffPanel ? "context-sidebar-splitter-active" : ""
                      }`}
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

                      if (event.key === "ArrowUp") {
                        event.preventDefault();
                        setContextPanelMobileHeight((current) =>
                          Math.min(
                            current + 24,
                            Math.floor(window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO),
                          ),
                        );
                      }

                      if (event.key === "ArrowDown") {
                        event.preventDefault();
                        setContextPanelMobileHeight((current) =>
                          Math.max(current - 24, CONTEXT_PANEL_MIN_HEIGHT),
                        );
                      }
                    }}
                    onPointerDown={(event) => {
                      event.preventDefault();
                      if (window.innerWidth < MOBILE_BREAKPOINT_PX) {
                        const maxHeight = Math.floor(
                          window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO,
                        );
                        const nextHeight = window.innerHeight - event.clientY;
                        setContextPanelMobileHeight(
                          Math.min(Math.max(nextHeight, CONTEXT_PANEL_MIN_HEIGHT), maxHeight),
                        );
                      } else {
                        const nextWidth = window.innerWidth - event.clientX;
                        setDiffPanelWidth(
                          Math.min(Math.max(nextWidth, CONTEXT_PANEL_MIN_WIDTH), CONTEXT_PANEL_MAX_WIDTH),
                        );
                      }
                      event.currentTarget.setPointerCapture(event.pointerId);
                      setIsResizingDiffPanel(true);
                    }}
                    role="separator"
                    tabIndex={0}
                  />
                  <ContextPanel
                    activeTab={contextPanelTab}
                    agentsPanel={
                      <AgentsRuntimePanel
                        activeChatId={
                          activeChatId && !isPendingChatId(activeChatId)
                            ? activeChatId
                            : null
                        }
                        error={agentTeamError}
                        isLoading={isLoadingAgentTeam}
                        onRefresh={async () => {
                          if (activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)) {
                            await loadAgentTeamSnapshot(activeWorkspaceId, activeChatId);
                          }
                        }}
                        onSelectInstance={openAgentInstanceTab}
                        selectedInstanceId={
                          activeMainTab.type === "agent"
                            ? activeMainTab.instanceId
                            : agentTeamSnapshot?.team.coordinatorInstanceId ?? null
                        }
                        snapshot={agentTeamSnapshot}
                      />
                    }
                    chatStatistics={displayedChatStatistics}
                    chatStatisticsError={chatStatisticsError}
                    contextMemories={contextMemories}
                    deletingContextMemoryId={deletingContextMemoryId}
                    contextUsage={contextUsage}
                    contextMemoryError={contextMemoryError}
                    diffError={diffError}
                    diffResponse={gitDiff}
                    files={gitDiff?.files ?? []}
                    gitCommitMessage={gitCommitMessage}
                    gitOperationKey={gitOperationKey}
                    expandedFileTreePaths={expandedFileTreePaths}
                    isLoadingChatStatistics={isLoadingChatStatistics}
                    isLoadingDiff={isLoadingDiff}
                    isLoadingContextMemories={isLoadingContextMemories}
                    isLoadingTodoGraph={isLoadingTodoGraph}
                    isLoadingWorkspaceSpec={isLoadingWorkspaceSpec}
                    isLoadingWorkspaceFiles={isLoadingWorkspaceFiles}
                    onGitCommit={handleGitCommit}
                    onGenerateGitCommitMessage={() => void handleGenerateGitCommitMessage()}
                    onGitCommitMessageChange={setGitCommitMessage}
                    onGitFileOperation={(action, path) => void handleGitFileOperation(action, path)}
                    onRefreshWorkspaceFiles={() => {
                      if (activeWorkspace?.id) {
                        void loadWorkspaceFiles(activeWorkspace.id);
                      }
                    }}
                    loadingWorkspaceDirectoryPaths={loadingWorkspaceDirectoryPaths}
                    onToggleFileTreePath={toggleWorkspaceFileTreePath}
                    onOpenWorkspaceFile={(node) => void openWorkspaceFileTab(node)}
                    onOpenWorkspaceFileMenu={(event, node) => {
                      if (!activeWorkspace) {
                        return;
                      }
                      event.preventDefault();
                      event.stopPropagation();
                      setWorkspaceFileContextMenu({
                        left: event.clientX,
                        node,
                        top: event.clientY,
                        workspacePath: activeWorkspace.path,
                      });
                    }}
                    onRefreshDiff={() => {
                      if (activeWorkspace?.id) {
                        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
                      }
                    }}
                    onForgetContextMemory={(memory) => void forgetContextMemory(memory)}
                    onMemoryPageChange={goToContextMemoryPage}
                    onReloadWorkspaceSpec={() => {
                      if (activeWorkspace?.id) {
                        void loadWorkspaceSpec(activeWorkspace.id);
                      }
                    }}
                    onSaveWorkspaceSpec={() => void saveWorkspaceSpecContent()}
                    onGenerateWorkspaceSpec={() => void generateWorkspaceSpec()}
                    onWorkspaceSpecContentChange={setWorkspaceSpecDraft}
                    onWorkspaceSpecPreviewChange={setWorkspaceSpecPreviewEnabled}
                    onWorkspaceSpecSettingsChange={(enabled, injectEnabled) => {
                      if (activeWorkspace?.id) {
                        void saveWorkspaceSpecSettings(
                          activeWorkspace.id,
                          enabled,
                          injectEnabled,
                        );
                      }
                    }}
                    onSelectDiffFile={setSelectedDiffPath}
                    onTabChange={(tab) => {
                      setContextPanelTab(tab);
                      setIsContextPanelOpen(true);
                    }}
                    selectedPath={selectedDiffPath}
                    todoGraph={todoGraph}
                    workspaceSpec={workspaceSpec}
                    workspaceSpecConflictMessage={workspaceSpecConflictMessage}
                    workspaceSpecDraft={workspaceSpecDraft}
                    workspaceSpecError={workspaceSpecError}
                    workspaceSpecOperationKey={workspaceSpecOperationKey}
                    workspaceSpecPreviewEnabled={workspaceSpecPreviewEnabled}
                    workspaceFiles={workspaceFiles}
                    workspaceFileOperationKey={workspaceFileOperationKey}
                    workspaceFilesError={workspaceFilesError}
                    todoGraphError={todoGraphError}
                  />
                </div>
              </aside>
            ) : null}
          </div>
        )}
        {isWorkspaceDialogOpen ? (
          <WorkspaceDialog
            canUseNativePicker={canUseNativePicker}
            iconDraft={workspaceIconDraft}
            iconInputRef={workspaceIconInputRef}
            isSelectingPath={isSelectingWorkspacePath}
            isSaving={isSavingWorkspace}
            name={workspaceName}
            onClearIcon={clearWorkspaceIconDraft}
            onClose={closeWorkspaceDialog}
            onIconFileChange={handleWorkspaceIconFileChange}
            onNameChange={setWorkspaceName}
            onPathChange={setWorkspacePath}
            onSelectPath={handleSelectWorkspacePath}
            onSpecEnabledChange={setWorkspaceSpecEnabled}
            onSubmit={handleWorkspaceSubmit}
            path={workspacePath}
            specEnabled={workspaceSpecEnabled}
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
                            className={`flex cursor-pointer gap-3 rounded-lg border px-3 py-2 text-sm transition ${isSelected
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

function MainTabBar({
  activeTab,
  onCloseTab,
  onSelectTab,
  runningChatKeys,
  tabs,
}: {
  activeTab: ActiveMainTab;
  onCloseTab: (tab: MainTabSummary) => void;
  onSelectTab: (tab: MainTabSummary) => void;
  runningChatKeys: Set<string>;
  tabs: MainTabSummary[];
}) {
  const { t } = useI18n();
  const tabsContainerRef = useRef<HTMLDivElement>(null);
  const tabListRef = useRef<HTMLDivElement>(null);
  const tabItemRefs = useRef(new Map<string, HTMLDivElement>());
  const hasTrackedTabKeysRef = useRef(false);
  const previousTabKeysRef = useRef<string[]>([]);
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

  useLayoutEffect(() => {
    const nextKeys = tabs.map(mainTabKey);
    if (!hasTrackedTabKeysRef.current) {
      hasTrackedTabKeysRef.current = true;
      previousTabKeysRef.current = nextKeys;
      return;
    }

    const previousKeys = new Set(previousTabKeysRef.current);
    const addedKey = nextKeys.find((key) => !previousKeys.has(key));
    previousTabKeysRef.current = nextKeys;

    if (!addedKey) {
      return;
    }

    tabItemRefs.current.get(addedKey)?.scrollIntoView?.({
      block: "nearest",
      inline: "nearest",
    });
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
            const isActive = mainTabMatches(activeTab, tab);
            const isRunning =
              tab.type === "chat" &&
              runningChatKeys.has(chatRunKey(tab.workspaceId, tab.chatId));
            const title = tab.title || t(tab.type === "chat" ? "Chat" : tab.type === "agent" ? "Agent" : "Files");
            const key = mainTabKey(tab);

            return (
              <div
                className={`chat-tab-item group flex h-12 min-w-36 max-w-64 shrink-0 items-center rounded-lg border px-2 py-1.5 transition-colors ${isActive
                    ? "border-teal-200 bg-white text-stone-950 shadow-sm"
                    : "border-stone-200 bg-stone-50/80 text-stone-600 hover:border-stone-300 hover:bg-white"
                  }`}
                key={key}
                ref={(element) => {
                  if (element) {
                    tabItemRefs.current.set(key, element);
                  } else {
                    tabItemRefs.current.delete(key);
                  }
                }}
              >
                <button
                  aria-selected={isActive}
                  className="min-w-0 flex-1 text-left"
                  onClick={() => onSelectTab(tab)}
                  role="tab"
                  title={title}
                  type="button"
                >
                  <span className="flex min-w-0 items-center gap-1.5 truncate text-sm font-semibold leading-5">
                    {tab.type === "file" ? (
                      <FileText aria-hidden="true" className="size-3.5 shrink-0 text-slate-500" />
                    ) : null}
                    {tab.type === "agent" ? (
                      <Bot aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
                    ) : null}
                    <span className="min-w-0 truncate">{title}</span>
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
                        className="chat-tab-running-spinner size-4 animate-spin text-teal-700"
                      />
                    </span>
                  ) : (
                    <button
                      aria-label={t("Close chat tab {title}", { title })}
                      className="inline-flex size-7 items-center justify-center rounded-md text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100 max-[767px]:opacity-100 max-[767px]:focus:opacity-100 max-[767px]:group-hover:opacity-100"
                      onClick={() => onCloseTab(tab)}
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

type NavRailAction = {
  active: boolean;
  disabled?: boolean;
  icon: (props: { className?: string; "aria-hidden"?: boolean | "true" | "false" }) => ReactNode;
  label: string;
  onClick: () => void;
};

function ResponsiveContextPanelIcon({
  className,
}: {
  className?: string;
}) {
  return (
    <>
      <PanelRight aria-hidden="true" className={`${className ?? ""} hidden md:block`} />
      <PanelBottom aria-hidden="true" className={`${className ?? ""} md:hidden`} />
    </>
  );
}

function FocoNavRail({
  activeMode,
  canLogout,
  contextPanelButton,
  isSavingTheme,
  onAddWorkspace,
  onLogout,
  onHomeClick,
  onOpenScheduledTasks,
  onOpenSettings,
  onOpenStats,
  onReturnHome,
  onToggleTheme,
  terminalButton,
  theme,
}: {
  activeMode: ViewMode;
  canLogout: boolean;
  contextPanelButton: NavRailAction | null;
  isSavingTheme: boolean;
  onAddWorkspace: () => void;
  onLogout: () => Promise<void>;
  onHomeClick: () => void;
  onOpenScheduledTasks: () => void;
  onOpenSettings: () => void;
  onOpenStats: () => void;
  onReturnHome: () => void;
  onToggleTheme: () => void;
  terminalButton: NavRailAction | null;
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
          onClick={onHomeClick}
        />
        <NavRailButton
          active={activeMode === "stats"}
          icon={Activity}
          label={t("API details")}
          onClick={onOpenStats}
        />
        <NavRailButton
          active={activeMode === "scheduled"}
          icon={CalendarClock}
          label={t("Scheduled tasks")}
          onClick={onOpenScheduledTasks}
        />
        <NavRailButton
          active={activeMode === "settings"}
          icon={Settings}
          label={t("Settings")}
          onClick={onOpenSettings}
        />
      </div>
      <div className="foco-nav-rail-bottom">
        {terminalButton ? <NavRailButton {...terminalButton} /> : null}
        {contextPanelButton ? <NavRailButton {...contextPanelButton} /> : null}
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
}: NavRailAction) {
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

function ApiOverviewPanel({
  activeWorkspaceId,
  autoLoadEnabled,
  settings,
  workspaces,
}: {
  activeWorkspaceId: string;
  autoLoadEnabled: boolean;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const initialWorkspaceId = preferredOverviewWorkspaceId(
    activeWorkspaceId,
    workspaces,
  );
  const [filters, setFilters] = useState({
    startedAfter: "",
    startedBefore: "",
    workspaceId: initialWorkspaceId,
  });
  const [hasAppliedInitialWorkspace, setHasAppliedInitialWorkspace] =
    useState(initialWorkspaceId !== "");
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const activeOverviewRequestRef = useRef<AbortController | null>(null);
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
  const preferredWorkspaceId = preferredOverviewWorkspaceId(
    activeWorkspaceId,
    workspaces,
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
    if (
      !autoLoadEnabled ||
      settings === null ||
      workspaces.length === 0 ||
      !hasAppliedInitialWorkspace
    ) {
      return;
    }

    activeOverviewRequestRef.current?.abort();
    const controller = new AbortController();
    activeOverviewRequestRef.current = controller;
    setIsLoading(true);
    setError(null);

    try {
      const query = aiOverviewQuery(filters);
      const data = await requestJson<AiStatisticsResponse>(
        `/api/ai-statistics${query ? `?${query}` : ""}`,
        { signal: controller.signal },
      );
      if (controller.signal.aborted) {
        return;
      }
      setStats(data);
    } catch (requestError) {
      if (controller.signal.aborted) {
        return;
      }
      setError(errorMessage(requestError));
    } finally {
      if (activeOverviewRequestRef.current === controller) {
        activeOverviewRequestRef.current = null;
        setIsLoading(false);
      }
    }
  }, [
    autoLoadEnabled,
    filters,
    hasAppliedInitialWorkspace,
    settings,
    workspaces.length,
  ]);

  useEffect(() => {
    if (autoLoadEnabled) {
      return;
    }

    activeOverviewRequestRef.current?.abort();
  }, [autoLoadEnabled]);

  useEffect(
    () => () => {
      activeOverviewRequestRef.current?.abort();
    },
    [],
  );

  useEffect(() => {
    if (hasAppliedInitialWorkspace || preferredWorkspaceId === "") {
      return;
    }

    setFilters((current) => ({
      ...current,
      workspaceId: preferredWorkspaceId,
    }));
    setHasAppliedInitialWorkspace(true);
  }, [hasAppliedInitialWorkspace, preferredWorkspaceId]);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  function updateOverviewFilters(update: Partial<typeof filters>) {
    setHasAppliedInitialWorkspace(true);
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
  onRoutePageChange,
  routePage,
  settings,
  workspaces,
}: {
  onRoutePageChange: (page: number) => void;
  routePage: number;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const {
    closeRequestDetail,
    copiedKey,
    copyAuditText,
    detail,
    detailError,
    error,
    filters,
    goToAuditPage: updateAuditPage,
    isLoading,
    isLoadingDetail,
    loadStats,
    openRequestDetail,
    selectedRequestId,
    setAuditPage,
    stats,
    updateAuditFilters,
  } = useAiStatisticsData(routePage);
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
    settings?.configuredModels
      .map((model) => ({
        label: model.displayName,
        value: model.id,
      }))
      .sort((left, right) => left.label.localeCompare(right.label)) ?? [],
    requests.map((request) => request.modelId),
  );
  const statusOptions = auditOptions(
    ["succeeded", "failed", "running", "cancelled", "completed"].map((status) => ({
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

  function goToAuditPage(page: number) {
    const nextPage = updateAuditPage(page, totalPages);
    onRoutePageChange(nextPage);
  }

  function updateFilters(update: Partial<AiStatsFilterState>) {
    updateAuditFilters(update);
    onRoutePageChange(1);
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

  useEffect(() => {
    writeAiStatsVisibleColumnIds(visibleColumnIds);
  }, [visibleColumnIds]);

  useEffect(() => {
    setAuditPage(routePage);
  }, [routePage, setAuditPage]);

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
                updateFilters({
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
              onChange={(value) => updateFilters({ chatId: value })}
              options={chatOptions}
              placeholder={t("All chats")}
              value={filters.chatId}
            />
            <FilterSelect
              label={t("Provider")}
              onChange={(value) => updateFilters({ providerId: value })}
              options={providerOptions}
              placeholder={t("All providers")}
              value={filters.providerId}
            />
            <FilterSelect
              label={t("Model")}
              onChange={(value) => updateFilters({ modelId: value })}
              options={modelOptions}
              placeholder={t("All models")}
              value={filters.modelId}
            />
            <FilterSelect
              label={t("Status")}
              onChange={(value) => updateFilters({ status: value })}
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
                  updateFilters({
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
                  updateFilters({
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
          <div
            className="panel-scroll min-w-0 overflow-x-auto"
            onWheel={(event) => {
              // overflow-x-auto forces overflow-y to compute to auto in Chromium,
              // so this container traps vertical wheel events even though the table
              // never overflows vertically. Forward them to the scrollable ancestor.
              if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) {
                return;
              }
              const deltaUnit =
                event.deltaMode === 1
                  ? 16
                  : event.deltaMode === 2
                    ? event.currentTarget.clientHeight
                    : 1;
              let node: HTMLElement | null = event.currentTarget.parentElement;
              while (node) {
                const overflowY = window.getComputedStyle(node).overflowY;
                if (
                  /(auto|scroll)/.test(overflowY) &&
                  node.scrollHeight > node.clientHeight
                ) {
                  node.scrollTop += event.deltaY * deltaUnit;
                  event.preventDefault();
                  return;
                }
                node = node.parentElement;
              }
            }}
          >
            <table className="w-full min-w-max text-left text-sm">
              <thead className="border-b border-stone-200 bg-white text-xs font-semibold text-stone-500">
                <tr>
                  {visibleColumns.map((column) => (
                    <th
                      className={`whitespace-nowrap px-4 py-3 ${column.headerClassName ?? ""
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
                    updateFilters({ pageSize: event.target.value })
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
                      className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === currentPage
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
          onClose={closeRequestDetail}
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
            className="audit-json-icon-button"
            onClick={onCopy}
            title={t("Copy {label}", { label })}
            type="button"
          >
            {copied ? (
              <CheckCircle2 aria-hidden="true" className="size-3.5" />
            ) : (
              <Copy aria-hidden="true" className="size-3.5" />
            )}
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
    <article className="rounded-2xl border border-stone-200/80 bg-white p-5 shadow-sm">
      <div className="flex items-center gap-2 text-sm font-semibold text-stone-600">
        <Icon aria-hidden="true" className="size-4" />
        <span>{label}</span>
      </div>
      <div className="mt-4 font-mono text-3xl font-semibold text-stone-950">
        {value}
      </div>
    </article>
  );
}

function ContextPanel({
  activeTab,
  agentsPanel,
  chatStatistics,
  chatStatisticsError,
  contextMemories,
  contextUsage,
  deletingContextMemoryId,
  contextMemoryError,
  diffError,
  diffResponse,
  files,
  gitCommitMessage,
  gitOperationKey,
  expandedFileTreePaths,
  isLoadingChatStatistics,
  isLoadingContextMemories,
  loadingWorkspaceDirectoryPaths,
  isLoadingDiff,
  isLoadingTodoGraph,
  isLoadingWorkspaceSpec,
  isLoadingWorkspaceFiles,
  onForgetContextMemory,
  onGenerateGitCommitMessage,
  onGenerateWorkspaceSpec,
  onGitCommit,
  onGitCommitMessageChange,
  onGitFileOperation,
  onMemoryPageChange,
  onReloadWorkspaceSpec,
  onRefreshDiff,
  onRefreshWorkspaceFiles,
  onSaveWorkspaceSpec,
  onToggleFileTreePath,
  onOpenWorkspaceFile,
  onOpenWorkspaceFileMenu,
  onSelectDiffFile,
  onTabChange,
  onWorkspaceSpecContentChange,
  onWorkspaceSpecPreviewChange,
  onWorkspaceSpecSettingsChange,
  selectedPath,
  todoGraph,
  todoGraphError,
  workspaceSpec,
  workspaceSpecConflictMessage,
  workspaceSpecDraft,
  workspaceSpecError,
  workspaceSpecOperationKey,
  workspaceSpecPreviewEnabled,
  workspaceFiles,
  workspaceFileOperationKey,
  workspaceFilesError,
}: {
  activeTab: ContextPanelTab;
  agentsPanel: ReactNode;
  chatStatistics: ChatStatisticsResponse | null;
  chatStatisticsError: string | null;
  contextMemories: ContextMemoryState;
  contextUsage: ContextUsageResponse | null;
  deletingContextMemoryId: string | null;
  contextMemoryError: string | null;
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  gitCommitMessage: string;
  gitOperationKey: string | null;
  expandedFileTreePaths: Set<string>;
  isLoadingChatStatistics: boolean;
  isLoadingContextMemories: boolean;
  loadingWorkspaceDirectoryPaths: Set<string>;
  isLoadingDiff: boolean;
  isLoadingTodoGraph: boolean;
  isLoadingWorkspaceSpec: boolean;
  isLoadingWorkspaceFiles: boolean;
  onForgetContextMemory: (memory: MemoryFactRecord) => void;
  onGenerateGitCommitMessage: () => void;
  onGenerateWorkspaceSpec: () => void;
  onGitCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGitCommitMessageChange: (message: string) => void;
  onGitFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onMemoryPageChange: (scope: "global" | "workspace", page: number) => void;
  onReloadWorkspaceSpec: () => void;
  onRefreshDiff: () => void;
  onRefreshWorkspaceFiles: () => void;
  onSaveWorkspaceSpec: () => void;
  onToggleFileTreePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  onOpenWorkspaceFile: (node: WorkspaceFileTreeNode) => void;
  onOpenWorkspaceFileMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onSelectDiffFile: (path: string | null) => void;
  onTabChange: (tab: ContextPanelTab) => void;
  onWorkspaceSpecContentChange: (content: string) => void;
  onWorkspaceSpecPreviewChange: (enabled: boolean) => void;
  onWorkspaceSpecSettingsChange: (enabled: boolean, injectEnabled: boolean) => void;
  selectedPath: string | null;
  todoGraph: TodoGraphResponse | null;
  todoGraphError: string | null;
  workspaceSpec: WorkspaceSpecResponse | null;
  workspaceSpecConflictMessage: string | null;
  workspaceSpecDraft: string;
  workspaceSpecError: string | null;
  workspaceSpecOperationKey: "generate" | "save" | "settings" | null;
  workspaceSpecPreviewEnabled: boolean;
  workspaceFiles: WorkspaceFilesResponse | null;
  workspaceFileOperationKey: string | null;
  workspaceFilesError: string | null;
}) {
  const { t } = useI18n();
  const tabs: { id: ContextPanelTab; label: string; icon: LucideIcon }[] = [
    { id: "todo", label: "ToDo", icon: ListChecks },
    { id: "files", label: "Files", icon: Files },
    { id: "git", label: "Git", icon: GitCompare },
    { id: "agents", label: "Agents", icon: Bot },
    { id: "memory", label: "Memory", icon: Brain },
    { id: "spec", label: "Spec", icon: ScrollText },
    { id: "stats", label: "Stats", icon: BarChart3 },
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

        {activeTab === "files" ? (
          <WorkspaceFilesTab
            error={workspaceFilesError}
            expandedPaths={expandedFileTreePaths}
            isLoading={isLoadingWorkspaceFiles}
            operationKey={workspaceFileOperationKey}
            loadingPaths={loadingWorkspaceDirectoryPaths}
            onOpenFile={onOpenWorkspaceFile}
            onOpenContextMenu={onOpenWorkspaceFileMenu}
            onRefresh={onRefreshWorkspaceFiles}
            onTogglePath={onToggleFileTreePath}
            response={workspaceFiles}
          />
        ) : null}

        {activeTab === "git" ? (
          <div className="flex min-h-0 flex-1 flex-col">
            <SourceControlPanel
              diffError={diffError}
              diffResponse={diffResponse}
              files={files}
              gitCommitMessage={gitCommitMessage}
              gitOperationKey={gitOperationKey}
              isLoading={isLoadingDiff}
              onCommit={onGitCommit}
              onGenerateCommitMessage={onGenerateGitCommitMessage}
              onCommitMessageChange={onGitCommitMessageChange}
              onFileOperation={onGitFileOperation}
              onRefresh={onRefreshDiff}
              onSelectFile={onSelectDiffFile}
              selectedPath={selectedPath}
            />
          </div>
        ) : null}

        {activeTab === "agents" ? agentsPanel : null}

        {activeTab === "memory" ? (
          <ContextMemoryTab
            deletingMemoryId={deletingContextMemoryId}
            error={contextMemoryError}
            isLoading={isLoadingContextMemories}
            memories={contextMemories}
            onForgetMemory={onForgetContextMemory}
            onPageChange={onMemoryPageChange}
          />
        ) : null}

        {activeTab === "spec" ? (
          <ContextSpecTab
            conflictMessage={workspaceSpecConflictMessage}
            contentDraft={workspaceSpecDraft}
            error={workspaceSpecError}
            isLoading={isLoadingWorkspaceSpec}
            onContentChange={onWorkspaceSpecContentChange}
            onGenerate={onGenerateWorkspaceSpec}
            onPreviewChange={onWorkspaceSpecPreviewChange}
            onReload={onReloadWorkspaceSpec}
            onSave={onSaveWorkspaceSpec}
            onSettingsChange={onWorkspaceSpecSettingsChange}
            operationKey={workspaceSpecOperationKey}
            previewEnabled={workspaceSpecPreviewEnabled}
            spec={workspaceSpec}
          />
        ) : null}

        {activeTab === "stats" ? (
          <ContextStatsTab
            contextUsage={contextUsage}
            error={chatStatisticsError}
            isLoading={isLoadingChatStatistics}
            statistics={chatStatistics}
          />
        ) : null}
      </div>
    </section>
  );
}

function WorkspaceFileEditorPanel({
  editor,
  file,
  onChangeContent,
  onReload,
  onSave,
}: {
  editor: WorkspaceFileEditorState | null;
  file: OpenFileTab;
  onChangeContent: (workspaceId: string, path: string, content: string) => void;
  onReload: (file: OpenFileTab) => Promise<void>;
  onSave: (file: OpenFileTab, content: string) => Promise<boolean> | boolean;
}) {
  const { t } = useI18n();
  const language = monacoLanguageForPath(file.path);
  const isMarkdown = isMarkdownFilePath(file.path);
  const editorPath = `${file.workspaceId}/${file.path}`;
  const handleChange = useCallback(
    (content: string) => onChangeContent(file.workspaceId, file.path, content),
    [file.path, file.workspaceId, onChangeContent],
  );
  const handleReload = useCallback(() => onReload(file), [file, onReload]);
  const handleSave = useCallback(
    (content: string) => onSave(file, content),
    [file, onSave],
  );

  return (
    <section className="workspace-file-editor flex min-h-0 flex-1 flex-col">
      {editor?.error ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {editor.error}
        </div>
      ) : null}
      <div className="workspace-file-editor-body">
        <MonacoFileEditor
          canSave={!editor?.isLoading && !editor?.isSaving}
          isDirty={editor?.isDirty ?? false}
          isMarkdown={isMarkdown}
          isSaving={editor?.isSaving ?? false}
          language={language}
          onChange={handleChange}
          onReload={handleReload}
          onSave={handleSave}
          path={editorPath}
          value={editor?.content ?? ""}
        />
      </div>
    </section>
  );
}

type MonacoFileEditorCommand =
  | "save"
  | "cut"
  | "copy"
  | "paste"
  | "undo"
  | "redo"
  | "find"
  | "toggleWordWrap";

function MonacoFileEditor({
  canSave,
  isDirty,
  isMarkdown,
  isSaving,
  language,
  onChange,
  onReload,
  onSave,
  path,
  value,
}: {
  canSave: boolean;
  isDirty: boolean;
  isMarkdown: boolean;
  isSaving: boolean;
  language: string;
  onChange: (value: string) => void;
  onReload: () => Promise<void>;
  onSave: (value: string) => Promise<boolean> | boolean;
  path: string;
  value: string;
}) {
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const ignoreModelChangeRef = useRef(false);
  const valueRef = useRef(value);
  const [previewEnabled, setPreviewEnabled] = useState(false);
  const [wordWrapEnabled, setWordWrapEnabled] = useState(false);
  const [isReloadConfirmOpen, setIsReloadConfirmOpen] = useState(false);
  const [isReloading, setIsReloading] = useState(false);
  const reloadConfirmTitleId = useId();
  const reloadConfirmDescriptionId = useId();

  useEffect(() => {
    valueRef.current = value;
  }, [value]);

  useEffect(() => {
    setPreviewEnabled(false);
  }, [path]);

  useEffect(() => {
    if (!previewEnabled) {
      window.setTimeout(() => editorRef.current?.layout(), 0);
    }
  }, [previewEnabled]);

  const focusEditor = useCallback(() => {
    editorRef.current?.focus();
  }, []);

  const reloadFile = useCallback(async () => {
    if (isReloading) {
      return;
    }

    setIsReloading(true);
    try {
      await onReload();
    } finally {
      setIsReloading(false);
      editorRef.current?.focus();
    }
  }, [isReloading, onReload]);

  const handleReloadClick = useCallback(() => {
    if (!isDirty) {
      void reloadFile();
      return;
    }

    setIsReloadConfirmOpen(true);
  }, [isDirty, reloadFile]);

  const handleReloadConfirm = useCallback(
    async (action: "save" | "discard" | "cancel") => {
      if (action === "cancel") {
        setIsReloadConfirmOpen(false);
        editorRef.current?.focus();
        return;
      }

      setIsReloadConfirmOpen(false);
      if (action === "save") {
        const saveResult = await onSave(editorRef.current?.getValue() ?? valueRef.current);
        if (saveResult === false) {
          editorRef.current?.focus();
          return;
        }
      }

      await reloadFile();
    },
    [onSave, reloadFile],
  );

  const runEditorCommand = useCallback(
    (command: MonacoFileEditorCommand) => {
      if (command === "save") {
        if (canSave) {
          onSave(editorRef.current?.getValue() ?? valueRef.current);
        }
        editorRef.current?.focus();
        return;
      }

      const editor = editorRef.current;
      if (!editor) {
        return;
      }

      if (command === "toggleWordWrap") {
        const nextEnabled = !wordWrapEnabled;
        editor.updateOptions({ wordWrap: nextEnabled ? "on" : "off" });
        setWordWrapEnabled(nextEnabled);
        editor.focus();
        return;
      }

      const commandIdByAction: Record<Exclude<MonacoFileEditorCommand, "save" | "toggleWordWrap">, string> = {
        copy: "editor.action.clipboardCopyAction",
        cut: "editor.action.clipboardCutAction",
        find: "actions.find",
        paste: "editor.action.clipboardPasteAction",
        redo: "redo",
        undo: "undo",
      };
      editor.trigger("workspace-file-toolbar", commandIdByAction[command], null);
      editor.focus();
    },
    [canSave, onSave, wordWrapEnabled],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }

    let disposed = false;
    let cleanupEditor: (() => void) | null = null;

    void import("monaco-editor").then((monaco) => {
      if (disposed) {
        return;
      }

      registerTomlMonacoLanguage(monaco);

      const model = monaco.editor.createModel(
        valueRef.current,
        language,
        monaco.Uri.parse(`file:///${path}`),
      );
      const editor = monaco.editor.create(container, {
        automaticLayout: true,
        fontSize: 13,
        language,
        minimap: { enabled: true },
        model,
        readOnly: false,
        scrollBeyondLastLine: false,
        theme: "vs",
        wordWrap: wordWrapEnabled ? "on" : "off",
      });
      const changeDisposable = model.onDidChangeContent(() => {
        if (!ignoreModelChangeRef.current) {
          onChange(model.getValue());
        }
      });
      editor.addCommand(
        monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
        () => {
          onSave(editor.getValue());
        },
      );
      editorRef.current = editor;
      modelRef.current = model;
      cleanupEditor = () => {
        changeDisposable.dispose();
        editor.dispose();
        model.dispose();
      };
    });

    return () => {
      disposed = true;
      cleanupEditor?.();
      editorRef.current = null;
      modelRef.current = null;
    };
  }, [language, onChange, onSave, path]);

  useEffect(() => {
    const model = modelRef.current;
    if (!model || model.getValue() === value) {
      return;
    }
    ignoreModelChangeRef.current = true;
    model.setValue(value);
    ignoreModelChangeRef.current = false;
  }, [value]);

  return (
    <div className="workspace-file-editor-shell">
      <div aria-label={t("Editor toolbar")} className="workspace-file-editor-toolbar" role="toolbar">
        <EditorToolbarButton
          disabled={!canSave || isReloading}
          icon={RefreshCw}
          label={t("Reload file")}
          onClick={handleReloadClick}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={!canSave || isSaving}
          icon={Save}
          isActive={isDirty}
          label={t("Save")}
          onClick={() => runEditorCommand("save")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Scissors}
          label={t("Cut")}
          onClick={() => runEditorCommand("cut")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Copy}
          label={t("Copy")}
          onClick={() => runEditorCommand("copy")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={ClipboardPaste}
          label={t("Paste")}
          onClick={() => runEditorCommand("paste")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Undo2}
          label={t("Undo")}
          onClick={() => runEditorCommand("undo")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Redo2}
          label={t("Redo")}
          onClick={() => runEditorCommand("redo")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Search}
          label={t("Find")}
          onClick={() => runEditorCommand("find")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={WrapText}
          isActive={wordWrapEnabled}
          label={t("Word wrap")}
          onClick={() => runEditorCommand("toggleWordWrap")}
        />
        {isMarkdown ? (
          <>
            <span className="workspace-file-editor-toolbar-separator" />
            <EditorToolbarButton
              icon={previewEnabled ? EyeOff : Eye}
              isActive={previewEnabled}
              label={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
              onClick={() => setPreviewEnabled((current) => !current)}
            />
          </>
        ) : null}
      </div>
      <div
        aria-hidden={previewEnabled || undefined}
        className={`workspace-file-monaco ${previewEnabled ? "workspace-file-monaco-hidden" : ""}`}
        onMouseDown={focusEditor}
        ref={containerRef}
      />
      {isMarkdown && previewEnabled ? (
        <div className="workspace-file-markdown-preview">
          <MarkdownContent
            content={value}
            isUser={false}
            selectedSkillPrefix={selectedSkillPrefix}
          />
        </div>
      ) : null}
      {isReloadConfirmOpen ? (
        <div className="workspace-file-reload-dialog-backdrop">
          <div
            aria-describedby={reloadConfirmDescriptionId}
            aria-labelledby={reloadConfirmTitleId}
            aria-modal="true"
            className="workspace-file-reload-dialog"
            role="dialog"
          >
            <h2 id={reloadConfirmTitleId}>{t("Reload file")}</h2>
            <p id={reloadConfirmDescriptionId}>{t("Save changes before reloading this file?")}</p>
            <div className="workspace-file-reload-dialog-actions">
              <button
                className="rounded-lg bg-stone-900 px-3 py-2 text-sm font-semibold text-white hover:bg-stone-800"
                onClick={() => void handleReloadConfirm("save")}
                type="button"
              >
                {t("Yes")}
              </button>
              <button
                className="rounded-lg border border-stone-300 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                onClick={() => void handleReloadConfirm("discard")}
                type="button"
              >
                {t("No")}
              </button>
              <button
                className="rounded-lg border border-stone-300 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                onClick={() => void handleReloadConfirm("cancel")}
                type="button"
              >
                {t("Cancel")}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function EditorToolbarButton({
  disabled = false,
  icon: Icon,
  isActive = false,
  label,
  onClick,
}: {
  disabled?: boolean;
  icon: LucideIcon;
  isActive?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      aria-pressed={isActive || undefined}
      className={`workspace-file-editor-toolbar-button ${isActive ? "workspace-file-editor-toolbar-button-active" : ""}`}
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function WorkspaceFilesTab({
  error,
  expandedPaths,
  isLoading,
  loadingPaths,
  operationKey,
  onOpenFile,
  onOpenContextMenu,
  onRefresh,
  onTogglePath,
  response,
}: {
  error: string | null;
  expandedPaths: Set<string>;
  isLoading: boolean;
  loadingPaths: Set<string>;
  operationKey: string | null;
  onOpenFile: (node: WorkspaceFileTreeNode) => void;
  onOpenContextMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onRefresh: () => void;
  onTogglePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  response: WorkspaceFilesResponse | null;
}) {
  const { t } = useI18n();

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-sky-50 text-sky-800">
            <Files aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Files")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {t("Workspace file tree")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Refresh files")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-600 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isLoading}
          onClick={onRefresh}
          title={t("Refresh files")}
          type="button"
        >
          <RefreshCw aria-hidden="true" className={`size-4 ${isLoading ? "animate-spin" : ""}`} />
        </button>
      </div>

      {error ? (
        <div className="mx-4 mt-3 rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-medium text-rose-700">
          {error}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
        {response ? (
          <div className="workspace-file-tree" role="tree">
            <WorkspaceFileTreeNodeRow
              depth={0}
              expandedPaths={expandedPaths}
              loadingPaths={loadingPaths}
              node={response.root}
              onOpenFile={onOpenFile}
              onOpenContextMenu={onOpenContextMenu}
              onTogglePath={onTogglePath}
              operationKey={operationKey}
            />
          </div>
        ) : (
          <div className="rounded-xl border border-dashed border-stone-300 bg-white/60 px-3 py-4 text-sm text-stone-500">
            {isLoading ? t("Loading files...") : t("No files")}
          </div>
        )}
      </div>
    </div>
  );
}

function WorkspaceFileTreeNodeRow({
  depth,
  expandedPaths,
  loadingPaths,
  node,
  onOpenFile,
  onOpenContextMenu,
  onTogglePath,
  operationKey,
}: {
  depth: number;
  expandedPaths: Set<string>;
  loadingPaths: Set<string>;
  node: WorkspaceFileTreeNode;
  onOpenFile: (node: WorkspaceFileTreeNode) => void;
  onOpenContextMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onTogglePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  operationKey: string | null;
}) {
  const { t } = useI18n();
  const isDirectory = node.kind === "directory";
  const isExpanded = expandedPaths.has(node.path);
  const isBusy = operationKey === `delete:${node.path}` || operationKey === `rename:${node.path}`;
  const isLoadingDirectory = loadingPaths.has(node.path);

  return (
    <div role="none">
      <div
        aria-expanded={isDirectory ? isExpanded : undefined}
        className="workspace-file-tree-row"
        onContextMenu={(event) => {
          if (node.path) {
            onOpenContextMenu(event, node);
          }
        }}
        onClick={() => {
          if (isDirectory) {
            void onTogglePath(node);
            return;
          }
          onOpenFile(node);
        }}
        role="treeitem"
        style={{ paddingLeft: `${depth * 0.875 + 0.25}rem` }}
      >
        <button
          aria-label={isExpanded ? t("Collapse folder") : t("Expand folder")}
          className="workspace-file-tree-toggle"
          disabled={!isDirectory}
          onClick={(event) => {
            event.stopPropagation();
            if (isDirectory) {
              void onTogglePath(node);
            }
          }}
          tabIndex={isDirectory ? 0 : -1}
          type="button"
        >
          {isDirectory ? (
            isExpanded ? (
              <ChevronDown aria-hidden="true" className="size-3.5" />
            ) : (
              <ChevronRight aria-hidden="true" className="size-3.5" />
            )
          ) : null}
        </button>
        {isDirectory ? (
          <Folder aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-folder-icon" />
        ) : (
          <FileText aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-file-icon" />
        )}
        <span className="workspace-file-tree-name" title={node.path || node.name}>
          {node.name}
        </span>
        {isBusy || isLoadingDirectory ? <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin text-stone-400" /> : null}
        {!isDirectory ? (
          <span className="workspace-file-tree-size">{formatFileSize(node.sizeBytes)}</span>
        ) : null}
      </div>
      {isDirectory && isExpanded
        ? node.children.map((child) => (
          <WorkspaceFileTreeNodeRow
            depth={depth + 1}
            expandedPaths={expandedPaths}
            key={child.path || child.name}
            loadingPaths={loadingPaths}
            node={child}
            onOpenFile={onOpenFile}
            onOpenContextMenu={onOpenContextMenu}
            onTogglePath={onTogglePath}
            operationKey={operationKey}
          />
        ))
        : null}
    </div>
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
            <h2 className="truncate text-sm font-semibold">{t("ToDo graph")}</h2>
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
  onPageChange,
}: {
  deletingMemoryId: string | null;
  error: string | null;
  isLoading: boolean;
  memories: ContextMemoryState;
  onForgetMemory: (memory: MemoryFactRecord) => void;
  onPageChange: (scope: "global" | "workspace", page: number) => void;
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
            memories={memories.global.memories}
            meta={{
              page: memories.global.page,
              pageSize: memories.global.pageSize,
              totalCount: memories.global.totalCount,
              totalPages: memories.global.totalPages,
            }}
            onForgetMemory={onForgetMemory}
            onPageChange={(page) => onPageChange("global", page)}
          />
          <ContextMemoryGroup
            deletingMemoryId={deletingMemoryId}
            emptyLabel={t("No memories")}
            label={t("Workspace memory")}
            memories={memories.workspace.memories}
            meta={{
              page: memories.workspace.page,
              pageSize: memories.workspace.pageSize,
              totalCount: memories.workspace.totalCount,
              totalPages: memories.workspace.totalPages,
            }}
            onForgetMemory={onForgetMemory}
            onPageChange={(page) => onPageChange("workspace", page)}
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
  meta,
  memories,
  onForgetMemory,
  onPageChange,
}: {
  deletingMemoryId: string | null;
  emptyLabel: string;
  label: string;
  meta: { page: number; pageSize: number; totalCount: number; totalPages: number };
  memories: MemoryFactRecord[];
  onForgetMemory: (memory: MemoryFactRecord) => void;
  onPageChange: (page: number) => void;
}) {
  const { language, t } = useI18n();
  const paginationItems = auditPaginationItems(meta.page, meta.totalPages);

  return (
    <div className="context-memory-group">
      <div className="context-panel-section-title">{label}</div>
      {memories.length ? (
        <>
          {memories.map((memory) => (
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
                {memory.scope} 路 {formatTodoGraphDate(memory.updatedAt)}
              </small>
            </article>
          ))}
          {meta.totalPages > 1 ? (
            <div className="context-memory-pagination-shell">
              <nav
                aria-label={t("Memory pagination")}
                className="context-memory-pagination"
              >
                <button
                  aria-label={t("Previous page")}
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={meta.page <= 1}
                  onClick={() => onPageChange(meta.page - 1)}
                  title={t("Previous page")}
                  type="button"
                >
                  <ChevronLeft aria-hidden="true" className="size-4" />
                </button>
                {paginationItems.map((item, index) =>
                  item === "ellipsis" ? (
                    <span
                      aria-hidden="true"
                      className="context-memory-pagination-control context-memory-pagination-ellipsis inline-flex items-center justify-center text-stone-400"
                      key={`cm-ellipsis-${index}`}
                    >
                      ...
                    </span>
                  ) : (
                    <button
                      aria-current={
                        item === meta.page ? "page" : undefined
                      }
                      aria-label={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      className={`context-memory-pagination-control inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                        item === meta.page
                          ? "border-teal-700 bg-teal-700 text-white"
                          : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        }`}
                      key={item}
                      onClick={() => onPageChange(item)}
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
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={meta.totalPages === 0 || meta.page >= meta.totalPages}
                  onClick={() => onPageChange(meta.page + 1)}
                  title={t("Next page")}
                  type="button"
                >
                  <ChevronRight aria-hidden="true" className="size-4" />
                </button>
              </nav>
            </div>
          ) : null}
        </>
      ) : (
        <div className="context-empty-inline">{emptyLabel}</div>
      )}
    </div>
  );
}

function ContextSpecTab({
  conflictMessage,
  contentDraft,
  error,
  isLoading,
  onContentChange,
  onGenerate,
  onPreviewChange,
  onReload,
  onSave,
  onSettingsChange,
  operationKey,
  previewEnabled,
  spec,
}: {
  conflictMessage: string | null;
  contentDraft: string;
  error: string | null;
  isLoading: boolean;
  onContentChange: (content: string) => void;
  onGenerate: () => void;
  onPreviewChange: (enabled: boolean) => void;
  onReload: () => void;
  onSave: () => void;
  onSettingsChange: (enabled: boolean, injectEnabled: boolean) => void;
  operationKey: "generate" | "save" | "settings" | null;
  previewEnabled: boolean;
  spec: WorkspaceSpecResponse | null;
}) {
  const { language, t } = useI18n();
  const enabled = spec?.settings.enabled ?? false;
  const injectEnabled = spec?.settings.injectEnabled ?? false;
  const isDirty = spec !== null && contentDraft !== spec.contentMarkdown;
  const isBusy = operationKey !== null;
  const latestJob = spec?.latestJob ?? null;
  const canEdit = enabled && spec !== null;
  const generateLabel = contentDraft.trim()
    ? t("Regenerate spec")
    : t("Generate spec");

  if (isLoading && !spec) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Project Spec")}</h2>
        <p>{t("Loading...")}</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[#f8f8f8]">
      <div className="flex min-h-[var(--foco-header-height)] items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-teal-50 text-teal-800">
            <ScrollText aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Project Spec")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {spec
                ? `${t("Revision")} ${formatNumber(spec.revision, language)}`
                : t("Workspace spec")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Reload spec")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-stone-600 hover:bg-stone-200/80 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-400"
          disabled={isLoading}
          onClick={onReload}
          title={t("Reload spec")}
          type="button"
        >
          <RefreshCw aria-hidden="true" className={`size-4 ${isLoading ? "animate-spin" : ""}`} />
        </button>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden px-3 py-3">
        {error ? (
          <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-medium text-rose-700">
            {error}
          </div>
        ) : null}

        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_2.25rem_2.25rem_2.25rem] items-center gap-2">
          <button
            className="inline-flex min-h-9 min-w-0 items-center justify-center gap-2 rounded-md bg-teal-700 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-teal-900/45 disabled:text-white/70"
            disabled={!enabled || isBusy || isLoading}
            onClick={onGenerate}
            title={generateLabel}
            type="button"
          >
            {operationKey === "generate" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Sparkles aria-hidden="true" className="size-4" />
            )}
            <span className="truncate">{generateLabel}</span>
          </button>
          <button
            aria-label={t("Save")}
            className="inline-flex size-9 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
            disabled={!canEdit || !isDirty || isBusy || isLoading}
            onClick={onSave}
            title={t("Save")}
            type="button"
          >
            {operationKey === "save" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Save aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
            aria-pressed={previewEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm ${previewEnabled
                ? "border-teal-300 bg-teal-700 text-white hover:bg-teal-800"
                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
              }`}
            onClick={() => onPreviewChange(!previewEnabled)}
            title={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
            type="button"
          >
            {previewEnabled ? (
              <EyeOff aria-hidden="true" className="size-4" />
            ) : (
              <Eye aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label={t("Inject into new chats")}
            aria-pressed={injectEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400 ${injectEnabled
                ? "border-teal-300 bg-teal-700 text-white hover:bg-teal-800"
                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
              }`}
            disabled={!enabled || isBusy || isLoading}
            onClick={() => onSettingsChange(enabled, !injectEnabled)}
            title={t("Inject into new chats")}
            type="button"
          >
            {operationKey === "settings" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <MessageSquare aria-hidden="true" className="size-4" />
            )}
          </button>
        </div>

        {conflictMessage ? (
          <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-800">
            <div>{conflictMessage}</div>
            <button
              className="mt-2 inline-flex items-center gap-2 rounded-md border border-amber-300 bg-white px-2.5 py-1.5 text-xs font-semibold text-amber-900 hover:bg-amber-100"
              onClick={onReload}
              type="button"
            >
              <RefreshCw aria-hidden="true" className="size-3.5" />
              {t("Reload spec")}
            </button>
          </div>
        ) : null}

        <div className="min-h-0 flex-1">
          {previewEnabled ? (
            <div className="h-full min-h-0 overflow-y-auto rounded-md border border-stone-200 bg-white px-4 py-3">
              {contentDraft.trim() ? (
                <MarkdownContent
                  content={contentDraft}
                  isUser={false}
                  selectedSkillPrefix={selectedSkillPrefix}
                />
              ) : (
                <div className="context-empty-inline">{t("No spec content")}</div>
              )}
            </div>
          ) : (
            <textarea
              aria-label={t("Project Spec Markdown")}
              className="h-full min-h-0 w-full resize-none rounded-md border border-stone-300 bg-white px-3 py-2 font-mono text-[13px] leading-5 text-stone-900 shadow-inner outline-none placeholder:text-stone-400 focus:border-teal-500 focus:ring-2 focus:ring-teal-100 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-500"
              disabled={!canEdit || isLoading}
              onChange={(event) => onContentChange(event.target.value)}
              placeholder={t("Generate or paste a Project Spec Markdown document.")}
              value={contentDraft}
            />
          )}
        </div>

        <div className="rounded-md border border-stone-200 bg-white px-3 py-2 text-xs leading-5 text-stone-600">
          {spec ? (
            <>
              <div>
                {t("Revision")} {formatNumber(spec.revision, language)}
                {spec.updatedAt ? ` · ${t("Updated")} ${formatTodoGraphDate(spec.updatedAt, language)}` : ""}
                {spec.generatedAt ? ` · ${t("Generated")} ${formatTodoGraphDate(spec.generatedAt, language)}` : ""}
              </div>
              <div>
                {latestJob
                  ? `${t("Latest job")}: ${t(workspaceSpecJobStatusLabel(latestJob.status))} · ${t(workspaceSpecTriggerLabel(latestJob.triggerType))} · ${latestJob.id}`
                  : t("No spec jobs")}
              </div>
              {latestJob?.errorMessage ? (
                <div className="break-words text-rose-700">{latestJob.errorMessage}</div>
              ) : null}
            </>
          ) : (
            t("No spec loaded")
          )}
        </div>
      </div>
    </div>
  );
}

function ContextStatsTab({
  contextUsage,
  error,
  isLoading,
  statistics,
}: {
  contextUsage: ContextUsageResponse | null;
  error: string | null;
  isLoading: boolean;
  statistics: ChatStatisticsResponse | null;
}) {
  const { language, t } = useI18n();

  if (isLoading && !statistics) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Stats")}</h2>
        <p>{t("Loading...")}</p>
      </div>
    );
  }

  if (error && !statistics) {
    return (
      <div className="context-empty-state">
        <BarChart3 aria-hidden="true" className="size-5" />
        <h2>{t("Stats")}</h2>
        <p>{error}</p>
      </div>
    );
  }

  if (!statistics) {
    return (
      <div className="context-empty-state">
        <BarChart3 aria-hidden="true" className="size-5" />
        <h2>{t("Stats")}</h2>
        <p>{t("No statistics for the active session yet.")}</p>
      </div>
    );
  }

  const tokenChart = [
    { id: "input", label: t("Input"), value: statistics.totalInputTokens },
    { id: "output", label: t("Output"), value: statistics.totalOutputTokens },
    { id: "cacheRead", label: t("Cache read"), value: statistics.totalCacheReadTokens },
    { id: "cacheWrite", label: t("Cache write"), value: statistics.totalCacheWriteTokens },
  ].filter((item) => item.value > 0);
  const modelChart = statistics.modelBreakdown.map((item) => ({
    id: item.modelId,
    label: item.modelId,
    value: item.requestCount,
  }));
  const contextChart = contextUsage
    ? contextUsage.tokenBreakdown.bySource
      .filter((item) => item.tokens > 0)
      .map((item) => ({
        id: item.source,
        label: contextSourceLabel(item.source, t),
        value: item.tokens,
      }))
    : [];

  return (
    <div className="context-stats-panel panel-scroll">
      <div className="context-stats-header">
        <div>
          <h2>{t("Session statistics")}</h2>
          <p>
            {t("Messages")}: {formatNumber(statistics.messageCount, language)}
          </p>
        </div>
        {isLoading ? (
          <LoaderCircle aria-label={t("Loading...")} className="size-4 animate-spin" />
        ) : null}
      </div>

      <div className="context-stats-metrics">
        <ContextStatMetric
          label={t("Total tokens")}
          value={formatCompactNumber(statistics.totalTokens, language)}
        />
        <ContextStatMetric
          label={t("Total time")}
          value={formatLatencySeconds(statistics.totalLatencyMs, language)}
        />
        <ContextStatMetric
          label={t("Memory refs")}
          value={formatNumber(statistics.memoryReferences, language)}
        />
        <ContextStatMetric
          label={t("New memories")}
          value={formatNumber(statistics.createdMemories, language)}
        />
        <ContextStatMetric
          label={t("LLM calls")}
          value={formatNumber(statistics.totalRequests, language)}
        />
        <ContextStatMetric
          label={t("Code changed")}
          value={`+${formatNumber(statistics.codeChangeStats.additions, language)} / -${formatNumber(statistics.codeChangeStats.deletions, language)}`}
        />
      </div>

      <ContextStatsSection title={t("Token usage")}>
        <ContextMiniBarChart
          data={tokenChart}
          emptyLabel={t("No token usage yet.")}
          valueFormatter={(value) => formatNumber(value, language)}
        />
      </ContextStatsSection>

      <ContextStatsSection title={t("Model calls")}>
        <ContextMiniBarChart
          data={modelChart}
          emptyLabel={t("No model calls yet.")}
          valueFormatter={(value) => formatNumber(value, language)}
        />
      </ContextStatsSection>

      <ContextStatsSection title={t("Context mix")}>
        {contextUsage ? (
          <>
            <div className="context-stats-inline">
              <span>{t("Context usage")}</span>
              <strong>
                {formatNumber(contextUsage.usedMessageTokens, language)} /{" "}
                {formatNumber(contextUsage.availableMessageTokens, language)}
              </strong>
            </div>
            <ContextMiniBarChart
              data={contextChart}
              emptyLabel={t("No context usage yet.")}
              valueFormatter={(value) => formatNumber(value, language)}
            />
            <ContextStatsRows
              emptyLabel={t("No context usage yet.")}
              rows={contextChart.map((item) => ({
                label: item.label,
                value: formatNumber(item.value, language),
              }))}
            />
          </>
        ) : (
          <div className="context-empty-inline">{t("Context usage unavailable.")}</div>
        )}
      </ContextStatsSection>

      <ContextStatsSection title={t("Tools and compression")}>
        <ContextStatsRows
          emptyLabel={t("No tools used yet.")}
          rows={[
            ...statistics.toolBreakdown.map((item) => ({
              label: item.toolName,
              value: formatNumber(item.callCount, language),
            })),
            {
              label: t("Rule compression snapshots"),
              value: formatNumber(statistics.compression.ruleSnapshotCount, language),
            },
            {
              label: t("LLM compression snapshots"),
              value: formatNumber(statistics.compression.llmSnapshotCount, language),
            },
            {
              label: t("Compression snapshots"),
              value: formatNumber(statistics.compression.snapshotCount, language),
            },
            {
              label: t("Tokens saved"),
              value: formatNumber(statistics.compression.savedTokenCount, language),
            },
          ]}
        />
      </ContextStatsSection>
    </div>
  );
}

function ContextStatMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="context-stat-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ContextStatsSection({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <section className="context-stats-section">
      <div className="context-panel-section-title">{title}</div>
      {children}
    </section>
  );
}

function ContextMiniBarChart({
  data,
  emptyLabel,
  valueFormatter,
}: {
  data: { id: string; label: string; value: number }[];
  emptyLabel: string;
  valueFormatter: (value: number) => string;
}) {
  if (!data.length) {
    return <div className="context-empty-inline">{emptyLabel}</div>;
  }

  const chartMax = Math.max(...data.map((item) => item.value), 1);

  return (
    <div className="context-mini-chart">
      <ResponsiveContainer height={Math.max(112, data.length * 34)} width="100%">
        <BarChart
          data={data}
          layout="vertical"
          margin={{ bottom: 0, left: 4, right: 12, top: 0 }}
        >
          <XAxis domain={[0, chartMax]} hide type="number" />
          <YAxis
            axisLine={false}
            dataKey="label"
            tick={{ fill: "#57534e", fontSize: 11 }}
            tickLine={false}
            type="category"
            width={96}
          />
          <Tooltip
            contentStyle={chartTooltipStyle}
            formatter={(value) => valueFormatter(Number(value))}
            labelStyle={chartTooltipLabelStyle}
          />
          <Bar dataKey="value" radius={[4, 4, 4, 4]}>
            {data.map((item, index) => (
              <Cell fill={chartColor(index)} key={item.id} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

function ContextStatsRows({
  emptyLabel,
  rows,
}: {
  emptyLabel: string;
  rows: { label: string; value: string }[];
}) {
  if (!rows.length) {
    return <div className="context-empty-inline">{emptyLabel}</div>;
  }

  return (
    <div className="context-stats-rows">
      {rows.map((row) => (
        <div className="context-stats-row" key={row.label}>
          <span>{row.label}</span>
          <strong>{row.value}</strong>
        </div>
      ))}
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
  const [isExpanded, setIsExpanded] = useState(false);
  const bodyId = `todo-graph-task-${task.id}-body`;

  return (
    <div>
      <div
        className="rounded-lg border border-stone-200 bg-white shadow-sm transition hover:border-stone-300 hover:bg-stone-50"
        style={{ marginLeft: level ? Math.min(level * 14, 42) : 0 }}
      >
        <button
          aria-controls={bodyId}
          aria-expanded={isExpanded}
          className="flex w-full min-w-0 items-start gap-2 px-3 py-2 text-left focus:outline-none focus:ring-2 focus:ring-inset focus:ring-stone-300"
          onClick={() => setIsExpanded((current) => !current)}
          type="button"
        >
          {isExpanded ? (
            <ChevronDown
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-stone-500"
            />
          ) : (
            <ChevronRight
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-stone-500"
            />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] font-semibold text-stone-500">
                {task.id}
              </span>
              <span className={taskStatusClass(task.status)}>
                {t(task.status)}
              </span>
            </div>
            <h3
              className={`mt-1 break-words text-sm font-semibold leading-snug text-stone-950 ${isExpanded ? "" : "line-clamp-2"
                }`}
            >
              {task.title}
            </h3>
            {task.summary ? (
              <p
                className={`mt-1 break-words text-xs leading-5 text-stone-600 ${isExpanded ? "" : "line-clamp-2"
                  }`}
              >
                {task.summary}
              </p>
            ) : null}
          </div>
        </button>
        {isExpanded ? (
          <div className="px-3 pb-2 pl-8" id={bodyId}>
            {task.dependsOn.length ? (
              <div className="mt-1 flex flex-wrap gap-1.5">
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

function SourceControlPanel({
  diffError,
  diffResponse,
  files,
  gitCommitMessage,
  gitOperationKey,
  isLoading,
  onCommit,
  onGenerateCommitMessage,
  onCommitMessageChange,
  onFileOperation,
  onRefresh,
  onSelectFile,
  selectedPath,
}: {
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  gitCommitMessage: string;
  gitOperationKey: string | null;
  isLoading: boolean;
  onCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGenerateCommitMessage: () => void;
  onCommitMessageChange: (message: string) => void;
  onFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onRefresh: () => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();
  const diffSections = parseGitDiffSections(diffResponse);
  const stagedFiles = diffResponse?.stagedFiles ?? [];
  const isCommitting = gitOperationKey === "commit";
  const isGeneratingCommitMessage = gitOperationKey === "generate-commit-message";
  const isCommitMessageInputDisabled = isCommitting || isGeneratingCommitMessage;

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col bg-[#f8f8f8]">
      <div className="flex min-h-[var(--foco-header-height)] items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-teal-50 text-teal-800">
            <GitCompare aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Source Control")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {selectedPath ?? t("Workspace changes")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Refresh diff")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-stone-600 hover:bg-stone-200/80 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-400"
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

      {diffError ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {diffError}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
        <form className="mb-3 space-y-2 px-1" onSubmit={onCommit}>
          <div className="relative">
            <textarea
              className="min-h-20 w-full resize-none rounded-md border border-stone-300 bg-white px-3 py-2 pr-11 text-sm text-stone-900 shadow-inner outline-none placeholder:text-stone-400 focus:border-teal-500 focus:ring-2 focus:ring-teal-100"
              disabled={isCommitMessageInputDisabled}
              onChange={(event) => onCommitMessageChange(event.target.value)}
              placeholder={t("Commit message")}
              value={gitCommitMessage}
            />
            <button
              aria-label={t("Generate commit message")}
              className="absolute right-2 top-2 inline-flex size-7 items-center justify-center rounded-md text-teal-700 hover:bg-teal-50 hover:text-teal-900 disabled:cursor-not-allowed disabled:text-stone-300 disabled:hover:bg-transparent"
              disabled={isCommitMessageInputDisabled || stagedFiles.length === 0}
              onClick={onGenerateCommitMessage}
              title={t("Generate commit message")}
              type="button"
            >
              {isGeneratingCommitMessage ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <Sparkles aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
          <button
            className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-teal-700 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:text-stone-500"
            disabled={isCommitMessageInputDisabled || !gitCommitMessage.trim() || stagedFiles.length === 0}
            type="submit"
          >
            {isCommitting ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin" /> : null}
            {t("Commit")}
          </button>
        </form>

        <section className="mb-3">
          <div className="mb-1 flex items-center justify-between px-1 text-[11px] font-semibold uppercase tracking-wide text-stone-500">
            <span>{t("Staged Changes")}</span>
            <span>{stagedFiles.length}</span>
          </div>
          <div className="space-y-0.5">
            {stagedFiles.length ? (
              stagedFiles.map((file) => (
                <GitFileRow
                  action="unstage"
                  diffSections={diffSections}
                  file={file}
                  gitOperationKey={gitOperationKey}
                  isLoading={isLoading}
                  key={`staged-${file.path}`}
                  onFileOperation={onFileOperation}
                  onSelectFile={onSelectFile}
                  selectedPath={selectedPath}
                  showDiscard={false}
                />
              ))
            ) : (
              <div className="rounded-md border border-dashed border-stone-300 bg-white/70 px-3 py-2 text-xs text-stone-500">
                {t("No staged changes")}
              </div>
            )}
          </div>
        </section>

        <section>
          <button
            className={diffFileButtonClass(selectedPath === null)}
            onClick={() => onSelectFile(null)}
            type="button"
          >
            <span className="truncate text-[11px] font-semibold uppercase tracking-wide">
              {t("Changes")}
            </span>
            <span className="text-xs text-stone-500">{files.length}</span>
          </button>
          <div className="mt-1 space-y-0.5">
            {files.length ? (
              files.map((file) => (
                <GitFileRow
                  action="stage"
                  diffSections={diffSections}
                  file={file}
                  gitOperationKey={gitOperationKey}
                  isLoading={isLoading}
                  key={`unstaged-${file.path}`}
                  onFileOperation={onFileOperation}
                  onSelectFile={onSelectFile}
                  selectedPath={selectedPath}
                  showDiscard
                />
              ))
            ) : (
              <div className="rounded-md border border-dashed border-stone-300 bg-white/70 px-3 py-2 text-xs text-stone-500">
                {t("No changes")}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function GitFileRow({
  action,
  diffSections,
  file,
  gitOperationKey,
  isLoading,
  onFileOperation,
  onSelectFile,
  selectedPath,
  showDiscard,
}: {
  action: "stage" | "unstage";
  diffSections: GitDiffSection[];
  file: GitStatusFileSummary;
  gitOperationKey: string | null;
  isLoading: boolean;
  onFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
  showDiscard: boolean;
}) {
  const { t } = useI18n();
  const isExpanded = selectedPath === file.path;
  const label = statusLabel(file);
  const actionKey = `${action}:${file.path}`;
  const discardKey = `discard:${file.path}`;
  const isActionLoading = gitOperationKey === actionKey;
  const isDiscardLoading = gitOperationKey === discardKey;
  const pathParts = gitFilePathParts(file.path);

  return (
    <div>
      <div className={diffFileButtonClass(isExpanded)}>
        <button
          aria-label={`${file.path} ${label}`}
          className="flex min-w-0 flex-1 items-center gap-1.5 py-0.5 text-left"
          onClick={() => onSelectFile(isExpanded ? null : file.path)}
          type="button"
        >
          {isExpanded ? (
            <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
          ) : (
            <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
          )}
          <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-left">
            <span className="min-w-0 truncate text-[13px] font-medium text-stone-900">
              {pathParts.name}
            </span>
            {pathParts.directory ? (
              <span className="shrink truncate text-xs text-stone-400">
                {pathParts.directory}
              </span>
            ) : null}
          </span>
        </button>
        <span className={gitStatusBadgeClass(label)}>{label}</span>
        <button
          aria-label={t(action === "stage" ? "Stage file" : "Unstage file")}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded text-stone-500 hover:bg-stone-200 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-300"
          disabled={gitOperationKey !== null}
          onClick={(event) => {
            event.stopPropagation();
            onFileOperation(action, file.path);
          }}
          title={t(action === "stage" ? "Stage file" : "Unstage file")}
          type="button"
        >
          {isActionLoading ? (
            <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          ) : action === "stage" ? (
            <Plus aria-hidden="true" className="size-3.5" />
          ) : (
            <Minus aria-hidden="true" className="size-3.5" />
          )}
        </button>
        {showDiscard ? (
          <button
            aria-label={t("Discard file changes")}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded text-stone-500 hover:bg-rose-100 hover:text-rose-700 disabled:cursor-not-allowed disabled:text-stone-300"
            disabled={gitOperationKey !== null}
            onClick={(event) => {
              event.stopPropagation();
              onFileOperation("discard", file.path);
            }}
            title={t("Discard file changes")}
            type="button"
          >
            {isDiscardLoading ? (
              <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
            ) : (
              <Undo2 aria-hidden="true" className="size-3.5" />
            )}
          </button>
        ) : null}
      </div>
      {isExpanded ? (
        <InlineGitDiff isLoading={isLoading} path={file.path} sections={diffSections} />
      ) : null}
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
  agentDefinitionOperationKey,
  agentDefinitions,
  agentDefinitionsError,
  canLogout,
  canUseNativePicker,
  isLoadingAgentDefinitions,
  nativeBrowserToken,
  onAddWorkspace,
  onActiveSectionChange,
  onCreateAgentDefinition,
  onDeleteAgentDefinition,
  onUpdateAgentDefinition,
  onLogout,
  onOpenChat,
  onSettingsChange,
  onWorkspacesChange,
  workspaceDialogRevision,
}: {
  activeSection: SettingsSection;
  agentDefinitionOperationKey: string | null;
  agentDefinitions: AgentDefinitionSettings[];
  agentDefinitionsError: string | null;
  canLogout: boolean;
  canUseNativePicker: boolean;
  isLoadingAgentDefinitions: boolean;
  nativeBrowserToken: string | null;
  onAddWorkspace: () => void;
  onActiveSectionChange: (section: SettingsSection) => void;
  onCreateAgentDefinition: (definition: AgentDefinitionInput) => Promise<boolean>;
  onDeleteAgentDefinition: (id: string) => Promise<void>;
  onUpdateAgentDefinition: (
    id: string,
    definition: AgentDefinitionInput,
  ) => Promise<boolean>;
  onLogout: () => Promise<void>;
  onOpenChat: (workspaceId: string, chatId: string) => void;
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
  const [webSearchForm, setWebSearchForm] = useState<WebSearchFormState>(() =>
    emptyWebSearchForm(),
  );
  const [promptSettingsForm, setPromptSettingsForm] =
    useState<PromptSettingsFormState>(() => emptyPromptSettingsForm());
  const [specSettingsForm, setSpecSettingsForm] =
    useState<SpecSettingsFormState>(() => emptySpecSettingsForm());
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
  const [memoryDreamJobs, setMemoryDreamJobs] = useState<MemoryDreamJobSummary[]>(
    [],
  );
  const [memoryDreamPage, setMemoryDreamPage] = useState(1);
  const [memoryDreamPageSize, setMemoryDreamPageSize] = useState(
    MEMORY_DREAM_DEFAULT_PAGE_SIZE,
  );
  const [memoryDreamChanges, setMemoryDreamChanges] = useState<
    MemoryDreamChangeSummary[]
  >([]);
  const [memoryDreamDetailJobId, setMemoryDreamDetailJobId] = useState<
    string | null
  >(null);
  const [memoryDreamError, setMemoryDreamError] = useState<string | null>(null);
  const [selectedMemoryId, setSelectedMemoryId] = useState<string | null>(null);
  const [memorySources, setMemorySources] = useState<MemorySourceRecord[]>([]);
  const [workspaceForm, setWorkspaceForm] = useState<WorkspaceFormState>(() =>
    emptyWorkspaceForm(),
  );
  const [isLoadingWorkspaceSpecSettings, setIsLoadingWorkspaceSpecSettings] =
    useState(false);
  const [isWorkspaceSpecSettingsLoaded, setIsWorkspaceSpecSettingsLoaded] =
    useState(false);
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
  const [isSavingWebSearch, setIsSavingWebSearch] = useState(false);
  const [isSavingPromptSettings, setIsSavingPromptSettings] = useState(false);
  const [isSavingSpecSettings, setIsSavingSpecSettings] = useState(false);
  const [isSelectingPromptFile, setIsSelectingPromptFile] = useState(false);
  const [isSavingMemorySettings, setIsSavingMemorySettings] = useState(false);
  const [isLoadingMemories, setIsLoadingMemories] = useState(false);
  const [isLoadingMemoryDreamJobs, setIsLoadingMemoryDreamJobs] = useState(false);
  const [isLoadingMemoryDreamChanges, setIsLoadingMemoryDreamChanges] =
    useState(false);
  const [isSavingMemory, setIsSavingMemory] = useState(false);
  const [memoryDreamRunKey, setMemoryDreamRunKey] = useState<string | null>(null);
  const [isClearingPassword, setIsClearingPassword] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingTheme, setIsSavingTheme] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSavingWorkspaceOrder, setIsSavingWorkspaceOrder] = useState(false);
  const [isSavingWorkspaceLogo, setIsSavingWorkspaceLogo] = useState(false);
  const [isSelectingWorkspaceFormPath, setIsSelectingWorkspaceFormPath] =
    useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [isRefreshingProviderModels, setIsRefreshingProviderModels] =
    useState(false);
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
  const [expandedProviderIds, setExpandedProviderIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [providerModelLists, setProviderModelLists] = useState<
    Record<string, ProviderModelListState>
  >({});
  const [error, setError] = useState<string | null>(null);
  const [isGeneralPasswordVisible, setIsGeneralPasswordVisible] = useState(false);
  const [isProviderApiKeyVisible, setIsProviderApiKeyVisible] = useState(false);
  const [isEditingGeneralPassword, setIsEditingGeneralPassword] = useState(false);

  const selectedMetadata = useMemo(
    () =>
      metadata?.models.find((model) => model.key === selectedMetadataKey) ??
      null,
    [metadata, selectedMetadataKey],
  );
  const inputModalityOptions = useMemo(
    () =>
      modelModalityOptions(
        metadata?.models ?? [],
        "inputModalities",
        form.inputModalities,
      ),
    [form.inputModalities, metadata],
  );
  const outputModalityOptions = useMemo(
    () =>
      modelModalityOptions(
        metadata?.models ?? [],
        "outputModalities",
        form.outputModalities,
      ),
    [form.outputModalities, metadata],
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
  const modelOutputsText = form.outputModalities.includes("text");
  const enabledNeedsLimits =
    form.enabled &&
    modelOutputsText &&
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
  const memoryDreamWorkspaceId = memoryFilter.workspaceId || memoryWorkspace?.id || "";
  const sortedMemoryDreamJobs = useMemo(
    () =>
      [...memoryDreamJobs].sort(
        (left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt),
      ),
    [memoryDreamJobs],
  );
  const memoryDreamTotalPages = sortedMemoryDreamJobs.length
    ? Math.ceil(sortedMemoryDreamJobs.length / memoryDreamPageSize)
    : 0;
  const currentMemoryDreamPage =
    memoryDreamTotalPages === 0
      ? 1
      : Math.min(memoryDreamPage, memoryDreamTotalPages);
  const paginatedMemoryDreamJobs = sortedMemoryDreamJobs.slice(
    (currentMemoryDreamPage - 1) * memoryDreamPageSize,
    currentMemoryDreamPage * memoryDreamPageSize,
  );
  const memoryDreamPaginationItems = auditPaginationItems(
    currentMemoryDreamPage,
    memoryDreamTotalPages,
  );
  const memoryDreamPageStart = paginatedMemoryDreamJobs.length
    ? (currentMemoryDreamPage - 1) * memoryDreamPageSize + 1
    : 0;
  const memoryDreamPageEnd = paginatedMemoryDreamJobs.length
    ? Math.min(
      sortedMemoryDreamJobs.length,
      memoryDreamPageStart + paginatedMemoryDreamJobs.length - 1,
    )
    : 0;
  const memoryDreamDetailJob =
    sortedMemoryDreamJobs.find((job) => job.id === memoryDreamDetailJobId) ??
    null;
  const activeMemoryDreamJobKeys = useMemo(
    () =>
      new Set(
        sortedMemoryDreamJobs
          .filter((job) => isActiveMemoryDreamStatus(job.status))
          .map((job) => memoryDreamJobKey(job.scope, job.workspaceId)),
      ),
    [sortedMemoryDreamJobs],
  );
  const globalDreamRunKey = memoryDreamJobKey("global", null);
  const workspaceDreamRunKey = memoryDreamJobKey(
    "workspace",
    memoryDreamWorkspaceId,
  );
  const latestSuccessfulMemoryDreamJob =
    sortedMemoryDreamJobs.find((job) => job.status === "completed") ?? null;
  const latestFailedMemoryDreamJob =
    sortedMemoryDreamJobs.find((job) => job.status === "failed") ?? null;
  const memoryDreamNextRunEstimate = nextMemoryDreamRunEstimate(
    latestSuccessfulMemoryDreamJob,
    memorySettingsForm.dream,
    language,
    t,
  );
  const latestMemoryDreamChangeCount = latestSuccessfulMemoryDreamJob
    ? memoryDreamAppliedChangeCount(latestSuccessfulMemoryDreamJob)
    : 0;
  const memoryDreamChangesByOperation = useMemo(
    () => groupMemoryDreamChanges(memoryDreamChanges),
    [memoryDreamChanges],
  );
  const isMemoryDreamRunnable =
    memorySettingsForm.enabled && memorySettingsForm.dream.enabled;
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
  const configuredModelsByName = useMemo(
    () =>
      [...configuredModels].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    [configuredModels],
  );
  const passwordInputValue =
    generalForm.password ||
    (settings?.general.webServer.passwordEnabled && !isEditingGeneralPassword
      ? SAVED_PASSWORD_MASK
      : "");
  const orderedConfiguredModels = useMemo(() => {
    const sortedConfiguredModels = [...configuredModels].sort((left, right) =>
      left.displayName.localeCompare(right.displayName),
    );

    if (!modelOrderPreview) {
      return sortedConfiguredModels;
    }

    const modelsById = new Map(
      sortedConfiguredModels.map((model) => [model.id, model]),
    );
    const previewModels = modelOrderPreview
      .map((modelId) => modelsById.get(modelId))
      .filter((model): model is ConfiguredModelSummary => Boolean(model));

    return previewModels.length === sortedConfiguredModels.length
      ? previewModels
      : sortedConfiguredModels;
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
  const providerServices = useMemo(
    () => providerServicesForKinds(providerKinds),
    [providerKinds],
  );
  const selectedProviderServiceId =
    providerForm.serviceId ||
    providerServiceIdForKind(providerForm.kind) ||
    providerServiceIdForKind(defaultProviderKind(providerKinds)) ||
    providerServices[0]?.id ||
    "";
  const selectedProviderService =
    providerServices.find((service) => service.id === selectedProviderServiceId) ??
    null;
  const providerProtocolKinds = selectedProviderService
    ? providerKinds.filter((kind) =>
      selectedProviderService.kindIds.includes(kind.kind),
    )
    : providerKinds;
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const apiProxyTypes = editingProvider?.apiProxy.supportedTypes ??
    providers[0]?.apiProxy.supportedTypes ?? [
      { label: "HTTP", proxyType: "http" },
      { label: "SOCKS", proxyType: "socks" },
    ];
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const hasProviderKeyClearButton = hasSavedProviderKey || providerForm.clearApiKey;
  const selectedProviderIds = new Set(form.providerIds);
  const systemPrompts = promptSettingsForm.systemPrompts.length
    ? promptSettingsForm.systemPrompts
    : settings
      ? normalizedSystemPromptSummaries(settings.prompts)
      : [];
  const savedSystemPrompts = settings
    ? normalizedSystemPromptSummaries(settings.prompts)
    : systemPrompts;
  const activeSystemPrompt =
    systemPrompts.find(
      (prompt) => prompt.name === promptSettingsForm.activeSystemPromptName,
    ) ??
    systemPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME) ??
    systemPrompts[0] ??
    null;

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
      apiRequestDetailRetentionDays: String(
        data.general.apiAudit.requestDetailRetentionDays,
      ),
      apiSaveRequestResponseDetails:
        data.general.apiAudit.saveRequestResponseDetails,
      autoStartEnabled: data.general.autoStartEnabled,
      hookAuditEnabled: data.general.hookAuditEnabled,
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
      llmRequestRetryCount: String(data.general.llmRequestRetryCount),
      password: "",
      theme: data.general.theme,
    });
  }

  function syncWebSearchForm(data: SettingsResponse) {
    setWebSearchForm({
      activeProvider:
        data.webSearch.activeProvider ||
        data.webSearch.providers[0]?.provider ||
        "tavily",
      apiProxyEnabled: data.webSearch.apiProxy.enabled,
      apiProxyType:
        data.webSearch.apiProxy.proxyType ||
        data.webSearch.apiProxy.supportedTypes[0]?.proxyType ||
        "http",
      apiProxyUrl: data.webSearch.apiProxy.url,
      braveApiKey: "",
      clearBraveApiKey: false,
      clearTavilyApiKey: false,
      enabled: data.webSearch.enabled,
      tavilyApiKey: "",
    });
  }

  function syncPromptSettingsForm(data: SettingsResponse) {
    const systemPrompts = normalizedSystemPromptSummaries(data.prompts);
    setPromptSettingsForm({
      activeSystemPromptName:
        systemPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)
          ?.name ??
        systemPrompts[0]?.name ??
        DEFAULT_SYSTEM_PROMPT_NAME,
      extraText: data.prompts.extraText,
      files: data.prompts.files,
      pendingFile: "",
      pendingSystemPromptName: "",
      pendingSystemPromptRename: "",
      renamingSystemPromptName: null,
      systemPrompts,
    });
  }

  function syncSpecSettingsForm(data: SettingsResponse) {
    setSpecSettingsForm({
      autoEnabled: data.spec.autoEnabled,
      generationModelId: data.spec.generationModelId ?? "",
      generationSystemPrompt: data.spec.generationSystemPrompt ?? "",
      updateSystemPrompt: data.spec.updateSystemPrompt ?? "",
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
      dream: {
        enabled: data.memory.dream.enabled,
        autoEnabled: data.memory.dream.autoEnabled,
        mode: data.memory.dream.mode,
        modelId: data.memory.dream.modelId ?? "",
        workspaceIntervalDays: String(data.memory.dream.workspaceIntervalDays),
        globalIntervalDays: String(data.memory.dream.globalIntervalDays),
        createTranscriptChat: data.memory.dream.createTranscriptChat,
        maxFactsPerRun: String(data.memory.dream.maxFactsPerRun),
        maxChangesPerRun: String(data.memory.dream.maxChangesPerRun),
        schedulerScanMinutes: String(data.memory.dream.schedulerScanMinutes),
      },
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
      syncWebSearchForm(data);
      syncPromptSettingsForm(data);
      syncSpecSettingsForm(data);
      syncMemorySettingsForm(data);
      setProviderForm((current) => ({
        ...current,
        kind: current.kind || defaultProviderKind(data.providerKinds),
        serviceId:
          current.serviceId ||
          providerServiceIdForKind(
            current.kind || defaultProviderKind(data.providerKinds),
          ) ||
          "",
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

  const loadMemoryDreamJobs = useCallback(async () => {
    setIsLoadingMemoryDreamJobs(true);
    setMemoryDreamError(null);

    try {
      const data = await requestJson<MemoryDreamJobsResponse>(
        "/api/memory/dream/jobs",
      );
      setMemoryDreamJobs(data.jobs);
      setMemoryDreamDetailJobId((current) =>
        current && data.jobs.some((job) => job.id === current) ? current : null,
      );
    } catch (requestError) {
      setMemoryDreamJobs([]);
      setMemoryDreamChanges([]);
      setMemoryDreamDetailJobId(null);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamJobs(false);
    }
  }, []);

  const loadMemoryDreamChanges = useCallback(async (jobId: string) => {
    setIsLoadingMemoryDreamChanges(true);
    setMemoryDreamError(null);

    try {
      const data = await requestJson<MemoryDreamChangesResponse>(
        `/api/memory/dream/jobs/${encodeURIComponent(jobId)}/changes`,
      );
      setMemoryDreamChanges(data.changes);
    } catch (requestError) {
      setMemoryDreamChanges([]);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamChanges(false);
    }
  }, []);

  function closeMemoryDreamDetailDialog() {
    setMemoryDreamDetailJobId(null);
    setMemoryDreamChanges([]);
  }

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
    if (activeSection === "memory") {
      void loadMemoryDreamJobs();
    }
  }, [activeSection, loadMemoryDreamJobs]);

  useEffect(() => {
    if (activeSection !== "memory" || !memoryDreamDetailJobId) {
      setMemoryDreamChanges([]);
      return;
    }

    if (!memoryDreamJobs.some((job) => job.id === memoryDreamDetailJobId)) {
      closeMemoryDreamDetailDialog();
      return;
    }

    void loadMemoryDreamChanges(memoryDreamDetailJobId);
  }, [
    activeSection,
    loadMemoryDreamChanges,
    memoryDreamDetailJobId,
    memoryDreamJobs,
  ]);

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
      enabled:
        outputModalitiesRequireLimits(defaultModalities(model.outputModalities)) ?
          model.contextWindow !== null && model.maxOutputTokens !== null
        : true,
      modelId: model.modelId,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: [],
      activeProviderId: "",
      inputModalities: defaultModalities(model.inputModalities),
      outputModalities: defaultModalities(model.outputModalities),
      thinkingLevel: "",
      systemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
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
        enabled:
          outputModalitiesRequireLimits(defaultModalities(model.outputModalities)) ?
            model.contextWindow !== null && model.maxOutputTokens !== null
          : true,
        modelId: model.modelId,
        contextWindow: numberInputValue(model.contextWindow),
        maxOutputTokens: numberInputValue(model.maxOutputTokens),
        inputModalities: defaultModalities(model.inputModalities),
        outputModalities: defaultModalities(model.outputModalities),
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
      inputModalities: defaultModalities(model.inputModalities),
      outputModalities: defaultModalities(model.outputModalities),
      thinkingLevel: model.thinkingLevel ?? "",
      systemPromptName: model.systemPromptName || DEFAULT_SYSTEM_PROMPT_NAME,
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
      autoSyncModels: provider.autoSyncModels,
      modelSyncFilterRegex: provider.modelSyncFilterRegex ?? "",
      name: provider.name,
      requestOverrides: provider.requestOverrides.map((overrideRule) => ({
        target: overrideRule.target,
        name: overrideRule.name,
        valueType: overrideRule.valueType,
        value:
          overrideRule.valueType === "boolean"
            ? Boolean(overrideRule.value)
            : String(overrideRule.value),
      })),
      serviceId: providerServiceIdForKind(provider.kind) || "",
    });
    setIsProviderApiKeyVisible(false);
    setIsProviderDialogOpen(true);
  }

  function startAddingProvider() {
    const kind = defaultProviderKind(providerKinds);
    setProviderForm({
      ...emptyProviderForm(),
      baseUrl: providerKindDefaultBaseUrl(providerKinds, kind),
      kind,
      serviceId: providerServiceIdForKind(kind) || "",
    });
    setIsProviderApiKeyVisible(false);
    setIsProviderDialogOpen(true);
  }

  function applyProviderService(serviceId: string) {
    const service = providerServices.find((item) => item.id === serviceId);

    if (!service) {
      return;
    }

    const kind = providerDefaultKindForService(service, providerKinds);
    const baseUrl = providerKindDefaultBaseUrl(providerKinds, kind);

    setProviderForm((current) => {
      const previousService = providerServices.find(
        (item) => item.id === current.serviceId,
      );
      const shouldFillName =
        !current.name.trim() || current.name === previousService?.label;

      return {
        ...current,
        baseUrl,
        kind,
        name: shouldFillName ? service.label : current.name,
        serviceId,
      };
    });
  }

  function updateProviderProtocol(kind: string) {
    setProviderForm((current) => ({
      ...current,
      baseUrl: providerKindDefaultBaseUrl(providerKinds, kind),
      kind,
      serviceId: providerServiceIdForKind(kind) || current.serviceId,
    }));
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

  async function editConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setWorkspaceForm({
      commonCommands: workspace.commonCommands.map((command) => ({ ...command })),
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
      pinned: workspace.pinned,
      specEnabled: false,
      specInjectEnabled: false,
      terminalShell: workspace.terminalShell,
    });
    setIsWorkspaceSpecSettingsLoaded(false);
    setIsWorkspaceDialogOpen(true);
    setIsLoadingWorkspaceSpecSettings(true);
    try {
      const data = await requestJson<WorkspaceSpecResponse>(
        `/api/workspaces/${encodeURIComponent(workspace.id)}/spec`,
      );
      setWorkspaceForm((current) =>
        current.id === workspace.id
          ? {
            ...current,
            specEnabled: data.settings.enabled,
            specInjectEnabled: data.settings.injectEnabled,
          }
          : current,
      );
      setIsWorkspaceSpecSettingsLoaded(true);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingWorkspaceSpecSettings(false);
    }
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
      const shouldSaveAutoStart =
        generalForm.autoStartEnabled ||
        Boolean(
          settings &&
          generalForm.autoStartEnabled !== settings.general.autoStartEnabled,
        );
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          apiAudit: {
            requestDetailRetentionDays: optionalPositiveInteger(
              generalForm.apiRequestDetailRetentionDays,
              t("API request detail retention days"),
            ),
            saveRequestResponseDetails: generalForm.apiSaveRequestResponseDetails,
          },
          ...(shouldSaveAutoStart
            ? { autoStartEnabled: generalForm.autoStartEnabled }
            : {}),
          clearPassword: false,
          listenHost: generalForm.listenHost,
          listenPort: optionalPositiveInteger(
            generalForm.listenPort,
            t("Listen port"),
          ),
          llmRequestRetryCount: optionalPositiveInteger(
            generalForm.llmRequestRetryCount,
            t("LLM request retries"),
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

  async function saveWebSearchSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWebSearch(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/web-search", {
        body: JSON.stringify({
          activeProvider: webSearchForm.activeProvider,
          apiProxy: {
            enabled: webSearchForm.apiProxyEnabled,
            proxyType: webSearchForm.apiProxyType,
            url: webSearchForm.apiProxyUrl,
          },
          braveApiKey: webSearchForm.braveApiKey.trim() || null,
          clearBraveApiKey: webSearchForm.clearBraveApiKey,
          clearTavilyApiKey: webSearchForm.clearTavilyApiKey,
          enabled: webSearchForm.enabled,
          tavilyApiKey: webSearchForm.tavilyApiKey.trim() || null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncWebSearchForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWebSearch(false);
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
          systemPrompts: promptSettingsForm.systemPrompts,
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

  async function saveSpecSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingSpecSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/spec", {
        body: JSON.stringify({
          autoEnabled: specSettingsForm.autoEnabled,
          generationModelId: specSettingsForm.generationModelId.trim() || null,
          generationSystemPrompt:
            specSettingsForm.generationSystemPrompt.trim() || null,
          updateSystemPrompt: specSettingsForm.updateSystemPrompt.trim() || null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSpecSettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingSpecSettings(false);
    }
  }

  function addSystemPrompt(name: string) {
    const nextName = name.trim();
    if (!nextName) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      if (currentSystemPrompts.some((prompt) => prompt.name === nextName)) {
        return {
          ...current,
          activeSystemPromptName: nextName,
          pendingSystemPromptName: "",
          systemPrompts: currentSystemPrompts,
        };
      }

      return {
        ...current,
        activeSystemPromptName: nextName,
        pendingSystemPromptName: "",
        systemPrompts: [
          ...currentSystemPrompts,
          {
            name: nextName,
            content: "",
          },
        ],
      };
    });
  }

  function removeSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      const systemPrompts = currentSystemPrompts.filter(
        (prompt) => prompt.name !== name,
      );
      return {
        ...current,
        activeSystemPromptName:
          current.activeSystemPromptName === name
            ? DEFAULT_SYSTEM_PROMPT_NAME
            : current.activeSystemPromptName,
        pendingSystemPromptRename:
          current.renamingSystemPromptName === name
            ? ""
            : current.pendingSystemPromptRename,
        renamingSystemPromptName:
          current.renamingSystemPromptName === name
            ? null
            : current.renamingSystemPromptName,
        systemPrompts,
      };
    });
  }

  function startRenameSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => ({
      ...current,
      activeSystemPromptName: name,
      pendingSystemPromptRename: name,
      renamingSystemPromptName: name,
    }));
  }

  function cancelRenameSystemPrompt() {
    setPromptSettingsForm((current) => ({
      ...current,
      pendingSystemPromptRename: "",
      renamingSystemPromptName: null,
    }));
  }

  function submitRenameSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => {
      const nextName = current.pendingSystemPromptRename.trim();
      if (!nextName) {
        return current;
      }

      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      if (
        currentSystemPrompts.some(
          (prompt) => prompt.name !== name && prompt.name === nextName,
        )
      ) {
        return current;
      }

      return {
        ...current,
        activeSystemPromptName: nextName,
        pendingSystemPromptRename: "",
        renamingSystemPromptName: null,
        systemPrompts: currentSystemPrompts.map((prompt) =>
          prompt.name === name
            ? {
              ...prompt,
              name: nextName,
            }
            : prompt,
        ),
      };
    });
  }

  function updateActiveSystemPromptContent(content: string) {
    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      return {
        ...current,
        systemPrompts: currentSystemPrompts.map((prompt) =>
          prompt.name === current.activeSystemPromptName
            ? {
              ...prompt,
              content,
            }
            : prompt,
        ),
      };
    });
  }

  function defaultSystemPromptContent(name: string) {
    if (!settings) {
      return null;
    }
    if (name === DEFAULT_SYSTEM_PROMPT_NAME) {
      return settings.prompts.defaultSystemPrompt;
    }
    if (name === IMAGE_AGENT_SYSTEM_PROMPT_NAME) {
      return settings.prompts.defaultImageGenerationSystemPrompt ?? null;
    }
    return null;
  }

  function restoreSystemPromptDefault(name: string) {
    const defaultContent = defaultSystemPromptContent(name);
    if (defaultContent === null) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      const hasPrompt = currentSystemPrompts.some((prompt) => prompt.name === name);
      const systemPrompts = hasPrompt
        ? currentSystemPrompts.map((prompt) =>
          prompt.name === name
            ? {
              ...prompt,
              content: defaultContent,
            }
            : prompt,
          )
        : [
          ...currentSystemPrompts,
          {
            content: defaultContent,
            name,
          },
        ];

      return {
        ...current,
        activeSystemPromptName: name,
        systemPrompts,
      };
    });
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
    if (!nativeBrowserToken) {
      setError(
        t(
          "Native file browsing is only available from a browser running on the Foco computer.",
        ),
      );
      return;
    }

    setIsSelectingPromptFile(true);
    setError(null);

    try {
      const data = await requestJson<{ files: NativeSelectedFile[] }>(
        "/api/native/select-files",
        nativePickerRequestInit(nativeBrowserToken),
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
          dream: {
            enabled: memorySettingsForm.dream.enabled,
            autoEnabled: memorySettingsForm.dream.autoEnabled,
            mode: memorySettingsForm.dream.mode,
            modelId: memorySettingsForm.dream.modelId.trim() || null,
            workspaceIntervalDays: requiredPositiveInteger(
              memorySettingsForm.dream.workspaceIntervalDays,
              t("Workspace interval days"),
            ),
            globalIntervalDays: requiredPositiveInteger(
              memorySettingsForm.dream.globalIntervalDays,
              t("Global interval days"),
            ),
            createTranscriptChat: memorySettingsForm.dream.createTranscriptChat,
            maxFactsPerRun: requiredPositiveInteger(
              memorySettingsForm.dream.maxFactsPerRun,
              t("Max facts per run"),
            ),
            maxChangesPerRun: requiredPositiveInteger(
              memorySettingsForm.dream.maxChangesPerRun,
              t("Max changes per run"),
            ),
            schedulerScanMinutes: requiredPositiveInteger(
              memorySettingsForm.dream.schedulerScanMinutes,
              t("Scheduler scan minutes"),
            ),
          },
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

  async function runMemoryDream(scope: MemoryDreamScope) {
    const workspaceId = scope === "workspace" ? memoryDreamWorkspaceId : null;
    const runKey = memoryDreamJobKey(scope, workspaceId);
    setMemoryDreamRunKey(runKey);
    setMemoryDreamError(null);

    try {
      await requestJson<MemoryDreamRunResponse>("/api/memory/dream/run", {
        body: JSON.stringify({
          scope,
          ...(workspaceId ? { workspaceId } : {}),
          triggerType: "manual",
          mode: memorySettingsForm.dream.mode,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemoryDreamJobs();
    } catch (requestError) {
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setMemoryDreamRunKey(null);
    }
  }

  function updateMemoryDreamForm(
    patch: Partial<MemorySettingsFormState["dream"]>,
  ) {
    setMemorySettingsForm((current) => ({
      ...current,
      dream: {
        ...current.dream,
        ...patch,
      },
    }));
  }

  function updateMemoryFilter(patch: Partial<MemoryFilterState>) {
    setMemoryFilter((current) => ({
      ...current,
      ...patch,
      page: 1,
    }));
  }

  function goToMemoryDreamPage(page: number) {
    const maxPage = memoryDreamTotalPages || 1;
    setMemoryDreamPage(Math.min(Math.max(1, page), maxPage));
  }

  function updateMemoryDreamPageSize(value: string) {
    setMemoryDreamPage(1);
    setMemoryDreamPageSize((current) =>
      Math.min(
        MEMORY_DREAM_MAX_PAGE_SIZE,
        positiveIntegerText(value, current),
      ),
    );
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
            contextWindow: optionalModelLimit(
              form.contextWindow,
              "Context window",
              modelOutputsText,
            ),
            maxOutputTokens: optionalModelLimit(
              form.maxOutputTokens,
              "Max output tokens",
              modelOutputsText,
            ),
            providerIds: form.providerIds,
            activeProviderId: form.activeProviderId,
            inputModalities: normalizeModalities(form.inputModalities),
            outputModalities: normalizeModalities(form.outputModalities),
            thinkingLevel: form.thinkingLevel || null,
            clearThinkingLevel: !form.thinkingLevel,
            systemPromptName: form.systemPromptName,
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
      if (editingWorkspace && isWorkspaceSpecSettingsLoaded) {
        try {
          await saveWorkspaceSpecSettingsRequest(
            workspaceForm.id,
            workspaceForm.specEnabled,
            workspaceForm.specEnabled ? workspaceForm.specInjectEnabled : false,
          );
        } catch (specError) {
          setError(errorMessage(specError));
        }
      }
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
    if (!nativeBrowserToken) {
      setError(
        t(
          "Native file browsing is only available from a browser running on the Foco computer.",
        ),
      );
      return;
    }

    setIsSelectingWorkspaceFormPath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        nativePickerRequestInit(nativeBrowserToken),
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

  function addProviderRequestOverride() {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: [
        ...current.requestOverrides,
        emptyProviderRequestOverride(),
      ],
    }));
  }

  function updateProviderRequestOverride(
    index: number,
    patch: Partial<ProviderRequestOverrideFormState>,
  ) {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: current.requestOverrides.map((overrideRule, overrideIndex) => {
        if (overrideIndex !== index) {
          return overrideRule;
        }

        const nextRule = { ...overrideRule, ...patch };
        if (patch.valueType === "boolean" && typeof nextRule.value !== "boolean") {
          nextRule.value = true;
        } else if (patch.valueType === "string" && typeof nextRule.value !== "string") {
          nextRule.value = String(nextRule.value);
        } else if (patch.valueType === "number" && typeof nextRule.value !== "number") {
          nextRule.value = "";
        }

        return nextRule;
      }),
    }));
  }

  function deleteProviderRequestOverride(index: number) {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: current.requestOverrides.filter(
        (_overrideRule, overrideIndex) => overrideIndex !== index,
      ),
    }));
  }

  async function saveProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingProvider(true);
    setError(null);

    try {
      const providerId =
        providerForm.id ||
        nextProviderId(providerForm.name, providerForm.kind, providers);
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
            id: providerId,
            kind: providerForm.kind,
            autoSyncModels: providerForm.autoSyncModels,
            modelSyncFilterRegex: providerForm.modelSyncFilterRegex || null,
            name: providerForm.name,
            requestOverrides: providerForm.requestOverrides.map((overrideRule) => ({
              ...overrideRule,
              value:
                overrideRule.valueType === "number"
                  ? Number(overrideRule.value)
                  : overrideRule.value,
            })),
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setExpandedProviderIds((current) => {
        const next = new Set(current);
        next.delete(providerId);
        return next;
      });
      setProviderModelLists((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      setProviderForm((current) => ({
        ...current,
        apiKey: "",
        clearApiKey: false,
      }));
      setIsProviderApiKeyVisible(false);
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
      setExpandedProviderIds((current) => {
        const next = new Set(current);
        next.delete(providerId);
        return next;
      });
      setProviderModelLists((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      setProviderForm({
        ...emptyProviderForm(),
        kind: defaultProviderKind(data.providerKinds),
      });
      setIsProviderApiKeyVisible(false);
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

  async function saveDefaultTeamModeEnabled(defaultTeamModeEnabled: boolean) {
    if (!settings) {
      return;
    }

    setIsSavingGeneral(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          defaultTeamModeEnabled,
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
    } catch (requestError) {
      setError(errorMessage(requestError));
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

  async function loadProviderModels(providerId: string) {
    setProviderModelLists((current) => ({
      ...current,
      [providerId]: { message: null, models: [], status: "loading" },
    }));
    setError(null);

    try {
      const data = await requestJson<ProviderModelsResponse>(
        "/api/providers/models",
        {
          body: JSON.stringify({ providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setProviderModelLists((current) => ({
        ...current,
        [providerId]: {
          message: null,
          models: data.models,
          status: "ok",
        },
      }));
    } catch (requestError) {
      setProviderModelLists((current) => ({
        ...current,
        [providerId]: {
          message: errorMessage(requestError),
          models: [],
          status: "error",
        },
      }));
    }
  }

  async function refreshProviderModels() {
    setIsRefreshingProviderModels(true);
    setError(null);

    try {
      const data = await requestJson<ProviderModelsRefreshResponse>(
        "/api/providers/models/refresh",
        { method: "POST" },
      );
      setSettings(data.settings);
      onSettingsChange(data.settings);
      setProviderTests({});
      setProviderModelLists((current) => {
        const next = { ...current };
        for (const provider of data.providers) {
          next[provider.providerId] = {
            message: null,
            models: provider.models,
            status: "ok",
          };
        }
        return next;
      });
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingProviderModels(false);
    }
  }

  function toggleProviderModels(providerId: string) {
    const shouldExpand = !expandedProviderIds.has(providerId);
    setExpandedProviderIds((current) => {
      const next = new Set(current);
      if (next.has(providerId)) {
        next.delete(providerId);
      } else {
        next.add(providerId);
      }
      return next;
    });

    if (shouldExpand && !providerModelLists[providerId]) {
      void loadProviderModels(providerId);
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

  function toggleModelModality(
    field: "inputModalities" | "outputModalities",
    modality: string,
    checked: boolean,
  ) {
    setForm((current) => {
      const values = checked
        ? [...current[field], modality]
        : current[field].filter((value) => value !== modality);

      return {
        ...current,
        [field]: normalizeModalities(values),
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
              active={activeSection === "agents"}
              icon={Bot}
              label={t("Agents")}
              onClick={() => onActiveSectionChange("agents")}
            />
            <SettingsNavButton
              active={activeSection === "prompts"}
              icon={ScrollText}
              label={t("Prompts")}
              onClick={() => onActiveSectionChange("prompts")}
            />
            <SettingsNavButton
              active={activeSection === "spec"}
              icon={FileText}
              label={t("Spec")}
              onClick={() => onActiveSectionChange("spec")}
            />
            <SettingsNavButton
              active={activeSection === "web-search"}
              icon={Search}
              label={t("Web Search")}
              onClick={() => onActiveSectionChange("web-search")}
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
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("LLM request retries")}
                    </span>
                    <input
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      inputMode="numeric"
                      min={1}
                      onChange={(event) =>
                        setGeneralForm((current) => ({
                          ...current,
                          llmRequestRetryCount: event.target.value,
                        }))
                      }
                      placeholder={String(settings?.general.llmRequestRetryCount ?? 3)}
                      step={1}
                      type="number"
                      value={generalForm.llmRequestRetryCount}
                    />
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("API request detail retention days")}
                    </span>
                    <input
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      inputMode="numeric"
                      min={1}
                      onChange={(event) =>
                        setGeneralForm((current) => ({
                          ...current,
                          apiRequestDetailRetentionDays: event.target.value,
                        }))
                      }
                      placeholder={String(
                        settings?.general.apiAudit.requestDetailRetentionDays ?? 3,
                      )}
                      step={1}
                      type="number"
                      value={generalForm.apiRequestDetailRetentionDays}
                    />
                  </label>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("API request details")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Database
                          aria-hidden="true"
                          className="size-4 shrink-0 text-teal-700"
                        />
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Save request and response bodies")}
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <CapabilityPill
                          label={
                            generalForm.apiSaveRequestResponseDetails
                              ? t("enabled")
                              : t("disabled")
                          }
                          ok={generalForm.apiSaveRequestResponseDetails}
                        />
                        <label
                          aria-label={t("Save request and response bodies")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                        >
                          <input
                            checked={generalForm.apiSaveRequestResponseDetails}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setGeneralForm((current) => ({
                                ...current,
                                apiSaveRequestResponseDetails: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                      </div>
                    </div>
                  </fieldset>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Startup")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Play
                          aria-hidden="true"
                          className="size-4 shrink-0 fill-current text-teal-700"
                        />
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Start Foco when Windows starts")}
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <CapabilityPill
                          label={
                            generalForm.autoStartEnabled ? t("enabled") : t("disabled")
                          }
                          ok={generalForm.autoStartEnabled}
                        />
                        <label
                          aria-label={t("Start Foco when Windows starts")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                        >
                          <input
                            checked={generalForm.autoStartEnabled}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setGeneralForm((current) => ({
                                ...current,
                                autoStartEnabled: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                      </div>
                    </div>
                  </fieldset>
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
                      !generalForm.listenPort.trim() ||
                      !generalForm.apiRequestDetailRetentionDays.trim()
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

          {activeSection === "web-search" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveWebSearchSettings(event)}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Search aria-hidden="true" className="size-5 text-teal-700" />
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("Web search")}
                    </h3>
                  </div>
                  <CapabilityPill
                    label={webSearchForm.enabled ? t("enabled") : t("disabled")}
                    ok={webSearchForm.enabled}
                  />
                </div>
                <div className="mt-4 grid gap-4">
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Runtime tool")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Expose web_search to chat runs")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-stone-500">
                          {t(
                            "web_fetch is available for known URLs; web_search requires an enabled search API.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Expose web_search to chat runs")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={webSearchForm.enabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Search API")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setWebSearchForm((current) => ({
                          ...current,
                          activeProvider: event.target.value,
                        }))
                      }
                      value={webSearchForm.activeProvider}
                    >
                      {(settings?.webSearch.providers ?? []).map((provider) => (
                        <option key={provider.provider} value={provider.provider}>
                          {provider.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Web search proxy")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Proxy search API requests")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-stone-500">
                          {t("Applies only to web_search requests sent to the configured search API.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable web search proxy")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={webSearchForm.apiProxyEnabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              apiProxyEnabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                    <div className="mt-3 grid gap-3 lg:grid-cols-[180px_1fr]">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Proxy type")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              apiProxyType: event.target.value,
                            }))
                          }
                          value={webSearchForm.apiProxyType}
                        >
                          {(settings?.webSearch.apiProxy.supportedTypes ?? apiProxyTypes).map(
                            (proxyType) => (
                              <option
                                key={proxyType.proxyType}
                                value={proxyType.proxyType}
                              >
                                {proxyType.label}
                              </option>
                            ),
                          )}
                        </select>
                      </label>
                      <TextField
                        label={t("Proxy server")}
                        onChange={(value) =>
                          setWebSearchForm((current) => ({
                            ...current,
                            apiProxyUrl: value,
                          }))
                        }
                        placeholder="127.0.0.1:7890"
                        value={webSearchForm.apiProxyUrl}
                      />
                    </div>
                  </fieldset>
                  <div className="grid gap-3 lg:grid-cols-2">
                    {(settings?.webSearch.providers ?? []).map((provider) => {
                      const keyField =
                        provider.provider === "brave"
                          ? "braveApiKey"
                          : "tavilyApiKey";
                      const clearField =
                        provider.provider === "brave"
                          ? "clearBraveApiKey"
                          : "clearTavilyApiKey";

                      return (
                        <div
                          className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3"
                          key={provider.provider}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-sm font-semibold text-stone-900">
                              {provider.label}
                            </span>
                            <CapabilityPill
                              label={provider.hasApiKey ? t("saved") : t("missing")}
                              ok={provider.hasApiKey}
                            />
                          </div>
                          <label className="mt-3 block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("API token")}
                            </span>
                            <input
                              autoComplete="off"
                              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                setWebSearchForm((current) => ({
                                  ...current,
                                  [keyField]: event.target.value,
                                }))
                              }
                              placeholder={
                                provider.hasApiKey
                                  ? t("Saved token is kept unless changed.")
                                  : t("Paste API token")
                              }
                              type="password"
                              value={String(webSearchForm[keyField])}
                            />
                          </label>
                          {provider.hasApiKey ? (
                            <label className="mt-3 flex items-center gap-2 text-xs font-semibold text-stone-600">
                              <input
                                checked={Boolean(webSearchForm[clearField])}
                                className="size-4 accent-teal-700"
                                onChange={(event) =>
                                  setWebSearchForm((current) => ({
                                    ...current,
                                    [clearField]: event.target.checked,
                                  }))
                                }
                                type="checkbox"
                              />
                              {t("Clear saved token")}
                            </label>
                          ) : null}
                        </div>
                      );
                    })}
                  </div>
                </div>
                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    aria-label={t("Save web search settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={isSavingWebSearch || !webSearchForm.activeProvider}
                    title={t("Save web search settings")}
                    type="submit"
                  >
                    {isSavingWebSearch ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                  <button
                    aria-label={t("Reload web search settings")}
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

          {activeSection === "agents" ? (
            <AgentsSettingsPanel
              agentTools={settings?.agentTools ?? []}
              defaultTeamModeEnabled={settings?.general.defaultTeamModeEnabled ?? false}
              definitions={agentDefinitions}
              error={agentDefinitionsError}
              isLoading={isLoadingAgentDefinitions}
              isSavingDefaultTeamMode={isSavingGeneral}
              models={configuredModelsByName}
              onCreateDefinition={onCreateAgentDefinition}
              onDefaultTeamModeEnabledChange={saveDefaultTeamModeEnabled}
              onDeleteDefinition={onDeleteAgentDefinition}
              onUpdateDefinition={onUpdateAgentDefinition}
              operationKey={agentDefinitionOperationKey}
              providers={providers}
              systemPrompts={savedSystemPrompts}
              thinkingLevels={thinkingLevels}
            />
          ) : null}

          {activeSection === "prompts" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void savePromptSettings(event)}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Bot aria-hidden="true" className="size-5 text-teal-700" />
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("System prompt")}
                    </h3>
                  </div>
                  <span className="rounded-full border border-stone-200 bg-stone-50 px-2.5 py-1 text-xs font-semibold text-stone-600">
                    {activeSystemPrompt?.name ?? DEFAULT_SYSTEM_PROMPT_NAME}
                  </span>
                </div>
                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(180px,240px)_minmax(0,1fr)]">
                  <div className="grid content-start gap-2">
                    <div className="overflow-hidden rounded-xl border border-stone-200 bg-stone-50/80">
                      {systemPrompts.map((prompt) => {
                        const isActive =
                          prompt.name === promptSettingsForm.activeSystemPromptName;
                        const isRenaming =
                          prompt.name === promptSettingsForm.renamingSystemPromptName;
                        const isFixed = isSystemPromptFixed(prompt.name);

                        return (
                          <div
                            className={`flex items-center gap-2 px-3 py-2 ${isActive
                                ? "bg-teal-50"
                                : "hover:bg-white"
                              }`}
                            key={prompt.name}
                          >
                            {isRenaming ? (
                              <>
                                <input
                                  aria-label={t("System prompt name")}
                                  autoComplete="off"
                                  autoFocus
                                  className="h-8 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-2 text-sm font-semibold text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                  onChange={(event) =>
                                    setPromptSettingsForm((current) => ({
                                      ...current,
                                      pendingSystemPromptRename: event.target.value,
                                    }))
                                  }
                                  onKeyDown={(event) => {
                                    if (event.key === "Enter") {
                                      event.preventDefault();
                                      submitRenameSystemPrompt(prompt.name);
                                    }
                                    if (event.key === "Escape") {
                                      event.preventDefault();
                                      cancelRenameSystemPrompt();
                                    }
                                  }}
                                  value={promptSettingsForm.pendingSystemPromptRename}
                                />
                                <button
                                  aria-label={t("Save system prompt name")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                                  disabled={
                                    !promptSettingsForm.pendingSystemPromptRename.trim()
                                  }
                                  onClick={() => submitRenameSystemPrompt(prompt.name)}
                                  title={t("Save system prompt name")}
                                  type="button"
                                >
                                  <CheckCircle2 aria-hidden="true" className="size-4" />
                                </button>
                                <button
                                  aria-label={t("Cancel system prompt rename")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                  onClick={cancelRenameSystemPrompt}
                                  title={t("Cancel system prompt rename")}
                                  type="button"
                                >
                                  <X aria-hidden="true" className="size-4" />
                                </button>
                              </>
                            ) : (
                              <>
                                <button
                                  className={`min-w-0 flex-1 truncate text-left text-sm font-semibold ${isActive
                                      ? "text-teal-900"
                                      : "text-stone-700"
                                    }`}
                                  onClick={() =>
                                    setPromptSettingsForm((current) => ({
                                      ...current,
                                      activeSystemPromptName: prompt.name,
                                    }))
                                  }
                                  type="button"
                                >
                                  {prompt.name}
                                </button>
                                {defaultSystemPromptContent(prompt.name) !== null ? (
                                  <button
                                    aria-label={t("Restore default system prompt")}
                                    className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                    disabled={isLoadingSettings || !settings}
                                    onClick={() => restoreSystemPromptDefault(prompt.name)}
                                    title={t("Restore default system prompt")}
                                    type="button"
                                  >
                                    <RefreshCw aria-hidden="true" className="size-4" />
                                  </button>
                                ) : isFixed ? null : (
                                  <>
                                    <button
                                      aria-label={t("Rename system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                      onClick={() => startRenameSystemPrompt(prompt.name)}
                                      title={t("Rename system prompt")}
                                      type="button"
                                    >
                                      <Pencil aria-hidden="true" className="size-4" />
                                    </button>
                                    <button
                                      aria-label={t("Remove system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                                      onClick={() => removeSystemPrompt(prompt.name)}
                                      title={t("Remove system prompt")}
                                      type="button"
                                    >
                                      <Trash2 aria-hidden="true" className="size-4" />
                                    </button>
                                  </>
                                )}
                              </>
                            )}
                          </div>
                        );
                      })}
                    </div>
                    <div className="flex gap-2">
                      <input
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setPromptSettingsForm((current) => ({
                            ...current,
                            pendingSystemPromptName: event.target.value,
                          }))
                        }
                        placeholder={t("Prompt name")}
                        value={promptSettingsForm.pendingSystemPromptName}
                      />
                      <button
                        aria-label={t("Add system prompt")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={!promptSettingsForm.pendingSystemPromptName.trim()}
                        onClick={() =>
                          addSystemPrompt(promptSettingsForm.pendingSystemPromptName)
                        }
                        title={t("Add system prompt")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("System prompt")}
                    </span>
                    <textarea
                      className="min-h-72 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateActiveSystemPromptContent(event.target.value)
                      }
                      value={activeSystemPrompt?.content ?? ""}
                    />
                  </label>
                </div>
                <div className="mt-6 flex items-center gap-2 border-t border-stone-200 pt-4">
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
                        disabled={isSelectingPromptFile || !canUseNativePicker}
                        onClick={() => void selectPromptFile()}
                        title={
                          canUseNativePicker
                            ? t("Choose prompt file")
                            : t("Local Foco browser required")
                        }
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
                </div>
              </form>
            </section>
          ) : null}

          {activeSection === "spec" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveSpecSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <FileText aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Auto Spec")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Automation")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Enable Auto Spec")}
                        </p>
                        <p className="mt-1 text-xs text-stone-500">
                          {t("Updates enabled workspace specs after successful chat turns.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable Auto Spec")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={specSettingsForm.autoEnabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setSpecSettingsForm((current) => ({
                              ...current,
                              autoEnabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec generation model")}
                    </span>
                    <select
                      aria-label={t("Spec generation model")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          generationModelId: event.target.value,
                        }))
                      }
                      value={specSettingsForm.generationModelId}
                    >
                      <option value="">{t("Automatic")}</option>
                      {configuredModelsByName.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.displayName}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec generation system prompt")}
                    </span>
                    <textarea
                      aria-label={t("Spec generation system prompt")}
                      className="min-h-44 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          generationSystemPrompt: event.target.value,
                        }))
                      }
                      placeholder={settings?.spec.defaultGenerationSystemPrompt ?? ""}
                      value={specSettingsForm.generationSystemPrompt}
                    />
                  </label>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec update system prompt")}
                    </span>
                    <textarea
                      aria-label={t("Spec update system prompt")}
                      className="min-h-44 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          updateSystemPrompt: event.target.value,
                        }))
                      }
                      placeholder={settings?.spec.defaultUpdateSystemPrompt ?? ""}
                      value={specSettingsForm.updateSystemPrompt}
                    />
                  </label>
                </div>

                <button
                  aria-label={t("Save spec settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={isSavingSpecSettings}
                  title={t("Save spec settings")}
                  type="submit"
                >
                  {isSavingSpecSettings ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                  {t("Save")}
                </button>
              </form>
            </section>
          ) : null}

          {activeSection === "memory" ? (
            <section className="grid gap-4">
              {isMemoryDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close memory dialog backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={closeMemoryDialog}
                    type="button"
                  />
                  <form
                    aria-label={
                      memoryDialogMode === "create"
                        ? t("Create memory")
                        : t("Edit memory")
                    }
                    className={`fixed left-1/2 top-1/2 z-50 max-h-[88vh] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)] ${memoryDialogMode === "edit" ? "w-[min(94vw,72rem)]" : "w-[min(92vw,34rem)]"
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
                            {configuredModelsByName.map((model) => (
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
                            {configuredModelsByName.map((model) => (
                              <option key={model.id} value={model.id}>
                                {model.displayName}
                              </option>
                            ))}
                          </select>
                        </label>
                      </div>
                    </fieldset>
                  </div>

                  <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Dream")}
                    </legend>
                    <div className="mb-3 flex items-start gap-2">
                      <Sparkles
                        aria-hidden="true"
                        className="mt-0.5 size-4 shrink-0 text-teal-700"
                      />
                      <p className="text-xs text-stone-500">
                        {t(
                          "Consolidates stale, duplicate, and pending memories without creating scheduled task rows.",
                        )}
                      </p>
                    </div>
                    <div className="grid gap-3 md:grid-cols-3">
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Enable Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.enabled}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  enabled: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Enable Auto Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Auto Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.autoEnabled}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  autoEnabled: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Create transcript chat")}
                          </span>
                          <label
                            aria-label={t("Create transcript chat")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.createTranscriptChat}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  createTranscriptChat: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                    </div>
                    <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Dream mode")}
                        </span>
                        <select
                          aria-label={t("Dream mode")}
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            updateMemoryDreamForm({
                              mode: event.target.value as MemoryDreamRunMode,
                            })
                          }
                          value={memorySettingsForm.dream.mode}
                        >
                          <option value="deterministic_only">
                            {t("Deterministic only")}
                          </option>
                          <option value="llm">{t("LLM")}</option>
                        </select>
                      </label>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Dream model")}
                        </span>
                        <select
                          aria-label={t("Dream model")}
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            updateMemoryDreamForm({
                              modelId: event.target.value,
                            })
                          }
                          value={memorySettingsForm.dream.modelId}
                        >
                          <option value="">{t("Fallback model")}</option>
                          {configuredModelsByName.map((model) => (
                            <option key={model.id} value={model.id}>
                              {model.displayName}
                            </option>
                          ))}
                        </select>
                      </label>
                      <TextField
                        inputMode="numeric"
                        label={t("Workspace interval days")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            workspaceIntervalDays: value,
                          })
                        }
                        placeholder="7"
                        value={memorySettingsForm.dream.workspaceIntervalDays}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Global interval days")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            globalIntervalDays: value,
                          })
                        }
                        placeholder="30"
                        value={memorySettingsForm.dream.globalIntervalDays}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Max facts per run")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            maxFactsPerRun: value,
                          })
                        }
                        placeholder="200"
                        value={memorySettingsForm.dream.maxFactsPerRun}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Max changes per run")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            maxChangesPerRun: value,
                          })
                        }
                        placeholder="50"
                        value={memorySettingsForm.dream.maxChangesPerRun}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Scheduler scan minutes")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            schedulerScanMinutes: value,
                          })
                        }
                        placeholder="60"
                        value={memorySettingsForm.dream.schedulerScanMinutes}
                      />
                    </div>
                  </fieldset>
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
                    <div className="flex items-center gap-2">
                      <Sparkles aria-hidden="true" className="size-5 text-teal-700" />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {t("Dream history")}
                      </h3>
                    </div>
                    <p className="mt-1 truncate text-xs text-stone-500">
                      {t("Memory maintenance jobs and applied changes")}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    <button
                      aria-label={t("Run workspace Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300"
                      disabled={
                        !isMemoryDreamRunnable ||
                        !memoryDreamWorkspaceId ||
                        activeMemoryDreamJobKeys.has(workspaceDreamRunKey) ||
                        memoryDreamRunKey === workspaceDreamRunKey
                      }
                      onClick={() => void runMemoryDream("workspace")}
                      title={t("Run workspace Dream now")}
                      type="button"
                    >
                      {memoryDreamRunKey === workspaceDreamRunKey ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <Play aria-hidden="true" className="size-4" />
                      )}
                      {t("Run workspace Dream now")}
                    </button>
                    <button
                      aria-label={t("Run global Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={
                        !isMemoryDreamRunnable ||
                        activeMemoryDreamJobKeys.has(globalDreamRunKey) ||
                        memoryDreamRunKey === globalDreamRunKey
                      }
                      onClick={() => void runMemoryDream("global")}
                      title={t("Run global Dream now")}
                      type="button"
                    >
                      {memoryDreamRunKey === globalDreamRunKey ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <Globe aria-hidden="true" className="size-4" />
                      )}
                      {t("Run global Dream now")}
                    </button>
                    <button
                      aria-label={t("Refresh Dream history")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={() => void loadMemoryDreamJobs()}
                      title={t("Refresh Dream history")}
                      type="button"
                    >
                      {isLoadingMemoryDreamJobs ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>

                <div className="mt-4 grid gap-3 md:grid-cols-4">
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Last successful run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {latestSuccessfulMemoryDreamJob
                        ? formatAuditDate(
                          latestSuccessfulMemoryDreamJob.completedAt ??
                          latestSuccessfulMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Last failed run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {latestFailedMemoryDreamJob
                        ? formatAuditDate(
                          latestFailedMemoryDreamJob.completedAt ??
                          latestFailedMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Next automatic run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {memoryDreamNextRunEstimate}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Latest applied changes")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {formatNumber(latestMemoryDreamChangeCount, language)}
                    </div>
                  </div>
                </div>

                {memoryDreamError ? (
                  <div className="mt-4 rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                    {memoryDreamError}
                  </div>
                ) : null}

                <div className="panel-scroll mt-4 overflow-x-auto rounded-xl border border-stone-200 bg-white">
                  <table className="min-w-full divide-y divide-stone-200 text-left text-sm">
                    <thead className="bg-stone-50 text-xs font-semibold uppercase tracking-wide text-stone-500">
                      <tr>
                        <th className="px-3 py-2">{t("Created")}</th>
                        <th className="px-3 py-2">{t("Scope")}</th>
                        <th className="px-3 py-2">{t("Trigger")}</th>
                        <th className="px-3 py-2">{t("Model")}</th>
                        <th className="px-3 py-2">{t("Status")}</th>
                        <th className="px-3 py-2">{t("Changes")}</th>
                        <th className="px-3 py-2 text-right">{t("Actions")}</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-stone-100">
                      {sortedMemoryDreamJobs.length === 0 ? (
                        <tr>
                          <td
                            className="px-3 py-6 text-center text-sm font-medium text-stone-500"
                            colSpan={7}
                          >
                            {isLoadingMemoryDreamJobs
                              ? t("Loading Dream history...")
                              : t("No Dream jobs")}
                          </td>
                        </tr>
                      ) : (
                        paginatedMemoryDreamJobs.map((job) => {
                          const transcriptWorkspaceId =
                            job.transcriptWorkspaceId ?? job.workspaceId;
                          const scopeLabel =
                            job.scope === "workspace"
                              ? workspaces.find((workspace) => workspace.id === job.workspaceId)
                                  ?.name ??
                                job.workspaceId ??
                                memoryDreamScopeLabel(job.scope, t)
                              : memoryDreamScopeLabel(job.scope, t);
                          return (
                            <tr
                              className="bg-white hover:bg-stone-50"
                              key={job.id}
                            >
                              <td className="px-3 py-2 align-top">
                                <span className="text-xs font-semibold text-stone-700">
                                  {formatAuditDate(job.createdAt, language)}
                                </span>
                              </td>
                              <td className="px-3 py-2 align-top">
                                {scopeLabel}
                              </td>
                              <td className="px-3 py-2 align-top">
                                {memoryDreamTriggerLabel(job.triggerType, t)}
                              </td>
                              <td className="px-3 py-2 align-top">
                                {job.modelId ?? t("Default")}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <CapabilityPill
                                  label={memoryDreamStatusLabel(job.status, t)}
                                  ok={job.status === "completed"}
                                />
                              </td>
                              <td className="px-3 py-2 align-top text-xs text-stone-600">
                                {t("added {count}", {
                                  count: job.changeCounts.added,
                                })}
                                {", "}
                                {t("updated {count}", {
                                  count: job.changeCounts.updated,
                                })}
                                {", "}
                                {t("superseded {count}", {
                                  count: job.changeCounts.superseded,
                                })}
                                {", "}
                                {t("expired {count}", {
                                  count: job.changeCounts.expired,
                                })}
                                {", "}
                                {t("rejected {count}", {
                                  count: job.changeCounts.rejected,
                                })}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <div className="flex items-center justify-end gap-1">
                                  <button
                                    aria-label={t("View details")}
                                    className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      setMemoryDreamDetailJobId(job.id);
                                    }}
                                    title={t("View details")}
                                    type="button"
                                  >
                                    <Eye aria-hidden="true" className="size-4" />
                                  </button>
                                  {job.transcriptChatId && transcriptWorkspaceId ? (
                                    <button
                                      aria-label={t("Open transcript")}
                                      className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                      onClick={(event) => {
                                        event.stopPropagation();
                                        onOpenChat(transcriptWorkspaceId, job.transcriptChatId!);
                                      }}
                                      title={t("Open transcript")}
                                      type="button"
                                    >
                                      <ScrollText aria-hidden="true" className="size-4" />
                                    </button>
                                  ) : job.transcriptChatId ? (
                                    <span className="text-xs text-stone-500">
                                      {job.transcriptChatId}
                                    </span>
                                  ) : null}
                                </div>
                              </td>
                            </tr>
                          );
                        })
                      )}
                    </tbody>
                  </table>
                </div>

                <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 pt-3 text-sm">
                  <div className="text-stone-500">
                    {t("Showing {start}-{end} of {total}", {
                      end: formatNumber(memoryDreamPageEnd, language),
                      start: formatNumber(memoryDreamPageStart, language),
                      total: formatNumber(sortedMemoryDreamJobs.length, language),
                    })}
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                      <span>{t("Page size")}</span>
                      <input
                        className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        max={MEMORY_DREAM_MAX_PAGE_SIZE}
                        min={1}
                        onChange={(event) => updateMemoryDreamPageSize(event.target.value)}
                        type="number"
                        value={memoryDreamPageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Dream history pagination")}
                      className="flex items-center gap-1"
                    >
                      <button
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={isLoadingMemoryDreamJobs || currentMemoryDreamPage <= 1}
                        onClick={() => goToMemoryDreamPage(currentMemoryDreamPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </button>
                      {memoryDreamPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-stone-400"
                            key={`memory-dream-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <button
                            aria-current={
                              item === currentMemoryDreamPage ? "page" : undefined
                            }
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === currentMemoryDreamPage
                                ? "border-teal-700 bg-teal-700 text-white"
                                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            disabled={isLoadingMemoryDreamJobs}
                            key={item}
                            onClick={() => goToMemoryDreamPage(item)}
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
                          isLoadingMemoryDreamJobs ||
                          memoryDreamTotalPages === 0 ||
                          currentMemoryDreamPage >= memoryDreamTotalPages
                        }
                        onClick={() => goToMemoryDreamPage(currentMemoryDreamPage + 1)}
                        title={t("Next page")}
                        type="button"
                      >
                        <ChevronRight aria-hidden="true" className="size-4" />
                      </button>
                    </nav>
                  </div>
                </div>

                {memoryDreamDetailJob ? (
                  <>
                    <button
                      aria-label={t("Close Dream job details backdrop")}
                      className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                      onClick={closeMemoryDreamDetailDialog}
                      type="button"
                    />
                    <div
                      aria-labelledby="memory-dream-detail-title"
                      aria-modal="true"
                      className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(94vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                      role="dialog"
                    >
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <Sparkles aria-hidden="true" className="size-5 text-teal-700" />
                            <h4
                              className="text-sm font-semibold text-stone-950"
                              id="memory-dream-detail-title"
                            >
                              {t("Dream job details")}
                            </h4>
                            <CapabilityPill
                              label={memoryDreamStatusLabel(memoryDreamDetailJob.status, t)}
                              ok={memoryDreamDetailJob.status === "completed"}
                            />
                            <CapabilityPill
                              label={memoryDreamScopeLabel(memoryDreamDetailJob.scope, t)}
                              ok={memoryDreamDetailJob.scope === "workspace"}
                            />
                          </div>
                          <div className="mt-1 text-xs text-stone-500">
                            {formatAuditDate(memoryDreamDetailJob.createdAt, language)}
                          </div>
                        </div>
                        <button
                          aria-label={t("Close Dream job details")}
                          className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                          onClick={closeMemoryDreamDetailDialog}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                      <p className="text-sm text-stone-600">
                        {memoryDreamDetailJob.summary ||
                          memoryDreamDetailJob.errorMessage ||
                          t("No summary")}
                      </p>
                      <div className="mt-3 grid gap-3">
                        {isLoadingMemoryDreamChanges ? (
                          <div className="text-sm text-stone-500">
                            {t("Loading Dream changes...")}
                          </div>
                        ) : memoryDreamChangesByOperation.length === 0 ? (
                          <div className="text-sm text-stone-500">
                            {t("No Dream changes")}
                          </div>
                        ) : (
                          memoryDreamChangesByOperation.map((group) => (
                            <div
                              className="rounded-lg border border-stone-200 bg-white px-3 py-3"
                              key={group.operation}
                            >
                              <div className="mb-3 flex flex-wrap items-center gap-2">
                                <h5 className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                                  {memoryDreamChangeOperationLabel(group.operation, t)}
                                </h5>
                                <CapabilityPill
                                  label={formatNumber(group.changes.length, language)}
                                  ok
                                />
                              </div>
                              <div className="grid gap-3">
                                {group.changes.map((change) => (
                                  <div
                                    className="rounded-lg border border-stone-100 bg-stone-50/70 px-3 py-3"
                                    key={change.id}
                                  >
                                    <div className="flex flex-wrap items-center gap-2">
                                      <CapabilityPill
                                        label={memoryDreamChangeStatusLabel(change.status, t)}
                                        ok={change.status === "applied"}
                                      />
                                      <CapabilityPill
                                        label={memoryDreamRiskLabel(change.riskLevel, t)}
                                        ok={change.riskLevel === "low"}
                                      />
                                      {change.confidence !== null ? (
                                        <span className="text-xs font-semibold text-stone-500">
                                          {formatNumber(
                                            Math.round(change.confidence * 100),
                                            language,
                                          )}
                                          %
                                        </span>
                                      ) : null}
                                    </div>
                                    <div className="mt-2 text-sm font-semibold text-stone-900">
                                      {change.reason}
                                    </div>
                                    {change.targetFactIds.length ? (
                                      <div className="mt-1 break-all text-xs text-stone-500">
                                        {change.targetFactIds.join(", ")}
                                      </div>
                                    ) : null}
                                    {change.errorMessage ? (
                                      <div className="mt-2 text-sm text-rose-700">
                                        {change.errorMessage}
                                      </div>
                                    ) : null}
                                    <div className="mt-3 grid gap-3 lg:grid-cols-3">
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("Before JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Memory state before this Dream change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.beforeJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("After JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Memory state Dream wrote or proposed.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.afterJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("Evidence JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Sources Dream used to justify the change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.evidence)}
                                        </pre>
                                      </div>
                                    </div>
                                  </div>
                                ))}
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </>
                ) : null}
              </section>

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
                        className={`grid gap-3 rounded-xl border px-3 py-3 sm:grid-cols-[minmax(0,1fr)_auto] ${selectedMemoryId === memory.id
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
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === memoryListMeta.page
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
                  <button
                    aria-label={t("Close workspace configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsWorkspaceDialogOpen(false)}
                    type="button"
                  />
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
                            disabled={isSelectingWorkspaceFormPath || !canUseNativePicker}
                            onClick={() => void selectWorkspaceFormPath()}
                            title={
                              canUseNativePicker
                                ? t("Choose workspace path")
                                : t("Local Foco browser required")
                            }
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
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                        <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-700">
                          <ScrollText
                            aria-hidden="true"
                            className="size-4 shrink-0 text-teal-700"
                          />
                          <span className="truncate">{t("Enable Project Spec")}</span>
                        </span>
                        <input
                          checked={workspaceForm.specEnabled}
                          className="size-4 accent-teal-700"
                          disabled={
                            isLoadingWorkspaceSpecSettings ||
                            !isWorkspaceSpecSettingsLoaded
                          }
                          onChange={(event) =>
                            setWorkspaceForm((current) => ({
                              ...current,
                              specEnabled: event.target.checked,
                              specInjectEnabled: event.target.checked
                                ? current.specInjectEnabled
                                : false,
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
                          isLoadingWorkspaceSpecSettings ||
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
                        className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${draggedWorkspaceId === workspace.id
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
                            className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${isSavingWorkspaceOrder
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
                            className={`inline-flex size-9 items-center justify-center rounded-lg border shadow-sm ${workspace.pinned
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
                            onClick={() => void editConfiguredWorkspace(workspace)}
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
                  <button
                    aria-label={t("Close hook run detail backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setHookRunDetail(null)}
                    type="button"
                  />
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
                  <button
                    aria-label={t("Close hook configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsHookDialogOpen(false)}
                    type="button"
                  />
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
                        className={`rounded-md px-3 text-sm font-semibold ${hookScope === scope
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
                      className={`mt-3 rounded-lg border px-3 py-2 text-sm ${hookImportResult.saved
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
                  <button
                    aria-label={t("Close provider configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsProviderDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Provider configuration")}
                    className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[min(96vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
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
                    <div className="grid gap-4 lg:grid-cols-[13rem_minmax(0,1fr)]">
                      <div className="flex h-full min-h-0 flex-col rounded-xl border border-stone-200 bg-stone-50/70 p-2">
                          <div className="px-2 pb-2 text-xs font-semibold text-stone-600">
                            {t("Service provider")}
                          </div>
                          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
                            {providerServices.map((service) => (
                              <button
                                aria-pressed={selectedProviderServiceId === service.id}
                                className={`flex min-h-9 w-full items-center justify-between gap-2 rounded-lg px-2 py-2 text-left text-sm font-semibold transition ${selectedProviderServiceId === service.id
                                    ? "bg-teal-700 text-white"
                                    : "text-stone-700 hover:bg-white hover:text-teal-800"
                                  }`}
                                key={service.id}
                                onClick={() => applyProviderService(service.id)}
                                type="button"
                              >
                                <span className="min-w-0 truncate">{service.label}</span>
                                <span
                                  className={`rounded-md px-1.5 py-0.5 text-[11px] ${selectedProviderServiceId === service.id
                                      ? "bg-white/15 text-white"
                                      : "bg-stone-200 text-stone-600"
                                    }`}
                                >
                                  {formatNumber(service.kindIds.length, language)}
                                </span>
                              </button>
                            ))}
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
                            updateProviderProtocol(event.target.value)
                          }
                          value={providerForm.kind || defaultProviderKind(providerKinds)}
                        >
                          {providerProtocolKinds.map((kind) => (
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
                            className={`h-10 w-full rounded-lg border border-stone-300 bg-white px-3 ${hasProviderKeyClearButton ? "pr-20" : "pr-11"} text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100`}
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
                            type={isProviderApiKeyVisible ? "text" : "password"}
                            value={providerForm.apiKey}
                          />
                          <button
                            aria-label={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            className={`absolute ${hasProviderKeyClearButton ? "right-10" : "right-1"} top-1 inline-flex size-8 items-center justify-center rounded-md text-stone-500 hover:bg-stone-100 hover:text-stone-900`}
                            onClick={() =>
                              setIsProviderApiKeyVisible((current) => !current)
                            }
                            title={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            type="button"
                          >
                            {isProviderApiKeyVisible ? (
                              <EyeOff aria-hidden="true" className="size-4" />
                            ) : (
                              <Eye aria-hidden="true" className="size-4" />
                            )}
                          </button>
                          {hasProviderKeyClearButton ? (
                            <button
                              aria-label={t("Clear saved API key")}
                              className={`absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md ${providerForm.clearApiKey
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
                            <RefreshCw aria-hidden="true" className="size-4 text-teal-700" />
                            <h4 className="text-sm font-semibold text-stone-950">
                              {t("Model sync")}
                            </h4>
                          </div>
                          <CapabilityPill
                            label={
                              providerForm.autoSyncModels
                                ? t("auto sync")
                                : t("manual sync")
                            }
                            ok={providerForm.autoSyncModels}
                          />
                        </div>
                        <div className="mt-3 grid gap-3">
                          <label className="inline-flex items-center gap-2 text-sm font-semibold text-stone-700">
                            <input
                              aria-label={t("Auto sync provider models")}
                              checked={providerForm.autoSyncModels}
                              className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                              onChange={(event) =>
                                setProviderForm((current) => ({
                                  ...current,
                                  autoSyncModels: event.target.checked,
                                }))
                              }
                              type="checkbox"
                            />
                            {t("Auto sync provider models")}
                          </label>
                          <TextField
                            label={t("Model sync filter regex")}
                            onChange={(value) =>
                              setProviderForm((current) => ({
                                ...current,
                                modelSyncFilterRegex: value,
                              }))
                            }
                            placeholder="^gpt-4|^o"
                            value={providerForm.modelSyncFilterRegex}
                          />
                        </div>
                      </div>
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
                      <div className="rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <SlidersHorizontal aria-hidden="true" className="size-4 text-teal-700" />
                            <h4 className="text-sm font-semibold text-stone-950">
                              {t("Request overrides")}
                            </h4>
                          </div>
                          <button
                            className="inline-flex h-8 items-center gap-1 rounded-lg border border-stone-200 bg-white px-2 text-xs font-semibold text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={addProviderRequestOverride}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-3.5" />
                            {t("Add override")}
                          </button>
                        </div>
                        <p className="mt-2 text-xs leading-5 text-stone-500">
                          {t("Override top-level request headers or body fields for this provider.")}
                        </p>
                        <div className="mt-3 space-y-3">
                          {providerForm.requestOverrides.length ? (
                            providerForm.requestOverrides.map((overrideRule, overrideIndex) => (
                              <div
                                className="rounded-lg border border-stone-200 bg-white p-3"
                                key={overrideIndex}
                              >
                                <div className="grid gap-3 lg:grid-cols-[7rem_minmax(0,1fr)_8rem_minmax(0,1fr)_2.5rem]">
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Target")}
                                    </span>
                                    <select
                                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                      onChange={(event) =>
                                        updateProviderRequestOverride(overrideIndex, {
                                          target: event.target.value as ProviderRequestOverrideTarget,
                                        })
                                      }
                                      value={overrideRule.target}
                                    >
                                      <option value="header">{t("Header")}</option>
                                      <option value="body">{t("Body")}</option>
                                    </select>
                                  </label>
                                  <TextField
                                    label={t("Field")}
                                    onChange={(value) =>
                                      updateProviderRequestOverride(overrideIndex, { name: value })
                                    }
                                    placeholder={overrideRule.target === "header" ? "User-Agent" : "model"}
                                    value={overrideRule.name}
                                  />
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Value type")}
                                    </span>
                                    <select
                                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                      onChange={(event) =>
                                        updateProviderRequestOverride(overrideIndex, {
                                          valueType: event.target.value as ProviderRequestOverrideValueType,
                                        })
                                      }
                                      value={overrideRule.valueType}
                                    >
                                      <option value="string">{t("String")}</option>
                                      <option value="number">{t("Number")}</option>
                                      <option value="boolean">{t("Boolean")}</option>
                                    </select>
                                  </label>
                                  {overrideRule.valueType === "boolean" ? (
                                    <label className="block">
                                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                        {t("Value")}
                                      </span>
                                      <select
                                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                        onChange={(event) =>
                                          updateProviderRequestOverride(overrideIndex, {
                                            value: event.target.value === "true",
                                          })
                                        }
                                        value={overrideRule.value ? "true" : "false"}
                                      >
                                        <option value="true">true</option>
                                        <option value="false">false</option>
                                      </select>
                                    </label>
                                  ) : (
                                    <TextField
                                      label={t("Value")}
                                      onChange={(value) =>
                                        updateProviderRequestOverride(overrideIndex, { value })
                                      }
                                      placeholder={
                                        overrideRule.valueType === "number"
                                          ? "1"
                                          : overrideRule.target === "header"
                                            ? "Foco/1.0"
                                            : "gpt-4.1"
                                      }
                                      value={String(overrideRule.value)}
                                    />
                                  )}
                                  <button
                                    aria-label={t("Delete override")}
                                    className="mt-6 inline-flex size-10 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 hover:bg-rose-50"
                                    onClick={() => deleteProviderRequestOverride(overrideIndex)}
                                    title={t("Delete override")}
                                    type="button"
                                  >
                                    <Trash2 aria-hidden="true" className="size-4" />
                                  </button>
                                </div>
                              </div>
                            ))
                          ) : (
                            <p className="rounded-lg border border-dashed border-stone-300 bg-white px-3 py-3 text-xs text-stone-500">
                              {t("No request overrides configured.")}
                            </p>
                          )}
                        </div>
                      </div>
                      <button
                        aria-label={t("Save provider")}
                        className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                        disabled={
                          isSavingProvider ||
                          !providerForm.name.trim() ||
                          !providerForm.kind.trim() ||
                          providerForm.requestOverrides.some(
                            (overrideRule) =>
                              !overrideRule.name.trim() ||
                              (overrideRule.valueType !== "boolean" &&
                                String(overrideRule.value).trim() === "") ||
                              (overrideRule.valueType === "number" &&
                                Number.isNaN(Number(overrideRule.value))),
                          )
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
                      aria-label={t("Refresh provider models")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      disabled={isLoadingSettings || isRefreshingProviderModels}
                      onClick={() => void refreshProviderModels()}
                      title={t("Refresh provider models")}
                      type="button"
                    >
                      {isRefreshingProviderModels ? (
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
                      const modelList = providerModelLists[provider.id];
                      const isExpanded = expandedProviderIds.has(provider.id);

                      return (
                        <div className="px-4 py-3" key={provider.id}>
                          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                            <button
                              aria-expanded={isExpanded}
                              aria-label={
                                isExpanded
                                  ? t("Hide provider models for {name}", {
                                    name: provider.name,
                                  })
                                  : t("Load provider models for {name}", {
                                    name: provider.name,
                                  })
                              }
                              className="-mx-2 -my-1 flex min-w-0 items-start gap-2 rounded-lg px-2 py-1 text-left hover:bg-stone-50 focus:outline-none focus:ring-2 focus:ring-teal-100"
                              onClick={() => toggleProviderModels(provider.id)}
                              title={
                                isExpanded
                                  ? t("Hide provider models")
                                  : t("Load provider models")
                              }
                              type="button"
                            >
                              <ChevronDown
                                aria-hidden="true"
                                className={`mt-0.5 size-4 shrink-0 text-stone-400 transition ${isExpanded ? "" : "-rotate-90"}`}
                              />
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
                                  <CapabilityPill
                                    label={
                                      provider.autoSyncModels
                                        ? t("auto sync")
                                        : t("manual sync")
                                    }
                                    ok={provider.autoSyncModels}
                                  />
                                </div>
                                <div className="mt-1 truncate text-xs font-medium text-stone-500">
                                  {provider.id} / {provider.kindLabel}
                                </div>
                                {provider.modelSyncFilterRegex ? (
                                  <div className="mt-1 truncate font-mono text-xs text-stone-500">
                                    {t("sync regex {pattern}", {
                                      pattern: provider.modelSyncFilterRegex,
                                    })}
                                  </div>
                                ) : null}
                                {provider.baseUrl ? (
                                  <div className="mt-1 truncate text-xs text-stone-500">
                                    {provider.baseUrl}
                                  </div>
                                ) : null}
                              </div>
                            </button>
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
                          {isExpanded ? (
                            <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50/70 px-3 py-3">
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-900">
                                  <ListChecks
                                    aria-hidden="true"
                                    className="size-4 shrink-0 text-teal-700"
                                  />
                                  <span>{t("Provider models")}</span>
                                </div>
                                {modelList?.status === "ok" ? (
                                  <CapabilityPill
                                    label={t("models {count}", {
                                      count: modelList.models.length,
                                    })}
                                    ok={modelList.models.length > 0}
                                  />
                                ) : null}
                              </div>
                              {modelList?.status === "error" ? (
                                <div className="mt-3 rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                                  {modelList.message}
                                </div>
                              ) : modelList?.status === "ok" ? (
                                modelList.models.length ? (
                                  <div className="mt-3 max-h-56 overflow-y-auto rounded-md border border-stone-200 bg-white">
                                    {modelList.models.map((modelId, modelIndex) => (
                                      <div
                                        className="border-b border-stone-100 px-3 py-2 font-mono text-xs text-stone-700 last:border-b-0"
                                        key={`${modelId}-${modelIndex}`}
                                      >
                                        {modelId}
                                      </div>
                                    ))}
                                  </div>
                                ) : (
                                  <div className="mt-3 rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-500">
                                    {t("No provider models returned")}
                                  </div>
                                )
                              ) : (
                                <div className="mt-3 flex items-center gap-2 rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-500">
                                  <LoaderCircle
                                    aria-hidden="true"
                                    className="size-4 animate-spin"
                                  />
                                  {t("Loading provider models...")}
                                </div>
                              )}
                            </div>
                          ) : null}
                          {test ? (
                            <div
                              className={`mt-3 rounded-lg border px-3 py-2 text-sm ${test.status === "ok"
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
                  <button
                    aria-label={t("Close MCP server configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsMcpDialogOpen(false)}
                    type="button"
                  />
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
                        className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${draggedModelId === model.id
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
                            className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${isSavingModelOrder
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
                  <button
                    aria-label={t("Close model configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsModelDialogOpen(false)}
                    type="button"
                  />
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
                      <div className="grid gap-3 sm:grid-cols-2">
                        <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                          <legend className="px-1 text-xs font-semibold text-stone-600">
                            {t("Input types")}
                          </legend>
                          <div className="grid gap-2">
                            {inputModalityOptions.map((modality) => (
                              <label
                                className="flex items-center justify-between gap-3 rounded-lg bg-white px-3 py-2 text-sm font-medium text-stone-700"
                                key={modality}
                              >
                                <span>{t(modality)}</span>
                                <input
                                  checked={form.inputModalities.includes(modality)}
                                  className="size-4 accent-teal-700"
                                  onChange={(event) =>
                                    toggleModelModality(
                                      "inputModalities",
                                      modality,
                                      event.target.checked,
                                    )
                                  }
                                  type="checkbox"
                                />
                              </label>
                            ))}
                          </div>
                        </fieldset>
                        <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                          <legend className="px-1 text-xs font-semibold text-stone-600">
                            {t("Output types")}
                          </legend>
                          <div className="grid gap-2">
                            {outputModalityOptions.map((modality) => (
                              <label
                                className="flex items-center justify-between gap-3 rounded-lg bg-white px-3 py-2 text-sm font-medium text-stone-700"
                                key={modality}
                              >
                                <span>{t(modality)}</span>
                                <input
                                  checked={form.outputModalities.includes(modality)}
                                  className="size-4 accent-teal-700"
                                  onChange={(event) =>
                                    toggleModelModality(
                                      "outputModalities",
                                      modality,
                                      event.target.checked,
                                    )
                                  }
                                  type="checkbox"
                                />
                              </label>
                            ))}
                          </div>
                        </fieldset>
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
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("System prompt")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setForm((current) => ({
                              ...current,
                              systemPromptName: event.target.value,
                            }))
                          }
                          value={form.systemPromptName}
                        >
                          {savedSystemPrompts.map((prompt) => (
                            <option key={prompt.name} value={prompt.name}>
                              {prompt.name}
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
                        className={`grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-stone-100 px-4 py-3 text-left hover:bg-stone-50 ${selectedMetadataKey === model.key ? "bg-teal-50" : "bg-white/70"
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
    <span
      aria-hidden="true"
      className="foco-logo-mark inline-flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-white shadow-[0_10px_24px_rgba(15,118,110,0.2)] ring-1 ring-stone-200/80"
      dangerouslySetInnerHTML={{ __html: focoLogoSvg }}
    />
  );
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

function hydrateAgentTab(
  tab: OpenAgentTab,
  workspaces: WorkspaceSummary[],
): OpenAgentTab & {
  title: string;
  workspaceName: string;
  workspaceLogoUrl: string | null;
} {
  const workspace = workspaces.find((workspace) => workspace.id === tab.workspaceId);

  return {
    ...tab,
    title: tab.fallbackTitle,
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

function browserRouteWithOpenTabs(
  route: Extract<BrowserRoute, { viewMode: "chat" }>,
  chatTabs: OpenChatTab[],
  fileTabs: OpenFileTab[],
): BrowserRoute {
  const nextRoute = route.tabs
    ? { ...route, tabs: dedupeBrowserRouteChatTabs(route.tabs) }
    : browserRouteWithOpenChatTabs(route, chatTabs);

  const routeFiles = route.files
    ? dedupeBrowserRouteFileTabs(route.files)
    : openFileTabsToBrowserRouteFileTabs(fileTabs);
  if (route.activeFile) {
    routeFiles.push(route.activeFile);
  }

  const dedupedFiles = dedupeBrowserRouteFileTabs(routeFiles);
  return {
    ...nextRoute,
    ...(dedupedFiles.length ? { files: dedupedFiles } : {}),
    ...(route.activeFile ? { activeFile: route.activeFile } : {}),
  };
}

function browserRouteWithOpenChatTabs(
  route: Extract<BrowserRoute, { viewMode: "chat" }>,
  tabs: OpenChatTab[],
): Extract<BrowserRoute, { viewMode: "chat" }> {
  const routeTabs = openChatTabsToBrowserRouteTabs(tabs);
  if (route.workspaceId && route.chatId) {
    routeTabs.push({ chatId: route.chatId, workspaceId: route.workspaceId });
  }

  return { ...route, tabs: dedupeBrowserRouteChatTabs(routeTabs) };
}

function openChatTabsToBrowserRouteTabs(tabs: OpenChatTab[]): BrowserRouteChatTab[] {
  return tabs.map((tab) => ({
    chatId: tab.chatId,
    workspaceId: tab.workspaceId,
  }));
}

function openFileTabsToBrowserRouteFileTabs(tabs: OpenFileTab[]): BrowserRouteFileTab[] {
  return tabs.map((tab) => ({
    path: tab.path,
    workspaceId: tab.workspaceId,
  }));
}

function browserRouteFileTabToOpenFileTab(
  file: BrowserRouteFileTab,
  workspace: WorkspaceSummary,
): OpenFileTab {
  return {
    name: fileNameFromPath(file.path),
    path: file.path,
    workspaceId: file.workspaceId,
    workspaceLogoUrl: workspace.logoUrl ?? null,
    workspaceName: workspace.name,
  };
}

function fileNameFromPath(path: string) {
  const normalized = path.replaceAll("\\", "/");
  return normalized.split("/").filter(Boolean).at(-1) ?? path;
}

function dedupeBrowserRouteChatTabs(tabs: BrowserRouteChatTab[]) {
  const seen = new Set<string>();
  return tabs.filter((tab) => {
    const key = `${tab.workspaceId}\u0000${tab.chatId}`;
    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function dedupeBrowserRouteFileTabs(tabs: BrowserRouteFileTab[]) {
  const seen = new Set<string>();
  return tabs.filter((tab) => {
    const key = `${tab.workspaceId}\u0000${tab.path}`;
    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function upsertOpenAgentTab(tabs: OpenAgentTab[], nextTab: OpenAgentTab) {
  if (
    tabs.some(
      (tab) =>
        tab.workspaceId === nextTab.workspaceId &&
        tab.chatId === nextTab.chatId &&
        tab.instanceId === nextTab.instanceId,
    )
  ) {
    return tabs;
  }

  return [...tabs, nextTab];
}

function upsertOpenFileTab(tabs: OpenFileTab[], nextTab: OpenFileTab) {
  if (
    tabs.some(
      (tab) => tab.workspaceId === nextTab.workspaceId && tab.path === nextTab.path,
    )
  ) {
    return tabs;
  }

  return [...tabs, nextTab];
}

function mainTabKey(tab: MainTabSummary) {
  if (tab.type === "chat") {
    return `chat:${chatRunKey(tab.workspaceId, tab.chatId)}`;
  }

  if (tab.type === "agent") {
    return `agent:${tab.workspaceId}:${tab.chatId}:${tab.instanceId}`;
  }

  return workspaceFileEditorKey(tab.workspaceId, tab.path);
}

function mainTabMatches(activeTab: ActiveMainTab, tab: MainTabSummary) {
  if (activeTab.type !== tab.type || activeTab.workspaceId !== tab.workspaceId) {
    return false;
  }

  if (tab.type === "chat") {
    return activeTab.type === "chat" && activeTab.chatId === tab.chatId;
  }

  if (tab.type === "agent") {
    return (
      activeTab.type === "agent" &&
      activeTab.chatId === tab.chatId &&
      activeTab.instanceId === tab.instanceId
    );
  }

  return activeTab.type === "file" && activeTab.path === tab.path;
}

function workspaceFileEditorKey(workspaceId: string, path: string) {
  return `${workspaceId}:${path}`;
}

function isMarkdownFilePath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return extension === "md" || extension === "markdown";
}

function workspaceRenamedFilePath(path: string, newName: string) {
  const separatorIndex = path.lastIndexOf("/");
  return separatorIndex < 0
    ? newName
    : `${path.slice(0, separatorIndex + 1)}${newName}`;
}

function replaceWorkspaceFileNodeChildren(
  node: WorkspaceFileTreeNode,
  path: string,
  children: WorkspaceFileTreeNode[],
): WorkspaceFileTreeNode {
  if (node.path === path) {
    return {
      ...node,
      children,
      childrenLoaded: true,
      hasChildren: children.length > 0,
    };
  }

  if (!node.children.length) {
    return node;
  }

  return {
    ...node,
    children: node.children.map((child) =>
      replaceWorkspaceFileNodeChildren(child, path, children),
    ),
  };
}

function registerTomlMonacoLanguage(monaco: typeof Monaco) {
  if (monaco.languages.getLanguages().some((language) => language.id === "toml")) {
    return;
  }

  monaco.languages.register({
    id: "toml",
    aliases: ["TOML", "toml"],
    extensions: [".toml"],
    mimetypes: ["application/toml"],
  });
  monaco.languages.setMonarchTokensProvider("toml", {
    defaultToken: "",
    tokenPostfix: ".toml",
    escapes: /\\(?:[btnfr"\\]|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,
    tokenizer: {
      root: [
        [/^\s*(\[+)([^\]]+)(\]+)/, ["delimiter.bracket", "type.identifier", "delimiter.bracket"]],
        [/^\s*([A-Za-z0-9_-]+)(\s*=)/, ["key", "delimiter"]],
        { include: "@values" },
        [/#.*$/, "comment"],
      ],
      values: [
        [/"""/, "string", "multilineDoubleString"],
        [/'''/, "string", "multilineSingleString"],
        [/"/, "string", "doubleString"],
        [/'/, "string", "singleString"],
        [/\b(?:true|false)\b/, "keyword"],
        [/\b\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)?\b/, "number.date"],
        [/[+-]?\b(?:0x[0-9A-Fa-f_]+|0o[0-7_]+|0b[01_]+)\b/, "number"],
        [/[+-]?\b\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?\b/, "number"],
        [/[\[\]{}.,]/, "delimiter"],
        [/#.*$/, "comment"],
      ],
      doubleString: [
        [/[^\\"#]+/, "string"],
        [/@escapes/, "string.escape"],
        [/\\./, "string.escape.invalid"],
        [/"/, "string", "@pop"],
        [/#.*$/, "comment"],
      ],
      singleString: [
        [/[^']+/, "string"],
        [/'/, "string", "@pop"],
      ],
      multilineDoubleString: [
        [/[^\\"]+/, "string"],
        [/@escapes/, "string.escape"],
        [/\\./, "string.escape.invalid"],
        [/"""/, "string", "@pop"],
        [/"/, "string"],
      ],
      multilineSingleString: [
        [/[^']+/, "string"],
        [/'''/, "string", "@pop"],
        [/'/, "string"],
      ],
    },
  });
}

function monacoLanguageForPath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  const languageByExtension: Record<string, string> = {
    c: "c",
    cc: "cpp",
    cpp: "cpp",
    cs: "csharp",
    css: "css",
    go: "go",
    h: "cpp",
    hpp: "cpp",
    html: "html",
    java: "java",
    js: "javascript",
    json: "json",
    jsx: "javascript",
    kt: "kotlin",
    less: "less",
    lua: "lua",
    markdown: "markdown",
    md: "markdown",
    php: "php",
    py: "python",
    rb: "ruby",
    rs: "rust",
    sass: "scss",
    scss: "scss",
    sh: "shell",
    sql: "sql",
    swift: "swift",
    toml: "toml",
    ts: "typescript",
    tsx: "typescript",
    xml: "xml",
    yaml: "yaml",
    yml: "yaml",
  };

  return languageByExtension[extension] ?? "plaintext";
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

function workspaceHasChatTab(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
) {
  return workspaces.some(
    (workspace) =>
      workspace.id === tab.workspaceId &&
      (isPendingChatId(tab.chatId) ||
        workspace.chats.some((chat) => chat.id === tab.chatId)),
  );
}

function diffFileButtonClass(active: boolean) {
  return `diff-file-button flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-sm ${active
      ? "diff-file-button-active bg-teal-50 text-teal-950 shadow-sm"
      : "text-stone-700 hover:bg-stone-50 hover:text-stone-950"
    }`;
}

function gitFilePathParts(path: string) {
  const separatorIndex = path.lastIndexOf("/");
  if (separatorIndex === -1) {
    return { directory: "", name: path };
  }

  return {
    directory: path.slice(0, separatorIndex),
    name: path.slice(separatorIndex + 1),
  };
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

  if (section === "spec") {
    return t("Spec settings");
  }

  if (section === "agents") {
    return t("Agent settings");
  }

  if (section === "web-search") {
    return t("Web search settings");
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
    return t("System prompt, prompt files, and extra instructions");
  }

  if (section === "spec") {
    return t("Auto Spec model and prompts");
  }

  if (section === "agents") {
    return t("Agent definitions, models, tools, and permissions");
  }

  if (section === "web-search") {
    return t("Search API credentials and runtime web tools");
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
      className={`inline-flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left text-sm font-semibold ${active
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

function emptyProviderRequestOverride(): ProviderRequestOverrideFormState {
  return {
    target: "header",
    name: "",
    valueType: "string",
    value: "",
  };
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
      className={`inline-flex min-h-6 max-w-full items-center rounded-md border px-2 py-0.5 text-xs font-semibold ${ok
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
    inputModalities: ["text"],
    outputModalities: ["text"],
    thinkingLevel: "",
    systemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
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
    autoSyncModels: false,
    modelSyncFilterRegex: "",
    name: "",
    requestOverrides: [],
    serviceId: "",
  };
}

function emptyGeneralForm(): GeneralFormState {
  return {
    apiRequestDetailRetentionDays: "3",
    apiSaveRequestResponseDetails: true,
    autoStartEnabled: false,
    hookAuditEnabled: false,
    language: "en",
    listenHost: "127.0.0.1",
    listenPort: "3210",
    llmRequestRetryCount: "3",
    password: "",
    theme: "light",
  };
}

function emptyWebSearchForm(): WebSearchFormState {
  return {
    activeProvider: "tavily",
    apiProxyEnabled: false,
    apiProxyType: "http",
    apiProxyUrl: "",
    braveApiKey: "",
    clearBraveApiKey: false,
    clearTavilyApiKey: false,
    enabled: false,
    tavilyApiKey: "",
  };
}

function emptyPromptSettingsForm(): PromptSettingsFormState {
  return {
    activeSystemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
    extraText: "",
    files: [],
    pendingFile: "",
    pendingSystemPromptName: "",
    pendingSystemPromptRename: "",
    renamingSystemPromptName: null,
    systemPrompts: [],
  };
}

function emptySpecSettingsForm(): SpecSettingsFormState {
  return {
    autoEnabled: true,
    generationModelId: "",
    generationSystemPrompt: "",
    updateSystemPrompt: "",
  };
}

function normalizedSystemPromptSummaries(
  prompts: PromptSettingsSummary,
): SystemPromptSummary[] {
  const systemPrompts = prompts.systemPrompts?.length
    ? prompts.systemPrompts
    : [
      {
        name: DEFAULT_SYSTEM_PROMPT_NAME,
        content: prompts.systemPrompt ?? prompts.defaultSystemPrompt,
      },
    ];

  if (systemPrompts.some((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)) {
    return systemPrompts;
  }

  return [
    {
      name: DEFAULT_SYSTEM_PROMPT_NAME,
      content: prompts.defaultSystemPrompt,
    },
    ...systemPrompts,
  ];
}

function isSystemPromptFixed(name: string): boolean {
  return (
    name === DEFAULT_SYSTEM_PROMPT_NAME ||
    name === IMAGE_AGENT_SYSTEM_PROMPT_NAME
  );
}

function emptyMemorySettingsForm(): MemorySettingsFormState {
  return {
    enabled: false,
    extractionMode: "manual",
    retrievalMode: "fts",
    extractionModelId: "",
    retrievalModelId: "",
    retentionDays: "",
    dream: {
      enabled: false,
      autoEnabled: false,
      mode: "llm",
      modelId: "",
      workspaceIntervalDays: "7",
      globalIntervalDays: "30",
      createTranscriptChat: true,
      maxFactsPerRun: "200",
      maxChangesPerRun: "50",
      schedulerScanMinutes: "60",
    },
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
    specEnabled: false,
    specInjectEnabled: false,
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  const groups = hookGroupsForEvent(nextConfig, event);
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
  return structuredClone(config);
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

function defaultProviderKind(providerKinds: SettingsResponse["providerKinds"]) {
  return (
    providerKinds.find((kind) => kind.kind === OPENAI_RESPONSES_PROVIDER_KIND)?.kind ??
    providerKinds[0]?.kind ??
    OPENAI_RESPONSES_PROVIDER_KIND
  );
}

function providerServicesForKinds(
  providerKinds: SettingsResponse["providerKinds"],
): ProviderServicePreset[] {
  const supportedKindIds = new Set(providerKinds.map((kind) => kind.kind));

  return PROVIDER_SERVICE_PRESETS.map((service) => ({
    ...service,
    kindIds: service.kindIds.filter((kindId) => supportedKindIds.has(kindId)),
  })).filter((service) => service.kindIds.length > 0);
}

function providerServiceIdForKind(kindId: string) {
  return PROVIDER_SERVICE_PRESETS.find((service) =>
    service.kindIds.includes(kindId),
  )?.id;
}

function providerDefaultKindForService(
  service: ProviderServicePreset,
  providerKinds: SettingsResponse["providerKinds"],
) {
  const supportedKindIds = new Set(providerKinds.map((kind) => kind.kind));

  if (supportedKindIds.has(service.defaultKindId)) {
    return service.defaultKindId;
  }

  return service.kindIds.find((kindId) => supportedKindIds.has(kindId)) ??
    defaultProviderKind(providerKinds);
}

function providerKindDefaultBaseUrl(
  providerKinds: SettingsResponse["providerKinds"],
  kindId: string,
) {
  return providerKinds.find((kind) => kind.kind === kindId)?.defaultBaseUrl ?? "";
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
  return localRandomId("attachment");
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

const MODEL_MODALITY_OPTIONS = ["text", "image", "audio", "video"];

type ModelModalityField = "inputModalities" | "outputModalities";

function modelModalityOptions(
  models: ModelMetadataRecord[],
  field: ModelModalityField,
  selected: string[],
) {
  const values = normalizeModalities([
    ...MODEL_MODALITY_OPTIONS,
    ...models.flatMap((model) => model[field]),
    ...selected,
  ]);

  return values;
}

function normalizeModalities(modalities: string[]) {
  return modalities
    .map((modality) => modality.trim().toLowerCase())
    .filter(Boolean)
    .filter(uniqueString);
}

function defaultModalities(modalities: string[], fallback = ["text"]) {
  const normalized = normalizeModalities(modalities);
  return normalized.length ? normalized : fallback;
}

function uniqueString(value: string, index: number, values: string[]) {
  return values.indexOf(value) === index;
}

function numberInputValue(value: number | null) {
  return value === null || value === 0 ? "" : String(value);
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

function optionalModelLimit(value: string, label: string, required: boolean) {
  const trimmed = value.trim();

  if (!trimmed || (!required && trimmed === "0")) {
    return null;
  }

  return optionalPositiveInteger(value, label);
}

function outputModalitiesRequireLimits(outputModalities: string[]) {
  return outputModalities.length === 0 || outputModalities.includes("text");
}

function requiredPositiveInteger(value: string, label: string) {
  const numberValue = optionalPositiveInteger(value, label);

  if (numberValue === null) {
    throw new Error(`${label} must be a positive whole number`);
  }

  return numberValue;
}

function memoryDreamJobKey(scope: MemoryDreamScope, workspaceId: string | null) {
  return scope === "global" ? "global" : `workspace:${workspaceId ?? ""}`;
}

function memoryDreamScopeLabel(scope: string, t: Translate) {
  return scope === "global" ? t("Global Dream") : t("Workspace Dream");
}

function memoryDreamTriggerLabel(triggerType: string, t: Translate) {
  if (triggerType === "auto_interval") {
    return t("Auto interval");
  }
  if (triggerType === "auto_threshold") {
    return t("Auto threshold");
  }
  return t("Manual");
}

function memoryDreamStatusLabel(status: string, t: Translate) {
  if (status === "completed") {
    return t("Completed");
  }
  if (status === "failed") {
    return t("Failed");
  }
  if (status === "queued") {
    return t("Queued");
  }
  if (status === "running") {
    return t("Running");
  }
  if (status === "cancelled") {
    return t("Cancelled");
  }
  if (status === "skipped") {
    return t("Skipped");
  }
  return status;
}

function memoryDreamChangeOperationLabel(operation: string, t: Translate) {
  const labels: Record<string, string> = {
    add_edge: "Dream change add edge",
    expire: "Dream change expire",
    merge: "Dream change merge",
    promote_to_global: "Dream change promote to global",
    reject: "Dream change reject",
    supersede: "Dream change supersede",
    update: "Dream change update",
  };

  return labels[operation] ? t(labels[operation]) : operation;
}

function memoryDreamChangeStatusLabel(status: string, t: Translate) {
  if (status === "applied") {
    return t("Dream change applied");
  }
  if (status === "failed") {
    return t("Failed");
  }
  return status;
}

function memoryDreamRiskLabel(riskLevel: string, t: Translate) {
  if (riskLevel === "low") {
    return t("Dream risk low");
  }
  if (riskLevel === "medium") {
    return t("Dream risk medium");
  }
  if (riskLevel === "high") {
    return t("Dream risk high");
  }
  return riskLevel;
}

function isActiveMemoryDreamStatus(status: string) {
  return status === "queued" || status === "running";
}

function memoryDreamAppliedChangeCount(job: MemoryDreamJobSummary) {
  return (
    job.changeCounts.added +
    job.changeCounts.updated +
    job.changeCounts.superseded +
    job.changeCounts.expired +
    job.changeCounts.rejected
  );
}

function nextMemoryDreamRunEstimate(
  latestSuccessfulJob: MemoryDreamJobSummary | null,
  dream: MemorySettingsFormState["dream"],
  language: AppLanguageId,
  t: Translate,
) {
  if (!dream.autoEnabled) {
    return t("Auto Dream disabled");
  }

  if (!latestSuccessfulJob) {
    return t("After first eligible scan");
  }

  const completedAt = latestSuccessfulJob.completedAt ?? latestSuccessfulJob.createdAt;
  const completedAtMs = Date.parse(completedAt);
  if (Number.isNaN(completedAtMs)) {
    return t("After next scheduler scan");
  }

  const intervalDays =
    latestSuccessfulJob.scope === "global"
      ? Number(dream.globalIntervalDays)
      : Number(dream.workspaceIntervalDays);
  if (!Number.isFinite(intervalDays) || intervalDays <= 0) {
    return t("After next scheduler scan");
  }

  return formatAuditDate(
    new Date(completedAtMs + intervalDays * 24 * 60 * 60 * 1000).toISOString(),
    language,
  );
}

function groupMemoryDreamChanges(changes: MemoryDreamChangeSummary[]) {
  return changes.reduce<Array<{ operation: string; changes: MemoryDreamChangeSummary[] }>>(
    (groups, change) => {
      const group = groups.find((item) => item.operation === change.operation);
      if (group) {
        group.changes.push(change);
      } else {
        groups.push({ operation: change.operation, changes: [change] });
      }
      return groups;
    },
    [],
  );
}

function memoryDreamJsonText(value: JsonValue | null) {
  return value === null ? "null" : JSON.stringify(value, null, 2);
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
    index === existingIndex
      ? mergeToolCallUpdate(toolCall, normalizedToolCall)
      : toolCall,
  );
}

function mergeToolCallUpdate(
  currentToolCall: ChatToolCallSummary,
  nextToolCall: ChatToolCallSummary,
): ChatToolCallSummary {
  const normalizedToolCall = normalizedToolCallSummary(nextToolCall);
  const keepExistingOutcome =
    currentToolCall.output !== null && normalizedToolCall.output === null;

  return {
    ...normalizedToolCall,
    status: keepExistingOutcome ? currentToolCall.status : normalizedToolCall.status,
    output: keepExistingOutcome ? currentToolCall.output : normalizedToolCall.output,
    isError: keepExistingOutcome ? currentToolCall.isError : normalizedToolCall.isError,
    liveOutput:
      normalizedToolCall.liveOutput ??
      (normalizedToolCall.output === null ? currentToolCall.liveOutput : undefined),
  };
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
        liveOutput: undefined,
      }
      : toolCall,
  );
}

function applyToolOutputDelta(
  toolCalls: ChatToolCallSummary[],
  toolCallId: string,
  stream: "stdout" | "stderr",
  delta: string,
) {
  return toolCalls.map((toolCall) =>
    toolCall.id === toolCallId && toolCall.output === null
      ? {
        ...toolCall,
        liveOutput: appendToolLiveOutput(toolCall.liveOutput, stream, delta),
      }
      : toolCall,
  );
}

function appendToolLiveOutput(
  liveOutput: ChatToolLiveOutput | undefined,
  stream: "stdout" | "stderr",
  delta: string,
): ChatToolLiveOutput {
  return {
    stdout:
      stream === "stdout"
        ? `${liveOutput?.stdout ?? ""}${delta}`
        : liveOutput?.stdout ?? "",
    stderr:
      stream === "stderr"
        ? `${liveOutput?.stderr ?? ""}${delta}`
        : liveOutput?.stderr ?? "",
  };
}

function addChatRunBadge(
  message: ShellMessage,
  badge: ChatRunBadge,
): ShellMessage {
  const runBadges = message.runBadges ?? [];
  if (runBadges.includes(badge)) {
    return message;
  }

  return { ...message, runBadges: [...runBadges, badge] };
}

function contextCompressionBadge(kind: "rule" | "llm"): ChatRunBadge {
  return kind === "llm" ? "contextCompressionLlm" : "contextCompressionRule";
}
function resetStreamingAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "streamReset" }>,
): ShellMessage {
  const toolCalls = streamEvent.toolCalls.map(normalizedToolCallSummary);
  return {
    ...addChatRunBadge(message, "llmReconnect"),
    content: streamEvent.text,
    reasoning: streamEvent.reasoning,
    toolCalls,
    parts: fallbackMessageParts({
      ...message,
      content: streamEvent.text,
      reasoning: streamEvent.reasoning,
      toolCalls,
    }),
  };
}

function completedAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "complete" }>,
  activeReasoningStartedAtMs: number | null,
  completedAtMs: number,
): ShellMessage {
  let parts = message.parts;
  const nextReasoning = streamEvent.reasoning ?? null;
  const reasoningDelta = missingFinalSuffix(message.reasoning ?? "", nextReasoning ?? "");
  if (reasoningDelta) {
    parts = appendReasoningPart(parts, reasoningDelta);
  }
  if (activeReasoningStartedAtMs !== null) {
    parts = finishActiveReasoningPart(
      parts,
      activeReasoningStartedAtMs,
      completedAtMs,
    );
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
  activeReasoningStartedAtMs: number | null,
  completedAtMs: number,
): ShellMessage {
  const parts = activeReasoningStartedAtMs === null
    ? message.parts
    : finishActiveReasoningPart(
      message.parts,
      activeReasoningStartedAtMs,
      completedAtMs,
    );

  return {
    ...message,
    metrics: streamEvent.metrics,
    memoriesUsed: streamEvent.memoriesUsed,
    extractedMemories: message.extractedMemories,
    status: undefined,
    parts: parts.length ? parts : fallbackMessageParts(message),
  };
}

function assistantMessageWithExtractedMemories(
  message: ShellMessage,
  extractedMemories: ChatExtractedMemorySummary[],
): ShellMessage {
  const memoriesById = new Map(
    message.extractedMemories.map((memory) => [memory.id, memory]),
  );
  for (const memory of extractedMemories) {
    memoriesById.set(memory.id, memory);
  }

  return {
    ...message,
    extractedMemories: Array.from(memoriesById.values()),
  };
}

function assistantMessageWithMemoriesUsed(
  message: ShellMessage,
  memoriesUsed: ChatMemoryUsedSummary[],
): ShellMessage {
  return {
    ...message,
    memoriesUsed,
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
        specUpdates: [],
    status: hasVisibleContent ? undefined : "error",
  };
}

function messageHasToolCall(message: ShellMessage, toolCallId: string) {
  return (
    message.role === "assistant" &&
    (message.toolCalls.some((toolCall) => toolCall.id === toolCallId) ||
      message.parts.some(
        (part) => part.type === "toolCall" && part.toolCall.id === toolCallId,
      ))
  );
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
  startedAtMs?: number,
): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "reasoning" || lastPart.durationMs !== undefined) {
    return startedAtMs === undefined
      ? [...parts, { type: "reasoning", text }]
      : [
        ...parts,
        {
          type: "reasoning",
          text,
          startedAtMs,
          liveDurationMs: 0,
        },
      ];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function updateActiveReasoningPartDuration(
  parts: ChatMessagePart[],
  startedAtMs: number,
  nowMs: number,
): ChatMessagePart[] {
  const lastPart = parts[parts.length - 1];
  if (
    lastPart?.type !== "reasoning" ||
    lastPart.startedAtMs !== startedAtMs ||
    lastPart.durationMs !== undefined
  ) {
    return parts;
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      liveDurationMs: Math.max(0, nowMs - startedAtMs),
    },
  ];
}

function finishActiveReasoningPart(
  parts: ChatMessagePart[],
  startedAtMs: number,
  endedAtMs: number,
): ChatMessagePart[] {
  const lastPart = parts[parts.length - 1];
  if (
    lastPart?.type !== "reasoning" ||
    lastPart.startedAtMs !== startedAtMs ||
    lastPart.durationMs !== undefined
  ) {
    return parts;
  }

  return [
    ...parts.slice(0, -1),
    {
      type: "reasoning",
      text: lastPart.text,
      durationMs: Math.max(0, endedAtMs - startedAtMs),
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
    index === existingIndex && part.type === "toolCall"
      ? {
        type: "toolCall",
        toolCall: mergeToolCallUpdate(part.toolCall, normalizedToolCall),
      }
      : part,
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
          liveOutput: undefined,
        },
      } satisfies ChatMessagePart)
      : part,
  );
}

function applyToolOutputDeltaToParts(
  parts: ChatMessagePart[],
  toolCallId: string,
  stream: "stdout" | "stderr",
  delta: string,
): ChatMessagePart[] {
  return parts.map((part) =>
    part.type === "toolCall" &&
      part.toolCall.id === toolCallId &&
      part.toolCall.output === null
      ? ({
        type: "toolCall",
        toolCall: {
          ...part.toolCall,
          liveOutput: appendToolLiveOutput(
            part.toolCall.liveOutput,
            stream,
            delta,
          ),
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

function toolLiveOutputText(liveOutput: ChatToolLiveOutput | undefined) {
  if (!liveOutput) {
    return null;
  }

  const parts: string[] = [];
  if (liveOutput.stdout) {
    parts.push(`[stdout]\n${liveOutput.stdout}`);
  }
  if (liveOutput.stderr) {
    parts.push(`[stderr]\n${liveOutput.stderr}`);
  }

  return parts.length ? parts.join("\n") : null;
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

type ToolCallChangeStats = {
  linesAdded: number;
  linesRemoved: number;
};

function toolCallChangeStats(toolCall: ChatToolCallSummary): ToolCallChangeStats | null {
  if (toolCall.name !== "edit_file" && toolCall.name !== "write_file") {
    return null;
  }
  if (toolCall.output === null || !isObjectRecord(toolCall.output)) {
    return null;
  }

  const linesAdded = numericField(toolCall.output, "linesAdded", "lines_added");
  const linesRemoved = numericField(toolCall.output, "linesRemoved", "lines_removed");
  if (linesAdded === null || linesRemoved === null) {
    return null;
  }

  return { linesAdded, linesRemoved };
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

function numericField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "number" && Number.isFinite(field) ? field : null;
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

function withLiveChatStatistics(
  statistics: ChatStatisticsResponse | null,
  live: LiveChatStatistics,
  messages: ShellMessage[],
  workspaceId: string,
  chatId: string | null,
): ChatStatisticsResponse | null {
  if (!chatId) {
    return statistics;
  }

  const inputTokens = live.usage?.inputTokens ?? 0;
  const outputTokens = live.usage?.outputTokens ?? 0;
  const cacheReadTokens = live.usage?.cacheReadTokens ?? 0;
  const cacheWriteTokens = live.usage?.cacheWriteTokens ?? 0;
  const totalTokens = inputTokens + outputTokens;
  const codeChangeStats = live.codeChangeStats ?? emptyGitDiffLineStats();
  const liveLatencyMs = Math.max(0, Date.now() - live.startedAtMs);
  const base =
    statistics ?? emptyChatStatistics(workspaceId, chatId, emptyAiStatisticsSummary());
  const totalRequests = base.totalRequests + 1;
  const totalLatencyMs = base.totalLatencyMs + liveLatencyMs;
  const messageCount = messages.length || base.messageCount;
  const userMessageCount = countMessagesByRole(messages, "user") || base.userMessageCount;
  const assistantMessageCount =
    countMessagesByRole(messages, "assistant") || base.assistantMessageCount;
  const toolMessageCount = countMessagesByRole(messages, "tool") || base.toolMessageCount;

  return {
    ...base,
    assistantMessageCount,
    averageLatencyMs: Math.round(totalLatencyMs / totalRequests),
    codeChangeStats: {
      additions: base.codeChangeStats.additions + codeChangeStats.additions,
      deletions: base.codeChangeStats.deletions + codeChangeStats.deletions,
    },
    messageCount,
    modelBreakdown: addLiveModelBreakdown(
      base.modelBreakdown,
      live.modelId,
      totalTokens,
    ),
    providerBreakdown: addLiveProviderBreakdown(
      base.providerBreakdown,
      live.providerId,
      totalTokens,
      liveLatencyMs,
    ),
    toolBreakdown: liveToolBreakdown(messages) ?? base.toolBreakdown,
    toolMessageCount,
    totalCacheReadTokens: base.totalCacheReadTokens + cacheReadTokens,
    totalCacheWriteTokens: base.totalCacheWriteTokens + cacheWriteTokens,
    totalInputTokens: base.totalInputTokens + inputTokens,
    totalLatencyMs,
    totalOutputTokens: base.totalOutputTokens + outputTokens,
    totalRequests,
    totalTokens: base.totalTokens + totalTokens,
    userMessageCount,
  };
}

function emptyChatStatistics(
  workspaceId: string,
  chatId: string,
  summary: AiStatisticsSummary,
): ChatStatisticsResponse {
  return {
    workspaceId,
    chatId,
    messageCount: 0,
    userMessageCount: 0,
    assistantMessageCount: 0,
    toolMessageCount: 0,
    totalRequests: summary.totalRequests,
    failedRequests: summary.failedRequests,
    totalInputTokens: summary.totalInputTokens,
    totalOutputTokens: summary.totalOutputTokens,
    totalCacheReadTokens: summary.totalCacheReadTokens,
    totalCacheWriteTokens: summary.totalCacheWriteTokens,
    totalTokens: summary.totalTokens,
    totalLatencyMs: 0,
    averageLatencyMs: summary.averageLatencyMs,
    memoryReferences: 0,
    createdMemories: 0,
    codeChangeStats: { additions: 0, deletions: 0 },
    modelBreakdown: summary.modelBreakdown,
    providerBreakdown: summary.providerBreakdown,
    toolBreakdown: [],
    compression: {
      snapshotCount: 0,
      ruleSnapshotCount: 0,
      llmSnapshotCount: 0,
      originalTokenCount: 0,
      summaryTokenCount: 0,
      savedTokenCount: 0,
    },
  };
}

function emptyGitDiffLineStats(): GitDiffLineStats {
  return { additions: 0, deletions: 0 };
}

function countMessagesByRole(messages: ShellMessage[], role: string) {
  return messages.filter((message) => message.role === role).length;
}

function addLiveModelBreakdown(
  breakdown: AiStatisticsModelBreakdown[],
  modelId: string,
  totalTokens: number,
) {
  return sortedModelBreakdown(
    upsertBreakdown(
      breakdown,
      modelId,
      (item) => item.modelId,
      (item) => ({
        ...item,
        requestCount: item.requestCount + 1,
        totalTokens: item.totalTokens + totalTokens,
      }),
      (id) => ({ modelId: id, requestCount: 1, totalTokens }),
    ),
  );
}

function addLiveProviderBreakdown(
  breakdown: AiStatisticsProviderBreakdown[],
  providerId: string,
  totalTokens: number,
  latencyMs: number,
) {
  return sortedProviderBreakdown(
    upsertBreakdown(
      breakdown,
      providerId,
      (item) => item.providerId,
      (item) => {
        const requestCount = item.requestCount + 1;
        const successCount = item.successCount + 1;
        const previousLatencyTotal =
          item.averageLatencyMs === null
            ? 0
            : item.averageLatencyMs * item.requestCount;

        return {
          ...item,
          averageLatencyMs: Math.round(
            (previousLatencyTotal + latencyMs) / requestCount,
          ),
          requestCount,
          successCount,
          successRate: successCount / requestCount,
          totalTokens: item.totalTokens + totalTokens,
        };
      },
      (id) => ({
        averageLatencyMs: latencyMs,
        failedCount: 0,
        providerId: id,
        requestCount: 1,
        successCount: 1,
        successRate: 1,
        totalTokens,
      }),
    ),
  );
}

function upsertBreakdown<T>(
  breakdown: T[],
  id: string,
  getId: (item: T) => string,
  update: (item: T) => T,
  create: (id: string) => T,
) {
  if (!id) {
    return breakdown;
  }

  let found = false;
  const next = breakdown.map((item) => {
    if (getId(item) !== id) {
      return item;
    }

    found = true;
    return update(item);
  });

  return found ? next : [...next, create(id)];
}

function sortedModelBreakdown(breakdown: AiStatisticsModelBreakdown[]) {
  return [...breakdown].sort(
    (left, right) =>
      right.totalTokens - left.totalTokens ||
      right.requestCount - left.requestCount ||
      left.modelId.localeCompare(right.modelId),
  );
}

function sortedProviderBreakdown(breakdown: AiStatisticsProviderBreakdown[]) {
  return [...breakdown].sort(
    (left, right) =>
      right.totalTokens - left.totalTokens ||
      right.requestCount - left.requestCount ||
      left.providerId.localeCompare(right.providerId),
  );
}

function liveToolBreakdown(messages: ShellMessage[]) {
  const counts = new Map<string, number>();
  for (const message of messages) {
    for (const toolCall of message.toolCalls) {
      counts.set(toolCall.name, (counts.get(toolCall.name) ?? 0) + 1);
    }
  }

  if (counts.size === 0) {
    return null;
  }

  return [...counts]
    .map(([toolName, callCount]) => ({ toolName, callCount }))
    .sort(
      (left, right) =>
        right.callCount - left.callCount || left.toolName.localeCompare(right.toolName),
    );
}

function preferredOverviewWorkspaceId(
  activeWorkspaceId: string,
  workspaces: WorkspaceSummary[],
) {
  if (
    activeWorkspaceId &&
    workspaces.some((workspace) => workspace.id === activeWorkspaceId)
  ) {
    return activeWorkspaceId;
  }

  return workspaces[0]?.id ?? "";
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

  if (status === "running") {
    return t("running");
  }

  if (status === "cancelled") {
    return t("cancelled");
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

  if (status === "cancelled") {
    return `${base} bg-amber-100 text-amber-800`;
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

function workspaceSpecJobStatusLabel(status: string) {
  switch (status) {
    case "queued":
      return "Queued";
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "skipped":
      return "Skipped";
    case "failed":
      return "Failed";
    default:
      return status;
  }
}

function workspaceSpecTriggerLabel(triggerType: string) {
  switch (triggerType) {
    case "manual_initial":
      return "Manual initial";
    case "manual_refresh":
      return "Manual refresh";
    case "chat_completed":
      return "Chat completed";
    default:
      return triggerType;
  }
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

function formatLatencySeconds(value: number, language: AppLanguageId = "en") {
  return `${new Intl.NumberFormat(language, {
    maximumFractionDigits: 0,
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

function contextSourceLabel(source: string, t: Translate) {
  const labels: Record<string, string> = {
    assistantDraft: t("Assistant draft"),
    compressionSnapshot: t("Compression"),
    currentUser: t("Current user"),
    guidance: t("Guidance"),
    hookContext: t("Hook context"),
    persistedHistory: t("History"),
    reservedPrompt: t("Prompt"),
    runtimeAssistant: t("Runtime assistant"),
    runtimeToolState: t("Runtime tools"),
    runtimeToolStateSnapshot: t("Tool snapshot"),
    stableInjection: t("Stable context"),
    todoGraph: t("ToDo"),
    turnMemory: t("Memory"),
  };

  return labels[source] ?? source;
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
    second: "2-digit",
    year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(date);
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

function isPendingChatId(chatId: string) {
  return chatId.startsWith("scheduled-chat-") || chatId.startsWith("pending:");
}

function chatKeyWorkspaceId(chatKey: string) {
  const separatorIndex = chatKey.indexOf(":");
  return separatorIndex > 0 ? chatKey.slice(0, separatorIndex) : null;
}

function chatTitleForDraft(
  content: string,
  attachments: ChatAttachmentPayload[],
) {
  const normalized = content.trim().replace(/\s+/g, " ");
  if (normalized) {
    return normalized.length > 48 ? `${normalized.slice(0, 48)}...` : normalized;
  }

  return attachments.length === 1
    ? attachments[0].name
    : `${attachments.length} attachments`;
}

function localUiId(prefix: string) {
  return `${prefix}-${localRandomId()}`;
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

function gitStatusBadgeClass(label: string) {
  const status = label[0] ?? ".";
  const colorClass =
    status === "M"
      ? "bg-amber-100 text-amber-700 border-amber-200"
      : status === "U" || status === "A"
        ? "bg-emerald-100 text-emerald-700 border-emerald-200"
        : status === "D"
          ? "bg-rose-100 text-rose-700 border-rose-200"
          : status === "R"
            ? "bg-sky-100 text-sky-700 border-sky-200"
            : "bg-stone-100 text-stone-600 border-stone-200";

  return `shrink-0 rounded border px-1.5 py-0.5 font-mono text-[11px] font-semibold leading-none ${colorClass}`;
}

function normalizeGitStatus(status: string) {
  const trimmed = status.trim();
  if (!trimmed) {
    return "";
  }

  return trimmed === "?" ? "U" : trimmed;
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
  let shouldStopReading = false;
  const handleEvent = (event: ChatStreamEvent) => {
    onEvent(event);
    if (event.type === "streamEnd") {
      shouldStopReading = true;
    }
  };

  while (!shouldStopReading) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    buffer += decoder.decode(value, { stream: true });
    buffer = readSseFrames(buffer, handleEvent);
  }

  if (shouldStopReading) {
    await reader.cancel();
    return;
  }

  buffer += decoder.decode();
  readSseFrames(`${buffer}\n\n`, handleEvent);
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

  if (
    value.type === "streamAttemptStart" ||
    value.type === "stream_attempt_start"
  ) {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const llmRequestId = stringField(value, "llmRequestId", "llm_request_id");

    if (!assistantMessageId || !llmRequestId) {
      return null;
    }

    return { type: "streamAttemptStart", assistantMessageId, llmRequestId };
  }

  if (value.type === "streamReset" || value.type === "stream_reset") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const reason = stringField(value, "reason");
    const text = stringField(value, "text");
    const reasoning = optionalNullableStringField(value, "reasoning");
    const toolCallsValue = fieldValue(value, "toolCalls", "tool_calls");

    if (
      !assistantMessageId ||
      !reason ||
      text === null ||
      reasoning === false ||
      !Array.isArray(toolCallsValue)
    ) {
      return null;
    }

    const toolCalls = toolCallsValue.map(parseChatToolCallSummary);
    if (toolCalls.some((toolCall) => toolCall === null)) {
      return null;
    }

    return {
      type: "streamReset",
      assistantMessageId,
      reason,
      text,
      reasoning: reasoning ?? null,
      toolCalls: toolCalls as ChatToolCallSummary[],
    };
  }

  if (
    value.type === "contextCompression" ||
    value.type === "context_compression"
  ) {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const snapshotId = stringField(value, "snapshotId", "snapshot_id");
    const kindValue = stringField(value, "kind") ?? "rule";
    const kind = kindValue === "llm" ? "llm" : "rule";

    if (!assistantMessageId || !snapshotId) {
      return null;
    }

    return { type: "contextCompression", assistantMessageId, snapshotId, kind };
  }
  if (value.type === "toolOutputDelta" || value.type === "tool_output_delta") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const toolCallId = stringField(value, "toolCallId", "tool_call_id");
    const stream = stringField(value, "stream");
    const delta = stringField(value, "delta");

    if (
      !assistantMessageId ||
      !toolCallId ||
      (stream !== "stdout" && stream !== "stderr") ||
      delta === null
    ) {
      return null;
    }

    return {
      type: "toolOutputDelta",
      assistantMessageId,
      toolCallId,
      stream,
      delta,
    };
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
    const codeChangeStatsValue = fieldValue(
      value,
      "codeChangeStats",
      "code_change_stats",
    );
    const codeChangeStats =
      typeof codeChangeStatsValue === "undefined"
        ? emptyGitDiffLineStats()
        : parseGitDiffLineStats(codeChangeStatsValue);

    if (!workspaceId || !codeChangeStats) {
      return null;
    }

    return { type: "gitDiffRefresh", workspaceId, codeChangeStats };
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

  if (
    value.type === "agentTeamRefresh" ||
    value.type === "agent_team_refresh"
  ) {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");
    const chatId = stringField(value, "chatId", "chat_id");
    const teamId = stringField(value, "teamId", "team_id");
    const instanceId = optionalStringField(value, "instanceId", "instance_id");
    const reason = stringField(value, "reason");
    const revealPanel = fieldValue(value, "revealPanel", "reveal_panel");

    if (
      !workspaceId ||
      !chatId ||
      !teamId ||
      instanceId === null ||
      !reason ||
      typeof revealPanel !== "boolean"
    ) {
      return null;
    }

    return {
      type: "agentTeamRefresh",
      workspaceId,
      chatId,
      teamId,
      instanceId,
      reason,
      revealPanel,
    };
  }

  if (
    value.type === "memoryExtractionComplete" ||
    value.type === "memory_extraction_complete"
  ) {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const extractedMemories = parseChatExtractedMemories(
      fieldValue(value, "extractedMemories", "extracted_memories"),
    );

    if (!assistantMessageId || extractedMemories === false) {
      return null;
    }

    return {
      type: "memoryExtractionComplete",
      assistantMessageId,
      extractedMemories,
    };
  }

  if (value.type === "memoryResolved" || value.type === "memory_resolved") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const memoriesUsed = parseChatMemoriesUsed(
      fieldValue(value, "memoriesUsed", "memories_used"),
    );

    if (!assistantMessageId || memoriesUsed === false) {
      return null;
    }

    return {
      type: "memoryResolved",
      assistantMessageId,
      memoriesUsed,
    };
  }

  if (value.type === "streamEnd" || value.type === "stream_end") {
    return { type: "streamEnd" };
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
function parseChatSpecUpdates(value: unknown): ChatSpecUpdateSummary[] | false {
  if (typeof value === "undefined" || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    return false;
  }

  const updates = value.map(parseChatSpecUpdateSummary);
  return updates.some((update) => update === null)
    ? false
    : (updates as ChatSpecUpdateSummary[]);
}

function parseChatSpecUpdateSummary(value: unknown): ChatSpecUpdateSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const jobId = stringField(value, "jobId", "job_id");
  const baseRevision = fieldValue(value, "baseRevision", "base_revision");
  const revision = fieldValue(value, "revision");
  const completedAt = stringField(value, "completedAt", "completed_at");
  const linesValue = fieldValue(value, "lines");
  const truncated = fieldValue(value, "truncated");

  if (
    !id ||
    !jobId ||
    typeof baseRevision !== "number" ||
    typeof revision !== "number" ||
    !completedAt ||
    !Array.isArray(linesValue) ||
    typeof truncated !== "boolean"
  ) {
    return null;
  }

  const lines = linesValue.map(parseChatSpecUpdateDiffLine);
  if (lines.some((line) => line === null)) {
    return null;
  }

  return {
    baseRevision,
    completedAt,
    id,
    jobId,
    lines: lines as ChatSpecUpdateSummary["lines"],
    revision,
    truncated,
  };
}

function parseChatSpecUpdateDiffLine(
  value: unknown,
): ChatSpecUpdateSummary["lines"][number] | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const kind = stringField(value, "kind");
  const text = stringField(value, "text");
  if ((kind !== "added" && kind !== "removed") || text === null) {
    return null;
  }
  return { kind, text };
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
        specUpdates: [],
    runBadges: [],
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
  const acceptingGuidanceValue = fieldValue(
    value,
    "acceptingGuidance",
    "accepting_guidance",
  );

  if (!runId || !workspaceId || !chatId) {
    return null;
  }

  return {
    runId,
    workspaceId,
    chatId,
    lastSequence:
      typeof lastSequenceValue === "number" ? lastSequenceValue : null,
    acceptingGuidance: acceptingGuidanceValue === true,
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
  const specUpdates = parseChatSpecUpdates(
    fieldValue(message, "specUpdates", "spec_updates"),
  );
  if (specUpdates === false) {
    throw new Error("chat message specUpdates are invalid");
  }

  const toolCalls = Array.isArray(message.toolCalls)
    ? message.toolCalls.map(normalizedToolCallSummary)
    : [];
  const partsSource = Array.isArray(message.parts) ? message.parts : [];
  const parts = partsSource
    .map((part) => normalizeChatMessagePart(part))
    .filter((part): part is ChatMessagePart => part !== null);
  const pendingMode =
    message.pendingMode === "queued" || message.pendingMode === "guidance"
      ? message.pendingMode
      : undefined;
  const queuedRun = normalizeQueuedMessageRunSummary(message.queuedRun);
  const normalizedMessage = {
    ...message,
    extractedMemories,
    metrics,
    memoriesUsed,
    pendingMode,
    queuedRun,
    runBadges: [],
    specUpdates,
    toolCalls,
    parts,
  };

  return {
    ...normalizedMessage,
    parts: parts.length ? parts : fallbackMessageParts(normalizedMessage),
  };
}

function normalizeQueuedMessageRunSummary(
  queuedRun: QueuedMessageRunSummary | null | undefined,
): QueuedMessageRunSummary | null {
  if (!queuedRun || typeof queuedRun !== "object") {
    return null;
  }
  const modelId = fieldValue(queuedRun, "modelId", "model_id");
  if (typeof modelId !== "string" || !modelId.trim()) {
    return null;
  }
  const providerId = fieldValue(queuedRun, "providerId", "provider_id");
  const thinkingLevel = fieldValue(queuedRun, "thinkingLevel", "thinking_level");
  const skillIds = fieldValue(queuedRun, "skillIds", "skill_ids");
  const assistantMessageId = fieldValue(
    queuedRun,
    "assistantMessageId",
    "assistant_message_id",
  );
  const assistantSequence = fieldValue(
    queuedRun,
    "assistantSequence",
    "assistant_sequence",
  );
  const status = fieldValue(queuedRun, "status");

  return {
    status: typeof status === "string" ? status : "queued",
    modelId,
    providerId: typeof providerId === "string" ? providerId : null,
    thinkingLevel: typeof thinkingLevel === "string" ? thinkingLevel : null,
    skillIds: Array.isArray(skillIds)
      ? skillIds.filter((skillId): skillId is string => typeof skillId === "string")
      : [],
    assistantMessageId:
      typeof assistantMessageId === "string" ? assistantMessageId : null,
    assistantSequence:
      typeof assistantSequence === "number" ? assistantSequence : null,
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
    if (typeof text !== "string") {
      return null;
    }
    const durationMs = fieldValue(part, "durationMs", "duration_ms");
    const liveDurationMs = fieldValue(part, "liveDurationMs", "live_duration_ms");
    const startedAtMs = fieldValue(part, "startedAtMs", "started_at_ms");
    return {
      type: "reasoning",
      text,
      ...(typeof durationMs === "number" ? { durationMs } : {}),
      ...(typeof liveDurationMs === "number" ? { liveDurationMs } : {}),
      ...(typeof startedAtMs === "number" ? { startedAtMs } : {}),
    };
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



function nativePickerRequestInit(nativeBrowserToken: string): RequestInit {
  return {
    body: JSON.stringify({ nativeBrowserToken }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  };
}

function localRandomId(fallbackPrefix?: string) {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (randomUUID) {
    return randomUUID.call(globalThis.crypto);
  }

  // ponytail: fallback is for local UI ids only; use requiredRandomUuid for tokens.
  const suffix = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return fallbackPrefix ? `${fallbackPrefix}-${suffix}` : suffix;
}

function requiredRandomUuid(label: string) {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) {
    throw new Error(`${label} requires crypto.randomUUID`);
  }

  return randomUUID.call(globalThis.crypto);
}

function createNativeBrowserToken() {
  return requiredRandomUuid("native browser token").replace(/[^A-Za-z0-9_-]/g, "-");
}

function probeNativeBrowser(port: number, token: string): Promise<boolean> {
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    return Promise.resolve(false);
  }

  const query = `token=${encodeURIComponent(token)}&t=${Date.now()}`;
  const urls = [
    `http://127.0.0.1:${port}/api/native/browser-probe.svg?${query}`,
    `http://localhost:${port}/api/native/browser-probe.svg?${query}`,
    `http://[::1]:${port}/api/native/browser-probe.svg?${query}`,
  ];

  return new Promise((resolve) => {
    let pending = urls.length;
    let resolved = false;

    const finish = (available: boolean) => {
      if (resolved) {
        return;
      }
      if (available) {
        resolved = true;
        resolve(true);
        return;
      }

      pending -= 1;
      if (pending === 0) {
        resolved = true;
        resolve(false);
      }
    };

    for (const url of urls) {
      void loadNativeProbeImage(url).then(finish);
    }
  });
}

function loadNativeProbeImage(url: string): Promise<boolean> {
  return new Promise((resolve) => {
    const image = new Image();
    let settled = false;
    const timeoutId = window.setTimeout(() => finish(false), 1500);

    function finish(available: boolean) {
      if (settled) {
        return;
      }

      settled = true;
      window.clearTimeout(timeoutId);
      image.onload = null;
      image.onerror = null;
      if (!available) {
        image.src = "";
      }
      resolve(available);
    }

    image.onload = () => finish(true);
    image.onerror = () => finish(false);
    image.src = url;
  });
}
