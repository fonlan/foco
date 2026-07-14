import focoLogoSvg from "../foco.svg?raw";
import {
  Activity,
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
  Server,
  Settings,
  ShoppingBag,
  SquareTerminal,
  SunMoon,
  Trash2,
  X,
} from "lucide-react";
import {
  CSSProperties,
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
  AiStatisticsSummary,
  AgentDefinitionInput,
  AgentDefinitionSettings,
  AgentDefinitionsResponse,
  AgentInstanceView,
  AgentTeamSnapshotResponse,
  AppLanguageId,
  AppThemeId,
  AiStatsFilterState,
  AuthStatusResponse,
  BrowserRoute,
  BrowserRouteChatTab,
  BrowserRouteFileTab,
  ChatAttachmentPartSummary,
  ChatAttachmentPayload,
  ChatCompressionStatistics,
  ChatContextCompressionDetail,
  ChatContextCompressionKind,
  ChatContextCompressionPart,
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
  FilePickerTarget,
  GenerateWorkspaceSpecResponse,
  GitBranchesResponse,
  GitCommitMessageResponse,
  GitDiffLineStats,
  GitDiffResponse,
  GitStatusFileSummary,
  EditChatUserMessageResponse,
  GitWorktreeSummary,
  HookNotificationSummary,
  InstallRipgrepResponse,
  JsonValue,
  LiveChatStatistics,
  MemoryFactRecord,
  MemoryListResponse,
  MemoryMutationResponse,
  UpdateModelRouteResponse,
  OpenChatTab,
  Plan,
  PlanAutoRunResponse,
  PlanResponse,
  PlanWorktreeAuditResponse,
  PlansResponse,
  PendingDeleteChat,
  PendingQuestionsResponse,
  QueueChatMessageResponse,
  QueuedMessageRunSummary,
  QuestionAnswerSubmission,
  QuestionItemSummary,
  QuestionOptionSummary,
  QuestionRequestSummary,
  RetryRunRequest,
  RemoteServerDiagnosticResponse,
  RemoteServerDiagnosticStage,
  RemoteServerResponse,
  ScheduledWorkspaceRun,
  SettingsResponse,
  SettingsSection,
  ShellMessage,
  TaskStatus,
  UpdateStatusSummary,
  TodoGraphResponse,
  TodoGraphTask,
  Translate,
  WorkspaceChatListItem,
  WorkspaceChatsResponse,
  WorkspaceFileChildrenResponse,
  WorkspaceFileContentResponse,
  WorkspaceFileSaveResponse,
  WorkspaceFilesResponse,
  WorkspaceFileTreeNode,
  WorkspaceIconDraft,
  WorkspaceSpecJobsResponse,
  WorkspaceSpecResponse,
  WorkspaceSummary,
  WorkspaceChatSearchResponse,
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
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
} from "./shared/thinking-levels";
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
import { FilePickerDialog, type FilePickerSelection } from "./features/file-picker/FilePickerDialog";
import { GitBranchDialog } from "./features/git/GitBranchDialog";
import { DeleteChatDialog } from "./features/chat/DeleteChatDialog";
import { ChatPanel, type ChatPanelHelpers } from "./features/chat/ChatPanel";
import { ModelRoutingPanel } from "./features/models/ModelRoutingPanel";
import {
  activeSkillQuery,
  chatAttachmentPayload,
  composerAttachmentFromSelectedFile,
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
  WorkspaceFileEditorPanel,
  type OpenFileTab,
  type WorkspaceFileEditorState,
} from "./features/files/WorkspaceFileEditorPanel";
const AgentsRuntimePanel = lazy(() =>
  import("./features/agents/AgentsRuntimePanel").then((m) => ({
    default: m.AgentsRuntimePanel,
  })),
);
import {
  ContextPanelSidebar,
  ResponsiveContextPanelIcon,
  type ContextPanelTab,
  type PlanPhaseRetryOverride,
} from "./features/context/ContextPanel";
const AgentTranscriptPanel = lazy(() =>
  import("./features/agents/AgentTranscriptPanel").then((m) => ({
    default: m.AgentTranscriptPanel,
  })),
);
import type { AgentTranscriptViewCacheEntry } from "./features/agents/AgentTranscriptPanel";

const SettingsPanel = lazy(() =>
  import("./features/settings/SettingsPanel").then((m) => ({
    default: m.SettingsPanel,
  })),
);
import { errorMessage, requestJson, responseErrorMessage } from "./shared/api-client";
import { installUpdateAndWaitForRestart } from "./shared/update-install";
const ScheduledTasksPage = lazy(() =>
  import("./features/scheduled-tasks/ScheduledTasksPage").then((m) => ({
    default: m.ScheduledTasksPage,
  })),
);
const SkillStorePage = lazy(() =>
  import("./features/skill-store/SkillStorePage").then((m) => ({
    default: m.SkillStorePage,
  })),
);

const PLAN_PHASE_RETRY_REFRESH_INTERVAL_MS = 3000;
const PLAN_AUTO_RUN_REFRESH_MS = 3000;
const REQUEST_STORM_DEDUPE_MS = 400;
const WORKSPACE_SPEC_JOB_POLL_DELAYS_MS = [
  1000,
  2000,
  4000,
  8000,
  15000,
  30000,
  45000,
  60000,
] as const;
const WORKSPACE_SPEC_JOB_STEADY_POLL_MS = 60000;

type SingleFlightEntry<T> = {
  promise: Promise<T>;
  queued?: boolean;
  settled: boolean;
  startedAtMs: number;
};

type WorkspaceSpecJobObserver = {
  cancelled: boolean;
  jobId: string;
  promise: Promise<void>;
};

function requestStormDedupeNow() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

function isDocumentVisible() {
  return typeof document === "undefined" || document.visibilityState === "visible";
}

function shouldReuseRequest<T>(
  entry: SingleFlightEntry<T> | undefined,
  nowMs: number,
  force = false,
) {
  if (!entry) {
    return false;
  }

  return !entry.settled || (!force && nowMs - entry.startedAtMs < REQUEST_STORM_DEDUPE_MS);
}

type ViewMode = BrowserRoute["viewMode"];
type PendingPlanPhaseRetryRefresh = {
  workspaceId: string;
  planId: string;
  phaseId: string;
};

function isAutoRunPlanInFlight(plan: Plan) {
  return (
    plan.status === "running" ||
    plan.phases.some(
      (phase) => phase.status === "queued" || phase.status === "running",
    )
  );
}

function isPlanOrderReorderable(plan: Plan) {
  return plan.status === "draft" || plan.status === "ready" || plan.status === "paused" || plan.status === "failed";
}

function activePlanOrderIds(plans: Plan[]) {
  return plans.filter(isPlanOrderReorderable).map((plan) => plan.id);
}

function reorderActivePlansByIds(plans: Plan[], planIds: string[]) {
  const reorderablePlans = plans.filter(isPlanOrderReorderable);
  if (reorderablePlans.length !== planIds.length) {
    return plans;
  }
  const plansById = new Map(reorderablePlans.map((plan) => [plan.id, plan]));
  const nextReorderablePlans = planIds
    .map((planId) => plansById.get(planId))
    .filter((plan): plan is Plan => Boolean(plan));
  if (nextReorderablePlans.length !== reorderablePlans.length) {
    return plans;
  }
  let nextIndex = 0;
  return plans.map((plan) => (isPlanOrderReorderable(plan) ? nextReorderablePlans[nextIndex++] ?? plan : plan));
}

export function trimInactiveChatMessageCaches(
  current: Record<string, ShellMessage[]>,
  accessOrder: string[],
  options: {
    activeChatKey: string | null;
    fullCacheLimit?: number;
    openChatKeys: Set<string>;
    pageLimit?: number;
    runningChatKeys: Set<string>;
  },
) {
  const pageLimit = options.pageLimit ?? CHAT_MESSAGES_PAGE_LIMIT;
  const fullCacheLimit = options.fullCacheLimit ?? INACTIVE_CHAT_FULL_CACHE_LIMIT;
  const inactiveChatKeys = accessOrder.filter(
    (chatKey) =>
      chatKey !== options.activeChatKey &&
      !options.runningChatKeys.has(chatKey) &&
      options.openChatKeys.has(chatKey) &&
      (current[chatKey]?.length ?? 0) > pageLimit,
  );
  if (inactiveChatKeys.length <= fullCacheLimit) {
    return { messagesByKey: current, trimmedChatKeys: [] as string[] };
  }

  const trimmedChatKeys = inactiveChatKeys.slice(
    0,
    inactiveChatKeys.length - fullCacheLimit,
  );
  let changed = false;
  const next = { ...current };
  for (const chatKey of trimmedChatKeys) {
    const cachedMessages = next[chatKey];
    if (!cachedMessages || cachedMessages.length <= pageLimit) {
      continue;
    }
    next[chatKey] = cachedMessages.slice(-pageLimit);
    changed = true;
  }
  return { messagesByKey: changed ? next : current, trimmedChatKeys };
}

export type MergeLoadedMessagesResult = {
  messages: ShellMessage[];
  preservedCachePrefix: boolean;
};

/**
 * Merge a freshly loaded latest page with the in-memory chat cache.
 * - When cache and loaded page share a stable message id, keep the cache prefix
 *   before that overlap and let the server page replace the overlap and suffix.
 * - When there is no overlap, drop unprovable cache history (edit rewrite / trim)
 *   and do not re-insert streaming bubbles from the discarded thread.
 * - When preserveStreamingPlaceholders is true and there is id continuity with
 *   the cache, re-insert streaming assistants the server has not yet returned.
 */
export function mergeLoadedMessagesWithStreamingPlaceholders(
  loadedMessages: ShellMessage[],
  cachedMessages: ShellMessage[],
  preserveStreamingPlaceholders: boolean,
): MergeLoadedMessagesResult {
  if (!cachedMessages.length) {
    return { messages: loadedMessages, preservedCachePrefix: false };
  }

  const cachedIndexById = new Map(
    cachedMessages.map((message, index) => [message.id, index]),
  );
  let cacheOverlapStart = -1;
  for (const message of loadedMessages) {
    const cachedIndex = cachedIndexById.get(message.id);
    if (cachedIndex !== undefined) {
      cacheOverlapStart = cachedIndex;
      break;
    }
  }

  const preservedPrefix =
    cacheOverlapStart > 0 ? cachedMessages.slice(0, cacheOverlapStart) : [];
  const preservedCachePrefix = preservedPrefix.length > 0;
  let nextMessages =
    preservedCachePrefix || cacheOverlapStart === 0
      ? [...preservedPrefix, ...loadedMessages]
      : [...loadedMessages];

  // No id continuity with cache: do not resurrect history or orphan streaming.
  if (!preserveStreamingPlaceholders || cacheOverlapStart < 0) {
    return { messages: nextMessages, preservedCachePrefix };
  }

  const loadedIds = new Set(nextMessages.map((message) => message.id));
  const placeholders = cachedMessages
    .map((message, index) => ({ index, message }))
    .filter(
      ({ message }) =>
        message.role === "assistant" &&
        message.status === "streaming" &&
        !loadedIds.has(message.id),
    );
  if (!placeholders.length) {
    return { messages: nextMessages, preservedCachePrefix };
  }

  for (const { index, message } of placeholders) {
    let anchor: ShellMessage | undefined;
    for (let anchorIndex = index - 1; anchorIndex >= 0; anchorIndex -= 1) {
      const candidate = cachedMessages[anchorIndex];
      if (candidate && nextMessages.some((item) => item.id === candidate.id)) {
        anchor = candidate;
        break;
      }
    }
    if (!anchor) {
      continue;
    }
    const anchorIndex = nextMessages.findIndex(
      (candidate) => candidate.id === anchor.id,
    );
    if (anchorIndex < 0) {
      continue;
    }
    const insertIndex = anchorIndex + 1;
    // ponytail: only preserves the live assistant bubble; if we later need
    // multi-placeholder replay ordering, use backend sequence numbers here.
    nextMessages = [
      ...nextMessages.slice(0, insertIndex),
      message,
      ...nextMessages.slice(insertIndex),
    ];
    loadedIds.add(message.id);
  }

  return { messages: nextMessages, preservedCachePrefix };
}

export function preserveCachedReasoningDurations(
  messages: ShellMessage[],
  cachedMessages: ShellMessage[],
): ShellMessage[] {
  if (!messages.length || !cachedMessages.length) {
    return messages;
  }

  const cachedMessagesById = new Map(
    cachedMessages.map((message) => [message.id, message]),
  );
  let changed = false;

  const nextMessages = messages.map((message) => {
    const cachedMessage = cachedMessagesById.get(message.id);
    if (!cachedMessage) {
      return message;
    }

    const cachedReasoningParts = cachedMessage.parts.filter(
      (part): part is Extract<ChatMessagePart, { type: "reasoning" }> =>
        part.type === "reasoning",
    );
    if (!cachedReasoningParts.length) {
      return message;
    }

    let reasoningIndex = 0;
    let partsChanged = false;
    const nextParts = message.parts.map((part) => {
      if (part.type !== "reasoning") {
        return part;
      }

      const cachedPart = cachedReasoningParts[reasoningIndex++];
      if (
        !cachedPart ||
        cachedPart.text !== part.text ||
        part.durationMs !== undefined ||
        part.liveDurationMs !== undefined
      ) {
        return part;
      }

      const durationPatch = {
        ...(cachedPart.durationMs !== undefined
          ? { durationMs: cachedPart.durationMs }
          : {}),
        ...(cachedPart.liveDurationMs !== undefined
          ? { liveDurationMs: cachedPart.liveDurationMs }
          : {}),
      };
      if (!Object.keys(durationPatch).length) {
        return part;
      }

      partsChanged = true;
      return { ...part, ...durationPatch };
    });

    if (!partsChanged) {
      return message;
    }

    changed = true;
    return { ...message, parts: nextParts };
  });

  return changed ? nextMessages : messages;
}

/** Stable UI expansion of assistant ordered parts that contain user interruptions. */
export function expandMessagesWithUserInterruptions(
  messages: ShellMessage[],
): ShellMessage[] {
  let changed = false;
  const expanded: ShellMessage[] = [];
  for (const message of messages) {
    if (
      message.role !== "assistant" ||
      !message.parts.some((part) => part.type === "userInterruption")
    ) {
      expanded.push(message);
      continue;
    }
    changed = true;
    expanded.push(...expandAssistantMessageWithInterruptions(message));
  }
  return changed ? expanded : messages;
}

function expandAssistantMessageWithInterruptions(
  message: ShellMessage,
): ShellMessage[] {
  const result: ShellMessage[] = [];
  let segmentParts: ChatMessagePart[] = [];
  let segmentId = message.id;

  const flushAssistantSegment = (
    metrics: ChatReplyMetrics | null | undefined,
    isLast: boolean,
  ) => {
    if (segmentParts.length === 0 && !isLast) {
      return;
    }
    // Skip trailing empty segment after the last interruption unless it is the
    // only way to place final metrics (prefer last non-empty when empty).
    if (segmentParts.length === 0 && isLast && result.length > 0) {
      const last = result[result.length - 1];
      if (last?.role === "assistant") {
        result[result.length - 1] = {
          ...last,
          metrics: message.metrics,
          memoriesUsed: message.memoriesUsed,
          extractedMemories: message.extractedMemories,
          specUpdates: message.specUpdates,
          status: message.status,
          runBadges: message.runBadges,
        };
        return;
      }
    }

    const toolCalls = segmentParts
      .filter(
        (part): part is Extract<ChatMessagePart, { type: "toolCall" }> =>
          part.type === "toolCall",
      )
      .map((part) => part.toolCall);
    const content = segmentParts
      .filter(
        (part): part is Extract<ChatMessagePart, { type: "text" | "error" }> =>
          part.type === "text" || part.type === "error",
      )
      .map((part) => part.text)
      .join("");
    const reasoningText = segmentParts
      .filter(
        (part): part is Extract<ChatMessagePart, { type: "reasoning" }> =>
          part.type === "reasoning",
      )
      .map((part) => part.text)
      .join("");

    result.push({
      ...message,
      id: segmentId,
      content,
      reasoning: reasoningText || null,
      parts: segmentParts,
      toolCalls,
      metrics: isLast ? message.metrics : metrics ?? null,
      memoriesUsed: isLast ? message.memoriesUsed : [],
      extractedMemories: isLast ? message.extractedMemories : [],
      specUpdates: isLast ? message.specUpdates : [],
      status: isLast ? message.status : undefined,
      runBadges: isLast ? message.runBadges : undefined,
      syntheticSource: undefined,
    });
  };

  for (const part of message.parts) {
    if (part.type === "userInterruption") {
      flushAssistantSegment(part.interruptedAssistantMetrics ?? null, false);
      result.push({
        id: part.id,
        role: "user",
        content: part.content,
        createdAt: message.createdAt,
        reasoning: null,
        status: undefined,
        sessionMode: undefined,
        runConfig: undefined,
        pendingMode: undefined,
        queuedRun: null,
        toolCalls: [],
        parts: [{ type: "text", text: part.content }],
        metrics: null,
        memoriesUsed: [],
        extractedMemories: [],
        specUpdates: [],
        syntheticSource: part.source ?? "userInterruption",
      });
      segmentParts = [];
      segmentId = `${part.id}-assistant`;
      continue;
    }
    segmentParts = [...segmentParts, part];
  }

  flushAssistantSegment(null, true);
  return result.length > 0 ? result : [message];
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

type FilePickerRequest = {
  initialPath?: string | null;
  mode: "file" | "directory";
  multiple?: boolean;
  readFiles?: boolean;
  target: FilePickerTarget;
  title: string;
  onSelect: (selection: FilePickerSelection[]) => void;
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

type ChatSessionStatusKind = "idle" | "open" | "scheduled" | "running" | "failed";

type ChatSessionStatus = {
  activeRun: ActiveRunInfo | ActiveChatRunSummary | null;
  kind: ChatSessionStatusKind;
};

type ChatSessionStatusInput = {
  activeChatKey: string | null;
  activeRunInfoByChatKey: Record<string, ActiveRunInfo>;
  chatKey: string;
  failedChatKeySet: Set<string>;
  openChatKeySet: Set<string>;
  runningChatKeys: Set<string>;
  scheduledChatKey?: string | null;
  scheduledStatus?: ScheduledWorkspaceRun["status"] | null;
  workspaceActiveRun?: ActiveChatRunSummary | null;
};

export function deriveChatSessionStatus({
  activeChatKey,
  activeRunInfoByChatKey,
  chatKey,
  failedChatKeySet,
  openChatKeySet,
  runningChatKeys,
  scheduledChatKey = null,
  scheduledStatus = null,
  workspaceActiveRun = null,
}: ChatSessionStatusInput): ChatSessionStatus {
  const statusChatKeys = scheduledChatKey && scheduledChatKey !== chatKey
    ? [chatKey, scheduledChatKey]
    : [chatKey];
  const activeRun =
    statusChatKeys
      .map((statusChatKey) => activeRunInfoByChatKey[statusChatKey] ?? null)
      .find((runInfo): runInfo is ActiveRunInfo => runInfo !== null) ??
    workspaceActiveRun;
  const isRunning =
    statusChatKeys.some((statusChatKey) => runningChatKeys.has(statusChatKey)) ||
    workspaceActiveRun !== null;
  const isScheduled = scheduledStatus === "queued" || scheduledStatus === "starting";
  const isOpen = statusChatKeys.some(
    (statusChatKey) => openChatKeySet.has(statusChatKey) || activeChatKey === statusChatKey,
  );
  const isFailed = isOpen && failedChatKeySet.has(chatKey);

  if (isRunning) {
    return { activeRun, kind: "running" };
  }
  if (isScheduled) {
    return { activeRun, kind: "scheduled" };
  }
  if (isFailed) {
    return { activeRun, kind: "failed" };
  }
  if (isOpen) {
    return { activeRun, kind: "open" };
  }
  return { activeRun, kind: "idle" };
}

export function chatSessionStatusDotClass(kind: ChatSessionStatusKind) {
  switch (kind) {
    case "running":
      return "session-status-dot-running";
    case "scheduled":
      return "session-status-dot-scheduled";
    case "failed":
      return "session-status-dot-error";
    case "open":
      return "session-status-dot-open";
    case "idle":
      return "session-status-dot-idle";
  }
}

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

const LIVE_REASONING_DURATION_REFRESH_MS = 1000;
const LIVE_CONTEXT_USAGE_REFRESH_MS = 5000;
const AGENT_TEAM_RUNNING_REFRESH_MS = 1000;
const CHAT_MESSAGES_PAGE_LIMIT = 100;
const INACTIVE_CHAT_FULL_CACHE_LIMIT = 8;
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-default";
const EMPTY_CONFIGURED_PROVIDERS: ConfiguredProviderSummary[] = [];
const EMPTY_GIT_STATUS_FILES: GitStatusFileSummary[] = [];

type SourceControlTarget = {
  kind: "workspace" | "worktree";
  path: string | null;
  label: string;
};

function sourceControlTargetKey(target: SourceControlTarget | null) {
  return target?.kind === "worktree" && target.path
    ? `worktree:${target.path}`
    : "workspace";
}

function pathBasename(path: string) {
  const normalized = path.replace(/[\\/]+$/, "").replace(/\\/g, "/");
  return normalized.split("/").filter(Boolean).at(-1) ?? normalized;
}

function remoteWorkspacePathBasename(path: string) {
  const normalized = path.replace(/\/+$/, "");
  return normalized.split("/").filter(Boolean).at(-1) ?? normalized;
}

function remoteWorkspacePendingStages(t: Translate): RemoteServerDiagnosticStage[] {
  return [
    { details: null, errorKind: null, message: t("Checking SSH"), stage: "ssh", status: "running" },
    { details: null, errorKind: null, message: t("Detecting target"), stage: "target", status: "pending" },
    { details: null, errorKind: null, message: t("Installing sidecar"), stage: "sidecarAsset", status: "pending" },
    {
      details: null,
      errorKind: null,
      message: t("Starting sidecar"),
      stage: "remoteInstallDirWritable",
      status: "pending",
    },
    {
      details: null,
      errorKind: null,
      message: t("Checking Sidecar version"),
      stage: "focoCommandVersion",
      status: "pending",
    },
  ];
}

function sourceControlLabelForWorktree(worktree: Pick<GitWorktreeSummary, "branch" | "name">) {
  return worktree.branch ?? worktree.name;
}

function worktreeMatchesExecutionRoot(worktreePath: string, executionRootPath: string) {
  const normalize = (path: string) => path.replace(/\\/g, "/").replace(/\/+$/, "");
  const left = normalize(worktreePath);
  const right = normalize(executionRootPath);
  return left === right || left.endsWith(`/${right}`) || right.endsWith(`/${left}`);
}

function sourceControlDefaultTarget(
  workspacePath: string | null | undefined,
  gitBranches: GitBranchesResponse | null,
  coordinatorInstance: AgentInstanceView | null,
): SourceControlTarget | null {
  const workspaceTarget: SourceControlTarget = {
    kind: "workspace",
    label: gitBranches?.currentBranch ?? (workspacePath ? pathBasename(workspacePath) : "Workspace"),
    path: null,
  };

  if (
    coordinatorInstance?.executionWorkspaceMode !== "isolated_worktree" ||
    coordinatorInstance.worktreeStatus === "deleted"
  ) {
    return workspaceTarget;
  }

  const byPath = coordinatorInstance.executionRootPath
    ? gitBranches?.worktrees.find((worktree) =>
        worktreeMatchesExecutionRoot(worktree.path, coordinatorInstance.executionRootPath!),
      ) ?? null
    : null;
  const byBranch = coordinatorInstance.worktreeBranch
    ? gitBranches?.worktrees.find(
        (worktree) => worktree.branch === coordinatorInstance.worktreeBranch,
      ) ?? null
    : null;
  const worktree = byPath ?? byBranch;
  if (worktree) {
    return {
      kind: "worktree",
      label: sourceControlLabelForWorktree(worktree),
      path: worktree.path,
    };
  }
  if (coordinatorInstance.executionRootPath) {
    return {
      kind: "worktree",
      label: coordinatorInstance.worktreeBranch ?? pathBasename(coordinatorInstance.executionRootPath),
      path: coordinatorInstance.executionRootPath,
    };
  }

  return workspaceTarget;
}

function sourceControlTargets(
  workspacePath: string | null | undefined,
  gitBranches: GitBranchesResponse | null,
): SourceControlTarget[] {
  const targets: SourceControlTarget[] = [
    {
      kind: "workspace",
      label: gitBranches?.currentBranch ?? (workspacePath ? pathBasename(workspacePath) : "Workspace"),
      path: null,
    },
  ];

  for (const worktree of gitBranches?.worktrees ?? []) {
    if (workspacePath && worktreeMatchesExecutionRoot(worktree.path, workspacePath)) {
      continue;
    }
    targets.push({
      kind: "worktree",
      label: sourceControlLabelForWorktree(worktree),
      path: worktree.path,
    });
  }

  return targets;
}

function sourceControlTargetFromKey(
  targets: SourceControlTarget[],
  key: string,
) {
  return targets.find((target) => sourceControlTargetKey(target) === key) ?? null;
}

function appendGitTargetParams(
  params: URLSearchParams,
  target: SourceControlTarget | null,
) {
  if (target?.kind === "worktree" && target.path) {
    params.set("worktreePath", target.path);
  }
}

function gitTargetRequestBody<T extends Record<string, unknown>>(
  body: T,
  target: SourceControlTarget | null,
) {
  return target?.kind === "worktree" && target.path
    ? { ...body, worktreePath: target.path }
    : body;
}

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

async function workspaceSummariesWithRemoteChats(
  workspaces: WorkspaceSummary[],
): Promise<WorkspaceSummary[]> {
  return Promise.all(
    workspaces.map(async (workspace) => {
      if (!workspace.serverId) {
        return workspace;
      }
      try {
        const params = new URLSearchParams({ limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE) });
        const data = await requestJson<WorkspaceChatsResponse>(
          `/api/workspaces/${encodeURIComponent(workspace.id)}/chats?${params.toString()}`,
        );
        return {
          ...workspace,
          chatPagination: {
            hasMore: data.hasMore,
            limit: data.limit,
            nextCursor: data.nextCursor,
            total: data.total,
          },
          chats: data.chats,
        };
      } catch {
        return workspace;
      }
    }),
  );
}

function workspaceChatPagingFromWorkspaces(
  workspaces: WorkspaceSummary[],
): WorkspaceChatPagingState {
  return Object.fromEntries(
    workspaces.map((workspace) => {
      const pagination = workspace.chatPagination ?? {
        hasMore: false,
        nextCursor: null,
        total: workspace.chats.length,
      };
      return [
        workspace.id,
        {
          hasMore: pagination.hasMore,
          isLoading: false,
          nextCursor: pagination.nextCursor,
          total: pagination.total,
        },
      ];
    }),
  );
}

function isAbortError(value: unknown) {
  return (
    typeof value === "object" &&
    value !== null &&
    "name" in value &&
    value.name === "AbortError"
  );
}

type WorkspaceChatPagingState = Record<
  string,
  {
    hasMore: boolean;
    isLoading: boolean;
    nextCursor: string | null;
    total: number;
  }
>;

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
  const [workspaceChatPaging, setWorkspaceChatPaging] =
    useState<WorkspaceChatPagingState>({});
  const [workspaceChatSearchOpen, setWorkspaceChatSearchOpen] = useState(false);
  const [workspaceChatSearchQuery, setWorkspaceChatSearchQuery] = useState("");
  const [workspaceChatSearchResults, setWorkspaceChatSearchResults] = useState<
    WorkspaceSummary[]
  >([]);
  const [isSearchingWorkspaceChats, setIsSearchingWorkspaceChats] =
    useState(false);
  const [workspaceChatSearchError, setWorkspaceChatSearchError] = useState<
    string | null
  >(null);
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
  const [statsRouteFilters, setStatsRouteFilters] = useState<
    Partial<AiStatsFilterState>
  >(initialBrowserRoute.viewMode === "stats" ? initialBrowserRoute.filters ?? {} : {});
  const statsRouteFiltersRef = useRef(statsRouteFilters);
  statsRouteFiltersRef.current = statsRouteFilters;
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceDialogRevision, setWorkspaceDialogRevision] = useState(0);
  const [workspaceMode, setWorkspaceMode] = useState<"local" | "ssh">("local");
  const [workspaceServerId, setWorkspaceServerId] = useState("");
  const [workspaceTestStages, setWorkspaceTestStages] = useState<
    RemoteServerDiagnosticResponse["result"]["stages"]
  >([]);
  const [isTestingWorkspaceConnection, setIsTestingWorkspaceConnection] = useState(false);
  const [inlineRemoteServerName, setInlineRemoteServerName] = useState("");
  const [inlineRemoteServerHost, setInlineRemoteServerHost] = useState("");
  const [isCreatingInlineRemoteServer, setIsCreatingInlineRemoteServer] = useState(false);
  const [retryingRemoteWorkspaceId, setRetryingRemoteWorkspaceId] = useState<string | null>(null);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [workspaceTerminalShell, setWorkspaceTerminalShell] = useState("");
  const [workspaceSpecEnabled, setWorkspaceSpecEnabled] = useState(false);
  const [workspaceIconDraft, setWorkspaceIconDraft] =
    useState<WorkspaceIconDraft | null>(null);
  const [filePickerRequest, setFilePickerRequest] = useState<FilePickerRequest | null>(null);
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
  const openAgentTabsRef = useRef<OpenAgentTab[]>([]);
  openAgentTabsRef.current = openAgentTabs;
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
  // ponytail: keep inactive chat cache ref-only so hot streaming paths don't
  // rerender App; ceiling is App still owns too much chat state, upgrade path is
  // moving this cache into a dedicated hook/store.
  const chatMessagesByKeyRef = useRef<Record<string, ShellMessage[]>>({});
  const cachedChatAccessOrderRef = useRef<string[]>([]);
  const trimmedChatCacheKeysRef = useRef<Set<string>>(new Set());
  function rememberChatCacheAccess(chatKey: string) {
    cachedChatAccessOrderRef.current = [
      ...cachedChatAccessOrderRef.current.filter((key) => key !== chatKey),
      chatKey,
    ];
  }

  function trimInactiveChatCaches() {
    const { messagesByKey, trimmedChatKeys } = trimInactiveChatMessageCaches(
      chatMessagesByKeyRef.current,
      cachedChatAccessOrderRef.current,
      {
        activeChatKey: activeChatKeyRef.current,
        openChatKeys: new Set(
          openChatTabsRef.current.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
        ),
        runningChatKeys: runningChatKeysRef.current,
      },
    );
    if (!trimmedChatKeys.length) {
      return;
    }

    // ponytail: cap only inactive, non-running chats and keep the newest page;
    // if offline tab switching must preserve full history, move this to an LRU store.
    setChatMessagesByKey(messagesByKey);
    trimmedChatCacheKeysRef.current = new Set([
      ...trimmedChatCacheKeysRef.current,
      ...trimmedChatKeys,
    ]);
  }

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
    cachedChatAccessOrderRef.current = cachedChatAccessOrderRef.current.filter(
      (chatKey) => chatKey in next,
    );
    trimmedChatCacheKeysRef.current = new Set(
      [...trimmedChatCacheKeysRef.current].filter((chatKey) => chatKey in next),
    );
  }
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
  }
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatusSummary | null>(null);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateInstallNotice, setUpdateInstallNotice] = useState<string | null>(null);
  const [agentDefinitions, setAgentDefinitions] = useState<AgentDefinitionSettings[]>([]);
  const [defaultAgentRolePrompts, setDefaultAgentRolePrompts] = useState<Record<string, string>>({});
  const [isTeamModeEnabled, setIsTeamModeEnabled] = useState(false);
  const [isPlanModeEnabled, setIsPlanModeEnabled] = useState(false);
  const planModeByChatKeyRef = useRef<Record<string, boolean>>({});
  const [isLoadingAgentDefinitions, setIsLoadingAgentDefinitions] = useState(false);
  const [agentDefinitionsError, setAgentDefinitionsError] = useState<string | null>(null);
  const [agentDefinitionOperationKey, setAgentDefinitionOperationKey] = useState<string | null>(null);
  const [agentTeamSnapshot, setAgentTeamSnapshot] = useState<AgentTeamSnapshotResponse | null>(null);
  const agentTeamSnapshotChatKeyRef = useRef<string | null>(null);
  const agentTeamSnapshotCacheRef = useRef(new Map<string, AgentTeamSnapshotResponse>());
  const agentTranscriptViewCacheRef = useRef(new Map<string, AgentTranscriptViewCacheEntry>());
  const [isLoadingAgentTeam, setIsLoadingAgentTeam] = useState(false);
  const [agentTeamError, setAgentTeamError] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
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
  const [selectedSourceControlTarget, setSelectedSourceControlTarget] =
    useState<SourceControlTarget | null>(null);
  const [selectedSourceControlTargetScope, setSelectedSourceControlTargetScope] = useState("");
  const [isSourceControlTargetManual, setIsSourceControlTargetManual] = useState(false);
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
  const [loadedActivePlansWorkspaceId, setLoadedActivePlansWorkspaceId] =
    useState<string | null>(null);
  const [isLoadingActivePlans, setIsLoadingActivePlans] = useState(false);
  const [activePlansError, setActivePlansError] = useState<string | null>(null);
  const [planOperationKey, setPlanOperationKey] = useState<string | null>(null);
  const [planAutoRunByWorkspace, setPlanAutoRunByWorkspace] =
    useState<Record<string, PlanAutoRunResponse>>({});
  const [isPlanAutoRunUpdating, setIsPlanAutoRunUpdating] = useState(false);
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
  const activePlansSingleFlightRef = useRef<
    Map<string, SingleFlightEntry<PlansResponse | null>>
  >(new Map());
  const planAutoRunSingleFlightRef = useRef<
    Map<string, SingleFlightEntry<PlanAutoRunResponse | null>>
  >(new Map());
  const chatStatisticsSingleFlightRef = useRef<
    Map<string, SingleFlightEntry<void>>
  >(new Map());
  const chatStatisticsRequestIdByChatKeyRef = useRef<Map<string, number>>(
    new Map(),
  );
  const workspaceSpecJobObserversRef = useRef<Map<string, WorkspaceSpecJobObserver>>(
    new Map(),
  );
  const gitBranchesRequestRef = useRef<AbortController | null>(null);
  const gitBranchesRequestIdRef = useRef(0);
  const selectedModelIdRef = useRef("");
  const selectedThinkingLevelRef = useRef("");
  const activeChatKeyRef = useRef<string | null>(null);
  const activeWorkspaceIdRef = useRef("");
  const activeChatIdRef = useRef<string | null>(null);
  const loadingChatKeysRef = useRef<Set<string>>(new Set());
  const loadingChatControllersRef = useRef<Map<string, AbortController>>(new Map());
  const loadingOlderChatMessageKeysRef = useRef<Set<string>>(new Set());
  const runningChatKeysRef = useRef<Set<string>>(new Set());
  const restoredPendingQuestionIdsRef = useRef<Set<string>>(new Set());
  const isCheckingPendingQuestionsRef = useRef(false);
  const activeRunInfoByChatKeyRef = useRef<Record<string, ActiveRunInfo>>({});
  const queuedRunRequestsByChatKeyRef = useRef<
    Record<string, RetryRunRequest[]>
  >({});
  const scheduledWorkspaceRunsRef = useRef<ScheduledWorkspaceRun[]>([]);
  const failedRestoredQueuedRunKeysRef = useRef<Set<string>>(new Set());
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
  const activeWorkspaceIdForPlanAutoRun = activeWorkspace?.id ?? "";
  const planAutoRunState = activeWorkspaceIdForPlanAutoRun
    ? planAutoRunByWorkspace[activeWorkspaceIdForPlanAutoRun] ?? null
    : null;
  const isPlanAutoRunEnabled =
    planAutoRunState?.desiredEnabled ?? planAutoRunState?.enabled ?? false;
  const isPlanAutoRunBusy = planAutoRunState?.busy ?? false;
  const planAutoRunBlockedReason = planAutoRunState?.blockedReason ?? null;
  const setPlanAutoRunStateForWorkspace = useCallback(
    (workspaceId: string, autoRun: PlanAutoRunResponse) => {
      if (!workspaceId) {
        return;
      }
      setPlanAutoRunByWorkspace((current) => ({
        ...current,
        [workspaceId]: autoRun,
      }));
    },
    [],
  );
  const loadPlanAutoRunState = useCallback(
    (workspaceId: string, options: { force?: boolean } = {}) => {
      if (!workspaceId) {
        return Promise.resolve(null);
      }

      const nowMs = requestStormDedupeNow();
      const existing = planAutoRunSingleFlightRef.current.get(workspaceId);
      if (shouldReuseRequest(existing, nowMs, options.force)) {
        return existing!.promise;
      }

      let promise: Promise<PlanAutoRunResponse | null> = Promise.resolve(null);
      promise = (async () => {
        try {
          const autoRun = await requestJson<PlanAutoRunResponse>(
            `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/auto-run`,
          );
          if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
            return null;
          }
          setPlanAutoRunStateForWorkspace(workspaceId, autoRun);
          return autoRun;
        } catch (requestError) {
          if (!activeWorkspaceIdRef.current || activeWorkspaceIdRef.current === workspaceId) {
            setActivePlansError(errorMessage(requestError));
          }
          return null;
        } finally {
          const current = planAutoRunSingleFlightRef.current.get(workspaceId);
          if (current?.promise === promise) {
            current.settled = true;
            window.setTimeout(() => {
              if (planAutoRunSingleFlightRef.current.get(workspaceId)?.promise === promise) {
                planAutoRunSingleFlightRef.current.delete(workspaceId);
              }
            }, REQUEST_STORM_DEDUPE_MS);
          }
        }
      })();
      // ponytail: per-tab single-flight only; cross-tab leader can use BroadcastChannel later.
      planAutoRunSingleFlightRef.current.set(workspaceId, {
        promise,
        settled: false,
        startedAtMs: nowMs,
      });
      return promise;
    },
    [setPlanAutoRunStateForWorkspace],
  );
  const setPlanAutoRunEnabledForWorkspace = useCallback(
    async (workspaceId: string, enabled: boolean) => {
      if (!workspaceId) {
        return;
      }
      setIsPlanAutoRunUpdating(true);
      setActivePlansError(null);
      try {
        const autoRun = await requestJson<PlanAutoRunResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/auto-run`,
          {
            body: JSON.stringify({ enabled }),
            headers: { "Content-Type": "application/json" },
            method: "PUT",
          },
        );
        setPlanAutoRunStateForWorkspace(workspaceId, autoRun);
      } catch (requestError) {
        setActivePlansError(errorMessage(requestError));
      } finally {
        setIsPlanAutoRunUpdating(false);
      }
    },
    [setPlanAutoRunStateForWorkspace],
  );
  const setIsPlanAutoRunEnabled = useCallback(
    (enabled: boolean) => {
      void setPlanAutoRunEnabledForWorkspace(
        activeWorkspaceIdForPlanAutoRun,
        enabled,
      );
    },
    [activeWorkspaceIdForPlanAutoRun, setPlanAutoRunEnabledForWorkspace],
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
  const latestProviderUsage =
    activeChatKey !== null && runningChatKeys.has(activeChatKey)
      ? liveChatStatistics?.usage ?? null
      : null;
  const displayedContextUsage = contextUsage
    ? contextUsageWithLatestProviderUsage(contextUsage, latestProviderUsage)
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
  const availableSourceControlTargets = useMemo(
    () => sourceControlTargets(activeWorkspace?.path, gitBranches),
    [activeWorkspace?.path, gitBranches],
  );
  const defaultSourceControlTarget = useMemo(
    () =>
      sourceControlDefaultTarget(
        activeWorkspace?.path,
        gitBranches,
        activeChatCoordinatorInstance,
      ),
    [activeChatCoordinatorInstance, activeWorkspace?.path, gitBranches],
  );
  const sourceControlTargetScope = activeWorkspace?.id && activeChatId
    ? `${activeWorkspace.id}:${activeChatId}`
    : "";
  const sourceControlTarget =
    isSourceControlTargetManual && selectedSourceControlTargetScope === sourceControlTargetScope
      ? selectedSourceControlTarget ?? defaultSourceControlTarget
      : defaultSourceControlTarget;
  const sourceControlTargetKeyValue = sourceControlTargetKey(sourceControlTarget);
  const isLoadingContextUsage = activeContextUsageKey
    ? contextUsageLoadingByChatKey[activeContextUsageKey] ?? false
    : false;
  const openChatKeySet = useMemo(
    () =>
      new Set(
        openChatTabs.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
      ),
    [openChatTabs],
  );
  const chatSessionStatusFor = useCallback(
    (
      chatKey: string,
      options: {
        scheduledChatKey?: string | null;
        scheduledStatus?: ScheduledWorkspaceRun["status"] | null;
        workspaceActiveRun?: ActiveChatRunSummary | null;
      } = {},
    ) =>
      deriveChatSessionStatus({
        activeChatKey,
        activeRunInfoByChatKey,
        chatKey,
        failedChatKeySet,
        openChatKeySet,
        runningChatKeys,
        scheduledChatKey: options.scheduledChatKey,
        scheduledStatus: options.scheduledStatus,
        workspaceActiveRun: options.workspaceActiveRun ?? null,
      }),
    [
      activeChatKey,
      activeRunInfoByChatKey,
      failedChatKeySet,
      openChatKeySet,
      runningChatKeys,
    ],
  );
  const activeChatSessionStatus = activeChatKey
    ? chatSessionStatusFor(activeChatKey)
    : { activeRun: null, kind: "idle" as const };
  const activeRunInfo = activeChatKey
    ? activeRunInfoByChatKey[activeChatKey] ?? null
    : null;
  const activeChatReadOnly = activeChatKey
    ? readOnlyChatKeys[activeChatKey] === true
    : false;
  const canUseTeamMode = agentDefinitions.length > 1;
  const isSendingMessage = activeChatSessionStatus.kind === "running";
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

  const configuredModelsByName = useMemo(
    () =>
      [...(settings?.configuredModels ?? [])].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    [settings?.configuredModels],
  );
  const enabledProviderIds = useMemo(
    () =>
      new Set(
        (settings?.providers ?? [])
          .filter((provider) => provider.enabled)
          .map((provider) => provider.id),
      ),
    [settings?.providers],
  );
  const availableModels = useMemo(
    () =>
      configuredModelsByName.flatMap((model) => {
        const providerIds = model.providerIds.filter((providerId) =>
          enabledProviderIds.has(providerId),
        );
        if (
          !model.enabled ||
          !model.canEnable ||
          model.activeProviderId === null ||
          providerIds.length === 0
        ) {
          return [];
        }

        return [
          {
            ...model,
            activeProviderId: providerIds.includes(model.activeProviderId)
              ? model.activeProviderId
              : providerIds[0],
            providerIds,
          },
        ];
      }),
    [configuredModelsByName, enabledProviderIds],
  );
  const selectedModel = useMemo(
    () => availableModels.find((model) => model.id === selectedModelId) ?? null,
    [availableModels, selectedModelId],
  );
  const selectedProviderId = useMemo(() => {
    if (!selectedModel?.providerIds.length) {
      return "";
    }
    if (
      selectedModel.activeProviderId &&
      selectedModel.providerIds.includes(selectedModel.activeProviderId)
    ) {
      return selectedModel.activeProviderId;
    }
    return selectedModel.providerIds[0] ?? "";
  }, [selectedModel]);
  const selectedProviderIdRef = useRef(selectedProviderId);
  selectedProviderIdRef.current = selectedProviderId;
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
        (model) => model.id === defaultAgentDefinition.modelId,
      );
      if (agentModel) {
        const providerId =
          agentModel.activeProviderId &&
          agentModel.providerIds.includes(agentModel.activeProviderId)
            ? agentModel.activeProviderId
            : agentModel.providerIds[0] ?? "";
        return {
          modelId: agentModel.id,
          providerId,
          thinkingLevel: isModelThinkingLevelSupported(
            agentModel,
            defaultAgentDefinition.modelOptions.thinkingLevel,
          )
            ? defaultAgentDefinition.modelOptions.thinkingLevel!
            : "",
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
      thinkingLevel: defaultThinkingLevelForModel(model),
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
  const selectedRequestThinkingLevel = isModelThinkingLevelSupported(
    selectedModel,
    selectedThinkingLevel,
  )
    ? selectedThinkingLevel
    : "";
  const isTerminalOpen = activeWorkspace
    ? terminalOpenWorkspaceIds.has(activeWorkspace.id)
    : false;
  const isGlobalView =
    viewMode === "settings" ||
    viewMode === "stats" ||
    viewMode === "scheduled" ||
    viewMode === "skill-store";
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
  const unsupportedDraftAttachmentMessage = unsupportedDraftAttachment
    ? unsupportedAttachmentMessage(selectedModel, unsupportedDraftAttachment, t)
    : null;

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
    for (const [workspaceId, observer] of workspaceSpecJobObserversRef.current) {
      if (workspaceId !== activeWorkspaceId) {
        observer.cancelled = true;
        workspaceSpecJobObserversRef.current.delete(workspaceId);
      }
    }
    activeChatIdRef.current = activeChatId;
    activeChatKeyRef.current =
      activeChatId === null || isPendingChatId(activeChatId)
        ? activeChatKeyRef.current
        : chatRunKey(activeWorkspaceId, activeChatId);
  }, [activeChatId, activeWorkspaceId]);

  useEffect(
    () => () => {
      for (const observer of workspaceSpecJobObserversRef.current.values()) {
        observer.cancelled = true;
      }
      workspaceSpecJobObserversRef.current.clear();
    },
    [],
  );

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
      selectedRequestThinkingLevel,
      ...selectedSkillIds,
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
    if (selectedModelId && selectedProviderId) {
      void refreshContextUsage({
        chatId: activeChatId,
        modelId: selectedModelId,
        providerId: selectedProviderId,
        skillIds: selectedSkillIds,
        thinkingLevel: selectedRequestThinkingLevel,
        workspaceId: activeWorkspaceId,
      });
    }
  }, [
    activeChatId,
    activeWorkspaceId,
    isSendingMessage,
    selectedModelId,
    selectedProviderId,
    selectedSkillIds,
    selectedRequestThinkingLevel,
  ]);

  useLayoutEffect(() => {
    selectedModelIdRef.current = selectedModelId;
  }, [selectedModelId]);

  useLayoutEffect(() => {
    selectedThinkingLevelRef.current = isModelThinkingLevelSupported(
      selectedModel,
      selectedRequestThinkingLevel,
    )
      ? selectedThinkingLevel
      : "";
  }, [selectedModel, selectedThinkingLevel]);

  useEffect(
    () => () => {
      for (const abortController of contextUsageAbortByChatKeyRef.current.values()) {
        abortController.abort();
      }
      contextUsageAbortByChatKeyRef.current.clear();
    },
    [],
  );

  useEffect(
    () => () => {
      for (const abortController of activeRunAbortByChatKeyRef.current.values()) {
        abortController.abort();
      }
      activeRunAbortByChatKeyRef.current.clear();

      for (const abortController of loadingChatControllersRef.current.values()) {
        abortController.abort();
      }
      loadingChatControllersRef.current.clear();
      loadingChatKeysRef.current.clear();
    },
    [],
  );

  useEffect(() => {
    if (!canUseApp) {
      return undefined;
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        recoverActiveChatStreams("visible");
      }
    };
    const handleOnline = () => recoverActiveChatStreams("online");

    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("online", handleOnline);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("online", handleOnline);
    };
  }, [canUseApp]);

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

  const checkPendingQuestions = useStableCallback(
    async (visibleWorkspaces: WorkspaceSummary[]) => {
      if (isCheckingPendingQuestionsRef.current) {
        return;
      }

      isCheckingPendingQuestionsRef.current = true;
      try {
        const data = await requestJson<PendingQuestionsResponse>(
          "/api/chat/questions/pending",
        );
        const question = data.questions
          .map(parseQuestionRequestSummary)
          .find(
            (candidate): candidate is QuestionRequestSummary =>
              candidate !== null &&
              !restoredPendingQuestionIdsRef.current.has(candidate.id),
          );

        if (!question) {
          return;
        }

        restoredPendingQuestionIdsRef.current.add(question.id);
        const workspace = visibleWorkspaces.find(
          (candidate) => candidate.id === question.workspaceId,
        );
        const chat = workspace?.chats.find(
          (candidate) => candidate.id === question.chatId,
        );

        if (!workspace || !chat) {
          setError(
            t("Pending question chat is no longer available: {workspaceId}/{chatId}", {
              chatId: question.chatId,
              workspaceId: question.workspaceId,
            }),
          );
          return;
        }

        if (
          activeWorkspaceIdRef.current === question.workspaceId &&
          activeChatIdRef.current === question.chatId
        ) {
          void loadChatMessages(question.workspaceId, question.chatId);
          return;
        }

        selectWorkspaceChat(question.workspaceId, question.chatId);
      } catch (requestError) {
        setError(errorMessage(requestError));
      } finally {
        isCheckingPendingQuestionsRef.current = false;
      }
    },
  );

  const syncOpenChatTabTitlesFromWorkspaces = useCallback(
    (nextWorkspaces: WorkspaceSummary[]) => {
      const titleByChatKey = new Map<string, string>();
      for (const workspace of nextWorkspaces) {
        for (const chat of workspace.chats) {
          const title = chat.title.trim();
          if (title) {
            titleByChatKey.set(chatRunKey(workspace.id, chat.id), title);
          }
        }
      }

      if (titleByChatKey.size === 0) {
        return;
      }

      setOpenChatTabs((current) => {
        let changed = false;
        const nextTabs = current.map((tab) => {
          const nextTitle = titleByChatKey.get(chatRunKey(tab.workspaceId, tab.chatId));
          if (!nextTitle || tab.fallbackTitle === nextTitle) {
            return tab;
          }
          changed = true;
          return { ...tab, fallbackTitle: nextTitle };
        });

        if (!changed) {
          return current;
        }
        openChatTabsRef.current = nextTabs;
        return nextTabs;
      });
    },
    [],
  );

  const refreshWorkspaces = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>("/api/workspaces");
      const workspacesWithRemoteChats = await workspaceSummariesWithRemoteChats(data.workspaces);
      setWorkspaces(workspacesWithRemoteChats);
      syncOpenChatTabTitlesFromWorkspaces(workspacesWithRemoteChats);
      setWorkspaceChatPaging(workspaceChatPagingFromWorkspaces(workspacesWithRemoteChats));
      setActiveWorkspaceId((current) =>
        workspacesWithRemoteChats.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );
      setExpandedWorkspaceId((current) =>
        current !== null &&
          workspacesWithRemoteChats.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, [syncOpenChatTabTitlesFromWorkspaces]);

  const loadSettings = useCallback(async () => {
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      setSettings(data);
      setUpdateStatus(data.update);
      setIsTeamModeEnabled(data.general.defaultTeamModeEnabled);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, []);

  const updateModelRoute = useCallback(
    async (modelId: string, providerId: string) => {
      // Captured once; Strict Mode may invoke the updater twice with the same base state.
      let previousActiveProviderId: string | null | undefined;
      setSettings((current) => {
        if (!current) {
          return current;
        }
        const existing = current.configuredModels.find(
          (model) => model.id === modelId,
        );
        if (previousActiveProviderId === undefined) {
          previousActiveProviderId = existing?.activeProviderId;
        }
        if (!existing || existing.activeProviderId === providerId) {
          return current;
        }
        return {
          ...current,
          configuredModels: current.configuredModels.map((model) =>
            model.id === modelId
              ? { ...model, activeProviderId: providerId }
              : model,
          ),
        };
      });

      try {
        const data = await requestJson<UpdateModelRouteResponse>("/api/models/route", {
          body: JSON.stringify({ modelId, providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        });
        setSettings((current) =>
          current
            ? {
                ...current,
                // Prefer patching the routed model so metadata-derived fields
                // (e.g. supportedThinkingLevels from /settings) stay intact.
                // Do not wholesale-replace configuredModels from the light
                // response (may lack catalog-derived metadata).
                configuredModels: current.configuredModels.map((model) =>
                  model.id === data.modelId
                    ? { ...model, activeProviderId: data.activeProviderId }
                    : model,
                ),
              }
            : current,
        );
        return { ok: true as const };
      } catch (requestError) {
        setSettings((current) => {
          if (!current || previousActiveProviderId === undefined) {
            return current;
          }
          return {
            ...current,
            configuredModels: current.configuredModels.map((model) =>
              model.id === modelId
                ? { ...model, activeProviderId: previousActiveProviderId ?? null }
                : model,
            ),
          };
        });
        return {
          ok: false as const,
          error: errorMessage(requestError) || t("Failed to update model route"),
        };
      }
    },
    [t],
  );

  const loadUpdateStatus = useCallback(async () => {
    try {
      const data = await requestJson<UpdateStatusSummary>("/api/update/status");
      setUpdateStatus(data);
    } catch {
      // ponytail: nav stays quiet on status failures; About exposes explicit check errors.
      setUpdateStatus(null);
    }
  }, []);

  async function installUpdateFromNav() {
    setIsInstallingUpdate(true);
    setUpdateInstallNotice(null);
    try {
      const data = await installUpdateAndWaitForRestart();
      setUpdateStatus(data);
      setUpdateInstallNotice(t("Foco is installing the update and will restart shortly."));
    } catch (requestError) {
      setError(errorMessage(requestError));
      setIsInstallingUpdate(false);
    }
  }

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
    async (
      workspaceId: string,
      chatId: string,
      options?: { silent?: boolean },
    ) => {
      const requestedChatKey = chatRunKey(workspaceId, chatId);
      const isCurrentAgentTeamRequest = () =>
        activeChatKeyRef.current === requestedChatKey;
      const silent =
        options?.silent ??
        (agentTeamSnapshotChatKeyRef.current === requestedChatKey);

      if (!silent) {
        setIsLoadingAgentTeam(true);
      }
      setAgentTeamError(null);

      try {
        const data = await requestJson<AgentTeamSnapshotResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/agent-team`,
        );
        agentTeamSnapshotCacheRef.current.set(requestedChatKey, data);
        if (isCurrentAgentTeamRequest()) {
          agentTeamSnapshotChatKeyRef.current = requestedChatKey;
          setAgentTeamSnapshot(data);
        }
        return data;
      } catch (requestError) {
        const message = errorMessage(requestError);
        if (isCurrentAgentTeamRequest()) {
          if (!silent) {
            agentTeamSnapshotChatKeyRef.current = null;
            setAgentTeamSnapshot(null);
          }
          if (!message.includes("has no Agent team")) {
            setAgentTeamError(message);
          }
        }
        return null;
      } finally {
        if (!silent && isCurrentAgentTeamRequest()) {
          setIsLoadingAgentTeam(false);
        }
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
      void loadAgentTeamSnapshot(event.workspaceId, event.chatId, { silent: true });
    },
    [loadAgentTeamSnapshot],
  );

  const refreshActiveAgentTeamSnapshot = useCallback(
    (workspaceId: string, chatId: string) => {
      if (activeChatKeyRef.current !== chatRunKey(workspaceId, chatId)) {
        return;
      }

      void loadAgentTeamSnapshot(workspaceId, chatId, { silent: true });
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
      // Leaving chat/agent main tabs must not wipe team snapshot cache; only clear live state.
      if (activeMainTab.type !== "chat" && activeMainTab.type !== "agent") {
        return;
      }
      agentTeamSnapshotChatKeyRef.current = null;
      setAgentTeamSnapshot(null);
      setAgentTeamError(null);
      return;
    }

    const requestedChatKey = chatRunKey(activeWorkspaceId, activeChatId);
    const cachedSnapshot = agentTeamSnapshotCacheRef.current.get(requestedChatKey);
    if (cachedSnapshot) {
      agentTeamSnapshotChatKeyRef.current = requestedChatKey;
      setAgentTeamSnapshot(cachedSnapshot);
      setAgentTeamError(null);
      void loadAgentTeamSnapshot(activeWorkspaceId, activeChatId, { silent: true });
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
      !visibleAgentSnapshotHasRunningTask
    ) {
      return;
    }

    let cancelled = false;
    let refreshTimer: number | null = null;
    const scheduleRefresh = () => {
      refreshTimer = window.setTimeout(() => {
        void loadAgentTeamSnapshot(
          visibleAgentSnapshotTarget.workspaceId,
          visibleAgentSnapshotTarget.chatId,
          { silent: true },
        ).finally(() => {
          if (!cancelled) {
            scheduleRefresh();
          }
        });
      }, AGENT_TEAM_RUNNING_REFRESH_MS);
    };

    scheduleRefresh();
    return () => {
      cancelled = true;
      if (refreshTimer !== null) {
        window.clearTimeout(refreshTimer);
      }
    };
  }, [
    canUseApp,
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

  const loadGitDiff = useCallback(
    async (workspaceId: string, path: string | null, target?: SourceControlTarget | null) => {
      setIsLoadingDiff(true);
      setDiffError(null);

      try {
        const params = new URLSearchParams();
        if (path) {
          params.set("path", path);
        }
        appendGitTargetParams(params, target ?? null);
        const queryString = params.toString();
        const query = queryString ? `?${queryString}` : "";
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
    },
    [],
  );

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
      if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
        return null;
      }
      setWorkspaceSpec(null);
      setWorkspaceSpecDraft("");
      setWorkspaceSpecPreviewEnabled(false);
      setWorkspaceSpecError(errorMessage(requestError));
      return null;
    } finally {
      if (!activeWorkspaceIdRef.current || activeWorkspaceIdRef.current === workspaceId) {
        setIsLoadingWorkspaceSpec(false);
      }
    }
  }, []);

  const loadActivePlans = useCallback((workspaceId: string, options: { force?: boolean } = {}) => {
    const nowMs = requestStormDedupeNow();
    const existing = activePlansSingleFlightRef.current.get(workspaceId);
    if (existing && !existing.settled) {
      existing.queued = true;
      return existing.promise;
    }
    if (shouldReuseRequest(existing, nowMs, options.force)) {
      return existing!.promise;
    }

    setIsLoadingActivePlans(true);
    setActivePlansError(null);

    let promise: Promise<PlansResponse | null> = Promise.resolve(null);
    promise = (async () => {
      try {
        const data = await requestJson<PlansResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans?view=active&limit=50`,
        );
        if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
          return null;
        }
        setActivePlans(data.plans);
        setLoadedActivePlansWorkspaceId(workspaceId);
        return data;
      } catch (requestError) {
        if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
          return null;
        }
        setActivePlans([]);
        setLoadedActivePlansWorkspaceId(null);
        setActivePlansError(errorMessage(requestError));
        return null;
      } finally {
        if (!activeWorkspaceIdRef.current || activeWorkspaceIdRef.current === workspaceId) {
          setIsLoadingActivePlans(false);
        }
        const current = activePlansSingleFlightRef.current.get(workspaceId);
        if (current?.promise === promise) {
          const shouldRefreshQueued = current.queued;
          current.settled = true;
          if (shouldRefreshQueued) {
            activePlansSingleFlightRef.current.delete(workspaceId);
            // ponytail: workspace-only queue is enough for the current active-plans endpoint;
            // upgrade to a request key if plan views or page sizes diverge.
            void loadActivePlans(workspaceId, { force: true });
          } else {
            window.setTimeout(() => {
              if (activePlansSingleFlightRef.current.get(workspaceId)?.promise === promise) {
                activePlansSingleFlightRef.current.delete(workspaceId);
              }
            }, REQUEST_STORM_DEDUPE_MS);
          }
        }
      }
    })();
    // ponytail: per-tab single-flight only; cross-tab leader can use BroadcastChannel later.
    activePlansSingleFlightRef.current.set(workspaceId, {
      promise,
      settled: false,
      startedAtMs: nowMs,
    });
    return promise;
  }, []);

  const savePlanOrder = useCallback(
    async (workspaceId: string, planIds: string[], previousPlans: Plan[]) => {
      if (sameStringList(planIds, activePlanOrderIds(previousPlans))) {
        return;
      }
      setActivePlans(reorderActivePlansByIds(previousPlans, planIds));
      setActivePlansError(null);

      try {
        const response = await requestJson<PlansResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/order`,
          {
            body: JSON.stringify({ planIds }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        if (activeWorkspaceIdRef.current && activeWorkspaceIdRef.current !== workspaceId) {
          return;
        }
        setActivePlans(response.plans);
        setLoadedActivePlansWorkspaceId(workspaceId);
      } catch (requestError) {
        setActivePlans(previousPlans);
        setActivePlansError(errorMessage(requestError));
      }
    },
    [],
  );

  const handlePlanRefresh = useCallback(
    (event: Extract<ChatStreamEvent, { type: "planRefresh" }>) => {
      if (activeWorkspaceIdRef.current !== event.workspaceId) {
        return;
      }

      setContextPanelTab("plan");
      setIsContextPanelOpen(true);
      void loadActivePlans(event.workspaceId, { force: true });
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
        const plansResponse = await loadActivePlans(workspaceId, { force: true });
        await refreshWorkspaces();
        const plan =
          action === "retry_merge"
            ? response.plan
            : plansResponse?.plans.find((candidate) => candidate.id === planId) ?? response.plan;
        if (action === "retry_merge") {
          setActivePlans((current) =>
            current.map((candidate) => (candidate.id === planId ? plan : candidate)),
          );
        }
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
      implementationChatId: string | null,
      override?: PlanPhaseRetryOverride,
    ) => {
      const operationKey = `retry-phase:${planId}:${phaseId}`;
      const refreshTarget = { phaseId, planId, workspaceId };
      setPlanOperationKey(operationKey);
      setActivePlansError(null);

      try {
        const response = await requestJson<PlanResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/${encodeURIComponent(planId)}/phases/${encodeURIComponent(phaseId)}/retry`,
          {
            body: JSON.stringify(
              override
                ? {
                    modelId: override.modelId,
                    providerId: override.providerId,
                    ...(override.thinkingLevel
                      ? { thinkingLevel: override.thinkingLevel }
                      : {}),
                  }
                : {},
            ),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        const plansResponse = await loadActivePlans(workspaceId, { force: true });
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
        await loadActivePlans(workspaceId, { force: true });
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

  const loadPlanWorktreeAudit = useCallback(async (workspaceId: string) => {
    return requestJson<PlanWorktreeAuditResponse>(
      `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/worktrees/audit`,
    );
  }, []);

  const cleanupPlanWorktree = useCallback(
    async (workspaceId: string, agentInstanceId: string) => {
      const operationKey = `cleanup-worktree:${agentInstanceId}`;
      setPlanOperationKey(operationKey);
      setActivePlansError(null);

      try {
        await requestJson(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/plans/worktrees/cleanup`,
          {
            body: JSON.stringify({ agentInstanceId, confirm: true }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        await loadActivePlans(workspaceId, { force: true });
        await refreshWorkspaces();
      } catch (requestError) {
        setActivePlansError(errorMessage(requestError));
        throw requestError;
      } finally {
        setPlanOperationKey((current) =>
          current === operationKey ? null : current,
        );
      }
    },
    [loadActivePlans, refreshWorkspaces],
  );

  // ponytail: observe a queued spec job until it settles; an SSE/job push can replace
  // this low-frequency tail later without changing the terminal-state handling.
  const pollWorkspaceSpecJobUntilSettled = useCallback(
    (workspaceId: string, jobId: string) => {
      const existing = workspaceSpecJobObserversRef.current.get(workspaceId);
      if (existing?.jobId === jobId && !existing.cancelled) {
        return existing.promise;
      }
      if (existing) {
        existing.cancelled = true;
      }

      const observer: WorkspaceSpecJobObserver = {
        cancelled: false,
        jobId,
        promise: Promise.resolve(),
      };
      observer.promise = (async () => {
        let pollIndex = 0;
        try {
          while (!observer.cancelled) {
            const delayMs =
              WORKSPACE_SPEC_JOB_POLL_DELAYS_MS[pollIndex] ??
              WORKSPACE_SPEC_JOB_STEADY_POLL_MS;
            pollIndex += 1;
            await new Promise<void>((resolve) => {
              window.setTimeout(resolve, delayMs);
            });
            if (observer.cancelled || activeWorkspaceIdRef.current !== workspaceId) {
              return;
            }

            let jobsResponse: WorkspaceSpecJobsResponse;
            try {
              jobsResponse = await requestJson<WorkspaceSpecJobsResponse>(
                `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs?limit=24`,
              );
            } catch (requestError) {
              if (!observer.cancelled && activeWorkspaceIdRef.current === workspaceId) {
                setWorkspaceSpecError(errorMessage(requestError));
              }
              continue;
            }
            if (observer.cancelled || activeWorkspaceIdRef.current !== workspaceId) {
              return;
            }

            setWorkspaceSpecError(null);
            const job = jobsResponse.jobs.find((candidate) => candidate.id === jobId);
            if (!job) {
              continue;
            }
            setWorkspaceSpec((current) =>
              current ? { ...current, latestJob: job } : current,
            );
            if (job.status === "queued" || job.status === "running") {
              continue;
            }
            if (job.status === "completed") {
              await loadWorkspaceSpec(workspaceId);
            } else if (job.status === "failed" || job.status === "skipped") {
              setWorkspaceSpecError(
                job.errorMessage?.trim() ||
                  (job.status === "skipped"
                    ? "Workspace spec generation was skipped"
                    : "Workspace spec generation failed"),
              );
            }
            return;
          }
        } finally {
          if (workspaceSpecJobObserversRef.current.get(workspaceId) === observer) {
            workspaceSpecJobObserversRef.current.delete(workspaceId);
          }
        }
      })();
      workspaceSpecJobObserversRef.current.set(workspaceId, observer);
      return observer.promise;
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
      // ponytail: keep observing the queued job through long local or brokered runs,
      // then reload spec content so the panel updates without a manual refresh.
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
    (workspaceId: string, chatId: string) => {
      const requestedChatKey = chatRunKey(workspaceId, chatId);
      const nowMs = requestStormDedupeNow();
      const existing = chatStatisticsSingleFlightRef.current.get(requestedChatKey);
      if (shouldReuseRequest(existing, nowMs)) {
        return existing!.promise;
      }

      const requestId =
        (chatStatisticsRequestIdByChatKeyRef.current.get(requestedChatKey) ?? 0) + 1;
      chatStatisticsRequestIdByChatKeyRef.current.set(requestedChatKey, requestId);
      const isCurrentStatisticsRequest = () =>
        chatStatisticsRequestIdByChatKeyRef.current.get(requestedChatKey) === requestId &&
        activeChatKeyRef.current === requestedChatKey;

      setIsLoadingChatStatistics(true);
      setChatStatisticsError(null);

      let promise: Promise<void> = Promise.resolve();
      promise = (async () => {
        try {
          const data = await requestJson<ChatStatisticsResponse>(
            `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/statistics`,
          );
          if (isCurrentStatisticsRequest()) {
            setChatStatistics(normalizeChatStatistics(data, workspaceId, chatId));
            if (!runningChatKeysRef.current.has(requestedChatKey)) {
              clearLiveChatStatistics(requestedChatKey);
            }
          }
        } catch (requestError) {
          if (isCurrentStatisticsRequest()) {
            setChatStatistics(null);
            setChatStatisticsError(errorMessage(requestError));
          }
        } finally {
          if (isCurrentStatisticsRequest()) {
            setIsLoadingChatStatistics(false);
          }
          if (chatStatisticsRequestIdByChatKeyRef.current.get(requestedChatKey) === requestId) {
            chatStatisticsRequestIdByChatKeyRef.current.delete(requestedChatKey);
          }
          const current = chatStatisticsSingleFlightRef.current.get(requestedChatKey);
          if (current?.promise === promise) {
            current.settled = true;
            window.setTimeout(() => {
              if (chatStatisticsSingleFlightRef.current.get(requestedChatKey)?.promise === promise) {
                chatStatisticsSingleFlightRef.current.delete(requestedChatKey);
              }
            }, REQUEST_STORM_DEDUPE_MS);
          }
        }
      })();
      // ponytail: per-tab single-flight only; cross-tab leader can use BroadcastChannel later.
      chatStatisticsSingleFlightRef.current.set(requestedChatKey, {
        promise,
        settled: false,
        startedAtMs: nowMs,
      });
      return promise;
    },
    [],
  );

  const loadGitBranches = useCallback(async (workspaceId: string) => {
    gitBranchesRequestRef.current?.abort();
    const requestId = gitBranchesRequestIdRef.current + 1;
    gitBranchesRequestIdRef.current = requestId;
    const abortController = new AbortController();
    gitBranchesRequestRef.current = abortController;
    setIsLoadingBranches(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/branches`,
        { signal: abortController.signal },
      );
      if (
        abortController.signal.aborted ||
        gitBranchesRequestIdRef.current !== requestId
      ) {
        return;
      }
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");
    } catch (requestError) {
      if (abortController.signal.aborted) {
        return;
      }
      if (gitBranchesRequestIdRef.current !== requestId) {
        return;
      }
      setGitBranches(null);
      setSelectedGitBranch("");
      setBranchError(errorMessage(requestError));
    } finally {
      if (gitBranchesRequestRef.current === abortController) {
        gitBranchesRequestRef.current = null;
      }
      if (gitBranchesRequestIdRef.current === requestId) {
        setIsLoadingBranches(false);
      }
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
    void loadUpdateStatus();
  }, [canUseApp, loadSettings, loadUpdateStatus, refreshWorkspaces]);

  useEffect(() => {
    if (!canUseApp || !updateStatus?.autoCheckEnabled) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadUpdateStatus();
    }, 10 * 60 * 1000);

    return () => window.clearInterval(intervalId);
  }, [canUseApp, loadUpdateStatus, updateStatus?.autoCheckEnabled]);

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
    setSelectedSourceControlTarget(null);
    setIsSourceControlTargetManual(false);
  }, [activeWorkspace?.id, activeChatId]);

  useEffect(() => {
    if (
      isSourceControlTargetManual &&
      !sourceControlTargetFromKey(availableSourceControlTargets, sourceControlTargetKeyValue)
    ) {
      setSelectedSourceControlTarget(null);
      setIsSourceControlTargetManual(false);
    }
  }, [
    availableSourceControlTargets,
    isSourceControlTargetManual,
    sourceControlTargetKeyValue,
  ]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setGitDiff(null);
      setSelectedDiffPath(null);
      setSelectedSourceControlTarget(null);
      setIsSourceControlTargetManual(false);
      setDiffError(null);
      return;
    }

    if (!isContextPanelOpen || contextPanelTab !== "git") {
      return;
    }

    void loadGitDiff(activeWorkspace.id, selectedDiffPath, sourceControlTarget);
  }, [
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    loadGitDiff,
    selectedDiffPath,
    sourceControlTargetKeyValue,
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
      setLoadedActivePlansWorkspaceId(null);
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
    if (
      !activeWorkspace?.id ||
      !isContextPanelOpen ||
      contextPanelTab !== "plan" ||
      !isPlanAutoRunEnabled ||
      !isPlanAutoRunBusy
    ) {
      return;
    }

    void loadActivePlans(activeWorkspace.id, { force: true });
  }, [
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    isPlanAutoRunBusy,
    isPlanAutoRunEnabled,
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
    setLoadedActivePlansWorkspaceId(null);
    setActivePlansError(null);
    setPlanOperationKey(null);
    setIsPlanAutoRunUpdating(false);
    setPendingPlanPhaseRetryRefresh(null);
  }, [activeWorkspace?.id]);

  useEffect(() => {
    if (!activeWorkspaceIdForPlanAutoRun) {
      return;
    }
    if (planAutoRunByWorkspace[activeWorkspaceIdForPlanAutoRun]) {
      return;
    }
    void loadPlanAutoRunState(activeWorkspaceIdForPlanAutoRun);
  }, [
    activeWorkspaceIdForPlanAutoRun,
    loadPlanAutoRunState,
    planAutoRunByWorkspace,
  ]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      return;
    }

    const shouldRefreshAutoRunState = isPlanAutoRunEnabled;
    const shouldRefreshRunningPlans =
      (isPlanAutoRunEnabled || (isContextPanelOpen && contextPanelTab === "plan")) &&
      (isPlanAutoRunBusy || activePlans.some(isAutoRunPlanInFlight));

    if (!shouldRefreshAutoRunState && !shouldRefreshRunningPlans) {
      return;
    }

    // ponytail: one interval owns Plan polling; split again only if these cadences diverge.
    const intervalId = window.setInterval(() => {
      if (!isDocumentVisible()) {
        return;
      }
      if (shouldRefreshAutoRunState) {
        void loadPlanAutoRunState(activeWorkspace.id);
      }
      if (shouldRefreshRunningPlans) {
        void loadActivePlans(activeWorkspace.id);
      }
    }, PLAN_AUTO_RUN_REFRESH_MS);

    return () => window.clearInterval(intervalId);
  }, [
    activePlans,
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    isPlanAutoRunBusy,
    isPlanAutoRunEnabled,
    loadActivePlans,
    loadPlanAutoRunState,
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
      if (!isDocumentVisible()) {
        return;
      }
      void refreshRetryPhase();
    }, PLAN_PHASE_RETRY_REFRESH_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [activeWorkspace?.id, loadActivePlans, pendingPlanPhaseRetryRefresh]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      gitBranchesRequestRef.current?.abort();
      gitBranchesRequestRef.current = null;
      gitBranchesRequestIdRef.current += 1;
      setGitBranches(null);
      setSelectedGitBranch("");
      setIsLoadingBranches(false);
      setBranchError(null);
      return;
    }

    void loadGitBranches(activeWorkspace.id);
    return () => {
      gitBranchesRequestRef.current?.abort();
      gitBranchesRequestRef.current = null;
      gitBranchesRequestIdRef.current += 1;
    };
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
        if (nextRun.request.queuedUserMessageId) {
          failedRestoredQueuedRunKeysRef.current.add(
            restoredQueuedRunKey(
              nextRun.workspaceId,
              nextRun.chatId,
              nextRun.request.queuedUserMessageId,
            ),
          );
        }
        updateScheduledWorkspaceRuns((current) =>
          current.filter((run) => run.id !== nextRun.id),
        );
      }
      void refreshWorkspaces();
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
      if (next.length !== current.length) {
        pruneAgentTabCaches(
          agentTeamSnapshotCacheRef.current,
          agentTranscriptViewCacheRef.current,
          next,
        );
      }
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
    setSelectedThinkingLevel((current) => {
      if (!selectedModel) {
        hasManuallySelectedThinkingLevelRef.current = false;
        return "";
      }

      const defaultThinkingLevel =
        !hasManuallySelectedModelRef.current &&
          selectedModel.id === defaultComposerSelection.modelId
          ? defaultComposerSelection.thinkingLevel
          : defaultThinkingLevelForModel(selectedModel);

      if (!hasManuallySelectedThinkingLevelRef.current) {
        return defaultThinkingLevel;
      }

      if (!current || isModelThinkingLevelSupported(selectedModel, current)) {
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
      const isRemoteWorkspace = workspaceMode === "ssh";
      const data = await requestJson<WorkspacesResponse>("/api/workspaces/add", {
        body: JSON.stringify({
          name: workspaceName,
          path: isRemoteWorkspace ? workspacePath : workspacePath,
          remotePath: isRemoteWorkspace ? workspacePath : null,
          serverId: isRemoteWorkspace ? workspaceServerId : null,
          terminalShell: workspaceTerminalShell || null,
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
      setWorkspaceChatPaging(workspaceChatPagingFromWorkspaces(data.workspaces));
      void loadSettings();
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
      setWorkspaceTerminalShell("");
      setWorkspaceServerId("");
      setWorkspaceMode("local");
      setWorkspaceTestStages([]);
      setWorkspaceSpecEnabled(false);
      closeWorkspaceDialog();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  async function testWorkspaceRemoteConnection() {
    if (!workspaceServerId) {
      setError(t("Select a remote server first."));
      return;
    }

    setIsTestingWorkspaceConnection(true);
    setError(null);
    setWorkspaceTestStages(remoteWorkspacePendingStages(t));

    try {
      const response = await requestJson<RemoteServerDiagnosticResponse>(
        `/api/remote-servers/${encodeURIComponent(workspaceServerId)}/connect`,
        { method: "POST" },
      );
      setWorkspaceTestStages([
        ...response.result.stages,
        {
          details: null,
          errorKind: response.result.ok ? null : response.result.errorKind,
          message: response.result.message ?? (response.result.ok ? t("Ready") : t("Failed")),
          stage: "ready",
          status: response.result.ok ? "success" : "failed",
        },
      ]);
      const nextSettings = await requestJson<SettingsResponse>("/api/settings");
      setSettings(nextSettings);
      if (
        response.result.hostKeyVerificationRequired ||
        response.result.errorKind === "host_key_unknown"
      ) {
        setError(
          t(
            "Host key verification required. Confirm the fingerprint in Remote Servers settings, or retry after trusting the host.",
          ),
        );
      } else if (response.result.errorKind === "host_key_changed") {
        setError(
          t("Host key changed — manual known_hosts fix required") +
            " " +
            t(
              "This host presented a different key than the one stored in known_hosts. Foco will not overwrite it. Remove or update the entry in your known_hosts file, then try again.",
            ),
        );
      } else if (!response.result.ok && response.result.message) {
        setError(response.result.message);
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
      setWorkspaceTestStages((current) => [
        ...current.filter((stage) => stage.stage !== "ready"),
        {
          details: null,
          errorKind: "request_failed",
          message: errorMessage(requestError),
          stage: "ready",
          status: "failed",
        },
      ]);
    } finally {
      setIsTestingWorkspaceConnection(false);
    }
  }

  async function createInlineRemoteServer() {
    if (!inlineRemoteServerName.trim() || !inlineRemoteServerHost.trim()) {
      setError(t("Remote server name and host are required."));
      return;
    }

    setIsCreatingInlineRemoteServer(true);
    setError(null);

    try {
      const response = await requestJson<RemoteServerResponse>("/api/remote-servers/create", {
        body: JSON.stringify({
          hostAlias: inlineRemoteServerHost.trim(),
          name: inlineRemoteServerName.trim(),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      try {
        const connectResponse = await requestJson<RemoteServerDiagnosticResponse>(
          `/api/remote-servers/${encodeURIComponent(response.server.id)}/connect`,
          { method: "POST" },
        );
        const nextSettings = await requestJson<SettingsResponse>("/api/settings");
        setSettings(nextSettings);
        setWorkspaceServerId(response.server.id);
        if (!workspacePath.trim() && response.server.defaultRemoteRoot) {
          setWorkspaceRemotePath(response.server.defaultRemoteRoot);
        }
        setInlineRemoteServerName("");
        setInlineRemoteServerHost("");
        if (
          connectResponse.result.hostKeyVerificationRequired ||
          connectResponse.result.errorKind === "host_key_unknown"
        ) {
          setError(
            t(
              "Host key verification required. Confirm the fingerprint in Remote Servers settings, or retry after trusting the host.",
            ),
          );
          return;
        }
        if (connectResponse.result.errorKind === "host_key_changed") {
          setError(
            t("Host key changed — manual known_hosts fix required") +
              " " +
              t(
                "This host presented a different key than the one stored in known_hosts. Foco will not overwrite it. Remove or update the entry in your known_hosts file, then try again.",
              ),
          );
          return;
        }
        if (!connectResponse.result.ok && connectResponse.result.message) {
          setError(connectResponse.result.message);
          return;
        }
      } catch (connectError) {
        const nextSettings = await requestJson<SettingsResponse>("/api/settings");
        setSettings(nextSettings);
        setWorkspaceServerId(response.server.id);
        throw connectError;
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsCreatingInlineRemoteServer(false);
    }
  }

  function setWorkspaceRemoteServer(serverId: string) {
    setWorkspaceServerId(serverId);
    const server = settings?.remoteServers.find((item) => item.id === serverId);
    if (server?.defaultRemoteRoot && !workspacePath.trim()) {
      setWorkspaceRemotePath(server.defaultRemoteRoot);
    }
  }

  function setWorkspaceRemotePath(path: string) {
    setWorkspacePath(path);
    setWorkspaceName((current) => current.trim() ? current : remoteWorkspacePathBasename(path));
  }

  async function retryRemoteWorkspace(workspace: WorkspaceSummary) {
    if (!workspace.serverId) {
      return;
    }

    setRetryingRemoteWorkspaceId(workspace.id);
    setError(null);

    try {
      await requestJson(
        `/api/remote-servers/${encodeURIComponent(workspace.serverId)}/workspaces/${encodeURIComponent(workspace.id)}/connect`,
        { method: "POST" },
      );
      await refreshWorkspaces();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setRetryingRemoteWorkspaceId(null);
    }
  }

  function clearWorkspaceIconDraft() {
    setWorkspaceIconDraft(null);
  }

  async function handleWorkspaceIconPickerSelection(selection: FilePickerSelection[]) {
    const file = selection[0]?.file;
    if (!file) {
      return;
    }
    if (file.sizeBytes === 0) {
      setWorkspaceIconDraft(null);
      return;
    }
    if (!file.contentBase64) {
      setError(t("Selected file content is missing."));
      return;
    }
    setWorkspaceIconDraft({
      contentBase64: file.contentBase64,
      name: file.name,
      previewUrl: file.contentType
        ? `data:${file.contentType};base64,${file.contentBase64}`
        : "",
    });
  }

  function handleSelectWorkspacePath() {
    setFilePickerRequest({
      initialPath: workspacePath,
      mode: "directory",
      target: workspaceMode === "ssh" && workspaceServerId
        ? { kind: "remoteServer", serverId: workspaceServerId }
        : { kind: "local" },
      title: t("Select workspace folder"),
      onSelect: (selection) => {
        const selectedPath = selection[0]?.path;
        if (!selectedPath) {
          return;
        }
        if (workspaceMode === "ssh") {
          setWorkspaceRemotePath(selectedPath);
          return;
        }
        setWorkspacePath(selectedPath);
        setWorkspaceName((current) =>
          current.trim() ? current : workspaceNameFromPath(selectedPath),
        );
      },
    });
  }

  function handleSelectWorkspaceIcon() {
    setFilePickerRequest({
      mode: "file",
      readFiles: true,
      target: workspaceMode === "ssh" && workspaceServerId
        ? { kind: "remoteServer", serverId: workspaceServerId }
        : { kind: "local" },
      title: t("Select workspace icon"),
      onSelect: (selection) => {
        void handleWorkspaceIconPickerSelection(selection);
      },
    });
  }

  function handleSelectDraftAttachments() {
    const target: FilePickerTarget = activeWorkspace?.id
      ? { kind: "workspace", workspaceId: activeWorkspace.id }
      : { kind: "local" };
    setFilePickerRequest({
      mode: "file",
      multiple: true,
      readFiles: true,
      target,
      title: t("Add attachment"),
      onSelect: (selection) => {
        void handleAddSelectedFileAttachments(
          selection
            .map((item) => item.file)
            .filter((file): file is NonNullable<FilePickerSelection["file"]> => Boolean(file)),
        );
      },
    });
  }

  function handleSelectEditAttachments(
    onSelected: (attachments: ComposerAttachment[]) => void,
  ) {
    const target: FilePickerTarget = activeWorkspace?.id
      ? { kind: "workspace", workspaceId: activeWorkspace.id }
      : { kind: "local" };
    setFilePickerRequest({
      mode: "file",
      multiple: true,
      readFiles: true,
      target,
      title: t("Add attachment"),
      onSelect: (selection) => {
        const attachments = selection
          .map((item) => item.file)
          .filter((file): file is NonNullable<FilePickerSelection["file"]> => Boolean(file))
          .map(composerAttachmentFromSelectedFile);
        if (attachments.length) {
          onSelected(attachments);
        }
      },
    });
  }

  async function handleAddSelectedFileAttachments(files: NonNullable<FilePickerSelection["file"]>[]) {
    if (!files.length) {
      return;
    }

    setIsSelectingAttachments(true);
    setError(null);

    try {
      const attachments = files.map(composerAttachmentFromSelectedFile);
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
      await handleAddDraftAttachments(attachments);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingAttachments(false);
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
    rememberChatCacheAccess(chatKey);
    trimInactiveChatCaches();

    if (activeChatKeyRef.current === chatKey) {
      setMessages(nextForKey);
    }
  }

  const STREAM_DELTA_FLUSH_MS = 32;

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

  function appendBufferedReasoningDelta(
    current: ShellMessage[],
    assistantMessageId: string,
    delta: string,
    startedAtMs: number,
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
      reasoning: `${message.reasoning ?? ""}${delta}`,
      parts: appendReasoningPart(message.parts, delta, startedAtMs),
    };
    return next;
  }

  type BufferedToolOutputDelta = {
    assistantMessageId: string;
    delta: string;
    stream: "stdout" | "stderr";
    toolCallId: string;
  };

  function appendBufferedToolOutputDeltas(
    current: ShellMessage[],
    deltas: BufferedToolOutputDelta[],
  ) {
    let next = current;
    for (const delta of deltas) {
      const messageOwnsToolCall = (message: ShellMessage) =>
        messageHasToolCall(message, delta.toolCallId);
      const updateExistingToolCall = next.some(messageOwnsToolCall);
      next = next.map((message) =>
        (updateExistingToolCall
          ? messageOwnsToolCall(message)
          : message.role === "assistant" && message.id === delta.assistantMessageId)
          ? {
            ...message,
            parts: applyToolOutputDeltaToParts(
              message.parts,
              delta.toolCallId,
              delta.stream,
              delta.delta,
            ),
            toolCalls: applyToolOutputDelta(
              message.toolCalls,
              delta.toolCallId,
              delta.stream,
              delta.delta,
            ),
          }
          : message,
      );
    }
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
        setMessagesForChatKey(chatKey, (current) => {
          let next = current;
          for (const [assistantMessageId, delta] of messageDeltas) {
            next = appendBufferedTextDelta(next, assistantMessageId, delta);
          }
          return next;
        });
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
        }, STREAM_DELTA_FLUSH_MS);
      },
    };
  }

  function createReasoningDeltaBuffer() {
    const bufferedDeltasByChatKey = new Map<
      string,
      Map<string, { delta: string; startedAtMs: number }>
    >();
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
        setMessagesForChatKey(chatKey, (current) => {
          let next = current;
          for (const [assistantMessageId, bufferedDelta] of messageDeltas) {
            next = appendBufferedReasoningDelta(
              next,
              assistantMessageId,
              bufferedDelta.delta,
              bufferedDelta.startedAtMs,
            );
          }
          return next;
        });
      }
    };

    return {
      flush,
      push(
        chatKey: string,
        assistantMessageId: string,
        delta: string,
        startedAtMs: number,
      ) {
        const messageDeltas =
          bufferedDeltasByChatKey.get(chatKey) ??
          new Map<string, { delta: string; startedAtMs: number }>();
        const current = messageDeltas.get(assistantMessageId);
        messageDeltas.set(assistantMessageId, {
          delta: `${current?.delta ?? ""}${delta}`,
          startedAtMs: current?.startedAtMs ?? startedAtMs,
        });
        bufferedDeltasByChatKey.set(chatKey, messageDeltas);

        if (flushTimer !== null) {
          return;
        }

        flushTimer = window.setTimeout(() => {
          flushTimer = null;
          flush();
        }, STREAM_DELTA_FLUSH_MS);
      },
    };
  }

  function createToolOutputDeltaBuffer() {
    const bufferedDeltasByChatKey = new Map<
      string,
      Map<string, BufferedToolOutputDelta>
    >();
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

      for (const [chatKey, toolDeltas] of bufferedDeltas) {
        setMessagesForChatKey(chatKey, (current) =>
          appendBufferedToolOutputDeltas(current, Array.from(toolDeltas.values())),
        );
      }
    };

    return {
      flush,
      push(
        chatKey: string,
        delta: BufferedToolOutputDelta,
      ) {
        const toolDeltas =
          bufferedDeltasByChatKey.get(chatKey) ??
          new Map<string, BufferedToolOutputDelta>();
        const key = `${delta.toolCallId}\u0000${delta.stream}`;
        const current = toolDeltas.get(key);
        toolDeltas.set(key, {
          ...delta,
          assistantMessageId: current?.assistantMessageId ?? delta.assistantMessageId,
          delta: `${current?.delta ?? ""}${delta.delta}`,
        });
        bufferedDeltasByChatKey.set(chatKey, toolDeltas);

        if (flushTimer !== null) {
          return;
        }

        // ponytail: simple Map buffer; very large tool output still grows in
        // memory until flush, upgrade path is backend summarization/truncation.
        flushTimer = window.setTimeout(() => {
          flushTimer = null;
          flush();
        }, STREAM_DELTA_FLUSH_MS);
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

  function activeRunSummaryFromInfo(
    runInfo: ActiveRunInfo,
  ): ActiveChatRunSummary | null {
    if (!runInfo.chatId || !runInfo.runId) {
      return null;
    }

    return {
      acceptingGuidance: runInfo.acceptingGuidance,
      chatId: runInfo.chatId,
      lastSequence: runInfo.lastSequence ?? null,
      runId: runInfo.runId,
      workspaceId: runInfo.workspaceId,
    };
  }

  function recoverActiveChatStreams(reason: "online" | "visible") {
    for (const runInfo of Object.values(activeRunInfoByChatKeyRef.current)) {
      if (
        !runningChatKeysRef.current.has(runInfo.chatKey) ||
        activeRunAbortByChatKeyRef.current.has(runInfo.chatKey)
      ) {
        continue;
      }

      const activeRun = activeRunSummaryFromInfo(runInfo);
      if (activeRun) {
        console.debug("[chat-stream] recovering active run stream", {
          chatId: activeRun.chatId,
          lastSequence: activeRun.lastSequence,
          reason,
          runId: activeRun.runId,
          workspaceId: activeRun.workspaceId,
        });
        void subscribeActiveChatRun(activeRun, true);
        continue;
      }

      if (runInfo.chatId) {
        console.debug("[chat-stream] refreshing running chat after recovery trigger", {
          chatId: runInfo.chatId,
          reason,
          workspaceId: runInfo.workspaceId,
        });
        void loadChatMessages(runInfo.workspaceId, runInfo.chatId);
      }
    }
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
          const restoredRunKey = restoredQueuedRunKey(
            workspace.id,
            chat.id,
            chat.queuedRun.userMessageId,
          );
          if (
            nextRunChatKeys.has(chatKey) ||
            failedRestoredQueuedRunKeysRef.current.has(restoredRunKey) ||
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
            skillIds: normalizeStringArray(chat.queuedRun.skillIds),
            localChatKey: chatKey,
            pendingUserMessageId: chat.queuedRun.userMessageId,
            queuedUserMessageId: chat.queuedRun.userMessageId,
            assistantMessageId: chat.queuedRun.assistantMessageId ?? undefined,
          };
          const existingRun = currentByChatKey.get(chatKey);
          const scheduledRun: ScheduledWorkspaceRun = existingRun
            ? { ...existingRun, request: queuedRequest }
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
        skillIds: normalizeStringArray(message.queuedRun?.skillIds),
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

  function setActiveWorkspaceChatRefs(
    workspaceId: string,
    chatId: string | null,
    options: { syncPlanMode?: boolean } = {},
  ) {
    activeWorkspaceIdRef.current = workspaceId;
    activeChatIdRef.current = chatId;
    activeChatKeyRef.current = chatId ? chatRunKey(workspaceId, chatId) : null;
    // Only restore plan/model on real chat navigation. Queue accept and stream
    // start also update active chat ids and must not rewrite a manual model pick.
    if (options.syncPlanMode) {
      restorePlanModeForChatKey(activeChatKeyRef.current);
    }
  }

  function restorePlanModeForChatKey(chatKey: string | null) {
    const enabled = chatKey ? planModeByChatKeyRef.current[chatKey] === true : false;
    setIsPlanModeEnabled(enabled);
    applyComposerModelForPlanMode(enabled);
  }

  function applyComposerModelForPlanMode(enabled: boolean) {
    if (enabled) {
      const modeModelId = settings?.plan.modeModelId?.trim() || "";
      if (!modeModelId) {
        return;
      }
      const model = availableModels.find((candidate) => candidate.id === modeModelId);
      if (!model) {
        return;
      }
      if (
        !(
          model.activeProviderId && model.providerIds.includes(model.activeProviderId)
            ? model.activeProviderId
            : model.providerIds[0]
        )
      ) {
        return;
      }
      hasManuallySelectedModelRef.current = true;
      hasManuallySelectedThinkingLevelRef.current = false;
      setSelectedModelId(model.id);
      setSelectedThinkingLevel(defaultThinkingLevelForModel(model));
      return;
    }

    hasManuallySelectedModelRef.current = false;
    hasManuallySelectedThinkingLevelRef.current = false;
    setSelectedModelId(defaultComposerSelection.modelId);
    setSelectedThinkingLevel(defaultComposerSelection.thinkingLevel);
  }

  function rememberPlanModeForChatKey(chatKey: string, value: boolean) {
    planModeByChatKeyRef.current[chatKey] = value;
  }

  function bindRequestPlanModeToChatKey(request: RetryRunRequest, chatKey: string) {
    rememberPlanModeForChatKey(chatKey, request.sessionMode === "plan");
  }

  function workspaceHasRunningOrStartingRun(workspaceId: string) {
    return (
      [...runningChatKeys].some(
        (chatKey) => chatKeyWorkspaceId(chatKey) === workspaceId,
      ) ||
      workspaces.some(
        (workspace) =>
          workspace.id === workspaceId &&
          workspace.chats.some(
            (chat) =>
              chatSessionStatusFor(chatRunKey(workspace.id, chat.id), {
                workspaceActiveRun: chat.activeRun,
              }).kind === "running",
          ),
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
    restorePlanModeForChatKey(run.chatKey);
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
      const normalizedMessages = expandMessagesWithUserInterruptions(
        data.messages.map(normalizeChatMessageSummary),
      );
      const activeRun = normalizeActiveChatRunSummary(data.activeRun);
      const cachedMessages = chatMessagesByKeyRef.current[chatKey] ?? [];
      const localRunInfo = activeRunInfoByChatKeyRef.current[chatKey] ?? null;
      const hasLocalActiveRun =
        Boolean(localRunInfo) && runningChatKeysRef.current.has(chatKey);
      // Preserve local streaming even when the messages API omits activeRun
      // (e.g. subagent return window) or run summary fields are incomplete.
      const preserveStreamingPlaceholders =
        Boolean(activeRun) || hasLocalActiveRun;
      const mergeResult = mergeLoadedMessagesWithStreamingPlaceholders(
        normalizedMessages,
        cachedMessages,
        preserveStreamingPlaceholders,
      );
      const nextMessages = preserveCachedReasoningDurations(
        mergeResult.messages,
        cachedMessages,
      );
      const restoredQuestion = parseQuestionRequestSummary(data.pendingQuestion);
      const serverPagination = normalizeChatMessagesPagination(data.pagination);
      const existingPagination = chatMessagePaginationByKeyRef.current[chatKey];
      const cacheWasTrimmed = trimmedChatCacheKeysRef.current.has(chatKey);
      const pagination =
        mergeResult.preservedCachePrefix && existingPagination && !cacheWasTrimmed
          ? existingPagination
          : serverPagination;
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
      rememberChatCacheAccess(chatKey);
      trimmedChatCacheKeysRef.current.delete(chatKey);
      setChatMessagePaginationByKey((current) => ({ ...current, [chatKey]: pagination }));
      trimInactiveChatCaches();
      restoreQueuedRunRequestsForChatKey(workspaceId, chatId, nextMessages);
      if (activeChatKeyRef.current === chatKey) {
        setMessages(nextMessages);
        setPendingQuestion((current) =>
          restoredQuestion ??
          (current?.workspaceId === workspaceId && current.chatId === chatId ? null : current),
        );
        if (restoredQuestion) {
          setQuestionError(null);
          setIsAnsweringQuestion(false);
        }
      }
      if (activeRun) {
        void subscribeActiveChatRun(activeRun);
      } else if (!hasLocalActiveRun) {
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
      const olderMessages = expandMessagesWithUserInterruptions(
        data.messages.map(normalizeChatMessageSummary),
      );
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
      rememberChatCacheAccess(chatKey);
      trimmedChatCacheKeysRef.current.delete(chatKey);
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

  async function ensureWorkspaceChatLoaded(workspaceId: string, chatId: string) {
    if (isPendingChatId(chatId)) {
      return;
    }
    const workspace = workspaces.find((candidate) => candidate.id === workspaceId);
    if (!workspace || workspace.chats.some((chat) => chat.id === chatId)) {
      return;
    }

    try {
      const params = new URLSearchParams({
        includeChatId: chatId,
        limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE),
      });
      const data = await requestJson<WorkspaceChatsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats?${params.toString()}`,
      );
      setWorkspaces((current) =>
        current.map((item) => {
          if (item.id !== workspaceId) {
            return item;
          }
          const existingChatIds = new Set(item.chats.map((chat) => chat.id));
          return {
            ...item,
            chatPagination: {
              hasMore: data.hasMore,
              limit: data.limit,
              nextCursor: data.nextCursor,
              total: data.total,
            },
            chats: [
              ...item.chats,
              ...data.chats.filter((chat) => !existingChatIds.has(chat.id)),
            ],
          };
        }),
      );
      setWorkspaceChatPaging((current) => ({
        ...current,
        [workspaceId]: {
          hasMore: data.hasMore,
          isLoading: false,
          nextCursor: data.nextCursor,
          total: data.total,
        },
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
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
      setActiveWorkspaceChatRefs(workspaceId, chatId, { syncPlanMode: true });
      setMessages(cachedMessages);
      setSelectedDiffPath(null);
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      if (options.updateUrl !== false) {
        updateBrowserRoute({ chatId: null, viewMode: "chat", workspaceId });
      }
      return;
    }

    void ensureWorkspaceChatLoaded(workspaceId, chatId);
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
    const cacheWasTrimmed = trimmedChatCacheKeysRef.current.has(chatKey);
    const localChatRunning = runningChatKeysRef.current.has(chatKey);

    if (!cachedMessages) {
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setActiveMainTab({ chatId, type: "chat", workspaceId });
      setExpandedWorkspaceId(workspaceId);
      openChatTab(workspaceId, chatId);
      setActiveWorkspaceChatRefs(workspaceId, chatId, { syncPlanMode: true });
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
    setActiveWorkspaceChatRefs(workspaceId, chatId, { syncPlanMode: true });
    rememberChatCacheAccess(chatKey);
    setMessages(cachedMessages);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      updateBrowserRoute({ chatId, viewMode: "chat", workspaceId });
    }
    if (workspaceChatActiveRun || cacheWasTrimmed || localChatRunning) {
      void loadChatMessages(workspaceId, chatId);
    }
  }

  function startNewWorkspaceChat(
    workspaceId: string,
    options: { updateUrl?: boolean } = {},
  ) {
    resetComposerDefaultsForNewChat();
    setExpandedWorkspaceId(workspaceId);
    setActiveWorkspaceChatRefs(workspaceId, null, { syncPlanMode: true });
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
    const cachedTeamSnapshot = agentTeamSnapshotCacheRef.current.get(chatKey);

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
    setActiveWorkspaceChatRefs(tab.workspaceId, tab.chatId, { syncPlanMode: true });
    setMessages(cachedMessages ?? []);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({
      chatId: tab.chatId,
      viewMode: "chat",
      workspaceId: tab.workspaceId,
    });

    if (cachedTeamSnapshot) {
      agentTeamSnapshotChatKeyRef.current = chatKey;
      setAgentTeamSnapshot(cachedTeamSnapshot);
      setAgentTeamError(null);
      setIsLoadingAgentTeam(false);
    }

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
    const tabsToClose = candidates;
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
    setOpenAgentTabs((current) => {
      const next = current.filter(
        (tab) =>
          !closedKeys.has(`agent:${tab.workspaceId}:${tab.chatId}:${tab.instanceId}`),
      );
      pruneAgentTabCaches(
        agentTeamSnapshotCacheRef.current,
        agentTranscriptViewCacheRef.current,
        next,
      );
      return next;
    });

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
    setActiveWorkspaceChatRefs(workspaceId, null, { syncPlanMode: true });
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
    setOpenAgentTabs((current) => {
      const next = current.filter(
        (current) =>
          current.workspaceId !== tab.workspaceId ||
          current.chatId !== tab.chatId ||
          current.instanceId !== tab.instanceId,
      );
      pruneAgentTabCaches(
        agentTeamSnapshotCacheRef.current,
        agentTranscriptViewCacheRef.current,
        next,
      );
      return next;
    });

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

    selectWorkspaceChat(tab.workspaceId, tab.chatId);
  }

  function closeChatTab(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);

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

    setActiveWorkspaceChatRefs(activeWorkspaceId || workspaceId, null, {
      syncPlanMode: true,
    });
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
    const chatKey = chatRunKey(workspace.id, chat.id);
    if (
      chatSessionStatusFor(chatKey, { workspaceActiveRun: chat.activeRun }).kind ===
      "running"
    ) {
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
    const chatKey = chatRunKey(workspaceId, chatId);
    const workspaceChat = workspaces
      .find((workspace) => workspace.id === workspaceId)
      ?.chats.find((chat) => chat.id === chatId);
    if (
      chatSessionStatusFor(chatKey, { workspaceActiveRun: workspaceChat?.activeRun ?? null })
        .kind === "running"
    ) {
      setError(t("Cancel the current run before deleting this chat."));
      return;
    }

    setError(null);

    try {
      await requestJson<unknown>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/delete`,
        { method: "POST" },
      );

      if (activeWorkspaceId === workspaceId && activeChatId === chatId) {
        const nextOpenChatTabs = openChatTabsRef.current.filter(
          (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
        );
        openChatTabsRef.current = nextOpenChatTabs;
        setOpenChatTabs(nextOpenChatTabs);
        setActiveWorkspaceChatRefs(workspaceId, null, { syncPlanMode: true });
        setActiveWorkspaceId(workspaceId);
        setActiveChatId(null);
        setActiveMainTab({ chatId: null, type: "chat", workspaceId });
        setMessages([]);
        updateBrowserRoute({
          chatId: null,
          tabs: openChatTabsToBrowserRouteTabs(nextOpenChatTabs),
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
      await refreshWorkspaces();
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
        void loadGitDiff(activeWorkspace.id, selectedDiffPath, sourceControlTarget);
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

  function workspaceFileDownloadUrl(workspaceId: string, path: string) {
    return `/api/workspaces/${encodeURIComponent(workspaceId)}/files/download?path=${encodeURIComponent(path)}`;
  }

  function downloadWorkspaceFile(node: WorkspaceFileTreeNode) {
    if (!activeWorkspace) {
      setWorkspaceFilesError(t("Select a workspace before using file actions."));
      return;
    }
    if (!node.path) {
      setWorkspaceFilesError(t("Select a workspace before using file actions."));
      return;
    }

    setWorkspaceFilesError(null);
    const anchor = document.createElement("a");
    anchor.href = workspaceFileDownloadUrl(activeWorkspace.id, node.path);
    anchor.download = node.name;
    anchor.rel = "noopener";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
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
          body: JSON.stringify(gitTargetRequestBody({ path }, sourceControlTarget)),
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
          body: JSON.stringify(gitTargetRequestBody({ message }, sourceControlTarget)),
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
          body: JSON.stringify(
            gitTargetRequestBody(
              {
                modelId: selectedModelId,
                providerId: selectedProviderId,
              },
              sourceControlTarget,
            ),
          ),
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
        void loadGitDiff(activeWorkspace.id, selectedDiffPath, sourceControlTarget);
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
        void loadGitDiff(activeWorkspace.id, selectedDiffPath, sourceControlTarget);
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
      thinkingLevel: isModelThinkingLevelSupported(selectedModel, selectedThinkingLevel)
        ? selectedThinkingLevel
        : "",
      workspaceId: currentWorkspace.id,
    };
  }

  function handlePlanModeEnabledChange(value: boolean) {
    setIsPlanModeEnabled(value);
    applyComposerModelForPlanMode(value);
    const chatKey = activeChatKeyRef.current;
    if (chatKey) {
      rememberPlanModeForChatKey(chatKey, value);
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
    const idempotencyKey = request.idempotencyKey ?? localRandomId("queue");
    return requestJson<QueueChatMessageResponse>(
      `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/queue`,
      {
        body: JSON.stringify({
          chatId: request.chatId,
          idempotencyKey,
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

  async function handleEditChatMessage(
    message: ShellMessage,
    content: string,
    editedSkillIds: string[],
    attachments: ComposerAttachment[],
    onAccepted: () => void,
  ): Promise<boolean> {
    const workspaceId = activeWorkspaceIdRef.current;
    const chatId = activeChatIdRef.current;
    if (!workspaceId || !chatId || isPendingChatId(chatId) || isSendingMessage) {
      return false;
    }
    const runConfig = message.runConfig;
    const modelId = runConfig?.modelId ?? selectedModelIdRef.current;
    const providerId = runConfig?.providerId ?? selectedProviderIdRef.current;
    if (!modelId || !providerId) {
      setError(t("This message does not have a reusable model configuration."));
      return false;
    }
    const chatKey = chatRunKey(workspaceId, chatId);
    const previousMessages = [...(chatMessagesByKeyRef.current[chatKey] ?? [])];
    setError(null);
    setIsPreparingChatRun(true);
    try {
      const edited = await requestJson<EditChatUserMessageResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages/${encodeURIComponent(message.id)}/edit`,
        {
          body: JSON.stringify({
            attachments: attachments.map(({ previewDataUrl: _previewDataUrl, ...attachment }) => attachment),
            expectedContent: message.content,
            message: content,
            modelId,
            providerId,
            thinkingLevel: runConfig?.thinkingLevel || null,
            selectedSkillIds: editedSkillIds,
            sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? null,
            teamModeEnabled: runConfig?.teamModeEnabled ?? false,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      const targetIndex = previousMessages.findIndex((item) => item.id === message.id);
      if (targetIndex < 0) {
        throw new Error(t("Edited message is no longer visible."));
      }
      setMessagesForChatKey(chatKey, () => [
        ...previousMessages.slice(0, targetIndex),
        {
          ...message,
          content: edited.content,
          parts: edited.parts,
          runConfig: {
            modelId,
            providerId,
            thinkingLevel: runConfig?.thinkingLevel ?? null,
            selectedSkillIds: editedSkillIds,
            sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? null,
            teamModeEnabled: runConfig?.teamModeEnabled ?? false,
          },
        },
      ]);
      onAccepted();
      updateQueuedRunRequestsForChatKey(chatKey, () => []);
      updateScheduledWorkspaceRuns((current) => current.filter((run) => run.chatKey !== chatKey));
      setRetryRunRequest(null);
      setPendingQuestion(null);
      setQuestionError(null);
      clearLiveChatStatistics(chatKey);
      setChatMessagePaginationByKey((current) => ({
        ...current,
        [chatKey]: { hasMoreBefore: false, nextBeforeSequence: null },
      }));
      contextUsageIdentityByChatKeyRef.current.delete(chatKey);
      contextUsageAbortByChatKeyRef.current.get(chatKey)?.abort();
      setContextUsageByChatKey((current) => {
        const next = { ...current };
        delete next[chatKey];
        return next;
      });
      await runChatMessage({
        attachments,
        chatId,
        content: edited.content,
        modelId,
        providerId,
        skillIds: editedSkillIds,
        sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? undefined,
        teamModeEnabled: runConfig?.teamModeEnabled ?? false,
        thinkingLevel: runConfig?.thinkingLevel ?? "",
        workspaceId,
        localChatKey: chatKey,
        pendingUserMessageId: edited.userMessageId,
        queuedUserMessageId: edited.userMessageId,
        assistantMessageId: edited.assistantMessageId,
      });
      await loadChatMessages(workspaceId, chatId);
      void loadChatStatistics(workspaceId, chatId);
      void refreshWorkspaces();
      refreshActiveAgentTeamSnapshot(workspaceId, chatId);
      return true;
    } catch (requestError) {
      setMessagesForChatKey(chatKey, () => previousMessages);
      setError(errorMessage(requestError));
      return false;
    } finally {
      setIsPreparingChatRun(false);
    }
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
      bindRequestPlanModeToChatKey(request, chatKey);
      setActiveWorkspaceChatRefs(request.workspaceId, queued.chatId);
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
    setSelectedSkillIds(retryRequest.skillIds);
    setSelectedThinkingLevel(retryRequest.thinkingLevel);
    await runChatMessage(retryRequest);
  }

  function handleChatModelChange(modelId: string) {
    hasManuallySelectedModelRef.current = true;
    hasManuallySelectedThinkingLevelRef.current = false;
    setSelectedModelId(modelId);
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
    openSkillStoreView,
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
    setStatsRouteFilters,
    setStatsRoutePage,
    setViewMode,
    updateBrowserRoute,
    workspaces,
  });

  const updateStatsRoute = useCallback(
    (page: number, filters: Partial<AiStatsFilterState> = statsRouteFiltersRef.current) => {
      setStatsRoutePage((current) => (current === page ? current : page));
      setStatsRouteFilters(filters);
      updateBrowserRoute({ filters, page, viewMode: "stats" });
    },
    [updateBrowserRoute],
  );

  const handleOpenMessageApiRequests = useCallback(
    (message: ShellMessage) => {
      const requestIds = message.metrics?.llmRequestIds.filter(Boolean) ?? [];
      if (!requestIds.length) {
        return;
      }

      const filters: Partial<AiStatsFilterState> = {
        chatId: activeChatId && !isPendingChatId(activeChatId) ? activeChatId : "",
        page: "1",
        requestIds: requestIds.join(","),
        workspaceId: activeWorkspace?.id ?? activeWorkspaceId,
      };
      setStatsRoutePage(1);
      setStatsRouteFilters(filters);
      setViewMode("stats");
      setIsMobileWorkspaceOpen(false);
      updateBrowserRoute({ filters, page: 1, viewMode: "stats" });
    },
    [activeChatId, activeWorkspace?.id, activeWorkspaceId, updateBrowserRoute],
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

  useEffect(() => {
    if (!canUseApp || isLoading || workspaces.length === 0) {
      return undefined;
    }

    const restorePendingQuestions = () => {
      void checkPendingQuestions(workspaces);
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        restorePendingQuestions();
      }
    };

    restorePendingQuestions();
    window.addEventListener("focus", restorePendingQuestions);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("focus", restorePendingQuestions);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [canUseApp, checkPendingQuestions, isLoading, workspaces]);

  useBrowserPopState(applyBrowserRouteRef);

  async function handleCancelRun() {
    const currentChatKey = activeChatKeyRef.current;
    if (!currentChatKey) {
      return;
    }

    const runInfo = activeRunInfoByChatKeyRef.current[currentChatKey] ?? null;
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
    setChatRunning(currentChatKey, false);
    setActiveRunInfoForChatKey(currentChatKey, null);
    clearLiveChatStatistics(currentChatKey);
    setChatRunFailed(currentChatKey, false);
    const cancelledChat = runInfo?.chatId
      ? { chatId: runInfo.chatId, workspaceId: runInfo.workspaceId }
      : parseChatRunKey(currentChatKey);
    if (cancelledChat) {
      clearWorkspaceChatActiveRun(cancelledChat.workspaceId, cancelledChat.chatId);
    }
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
        ...request.skillIds,
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
            ...(request.assistantDraft ? { assistantDraft: request.assistantDraft } : {}),
            ...(request.assistantDraftReasoning
              ? { assistantDraftReasoning: request.assistantDraftReasoning }
              : {}),
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
      source?: string;
    },
    assistantId: string,
    previousAssistantId: string,
  ) {
    const pendingGuidanceMessageId =
      pendingGuidanceMessageIdsRef.current.get(guidance.id) ?? null;
    pendingGuidanceMessageIdsRef.current.delete(guidance.id);
    const syntheticSource =
      guidance.source === "reasoningLoopGuard"
        ? "reasoningLoopGuard"
        : undefined;
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
              syntheticSource,
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
              syntheticSource,
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

  async function subscribeActiveChatRun(
    activeRun: ActiveChatRunSummary,
    isReconnect = false,
  ) {
    const chatKey = chatRunKey(activeRun.workspaceId, activeRun.chatId);
    const existingAbortController = activeRunAbortByChatKeyRef.current.get(chatKey);
    if (existingAbortController) {
      const existingRunId = activeRunInfoByChatKeyRef.current[chatKey]?.runId;
      if (existingRunId === activeRun.runId && !isReconnect) {
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
    let liveAssistantDraft = "";
    let liveAssistantDraftReasoning = "";
    let lastLiveContextUsageRefreshAtMs = Date.now();
    let hasGuidanceTurns = false;
    let terminalContextUsageRefreshRequested = false;
    const textDeltaBuffer = createTextDeltaBuffer();
    const reasoningDeltaBuffer = createReasoningDeltaBuffer();
    const toolOutputDeltaBuffer = createToolOutputDeltaBuffer();
    const flushStreamDeltaBuffers = () => {
      textDeltaBuffer.flush();
      reasoningDeltaBuffer.flush();
      toolOutputDeltaBuffer.flush();
    };
    const refreshRunContextUsage = (): boolean => {
      const modelId = selectedModelIdRef.current;
      const providerId = selectedProviderIdRef.current;
      if (!modelId || !providerId) {
        return false;
      }

      void refreshContextUsage({
        chatId: activeRun.chatId,
        modelId,
        providerId,
        skillIds: [],
        thinkingLevel: selectedThinkingLevelRef.current,
        workspaceId: activeRun.workspaceId,
      });
      return true;
    };
    const refreshTerminalContextUsage = () => {
      if (terminalContextUsageRefreshRequested) {
        return;
      }

      terminalContextUsageRefreshRequested = refreshRunContextUsage();
    };
    const scheduleLiveContextUsageRefresh = () => {
      if (!liveAssistantDraft && !liveAssistantDraftReasoning) {
        return;
      }
      const now = Date.now();
      if (now - lastLiveContextUsageRefreshAtMs < LIVE_CONTEXT_USAGE_REFRESH_MS) {
        return;
      }
      const modelId = selectedModelIdRef.current;
      const providerId = selectedProviderIdRef.current;
      if (!modelId || !providerId) {
        return;
      }

      lastLiveContextUsageRefreshAtMs = now;
      void refreshContextUsage({
        assistantDraft: liveAssistantDraft,
        assistantDraftReasoning: liveAssistantDraftReasoning,
        chatId: activeRun.chatId,
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
    const finishLiveReasoningDuration = (
      eventAssistantMessageId?: string,
      reasoningDurationMs?: number | null,
    ) => {
      const startedAtMs = activeReasoningStartedAtMs;
      if (startedAtMs === null) {
        return;
      }
      activeReasoningStartedAtMs = null;
      stopLiveReasoningDuration();
      const endedAtMs = Date.now();
      setMessagesForChatKey(chatKey, (current) =>
        current.map((message) => {
          if (!isCurrentAssistantMessage(message, eventAssistantMessageId)) {
            return message;
          }
          const serverParts = finishReasoningPartWithDuration(
            message.parts,
            reasoningDurationMs,
          );
          return {
            ...message,
            parts:
              serverParts === message.parts
                ? finishActiveReasoningPart(message.parts, startedAtMs, endedAtMs)
                : serverParts,
          };
        }),
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

    let lastProcessedSequence = activeRun.lastSequence ?? -1;
    const lastSequenceForState = () =>
      lastProcessedSequence >= 0 ? lastProcessedSequence : null;
    const activeRunWithCurrentSequence = (): ActiveChatRunSummary => ({
      ...activeRun,
      lastSequence: lastSequenceForState(),
    });
    const updateLastProcessedSequence = (sequence: number | null) => {
      if (sequence === null || sequence <= lastProcessedSequence) {
        return;
      }
      lastProcessedSequence = sequence;
      const currentRunInfo = activeRunInfoByChatKeyRef.current[chatKey];
      if (currentRunInfo?.runId === activeRun.runId) {
        setActiveRunInfoForChatKey(chatKey, {
          ...currentRunInfo,
          lastSequence: sequence,
        });
      }
    };

    setChatRunning(chatKey, true);
    setChatRunFailed(chatKey, false);
    setActiveRunInfoForChatKey(chatKey, {
      acceptingGuidance: activeRun.acceptingGuidance,
      chatId: activeRun.chatId,
      chatKey,
      lastSequence: lastSequenceForState(),
      runId: activeRun.runId,
      workspaceId: activeRun.workspaceId,
    });
    activeRunAbortByChatKeyRef.current.set(chatKey, abortController);
    let shouldReconnect = false;

    try {
      const afterSequence = lastProcessedSequence;
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(activeRun.workspaceId)}/chat/runs/${encodeURIComponent(activeRun.runId)}/stream?afterSequence=${encodeURIComponent(String(afterSequence))}`,
        {
          cache: "no-store",
          credentials: "same-origin",
          signal: abortController.signal,
        },
      );
      if (!response.ok) {
        const message = await responseErrorMessage(response);
        if (
          (response.status === 400 || response.status === 404) &&
          isStaleActiveRunError(message)
        ) {
          console.debug("[chat-stream] active run stream is stale; refreshing messages", {
            chatId: activeRun.chatId,
            runId: activeRun.runId,
            status: response.status,
            workspaceId: activeRun.workspaceId,
          });
          setChatRunning(chatKey, false);
          setActiveRunInfoForChatKey(chatKey, null);
          clearLiveChatStatistics(chatKey);
          clearWorkspaceChatActiveRun(activeRun.workspaceId, activeRun.chatId);
          await loadChatMessages(activeRun.workspaceId, activeRun.chatId);
          return;
        }
        console.debug("[chat-stream] active run stream returned backend error", {
          chatId: activeRun.chatId,
          runId: activeRun.runId,
          status: response.status,
          workspaceId: activeRun.workspaceId,
        });
        throw new Error(message);
      }

      await readChatStream(response, (streamEvent, meta) => {
        const eventSequence = meta.id === null ? null : Number(meta.id);
        updateLastProcessedSequence(Number.isFinite(eventSequence) ? eventSequence : null);
        if (streamEvent.type !== "textDelta") {
          textDeltaBuffer.flush();
        }
        if (streamEvent.type !== "reasoningDelta") {
          reasoningDeltaBuffer.flush();
        }
        if (streamEvent.type !== "toolOutputDelta") {
          toolOutputDeltaBuffer.flush();
        }

        if (streamEvent.type === "connecting") {
          return;
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
            lastSequence: lastSequenceForState(),
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          liveStartedAtMs = Date.now();
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
          finishLiveReasoningDuration(
            streamEvent.assistantMessageId,
            streamEvent.reasoningDurationMs,
          );
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          textDeltaBuffer.push(
            chatKey,
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
            streamEvent.delta,
          );
          liveAssistantDraft += streamEvent.delta;
          scheduleLiveContextUsageRefresh();
          return;
        }

        if (streamEvent.type === "reasoningDelta") {
          const reasoningStartedAtMs = startLiveReasoningDuration();
          const targetAssistantMessageId = resolvedAssistantMessageId(
            streamEvent.assistantMessageId,
          );
          ensureStreamingAssistantMessage(targetAssistantMessageId);
          reasoningDeltaBuffer.push(
            chatKey,
            targetAssistantMessageId,
            streamEvent.delta,
            reasoningStartedAtMs,
          );
          liveAssistantDraftReasoning += streamEvent.delta;
          scheduleLiveContextUsageRefresh();
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
            lastSequence: lastSequenceForState(),
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          return;
        }

        if (streamEvent.type === "streamReset") {
          finishLiveReasoningDuration(streamEvent.assistantMessageId);
          latestResponseUsage = null;
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
                  ? assistantMessageWithContextCompression(message, streamEvent)
                  : message,
              ),
            );
            if (streamEvent.status === "completed") {
              refreshRunContextUsage();
            }
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
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          finishLiveReasoningDuration(currentAssistantMessageId);
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          interruptedAssistantMessageId = previousAssistantId;
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
          const liveStatisticsUsage =
            streamEvent.usage &&
              streamEvent.usage.inputTokens !== null &&
              streamEvent.usage.outputTokens !== null
              ? streamEvent.usage
              : latestResponseUsage;
          if (!latestResponseUsage && liveStatisticsUsage) {
            latestResponseUsage = liveStatisticsUsage;
          }
          refreshRunContextUsage();
          updateLiveChatStatistics(chatKey, {
            modelId: streamEvent.metrics.modelId,
            providerId: streamEvent.metrics.providerId,
            startedAtMs: liveStartedAtMs,
            usage: liveStatisticsUsage,
          });
          void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
          void refreshWorkspaces();
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
          finishLiveReasoningDuration(
            streamEvent.assistantMessageId,
            streamEvent.reasoningDurationMs,
          );
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
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                    ),
                    toolCalls: applyToolResult(
                      message.toolCalls,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                    ),
                  }
                  : message,
              );
            });
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          const targetAssistantMessageId = resolvedAssistantMessageId(
            streamEvent.assistantMessageId,
          );
          toolOutputDeltaBuffer.push(chatKey, {
            assistantMessageId: targetAssistantMessageId,
            delta: streamEvent.delta,
            stream: streamEvent.stream,
            toolCallId: streamEvent.toolCallId,
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
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath, sourceControlTarget);
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
          refreshTerminalContextUsage();
          refreshActiveAgentTeamSnapshot(activeRun.workspaceId, activeRun.chatId);
          void refreshMessagesAfterSpecJobSettles(
            activeRun.workspaceId,
            activeRun.chatId,
            activeRun.runId,
          );
          return;
        }

        if (streamEvent.type === "error") {
          console.debug("[chat-stream] active run stream emitted backend error event", {
            chatId: activeRun.chatId,
            message: streamEvent.message,
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          finishLiveReasoningDuration();
          stopLiveReasoningDuration();
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
      }, { signal: abortController.signal });

      await refreshWorkspaces();
    } catch (requestError) {
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      if (isStreamIdleError(requestError)) {
        console.debug("[chat-stream] active run stream idle timeout; reconnecting", {
          chatId: activeRun.chatId,
          lastSequence: lastSequenceForState(),
          runId: activeRun.runId,
          workspaceId: activeRun.workspaceId,
        });
        shouldReconnect = true;
      } else if (wasCancelled) {
        console.debug("[chat-stream] active run stream cancelled", {
          chatId: activeRun.chatId,
          runId: activeRun.runId,
          workspaceId: activeRun.workspaceId,
        });
      } else {
        console.debug("[chat-stream] active run stream failed", {
          chatId: activeRun.chatId,
          message: errorMessage(requestError),
          runId: activeRun.runId,
          workspaceId: activeRun.workspaceId,
        });
        setChatRunFailed(chatKey, true);
        setError(errorMessage(requestError));
      }
    } finally {
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      if (!shouldReconnect) {
        refreshTerminalContextUsage();
      }
      if (activeRunAbortByChatKeyRef.current.get(chatKey) === abortController) {
        activeRunAbortByChatKeyRef.current.delete(chatKey);
        if (shouldReconnect) {
          setChatRunning(chatKey, true);
          setActiveRunInfoForChatKey(chatKey, {
            acceptingGuidance: activeRun.acceptingGuidance,
            chatId: activeRun.chatId,
            chatKey,
            lastSequence: lastSequenceForState(),
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          void subscribeActiveChatRun(activeRunWithCurrentSequence(), true);
        } else {
          setChatRunning(chatKey, false);
          setActiveRunInfoForChatKey(chatKey, null);
          clearLiveChatStatistics(chatKey);
          clearWorkspaceChatActiveRun(activeRun.workspaceId, activeRun.chatId);
        }
      }
    }
  }

  async function runChatMessage(initialRequest: RetryRunRequest): Promise<string | null> {
    const requestModel = availableModels.find(
      (model) => model.id === initialRequest.modelId,
    );
    let request = {
      ...initialRequest,
      thinkingLevel: isModelThinkingLevelSupported(
        requestModel,
        initialRequest.thinkingLevel,
      )
        ? initialRequest.thinkingLevel
        : "",
    };
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
        bindRequestPlanModeToChatKey(request, queuedChatKey);
        setActiveWorkspaceChatRefs(request.workspaceId, queued.chatId);
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
    let liveAssistantDraft = "";
    let liveAssistantDraftReasoning = "";
    let lastLiveContextUsageRefreshAtMs = Date.now();
    let runSucceeded = false;
    let streamHadError = false;
    let hasGuidanceTurns = false;
    let activeRunId: string | null = null;
    let terminalContextUsageRefreshRequested = false;
    const abortController = new AbortController();
    const textDeltaBuffer = createTextDeltaBuffer();
    const reasoningDeltaBuffer = createReasoningDeltaBuffer();
    const toolOutputDeltaBuffer = createToolOutputDeltaBuffer();
    const flushStreamDeltaBuffers = () => {
      textDeltaBuffer.flush();
      reasoningDeltaBuffer.flush();
      toolOutputDeltaBuffer.flush();
    };
    const refreshRunContextUsage = (): boolean => {
      if (!requestChatId) {
        return false;
      }

      void refreshContextUsage({
        chatId: requestChatId,
        modelId: request.modelId,
        providerId: request.providerId,
        skillIds: request.skillIds,
        thinkingLevel: request.thinkingLevel,
        workspaceId: request.workspaceId,
      });
      return true;
    };
    const refreshTerminalContextUsage = () => {
      if (terminalContextUsageRefreshRequested) {
        return;
      }

      terminalContextUsageRefreshRequested = refreshRunContextUsage();
    };
    const scheduleLiveContextUsageRefresh = () => {
      if (!liveAssistantDraft && !liveAssistantDraftReasoning) {
        return;
      }
      const now = Date.now();
      if (now - lastLiveContextUsageRefreshAtMs < LIVE_CONTEXT_USAGE_REFRESH_MS) {
        return;
      }

      lastLiveContextUsageRefreshAtMs = now;
      void refreshContextUsage({
        assistantDraft: liveAssistantDraft,
        assistantDraftReasoning: liveAssistantDraftReasoning,
        chatId: requestChatId,
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
    const finishLiveReasoningDuration = (
      eventAssistantMessageId?: string,
      reasoningDurationMs?: number | null,
    ) => {
      const startedAtMs = activeReasoningStartedAtMs;
      if (startedAtMs === null) {
        return;
      }
      activeReasoningStartedAtMs = null;
      stopLiveReasoningDuration();
      const endedAtMs = Date.now();
      setMessagesForChatKey(runMessagesKey, (current) =>
        current.map((message) => {
          if (!isCurrentAssistantMessage(message, eventAssistantMessageId)) {
            return message;
          }
          const serverParts = finishReasoningPartWithDuration(
            message.parts,
            reasoningDurationMs,
          );
          return {
            ...message,
            parts:
              serverParts === message.parts
                ? finishActiveReasoningPart(message.parts, startedAtMs, endedAtMs)
                : serverParts,
          };
        }),
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
        if (streamEvent.type !== "reasoningDelta") {
          reasoningDeltaBuffer.flush();
        }
        if (streamEvent.type !== "toolOutputDelta") {
          toolOutputDeltaBuffer.flush();
        }

        if (streamEvent.type === "connecting") {
          return;
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
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
          refreshRunContextUsage();
          void refreshWorkspaces();
          return;
        }
        if (streamEvent.type === "textDelta") {
          finishLiveReasoningDuration(
            streamEvent.assistantMessageId,
            streamEvent.reasoningDurationMs,
          );

          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          textDeltaBuffer.push(
            runMessagesKey,
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
            streamEvent.delta,
          );
          liveAssistantDraft += streamEvent.delta;
          scheduleLiveContextUsageRefresh();
          return;
        }

        if (streamEvent.type === "reasoningDelta") {
          const reasoningStartedAtMs = startLiveReasoningDuration();
          const targetAssistantMessageId = resolvedAssistantMessageId(
            streamEvent.assistantMessageId,
          );
          ensureStreamingAssistantMessage(targetAssistantMessageId);
          reasoningDeltaBuffer.push(
            runMessagesKey,
            targetAssistantMessageId,
            streamEvent.delta,
            reasoningStartedAtMs,
          );
          liveAssistantDraftReasoning += streamEvent.delta;
          scheduleLiveContextUsageRefresh();
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
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
                  ? assistantMessageWithContextCompression(message, streamEvent)
                  : message,
              ),
            );
            if (streamEvent.status === "completed") {
              refreshRunContextUsage();
            }
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
          return;
        }

        if (streamEvent.type === "guidanceApplied") {
          finishLiveReasoningDuration(currentAssistantMessageId);
          const previousAssistantId = currentAssistantMessageId;
          const guidanceAssistantId = `${streamEvent.id}-assistant`;
          currentAssistantMessageId = guidanceAssistantId;
          interruptedAssistantMessageId = previousAssistantId;
          liveAssistantDraft = "";
          liveAssistantDraftReasoning = "";
          lastLiveContextUsageRefreshAtMs = Date.now();
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
          const liveStatisticsUsage =
            streamEvent.usage &&
              streamEvent.usage.inputTokens !== null &&
              streamEvent.usage.outputTokens !== null
              ? streamEvent.usage
              : latestResponseUsage;
          if (!latestResponseUsage && liveStatisticsUsage) {
            latestResponseUsage = liveStatisticsUsage;
          }
          refreshRunContextUsage();
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
          void refreshWorkspaces();
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
          finishLiveReasoningDuration(
            streamEvent.assistantMessageId,
            streamEvent.reasoningDurationMs,
          );
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
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                    ),
                    parts: applyToolResultToParts(
                      message.parts,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                    ),
                  }
                  : message,
              );
            });
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          const targetAssistantMessageId = resolvedAssistantMessageId(
            streamEvent.assistantMessageId,
          );
          ensureStreamingAssistantMessage(targetAssistantMessageId);
          toolOutputDeltaBuffer.push(runMessagesKey, {
            assistantMessageId: targetAssistantMessageId,
            delta: streamEvent.delta,
            stream: streamEvent.stream,
            toolCallId: streamEvent.toolCallId,
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
            void loadGitDiff(streamEvent.workspaceId, selectedDiffPath, sourceControlTarget);
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
          refreshTerminalContextUsage();
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
      flushStreamDeltaBuffers();
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
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      stopLiveReasoningDuration();
      refreshTerminalContextUsage();
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
  }

  async function showMoreWorkspaceChats(workspaceId: string) {
    const paging = workspaceChatPaging[workspaceId];
    if (!paging?.hasMore || paging.isLoading) {
      return;
    }

    setWorkspaceChatPaging((current) => ({
      ...current,
      [workspaceId]: { ...current[workspaceId], isLoading: true },
    }));

    try {
      const params = new URLSearchParams({ limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE) });
      if (paging.nextCursor) {
        params.set("cursor", paging.nextCursor);
      }
      const data = await requestJson<WorkspaceChatsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats?${params.toString()}`,
      );
      setWorkspaces((current) =>
        current.map((workspace) => {
          if (workspace.id !== workspaceId) {
            return workspace;
          }
          const existingChatIds = new Set(workspace.chats.map((chat) => chat.id));
          return {
            ...workspace,
            chatPagination: {
              hasMore: data.hasMore,
              limit: data.limit,
              nextCursor: data.nextCursor,
              total: data.total,
            },
            chats: [
              ...workspace.chats,
              ...data.chats.filter((chat) => !existingChatIds.has(chat.id)),
            ],
          };
        }),
      );
      setWorkspaceChatPaging((current) => ({
        ...current,
        [workspaceId]: {
          hasMore: data.hasMore,
          isLoading: false,
          nextCursor: data.nextCursor,
          total: data.total,
        },
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
      setWorkspaceChatPaging((current) => ({
        ...current,
        [workspaceId]: { ...current[workspaceId], isLoading: false },
      }));
    }
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
    setWorkspaceSpecEnabled(false);
    setWorkspaceTerminalShell("");
    setWorkspaceMode("local");
    setWorkspaceServerId(settings?.remoteServers[0]?.id ?? "");
    setWorkspaceTestStages([]);
    setInlineRemoteServerName("");
    setInlineRemoteServerHost("");
    setError(null);
    setWorkspaceDialogRevision((current) => current + 1);
    setIsWorkspaceDialogOpen(true);
  }

  function closeWorkspaceDialog() {
    setWorkspaceIconDraft(null);
    setWorkspaceTestStages([]);
    setWorkspaceDialogRevision((current) => current + 1);
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
    setUpdateStatus(data.update);
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
      formatReplyDuration: (value, nextLanguage) =>
        formatReplyDuration(value, nextLanguage as AppLanguageId),
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
        workspaces={workspaces}
      />
    ),
    [activeWorkspaceId, workspaces],
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
    () => handleSelectDraftAttachments(),
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
  const refreshAgentPanelForContextPanel = useStableCallback(async () => {
    if (activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)) {
      await loadAgentTeamSnapshot(activeWorkspaceId, activeChatId, { silent: false });
    }
  });
  const openAgentInstanceTabForContextPanel = useStableCallback(openAgentInstanceTab);
  const agentsPanelForContextPanel = useMemo(
    () => (
      <Suspense fallback={<PanelLoadingFallback />}>
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
      </Suspense>
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
  const sourceControlTargetOptions = useMemo(
    () =>
      availableSourceControlTargets.map((target) => ({
        description: target.path ?? activeWorkspace?.path ?? "",
        key: sourceControlTargetKey(target),
        label: target.label,
      })),
    [activeWorkspace?.path, availableSourceControlTargets],
  );
  const handleSourceControlTargetChange = useStableCallback((targetKey: string) => {
    if (targetKey === sourceControlTargetKeyValue) {
      return;
    }
    const target = sourceControlTargetFromKey(availableSourceControlTargets, targetKey);
    if (!target) {
      return;
    }
    setIsSourceControlTargetManual(true);
    setSelectedSourceControlTargetScope(sourceControlTargetScope);
    setSelectedSourceControlTarget(target);
    setSelectedDiffPath(null);
  });
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
      void loadGitDiff(activeWorkspace.id, selectedDiffPath, sourceControlTarget);
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
    setContextPanelTab(tab);
    setIsContextPanelOpen(true);
  });
  const contextPanelFiles = gitDiff?.files ?? EMPTY_GIT_STATUS_FILES;
  const normalizedWorkspaceChatSearchQuery = workspaceChatSearchQuery.trim();
  const isWorkspaceSearchActive =
    workspaceChatSearchOpen && normalizedWorkspaceChatSearchQuery.length > 0;

  useEffect(() => {
    if (!isWorkspaceSearchActive) {
      setWorkspaceChatSearchResults([]);
      setWorkspaceChatSearchError(null);
      setIsSearchingWorkspaceChats(false);
      return;
    }

    const abortController = new AbortController();

    setIsSearchingWorkspaceChats(true);
    setWorkspaceChatSearchError(null);

    void requestJson<WorkspaceChatSearchResponse>(
      `/api/workspaces/search-chats?query=${encodeURIComponent(normalizedWorkspaceChatSearchQuery)}&limit=${WORKSPACE_CHAT_HISTORY_PAGE_SIZE}`,
      { signal: abortController.signal },
    )
      .then((data) => {
        setWorkspaceChatSearchResults(data.workspaces);
      })
      .catch((requestError) => {
        if (isAbortError(requestError)) {
          return;
        }

        setWorkspaceChatSearchResults([]);
        setWorkspaceChatSearchError(errorMessage(requestError));
      })
      .finally(() => {
        if (!abortController.signal.aborted) {
          setIsSearchingWorkspaceChats(false);
        }
      });

    return () => abortController.abort();
  }, [isWorkspaceSearchActive, normalizedWorkspaceChatSearchQuery]);

  const sidebarWorkspaces = isWorkspaceSearchActive
    ? workspaceChatSearchResults
    : displayedWorkspaces;
  const updateNavButton =
    updateStatus?.updateAvailable && !updateStatus.error
      ? {
          active: false,
          disabled: isInstallingUpdate,
          icon: isInstallingUpdate ? LoaderCircle : Download,
          label: isInstallingUpdate ? t("Installing update...") : t("Install update"),
          onClick: () => void installUpdateFromNav(),
        }
      : null;

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
      <main className="app-root foco-workbench">
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
        {updateInstallNotice ? (
          <section
            aria-live="polite"
            className="app-status-toast"
            role="status"
          >
            <CheckCircle2 aria-hidden="true" className="app-status-toast-icon" />
            <div className="app-error-toast-message">{updateInstallNotice}</div>
            <button
              aria-label={t("Dismiss update message")}
              className="app-status-toast-close"
              onClick={() => setUpdateInstallNotice(null)}
              title={t("Close")}
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
              onOpenSkillStore={openSkillStoreView}
              onOpenStats={openStatsView}
              onReturnHome={handleLogoNavClick}
              onToggleTheme={() =>
                void saveAppTheme(theme === "dark" ? "light" : "dark")
              }
              terminalButton={null}
              theme={theme}
              updateButton={updateNavButton}
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
                  activeWorkspaceId={activeWorkspace?.id ?? activeWorkspaceId ?? null}
                  activeSection={settingsSection}
                  isLoadingAgentDefinitions={isLoadingAgentDefinitions}
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
              ) : viewMode === "skill-store" ? (
                <SkillStorePage
                  onSettingsChange={handleSettingsPanelSettingsChange}
                  onWorkspacesChange={refreshWorkspaces}
                  settings={settings}
                  workspaces={workspaces}
                />
              ) : (
                <ApiStatsPanel
                  initialFilters={statsRouteFilters}
                  onRouteChange={updateStatsRoute}
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
              onOpenSkillStore={openSkillStoreView}
              onOpenStats={openStatsView}
              onHomeClick={handleHomeNavClick}
              onReturnHome={handleLogoNavClick}
              onToggleTheme={() =>
                void saveAppTheme(theme === "dark" ? "light" : "dark")
              }
              theme={theme}
              updateButton={updateNavButton}
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
                      className="inline-flex size-8 items-center justify-center rounded-lg text-stone-600 transition hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400 disabled:hover:bg-transparent"
                      disabled={isLoading}
                      onClick={() => void refreshWorkspaces()}
                      title={t("Refresh workspaces")}
                      type="button"
                    >
                      <RefreshCw
                        aria-hidden="true"
                        className={`size-3.5 ${isLoading ? "animate-spin" : ""}`}
                      />
                    </button>
                    <button
                      aria-label={t("Search chats")}
                      aria-pressed={workspaceChatSearchOpen}
                      className="inline-flex size-8 items-center justify-center rounded-lg text-stone-600 transition hover:bg-teal-50 hover:text-teal-800"
                      onClick={() => setWorkspaceChatSearchOpen((current) => !current)}
                      title={t("Search chats")}
                      type="button"
                    >
                      <Search aria-hidden="true" className="size-3.5" />
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
                    <div className="relative">
                      <input
                        aria-label={t("Search chats")}
                        className="workspace-chat-search-input h-9 w-full rounded-lg border border-stone-300 bg-white px-3 pr-8 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) => setWorkspaceChatSearchQuery(event.target.value)}
                        placeholder={t("Search chats placeholder")}
                        type="search"
                        value={workspaceChatSearchQuery}
                      />
                      {workspaceChatSearchQuery.length ? (
                        <button
                          aria-label={t("Clear search")}
                          className="absolute right-2 top-1/2 inline-flex size-5 -translate-y-1/2 items-center justify-center rounded-full text-stone-400 hover:bg-stone-100 hover:text-stone-700"
                          onClick={() => setWorkspaceChatSearchQuery("")}
                          title={t("Clear search")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-3.5" />
                        </button>
                      ) : null}
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
                      const workspaceChats = isWorkspaceSearchActive
                        ? workspace.chats.map(
                          (chat): WorkspaceChatListItem => ({
                            ...chat,
                            scheduledStatus:
                              chat.queuedRun?.status === "queued" ? "queued" : undefined,
                          }),
                        )
                        : workspaceChatListItemsFor(workspace);
                      const paging = workspaceChatPaging[workspace.id];
                      const visibleChats = workspaceChats;
                      const hiddenChatCount = isWorkspaceSearchActive
                        ? 0
                        : Math.max((paging?.total ?? workspace.chats.length) - workspace.chats.length, 0);
                      const nextVisibleChatCount = Math.min(
                        WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
                        hiddenChatCount,
                      );
                      const isRemoteWorkspace = workspace.serverId !== null;
                      const isRemoteReady = workspaceConnectionLooksReady(workspace.connectionStatus);

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
                              <span className="relative inline-flex shrink-0">
                                <WorkspaceIcon
                                  className="size-4 shrink-0 rounded object-cover"
                                  fallbackClassName="size-4 shrink-0"
                                  isRemote={isRemoteWorkspace}
                                  logoUrl={workspace.logoUrl}
                                />
                                {isRemoteWorkspace ? (
                                  <span className={`absolute -bottom-0.5 -right-0.5 size-2 rounded-full border border-white ${workspaceConnectionDotClass(workspace.connectionStatus)}`} />
                                ) : null}
                              </span>
                              <span className="min-w-0 flex-1 text-left">
                                <span className="block truncate">{workspace.name}</span>
                                <span className="block truncate text-[10px] font-medium leading-3 text-stone-400">
                                  {workspace.displayPath}
                                </span>
                              </span>
                            </button>
                            <button
                              aria-label={t("New chat in {name}", {
                                name: workspace.name,
                              })}
                              className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg text-stone-500 hover:text-teal-800"
                              disabled={isRemoteWorkspace && !isRemoteReady}
                              onClick={() => {
                                if (isRemoteWorkspace && !isRemoteReady) {
                                  setError(t("Remote workspace is offline. Retry the connection before opening remote operations."));
                                  return;
                                }
                                startNewWorkspaceChat(workspace.id);
                              }}
                              title={isRemoteWorkspace && !isRemoteReady ? t("Remote workspace is offline") : t("New chat")}
                              type="button"
                            >
                              <Plus aria-hidden="true" className="size-4" />
                            </button>
                          </div>
                          {isRemoteWorkspace && !isRemoteReady ? (
                            <div className="ml-9 mt-1 flex items-center gap-2 pr-1.5 text-[11px] leading-4 text-stone-500">
                              {workspace.lastRemoteError ? (
                                <span className="min-w-0 flex-1 truncate">
                                  {workspace.lastRemoteError}
                                </span>
                              ) : null}
                              <button
                                className="inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-semibold text-teal-800 hover:border-teal-200 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-400"
                                disabled={retryingRemoteWorkspaceId === workspace.id}
                                onClick={() => void retryRemoteWorkspace(workspace)}
                                type="button"
                              >
                                {retryingRemoteWorkspaceId === workspace.id ? (
                                  <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />
                                ) : (
                                  <RefreshCw aria-hidden="true" className="size-3" />
                                )}
                                {t("Retry")}
                              </button>
                            </div>
                          ) : null}
                          {isExpanded ? (
                            <div className="mt-1 space-y-1 border-l border-stone-200/80 pl-3 pr-1.5">
                              {visibleChats.length > 0 ? (
                                <>
                                  {visibleChats.map((chat) => {
                                    const chatKey = chatRunKey(workspace.id, chat.id);
                                    const scheduledChatKey =
                                      chat.scheduledChatKey ?? null;
                                    const sessionStatus = chatSessionStatusFor(chatKey, {
                                      scheduledChatKey,
                                      scheduledStatus: chat.scheduledStatus ?? null,
                                      workspaceActiveRun: chat.activeRun,
                                    });
                                    const statusDotClass = chatSessionStatusDotClass(sessionStatus.kind);
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
                                      disabled={paging?.isLoading}
                                      onClick={() =>
                                        void showMoreWorkspaceChats(workspace.id)
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
                        ? isSearchingWorkspaceChats
                          ? t("Searching chats...")
                          : workspaceChatSearchError ?? t("No matching chats")
                        : isLoading
                          ? t("Loading workspaces...")
                          : t("No workspaces")}
                    </div>
                  )}
                </nav>
                <ModelRoutingPanel
                  models={settings?.configuredModels ?? []}
                  onRouteChange={updateModelRoute}
                  providers={settings?.providers ?? EMPTY_CONFIGURED_PROVIDERS}
                />
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
                {workspaceFileContextMenu.node.kind === "file" ? (
                  <button
                    className="workspace-chat-context-menu-item"
                    onClick={() => {
                      const { node } = workspaceFileContextMenu;
                      setWorkspaceFileContextMenu(null);
                      downloadWorkspaceFile(node);
                    }}
                    role="menuitem"
                    type="button"
                  >
                    <Download aria-hidden="true" className="size-3.5" />
                    <span>{t("Download")}</span>
                  </button>
                ) : null}
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
                    chatSessionStatusFor={chatSessionStatusFor}
                    onCloseTab={closeMainTab}
                    onCloseTabs={closeMainTabs}
                    onSelectTab={selectMainTab}
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
                <Suspense fallback={<PanelLoadingFallback />}>
                  <AgentTranscriptPanel
                    key={agentTranscriptViewCacheKey(
                      activeAgentTab.workspaceId,
                      activeAgentTab.chatId,
                      activeAgentTab.instanceId,
                    )}
                    chatId={activeAgentTab.chatId}
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
                        { silent: false },
                      );
                    }}
                    readTranscriptCache={() =>
                      agentTranscriptViewCacheRef.current.get(
                        agentTranscriptViewCacheKey(
                          activeAgentTab.workspaceId,
                          activeAgentTab.chatId,
                          activeAgentTab.instanceId,
                        ),
                      ) ?? null
                    }
                    snapshot={
                      agentTeamSnapshot?.team.chatId === activeAgentTab.chatId
                        ? agentTeamSnapshot
                        : agentTeamSnapshotCacheRef.current.get(
                            chatRunKey(
                              activeAgentTab.workspaceId,
                              activeAgentTab.chatId,
                            ),
                          ) ?? null
                    }
                    writeTranscriptCache={(entry) => {
                      const tabStillOpen = openAgentTabsRef.current.some(
                        (tab) =>
                          tab.workspaceId === activeAgentTab.workspaceId &&
                          tab.chatId === activeAgentTab.chatId &&
                          tab.instanceId === activeAgentTab.instanceId,
                      );
                      if (!tabStillOpen) {
                        return;
                      }
                      agentTranscriptViewCacheRef.current.set(
                        agentTranscriptViewCacheKey(
                          activeAgentTab.workspaceId,
                          activeAgentTab.chatId,
                          activeAgentTab.instanceId,
                        ),
                        entry,
                      );
                    }}
                    workspaceId={activeAgentTab.workspaceId}
                  />
                </Suspense>
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
                  draftAttachments={draftAttachments}
                  draftMessage={draftMessage}
                  draftUnsupportedAttachmentMessage={unsupportedDraftAttachmentMessage}
                  gitBranches={gitBranches}
                  contextUsage={displayedContextUsage}
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
                  onBranchMenuOpen={() => {
                    if (activeWorkspace?.id) {
                      void loadGitBranches(activeWorkspace.id);
                    }
                  }}
                  onDraftMessageChange={setDraftMessage}
                  onEditMessage={handleEditChatMessage}
                  onGuideQueuedMessage={handleGuideQueuedMessageForChatPanel}
                  onLoadMoreMessages={() => {
                    if (!activeWorkspaceId || !activeChatId || isPendingChatId(activeChatId)) {
                      return Promise.resolve();
                    }
                    return loadOlderChatMessages(activeWorkspaceId, activeChatId);
                  }}
                  onSelectAttachments={handleSelectDraftAttachmentsForChatPanel}
                  onSelectEditAttachments={handleSelectEditAttachments}
                  onCancelRun={handleCancelRunForChatPanel}
                  onGuideActiveRun={handleGuideActiveRunForChatPanel}
                  onQueueActiveRun={handleQueueActiveRunForChatPanel}
                  onModelChange={handleModelChangeForChatPanel}
                  onOpenMessageApiRequests={handleOpenMessageApiRequests}
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
                  selectedSkillIds={selectedSkillIds}
                  selectedThinkingLevel={selectedThinkingLevel}
                  settings={settings}
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
                contextUsage={displayedContextUsage}
                deletingContextMemoryId={deletingContextMemoryId}
                contextMemoryError={contextMemoryError}
                diffError={diffError}
                diffPanelWidth={diffPanelWidth}
                diffResponse={gitDiff}
                files={contextPanelFiles}
                gitCommitMessage={gitCommitMessage}
                gitOperationKey={gitOperationKey}
                sourceControlTargetKey={sourceControlTargetKeyValue}
                sourceControlTargetLabel={sourceControlTarget?.label ?? t("Workspace changes")}
                sourceControlTargets={sourceControlTargetOptions}
                expandedFileTreePaths={expandedFileTreePaths}
                isLoadingChatStatistics={isLoadingChatStatistics}
                isLoadingDiff={isLoadingDiff}
                isLoadingContextMemories={isLoadingContextMemories}
                isLoadingPlans={isLoadingActivePlans}
                isPlanAutoRunBusy={isPlanAutoRunBusy || isPlanAutoRunUpdating || planOperationKey !== null}
                isPlanAutoRunEnabled={isPlanAutoRunEnabled}
                planAutoRunBlockedReason={planAutoRunBlockedReason}
                isPlanAutoRunToggleDisabled={!activeWorkspace?.id}
                runtimeToolStateCompressionEnabled={
                  settings?.general.runtimeToolStateCompressionEnabled ?? false
                }
                isLoadingTodoGraph={isLoadingTodoGraph}
                isLoadingWorkspaceSpec={isLoadingWorkspaceSpec}
                isLoadingWorkspaceFiles={isLoadingWorkspaceFiles}
                isResizing={isResizingDiffPanel}
                loadingWorkspaceDirectoryPaths={loadingWorkspaceDirectoryPaths}
                onGitCommit={handleGitCommit}
                onGenerateGitCommitMessage={handleGenerateGitCommitMessageForContextPanel}
                onGitCommitMessageChange={setGitCommitMessage}
                onGitFileOperation={handleGitFileOperationForContextPanel}
                onSourceControlTargetChange={handleSourceControlTargetChange}
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
                onLoadPlanWorktreeAudit={() => {
                  const workspaceId = activeWorkspace?.id;
                  if (!workspaceId) {
                    return Promise.resolve({ items: [], recoveryNote: "" });
                  }
                  return loadPlanWorktreeAudit(workspaceId);
                }}
                onCleanupPlanWorktree={(agentInstanceId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (!workspaceId) {
                    return Promise.resolve();
                  }
                  return cleanupPlanWorktree(workspaceId, agentInstanceId);
                }}
                onOpenPlanPhaseChat={(chatId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    selectWorkspaceChat(workspaceId, chatId);
                  }
                }}
                onPlanPhaseRetry={(planId, phaseId, implementationChatId) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void runPlanPhaseRetry(
                      workspaceId,
                      planId,
                      phaseId,
                      implementationChatId,
                    );
                  }
                }}
                onPlanPhaseRetryWithOverride={(planId, phaseId, implementationChatId, override) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void runPlanPhaseRetry(
                      workspaceId,
                      planId,
                      phaseId,
                      implementationChatId,
                      override,
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
                onPlanOrderChange={(planIds) => {
                  const workspaceId = activeWorkspace?.id;
                  if (workspaceId) {
                    void savePlanOrder(workspaceId, planIds, activePlans);
                  }
                }}
                selectedPath={selectedDiffPath}
                selectedSkillPrefix={selectedSkillPrefix}
                setMobileHeight={setContextPanelMobileHeight}
                setWidth={setDiffPanelWidth}
                onResizeStart={() => setIsResizingDiffPanel(true)}
                todoGraph={todoGraph}
                availableModels={availableModels}
                plans={activePlans}
                providers={providersForChatPanel}
                thinkingLevels={thinkingLevels}
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
            iconDraft={workspaceIconDraft}
            inlineServerHost={inlineRemoteServerHost}
            inlineServerName={inlineRemoteServerName}
            isCreatingInlineServer={isCreatingInlineRemoteServer}
            isSaving={isSavingWorkspace}
            isTestingConnection={isTestingWorkspaceConnection}
            mode={workspaceMode}
            name={workspaceName}
            onClearIcon={clearWorkspaceIconDraft}
            onClose={closeWorkspaceDialog}
            onCreateInlineServer={() => void createInlineRemoteServer()}
            onInlineServerHostChange={setInlineRemoteServerHost}
            onInlineServerNameChange={setInlineRemoteServerName}
            onModeChange={(nextMode) => {
              setWorkspaceMode(nextMode);
              setWorkspaceTestStages([]);
              if (nextMode === "ssh" && !workspaceServerId) {
                setWorkspaceRemoteServer(settings?.remoteServers[0]?.id ?? "");
              }
            }}
            onNameChange={setWorkspaceName}
            onPathChange={workspaceMode === "ssh" ? setWorkspaceRemotePath : setWorkspacePath}
            onSelectPath={handleSelectWorkspacePath}
            onSelectIcon={handleSelectWorkspaceIcon}
            onServerChange={setWorkspaceRemoteServer}
            onSpecEnabledChange={setWorkspaceSpecEnabled}
            onSubmit={handleWorkspaceSubmit}
            onTerminalShellChange={setWorkspaceTerminalShell}
            onTestConnection={() => void testWorkspaceRemoteConnection()}
            path={workspacePath}
            remoteServers={settings?.remoteServers ?? []}
            selectedServerId={workspaceServerId}
            specEnabled={workspaceSpecEnabled}
            terminalShell={workspaceTerminalShell}
            testStages={workspaceTestStages}
          />
        ) : null}
        {filePickerRequest ? (
          <FilePickerDialog
            initialPath={filePickerRequest.initialPath}
            mode={filePickerRequest.mode}
            multiple={filePickerRequest.multiple}
            open={true}
            readFiles={filePickerRequest.readFiles}
            target={filePickerRequest.target}
            title={filePickerRequest.title}
            t={t}
            onClose={() => setFilePickerRequest(null)}
            onSelect={(selection) => {
              setFilePickerRequest(null);
              filePickerRequest.onSelect(selection);
            }}
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
  chatSessionStatusFor,
  onCloseTab,
  onCloseTabs,
  onSelectTab,
  tabs,
}: {
  activeTab: ActiveMainTab;
  chatSessionStatusFor: (chatKey: string) => ChatSessionStatus;
  onCloseTab: (tab: MainTabSummary) => void;
  onCloseTabs: (scope: MainTabCloseScope, anchorTab: MainTabSummary) => void;
  onSelectTab: (tab: MainTabSummary) => void;
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

    return () => {
      window.removeEventListener("pointerdown", closeContextMenuForPointer);
      window.removeEventListener("keydown", closeContextMenuForKey);
      window.removeEventListener("resize", closeContextMenu);
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

  function handleTabListScroll() {
    updateScrollState();
    if (contextMenu) {
      setContextMenu(null);
    }
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

  function hasClosableTabs(scope: MainTabCloseScope, anchorTab: MainTabSummary) {
    const anchorIndex = tabs.findIndex((tab) => mainTabKey(tab) === mainTabKey(anchorTab));
    if (anchorIndex < 0) {
      return false;
    }

    return tabs.some((tab, index) => {
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
        onScroll={handleTabListScroll}
        onWheel={handleWheel}
        ref={tabListRef}
        role="tablist"
      >
        {tabs.length ? (
          tabs.map((tab) => {
            const isActive = mainTabMatches(activeTab, tab);
            const isRunning =
              tab.type === "chat" &&
              chatSessionStatusFor(chatRunKey(tab.workspaceId, tab.chatId)).kind === "running";
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
                    {isRunning ? (
                      <span aria-label={t("Chat is running")} className="inline-flex shrink-0" role="status">
                        <LoaderCircle
                          aria-hidden="true"
                          className="chat-tab-running-spinner size-3.5 animate-spin text-teal-700"
                        />
                      </span>
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
                  <button
                    aria-label={t("Close chat tab {title}", { title })}
                    className="inline-flex size-7 items-center justify-center rounded-md text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100 max-[767px]:opacity-100 max-[767px]:focus:opacity-100 max-[767px]:group-hover:opacity-100"
                    onClick={() => onCloseTab(tab)}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-3.5" />
                  </button>
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
  onOpenSkillStore,
  onOpenStats,
  onReturnHome,
  onToggleTheme,
  terminalButton,
  theme,
  updateButton,
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
  onOpenSkillStore: () => void;
  onOpenStats: () => void;
  onReturnHome: () => void;
  onToggleTheme: () => void;
  terminalButton: NavRailAction | null;
  theme: AppThemeId;
  updateButton: NavRailAction | null;
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
          active={activeMode === "skill-store"}
          icon={ShoppingBag}
          label={t("Skill Store")}
          onClick={onOpenSkillStore}
        />
        <NavRailButton
          active={activeMode === "settings"}
          icon={Settings}
          label={t("Settings")}
          onClick={onOpenSettings}
        />
      </div>
      <div className="foco-nav-rail-bottom">
        {updateButton ? <NavRailButton {...updateButton} /> : null}
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
  workspaces,
}: {
  activeWorkspaceId: string;
  workspaces: WorkspaceSummary[];
}) {
  const { t } = useI18n();
  const selectedWorkspace =
    workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
    workspaces[0] ??
    null;

  return (
    <section className="api-overview-panel grid min-h-[18rem] w-full place-items-center px-4 py-10 text-center">
      <div className="flex max-w-full flex-col items-center gap-4">
        <span className="inline-flex size-20 items-center justify-center overflow-hidden rounded-2xl text-teal-800">
          <WorkspaceIcon
            className="size-20 rounded-2xl object-cover"
            fallbackClassName="size-10"
            isRemote={Boolean(selectedWorkspace?.serverId)}
            logoUrl={selectedWorkspace?.logoUrl}
          />
        </span>
        <div className="min-w-0">
          <span className="foco-eyebrow">{t("Workspace")}</span>
          <h2 className="foco-display mt-1 truncate text-3xl leading-tight text-stone-950">
            {selectedWorkspace?.name ?? t("No workspace selected")}
          </h2>
        </div>
      </div>
    </section>
  );
}

function workspaceConnectionLooksReady(status: string) {
  const normalized = status.toLowerCase();
  return normalized === "connected" || normalized === "ready" || normalized === "degraded";
}

function workspaceConnectionDotClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return "bg-emerald-500";
  }
  if (normalized === "checking" || normalized === "connecting" || normalized === "reconnecting") {
    return "bg-amber-500";
  }
  if (normalized === "failed" || normalized === "failedauth") {
    return "bg-rose-500";
  }
  if (normalized === "degraded") {
    return "bg-yellow-500";
  }
  return "bg-stone-300";
}

function PanelLoadingFallback() {
  return (
    <div className="grid h-full w-full place-items-center p-8 text-stone-400">
      <LoaderCircle aria-hidden="true" className="size-6 animate-spin" />
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
    startedAt: normalizedToolCall.startedAt ?? currentToolCall.startedAt,
    completedAt: keepExistingOutcome
      ? currentToolCall.completedAt
      : normalizedToolCall.completedAt ?? currentToolCall.completedAt,
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
  startedAt?: string | null,
  completedAt?: string | null,
) {
  return toolCalls.map((toolCall) =>
    toolCall.id === toolCallId
      ? {
        ...toolCall,
        output,
        isError,
        status: isError ? "error" : "completed",
        startedAt: startedAt ?? toolCall.startedAt ?? null,
        completedAt: completedAt ?? toolCall.completedAt ?? null,
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

function contextCompressionBadge(kind: ChatContextCompressionKind): ChatRunBadge {
  if (kind === "llm") {
    return "contextCompressionLlm";
  }
  if (kind === "runtimeToolState") {
    return "contextCompressionRuntime";
  }
  return "contextCompressionRule";
}

function assistantMessageWithContextCompression(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "contextCompression" }>,
): ShellMessage {
  const messageWithBadge = addChatRunBadge(
    message,
    contextCompressionBadge(streamEvent.kind),
  );
  return {
    ...messageWithBadge,
    parts: upsertContextCompressionPart(messageWithBadge.parts, streamEvent),
  };
}

function contextCompressionEventPart(
  streamEvent: Extract<ChatStreamEvent, { type: "contextCompression" }>,
): ChatContextCompressionPart {
  const detail = normalizedContextCompressionDetail({
    ...(streamEvent.detail ?? {}),
    status: streamEvent.detail?.status ?? streamEvent.status,
    kind: streamEvent.detail?.kind ?? streamEvent.kind,
    snapshotId: streamEvent.detail?.snapshotId ?? streamEvent.snapshotId ?? null,
  });
  return {
    type: "contextCompression",
    id: contextCompressionPartId(streamEvent.kind, detail),
    status: streamEvent.status,
    kind: streamEvent.kind,
    detail,
  };
}

function contextCompressionPartId(
  kind: ChatContextCompressionKind,
  detail: ChatContextCompressionDetail,
) {
  return detail.snapshotId ?? `${kind}:${detail.startedAt ?? "pending"}`;
}

function upsertContextCompressionPart(
  parts: ChatMessagePart[],
  streamEvent: Extract<ChatStreamEvent, { type: "contextCompression" }>,
): ChatMessagePart[] {
  const nextPart = contextCompressionEventPart(streamEvent);
  const existingIndex = parts.findIndex((part) => {
    if (part.type !== "contextCompression") {
      return false;
    }
    return part.id === nextPart.id || contextCompressionPartsMatch(part, nextPart);
  });

  if (existingIndex === -1) {
    return [...parts, nextPart];
  }

  return parts.map((part, index) =>
    index === existingIndex && part.type === "contextCompression"
      ? mergeContextCompressionPart(part, nextPart)
      : part,
  );
}

function contextCompressionPartsMatch(
  current: ChatContextCompressionPart,
  next: ChatContextCompressionPart,
) {
  if (current.kind !== next.kind) {
    return false;
  }
  if (
    current.detail.startedAt &&
    next.detail.startedAt &&
    current.detail.startedAt === next.detail.startedAt
  ) {
    return true;
  }
  return (
    current.status === "start" &&
    next.status === "completed" &&
    !current.detail.snapshotId
  );
}

function mergeContextCompressionPart(
  current: ChatContextCompressionPart,
  next: ChatContextCompressionPart,
): ChatContextCompressionPart {
  const detail = normalizedContextCompressionDetail({
    ...current.detail,
    ...next.detail,
    snapshotId: next.detail.snapshotId ?? current.detail.snapshotId ?? null,
    originalTokenCount:
      next.detail.originalTokenCount ?? current.detail.originalTokenCount ?? null,
    summaryTokenCount:
      next.detail.summaryTokenCount ?? current.detail.summaryTokenCount ?? null,
    startedAt: next.detail.startedAt ?? current.detail.startedAt ?? null,
    completedAt: next.detail.completedAt ?? current.detail.completedAt ?? null,
    providerId: next.detail.providerId ?? current.detail.providerId ?? null,
    modelId: next.detail.modelId ?? current.detail.modelId ?? null,
  });
  return {
    ...next,
    id: detail.snapshotId ?? current.id,
    detail,
  };
}

function normalizedContextCompressionDetail(
  detail: ChatContextCompressionDetail,
): ChatContextCompressionDetail {
  return {
    status: detail.status,
    kind: detail.kind,
    snapshotId: detail.snapshotId ?? null,
    originalTokenCount: detail.originalTokenCount ?? null,
    summaryTokenCount: detail.summaryTokenCount ?? null,
    startedAt: detail.startedAt ?? null,
    completedAt: detail.completedAt ?? null,
    providerId: detail.providerId ?? null,
    modelId: detail.modelId ?? null,
  };
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
    const serverParts = finishReasoningPartWithDuration(
      parts,
      streamEvent.reasoningDurationMs,
    );
    parts = serverParts === parts
      ? finishActiveReasoningPart(parts, activeReasoningStartedAtMs, completedAtMs)
      : serverParts;
  } else {
    parts = finishReasoningPartWithDuration(parts, streamEvent.reasoningDurationMs);
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
  const parts = (() => {
    if (activeReasoningStartedAtMs !== null) {
      const serverParts = finishReasoningPartWithDuration(
        message.parts,
        streamEvent.reasoningDurationMs,
      );
      return serverParts === message.parts
        ? finishActiveReasoningPart(
          message.parts,
          activeReasoningStartedAtMs,
          completedAtMs,
        )
        : serverParts;
    }
    return finishReasoningPartWithDuration(
      message.parts,
      streamEvent.reasoningDurationMs,
    );
  })();

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

function finishReasoningPartWithDuration(
  parts: ChatMessagePart[],
  durationMs: number | null | undefined,
): ChatMessagePart[] {
  if (typeof durationMs !== "number" || !Number.isFinite(durationMs)) {
    return parts;
  }
  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "reasoning" || lastPart.durationMs !== undefined) {
    return parts;
  }

  return [
    ...parts.slice(0, -1),
    {
      type: "reasoning",
      text: lastPart.text,
      durationMs: Math.max(0, durationMs),
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
  startedAt?: string | null,
  completedAt?: string | null,
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
          startedAt: startedAt ?? part.toolCall.startedAt ?? null,
          completedAt: completedAt ?? part.toolCall.completedAt ?? null,
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
      if (part.type === "userInterruption") {
        return part.content;
      }
      if (part.type === "attachment") {
        return part.attachment.path ?? part.attachment.name;
      }
      if (part.type === "contextCompression") {
        return `context compression ${part.kind} ${part.status}`.trim();
      }
      if (part.type === "toolCall") {
        return `${part.toolCall.name} ${part.toolCall.status}`.trim();
      }
      return "";
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
    startedAt: toolCall.startedAt ?? null,
    completedAt: toolCall.completedAt ?? null,
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
    requestKindBreakdown: [],
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

function contextUsageWithLatestProviderUsage(
  usage: ContextUsageResponse,
  latestProviderUsage: ChatUsage | null,
): ContextUsageResponse {
  const inputTokens = latestProviderUsage?.inputTokens;
  if (typeof inputTokens !== "number" || inputTokens < 0 || usage.contextWindow <= 0) {
    return usage;
  }

  const segments = contextUsageSegmentsForProviderInput(usage, inputTokens);
  return {
    ...usage,
    compressionSnapshotTokens: segments.compressionSnapshot,
    historyTokens: segments.history,
    segments,
    systemPromptTokens: segments.systemPrompt,
    toolSchemaTokens: segments.toolSchema,
    totalUsedContextTokens: inputTokens,
    usagePercent: Math.ceil((inputTokens * 100) / usage.contextWindow),
  };
}

function contextUsageSegmentsForProviderInput(
  usage: ContextUsageResponse,
  inputTokens: number,
) {
  const segments = { ...usage.segments };
  const estimatedTokens =
    segments.systemPrompt +
    segments.toolSchema +
    segments.compressionSnapshot +
    segments.history;
  let tokensToRemove = Math.max(0, estimatedTokens - inputTokens);
  for (const key of ["history", "compressionSnapshot", "toolSchema", "systemPrompt"] as const) {
    if (tokensToRemove <= 0) {
      break;
    }
    const removedTokens = Math.min(segments[key], tokensToRemove);
    segments[key] -= removedTokens;
    tokensToRemove -= removedTokens;
  }
  if (inputTokens > estimatedTokens) {
    segments.history += inputTokens - estimatedTokens;
  }
  return segments;
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
    contextUsageTimeline: [],
  };
}

function normalizeChatStatistics(
  payload: Partial<ChatStatisticsResponse> | null | undefined,
  workspaceId: string,
  chatId: string,
): ChatStatisticsResponse {
  const base = emptyChatStatistics(workspaceId, chatId, emptyAiStatisticsSummary());
  const codeChangeStats = isObjectRecord(payload?.codeChangeStats)
    ? (payload.codeChangeStats as Partial<GitDiffLineStats>)
    : {};
  const compression = isObjectRecord(payload?.compression)
    ? (payload.compression as Partial<ChatCompressionStatistics>)
    : {};

  // ponytail: compatibility shim for remote/old sidecar partial stats payloads;
  // ceiling is shape-only defaults, upgrade path is sidecar protocol versioning.
  return {
    ...base,
    ...(payload ?? {}),
    workspaceId: payload?.workspaceId ?? workspaceId,
    chatId: payload?.chatId ?? chatId,
    codeChangeStats: {
      ...base.codeChangeStats,
      ...codeChangeStats,
    },
    compression: {
      ...base.compression,
      ...compression,
    },
    modelBreakdown: Array.isArray(payload?.modelBreakdown)
      ? payload.modelBreakdown
      : base.modelBreakdown,
    providerBreakdown: Array.isArray(payload?.providerBreakdown)
      ? payload.providerBreakdown
      : base.providerBreakdown,
    toolBreakdown: Array.isArray(payload?.toolBreakdown)
      ? payload.toolBreakdown
      : base.toolBreakdown,
    contextUsageTimeline: Array.isArray(payload?.contextUsageTimeline)
      ? payload.contextUsageTimeline
      : base.contextUsageTimeline,
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

function formatReplyDuration(
  value: number | null,
  language: AppLanguageId = "en",
) {
  if (value === null) {
    return "n/a";
  }

  const roundedMs = Math.max(0, Math.round(value));
  if (roundedMs < 1000) {
    return `${new Intl.NumberFormat(language).format(roundedMs)} ms`;
  }

  const totalSeconds = Math.round(roundedMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const format = new Intl.NumberFormat(language);
  if (language.startsWith("zh")) {
    return minutes > 0
      ? `${format.format(minutes)} 分 ${format.format(seconds)} 秒`
      : `${format.format(seconds)} 秒`;
  }

  return minutes > 0
    ? `${format.format(minutes)} min ${format.format(seconds)} sec`
    : `${format.format(seconds)} sec`;
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

function agentTranscriptViewCacheKey(
  workspaceId: string,
  chatId: string,
  instanceId: string,
) {
  return `${workspaceId}:${chatId}:${instanceId}`;
}

function pruneAgentTabCaches(
  snapshotCache: Map<string, AgentTeamSnapshotResponse>,
  transcriptCache: Map<string, AgentTranscriptViewCacheEntry>,
  openTabs: OpenAgentTab[],
) {
  const openChatKeys = new Set(
    openTabs.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
  );
  const openTranscriptKeys = new Set(
    openTabs.map((tab) =>
      agentTranscriptViewCacheKey(tab.workspaceId, tab.chatId, tab.instanceId),
    ),
  );

  for (const key of snapshotCache.keys()) {
    if (!openChatKeys.has(key)) {
      snapshotCache.delete(key);
    }
  }
  for (const key of transcriptCache.keys()) {
    if (!openTranscriptKeys.has(key)) {
      transcriptCache.delete(key);
    }
  }
}

function restoredQueuedRunKey(
  workspaceId: string,
  chatId: string,
  userMessageId: string,
) {
  return `${workspaceId}\u0000${chatId}\u0000${userMessageId}`;
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
  const phase = plan.phases.find((candidate) => candidate.id === target.phaseId);
  return phase?.status === "running";
}

function samePlanPhaseRetryRefreshTarget(
  left: PendingPlanPhaseRetryRefresh,
  right: PendingPlanPhaseRetryRefresh,
) {
  return (
    left.workspaceId === right.workspaceId &&
    left.planId === right.planId &&
    left.phaseId === right.phaseId
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

const CHAT_STREAM_IDLE_TIMEOUT_MS = 35_000;

type ChatStreamFrameMeta = {
  id: string | null;
};

class StreamIdleError extends Error {
  constructor(timeoutMs: number) {
    super(`chat stream was idle for ${timeoutMs}ms`);
    this.name = "StreamIdleError";
  }
}

function isStreamIdleError(error: unknown) {
  return error instanceof StreamIdleError;
}

function isStaleActiveRunError(message: string) {
  return message.startsWith("active chat run was not found");
}

function chatStreamIdleTimeoutMs() {
  const configured = (globalThis as {
    __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: unknown;
  }).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__;
  return typeof configured === "number" && configured > 0
    ? configured
    : CHAT_STREAM_IDLE_TIMEOUT_MS;
}

async function readChatStream(
  response: Response,
  onEvent: (event: ChatStreamEvent, meta: ChatStreamFrameMeta) => void,
  options: { idleTimeoutMs?: number; signal?: AbortSignal } = {},
) {
  if (!response.body) {
    throw new Error("chat stream response has no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  // ponytail: fixed threshold; if backend keep-alive becomes configurable, derive this from server capabilities.
  const idleTimeoutMs = options.idleTimeoutMs ?? chatStreamIdleTimeoutMs();
  let buffer = "";
  let sawCompletionEvent = false;
  let shouldStopReading = false;
  const handleEvent = (event: ChatStreamEvent, meta: ChatStreamFrameMeta) => {
    onEvent(event, meta);
    if (event.type === "complete") {
      sawCompletionEvent = true;
    }
    if (event.type === "streamEnd") {
      shouldStopReading = true;
    }
  };

  while (!shouldStopReading) {
    let readResult: ReadableStreamReadResult<Uint8Array>;
    try {
      readResult = await readStreamChunkWithIdleTimeout(
        reader,
        idleTimeoutMs,
        options.signal,
      );
    } catch (error) {
      if (sawCompletionEvent && isChatStreamTransportCloseError(error)) {
        return;
      }
      throw error;
    }

    const { done, value } = readResult;

    if (done) {
      break;
    }

    buffer += decoder.decode(value, { stream: true });
    buffer = readSseFrames(buffer, handleEvent);
  }

  if (shouldStopReading) {
    try {
      await reader.cancel();
    } catch (error) {
      if (!isChatStreamTransportCloseError(error)) {
        throw error;
      }
    }
    return;
  }

  buffer += decoder.decode();
  readSseFrames(`${buffer}\n\n`, handleEvent);
}

function isChatStreamTransportCloseError(error: unknown) {
  return (
    error instanceof TypeError ||
    (error instanceof DOMException && error.name === "NetworkError")
  );
}

function readStreamChunkWithIdleTimeout(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  idleTimeoutMs: number,
  signal?: AbortSignal,
) {
  if (signal?.aborted) {
    throw new DOMException("The operation was aborted.", "AbortError");
  }

  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let abortListener: (() => void) | null = null;
  const readPromise = reader.read();
  const idlePromise = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new StreamIdleError(idleTimeoutMs));
      void reader.cancel();
    }, idleTimeoutMs);
    abortListener = () => {
      reject(new DOMException("The operation was aborted.", "AbortError"));
    };
    signal?.addEventListener("abort", abortListener, { once: true });
  });

  return Promise.race([readPromise, idlePromise]).finally(() => {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
    if (abortListener) {
      signal?.removeEventListener("abort", abortListener);
    }
  });
}

function readSseFrames(
  buffer: string,
  onEvent: (event: ChatStreamEvent, meta: ChatStreamFrameMeta) => void,
) {
  const normalized = buffer.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const frames = normalized.split("\n\n");
  const remaining = frames.pop() ?? "";

  for (const frame of frames) {
    const lines = frame.split("\n");
    const id = lines
      .filter((line) => line.startsWith("id:"))
      .map((line) => line.slice(3).trimStart())
      .at(-1) ?? null;
    const data = lines
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

    onEvent(event, { id });
  }

  return remaining;
}

function parseChatStreamEvent(value: unknown): ChatStreamEvent | null {
  if (!isObjectRecord(value) || typeof value.type !== "string") {
    return null;
  }

  if (isObjectRecord(value.value) && typeof value.value.type !== "string") {
    return parseChatStreamEvent({ ...value.value, type: value.type });
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

  if (value.type === "connecting") {
    const message = optionalStringField(value, "message");
    if (message === null) {
      return null;
    }

    return message === undefined ? { type: "connecting" } : { type: "connecting", message };
  }

  if (value.type === "textDelta" || value.type === "text_delta") {
    const assistantMessageId = optionalStringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const delta = stringField(value, "delta");
    const reasoningDurationMs = optionalNumberField(
      value,
      "reasoningDurationMs",
      "reasoning_duration_ms",
    );

    if (assistantMessageId === null || delta === null || reasoningDurationMs === false) {
      return null;
    }

    return { type: "textDelta", assistantMessageId, delta, reasoningDurationMs };
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
    const kind = parseContextCompressionKind(fieldValue(value, "kind"));
    const status = stringField(value, "status") ?? "completed";
    const detail = parseContextCompressionDetail(fieldValue(value, "detail"));

    if (!assistantMessageId || !kind || detail === false) {
      return null;
    }

    return {
      type: "contextCompression",
      assistantMessageId,
      ...(snapshotId ? { snapshotId } : {}),
      kind,
      status,
      detail: detail ?? null,
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
    const reasoningDurationMs = optionalNumberField(
      value,
      "reasoningDurationMs",
      "reasoning_duration_ms",
    );
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
      reasoningDurationMs === false ||
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
      reasoningDurationMs,
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
    const reasoningDurationMs = optionalNumberField(
      value,
      "reasoningDurationMs",
      "reasoning_duration_ms",
    );

    if (!assistantMessageId || !toolCall || reasoningDurationMs === false) {
      return null;
    }

    return { type: "toolCall", assistantMessageId, reasoningDurationMs, toolCall };
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
    const startedAt = optionalNullableStringField(value, "startedAt", "started_at");
    const completedAt = optionalNullableStringField(value, "completedAt", "completed_at");

    if (
      !assistantMessageId ||
      !toolCallId ||
      !isJsonValue(output) ||
      typeof isError !== "boolean" ||
      startedAt === false ||
      completedAt === false
    ) {
      return null;
    }

    return {
      type: "toolResult",
      assistantMessageId,
      toolCallId,
      output,
      isError,
      startedAt: startedAt ?? null,
      completedAt: completedAt ?? null,
    };
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

    const interruptedAssistantId = optionalNullableStringField(
      value,
      "interruptedAssistantId",
      "interrupted_assistant_id",
    );
    if (interruptedAssistantId === false) {
      return null;
    }

    return {
      type: "guidanceApplied",
      id,
      content,
      parts: parts as ChatMessagePart[],
      interruptedAssistantMetrics,
      source: stringField(value, "source") ?? undefined,
      interruptedAssistantId: interruptedAssistantId ?? undefined,
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

function parseContextCompressionKind(
  value: unknown,
): ChatContextCompressionKind | null {
  if (value === "llm") {
    return "llm";
  }
  if (value === "runtimeToolState" || value === "runtime_tool_state") {
    return "runtimeToolState";
  }
  if (value === "rule" || typeof value === "undefined" || value === null) {
    return "rule";
  }
  return null;
}

function parseContextCompressionDetail(
  value: unknown,
): ChatContextCompressionDetail | null | false {
  if (typeof value === "undefined" || value === null) {
    return null;
  }
  if (!isObjectRecord(value)) {
    return false;
  }

  const kind = parseContextCompressionKind(fieldValue(value, "kind"));
  const status = optionalStringField(value, "status");
  const snapshotId = optionalNullableStringField(value, "snapshotId", "snapshot_id");
  const startedAt = optionalNullableStringField(value, "startedAt", "started_at");
  const completedAt = optionalNullableStringField(value, "completedAt", "completed_at");
  const providerId = optionalNullableStringField(value, "providerId", "provider_id");
  const modelId = optionalNullableStringField(value, "modelId", "model_id");
  const originalTokenCount = optionalNumberField(
    value,
    "originalTokenCount",
    "original_token_count",
  );
  const summaryTokenCount = optionalNumberField(
    value,
    "summaryTokenCount",
    "summary_token_count",
  );

  if (
    !kind ||
    status === null ||
    snapshotId === false ||
    startedAt === false ||
    completedAt === false ||
    providerId === false ||
    modelId === false ||
    originalTokenCount === false ||
    summaryTokenCount === false
  ) {
    return false;
  }

  return normalizedContextCompressionDetail({
    ...(status ? { status } : {}),
    kind,
    snapshotId: snapshotId ?? null,
    originalTokenCount: originalTokenCount ?? null,
    summaryTokenCount: summaryTokenCount ?? null,
    startedAt: startedAt ?? null,
    completedAt: completedAt ?? null,
    providerId: providerId ?? null,
    modelId: modelId ?? null,
  });
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
  const startedAt = optionalNullableStringField(value, "startedAt", "started_at");
  const completedAt = optionalNullableStringField(value, "completedAt", "completed_at");

  if (
    !id ||
    !name ||
    !status ||
    !isJsonValue(input) ||
    !isJsonValue(output) ||
    typeof isError !== "boolean" ||
    startedAt === false ||
    completedAt === false
  ) {
    return null;
  }

  return normalizedToolCallSummary({
    id,
    name,
    status,
    input,
    output,
    isError,
    startedAt: startedAt ?? null,
    completedAt: completedAt ?? null,
  });
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

function normalizeChatMessageStatus(value: unknown): "error" | "streaming" | undefined {
  return value === "error" || value === "streaming" ? value : undefined;
}

export function normalizeChatMessageSummary(
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
  const status = normalizeChatMessageStatus(fieldValue(message, "status"));
  const queuedRun = normalizeQueuedMessageRunSummary(message.queuedRun);
  const runConfigValue = fieldValue(message, "runConfig", "run_config");
  const runConfigRecord = runConfigValue && typeof runConfigValue === "object"
    ? runConfigValue as Record<string, unknown>
    : null;
  const runConfig = runConfigRecord
    ? {
      modelId: String(fieldValue(runConfigRecord, "modelId", "model_id") ?? ""),
      providerId: typeof fieldValue(runConfigRecord, "providerId", "provider_id") === "string"
        ? String(fieldValue(runConfigRecord, "providerId", "provider_id"))
        : null,
      thinkingLevel: typeof fieldValue(runConfigRecord, "thinkingLevel", "thinking_level") === "string"
        ? String(fieldValue(runConfigRecord, "thinkingLevel", "thinking_level"))
        : null,
      selectedSkillIds: Array.isArray(fieldValue(runConfigRecord, "selectedSkillIds", "selected_skill_ids"))
        ? (fieldValue(runConfigRecord, "selectedSkillIds", "selected_skill_ids") as unknown[])
          .filter((value): value is string => typeof value === "string")
        : [],
      sessionMode: fieldValue(runConfigRecord, "sessionMode", "session_mode") === "plan" ? "plan" as const : null,
      teamModeEnabled: fieldValue(runConfigRecord, "teamModeEnabled", "team_mode_enabled") === true,
    }
    : null;
  const normalizedMessage = {
    ...message,
    extractedMemories,
    metrics,
    memoriesUsed,
    pendingMode,
    queuedRun,
    runConfig,
    runBadges: [],
    sessionMode,
    status,
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

function normalizeStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
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
    skillIds: normalizeStringArray(skillIds),
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

  if (part.type === "contextCompression" || part.type === "context_compression") {
    const kind = parseContextCompressionKind(fieldValue(part, "kind"));
    const status = stringField(part, "status") ?? "completed";
    const detail = parseContextCompressionDetail(fieldValue(part, "detail"));
    if (!kind || detail === false) {
      return null;
    }
    const normalizedDetail = normalizedContextCompressionDetail({
      ...(detail ?? {}),
      status: detail?.status ?? status,
      kind: detail?.kind ?? kind,
    });
    const id = stringField(part, "id") ?? contextCompressionPartId(kind, normalizedDetail);
    return {
      type: "contextCompression",
      id,
      status,
      kind,
      detail: normalizedDetail,
    };
  }

  if (part.type === "userInterruption" || part.type === "user_interruption") {
    const id = stringField(part, "id");
    const content = stringField(part, "content");
    if (!id || content === null) {
      return null;
    }
    const source = stringField(part, "source") ?? undefined;
    const interruptedAssistantMetrics = parseOptionalChatReplyMetrics(
      fieldValue(
        part,
        "interruptedAssistantMetrics",
        "interrupted_assistant_metrics",
      ),
    );
    if (interruptedAssistantMetrics === false) {
      return null;
    }
    return {
      type: "userInterruption",
      id,
      content,
      ...(source ? { source } : {}),
      ...(interruptedAssistantMetrics
        ? { interruptedAssistantMetrics }
        : {}),
    };
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
  const llmRequestIds =
    stringArrayField(value, "llmRequestIds", "llm_request_ids") ?? [];

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
    llmRequestIds,
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

function optionalNumberField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);

  if (typeof field === "undefined" || field === null) {
    return field;
  }

  return typeof field === "number" && Number.isFinite(field) ? field : false;
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
