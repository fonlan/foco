import focoLogoSvg from "../foco.svg?raw";
import {
  Activity,
  BarChart3,
  Bot,
  CalendarClock,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Copy,
  Download,
  FileText,
  FolderPlus,
  Home,
  Lock,
  LogOut,
  LoaderCircle,
  MessageSquare,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Settings,
  SlidersHorizontal,
  SquareTerminal,
  SunMoon,
  Trash2,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  CSSProperties,
  ChangeEvent as ReactChangeEvent,
  DragEvent as ReactDragEvent,
  FormEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  WheelEvent as ReactWheelEvent,
  Suspense,
  lazy,
  memo,
  startTransition,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type {
  ActiveChatRunSummary,
  ActiveRunInfo,
  AiStatisticsModelBreakdown,
  AiStatisticsProviderBreakdown,
  AiStatisticsResponse,
  AiStatisticsSummary,
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
  ChatToolCallSummary,
  ChatToolLiveOutput,
  ChatTabSummary,
  ChatUsage,
  ComposerAttachment,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  ContextMemoryState,
  ContextUsageRefreshRequest,
  ContextUsageResponse,
  GenerateWorkspaceSpecResponse,
  GitBranchesResponse,
  GitCommitMessageResponse,
  GitDiffLineStats,
  GitDiffResponse,
  GitStatusFileSummary,
  HookNotificationSummary,
  InstallRipgrepResponse,
  JsonValue,
  LiveChatStatistics,
  MemoryFactRecord,
  MemoryListResponse,
  MemoryMutationResponse,
  OpenChatTab,
  Plan,
  PlanResponse,
  PlansResponse,
  PendingDeleteChat,
  QueueChatMessageResponse,
  QueuedMessageRunSummary,
  QuestionAnswerSubmission,
  QuestionItemSummary,
  QuestionOptionSummary,
  QuestionRequestSummary,
  RetryRunRequest,
  ScheduledWorkspaceRun,
  SettingsResponse,
  SettingsSection,
  ShellMessage,
  TaskStatus,
  TodoGraphResponse,
  TodoGraphTask,
  Translate,
  WorkspaceChatListItem,
  WorkspaceFileChildrenResponse,
  WorkspaceFileContentResponse,
  WorkspaceFileSaveResponse,
  WorkspaceFilesResponse,
  WorkspaceFileTreeNode,
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
  type GitDiffSection,
} from "./features/git/diff-parser";
import {
  PLAN_AUTO_RUN_ENABLED_STORAGE_KEY,
  chartColor,
  CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT,
  CONTEXT_PANEL_DEFAULT_WIDTH,
  CONTEXT_PANEL_MAX_HEIGHT_RATIO,
  CONTEXT_PANEL_MAX_WIDTH,
  CONTEXT_PANEL_MIN_HEIGHT,
  CONTEXT_PANEL_MIN_WIDTH,
  CREATE_BRANCH_OPTION_VALUE,
  MAX_CHAT_ATTACHMENTS,
  MAX_CHAT_ATTACHMENT_BYTES,
  MAX_CHAT_ATTACHMENT_TOTAL_BYTES,
  MOBILE_BREAKPOINT_PX,
  WORKSPACE_CHAT_CONTEXT_MENU_LONG_PRESS_MS,
  WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
  WORKSPACE_SIDEBAR_MAX_WIDTH,
  WORKSPACE_SIDEBAR_MIN_WIDTH,
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
const TerminalPanel = lazy(() =>
  import("./features/terminal/TerminalPanel").then((m) => ({
    default: m.TerminalPanel,
  })),
);
const ApiStatsPanel = lazy(() =>
  import("./features/stats/ApiStatsPanel").then((m) => ({
    default: m.ApiStatsPanel,
  })),
);
import { WorkspaceIcon } from "./features/workspaces/WorkspaceIcon";
import {
  chatItemClass,
  moveItemId,
  reorderWorkspacesByIds,
  sameStringList,
  workspaceItemClass,
  workspaceMenuClass,
  workspaceNameFromPath,
} from "./features/workspaces/workspace-helpers";
import { WorkspaceDialog } from "./features/workspaces/WorkspaceDialog";
import { GitBranchDialog } from "./features/git/GitBranchDialog";
import { DeleteChatDialog } from "./features/chat/DeleteChatDialog";
import { ChatPanel, type ChatPanelHelpers } from "./features/chat/ChatPanel";
import {
  activeSkillQuery,
  chatAttachmentPayload,
  fileToBase64,
  fileToComposerAttachment,
  formatFileSize,
  isSkillAvailableForWorkspace,
  messageWithSelectedSkills,
  removeActiveSkillToken,
  selectedSkillPrefix,
  skillScopeLabel,
  unsupportedAttachmentInputModality,
  unsupportedAttachmentMessage,
  unsupportedFileAttachmentMessage,
  userMessageParts,
} from "./features/chat/chat-helpers";
import {
  isWorkspaceImageFilePath,
  preloadOptionalMonaco,
  scheduleOptionalMonacoPreload,
  WorkspaceFileEditorPanel,
  type OpenFileTab,
  type WorkspaceFileEditorState,
} from "./features/files/WorkspaceFileEditorPanel";
import { AgentsRuntimePanel } from "./features/agents/AgentsRuntimePanel";
import {
  ContextPanelSidebar,
  ResponsiveContextPanelIcon,
  type ContextPanelTab,
} from "./features/context/ContextPanel";
import { AgentTranscriptPanel } from "./features/agents/AgentTranscriptPanel";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { errorMessage, requestJson, responseErrorMessage } from "./shared/api-client";
const ScheduledTasksPage = lazy(() =>
  import("./features/scheduled-tasks/ScheduledTasksPage").then((m) => ({
    default: m.ScheduledTasksPage,
  })),
);

const PLAN_PHASE_RETRY_REFRESH_INTERVAL_MS = 3000;
const PLAN_AUTO_RUN_REFRESH_MS = 3000;

type ViewMode = BrowserRoute["viewMode"];
type PlanAutoRunnableAction = "start" | "resume";

type PendingPlanPhaseRetryRefresh = {
  workspaceId: string;
  planId: string;
  phaseId: string;
  agentTaskId: string;
};

function isAutoRunPlanInFlight(plan: Plan) {
  return (
    plan.status === "running" ||
    plan.phases.some(
      (phase) => phase.status === "queued" || phase.status === "running",
    )
  );
}

export function nextAutoRunnablePlan(
  plans: Plan[],
): { planId: string; action: PlanAutoRunnableAction } | null {
  for (const plan of plans) {
    if (
      plan.status === "ready" ||
      plan.status === "draft" ||
      plan.status === "failed"
    ) {
      return { planId: plan.id, action: "start" };
    }

    if (plan.status === "paused") {
      return { planId: plan.id, action: "resume" };
    }
  }

  return null;
}


type WorkspaceChatContextMenuState = {
  chat: WorkspaceChatListItem;
  left: number;
  top: number;
  workspace: WorkspaceSummary;
};

type ChatMessagesPaginationState = {
  hasMoreBefore: boolean;
  nextBeforeSequence: number | null;
};

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

type MainTabCloseScope = "current" | "others" | "all" | "right" | "left";

type MainTabContextMenuState = {
  left: number;
  positioned: boolean;
  tab: MainTabSummary;
  top: number;
};

type WorkspaceFileContextMenuState = {
  left: number;
  node: WorkspaceFileTreeNode;
  top: number;
  workspacePath: string;
};

const LIVE_REASONING_DURATION_REFRESH_MS = 250;
const AGENT_TEAM_RUNNING_REFRESH_MS = 1000;
const CHAT_MESSAGES_PAGE_LIMIT = 100;
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-default";
const EMPTY_CONFIGURED_PROVIDERS: ConfiguredProviderSummary[] = [];
const EMPTY_GIT_STATUS_FILES: GitStatusFileSummary[] = [];

function deferStreamSideUpdate(update: () => void) {
  // ponytail: transition is enough for sparse side events; add a real queue only
  // if profiler shows usage/tool/context storms.
  startTransition(update);
}

type ComposerDefaultSelection = {
  modelId: string;
  providerId: string;
  thinkingLevel: string;
};

function useStableCallback<T extends (...args: any[]) => unknown>(callback: T): T {
  const callbackRef = useRef(callback);

  useLayoutEffect(() => {
    callbackRef.current = callback;
  });

  return useCallback(
    ((...args: Parameters<T>) => callbackRef.current(...args)) as T,
    [],
  );
}

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
  const [draggedWorkspaceId, setDraggedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceOrderPreview, setWorkspaceOrderPreview] = useState<
    string[] | null
  >(null);
  const [workspaceChatVisibleCounts, setWorkspaceChatVisibleCounts] = useState<
    Record<string, number>
  >({});
  const [workspaceChatSearchOpen, setWorkspaceChatSearchOpen] = useState(false);
  const [workspaceChatSearchQuery, setWorkspaceChatSearchQuery] = useState("");
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
  const [loadingOlderChatMessageKeys, setLoadingOlderChatMessageKeys] = useState<Set<string>>(
    () => new Set(),
  );
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
  const [, setChatMessagesByKeyState] = useState<
    Record<string, ShellMessage[]>
  >({});
  const chatMessagesByKeyRef = useRef<Record<string, ShellMessage[]>>({});
  function setChatMessagesByKey(
    updater:
      | Record<string, ShellMessage[]>
      | ((
        current: Record<string, ShellMessage[]>,
      ) => Record<string, ShellMessage[]>),
  ) {
    const next =
      typeof updater === "function"
        ? updater(chatMessagesByKeyRef.current)
        : updater;
    chatMessagesByKeyRef.current = next;
    setChatMessagesByKeyState(next);
  }
  const [, setChatMessagePaginationByKeyState] = useState<
    Record<string, ChatMessagesPaginationState>
  >({});
  const chatMessagePaginationByKeyRef = useRef<
    Record<string, ChatMessagesPaginationState>
  >({});
  function setChatMessagePaginationByKey(
    updater:
      | Record<string, ChatMessagesPaginationState>
      | ((
        current: Record<string, ChatMessagesPaginationState>,
      ) => Record<string, ChatMessagesPaginationState>),
  ) {
    const next =
      typeof updater === "function"
        ? updater(chatMessagePaginationByKeyRef.current)
        : updater;
    chatMessagePaginationByKeyRef.current = next;
    setChatMessagePaginationByKeyState(next);
  }
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [agentDefinitions, setAgentDefinitions] = useState<AgentDefinitionSettings[]>([]);
  const [defaultAgentRolePrompts, setDefaultAgentRolePrompts] = useState<Record<string, string>>({});
  const [isTeamModeEnabled, setIsTeamModeEnabled] = useState(false);
  const [isPlanModeEnabled, setIsPlanModeEnabled] = useState(false);
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
  const [activePlans, setActivePlans] = useState<Plan[]>([]);
  const [isLoadingActivePlans, setIsLoadingActivePlans] = useState(false);
  const [activePlansError, setActivePlansError] = useState<string | null>(null);
  const [planOperationKey, setPlanOperationKey] = useState<string | null>(null);
  const [isPlanAutoRunEnabled, setIsPlanAutoRunEnabled] = useState(
    () => window.localStorage.getItem(PLAN_AUTO_RUN_ENABLED_STORAGE_KEY) === "true",
  );
  const [isPlanAutoRunDispatching, setIsPlanAutoRunDispatching] =
    useState(false);
  const [pendingPlanPhaseRetryRefresh, setPendingPlanPhaseRetryRefresh] =
    useState<PendingPlanPhaseRetryRefresh | null>(null);
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
  const loadingOlderChatMessageKeysRef = useRef<Set<string>>(new Set());
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
  const workspaceOrderPreviewRef = useRef<string[] | null>(null);
  const workspaceOrderDropHandledRef = useRef(false);
  const displayedWorkspaces = useMemo(
    () =>
      workspaceOrderPreview
        ? reorderWorkspacesByIds(workspaces, workspaceOrderPreview)
        : workspaces,
    [workspaceOrderPreview, workspaces],
  );
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
  const activeChatPagination =
    activeChatKey !== null
      ? chatMessagePaginationByKeyRef.current[activeChatKey] ?? null
      : null;
  const isLoadingOlderActiveChatMessages =
    activeChatKey !== null && loadingOlderChatMessageKeys.has(activeChatKey);
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
  // ponytail: stats only tracks message shape here; add text hashes if live stats need per-token updates.
  const chatStatisticsMessageFingerprint = useMemo(
    () =>
      messages
        .map(
          (message) =>
            `${message.id}:${message.role}:${message.status}:${message.toolCalls.length}:${message.parts.length}`,
        )
        .join("|"),
    [messages],
  );
  const displayedChatStatistics = useMemo(
    () =>
      liveChatStatistics
        ? withLiveChatStatistics(
          chatStatistics,
          liveChatStatistics,
          messages,
          activeWorkspaceId,
          activeChatId,
        )
        : chatStatistics,
    [
      activeChatId,
      activeWorkspaceId,
      chatStatistics,
      chatStatisticsMessageFingerprint,
      liveChatStatistics,
    ],
  );
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
  const selectedModel = useMemo(
    () => availableModels.find((model) => model.id === selectedModelId) ?? null,
    [availableModels, selectedModelId],
  );
  const unsupportedDraftAttachment = useMemo(
    () =>
      draftAttachments.find((attachment) =>
        unsupportedAttachmentInputModality(selectedModel, attachment.contentType),
      ) ?? null,
    [draftAttachments, selectedModel],
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
  const availableSkills = useMemo(
    () => detectedSkills.filter((skill) => isSkillAvailableForWorkspace(skill, activeWorkspace?.id ?? null)),
    [activeWorkspace?.id, detectedSkills],
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
  const unsupportedDraftAttachmentMessage = unsupportedDraftAttachment
    ? unsupportedAttachmentMessage(selectedModel, unsupportedDraftAttachment, t)
    : null;

  useEffect(() => {
    if (!canUseApp) {
      return undefined;
    }

    return scheduleOptionalMonacoPreload();
  }, [canUseApp]);

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
      setDefaultAgentRolePrompts(data.defaultRolePrompts ?? {});
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
      setDefaultAgentRolePrompts(data.defaultRolePrompts ?? {});
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
      setDefaultAgentRolePrompts(data.defaultRolePrompts ?? {});
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
      setDefaultAgentRolePrompts(data.defaultRolePrompts ?? {});
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
      setWorkspaceSpecPreviewEnabled(data.contentMarkdown.trim().length > 0);
      return data;
    } catch (requestError) {
      setWorkspaceSpec(null);
      setWorkspaceSpecDraft("");
      setWorkspaceSpecPreviewEnabled(false);
      setWorkspaceSpecError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingWorkspaceSpec(false);
    }
  }, []);

  const loadActivePlans = useCallback(async (workspaceId: string) => {
    setIsLoadingActivePlans(true);
    setActivePlansError(null);

    try {
      const data = await requestJson<PlansResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/plans?view=active&limit=50`,
      );
      if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
        return null;
      }
      setActivePlans(data.plans);
      return data;
    } catch (requestError) {
      setActivePlans([]);
      setActivePlansError(errorMessage(requestError));
      return null;
    } finally {
      setIsLoadingActivePlans(false);
    }
  }, []);

  const handlePlanRefresh = useCallback(
    (event: Extract<ChatStreamEvent, { type: "planRefresh" }>) => {
      if (activeWorkspaceIdRef.current !== event.workspaceId) {
        return;
      }

      setContextPanelTab("plan");
      setIsContextPanelOpen(true);
      void loadActivePlans(event.workspaceId);
    },
    [loadActivePlans],
  );

  const runPlanAction = useCallback(
    async (workspaceId: string, planId: string, action: string) => {
      const operationKey = `${action}:${planId}`;
      setPlanOperationKey(operationKey);
      setActivePlansError(null);

      try {
        const response = await requestJson<PlanResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/${encodeURIComponent(planId)}/action`,
          {
            body: JSON.stringify({ action }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        const plansResponse = await loadActivePlans(workspaceId);
        await refreshWorkspaces();
        const plan =
          plansResponse?.plans.find((candidate) => candidate.id === planId) ??
          response.plan;
        const implementationChatId =
          action === "start" || action === "resume"
            ? plan.phases.find((phase) => phase.id === plan.activePhaseId)
              ?.implementationChatId ?? null
            : null;
        if (implementationChatId) {
          selectWorkspaceChat(workspaceId, implementationChatId);
        }
        return true;
      } catch (requestError) {
        setActivePlansError(errorMessage(requestError));
        return false;
      } finally {
        setPlanOperationKey((current) =>
          current === operationKey ? null : current,
        );
      }
    },
    [loadActivePlans, refreshWorkspaces],
  );

  const runPlanPhaseRetry = useCallback(
    async (
      workspaceId: string,
      planId: string,
      phaseId: string,
      agentTaskId: string,
      implementationChatId: string | null,
    ) => {
      const operationKey = `retry-phase:${agentTaskId}`;
      const refreshTarget = { agentTaskId, phaseId, planId, workspaceId };
      setPlanOperationKey(operationKey);
      setActivePlansError(null);

      try {
        const response = await requestJson<PlanResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/${encodeURIComponent(planId)}/action`,
          {
            body: JSON.stringify({ action: "start" }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        const plansResponse = await loadActivePlans(workspaceId);
        const plan =
          plansResponse?.plans.find((candidate) => candidate.id === planId) ??
          response.plan;
        const retriedPhase =
          plan.phases.find((phase) => phase.id === phaseId) ?? null;
        setPendingPlanPhaseRetryRefresh(
          plansResponse &&
            !planPhaseRetryRefreshStillRunning(plansResponse.plans, refreshTarget)
            ? null
            : refreshTarget,
        );
        await refreshWorkspaces();
        const chatId = retriedPhase?.implementationChatId ?? implementationChatId;
        if (chatId) {
          selectWorkspaceChat(workspaceId, chatId);
        }
      } catch (requestError) {
        setActivePlansError(errorMessage(requestError));
      } finally {
        setPlanOperationKey((current) =>
          current === operationKey ? null : current,
        );
      }
    },
    [loadActivePlans, refreshWorkspaces],
  );

  const deletePlan = useCallback(
    async (workspaceId: string, planId: string) => {
      if (!window.confirm(t("Delete plan confirmation"))) {
        return;
      }

      const operationKey = `delete:${planId}`;
      setPlanOperationKey(operationKey);
      setActivePlansError(null);

      try {
        await requestJson<{ deleted: boolean }>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/${encodeURIComponent(planId)}`,
          { method: "DELETE" },
        );
        await loadActivePlans(workspaceId);
        await refreshWorkspaces();
      } catch (requestError) {
        setActivePlansError(errorMessage(requestError));
      } finally {
        setPlanOperationKey((current) =>
          current === operationKey ? null : current,
        );
      }
    },
    [loadActivePlans, refreshWorkspaces, t],
  );

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

    if (!isContextPanelOpen || contextPanelTab !== "todo") {
      return;
    }

    setTodoGraph(null);
    setTodoGraphError(null);
    void loadTodoGraph(
      todoGraphChatTarget.workspaceId,
      todoGraphChatTarget.chatId,
    );
  }, [activeChatKey, activeWorkspace?.id, contextPanelTab, isContextPanelOpen, loadTodoGraph]);

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

    if (!isContextPanelOpen || contextPanelTab !== "stats") {
      return;
    }

    const requestedChatKey = chatRunKey(activeWorkspace.id, activeChatId);
    setChatStatistics(null);
    setChatStatisticsError(null);
    if (!runningChatKeysRef.current.has(requestedChatKey)) {
      clearLiveChatStatistics(requestedChatKey);
    }
    void loadChatStatistics(activeWorkspace.id, activeChatId);
  }, [
    activeChatId,
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
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
    if (!activeWorkspace?.id) {
      setActivePlans([]);
      setActivePlansError(null);
      setIsLoadingActivePlans(false);
      return;
    }

    if (!isContextPanelOpen || contextPanelTab !== "plan") {
      return;
    }

    void loadActivePlans(activeWorkspace.id);
  }, [
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    loadActivePlans,
  ]);

  useEffect(() => {
    setContextMemoryPages({
      global: { page: 1, pageSize: 10 },
      workspace: { page: 1, pageSize: 10 },
    });
  }, [activeWorkspace?.id]);

  useEffect(() => {
    setWorkspaceSpec(null);
    setWorkspaceSpecDraft("");
    setWorkspaceSpecPreviewEnabled(false);
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);
  }, [activeWorkspace?.id]);

  useEffect(() => {
    setActivePlans([]);
    setActivePlansError(null);
    setPlanOperationKey(null);
    setIsPlanAutoRunDispatching(false);
    setPendingPlanPhaseRetryRefresh(null);
  }, [activeWorkspace?.id]);

  useEffect(() => {
    window.localStorage.setItem(
      PLAN_AUTO_RUN_ENABLED_STORAGE_KEY,
      isPlanAutoRunEnabled ? "true" : "false",
    );
  }, [isPlanAutoRunEnabled]);

  useEffect(() => {
    if (!isPlanAutoRunEnabled || !activeWorkspace?.id) {
      return;
    }
    if (isPlanAutoRunDispatching || planOperationKey) {
      return;
    }
    if (activePlans.some(isAutoRunPlanInFlight)) {
      return;
    }

    const nextPlanAction = nextAutoRunnablePlan(activePlans);
    if (!nextPlanAction) {
      return;
    }

    // ponytail: this is a frontend queue pump over the current active view.
    // Ceiling: active view limit=50; upgrade path is a backend persisted runner.
    setIsPlanAutoRunDispatching(true);
    void runPlanAction(
      activeWorkspace.id,
      nextPlanAction.planId,
      nextPlanAction.action,
    ).then((ok) => {
      setIsPlanAutoRunDispatching(false);
      if (!ok) {
        setIsPlanAutoRunEnabled(false);
      }
    });
  }, [
    activePlans,
    activeWorkspace?.id,
    isPlanAutoRunDispatching,
    isPlanAutoRunEnabled,
    planOperationKey,
    runPlanAction,
  ]);

  useEffect(() => {
    if (!isPlanAutoRunEnabled || !activeWorkspace?.id) {
      return;
    }
    if (!activePlans.some(isAutoRunPlanInFlight)) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadActivePlans(activeWorkspace.id);
    }, PLAN_AUTO_RUN_REFRESH_MS);

    return () => window.clearInterval(intervalId);
  }, [
    activePlans,
    activeWorkspace?.id,
    isPlanAutoRunEnabled,
    loadActivePlans,
  ]);

  useEffect(() => {
    const refreshTarget = pendingPlanPhaseRetryRefresh;
    if (!refreshTarget) {
      return;
    }
    if (activeWorkspace?.id !== refreshTarget.workspaceId) {
      setPendingPlanPhaseRetryRefresh(null);
      return;
    }
    const target = refreshTarget;

    let cancelled = false;
    async function refreshRetryPhase() {
      const plansResponse = await loadActivePlans(target.workspaceId);
      if (cancelled || !plansResponse) {
        return;
      }
      if (!planPhaseRetryRefreshStillRunning(plansResponse.plans, target)) {
        setPendingPlanPhaseRetryRefresh((current) =>
          current && samePlanPhaseRetryRefreshTarget(current, target)
            ? null
            : current,
        );
      }
    }

    const intervalId = window.setInterval(() => {
      void refreshRetryPhase();
    }, PLAN_PHASE_RETRY_REFRESH_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [activeWorkspace?.id, loadActivePlans, pendingPlanPhaseRetryRefresh]);

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
      availableSkills.map((skill) => skill.key),
    );

    setSelectedSkillIds((current) => {
      const next = current.filter((skillId) => enabledSkillIds.has(skillId));
      return next.length === current.length ? current : next;
    });
  }, [availableSkills]);

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

    const currentByKey = chatMessagesByKeyRef.current;
    const nextForKey = resolveNext(currentByKey[chatKey] ?? []);
    const nextByKey = { ...currentByKey, [chatKey]: nextForKey };
    setChatMessagesByKey(nextByKey);

    if (activeChatKeyRef.current === chatKey) {
      setMessages(nextForKey);
    }
  }

  const STREAM_TEXT_DELTA_FLUSH_MS = 32;

  function appendBufferedTextDelta(
    current: ShellMessage[],
    assistantMessageId: string,
    delta: string,
  ) {
    const messageIndex = current.findIndex(
      (message) =>
        message.role === "assistant" && message.id === assistantMessageId,
    );
    if (messageIndex < 0) {
      return current;
    }

    const message = current[messageIndex];
    const next = [...current];
    next[messageIndex] = {
      ...message,
      content: message.content + delta,
      parts: appendTextPart(message.parts, delta),
    };
    return next;
  }

  function createTextDeltaBuffer() {
    const bufferedDeltasByChatKey = new Map<string, Map<string, string>>();
    let flushTimer: number | null = null;

    const cancelScheduledFlush = () => {
      if (flushTimer === null) {
        return;
      }

      window.clearTimeout(flushTimer);
      flushTimer = null;
    };

    const flush = () => {
      cancelScheduledFlush();
      if (!bufferedDeltasByChatKey.size) {
        return;
      }

      const bufferedDeltas = Array.from(bufferedDeltasByChatKey.entries());
      bufferedDeltasByChatKey.clear();

      for (const [chatKey, messageDeltas] of bufferedDeltas) {
        for (const [assistantMessageId, delta] of messageDeltas) {
          setMessagesForChatKey(chatKey, (current) =>
            appendBufferedTextDelta(current, assistantMessageId, delta),
          );
        }
      }
    };

    return {
      flush,
      push(
        chatKey: string,
        assistantMessageId: string,
        delta: string,
      ) {
        const messageDeltas =
          bufferedDeltasByChatKey.get(chatKey) ?? new Map<string, string>();
        messageDeltas.set(
          assistantMessageId,
          `${messageDeltas.get(assistantMessageId) ?? ""}${delta}`,
        );
        bufferedDeltasByChatKey.set(chatKey, messageDeltas);

        if (flushTimer !== null) {
          return;
        }

        // ponytail: 32ms batching keeps the hot path simple; swap to RAF if we
        // ever need tighter frame alignment.
        flushTimer = window.setTimeout(() => {
          flushTimer = null;
          flush();
        }, STREAM_TEXT_DELTA_FLUSH_MS);
      },
    };
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

    moveChatPaginationForChatKey(fromChatKey, toChatKey);
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

  function moveChatPaginationForChatKey(fromChatKey: string, toChatKey: string) {
    setChatMessagePaginationByKey((current) => {
      const pagination = current[fromChatKey];
      if (!pagination) {
        return current;
      }
      const { [fromChatKey]: _removed, ...next } = current;
      return { ...next, [toChatKey]: pagination };
    });
  }

  function removeChatPaginationForChatKey(chatKey: string) {
    setChatMessagePaginationByKey((current) => {
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

  function workspaceChatListItemsFor(workspace: WorkspaceSummary) {
    const persistedWorkspaceChatIds = new Set(
      workspace.chats.map((chat) => chat.id),
    );
    const scheduledChats = scheduledWorkspaceRunsFor(workspace.id)
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
        scheduledStatus: chat.queuedRun?.status === "queued" ? "queued" : undefined,
      }),
    );

    return [...scheduledChats, ...persistedWorkspaceChats].sort(
      compareWorkspaceChatListItemsByCreatedAtDesc,
    );
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
    const cachedMessages = chatMessagesByKeyRef.current[run.chatKey] ?? [];
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
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages?limit=${CHAT_MESSAGES_PAGE_LIMIT}`,
        { signal: controller.signal },
      );
      const nextMessages = data.messages.map(normalizeChatMessageSummary);
      const activeRun = normalizeActiveChatRunSummary(data.activeRun);
      const pagination = normalizeChatMessagesPagination(data.pagination);
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
      setChatMessagePaginationByKey((current) => ({ ...current, [chatKey]: pagination }));
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

  async function loadOlderChatMessages(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);
    const pagination = chatMessagePaginationByKeyRef.current[chatKey];
    if (
      !pagination?.hasMoreBefore ||
      pagination.nextBeforeSequence === null ||
      loadingOlderChatMessageKeysRef.current.has(chatKey)
    ) {
      return;
    }

    loadingOlderChatMessageKeysRef.current.add(chatKey);
    setLoadingOlderChatMessageKeys((current) => new Set(current).add(chatKey));
    setError(null);

    try {
      const params = new URLSearchParams({
        beforeSequence: String(pagination.nextBeforeSequence),
        limit: String(CHAT_MESSAGES_PAGE_LIMIT),
      });
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages?${params}`,
      );
      const olderMessages = data.messages.map(normalizeChatMessageSummary);
      const nextPagination = normalizeChatMessagesPagination(data.pagination);
      let nextMessagesForChat = chatMessagesByKeyRef.current[chatKey] ?? [];

      setChatMessagesByKey((current) => {
        const existingMessages = current[chatKey] ?? [];
        const existingIds = new Set(existingMessages.map((message) => message.id));
        nextMessagesForChat = [
          ...olderMessages.filter((message) => !existingIds.has(message.id)),
          ...existingMessages,
        ];
        return { ...current, [chatKey]: nextMessagesForChat };
      });
      setChatMessagePaginationByKey((current) => ({
        ...current,
        [chatKey]: nextPagination,
      }));
      if (activeChatKeyRef.current === chatKey) {
        setMessages(nextMessagesForChat);
      }
    } catch (requestError) {
      if (activeChatKeyRef.current === chatKey) {
        setError(errorMessage(requestError));
      }
    } finally {
      loadingOlderChatMessageKeysRef.current.delete(chatKey);
      setLoadingOlderChatMessageKeys((current) => {
        const next = new Set(current);
        next.delete(chatKey);
        return next;
      });
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
      const cachedMessages = chatMessagesByKeyRef.current[chatKey] ?? [];
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
    const workspaceChatActiveRun = normalizeActiveChatRunSummary(
      workspaces
        .find((workspace) => workspace.id === workspaceId)
        ?.chats.find((chat) => chat.id === chatId)?.activeRun,
    );
    for (const [loadingChatKey, controller] of loadingChatControllersRef.current) {
      if (loadingChatKey !== chatKey) {
        controller.abort();
      }
    }
    const cachedMessages = chatMessagesByKeyRef.current[chatKey];

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
    if (workspaceChatActiveRun) {
      void subscribeActiveChatRun(workspaceChatActiveRun);
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
    const cachedMessages = chatMessagesByKeyRef.current[chatKey];

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

    if (!isWorkspaceImageFilePath(file.path)) {
      preloadOptionalMonaco();
    }

    selectWorkspaceFileTab(file);
    if (!isWorkspaceImageFilePath(file.path)) {
      await loadWorkspaceFileEditor(file);
    }
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
    if (!isWorkspaceImageFilePath(selectedFile.path)) {
      preloadOptionalMonaco();
      void loadWorkspaceFileEditor(selectedFile);
    }
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
    if (!isWorkspaceImageFilePath(file.path)) {
      initWorkspaceFileEditor(file.workspaceId, file.path);
    }
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
    if (isWorkspaceImageFilePath(tab.path)) {
      return;
    }

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

  function closeMainTabs(scope: MainTabCloseScope, anchorTab: MainTabSummary) {
    const anchorIndex = mainTabs.findIndex(
      (tab) => mainTabKey(tab) === mainTabKey(anchorTab),
    );
    if (anchorIndex < 0) {
      return;
    }

    const candidates = mainTabs.filter((tab, index) => {
      if (scope === "current") {
        return index === anchorIndex;
      }
      if (scope === "others") {
        return index !== anchorIndex;
      }
      if (scope === "right") {
        return index > anchorIndex;
      }
      if (scope === "left") {
        return index < anchorIndex;
      }
      return true;
    });
    const tabsToClose = candidates.filter(
      (tab) =>
        tab.type !== "chat" ||
        !runningChatKeys.has(chatRunKey(tab.workspaceId, tab.chatId)),
    );
    if (!tabsToClose.length) {
      return;
    }

    const closedKeys = new Set(tabsToClose.map(mainTabKey));
    const nextTabs = mainTabs.filter((tab) => !closedKeys.has(mainTabKey(tab)));
    const nextOpenChatTabs = openChatTabsRef.current.filter(
      (tab) => !closedKeys.has(`chat:${chatRunKey(tab.workspaceId, tab.chatId)}`),
    );
    const nextOpenFileTabs = openFileTabsRef.current.filter(
      (tab) => !closedKeys.has(workspaceFileEditorKey(tab.workspaceId, tab.path)),
    );

    openChatTabsRef.current = nextOpenChatTabs;
    openFileTabsRef.current = nextOpenFileTabs;
    setOpenChatTabs(nextOpenChatTabs);
    setOpenFileTabs(nextOpenFileTabs);
    setOpenAgentTabs((current) =>
      current.filter(
        (tab) =>
          !closedKeys.has(`agent:${tab.workspaceId}:${tab.chatId}:${tab.instanceId}`),
      ),
    );

    for (const tab of tabsToClose) {
      if (tab.type !== "chat") {
        continue;
      }
      const chatKey = chatRunKey(tab.workspaceId, tab.chatId);
      setChatRunFailed(chatKey, false);
      removeMessagesForChatKey(chatKey);
      removeChatPaginationForChatKey(chatKey);
      removeContextUsageForChatKey(chatKey);
    }

    setWorkspaceFileEditors((current) => {
      const next = { ...current };
      for (const tab of tabsToClose) {
        if (tab.type === "file") {
          delete next[workspaceFileEditorKey(tab.workspaceId, tab.path)];
        }
      }
      return next;
    });

    const activeWasClosed = tabsToClose.some((tab) => mainTabMatches(activeMainTab, tab));
    if (!activeWasClosed) {
      if (activeMainTab.type === "file" && activeFileTab) {
        updateBrowserRoute(browserRouteForActiveFile(activeFileTab), "replace");
      } else {
        updateBrowserRoute({
          chatId: activeChatId,
          viewMode: "chat",
          workspaceId: activeWorkspaceId || anchorTab.workspaceId,
        }, "replace");
      }
      return;
    }

    const nextTab = nextTabs[Math.min(anchorIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    const workspaceId = activeWorkspaceId || anchorTab.workspaceId;
    setActiveWorkspaceChatRefs(workspaceId, null);
    setActiveChatId(null);
    setMessages([]);
    setActiveMainTab({ chatId: null, type: "chat", workspaceId });
    updateBrowserRoute({
      chatId: null,
      viewMode: "chat",
      workspaceId,
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
    removeChatPaginationForChatKey(chatKey);
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

      const chatKey = chatRunKey(workspaceId, chatId);
      removeMessagesForChatKey(chatKey);
      removeChatPaginationForChatKey(chatKey);
      removeContextUsageForChatKey(chatKey);
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

    for (const attachment of attachments) {
      const unsupportedMessage = unsupportedAttachmentMessage(
        selectedModel,
        attachment,
        t,
      );
      if (unsupportedMessage) {
        setError(unsupportedMessage);
        return;
      }
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
      for (const file of files) {
        const unsupportedMessage = unsupportedFileAttachmentMessage(
          selectedModel,
          file,
          t,
        );
        if (unsupportedMessage) {
          setError(unsupportedMessage);
          return;
        }
      }
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
      for (const file of files) {
        const unsupportedMessage = unsupportedFileAttachmentMessage(
          selectedModel,
          file,
          t,
        );
        if (unsupportedMessage) {
          setError(unsupportedMessage);
          return;
        }
      }
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

    const unsupportedAttachment = attachments.find((attachment) =>
      unsupportedAttachmentInputModality(selectedModel, attachment.contentType),
    );
    if (unsupportedAttachment) {
      const message = unsupportedAttachmentMessage(
        selectedModel,
        unsupportedAttachment,
        t,
      );
      setError(message ?? t("Selected model does not support this attachment."));
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
      sessionMode: isPlanModeEnabled ? "plan" : undefined,
      teamModeEnabled: !isPlanModeEnabled && canUseTeamMode && isTeamModeEnabled,
      thinkingLevel: selectedThinkingLevel,
      workspaceId: currentWorkspace.id,
    };
  }

  function handlePlanModeEnabledChange(value: boolean) {
    setIsPlanModeEnabled(value);
    if (value) {
      setIsTeamModeEnabled(false);
    }
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
          sessionMode: request.sessionMode ?? null,
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
          sessionMode: queued.sessionMode ?? request.sessionMode,
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
          sessionMode: queued.sessionMode ?? request.sessionMode,
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

  function handleLogoNavClick() {
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({
      chatId: null,
      files: [],
      tabs: [],
      viewMode: "chat",
      workspaceId: null,
    });
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
    deferStreamSideUpdate(() => {
      setContextUsageLoadingByChatKey((current) => ({
        ...current,
        [chatKey]: true,
      }));
    });

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
        deferStreamSideUpdate(() => {
          setContextUsageByChatKey((current) => ({ ...current, [chatKey]: data }));
        });
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
        deferStreamSideUpdate(() => {
          setContextUsageLoadingByChatKey((current) => ({
            ...current,
            [chatKey]: false,
          }));
        });
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
    const existingAbortController = activeRunAbortByChatKeyRef.current.get(chatKey);
    if (existingAbortController) {
      const existingRunId = activeRunInfoByChatKeyRef.current[chatKey]?.runId;
      if (existingRunId === activeRun.runId) {
        return;
      }
      existingAbortController.abort();
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
    const textDeltaBuffer = createTextDeltaBuffer();
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
    const streamAttemptSnapshots = new Map<string, StreamAttemptSnapshot>();
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
        if (streamEvent.type !== "textDelta") {
          textDeltaBuffer.flush();
        }

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
          textDeltaBuffer.push(
            chatKey,
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
            streamEvent.delta,
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
          const snapshotKey = resolvedAssistantMessageId(streamEvent.assistantMessageId);
          streamAttemptSnapshots.set(snapshotKey, emptyStreamingAttemptSnapshot());
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(chatKey, (current) => {
            const message = current.find((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
            );
            if (message) {
              streamAttemptSnapshots.set(snapshotKey, streamingAttemptSnapshot(message));
            }
            return current;
          });
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
                ? resetStreamingAssistantMessage(
                  message,
                  streamEvent,
                  streamAttemptSnapshots.get(
                    resolvedAssistantMessageId(streamEvent.assistantMessageId),
                  ),
                )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "contextCompression") {
          deferStreamSideUpdate(() => {
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                  ? addChatRunBadge(message, contextCompressionBadge(streamEvent.kind))
                  : message,
              ),
            );
          });
          return;
        }

        if (streamEvent.type === "usage") {
          latestResponseUsage =
            streamEvent.usage &&
              streamEvent.usage.inputTokens !== null &&
              streamEvent.usage.outputTokens !== null
              ? streamEvent.usage
              : null;
          deferStreamSideUpdate(() => {
            updateLiveChatStatistics(chatKey, {
              modelId: selectedModelIdRef.current,
              providerId: selectedProviderIdRef.current,
              startedAtMs: liveStartedAtMs,
              usage: latestResponseUsage,
            });
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
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "toolResult") {
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          deferStreamSideUpdate(() => {
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
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          if (isContextPanelOpen && contextPanelTab === "git") {
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          }
          deferStreamSideUpdate(() => {
            updateLiveChatStatistics(chatKey, {
              codeChangeStats: streamEvent.codeChangeStats,
              modelId: selectedModelIdRef.current,
              providerId: selectedProviderIdRef.current,
              startedAtMs: liveStartedAtMs,
              usage: latestResponseUsage,
            });
          });
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          const isActiveTodoChat =
            activeChatKeyRef.current ===
            chatRunKey(streamEvent.workspaceId, streamEvent.chatId);
          if (isActiveTodoChat) {
            setContextPanelTab("todo");
            setIsContextPanelOpen(true);
            void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId, {
              ignoreRequestInvalidation: true,
            });
          }
          return;
        }

        if (streamEvent.type === "planRefresh") {
          handlePlanRefresh(streamEvent);
          return;
        }

        if (streamEvent.type === "agentTeamRefresh") {
          handleAgentTeamRefresh(streamEvent);
          return;
        }

        if (streamEvent.type === "memoryExtractionComplete") {
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          deferStreamSideUpdate(() => {
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
          });
          return;
        }
        if (streamEvent.type === "memoryResolved") {
          deferStreamSideUpdate(() => {
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
          });
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
      textDeltaBuffer.flush();
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (!wasCancelled) {
        setChatRunFailed(chatKey, true);
        setError(errorMessage(requestError));
      }
    } finally {
      textDeltaBuffer.flush();
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
    const textDeltaBuffer = createTextDeltaBuffer();
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
                sessionMode: request.sessionMode,
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
          sessionMode: request.sessionMode,
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
    const streamAttemptSnapshots = new Map<string, StreamAttemptSnapshot>();
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
            sessionMode: request.sessionMode ?? null,
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
        if (streamEvent.type !== "textDelta") {
          textDeltaBuffer.flush();
        }

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
          textDeltaBuffer.push(
            runMessagesKey,
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
            streamEvent.delta,
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
          const snapshotKey = resolvedAssistantMessageId(streamEvent.assistantMessageId);
          streamAttemptSnapshots.set(snapshotKey, emptyStreamingAttemptSnapshot());
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(runMessagesKey, (current) => {
            const message = current.find((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
            );
            if (message) {
              streamAttemptSnapshots.set(snapshotKey, streamingAttemptSnapshot(message));
            }
            return current;
          });
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
                ? resetStreamingAssistantMessage(
                  message,
                  streamEvent,
                  streamAttemptSnapshots.get(
                    resolvedAssistantMessageId(streamEvent.assistantMessageId),
                  ),
                )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "contextCompression") {
          deferStreamSideUpdate(() => {
            setMessagesForChatKey(runMessagesKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                  ? addChatRunBadge(message, contextCompressionBadge(streamEvent.kind))
                  : message,
              ),
            );
          });
          return;
        }

        if (streamEvent.type === "usage") {
          latestResponseUsage =
            streamEvent.usage &&
              streamEvent.usage.inputTokens !== null &&
              streamEvent.usage.outputTokens !== null
              ? streamEvent.usage
              : null;
          deferStreamSideUpdate(() => {
            updateLiveChatStatistics(runMessagesKey, {
              modelId: request.modelId,
              providerId: request.providerId,
              startedAtMs: liveStartedAtMs,
              usage: latestResponseUsage,
            });
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
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "toolResult") {
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          deferStreamSideUpdate(() => {
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
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          if (isContextPanelOpen && contextPanelTab === "git") {
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          }
          deferStreamSideUpdate(() => {
            updateLiveChatStatistics(runMessagesKey, {
              codeChangeStats: streamEvent.codeChangeStats,
              modelId: request.modelId,
              providerId: request.providerId,
              startedAtMs: liveStartedAtMs,
              usage: latestResponseUsage,
            });
          });
          if (requestChatId) {
            void loadChatStatistics(request.workspaceId, requestChatId);
          }
          return;
        }

        if (streamEvent.type === "todoGraphRefresh") {
          const isActiveTodoChat =
            activeChatKeyRef.current ===
            chatRunKey(streamEvent.workspaceId, streamEvent.chatId);
          if (isActiveTodoChat) {
            setContextPanelTab("todo");
            setIsContextPanelOpen(true);
            void loadTodoGraph(streamEvent.workspaceId, streamEvent.chatId, {
              ignoreRequestInvalidation: true,
            });
          }
          return;
        }

        if (streamEvent.type === "planRefresh") {
          handlePlanRefresh(streamEvent);
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
          deferStreamSideUpdate(() => {
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
          });
          return;
        }

        if (streamEvent.type === "memoryResolved") {
          deferStreamSideUpdate(() => {
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
          });
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
      textDeltaBuffer.flush();
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
      textDeltaBuffer.flush();
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

  async function saveWorkspaceOrder(
    workspaceIds: string[],
    previousWorkspaces: WorkspaceSummary[],
  ) {
    setError(null);
    setWorkspaces((current) => reorderWorkspacesByIds(current, workspaceIds));

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/order", {
        body: JSON.stringify({ workspaceIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      setWorkspaces((current) =>
        reorderWorkspacesByIds(
          current,
          data.workspaces.map((workspace) => workspace.id),
        ),
      );
    } catch (requestError) {
      setWorkspaces(previousWorkspaces);
      setError(errorMessage(requestError));
    }
  }

  function handleWorkspaceDragStart(
    event: ReactDragEvent<HTMLDivElement>,
    workspaceId: string,
  ) {
    const workspaceIds = workspaces.map((workspace) => workspace.id);
    setDraggedWorkspaceId(workspaceId);
    workspaceOrderDropHandledRef.current = false;
    workspaceOrderPreviewRef.current = workspaceIds;
    setWorkspaceOrderPreview(workspaceIds);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", workspaceId);
  }

  function handleWorkspaceDragOver(
    event: ReactDragEvent<HTMLDivElement>,
    targetWorkspaceId: string,
  ) {
    const sourceWorkspaceId = draggedWorkspaceId;
    if (!sourceWorkspaceId || sourceWorkspaceId === targetWorkspaceId) {
      return;
    }

    const sourceWorkspace = workspaces.find(
      (workspace) => workspace.id === sourceWorkspaceId,
    );
    const targetWorkspace = workspaces.find(
      (workspace) => workspace.id === targetWorkspaceId,
    );
    if (!sourceWorkspace || !targetWorkspace || sourceWorkspace.pinned !== targetWorkspace.pinned) {
      return;
    }

    event.preventDefault();
    const workspaceIds = moveItemId(
      workspaceOrderPreviewRef.current ?? workspaces.map((workspace) => workspace.id),
      sourceWorkspaceId,
      targetWorkspaceId,
    );
    workspaceOrderPreviewRef.current = workspaceIds;
    setWorkspaceOrderPreview(workspaceIds);
  }

  async function commitWorkspaceOrderPreview(workspaceIds: string[] | null) {
    const previousWorkspaces = workspaces;
    setDraggedWorkspaceId(null);
    workspaceOrderPreviewRef.current = null;
    setWorkspaceOrderPreview(null);

    if (!workspaceIds || sameStringList(workspaceIds, previousWorkspaces.map((workspace) => workspace.id))) {
      return;
    }

    await saveWorkspaceOrder(workspaceIds, previousWorkspaces);
  }

  async function handleWorkspaceDrop(event: ReactDragEvent<HTMLDivElement>) {
    event.preventDefault();
    workspaceOrderDropHandledRef.current = true;
    await commitWorkspaceOrderPreview(workspaceOrderPreviewRef.current);
  }

  function handleWorkspaceDragEnd() {
    if (workspaceOrderDropHandledRef.current) {
      workspaceOrderDropHandledRef.current = false;
      setDraggedWorkspaceId(null);
      workspaceOrderPreviewRef.current = null;
      setWorkspaceOrderPreview(null);
      return;
    }

    void commitWorkspaceOrderPreview(workspaceOrderPreviewRef.current);
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
    void loadAgentDefinitions();
  }, [loadAgentDefinitions]);

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

  const chatPanelHelpers = useMemo<ChatPanelHelpers>(
    () => ({
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
    }),
    [],
  );
  const chatOverviewRenderer = useCallback(
    () => (
      <ApiOverviewPanel
        activeWorkspaceId={activeWorkspaceId}
        autoLoadEnabled={!isPreparingChatRun}
        settings={settings}
        workspaces={workspaces}
      />
    ),
    [activeWorkspaceId, isPreparingChatRun, settings, workspaces],
  );
  const handleAddPastedImageAttachmentsForChatPanel = useStableCallback(
    (files: File[]) => void handleAddPastedImageAttachments(files),
  );
  const handleBranchChangeForChatPanel = useStableCallback(
    (branch: string) => void handleGitBranchChange(branch),
  );
  const handleGuideQueuedMessageForChatPanel = useStableCallback(
    (messageId: string) => void handleGuideQueuedMessage(messageId),
  );
  const handleSelectDraftAttachmentsForChatPanel = useStableCallback(
    (files: File[]) => void handleSelectDraftAttachments(files),
  );
  const handleCancelRunForChatPanel = useStableCallback(
    () => void handleCancelRun(),
  );
  const handleGuideActiveRunForChatPanel = useStableCallback(
    () => void handleGuideActiveRun(),
  );
  const handleQueueActiveRunForChatPanel = useStableCallback(
    () => void handleQueueActiveRun(),
  );
  const handleRetryRunForChatPanel = useStableCallback(
    () => void handleRetryRun(),
  );
  const handleSubmitForChatPanel = useStableCallback(
    (
      event: FormEvent<HTMLFormElement>,
      options?: { schedule?: boolean },
    ) => void handleSendMessage(event, options),
  );
  const handleModelChangeForChatPanel = useStableCallback(handleChatModelChange);
  const handleProviderChangeForChatPanel = useStableCallback(handleChatProviderChange);
  const handleRemoveAttachmentForChatPanel = useStableCallback(handleRemoveDraftAttachment);
  const handleRemoveSkillForChatPanel = useStableCallback(removeSelectedSkill);
  const handleThinkingLevelChangeForChatPanel = useStableCallback(
    handleChatThinkingLevelChange,
  );
  const handleToggleSkillForChatPanel = useStableCallback(toggleSelectedSkill);
  const handleWithdrawQueuedMessageForChatPanel = useStableCallback(
    handleWithdrawQueuedMessage,
  );
  const providersForChatPanel = settings?.providers ?? EMPTY_CONFIGURED_PROVIDERS;
  const activeChatCoordinatorInstance =
    agentTeamSnapshot?.team.chatId === activeChatId
      ? agentTeamSnapshot.instances.find(
          (instance) => instance.id === agentTeamSnapshot.team.coordinatorInstanceId,
        ) ?? null
      : null;
  const activeChatWorktreeBranch =
    activeChatCoordinatorInstance?.executionWorkspaceMode === "isolated_worktree" &&
    activeChatCoordinatorInstance.worktreeStatus !== "deleted"
      ? activeChatCoordinatorInstance.worktreeBranch
      : null;
  const refreshAgentPanelForContextPanel = useStableCallback(async () => {
    if (activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)) {
      await loadAgentTeamSnapshot(activeWorkspaceId, activeChatId);
    }
  });
  const openAgentInstanceTabForContextPanel = useStableCallback(openAgentInstanceTab);
  const agentsPanelForContextPanel = useMemo(
    () => (
      <AgentsRuntimePanel
        activeChatId={
          activeChatId && !isPendingChatId(activeChatId)
            ? activeChatId
            : null
        }
        error={agentTeamError}
        isLoading={isLoadingAgentTeam}
        onRefresh={refreshAgentPanelForContextPanel}
        onSelectInstance={openAgentInstanceTabForContextPanel}
        selectedInstanceId={
          activeMainTab.type === "agent"
            ? activeMainTab.instanceId
            : agentTeamSnapshot?.team.coordinatorInstanceId ?? null
        }
        snapshot={agentTeamSnapshot}
      />
    ),
    [
      activeChatId,
      activeMainTab,
      agentTeamError,
      agentTeamSnapshot,
      isLoadingAgentTeam,
      openAgentInstanceTabForContextPanel,
      refreshAgentPanelForContextPanel,
    ],
  );
  const handleGenerateGitCommitMessageForContextPanel = useStableCallback(
    () => void handleGenerateGitCommitMessage(),
  );
  const handleGitFileOperationForContextPanel = useStableCallback(
    (action: "stage" | "unstage" | "discard", path: string) =>
      void handleGitFileOperation(action, path),
  );
  const handleRefreshWorkspaceFilesForContextPanel = useStableCallback(() => {
    if (activeWorkspace?.id) {
      void loadWorkspaceFiles(activeWorkspace.id);
    }
  });
  const handleOpenWorkspaceFileForContextPanel = useStableCallback(
    (node: WorkspaceFileTreeNode) => void openWorkspaceFileTab(node),
  );
  const handleOpenWorkspaceFileMenuForContextPanel = useStableCallback(
    (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => {
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
    },
  );
  const handleRefreshDiffForContextPanel = useStableCallback(() => {
    if (activeWorkspace?.id) {
      void loadGitDiff(activeWorkspace.id, selectedDiffPath);
    }
  });
  const handleForgetContextMemoryForContextPanel = useStableCallback(
    (memory: MemoryFactRecord) => void forgetContextMemory(memory),
  );
  const handleReloadWorkspaceSpecForContextPanel = useStableCallback(() => {
    if (activeWorkspace?.id) {
      void loadWorkspaceSpec(activeWorkspace.id);
    }
  });
  const handleSaveWorkspaceSpecForContextPanel = useStableCallback(
    () => void saveWorkspaceSpecContent(),
  );
  const handleGenerateWorkspaceSpecForContextPanel = useStableCallback(
    () => void generateWorkspaceSpec(),
  );
  const handleWorkspaceSpecSettingsChangeForContextPanel = useStableCallback(
    (enabled: boolean, injectEnabled: boolean) => {
      if (activeWorkspace?.id) {
        void saveWorkspaceSpecSettings(
          activeWorkspace.id,
          enabled,
          injectEnabled,
        );
      }
    },
  );
  const handleContextPanelTabChange = useStableCallback((tab: ContextPanelTab) => {
    if (tab === "files") {
      preloadOptionalMonaco();
    }

    setContextPanelTab(tab);
    setIsContextPanelOpen(true);
  });
  const contextPanelFiles = gitDiff?.files ?? EMPTY_GIT_STATUS_FILES;
  const normalizedWorkspaceChatSearchQuery = workspaceChatSearchQuery
    .trim()
    .toLocaleLowerCase();
  const isWorkspaceSearchActive = normalizedWorkspaceChatSearchQuery.length > 0;
  const sidebarWorkspaces = isWorkspaceSearchActive
    ? displayedWorkspaces.filter((workspace) =>
      workspaceChatListItemsFor(workspace).some((chat) =>
        chat.title.toLocaleLowerCase().includes(normalizedWorkspaceChatSearchQuery),
      ),
    )
    : displayedWorkspaces;

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
              onReturnHome={handleLogoNavClick}
              onToggleTheme={() =>
                void saveAppTheme(theme === "dark" ? "light" : "dark")
              }
              terminalButton={null}
              theme={theme}
            />
            <section className="global-main-panel min-w-0">
              <Suspense fallback={<PanelLoadingFallback />}>
              {viewMode === "settings" ? (
                <SettingsPanel
                  agentDefinitionOperationKey={agentDefinitionOperationKey}
                  agentDefinitions={agentDefinitions}
                  agentDefinitionsError={agentDefinitionsError}
                  defaultAgentRolePrompts={defaultAgentRolePrompts}
                  canLogout={canLogout}
                  canUseNativePicker={canUseNativePicker}
                  activeWorkspaceId={activeWorkspace?.id ?? activeWorkspaceId ?? null}
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
              </Suspense>
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
              onReturnHome={handleLogoNavClick}
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
                      aria-label={t("Refresh workspaces")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={isLoading}
                      onClick={() => void refreshWorkspaces()}
                      title={t("Refresh workspaces")}
                      type="button"
                    >
                      <RefreshCw
                        aria-hidden="true"
                        className={`size-4 ${isLoading ? "animate-spin" : ""}`}
                      />
                    </button>
                    <button
                      aria-label={t("Search chats")}
                      aria-pressed={workspaceChatSearchOpen}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={() => setWorkspaceChatSearchOpen((current) => !current)}
                      title={t("Search chats")}
                      type="button"
                    >
                      <Search aria-hidden="true" className="size-4" />
                    </button>
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

                {workspaceChatSearchOpen ? (
                  <div className="border-b border-stone-200/80 px-3 py-2">
                    <div className="flex items-center gap-2">
                      <input
                        aria-label={t("Search chats")}
                        className="h-9 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) => setWorkspaceChatSearchQuery(event.target.value)}
                        placeholder={t("Search chats placeholder")}
                        type="search"
                        value={workspaceChatSearchQuery}
                      />
                      <button
                        aria-label={t("Clear search")}
                        className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-500 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-300"
                        disabled={workspaceChatSearchQuery.length === 0}
                        onClick={() => setWorkspaceChatSearchQuery("")}
                        title={t("Clear search")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                  </div>
                ) : null}

                <nav
                  aria-label={t("Workspace list")}
                  className="workspace-nav panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3"
                >
                  {sidebarWorkspaces.length ? (
                    sidebarWorkspaces.map((workspace) => {
                      const isExpanded =
                        isWorkspaceSearchActive || expandedWorkspaceId === workspace.id;
                      const isActive = workspace.id === activeWorkspace?.id;
                      const workspaceChats = workspaceChatListItemsFor(workspace);
                      const searchedWorkspaceChats = isWorkspaceSearchActive
                        ? workspaceChats.filter((chat) =>
                          chat.title
                            .toLocaleLowerCase()
                            .includes(normalizedWorkspaceChatSearchQuery),
                        )
                        : workspaceChats;
                      const selectedChatIndex =
                        isActive && activeChatId
                          ? workspaceChats.findIndex((chat) => chat.id === activeChatId)
                          : -1;
                      const configuredVisibleChatCount =
                        workspaceChatVisibleCounts[workspace.id] ??
                        WORKSPACE_CHAT_HISTORY_PAGE_SIZE;
                      const visibleChatCount = isWorkspaceSearchActive
                        ? searchedWorkspaceChats.length
                        : selectedChatIndex >= configuredVisibleChatCount
                          ? selectedChatIndex + 1
                          : configuredVisibleChatCount;
                      const visibleChats = searchedWorkspaceChats.slice(0, visibleChatCount);
                      const hiddenChatCount = isWorkspaceSearchActive
                        ? 0
                        : Math.max(
                          workspaceChats.length - visibleChats.length,
                          0,
                        );
                      const nextVisibleChatCount = Math.min(
                        WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
                        hiddenChatCount,
                      );

                      return (
                        <div
                          className={`mb-1.5 ${draggedWorkspaceId === workspace.id ? "opacity-80" : ""}`}
                          draggable
                          key={workspace.id}
                          onDragEnd={handleWorkspaceDragEnd}
                          onDragOver={(event) =>
                            handleWorkspaceDragOver(event, workspace.id)
                          }
                          onDragStart={(event) =>
                            handleWorkspaceDragStart(event, workspace.id)
                          }
                          onDrop={(event) => void handleWorkspaceDrop(event)}
                        >
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
                              {visibleChats.length > 0 ? (
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
                      {isWorkspaceSearchActive
                        ? t("No matching chats")
                        : isLoading
                          ? t("Loading workspaces...")
                          : t("No workspaces")}
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
                    if (
                      !window.confirm(
                        t("Delete this file or folder?\n\nPath: {path}", { path: node.path }),
                      )
                    ) {
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
                    onCloseTabs={closeMainTabs}
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
                  draftUnsupportedAttachmentMessage={unsupportedDraftAttachmentMessage}
                  gitBranches={gitBranches}
                  contextUsage={contextUsage}
                  isLoadingSettings={isLoadingSettings}
                  isLoadingBranches={isLoadingBranches}
                  isLoadingContextUsage={isLoadingContextUsage}
                  isLoadingMessages={isLoadingActiveChatMessages}
                  hasMoreMessagesBefore={activeChatPagination?.hasMoreBefore === true}
                  isLoadingMoreMessages={isLoadingOlderActiveChatMessages}
                  isSendingMessage={isSendingMessage}
                  isSelectingAttachments={isSelectingAttachments}
                  isPlanModeEnabled={isPlanModeEnabled}
                  messages={messages}
                  readOnly={activeChatReadOnly}
                  overviewRenderer={chatOverviewRenderer}
                  onAddPastedImageAttachments={handleAddPastedImageAttachmentsForChatPanel}
                  onBranchChange={handleBranchChangeForChatPanel}
                  onDraftMessageChange={setDraftMessage}
                  onGuideQueuedMessage={handleGuideQueuedMessageForChatPanel}
                  onLoadMoreMessages={() => {
                    if (!activeWorkspaceId || !activeChatId || isPendingChatId(activeChatId)) {
                      return Promise.resolve();
                    }
                    return loadOlderChatMessages(activeWorkspaceId, activeChatId);
                  }}
                  onSelectAttachments={handleSelectDraftAttachmentsForChatPanel}
                  onCancelRun={handleCancelRunForChatPanel}
                  onGuideActiveRun={handleGuideActiveRunForChatPanel}
                  onQueueActiveRun={handleQueueActiveRunForChatPanel}
                  onModelChange={handleModelChangeForChatPanel}
                  onProviderChange={handleProviderChangeForChatPanel}
                  onRemoveAttachment={handleRemoveAttachmentForChatPanel}
                  onRemoveSkill={handleRemoveSkillForChatPanel}
                  onRetryRun={handleRetryRunForChatPanel}
                  onSubmit={handleSubmitForChatPanel}
                  onPlanModeEnabledChange={handlePlanModeEnabledChange}
                  onThinkingLevelChange={handleThinkingLevelChangeForChatPanel}
                  onToggleSkill={handleToggleSkillForChatPanel}
                  onWithdrawQueuedMessage={handleWithdrawQueuedMessageForChatPanel}
                  canRetryRun={retryRunRequest !== null && !isSendingMessage}
                  queuedRunCount={queuedRunRequests.length}
                  queuedMessageIds={queuedMessageIds}
                  selectedGitBranch={selectedGitBranch}
                  worktreeBranch={activeChatWorktreeBranch}
                  selectedModelId={selectedModelId}
                  selectedProviderId={selectedProviderId}
                  selectedSkillIds={selectedSkillIds}
                  selectedThinkingLevel={selectedThinkingLevel}
                  settings={settings}
                  providers={providersForChatPanel}
                  skills={availableSkills}
                  thinkingLevels={thinkingLevels}
                  workspaces={workspaces}
                  workspaceId={activeWorkspace?.id ?? (activeWorkspaceId || null)}
                />
              )}
              {workspaces
                .filter((workspace) => terminalOpenWorkspaceIds.has(workspace.id))
                .map((workspace) => (
                  <Suspense fallback={null} key={workspace.id}>
                    <TerminalPanel
                      errorMessage={errorMessage}
                      isVisible={workspace.id === activeWorkspace?.id}
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
                  </Suspense>
                ))}
            </section>

            {showContextPanel ? (
              <ContextPanelSidebar
                activeTab={contextPanelTab}
                agentsPanel={agentsPanelForContextPanel}
                chatStatistics={displayedChatStatistics}
                chatStatisticsError={chatStatisticsError}
                contextMemories={contextMemories}
                contextUsage={contextUsage}
                deletingContextMemoryId={deletingContextMemoryId}
                contextMemoryError={contextMemoryError}
                diffError={diffError}
                diffPanelWidth={diffPanelWidth}
                diffResponse={gitDiff}
                files={contextPanelFiles}
                gitCommitMessage={gitCommitMessage}
                gitOperationKey={gitOperationKey}
                expandedFileTreePaths={expandedFileTreePaths}
                isLoadingChatStatistics={isLoadingChatStatistics}
                isLoadingDiff={isLoadingDiff}
                isLoadingContextMemories={isLoadingContextMemories}
                isLoadingPlans={isLoadingActivePlans}
                isPlanAutoRunBusy={isPlanAutoRunDispatching || planOperationKey !== null}
                isPlanAutoRunEnabled={isPlanAutoRunEnabled}
                isPlanAutoRunToggleDisabled={!activeWorkspace?.id}
                isLoadingTodoGraph={isLoadingTodoGraph}
                isLoadingWorkspaceSpec={isLoadingWorkspaceSpec}
                isLoadingWorkspaceFiles={isLoadingWorkspaceFiles}
                isResizing={isResizingDiffPanel}
                loadingWorkspaceDirectoryPaths={loadingWorkspaceDirectoryPaths}
                onGitCommit={handleGitCommit}
                onGenerateGitCommitMessage={handleGenerateGitCommitMessageForContextPanel}
                onGitCommitMessageChange={setGitCommitMessage}
                onGitFileOperation={handleGitFileOperationForContextPanel}
                onRefreshWorkspaceFiles={handleRefreshWorkspaceFilesForContextPanel}
                onToggleFileTreePath={toggleWorkspaceFileTreePath}
                onOpenWorkspaceFile={handleOpenWorkspaceFileForContextPanel}
                onOpenWorkspaceFileMenu={handleOpenWorkspaceFileMenuForContextPanel}
                onRefreshDiff={handleRefreshDiffForContextPanel}
                onForgetContextMemory={handleForgetContextMemoryForContextPanel}
                onMemoryPageChange={goToContextMemoryPage}
                onPlanAction={(planId, action) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void runPlanAction(workspaceId, planId, action);
                  }
                }}
                onDeletePlan={(planId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void deletePlan(workspaceId, planId);
                  }
                }}
                onOpenPlanPhaseChat={(chatId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    selectWorkspaceChat(workspaceId, chatId);
                  }
                }}
                onPlanPhaseRetry={(planId, phaseId, agentTaskId, implementationChatId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void runPlanPhaseRetry(
                      workspaceId,
                      planId,
                      phaseId,
                      agentTaskId,
                      implementationChatId,
                    );
                  }
                }}
                onReloadWorkspaceSpec={handleReloadWorkspaceSpecForContextPanel}
                onSaveWorkspaceSpec={handleSaveWorkspaceSpecForContextPanel}
                onGenerateWorkspaceSpec={handleGenerateWorkspaceSpecForContextPanel}
                onWorkspaceSpecContentChange={setWorkspaceSpecDraft}
                onWorkspaceSpecPreviewChange={setWorkspaceSpecPreviewEnabled}
                onWorkspaceSpecSettingsChange={handleWorkspaceSpecSettingsChangeForContextPanel}
                onSelectDiffFile={setSelectedDiffPath}
                onTabChange={handleContextPanelTabChange}
                onPlanAutoRunToggle={setIsPlanAutoRunEnabled}
                selectedPath={selectedDiffPath}
                selectedSkillPrefix={selectedSkillPrefix}
                setMobileHeight={setContextPanelMobileHeight}
                setWidth={setDiffPanelWidth}
                onResizeStart={() => setIsResizingDiffPanel(true)}
                todoGraph={todoGraph}
                plans={activePlans}
                planError={activePlansError}
                planOperationKey={planOperationKey}
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
  onCloseTabs,
  onSelectTab,
  runningChatKeys,
  tabs,
}: {
  activeTab: ActiveMainTab;
  onCloseTab: (tab: MainTabSummary) => void;
  onCloseTabs: (scope: MainTabCloseScope, anchorTab: MainTabSummary) => void;
  onSelectTab: (tab: MainTabSummary) => void;
  runningChatKeys: Set<string>;
  tabs: MainTabSummary[];
}) {
  const { t } = useI18n();
  const tabsContainerRef = useRef<HTMLDivElement>(null);
  const tabListRef = useRef<HTMLDivElement>(null);
  const tabItemRefs = useRef(new Map<string, HTMLDivElement>());
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const hasTrackedTabKeysRef = useRef(false);
  const previousTabKeysRef = useRef<string[]>([]);
  const [contextMenu, setContextMenu] = useState<MainTabContextMenuState | null>(null);
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

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    function closeContextMenuForPointer(event: PointerEvent) {
      const target = event.target;
      if (
        target instanceof Element &&
        target.closest(".main-tab-context-menu")
      ) {
        return;
      }
      setContextMenu(null);
    }

    function closeContextMenuForKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    }

    function closeContextMenu() {
      setContextMenu(null);
    }

    window.addEventListener("pointerdown", closeContextMenuForPointer);
    window.addEventListener("keydown", closeContextMenuForKey);
    window.addEventListener("resize", closeContextMenu);
    window.addEventListener("scroll", closeContextMenu, true);

    return () => {
      window.removeEventListener("pointerdown", closeContextMenuForPointer);
      window.removeEventListener("keydown", closeContextMenuForKey);
      window.removeEventListener("resize", closeContextMenu);
      window.removeEventListener("scroll", closeContextMenu, true);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const contextMenuTabKey = mainTabKey(contextMenu.tab);
    if (!tabs.some((tab) => mainTabKey(tab) === contextMenuTabKey)) {
      setContextMenu(null);
    }
  }, [contextMenu, tabs]);

  useLayoutEffect(() => {
    if (!contextMenu || contextMenu.positioned) {
      return;
    }

    const element = contextMenuRef.current;
    if (!element || typeof window === "undefined") {
      return;
    }

    const margin = 8;
    const rect = element.getBoundingClientRect();
    const nextLeft = Math.max(
      margin,
      Math.min(contextMenu.left, window.innerWidth - rect.width - margin),
    );
    const nextTop = Math.max(
      margin,
      Math.min(contextMenu.top, window.innerHeight - rect.height - margin),
    );
    setContextMenu({
      ...contextMenu,
      left: nextLeft,
      positioned: true,
      top: nextTop,
    });
  }, [contextMenu]);

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

  function handleContextMenu(event: ReactMouseEvent<HTMLDivElement>, tab: MainTabSummary) {
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({
      left: event.clientX,
      positioned: false,
      tab,
      top: event.clientY,
    });
  }

  function closeTabsFromMenu(scope: MainTabCloseScope) {
    if (!contextMenu) {
      return;
    }

    const { tab } = contextMenu;
    setContextMenu(null);
    onCloseTabs(scope, tab);
  }

  function canCloseTab(tab: MainTabSummary) {
    return tab.type !== "chat" || !runningChatKeys.has(chatRunKey(tab.workspaceId, tab.chatId));
  }

  function hasClosableTabs(scope: MainTabCloseScope, anchorTab: MainTabSummary) {
    const anchorIndex = tabs.findIndex((tab) => mainTabKey(tab) === mainTabKey(anchorTab));
    if (anchorIndex < 0) {
      return false;
    }

    return tabs.some((tab, index) => {
      if (!canCloseTab(tab)) {
        return false;
      }
      if (scope === "current") {
        return index === anchorIndex;
      }
      if (scope === "others") {
        return index !== anchorIndex;
      }
      if (scope === "right") {
        return index > anchorIndex;
      }
      if (scope === "left") {
        return index < anchorIndex;
      }
      return true;
    });
  }

  const contextMenuItems: Array<{ label: string; scope: MainTabCloseScope }> = [
    { label: "Close current tab", scope: "current" },
    { label: "Close other tabs", scope: "others" },
    { label: "Close all tabs", scope: "all" },
    { label: "Close tabs to the right", scope: "right" },
    { label: "Close tabs to the left", scope: "left" },
  ];

  const contextMenuElement = contextMenu ? (
    <div
      aria-label={contextMenu.tab.title}
      className="workspace-chat-context-menu main-tab-context-menu"
      ref={contextMenuRef}
      role="menu"
      style={{
        left: contextMenu.left,
        top: contextMenu.top,
        visibility: contextMenu.positioned ? "visible" : "hidden",
      }}
    >
      {contextMenuItems.map((item) => (
        <button
          className="workspace-chat-context-menu-item"
          disabled={!hasClosableTabs(item.scope, contextMenu.tab)}
          key={item.scope}
          onClick={() => closeTabsFromMenu(item.scope)}
          role="menuitem"
          type="button"
        >
          <X aria-hidden="true" className="size-3.5" />
          <span>{t(item.label)}</span>
        </button>
      ))}
    </div>
  ) : null;

  return (
    <>
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
                onContextMenu={(event) => handleContextMenu(event, tab)}
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
      {contextMenuElement && typeof document !== "undefined"
        ? createPortal(contextMenuElement, document.body)
        : null}
    </>
  );
}

type NavRailAction = {
  active: boolean;
  disabled?: boolean;
  icon: (props: { className?: string; "aria-hidden"?: boolean | "true" | "false" }) => ReactNode;
  label: string;
  onClick: () => void;
};

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
  const trendData = summary.trend.map((point) => ({
    id: point.bucket,
    label: formatTrendBucket(point.bucket, language),
    primaryValue: point.requestCount,
    secondaryValue: point.totalTokens,
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
  const providerQualityData = summary.providerBreakdown.flatMap((item) => {
    if (item.averageLatencyMs === null || item.successRate === null) {
      return [];
    }

    return [
      {
        displayXValue: formatNullableLatencySeconds(item.averageLatencyMs, language),
        displayYValue: formatPercent(item.successRate, language),
        id: item.providerId,
        label: providerLabels.get(item.providerId) ?? item.providerId,
        x: item.averageLatencyMs,
        y: item.successRate,
      },
    ];
  });

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
    if (preferredWorkspaceId === "") {
      return;
    }

    setFilters((current) => ({
      ...current,
      workspaceId: preferredWorkspaceId,
    }));
    setHasAppliedInitialWorkspace(true);
  }, [preferredWorkspaceId]);

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
      <section className="foco-reticle rounded-2xl border border-stone-200 bg-white/85 px-5 py-5 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="inline-flex size-11 items-center justify-center rounded-xl bg-teal-50 text-teal-800 shadow-[inset_0_0_0_1px_rgba(200,101,27,0.18)]">
              <BarChart3 aria-hidden="true" className="size-5" />
            </span>
            <div className="min-w-0">
              <span className="foco-eyebrow">{t("API overview")}</span>
              <h2 className="foco-display mt-0.5 truncate text-2xl leading-tight text-stone-950">
                {selectedWorkspace?.name ?? t("All workspaces")}
              </h2>
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
            <RefreshCw
              aria-hidden="true"
              className="api-refresh-icon size-4"
              data-loading={isLoading ? "true" : "false"}
            />
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
        <Suspense fallback={<PanelLoadingFallback />}>
        <section className="grid gap-4 xl:grid-cols-2">
          <DualLineChartCard
            data={trendData}
            primaryFormatter={(value) => formatNumber(value, language)}
            primaryLabel={t("Requests")}
            secondaryFormatter={(value) => formatCompactNumber(value, language)}
            secondaryLabel={t("Tokens")}
            title={t("Requests and tokens trend")}
          />
          <DoubleDonutChartCard
            innerData={modelTokenData}
            innerFormatter={(value) => formatCompactNumber(value, language)}
            innerLabel={t("Tokens")}
            outerData={modelRequestData}
            outerFormatter={(value) => formatNumber(value, language)}
            outerLabel={t("Requests")}
            title={t("Model distribution")}
          />
          <DoubleDonutChartCard
            innerData={providerTokenData}
            innerFormatter={(value) => formatCompactNumber(value, language)}
            innerLabel={t("Tokens")}
            outerData={providerRequestData}
            outerFormatter={(value) => formatNumber(value, language)}
            outerLabel={t("Requests")}
            title={t("Channel distribution")}
          />
          <ScatterChartCard
            data={providerQualityData}
            title={t("Channel quality")}
            xFormatter={(value) => formatNullableLatencySeconds(value, language)}
            xLabel={t("Response time")}
            yFormatter={(value) => formatPercent(value, language)}
            yLabel={t("Success rate")}
          />
        </section>
        </Suspense>
      )}
    </div>
  );
}

const DualLineChartCard = lazy(() =>
  import("./features/stats/StatCharts").then((m) => ({ default: m.DualLineChartCard })),
);
const DoubleDonutChartCard = lazy(() =>
  import("./features/stats/StatCharts").then((m) => ({ default: m.DoubleDonutChartCard })),
);
const ScatterChartCard = lazy(() =>
  import("./features/stats/StatCharts").then((m) => ({ default: m.ScatterChartCard })),
);

function PanelLoadingFallback() {
  return (
    <div className="grid h-full w-full place-items-center p-8 text-stone-400">
      <LoaderCircle aria-hidden="true" className="size-6 animate-spin" />
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
      <div className="foco-eyebrow flex items-center gap-2">
        <Icon aria-hidden="true" className="size-4 text-teal-700" />
        <span>{label}</span>
      </div>
      <div className="foco-display mt-3 text-4xl leading-none text-stone-950">
        {value}
      </div>
    </article>
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
            <h1 className="foco-display text-2xl leading-none text-stone-950">Foco</h1>
            <p className="foco-eyebrow mt-1.5">
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

function contextCompressionBadge(
  kind: "rule" | "llm" | "runtimeToolState",
): ChatRunBadge {
  if (kind === "llm") {
    return "contextCompressionLlm";
  }
  if (kind === "runtimeToolState") {
    return "contextCompressionRuntime";
  }
  return "contextCompressionRule";
}

type StreamAttemptSnapshot = {
  content: string;
  reasoning: string | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
};

function emptyStreamingAttemptSnapshot(): StreamAttemptSnapshot {
  return {
    content: "",
    reasoning: null,
    toolCalls: [],
    parts: [],
  };
}

function streamingAttemptSnapshot(message: ShellMessage): StreamAttemptSnapshot {
  return {
    content: message.content,
    reasoning: message.reasoning,
    toolCalls: message.toolCalls,
    parts: message.parts,
  };
}

function resetStreamingAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "streamReset" }>,
  attemptSnapshot?: StreamAttemptSnapshot,
): ShellMessage {
  const toolCalls = streamEvent.toolCalls.map(normalizedToolCallSummary);
  if (attemptSnapshot) {
    return {
      ...addChatRunBadge(message, "llmReconnect"),
      content: attemptSnapshot.content,
      reasoning: attemptSnapshot.reasoning,
      toolCalls: attemptSnapshot.toolCalls,
      parts: attemptSnapshot.parts,
    };
  }
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
      runtimeToolStateSnapshotCount: 0,
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

function planPhaseRetryRefreshStillRunning(
  plans: Plan[],
  target: PendingPlanPhaseRetryRefresh,
) {
  const plan = plans.find((candidate) => candidate.id === target.planId);
  if (!plan) {
    return false;
  }
  const phase = plan.phases.find(
    (candidate) =>
      candidate.id === target.phaseId ||
      candidate.agentTaskId === target.agentTaskId,
  );
  return phase?.status === "running";
}

function samePlanPhaseRetryRefreshTarget(
  left: PendingPlanPhaseRetryRefresh,
  right: PendingPlanPhaseRetryRefresh,
) {
  return (
    left.workspaceId === right.workspaceId &&
    left.planId === right.planId &&
    left.phaseId === right.phaseId &&
    left.agentTaskId === right.agentTaskId
  );
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
    const kind =
      kindValue === "llm"
        ? "llm"
        : kindValue === "runtimeToolState" || kindValue === "runtime_tool_state"
          ? "runtimeToolState"
          : "rule";

    if (!assistantMessageId || (kind !== "runtimeToolState" && !snapshotId)) {
      return null;
    }

    return {
      type: "contextCompression",
      assistantMessageId,
      ...(snapshotId ? { snapshotId } : {}),
      kind,
    };
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

  if (value.type === "planRefresh" || value.type === "plan_refresh") {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");

    if (!workspaceId) {
      return null;
    }

    return { type: "planRefresh", workspaceId };
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
  const rawSessionMode = fieldValue(message, "sessionMode", "session_mode");
  const sessionMode: "plan" | null =
    rawSessionMode === "plan" ? "plan" : null;
  const queuedRun = normalizeQueuedMessageRunSummary(message.queuedRun);
  const normalizedMessage = {
    ...message,
    extractedMemories,
    metrics,
    memoriesUsed,
    pendingMode,
    queuedRun,
    runBadges: [],
    sessionMode,
    specUpdates,
    toolCalls,
    parts,
  };

  return {
    ...normalizedMessage,
    parts: parts.length ? parts : fallbackMessageParts(normalizedMessage),
  };
}

function normalizeChatMessagesPagination(
  value: ChatMessagesResponse["pagination"] | undefined,
): ChatMessagesPaginationState {
  if (!value || typeof value !== "object") {
    return { hasMoreBefore: false, nextBeforeSequence: null };
  }
  const hasMoreBefore = fieldValue(value, "hasMoreBefore", "has_more_before") === true;
  const nextBeforeSequence = fieldValue(
    value,
    "nextBeforeSequence",
    "next_before_sequence",
  );
  return {
    hasMoreBefore,
    nextBeforeSequence:
      typeof nextBeforeSequence === "number" ? nextBeforeSequence : null,
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
  const rawSessionMode = fieldValue(queuedRun, "sessionMode", "session_mode");
  const sessionMode: "plan" | null =
    rawSessionMode === "plan" ? "plan" : null;

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
    sessionMode,
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
