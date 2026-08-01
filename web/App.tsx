import focoLogoSvg from "../foco.svg?raw";
import {
  Activity,
  AppWindow,
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
import { isToolCallLoopGuardBlockedPayload } from "./api/types";
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
  BrowserRouteHtmlPreviewTab,
  ChatAttachmentPartSummary,
  ChatAttachmentPayload,
  ChatAgentTaskLifecycle,
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
  UpdateModelFastModeResponse,
  UpdateModelRouteResponse,
  OpenChatTab,
  Plan,
  PlanAutoRunResponse,
  PlanResponse,
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
  CONTEXT_PANEL_STACKED_BREAKPOINT_PX,
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
  type PanelResizeDragSession,
} from "./app/app-effects";
import { useAppRouting } from "./app/app-routing";
import {
  browserPathForRoute,
  currentBrowserRoute,
  isHtmlPreviewPath,
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
  workspaceNewChatButtonClass,
  workspaceNameFromPath,
} from "./features/workspaces/workspace-helpers";
import { WorkspaceDialog } from "./features/workspaces/WorkspaceDialog";
import {
  FilePickerDialog,
  type FilePickerSelection,
} from "./features/file-picker/FilePickerDialog";
import { DeleteChatDialog } from "./features/chat/DeleteChatDialog";
import {
  Accordion,
  Button,
  ContextMenu,
  Input,
  Label,
  Modal,
  Radio,
  RadioGroup,
  Spinner,
  TextArea,
  TextField,
} from "./shared/ui";
import { ChatPanel, type ChatPanelHelpers } from "./features/chat/ChatPanel";
import { ModelRoutingPanel } from "./features/models/ModelRoutingPanel";
import {
  activeSkillQuery,
  chatAttachmentPayload,
  composerAttachmentFromSelectedFile,
  fileToComposerAttachment,
  formatFileSize,
  messageWithSelectedSkills,
  removeActiveSkillToken,
  selectedSkillPrefix,
  skillScopeLabel,
  unsupportedAttachmentInputModality,
  unsupportedAttachmentMessage,
  unsupportedFileAttachmentMessage,
  userMessageParts,
} from "./features/chat/chat-helpers";
import { useWorkspaceSkillCatalog } from "./features/chat/use-workspace-skill-catalog";
import {
  isHtmlFilePath,
  isWorkspaceImageFilePath,
  preloadOptionalMonaco,
  WorkspaceFileEditorPanel,
  type MonacoEditorViewState,
  type OpenFileTab,
  type WorkspaceFileEditorState,
} from "./features/files/WorkspaceFileEditorPanel";
import {
  WorkspaceHtmlPreviewPanel,
  type OpenHtmlPreviewTab,
} from "./features/files/WorkspaceHtmlPreviewPanel";
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
import {
  errorDiagnostic,
  errorMessage,
  requestJson,
  responseError,
  responseErrorMessage,
  type ApiDiagnostic,
} from "./shared/api-client";
import { installUpdateAndWaitForRestart } from "./shared/update-install";
import { fetchWorkspaceSpecJobsList } from "./shared/workspace-spec-jobs-list";
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
  1000, 2000, 4000, 8000, 15000, 30000, 45000, 60000,
] as const;
const WORKSPACE_SPEC_JOB_STEADY_POLL_MS = 60000;

function workspaceSpecJobPollDelayMs(pollIndex: number) {
  return (
    WORKSPACE_SPEC_JOB_POLL_DELAYS_MS[pollIndex] ??
    WORKSPACE_SPEC_JOB_STEADY_POLL_MS
  );
}

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
  return (
    typeof document === "undefined" || document.visibilityState === "visible"
  );
}

function shouldReuseRequest<T>(
  entry: SingleFlightEntry<T> | undefined,
  nowMs: number,
  force = false,
) {
  if (!entry) {
    return false;
  }

  return (
    !entry.settled ||
    (!force && nowMs - entry.startedAtMs < REQUEST_STORM_DEDUPE_MS)
  );
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
  return (
    plan.status === "draft" ||
    plan.status === "ready" ||
    plan.status === "paused" ||
    plan.status === "failed"
  );
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
  return plans.map((plan) =>
    isPlanOrderReorderable(plan)
      ? (nextReorderablePlans[nextIndex++] ?? plan)
      : plan,
  );
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
  const pageLimit = options.pageLimit ?? CHAT_MESSAGES_INITIAL_PAGE_LIMIT;
  const fullCacheLimit =
    options.fullCacheLimit ?? INACTIVE_CHAT_FULL_CACHE_LIMIT;
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
  /** True when older cached history was retained (overlap prefix or active-run disjoint baseline). */
  preservedCachePrefix: boolean;
};

/**
 * Options for merging a freshly loaded latest page with the in-memory chat cache.
 * - preserveStreamingPlaceholders: re-insert local streaming assistants the server
 *   has not yet returned (requires id continuity unless disjoint active-run preserve).
 * - preserveDisjointActiveRunCache: when the latest page shares no message ids with
 *   cache (e.g. coordinator attempt boundary), keep the full cache as history and
 *   append the server page. Callers must enable this only for the same continuous
 *   local run via isSameContinuousLocalActiveRun (matching runId, or temporary null
 *   activeRun while a live local SSE remains open). Ordinary authoritative reloads
 *   and unrelated/canceled runs must leave this false so edit rewrites / history
 *   trims do not resurrect deleted threads.
 */
export type MergeLoadedMessagesOptions = {
  preserveStreamingPlaceholders?: boolean;
  preserveDisjointActiveRunCache?: boolean;
  /**
   * The messages request began before this chat received newer live events.
   * Keep only context-compression parts that the stale server snapshot cannot
   * know about; this is deliberately narrower than general message merging.
   */
  preserveLiveContextCompressionParts?: boolean;
  /**
   * The messages request began before this chat received a runtime-projected
   * delegated-Agent terminal event. Keep those idempotent timeline parts from
   * the live cache so an older server snapshot cannot erase them.
   */
  preserveLiveAgentTaskLifecycleParts?: boolean;
  /**
   * Assistant ids that changed after this `/messages` request began. For a
   * proven same-run stale response, retain the local assistant wholesale so
   * older text, reasoning, tool state, terminal status, and metrics cannot
   * overwrite a newer SSE mutation.
   */
  preserveLiveAssistantMessageIds?: ReadonlySet<string>;
};

export type ContinuousActiveRunMatchInput = {
  /** Local chat is marked running with an activeRunInfo entry. */
  hasLocalActiveRun: boolean;
  localRunId: string | null;
  /** Server messages payload activeRun.runId, or null when omitted. */
  serverActiveRunId: string | null;
  /**
   * True when this client still holds a non-aborted SSE AbortController for the
   * chat. Required to trust a temporary null server activeRun across attempt
   * boundaries; without it, zero-overlap preserve must not fire (edit rewrites
   * can cancel the old run and briefly report null before a replacement).
   */
  hasOpenLocalStream: boolean;
};

/**
 * Decide whether the latest-page load is still the same continuous local run.
 * Used for zero-overlap cache retention and streaming-placeholder preserve.
 *
 * - Matching non-null server runId proves continuity even mid-reconnect.
 * - Temporary null server activeRun is only continuous when this client still
 *   has a live local stream for the recorded run (Coordinator handoff gap).
 * - A different server runId is never continuous with the local cache thread.
 */
export function isSameContinuousLocalActiveRun(
  input: ContinuousActiveRunMatchInput,
): boolean {
  if (!input.hasLocalActiveRun || !input.localRunId) {
    return false;
  }
  if (input.serverActiveRunId != null) {
    return input.serverActiveRunId === input.localRunId;
  }
  return input.hasOpenLocalStream;
}

/**
 * Merge a freshly loaded latest page with the in-memory chat cache.
 * - When cache and loaded page share a stable message id, keep the cache prefix
 *   before that overlap and let the server page replace the overlap and suffix.
 * - When there is no overlap and preserveDisjointActiveRunCache is false, drop
 *   unprovable cache history (edit rewrite / trim) and do not re-insert streaming
 *   bubbles from the discarded thread.
 * - When there is no overlap and preserveDisjointActiveRunCache is true, keep the
 *   full cache as the history baseline, overlay same-id server versions, and append
 *   server-only messages in server order.
 * - When preserveStreamingPlaceholders is true and there is id continuity with
 *   the cache, re-insert streaming assistants the server has not yet returned.
 */
export function mergeLoadedMessagesWithStreamingPlaceholders(
  loadedMessages: ShellMessage[],
  cachedMessages: ShellMessage[],
  options: boolean | MergeLoadedMessagesOptions = false,
): MergeLoadedMessagesResult {
  const {
    preserveStreamingPlaceholders = false,
    preserveDisjointActiveRunCache = false,
    preserveLiveContextCompressionParts = false,
    preserveLiveAgentTaskLifecycleParts = false,
    preserveLiveAssistantMessageIds,
  } =
    typeof options === "boolean"
      ? {
          preserveStreamingPlaceholders: options,
          preserveDisjointActiveRunCache: false,
        }
      : options;

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

  let nextMessages: ShellMessage[];
  let preservedCachePrefix: boolean;

  if (cacheOverlapStart < 0 && preserveDisjointActiveRunCache) {
    // Active continuous refresh with a momentarily disjoint latest page:
    // cache is the history baseline; server page is the authoritative tail.
    const cachedIds = new Set(cachedMessages.map((message) => message.id));
    const serverById = new Map(
      loadedMessages.map((message) => [message.id, message]),
    );
    nextMessages = [
      ...cachedMessages.map((message) => serverById.get(message.id) ?? message),
      ...loadedMessages.filter((message) => !cachedIds.has(message.id)),
    ];
    preservedCachePrefix = true;
  } else {
    const preservedPrefix =
      cacheOverlapStart > 0 ? cachedMessages.slice(0, cacheOverlapStart) : [];
    preservedCachePrefix = preservedPrefix.length > 0;
    nextMessages =
      preservedCachePrefix || cacheOverlapStart === 0
        ? [...preservedPrefix, ...loadedMessages]
        : [...loadedMessages];
  }

  if (preserveLiveContextCompressionParts) {
    nextMessages = overlayStaleLoadedContextCompressionParts(
      nextMessages,
      cachedMessages,
    );
  }

  if (preserveLiveAgentTaskLifecycleParts) {
    nextMessages = overlayStaleLoadedAgentTaskLifecycleParts(
      nextMessages,
      cachedMessages,
    );
  }

  if (preserveLiveAssistantMessageIds?.size) {
    nextMessages = overlayStaleLoadedAssistantMessages(
      nextMessages,
      cachedMessages,
      preserveLiveAssistantMessageIds,
    );
  }

  // No id continuity with cache: do not resurrect orphan streaming from a
  // discarded thread. Disjoint active-run preserve already kept the full cache
  // (including any live streaming bubble) above.
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

/**
 * A `/messages` response is a point-in-time snapshot. Once an assistant has
 * been changed by a later live event, its cached representation is the only
 * monotonic view until a fresh request is made. This deliberately preserves
 * only same-id assistants supplied by the caller's run/session proof; normal
 * history reloads continue to replace the cache authoritatively.
 */
export function overlayStaleLoadedAssistantMessages(
  loadedMessages: ShellMessage[],
  cachedMessages: ShellMessage[],
  liveAssistantMessageIds: ReadonlySet<string>,
): ShellMessage[] {
  const cachedAssistantsById = new Map(
    cachedMessages
      .filter((message) => message.role === "assistant")
      .map((message) => [message.id, message]),
  );

  let nextMessages = loadedMessages.map((message) => {
    if (
      message.role !== "assistant" ||
      !liveAssistantMessageIds.has(message.id)
    ) {
      return message;
    }
    return cachedAssistantsById.get(message.id) ?? message;
  });

  const loadedIds = new Set(nextMessages.map((message) => message.id));
  for (const [cachedIndex, message] of cachedMessages.entries()) {
    if (
      message.role !== "assistant" ||
      !liveAssistantMessageIds.has(message.id) ||
      loadedIds.has(message.id)
    ) {
      continue;
    }

    // A stale response can predate creation of the terminal assistant and
    // omit it entirely. Reinsert after the nearest stable predecessor from the
    // cached timeline; never append blindly or restore an unanchored bubble.
    let anchorId: string | null = null;
    for (let index = cachedIndex - 1; index >= 0; index -= 1) {
      const candidate = cachedMessages[index];
      if (candidate && loadedIds.has(candidate.id)) {
        anchorId = candidate.id;
        break;
      }
    }
    if (anchorId === null) {
      continue;
    }
    const anchorIndex = nextMessages.findIndex(
      (candidate) => candidate.id === anchorId,
    );
    if (anchorIndex < 0) {
      continue;
    }
    nextMessages = [
      ...nextMessages.slice(0, anchorIndex + 1),
      message,
      ...nextMessages.slice(anchorIndex + 1),
    ];
    loadedIds.add(message.id);
  }

  return nextMessages;
}

/**
 * Overlay live compression lifecycle parts onto a same-thread assistant from
 * an older `/messages` response. This never restores messages the server did
 * not return: callers must first prove the response predates live revisions
 * and belongs to the same continuous active run.
 */
export function overlayStaleLoadedContextCompressionParts(
  loadedMessages: ShellMessage[],
  cachedMessages: ShellMessage[],
): ShellMessage[] {
  const cachedAssistantsById = new Map(
    cachedMessages
      .filter((message) => message.role === "assistant")
      .map((message) => [message.id, message]),
  );

  return loadedMessages.map((loadedMessage) => {
    if (loadedMessage.role !== "assistant") {
      return loadedMessage;
    }
    const cachedMessage = cachedAssistantsById.get(loadedMessage.id);
    if (!cachedMessage) {
      return loadedMessage;
    }

    const liveCompressionParts = cachedMessage.parts.filter(
      (part): part is ChatContextCompressionPart =>
        part.type === "contextCompression",
    );
    if (!liveCompressionParts.length) {
      return loadedMessage;
    }

    let parts = [...loadedMessage.parts];
    let changed = false;
    for (const livePart of liveCompressionParts) {
      const serverIndex = parts.findIndex(
        (part) =>
          part.type === "contextCompression" &&
          contextCompressionPartsMatch(part, livePart),
      );
      if (serverIndex < 0) {
        parts.push(livePart);
        changed = true;
        continue;
      }

      const serverPart = parts[serverIndex];
      if (serverPart?.type !== "contextCompression") {
        continue;
      }
      const merged = mergeContextCompressionPart(serverPart, livePart);
      if (merged !== serverPart) {
        parts[serverIndex] = merged;
        changed = true;
      }
    }

    return changed ? { ...loadedMessage, parts } : loadedMessage;
  });
}

/**
 * Preserve runtime-projected subagent terminal events from an older messages
 * response. Unlike a streaming placeholder, a lifecycle part belongs to an
 * existing assistant message and is keyed by its stable event id.
 */
export function overlayStaleLoadedAgentTaskLifecycleParts(
  loadedMessages: ShellMessage[],
  cachedMessages: ShellMessage[],
): ShellMessage[] {
  const cachedAssistantsById = new Map(
    cachedMessages
      .filter((message) => message.role === "assistant")
      .map((message) => [message.id, message]),
  );

  return loadedMessages.map((loadedMessage) => {
    if (loadedMessage.role !== "assistant") {
      return loadedMessage;
    }
    const cachedMessage = cachedAssistantsById.get(loadedMessage.id);
    if (!cachedMessage) {
      return loadedMessage;
    }

    const liveParts = cachedMessage.parts
      .map((part, index) => ({ index, part }))
      .filter(
        (entry): entry is {
          index: number;
          part: Extract<ChatMessagePart, { type: "agentTaskLifecycle" }>;
        } => entry.part.type === "agentTaskLifecycle",
      );
    if (!liveParts.length) {
      return loadedMessage;
    }

    let parts = [...loadedMessage.parts];
    let changed = false;
    for (const { index, part } of liveParts) {
      if (
        parts.some(
          (candidate) =>
            candidate.type === "agentTaskLifecycle" &&
            candidate.lifecycle.eventId === part.lifecycle.eventId,
        )
      ) {
        continue;
      }
      // The cached part order is the event order. Its index preserves the
      // position before later tool output or the recovered assistant summary
      // when a stale history response did not yet include this terminal event.
      parts.splice(Math.min(index, parts.length), 0, part);
      changed = true;
    }
    return changed ? { ...loadedMessage, parts } : loadedMessage;
  });
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

/**
 * Automatic progress-guard sources (reasoning / tool-call loop recovery).
 * These become non-editable synthetic user bubbles in the chat UI.
 */
export function isAutomaticGuardSource(
  source: string | null | undefined,
): boolean {
  return source === "reasoningLoopGuard" || source === "toolCallLoopGuard";
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
      metrics: isLast ? message.metrics : (metrics ?? null),
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

/**
 * Derive whether Plan mode should be on for a chat from its loaded messages.
 * Uses the last real user message only; synthetic interruptions are ignored.
 */
export function planModeEnabledFromMessages(messages: ShellMessage[]): boolean {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role !== "user" || message.syntheticSource) {
      continue;
    }
    return message.sessionMode === "plan";
  }
  return false;
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
  | { type: "htmlPreview"; workspaceId: string; path: string }
  | {
      type: "agent";
      workspaceId: string;
      chatId: string;
      teamId: string;
      instanceId: string;
    };

type FilePickerRequest = {
  allowOutsideWorkspace?: boolean;
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
  | (OpenHtmlPreviewTab & { type: "htmlPreview"; title: string })
  | (OpenAgentTab & {
      type: "agent";
      title: string;
      workspaceName: string;
      workspaceLogoUrl: string | null;
    });

type DurableRunTermination = {
  runId: string;
};

type ChatStreamHandoff = {
  chatId: string;
  lastSequence: number | null;
  runId: string | null;
  workspaceId: string;
};

type MainTabCloseScope = "current" | "others" | "all" | "right" | "left";

type ChatSessionStatusKind =
  "idle" | "open" | "scheduled" | "running" | "failed";

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
  /** Durable queuedRun.status === "running" (for example, coordinator waiting). */
  persistedRunning?: boolean;
  runningChatKeys: Set<string>;
  scheduledChatKey?: string | null;
  scheduledStatus?: ScheduledWorkspaceRun["status"] | null;
  terminalRunId?: string | null;
  workspaceActiveRun?: ActiveChatRunSummary | null;
};

export function isTerminalActiveRun(
  activeRun: ActiveChatRunSummary | null | undefined,
  terminalRunId: string | null | undefined,
): boolean {
  return Boolean(
    activeRun?.runId && terminalRunId && activeRun.runId === terminalRunId,
  );
}

export function isGuidableActiveRun(
  runInfo: ActiveRunInfo | null | undefined,
  isRunning: boolean,
): runInfo is ActiveRunInfo & { runId: string } {
  return Boolean(isRunning && runInfo?.runId && runInfo.acceptingGuidance);
}

export function deriveChatSessionStatus({
  activeChatKey,
  activeRunInfoByChatKey,
  chatKey,
  failedChatKeySet,
  openChatKeySet,
  persistedRunning = false,
  runningChatKeys,
  scheduledChatKey = null,
  scheduledStatus = null,
  terminalRunId = null,
  workspaceActiveRun = null,
}: ChatSessionStatusInput): ChatSessionStatus {
  const statusChatKeys =
    scheduledChatKey && scheduledChatKey !== chatKey
      ? [chatKey, scheduledChatKey]
      : [chatKey];
  const operationalWorkspaceActiveRun = isTerminalActiveRun(
    workspaceActiveRun,
    terminalRunId,
  )
    ? null
    : workspaceActiveRun;
  const activeRun =
    statusChatKeys
      .map((statusChatKey) => activeRunInfoByChatKey[statusChatKey] ?? null)
      .find((runInfo): runInfo is ActiveRunInfo => runInfo !== null) ??
    operationalWorkspaceActiveRun;
  // A durable queued run is a visual running state, but never invents an active
  // run identity for cancellation or guidance.
  const isRunning =
    statusChatKeys.some((statusChatKey) =>
      runningChatKeys.has(statusChatKey),
    ) ||
    operationalWorkspaceActiveRun !== null ||
    persistedRunning;
  const isScheduled =
    scheduledStatus === "queued" || scheduledStatus === "starting";
  const isOpen = statusChatKeys.some(
    (statusChatKey) =>
      openChatKeySet.has(statusChatKey) || activeChatKey === statusChatKey,
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

/** True when workspace chat metadata still marks a durable main-agent lifecycle as running. */
export function isPersistedQueuedRunRunning(
  queuedRun: { status?: string | null } | null | undefined,
): boolean {
  return queuedRun?.status === "running";
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
  positioned: boolean;
  top: number;
  workspacePath: string;
};

const LIVE_CONTEXT_USAGE_REFRESH_MS = 5000;
const AGENT_TEAM_UNSETTLED_REFRESH_MS = 1000;
const UNSETTLED_AGENT_TASK_STATUSES = new Set([
  "queued",
  "running",
  "waiting",
]);
/** Latest-page size when opening/switching chats or filling a missing cache. */
export const CHAT_MESSAGES_INITIAL_PAGE_LIMIT = 60;
/** Older-history page size when the user loads earlier messages. */
export const CHAT_MESSAGES_HISTORY_PAGE_LIMIT = 100;
const INACTIVE_CHAT_FULL_CACHE_LIMIT = 8;
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-default";
const EMPTY_CONFIGURED_PROVIDERS: ConfiguredProviderSummary[] = [];
const EMPTY_GIT_STATUS_FILES: GitStatusFileSummary[] = [];

type SourceControlTarget = {
  kind: "workspace" | "worktree";
  path: string | null;
  label: string;
};

type SourceControlView = {
  chatKey: string | null;
  isVisible: boolean;
  selectedDiffPath: string | null;
  target: SourceControlTarget | null;
  workspaceId: string;
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

function remoteWorkspacePendingStages(
  t: Translate,
): RemoteServerDiagnosticStage[] {
  return [
    {
      details: null,
      errorKind: null,
      message: t("Checking SSH"),
      stage: "ssh",
      status: "running",
    },
    {
      details: null,
      errorKind: null,
      message: t("Detecting target"),
      stage: "target",
      status: "pending",
    },
    {
      details: null,
      errorKind: null,
      message: t("Installing sidecar"),
      stage: "sidecarAsset",
      status: "pending",
    },
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

function sourceControlLabelForWorktree(
  worktree: Pick<GitWorktreeSummary, "branch" | "name">,
) {
  return worktree.branch ?? worktree.name;
}

function worktreeMatchesExecutionRoot(
  worktreePath: string,
  executionRootPath: string,
) {
  const normalize = (path: string) =>
    path.replace(/\\/g, "/").replace(/\/+$/, "");
  const left = normalize(worktreePath);
  const right = normalize(executionRootPath);
  return (
    left === right || left.endsWith(`/${right}`) || right.endsWith(`/${left}`)
  );
}

function sourceControlDefaultTarget(
  workspacePath: string | null | undefined,
  gitBranches: GitBranchesResponse | null,
  coordinatorInstance: AgentInstanceView | null,
): SourceControlTarget | null {
  const workspaceTarget: SourceControlTarget = {
    kind: "workspace",
    label:
      gitBranches?.currentBranch ??
      (workspacePath ? pathBasename(workspacePath) : "Workspace"),
    path: null,
  };

  if (
    coordinatorInstance?.executionWorkspaceMode !== "isolated_worktree" ||
    coordinatorInstance.worktreeStatus === "deleted"
  ) {
    return workspaceTarget;
  }

  const byPath = coordinatorInstance.executionRootPath
    ? (gitBranches?.worktrees.find((worktree) =>
        worktreeMatchesExecutionRoot(
          worktree.path,
          coordinatorInstance.executionRootPath!,
        ),
      ) ?? null)
    : null;
  const byBranch = coordinatorInstance.worktreeBranch
    ? (gitBranches?.worktrees.find(
        (worktree) => worktree.branch === coordinatorInstance.worktreeBranch,
      ) ?? null)
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
      label:
        coordinatorInstance.worktreeBranch ??
        pathBasename(coordinatorInstance.executionRootPath),
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
      label:
        gitBranches?.currentBranch ??
        (workspacePath ? pathBasename(workspacePath) : "Workspace"),
      path: null,
    },
  ];

  for (const worktree of gitBranches?.worktrees ?? []) {
    if (
      workspacePath &&
      worktreeMatchesExecutionRoot(worktree.path, workspacePath)
    ) {
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
  return (
    targets.find((target) => sourceControlTargetKey(target) === key) ?? null
  );
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

// Low-priority stream updates for auxiliary panel/statistics only.
// Never wrap setMessagesForChatKey or other AI bubble content here: startTransition
// can delay committing the active messages list while chatMessagesByKey cache is
// already updated, so tool/context/memory parts only appear after a tab switch.
// Bubble-visible stream events must call setMessagesForChatKey at normal priority.
function deferStreamAuxiliaryUpdate(update: () => void) {
  const testScheduler = (
    globalThis as {
      __FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__?: (
        update: () => void,
      ) => void;
    }
  ).__FOCO_TEST_STREAM_AUXILIARY_UPDATE_SCHEDULER__;
  if (typeof testScheduler === "function") {
    testScheduler(update);
    return;
  }
  // ponytail: transition is enough for sparse usage/stats; add a real queue only
  // if profiler shows auxiliary panel storms.
  startTransition(update);
}

type ComposerDefaultSelection = {
  modelId: string;
  providerId: string;
  thinkingLevel: string;
};

function latencyModeFromValue(value: unknown): "standard" | "fast" {
  return value === "fast" ? "fast" : "standard";
}

function latencyModeForModel(
  model: ConfiguredModelSummary | null | undefined,
): "standard" | "fast" {
  return model?.supportsFast === true && model.fastModeEnabled === true
    ? "fast"
    : "standard";
}

function useStableCallback<T extends (...args: any[]) => unknown>(
  callback: T,
): T {
  const callbackRef = useRef(callback);

  useLayoutEffect(() => {
    callbackRef.current = callback;
  });

  return useCallback(
    ((...args: Parameters<T>) => callbackRef.current(...args)) as T,
    [],
  );
}

function workspaceConnectionLooksReady(status: string) {
  const normalized = status.toLowerCase();
  return (
    normalized === "connected" ||
    normalized === "ready" ||
    normalized === "degraded"
  );
}

/**
 * Base workspace summaries from GET /api/workspaces. Remote entries ship empty chats
 * with hasMore=false from the server; preserve any already-hydrated remote chat page
 * and use hasMore=true placeholders so open tabs stay "unknown" (not "missing").
 */
function normalizeBaseWorkspaceSummaries(
  workspaces: WorkspaceSummary[],
  previousWorkspaces: WorkspaceSummary[],
): WorkspaceSummary[] {
  const previousById = new Map(
    previousWorkspaces.map((workspace) => [workspace.id, workspace] as const),
  );

  return workspaces.map((workspace) => {
    if (!workspace.serverId) {
      return workspace;
    }

    const previous = previousById.get(workspace.id);
    if (previous?.serverId) {
      return {
        ...workspace,
        chatPagination: previous.chatPagination ?? {
          hasMore: true,
          limit: WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
          nextCursor: null,
          total: previous.chats.length,
        },
        chats: previous.chats,
      };
    }

    return {
      ...workspace,
      chatPagination: {
        hasMore: true,
        limit: WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
        nextCursor: null,
        total: 0,
      },
      chats: [],
    };
  });
}

function shouldHydrateRemoteWorkspaceChats(workspace: WorkspaceSummary) {
  return (
    Boolean(workspace.serverId) &&
    workspaceConnectionLooksReady(workspace.connectionStatus)
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

function applyRemoteWorkspaceChatsPatch(
  workspaces: WorkspaceSummary[],
  workspaceId: string,
  data: WorkspaceChatsResponse,
): WorkspaceSummary[] {
  let changed = false;
  const next = workspaces.map((workspace) => {
    if (workspace.id !== workspaceId) {
      return workspace;
    }
    changed = true;
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
  });
  return changed ? next : workspaces;
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

const MOBILE_SIDEBAR_OUTSIDE_TARGET_PX = 48;

function mobileSafeAreaInsetWidth() {
  if (typeof document === "undefined" || !document.body) {
    return 0;
  }

  const probe = document.createElement("div");
  probe.style.cssText = [
    "position:fixed",
    "visibility:hidden",
    "pointer-events:none",
    "padding-left:env(safe-area-inset-left, 0px)",
    "padding-right:env(safe-area-inset-right, 0px)",
  ].join(";");
  document.body.append(probe);

  const styles = window.getComputedStyle(probe);
  const insetWidth =
    (Number.parseFloat(styles.paddingLeft) || 0) +
    (Number.parseFloat(styles.paddingRight) || 0);
  probe.remove();
  return insetWidth;
}

function workspaceSidebarMaxWidthForViewport(
  viewportWidth: number,
  safeAreaInsetWidth = 0,
) {
  if (viewportWidth >= MOBILE_BREAKPOINT_PX) {
    return WORKSPACE_SIDEBAR_MAX_WIDTH;
  }

  return Math.min(
    WORKSPACE_SIDEBAR_MAX_WIDTH,
    Math.max(0, viewportWidth - MOBILE_SIDEBAR_OUTSIDE_TARGET_PX - safeAreaInsetWidth),
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
  >(
    initialBrowserRoute.viewMode === "stats"
      ? (initialBrowserRoute.filters ?? {})
      : {},
  );
  const statsRouteFiltersRef = useRef(statsRouteFilters);
  statsRouteFiltersRef.current = statsRouteFilters;
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceDialogRevision, setWorkspaceDialogRevision] = useState(0);
  const [workspaceMode, setWorkspaceMode] = useState<"local" | "ssh">("local");
  const [workspaceServerId, setWorkspaceServerId] = useState("");
  const [workspaceTestStages, setWorkspaceTestStages] = useState<
    RemoteServerDiagnosticResponse["result"]["stages"]
  >([]);
  const [isTestingWorkspaceConnection, setIsTestingWorkspaceConnection] =
    useState(false);
  const [inlineRemoteServerName, setInlineRemoteServerName] = useState("");
  const [inlineRemoteServerHost, setInlineRemoteServerHost] = useState("");
  const [isCreatingInlineRemoteServer, setIsCreatingInlineRemoteServer] =
    useState(false);
  const [retryingRemoteWorkspaceId, setRetryingRemoteWorkspaceId] = useState<
    string | null
  >(null);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [workspaceTerminalShell, setWorkspaceTerminalShell] = useState("");
  const [workspaceSpecEnabled, setWorkspaceSpecEnabled] = useState(false);
  const [workspaceCodeGraphEnabled, setWorkspaceCodeGraphEnabled] =
    useState(false);
  const [workspaceIconDraft, setWorkspaceIconDraft] =
    useState<WorkspaceIconDraft | null>(null);
  const [filePickerRequest, setFilePickerRequest] =
    useState<FilePickerRequest | null>(null);
  const [draftMessage, setDraftMessage] = useState("");
  const [draftAttachments, setDraftAttachments] = useState<
    ComposerAttachment[]
  >([]);
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
  const [loadingChatKeys, setLoadingChatKeys] = useState<Set<string>>(
    () => new Set(),
  );
  const [loadingOlderChatMessageKeys, setLoadingOlderChatMessageKeys] =
    useState<Set<string>>(() => new Set());
  const [openFileTabs, setOpenFileTabs] = useState<OpenFileTab[]>([]);
  const openFileTabsRef = useRef<OpenFileTab[]>([]);
  // View state is intentionally ephemeral: keeping it in a ref preserves each
  // active editor's position without rendering App for Monaco scroll updates.
  const workspaceFileEditorViewStatesRef = useRef<
    Record<string, MonacoEditorViewState>
  >({});
  // Preview scroll positions follow the same ephemeral, per-open-file lifetime
  // as Monaco view states, without rerendering App for scroll updates.
  const workspaceMarkdownPreviewScrollTopsRef = useRef<Record<string, number>>(
    {},
  );
  const getWorkspaceFileEditorViewState = useCallback(
    (workspaceId: string, path: string) =>
      workspaceFileEditorViewStatesRef.current[
        workspaceFileEditorKey(workspaceId, path)
      ] ?? null,
    [],
  );
  const getWorkspaceMarkdownPreviewScrollTop = useCallback(
    (workspaceId: string, path: string) =>
      workspaceMarkdownPreviewScrollTopsRef.current[
        workspaceFileEditorKey(workspaceId, path)
      ] ?? 0,
    [],
  );
  const saveWorkspaceMarkdownPreviewScrollTop = useCallback(
    (workspaceId: string, path: string, scrollTop: number) => {
      const editorKey = workspaceFileEditorKey(workspaceId, path);
      const isOpen = openFileTabsRef.current.some(
        (tab) => tab.workspaceId === workspaceId && tab.path === path,
      );

      if (isOpen) {
        workspaceMarkdownPreviewScrollTopsRef.current[editorKey] = scrollTop;
      } else {
        delete workspaceMarkdownPreviewScrollTopsRef.current[editorKey];
      }
    },
    [],
  );
  const saveWorkspaceFileEditorViewState = useCallback(
    (workspaceId: string, path: string, viewState: MonacoEditorViewState) => {
      const editorKey = workspaceFileEditorKey(workspaceId, path);
      const isOpen = openFileTabsRef.current.some(
        (tab) => tab.workspaceId === workspaceId && tab.path === path,
      );

      if (isOpen) {
        workspaceFileEditorViewStatesRef.current[editorKey] = viewState;
      } else {
        delete workspaceFileEditorViewStatesRef.current[editorKey];
      }
    },
    [],
  );
  const [openHtmlPreviewTabs, setOpenHtmlPreviewTabs] = useState<
    OpenHtmlPreviewTab[]
  >([]);
  const openHtmlPreviewTabsRef = useRef<OpenHtmlPreviewTab[]>([]);
  const [workspaceFileEditors, setWorkspaceFileEditors] = useState<
    Record<string, WorkspaceFileEditorState>
  >({});
  const [pendingDeleteChat, setPendingDeleteChat] =
    useState<PendingDeleteChat | null>(null);
  const [workspaceChatContextMenu, setWorkspaceChatContextMenu] =
    useState<WorkspaceChatContextMenuState | null>(null);
  const [workspaceFileContextMenu, setWorkspaceFileContextMenu] =
    useState<WorkspaceFileContextMenuState | null>(null);
  const workspaceFileContextMenuRef = useRef<HTMLElement | null>(null);
  // ponytail: keep inactive chat cache ref-only so hot streaming paths don't
  // rerender App; ceiling is App still owns too much chat state, upgrade path is
  // moving this cache into a dedicated hook/store.
  const chatMessagesByKeyRef = useRef<Record<string, ShellMessage[]>>({});
  // A per-chat monotonic boundary between a `/messages` request and live SSE
  // mutations. It is intentionally ref-only: cache protection is ordering
  // metadata, not render state.
  const liveMessageRevisionByChatKeyRef = useRef<Map<string, number>>(
    new Map(),
  );
  // A chat-level revision cheaply detects that a request raced any visible
  // message update; assistant-level revisions identify exactly which bubbles
  // must not be replaced by that stale snapshot.
  const liveAssistantMessageRevisionByChatKeyRef = useRef<
    Map<string, Map<string, number>>
  >(new Map());
  function advanceLiveMessageRevision(
    chatKey: string,
    previousMessages: ShellMessage[] = [],
    nextMessages: ShellMessage[] = [],
  ) {
    const revisions = liveMessageRevisionByChatKeyRef.current;
    const revision = (revisions.get(chatKey) ?? 0) + 1;
    revisions.set(chatKey, revision);

    const previousAssistantsById = new Map(
      previousMessages
        .filter((message) => message.role === "assistant")
        .map((message) => [message.id, message]),
    );
    const assistantRevisions =
      liveAssistantMessageRevisionByChatKeyRef.current.get(chatKey) ??
      new Map<string, number>();
    for (const message of nextMessages) {
      if (
        message.role === "assistant" &&
        previousAssistantsById.get(message.id) !== message
      ) {
        assistantRevisions.set(message.id, revision);
      }
    }
    liveAssistantMessageRevisionByChatKeyRef.current.set(
      chatKey,
      assistantRevisions,
    );
  }
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
          openChatTabsRef.current.map((tab) =>
            chatRunKey(tab.workspaceId, tab.chatId),
          ),
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
  const settingsSkillsSnapshotRef = useRef<string | null>(null);
  const settingsModelsSnapshotRef = useRef<string | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatusSummary | null>(
    null,
  );
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateInstallNotice, setUpdateInstallNotice] = useState<string | null>(
    null,
  );
  const [agentDefinitions, setAgentDefinitions] = useState<
    AgentDefinitionSettings[]
  >([]);
  const [defaultAgentRolePrompts, setDefaultAgentRolePrompts] = useState<
    Record<string, string>
  >({});
  const [isTeamModeEnabled, setIsTeamModeEnabled] = useState(false);
  const [isPlanModeEnabled, setIsPlanModeEnabled] = useState(false);
  const planModeByChatKeyRef = useRef<Record<string, boolean>>({});
  const [isLoadingAgentDefinitions, setIsLoadingAgentDefinitions] =
    useState(false);
  const [agentDefinitionsError, setAgentDefinitionsError] = useState<
    string | null
  >(null);
  const [agentDefinitionOperationKey, setAgentDefinitionOperationKey] =
    useState<string | null>(null);
  const [agentTeamSnapshot, setAgentTeamSnapshot] =
    useState<AgentTeamSnapshotResponse | null>(null);
  const agentTeamSnapshotChatKeyRef = useRef<string | null>(null);
  const agentTeamSnapshotCacheRef = useRef(
    new Map<string, AgentTeamSnapshotResponse>(),
  );
  const agentTeamSnapshotRequestSequenceRef = useRef(0);
  const latestAgentTeamSnapshotRequestByChatKeyRef = useRef(
    new Map<string, number>(),
  );
  const loadingAgentTeamSnapshotRequestByChatKeyRef = useRef(
    new Map<string, number>(),
  );
  const agentTranscriptViewCacheRef = useRef(
    new Map<string, AgentTranscriptViewCacheEntry>(),
  );
  const [isLoadingAgentTeam, setIsLoadingAgentTeam] = useState(false);
  const [agentTeamError, setAgentTeamError] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [selectedThinkingLevel, setSelectedThinkingLevel] = useState("");
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
  const [gitBranches, setGitBranches] = useState<GitBranchesResponse | null>(
    null,
  );
  const [isContextPanelOpen, setIsContextPanelOpen] = useState(
    () => typeof window !== "undefined" && window.innerWidth >= 768,
  );
  const [contextPanelTab, setContextPanelTab] =
    useState<ContextPanelTab>("todo");
  const [diffPanelWidth, setDiffPanelWidth] = useState(
    CONTEXT_PANEL_DEFAULT_WIDTH,
  );
  const [contextPanelMobileHeight, setContextPanelMobileHeight] = useState(
    CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT,
  );
  const [isResizingDiffPanel, setIsResizingDiffPanel] = useState(false);
  const appShellRef = useRef<HTMLDivElement | null>(null);
  const contextPanelResizeDragRef = useRef<PanelResizeDragSession | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(WORKSPACE_SIDEBAR_MIN_WIDTH);
  const [sidebarViewportWidth, setSidebarViewportWidth] = useState(() =>
    typeof window === "undefined" ? MOBILE_BREAKPOINT_PX : window.innerWidth,
  );
  const [sidebarSafeAreaInsetWidth, setSidebarSafeAreaInsetWidth] = useState(0);
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
  const [
    selectedSourceControlTargetScope,
    setSelectedSourceControlTargetScope,
  ] = useState("");
  const [isSourceControlTargetManual, setIsSourceControlTargetManual] =
    useState(false);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [gitCommitMessage, setGitCommitMessage] = useState("");
  const [gitOperationKey, setGitOperationKey] = useState<string | null>(null);
  const [workspaceFiles, setWorkspaceFiles] =
    useState<WorkspaceFilesResponse | null>(null);
  const [expandedFileTreePaths, setExpandedFileTreePaths] = useState<
    Set<string>
  >(() => new Set([""]));
  const [loadingWorkspaceDirectoryPaths, setLoadingWorkspaceDirectoryPaths] =
    useState<Set<string>>(() => new Set());
  const [isLoadingWorkspaceFiles, setIsLoadingWorkspaceFiles] = useState(false);
  const [workspaceFilesError, setWorkspaceFilesError] = useState<string | null>(
    null,
  );
  const [workspaceFileOperationKey, setWorkspaceFileOperationKey] = useState<
    string | null
  >(null);
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
    global: {
      memories: [],
      page: 1,
      pageSize: 10,
      totalCount: 0,
      totalPages: 0,
    },
    workspace: {
      memories: [],
      page: 1,
      pageSize: 10,
      totalCount: 0,
      totalPages: 0,
    },
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
  const [workspaceSpec, setWorkspaceSpec] =
    useState<WorkspaceSpecResponse | null>(null);
  const [workspaceSpecDraft, setWorkspaceSpecDraft] = useState("");
  const [isLoadingWorkspaceSpec, setIsLoadingWorkspaceSpec] = useState(false);
  const [workspaceSpecError, setWorkspaceSpecError] = useState<string | null>(
    null,
  );
  const [workspaceSpecConflictMessage, setWorkspaceSpecConflictMessage] =
    useState<string | null>(null);
  const [workspaceSpecPreviewEnabled, setWorkspaceSpecPreviewEnabled] =
    useState(false);
  const [workspaceSpecOperationKey, setWorkspaceSpecOperationKey] = useState<
    "generate" | "save" | "settings" | null
  >(null);
  const [activePlans, setActivePlans] = useState<Plan[]>([]);
  const [loadedActivePlansWorkspaceId, setLoadedActivePlansWorkspaceId] =
    useState<string | null>(null);
  const [isLoadingActivePlans, setIsLoadingActivePlans] = useState(false);
  const [activePlansError, setActivePlansError] = useState<string | null>(null);
  const [planOperationKey, setPlanOperationKey] = useState<string | null>(null);
  const [planAutoRunByWorkspace, setPlanAutoRunByWorkspace] = useState<
    Record<string, PlanAutoRunResponse>
  >({});
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
  const [readOnlyChatKeys, setReadOnlyChatKeys] = useState<
    Record<string, boolean>
  >({});
  const [contextUsageByChatKey, setContextUsageByChatKey] = useState<
    Record<string, ContextUsageResponse>
  >({});
  const [contextUsageLoadingByChatKey, setContextUsageLoadingByChatKey] =
    useState<Record<string, boolean>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSelectingAttachments, setIsSelectingAttachments] = useState(false);
  const [pendingQuestion, setPendingQuestion] =
    useState<QuestionRequestSummary | null>(null);
  const [isAnsweringQuestion, setIsAnsweringQuestion] = useState(false);
  const [questionError, setQuestionError] = useState<string | null>(null);
  const [isRipgrepDialogDismissed, setIsRipgrepDialogDismissed] =
    useState(false);
  const [isInstallingRipgrep, setIsInstallingRipgrep] = useState(false);
  const [ripgrepInstallError, setRipgrepInstallError] = useState<string | null>(
    null,
  );
  const [error, setErrorState] = useState<string | null>(null);
  const [errorDiagnosticReference, setErrorDiagnosticReference] =
    useState<ApiDiagnostic | null>(null);
  const [contextUsageErrorByChatKey, setContextUsageErrorByChatKey] = useState<
    Record<string, { diagnostic: ApiDiagnostic | null; message: string }>
  >({});
  const setError = useCallback((message: string | null) => {
    setErrorState(message);
    setErrorDiagnosticReference(null);
  }, []);
  const setRunError = useCallback((requestError: unknown) => {
    setErrorState(errorMessage(requestError));
    setErrorDiagnosticReference(errorDiagnostic(requestError));
  }, []);
  const activeRunAbortByChatKeyRef = useRef<Map<string, AbortController>>(
    new Map(),
  );
  // A stream can be reattached, retried, or superseded while fetch/readable-stream
  // callbacks from an older subscription are still queued. Keep one explicit owner
  // per chat so those callbacks cannot mutate the newer UI state.
  const chatStreamSessionsByChatKeyRef = useRef<Map<string, ChatStreamSession>>(
    new Map(),
  );
  const chatStreamEpochRef = useRef(0);
  // Assistants that received live stream content for a chat. Survives GET
  // reattach across Coordinator wait handoffs so later `start` events keep
  // history instead of wiping the bubble.
  const liveStreamAssistantIdsByChatKeyRef = useRef<Map<string, Set<string>>>(
    new Map(),
  );
  const contextUsageAbortByChatKeyRef = useRef<Map<string, AbortController>>(
    new Map(),
  );
  const contextUsageIdentityByChatKeyRef = useRef<Map<string, string>>(
    new Map(),
  );
  const skipComposerContextUsageRefreshAfterRunByChatKeyRef = useRef<
    Set<string>
  >(new Set());
  const contextUsageRequestIdByChatKeyRef = useRef<Map<string, number>>(
    new Map(),
  );
  const todoGraphRequestIdRef = useRef(0);
  const themeSaveRevisionRef = useRef(0);
  const themeSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
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
  const workspaceSpecJobObserversRef = useRef<
    Map<string, WorkspaceSpecJobObserver>
  >(new Map());
  const gitDiffRequestRef = useRef<AbortController | null>(null);
  const gitDiffRequestIdRef = useRef(0);
  const gitOperationRequestIdRef = useRef(0);
  const sourceControlTargetIdentityRef = useRef<string | null>(null);
  const sourceControlViewRef = useRef<SourceControlView>({
    chatKey: null,
    isVisible: false,
    selectedDiffPath: null,
    target: null,
    workspaceId: "",
  });
  const gitBranchesRequestRef = useRef<AbortController | null>(null);
  const gitBranchesRequestIdRef = useRef(0);
  const workspacesRefreshGenerationRef = useRef(0);
  const remoteChatsHydrationAbortRef = useRef<AbortController | null>(null);
  const selectedModelIdRef = useRef("");
  const selectedThinkingLevelRef = useRef("");
  const activeChatKeyRef = useRef<string | null>(null);
  const activeWorkspaceIdRef = useRef("");
  const activeChatIdRef = useRef<string | null>(null);
  const workspacesRef = useRef<WorkspaceSummary[]>([]);
  const loadingChatKeysRef = useRef<Set<string>>(new Set());
  const loadingChatControllersRef = useRef<Map<string, AbortController>>(
    new Map(),
  );
  const loadingOlderChatMessageKeysRef = useRef<Set<string>>(new Set());
  const runningChatKeysRef = useRef<Set<string>>(new Set());
  const restoredPendingQuestionIdsRef = useRef<Set<string>>(new Set());
  const isCheckingPendingQuestionsRef = useRef(false);
  const activeRunInfoByChatKeyRef = useRef<Record<string, ActiveRunInfo>>({});
  // Durable terminal identity only. A streamEnd closes one SSE session, but the
  // underlying coordinator run may be reattached later with that same run id.
  const durableRunTerminationByChatKeyRef = useRef<
    Map<string, DurableRunTermination>
  >(new Map());
  const chatStreamHandoffTimersByChatKeyRef = useRef<Map<string, number>>(
    new Map(),
  );
  const chatStreamHandoffsByChatKeyRef = useRef<Map<string, ChatStreamHandoff>>(
    new Map(),
  );
  const queuedRunRequestsByChatKeyRef = useRef<
    Record<string, RetryRunRequest[]>
  >({});
  const scheduledWorkspaceRunsRef = useRef<ScheduledWorkspaceRun[]>([]);
  const failedRestoredQueuedRunKeysRef = useRef<Set<string>>(new Set());
  const pendingGuidanceMessageIdsRef = useRef<Map<string, string>>(new Map());
  const applyBrowserRouteRef = useRef<(route: BrowserRoute) => void>(() => {});
  const hasAppliedInitialBrowserRouteRef = useRef(false);
  const hasManuallySelectedModelRef = useRef(false);
  const hasManuallySelectedThinkingLevelRef = useRef(false);
  const isLoadingSettingsRef = useRef(isLoadingSettings);
  isLoadingSettingsRef.current = isLoadingSettings;
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
    ? (planAutoRunByWorkspace[activeWorkspaceIdForPlanAutoRun] ?? null)
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
          if (
            activeWorkspaceIdRef.current &&
            activeWorkspaceIdRef.current !== workspaceId
          ) {
            return null;
          }
          setPlanAutoRunStateForWorkspace(workspaceId, autoRun);
          return autoRun;
        } catch (requestError) {
          if (
            !activeWorkspaceIdRef.current ||
            activeWorkspaceIdRef.current === workspaceId
          ) {
            setActivePlansError(errorMessage(requestError));
          }
          return null;
        } finally {
          const current = planAutoRunSingleFlightRef.current.get(workspaceId);
          if (current?.promise === promise) {
            current.settled = true;
            window.setTimeout(() => {
              if (
                planAutoRunSingleFlightRef.current.get(workspaceId)?.promise ===
                promise
              ) {
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
      ? (chatMessagePaginationByKeyRef.current[activeChatKey] ?? null)
      : null;
  const isLoadingOlderActiveChatMessages =
    activeChatKey !== null && loadingOlderChatMessageKeys.has(activeChatKey);
  const activeContextUsageKey =
    activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)
      ? chatRunKey(activeWorkspaceId, activeChatId)
      : null;
  const contextUsage = activeContextUsageKey
    ? (contextUsageByChatKey[activeContextUsageKey] ?? null)
    : null;
  const contextUsageError = activeContextUsageKey
    ? (contextUsageErrorByChatKey[activeContextUsageKey] ?? null)
    : null;
  const liveChatStatistics = activeChatKey
    ? (liveChatStatisticsByKey[activeChatKey] ?? null)
    : null;
  const latestProviderUsage =
    activeChatKey !== null && runningChatKeys.has(activeChatKey)
      ? liveChatStatistics
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
      ? (agentTeamSnapshot.instances.find(
          (instance) =>
            instance.id === agentTeamSnapshot.team.coordinatorInstanceId,
        ) ?? null)
      : null;
  const activeChatWorktreeBranch =
    activeChatCoordinatorInstance?.executionWorkspaceMode ===
      "isolated_worktree" &&
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
  const sourceControlTargetScope =
    activeWorkspace?.id && activeChatId
      ? `${activeWorkspace.id}:${activeChatId}`
      : "";
  const sourceControlTarget =
    isSourceControlTargetManual &&
    selectedSourceControlTargetScope === sourceControlTargetScope
      ? (selectedSourceControlTarget ?? defaultSourceControlTarget)
      : defaultSourceControlTarget;
  const sourceControlTargetKeyValue =
    sourceControlTargetKey(sourceControlTarget);
  sourceControlViewRef.current = {
    chatKey: activeChatKey,
    isVisible: isContextPanelOpen && contextPanelTab === "git",
    selectedDiffPath,
    target: sourceControlTarget,
    workspaceId: activeWorkspace?.id ?? "",
  };
  const isLoadingContextUsage = activeContextUsageKey
    ? (contextUsageLoadingByChatKey[activeContextUsageKey] ?? false)
    : false;
  const openChatKeySet = useMemo(
    () =>
      new Set(
        openChatTabs.map((tab) => chatRunKey(tab.workspaceId, tab.chatId)),
      ),
    [openChatTabs],
  );
  const persistedRunningChatKeys = useMemo(() => {
    const keys = new Set<string>();
    for (const workspace of workspaces) {
      for (const chat of workspace.chats) {
        if (isPersistedQueuedRunRunning(chat.queuedRun)) {
          keys.add(chatRunKey(workspace.id, chat.id));
        }
      }
    }
    return keys;
  }, [workspaces]);
  const chatSessionStatusFor = useCallback(
    (
      chatKey: string,
      options: {
        scheduledChatKey?: string | null;
        scheduledStatus?: ScheduledWorkspaceRun["status"] | null;
        workspaceActiveRun?: ActiveChatRunSummary | null;
      } = {},
    ) => {
      const statusChatKeys =
        options.scheduledChatKey && options.scheduledChatKey !== chatKey
          ? [chatKey, options.scheduledChatKey]
          : [chatKey];
      const persistedRunning = statusChatKeys.some((statusChatKey) =>
        persistedRunningChatKeys.has(statusChatKey),
      );
      return deriveChatSessionStatus({
        activeChatKey,
        activeRunInfoByChatKey,
        chatKey,
        failedChatKeySet,
        openChatKeySet,
        persistedRunning,
        runningChatKeys,
        scheduledChatKey: options.scheduledChatKey,
        scheduledStatus: options.scheduledStatus,
        terminalRunId:
          durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId ??
          chatStreamHandoffsByChatKeyRef.current.get(chatKey)?.runId ??
          null,
        workspaceActiveRun: options.workspaceActiveRun ?? null,
      });
    },
    [
      activeChatKey,
      activeRunInfoByChatKey,
      failedChatKeySet,
      openChatKeySet,
      persistedRunningChatKeys,
      runningChatKeys,
    ],
  );
  // Derive Agent tab spinner from live/cached team snapshot; no copied React state.
  const agentInstanceIsRunning = useCallback(
    (workspaceId: string, chatId: string, instanceId: string): boolean => {
      const chatKey = chatRunKey(workspaceId, chatId);
      const snapshot =
        (agentTeamSnapshotChatKeyRef.current === chatKey
          ? agentTeamSnapshot
          : null) ??
        agentTeamSnapshotCacheRef.current.get(chatKey) ??
        null;
      if (!snapshot) {
        return false;
      }
      return snapshot.instances.some(
        (instance) =>
          instance.id === instanceId && instance.status === "running",
      );
    },
    [agentTeamSnapshot],
  );
  const activeChatSessionStatus = activeChatKey
    ? chatSessionStatusFor(activeChatKey)
    : { activeRun: null, kind: "idle" as const };
  const activeRunInfo = activeChatKey
    ? (activeRunInfoByChatKey[activeChatKey] ?? null)
    : null;
  const activeChatReadOnly = activeChatKey
    ? readOnlyChatKeys[activeChatKey] === true
    : false;
  const canUseTeamMode = agentDefinitions.length > 1;
  const isActiveChatSessionRunning =
    activeChatSessionStatus.kind === "running";
  // All chat surfaces share the same durable session status. A suspended
  // coordinator remains visually running, but it has no live run identity to
  // cancel or guide until the scheduler reattaches a fresh attempt.
  const isSendingMessage =
    isActiveChatSessionRunning && activeChatSessionStatus.activeRun !== null;
  const queuedRunRequests = activeChatKey
    ? (queuedRunRequestsByChatKey[activeChatKey] ?? [])
    : [];
  const queuedMessageIds = useMemo(
    () =>
      new Set(
        queuedRunRequests.flatMap(
          (request) => request.pendingUserMessageId ?? [],
        ),
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
      ...openHtmlPreviewTabs.map((tab) => ({
        ...tab,
        title: tab.name,
        type: "htmlPreview" as const,
      })),
    ],
    [
      openAgentTabs,
      openChatTabs,
      openFileTabs,
      openHtmlPreviewTabs,
      workspaces,
    ],
  );
  const activeFileEditorKey =
    activeMainTab.type === "file"
      ? workspaceFileEditorKey(activeMainTab.workspaceId, activeMainTab.path)
      : null;
  const activeFileTab =
    activeMainTab.type === "file"
      ? (openFileTabs.find(
          (tab) =>
            tab.workspaceId === activeMainTab.workspaceId &&
            tab.path === activeMainTab.path,
        ) ?? null)
      : null;
  const activeHtmlPreviewTab =
    activeMainTab.type === "htmlPreview"
      ? (openHtmlPreviewTabs.find(
          (tab) =>
            tab.workspaceId === activeMainTab.workspaceId &&
            tab.path === activeMainTab.path,
        ) ?? null)
      : null;
  const activeAgentTab =
    activeMainTab.type === "agent"
      ? (mainTabs.find(
          (tab): tab is Extract<MainTabSummary, { type: "agent" }> =>
            tab.type === "agent" &&
            tab.workspaceId === activeMainTab.workspaceId &&
            tab.chatId === activeMainTab.chatId &&
            tab.instanceId === activeMainTab.instanceId,
        ) ?? null)
      : null;
  const activeFileEditor = activeFileEditorKey
    ? (workspaceFileEditors[activeFileEditorKey] ?? null)
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
        unsupportedAttachmentInputModality(
          selectedModel,
          attachment.contentType,
        ),
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
            : (agentModel.providerIds[0] ?? "");
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
      model.activeProviderId &&
      model.providerIds.includes(model.activeProviderId)
        ? model.activeProviderId
        : (model.providerIds[0] ?? "");

    return {
      modelId: model.id,
      providerId,
      thinkingLevel: defaultThinkingLevelForModel(model),
    };
  }, [availableModels, defaultAgentDefinition]);
  // Latest catalog/settings for async restore paths (e.g. loadChatMessages).
  // Closures must not apply a stale empty catalog after settings have arrived.
  const availableModelsRef = useRef(availableModels);
  availableModelsRef.current = availableModels;
  const defaultComposerSelectionRef = useRef(defaultComposerSelection);
  defaultComposerSelectionRef.current = defaultComposerSelection;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const {
    skills: availableSkills,
    status: skillCatalogStatus,
    error: skillCatalogError,
    refreshError: skillCatalogRefreshError,
    reload: reloadWorkspaceSkillCatalog,
  } = useWorkspaceSkillCatalog(activeWorkspace?.id ?? null);
  // Only emit skillIds that belong to an authoritative ready catalog for the
  // active workspace. Cross-workspace loading keeps selectedSkillIds in memory
  // until prune, but must not attach the previous workspace's keys to send,
  // queue, guidance, or context-usage requests.
  const effectiveSelectedSkillIds = useMemo(() => {
    if (skillCatalogStatus !== "ready") {
      return [] as string[];
    }

    const enabledSkillIds = new Set(availableSkills.map((skill) => skill.key));
    return selectedSkillIds.filter((skillId) => enabledSkillIds.has(skillId));
  }, [availableSkills, selectedSkillIds, skillCatalogStatus]);
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const selectedRequestThinkingLevel = isModelThinkingLevelSupported(
    selectedModel,
    selectedThinkingLevel,
  )
    ? selectedThinkingLevel
    : "";
  const selectedRequestLatencyMode = latencyModeForModel(selectedModel);
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

  const workspaceSidebarMaxWidth = workspaceSidebarMaxWidthForViewport(
    sidebarViewportWidth,
    sidebarSafeAreaInsetWidth,
  );
  const workspaceSidebarMinWidth = Math.min(
    WORKSPACE_SIDEBAR_MIN_WIDTH,
    workspaceSidebarMaxWidth,
  );
  const clampWorkspaceSidebarWidth = useCallback((width: number) => {
    return Math.min(
      Math.max(width, workspaceSidebarMinWidth),
      workspaceSidebarMaxWidth,
    );
  }, [workspaceSidebarMaxWidth, workspaceSidebarMinWidth]);

  const updateSidebarWidthFromClientX = useCallback((clientX: number) => {
    const sidebarLeft =
      workspaceSidebarRef.current?.getBoundingClientRect().left ?? 0;
    const nextWidth = clientX - sidebarLeft;

    setSidebarWidth(clampWorkspaceSidebarWidth(nextWidth));
  }, [clampWorkspaceSidebarWidth]);

  useEffect(() => {
    function updateSidebarWidthForViewport() {
      const viewportWidth = window.innerWidth;
      const safeAreaInsetWidth = mobileSafeAreaInsetWidth();
      const maxWidth = workspaceSidebarMaxWidthForViewport(
        viewportWidth,
        safeAreaInsetWidth,
      );

      setSidebarViewportWidth(viewportWidth);
      setSidebarSafeAreaInsetWidth(safeAreaInsetWidth);
      setSidebarWidth((current) =>
        Math.min(Math.max(current, Math.min(WORKSPACE_SIDEBAR_MIN_WIDTH, maxWidth)), maxWidth),
      );
    }

    updateSidebarWidthForViewport();
    window.addEventListener("resize", updateSidebarWidthForViewport);
    return () => {
      window.removeEventListener("resize", updateSidebarWidthForViewport);
    };
  }, []);

  const previewContextPanelHeight = useCallback((value: number) => {
    appShellRef.current?.style.setProperty(
      "--context-panel-mobile-height",
      `${value}px`,
    );
  }, []);

  const previewContextPanelWidth = useCallback((value: number) => {
    appShellRef.current?.style.setProperty("--diff-panel-width", `${value}px`);
  }, []);

  const handleContextPanelResizeStart = useCallback(
    (session: PanelResizeDragSession) => {
      contextPanelResizeDragRef.current = session;
      if (session.stacked) {
        previewContextPanelHeight(session.startHeight);
      } else {
        previewContextPanelWidth(session.startWidth);
      }
      setIsResizingDiffPanel(true);
    },
    [previewContextPanelHeight, previewContextPanelWidth],
  );

  const handleContextPanelResizeEnd = useCallback(
    (finalSize: { height: number; stacked: boolean; width: number }) => {
      contextPanelResizeDragRef.current = null;
      if (finalSize.stacked) {
        setContextPanelMobileHeight(finalSize.height);
        previewContextPanelHeight(finalSize.height);
      } else {
        setDiffPanelWidth(finalSize.width);
        previewContextPanelWidth(finalSize.width);
      }
      setIsResizingDiffPanel(false);
    },
    [previewContextPanelHeight, previewContextPanelWidth],
  );
  const updateBrowserRoute = useCallback(
    (route: BrowserRoute, mode: "push" | "replace" = "push") => {
      if (typeof window === "undefined") {
        return;
      }

      const routeWithTabs =
        route.viewMode === "chat"
          ? browserRouteWithOpenTabs(
              route,
              openChatTabsRef.current,
              openFileTabsRef.current,
              openHtmlPreviewTabsRef.current,
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
    openHtmlPreviewTabsRef.current = openHtmlPreviewTabs;
  }, [openHtmlPreviewTabs]);

  useEffect(() => {
    workspacesRef.current = workspaces;
  }, [workspaces]);

  useEffect(() => {
    activeWorkspaceIdRef.current = activeWorkspaceId;
    for (const [
      workspaceId,
      observer,
    ] of workspaceSpecJobObserversRef.current) {
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
      ...effectiveSelectedSkillIds,
    ].join("\u0000");

    if (!chatKey) {
      return;
    }

    const matchesCurrentIdentity =
      contextUsageIdentityByChatKeyRef.current.get(chatKey) === identity;

    const composerRefreshAction = composerContextUsageRefreshAction({
      hasPendingSkip: isActiveChatSessionRunning
        ? false
        : skipComposerContextUsageRefreshAfterRunByChatKeyRef.current.delete(
            chatKey,
          ),
      isSendingMessage: isActiveChatSessionRunning,
      matchesCurrentIdentity,
    });
    if (composerRefreshAction === "record-skip") {
      // A model selection made while a run is active must not be replayed as a
      // composer context-usage request immediately after the run finishes. The
      // terminal refresh belongs to the run's immutable route instead.
      skipComposerContextUsageRefreshAfterRunByChatKeyRef.current.add(chatKey);
      return;
    }

    if (composerRefreshAction !== "refresh") {
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
        skillIds: effectiveSelectedSkillIds,
        thinkingLevel: selectedRequestThinkingLevel,
        workspaceId: activeWorkspaceId,
      });
    }
  }, [
    activeChatId,
    activeWorkspaceId,
    effectiveSelectedSkillIds,
    isActiveChatSessionRunning,
    selectedModelId,
    selectedProviderId,
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

  useEffect(() => {
    if (!workspaceFileContextMenu) {
      workspaceFileContextMenuRef.current = null;
      return;
    }

    function closeWorkspaceFileContextMenuForScroll(event: Event) {
      const target = event.target;
      if (
        target === document ||
        target === document.documentElement ||
        target === document.body ||
        target === window
      ) {
        setWorkspaceFileContextMenu(null);
        return;
      }
      if (target instanceof Element && target.closest(".context-panel")) {
        setWorkspaceFileContextMenu(null);
      }
    }

    function closeWorkspaceFileContextMenu() {
      setWorkspaceFileContextMenu(null);
    }

    window.addEventListener("resize", closeWorkspaceFileContextMenu);
    window.addEventListener(
      "scroll",
      closeWorkspaceFileContextMenuForScroll,
      true,
    );

    return () => {
      window.removeEventListener("resize", closeWorkspaceFileContextMenu);
      window.removeEventListener(
        "scroll",
        closeWorkspaceFileContextMenuForScroll,
        true,
      );
    };
  }, [workspaceFileContextMenu]);

  useLayoutEffect(() => {
    if (!workspaceFileContextMenu || workspaceFileContextMenu.positioned) {
      return;
    }

    if (typeof window === "undefined") {
      return;
    }

    function clampToViewport(element: HTMLElement) {
      const margin = 8;
      const rect = element.getBoundingClientRect();
      workspaceFileContextMenuRef.current = element;
      setWorkspaceFileContextMenu((current) => {
        if (!current || current.positioned) {
          return current;
        }
        return {
          ...current,
          left: Math.max(
            margin,
            Math.min(current.left, window.innerWidth - rect.width - margin),
          ),
          positioned: true,
          top: Math.max(
            margin,
            Math.min(current.top, window.innerHeight - rect.height - margin),
          ),
        };
      });
    }

    const cached = workspaceFileContextMenuRef.current;
    const element =
      cached && cached.isConnected
        ? cached
        : (document.querySelector(
            ".workspace-file-context-menu",
          ) as HTMLElement | null);

    if (element) {
      clampToViewport(element);
      return;
    }

    // Popover may mount one frame after controlled isOpen; retry once, then
    // show unclamped rather than staying forever hidden.
    const retryId = window.requestAnimationFrame(() => {
      const retry = document.querySelector(
        ".workspace-file-context-menu",
      ) as HTMLElement | null;
      if (retry) {
        clampToViewport(retry);
        return;
      }
      setWorkspaceFileContextMenu((current) =>
        current && !current.positioned
          ? { ...current, positioned: true }
          : current,
      );
    });
    return () => window.cancelAnimationFrame(retryId);
  }, [workspaceFileContextMenu]);

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
      for (const session of chatStreamSessionsByChatKeyRef.current.values()) {
        session.abortController.abort();
      }
      chatStreamSessionsByChatKeyRef.current.clear();
      for (const timer of chatStreamHandoffTimersByChatKeyRef.current.values()) {
        window.clearTimeout(timer);
      }
      chatStreamHandoffTimersByChatKeyRef.current.clear();
      chatStreamHandoffsByChatKeyRef.current.clear();

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
            t(
              "Pending question chat is no longer available: {workspaceId}/{chatId}",
              {
                chatId: question.chatId,
                workspaceId: question.workspaceId,
              },
            ),
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
          const nextTitle = titleByChatKey.get(
            chatRunKey(tab.workspaceId, tab.chatId),
          );
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

  const reconcileActiveWorkspaceSelection = useCallback(
    (
      nextWorkspaces: WorkspaceSummary[],
      activeWorkspaceIdFromServer: string | null | undefined,
    ) => {
      const previousWorkspaceId = activeWorkspaceIdRef.current;
      const previousStillPresent = nextWorkspaces.some(
        (workspace) => workspace.id === previousWorkspaceId,
      );
      if (!previousStillPresent) {
        const nextWorkspaceId = activeWorkspaceIdFromServer ?? "";
        // Same class of bug as add-workspace: do not keep the previous chatId when
        // the active workspace disappears and we fall back to another workspace.
        if (previousWorkspaceId) {
          activeWorkspaceIdRef.current = nextWorkspaceId;
          activeChatIdRef.current = null;
          activeChatKeyRef.current = null;
          setActiveWorkspaceId(nextWorkspaceId);
          setActiveChatId(null);
          setActiveMainTab({
            chatId: null,
            type: "chat",
            workspaceId: nextWorkspaceId,
          });
          setMessages([]);
          setSelectedDiffPath(null);
          // Keep React state and address bar aligned so refresh does not revive the
          // removed workspace/chat path. Leave non-chat views (settings, etc.) alone.
          if (currentBrowserRoute().viewMode === "chat") {
            updateBrowserRoute(
              {
                chatId: null,
                viewMode: "chat",
                workspaceId: nextWorkspaceId || null,
              },
              "replace",
            );
          }
        } else {
          setActiveWorkspaceId(nextWorkspaceId);
        }
        setExpandedWorkspaceId((current) =>
          current !== null &&
          nextWorkspaces.some((workspace) => workspace.id === current)
            ? current
            : nextWorkspaceId || null,
        );
      } else {
        setExpandedWorkspaceId((current) =>
          current !== null &&
          nextWorkspaces.some((workspace) => workspace.id === current)
            ? current
            : (activeWorkspaceIdFromServer ?? null),
        );
      }
    },
    [updateBrowserRoute],
  );

  const applyRemoteWorkspaceChatsHydration = useCallback(
    (workspaceId: string, data: WorkspaceChatsResponse, generation: number) => {
      if (workspacesRefreshGenerationRef.current !== generation) {
        return;
      }

      setWorkspaces((current) => {
        if (workspacesRefreshGenerationRef.current !== generation) {
          return current;
        }
        if (!current.some((workspace) => workspace.id === workspaceId)) {
          return current;
        }
        const next = applyRemoteWorkspaceChatsPatch(current, workspaceId, data);
        if (next === current) {
          return current;
        }
        workspacesRef.current = next;
        syncOpenChatTabTitlesFromWorkspaces(next);
        return next;
      });

      setWorkspaceChatPaging((current) => {
        if (workspacesRefreshGenerationRef.current !== generation) {
          return current;
        }
        if (
          !(workspaceId in current) &&
          !workspacesRef.current.some((item) => item.id === workspaceId)
        ) {
          return current;
        }
        return {
          ...current,
          [workspaceId]: {
            hasMore: data.hasMore,
            isLoading: false,
            nextCursor: data.nextCursor,
            total: data.total,
          },
        };
      });
    },
    [syncOpenChatTabTitlesFromWorkspaces],
  );

  const hydrateRemoteWorkspaceChatsInBackground = useCallback(
    (workspacesToHydrate: WorkspaceSummary[], generation: number) => {
      remoteChatsHydrationAbortRef.current?.abort();
      const abortController = new AbortController();
      remoteChatsHydrationAbortRef.current = abortController;

      for (const workspace of workspacesToHydrate) {
        if (!shouldHydrateRemoteWorkspaceChats(workspace)) {
          continue;
        }

        const workspaceId = workspace.id;
        const params = new URLSearchParams({
          limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE),
        });
        void requestJson<WorkspaceChatsResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats?${params.toString()}`,
          { signal: abortController.signal },
        )
          .then((data) => {
            if (abortController.signal.aborted) {
              return;
            }
            applyRemoteWorkspaceChatsHydration(workspaceId, data, generation);
          })
          .catch((requestError) => {
            if (isAbortError(requestError) || abortController.signal.aborted) {
              return;
            }
            // Local isolation: keep base connection status / Retry UI; never set app error.
          });
      }
    },
    [applyRemoteWorkspaceChatsHydration],
  );

  const refreshWorkspaces = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>("/api/workspaces");
      const generation = workspacesRefreshGenerationRef.current + 1;
      workspacesRefreshGenerationRef.current = generation;
      remoteChatsHydrationAbortRef.current?.abort();

      const baseWorkspaces = normalizeBaseWorkspaceSummaries(
        data.workspaces,
        workspacesRef.current,
      );
      workspacesRef.current = baseWorkspaces;
      setWorkspaces(baseWorkspaces);
      reconcileHandoffsFromWorkspaceSummaries(baseWorkspaces);
      syncOpenChatTabTitlesFromWorkspaces(baseWorkspaces);
      setWorkspaceChatPaging(workspaceChatPagingFromWorkspaces(baseWorkspaces));
      reconcileActiveWorkspaceSelection(baseWorkspaces, data.activeWorkspaceId);
      setIsLoading(false);
      hydrateRemoteWorkspaceChatsInBackground(baseWorkspaces, generation);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setIsLoading(false);
    }
  }, [
    hydrateRemoteWorkspaceChatsInBackground,
    reconcileActiveWorkspaceSelection,
    syncOpenChatTabTitlesFromWorkspaces,
  ]);

  const loadSettings = useCallback(async () => {
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      settingsSkillsSnapshotRef.current = JSON.stringify(data.skills);
      settingsModelsSnapshotRef.current = JSON.stringify(data.configuredModels);
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
        const data = await requestJson<UpdateModelRouteResponse>(
          "/api/models/route",
          {
            body: JSON.stringify({ modelId, providerId }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
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
                    ? {
                        ...model,
                        activeProviderId: data.activeProviderId,
                        fastModeEnabled:
                          data.configuredModels.find(
                            (updated) => updated.id === data.modelId,
                          )?.fastModeEnabled ?? false,
                        supportsFast:
                          data.configuredModels.find(
                            (updated) => updated.id === data.modelId,
                          )?.supportsFast ?? false,
                      }
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
                ? {
                    ...model,
                    activeProviderId: previousActiveProviderId ?? null,
                  }
                : model,
            ),
          };
        });
        return {
          ok: false as const,
          error:
            errorMessage(requestError) || t("Failed to update model route"),
        };
      }
    },
    [t],
  );

  const updateModelFastMode = useCallback(
    async (modelId: string, fastModeEnabled: boolean) => {
      let previousFastModeEnabled: boolean | undefined;
      setSettings((current) => {
        if (!current) {
          return current;
        }
        const existing = current.configuredModels.find(
          (model) => model.id === modelId,
        );
        if (previousFastModeEnabled === undefined) {
          previousFastModeEnabled = existing?.fastModeEnabled ?? false;
        }
        if (!existing || existing.fastModeEnabled === fastModeEnabled) {
          return current;
        }
        return {
          ...current,
          configuredModels: current.configuredModels.map((model) =>
            model.id === modelId ? { ...model, fastModeEnabled } : model,
          ),
        };
      });

      try {
        const data = await requestJson<UpdateModelFastModeResponse>(
          "/api/models/fast-mode",
          {
            body: JSON.stringify({ modelId, fastModeEnabled }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        setSettings((current) =>
          current
            ? {
                ...current,
                configuredModels: current.configuredModels.map((model) =>
                  model.id === data.modelId
                    ? {
                        ...model,
                        fastModeEnabled: data.fastModeEnabled,
                        supportsFast:
                          data.configuredModels.find(
                            (updated) => updated.id === data.modelId,
                          )?.supportsFast ?? model.supportsFast,
                      }
                    : model,
                ),
              }
            : current,
        );
        return { ok: true as const };
      } catch (requestError) {
        setSettings((current) => {
          if (!current || previousFastModeEnabled === undefined) {
            return current;
          }
          return {
            ...current,
            configuredModels: current.configuredModels.map((model) =>
              model.id === modelId
                ? { ...model, fastModeEnabled: previousFastModeEnabled }
                : model,
            ),
          };
        });
        return {
          ok: false as const,
          error:
            errorMessage(requestError) || t("Failed to update Fast mode"),
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
      setUpdateInstallNotice(
        t("Foco is installing the update and will restart shortly."),
      );
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
      const requestSequence =
        agentTeamSnapshotRequestSequenceRef.current + 1;
      agentTeamSnapshotRequestSequenceRef.current = requestSequence;
      latestAgentTeamSnapshotRequestByChatKeyRef.current.set(
        requestedChatKey,
        requestSequence,
      );
      const isLatestAgentTeamRequest = () =>
        latestAgentTeamSnapshotRequestByChatKeyRef.current.get(
          requestedChatKey,
        ) === requestSequence;
      const isCurrentAgentTeamRequest = () =>
        activeChatKeyRef.current === requestedChatKey;
      const silent =
        options?.silent ??
        agentTeamSnapshotChatKeyRef.current === requestedChatKey;

      if (!silent) {
        loadingAgentTeamSnapshotRequestByChatKeyRef.current.set(
          requestedChatKey,
          requestSequence,
        );
        setIsLoadingAgentTeam(true);
      }
      setAgentTeamError(null);

      try {
        const data = await requestJson<AgentTeamSnapshotResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/agent-team`,
        );
        if (!isLatestAgentTeamRequest()) {
          return data;
        }
        agentTeamSnapshotCacheRef.current.set(requestedChatKey, data);
        if (isCurrentAgentTeamRequest()) {
          agentTeamSnapshotChatKeyRef.current = requestedChatKey;
          setAgentTeamSnapshot(data);
        }
        return data;
      } catch (requestError) {
        const message = errorMessage(requestError);
        if (isLatestAgentTeamRequest() && isCurrentAgentTeamRequest()) {
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
        if (
          !silent &&
          loadingAgentTeamSnapshotRequestByChatKeyRef.current.get(
            requestedChatKey,
          ) === requestSequence
        ) {
          loadingAgentTeamSnapshotRequestByChatKeyRef.current.delete(
            requestedChatKey,
          );
          if (isCurrentAgentTeamRequest()) {
            setIsLoadingAgentTeam(false);
          }
        }
      }
    },
    [],
  );

  const handleAgentTeamRefresh = useCallback(
    (event: Extract<ChatStreamEvent, { type: "agentTeamRefresh" }>) => {
      if (
        activeChatKeyRef.current !== chatRunKey(event.workspaceId, event.chatId)
      ) {
        return;
      }

      if (event.revealPanel) {
        setContextPanelTab("agents");
        setIsContextPanelOpen(true);
      }
      void loadAgentTeamSnapshot(event.workspaceId, event.chatId, {
        silent: true,
      });
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
    const cachedSnapshot =
      agentTeamSnapshotCacheRef.current.get(requestedChatKey);
    if (cachedSnapshot) {
      agentTeamSnapshotChatKeyRef.current = requestedChatKey;
      setAgentTeamSnapshot(cachedSnapshot);
      setAgentTeamError(null);
      void loadAgentTeamSnapshot(activeWorkspaceId, activeChatId, {
        silent: true,
      });
      return;
    }

    void loadAgentTeamSnapshot(activeWorkspaceId, activeChatId);
  }, [
    activeChatId,
    activeMainTab.type,
    activeWorkspaceId,
    canUseApp,
    loadAgentTeamSnapshot,
  ]);

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

  const visibleAgentSnapshotHasUnsettledTask = Boolean(
    visibleAgentSnapshotTarget &&
    agentTeamSnapshot?.team.chatId === visibleAgentSnapshotTarget.chatId &&
    agentTeamSnapshot.tasks.some((task) =>
      UNSETTLED_AGENT_TASK_STATUSES.has(task.status),
    ),
  );

  useEffect(() => {
    if (
      !canUseApp ||
      !visibleAgentSnapshotTarget ||
      !visibleAgentSnapshotHasUnsettledTask
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
      }, AGENT_TEAM_UNSETTLED_REFRESH_MS);
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
    visibleAgentSnapshotHasUnsettledTask,
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
      setLoadingWorkspaceDirectoryPaths((current) =>
        new Set(current).add(path),
      );
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
                root: replaceWorkspaceFileNodeChildren(
                  current.root,
                  data.path,
                  data.children,
                ),
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

  const invalidateGitDiffRequest = useCallback(() => {
    gitDiffRequestRef.current?.abort();
    gitDiffRequestRef.current = null;
    gitDiffRequestIdRef.current += 1;
    setIsLoadingDiff(false);
  }, []);

  const loadGitDiff = useCallback(
    async (
      workspaceId: string,
      path: string | null,
      target?: SourceControlTarget | null,
    ) => {
      if (activeWorkspaceIdRef.current !== workspaceId) {
        return null;
      }

      const requestedTargetKey = sourceControlTargetKey(target ?? null);
      invalidateGitDiffRequest();
      const requestId = gitDiffRequestIdRef.current;
      const abortController = new AbortController();
      gitDiffRequestRef.current = abortController;
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
          { signal: abortController.signal },
        );
        if (
          abortController.signal.aborted ||
          gitDiffRequestIdRef.current !== requestId ||
          activeWorkspaceIdRef.current !== workspaceId ||
          sourceControlViewRef.current.workspaceId !== workspaceId ||
          sourceControlTargetKey(sourceControlViewRef.current.target) !==
            requestedTargetKey
        ) {
          return null;
        }
        setGitDiff(data);
        setSelectedDiffPath(
          path && data.files.some((file) => file.path === path) ? path : null,
        );
        return data;
      } catch (requestError) {
        if (
          abortController.signal.aborted ||
          gitDiffRequestIdRef.current !== requestId ||
          activeWorkspaceIdRef.current !== workspaceId ||
          sourceControlViewRef.current.workspaceId !== workspaceId ||
          sourceControlTargetKey(sourceControlViewRef.current.target) !==
            requestedTargetKey
        ) {
          return null;
        }
        setGitDiff(null);
        setDiffError(errorMessage(requestError));
        return null;
      } finally {
        if (gitDiffRequestRef.current === abortController) {
          gitDiffRequestRef.current = null;
        }
        if (
          gitDiffRequestIdRef.current === requestId &&
          activeWorkspaceIdRef.current === workspaceId
        ) {
          setIsLoadingDiff(false);
        }
      }
    },
    [invalidateGitDiffRequest],
  );

  const loadContextMemories = useCallback(
    async (workspaceId: string) => {
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
          requestJson<MemoryListResponse>(
            `/api/memory?${globalParams.toString()}`,
          ),
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
          global: {
            memories: [],
            page: 1,
            pageSize: 10,
            totalCount: 0,
            totalPages: 0,
          },
          workspace: {
            memories: [],
            page: 1,
            pageSize: 10,
            totalCount: 0,
            totalPages: 0,
          },
        });
        setContextMemoryError(errorMessage(requestError));
      } finally {
        setIsLoadingContextMemories(false);
      }
    },
    [contextMemoryPages],
  );

  const loadWorkspaceSpec = useCallback(async (workspaceId: string) => {
    setIsLoadingWorkspaceSpec(true);
    setWorkspaceSpecError(null);
    setWorkspaceSpecConflictMessage(null);

    try {
      const data = await requestJson<WorkspaceSpecResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec`,
      );
      if (
        activeWorkspaceIdRef.current &&
        activeWorkspaceIdRef.current !== workspaceId
      ) {
        return null;
      }
      setWorkspaceSpec(data);
      setWorkspaceSpecDraft(data.contentMarkdown);
      setWorkspaceSpecPreviewEnabled(data.contentMarkdown.trim().length > 0);
      return data;
    } catch (requestError) {
      if (
        activeWorkspaceIdRef.current &&
        activeWorkspaceIdRef.current !== workspaceId
      ) {
        return null;
      }
      setWorkspaceSpec(null);
      setWorkspaceSpecDraft("");
      setWorkspaceSpecPreviewEnabled(false);
      setWorkspaceSpecError(errorMessage(requestError));
      return null;
    } finally {
      if (
        !activeWorkspaceIdRef.current ||
        activeWorkspaceIdRef.current === workspaceId
      ) {
        setIsLoadingWorkspaceSpec(false);
      }
    }
  }, []);

  const loadActivePlans = useCallback(
    (workspaceId: string, options: { force?: boolean } = {}) => {
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
          if (
            activeWorkspaceIdRef.current &&
            activeWorkspaceIdRef.current !== workspaceId
          ) {
            return null;
          }
          setActivePlans(data.plans);
          setLoadedActivePlansWorkspaceId(workspaceId);
          return data;
        } catch (requestError) {
          if (
            activeWorkspaceIdRef.current &&
            activeWorkspaceIdRef.current !== workspaceId
          ) {
            return null;
          }
          setActivePlans([]);
          setLoadedActivePlansWorkspaceId(null);
          setActivePlansError(errorMessage(requestError));
          return null;
        } finally {
          if (
            !activeWorkspaceIdRef.current ||
            activeWorkspaceIdRef.current === workspaceId
          ) {
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
                if (
                  activePlansSingleFlightRef.current.get(workspaceId)
                    ?.promise === promise
                ) {
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
    },
    [],
  );

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
        if (
          activeWorkspaceIdRef.current &&
          activeWorkspaceIdRef.current !== workspaceId
        ) {
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
        const plansResponse = await loadActivePlans(workspaceId, {
          force: true,
        });
        await refreshWorkspaces();
        const plan =
          action === "retry_merge"
            ? response.plan
            : (plansResponse?.plans.find(
                (candidate) => candidate.id === planId,
              ) ?? response.plan);
        if (action === "retry_merge") {
          setActivePlans((current) =>
            current.map((candidate) =>
              candidate.id === planId ? plan : candidate,
            ),
          );
        }
        const implementationChatId =
          action === "start" || action === "resume"
            ? (plan.phases.find((phase) => phase.id === plan.activePhaseId)
                ?.implementationChatId ?? null)
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
        const plansResponse = await loadActivePlans(workspaceId, {
          force: true,
        });
        const plan =
          plansResponse?.plans.find((candidate) => candidate.id === planId) ??
          response.plan;
        const retriedPhase =
          plan.phases.find((phase) => phase.id === phaseId) ?? null;
        setPendingPlanPhaseRetryRefresh(
          plansResponse &&
            !planPhaseRetryRefreshStillRunning(
              plansResponse.plans,
              refreshTarget,
            )
            ? null
            : refreshTarget,
        );
        await refreshWorkspaces();
        const chatId =
          retriedPhase?.implementationChatId ?? implementationChatId;
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
            const delayMs = workspaceSpecJobPollDelayMs(pollIndex);
            pollIndex += 1;
            await new Promise<void>((resolve) => {
              window.setTimeout(resolve, delayMs);
            });
            if (
              observer.cancelled ||
              activeWorkspaceIdRef.current !== workspaceId
            ) {
              return;
            }

            let jobsResponse: WorkspaceSpecJobsResponse;
            try {
              jobsResponse = await fetchWorkspaceSpecJobsList(workspaceId, 24);
            } catch (requestError) {
              if (
                !observer.cancelled &&
                activeWorkspaceIdRef.current === workspaceId
              ) {
                setWorkspaceSpecError(errorMessage(requestError));
              }
              continue;
            }
            if (
              observer.cancelled ||
              activeWorkspaceIdRef.current !== workspaceId
            ) {
              return;
            }

            setWorkspaceSpecError(null);
            const job = jobsResponse.jobs.find(
              (candidate) => candidate.id === jobId,
            );
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
          if (
            workspaceSpecJobObserversRef.current.get(workspaceId) === observer
          ) {
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
    async (workspaceId: string, enabled: boolean, injectEnabled: boolean) => {
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
            workspaceId: memory.scope === "global" ? null : activeWorkspace.id,
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
      const existing =
        chatStatisticsSingleFlightRef.current.get(requestedChatKey);
      if (shouldReuseRequest(existing, nowMs)) {
        return existing!.promise;
      }

      const requestId =
        (chatStatisticsRequestIdByChatKeyRef.current.get(requestedChatKey) ??
          0) + 1;
      chatStatisticsRequestIdByChatKeyRef.current.set(
        requestedChatKey,
        requestId,
      );
      const isCurrentStatisticsRequest = () =>
        chatStatisticsRequestIdByChatKeyRef.current.get(requestedChatKey) ===
          requestId && activeChatKeyRef.current === requestedChatKey;

      setIsLoadingChatStatistics(true);
      setChatStatisticsError(null);

      let promise: Promise<void> = Promise.resolve();
      promise = (async () => {
        try {
          const data = await requestJson<ChatStatisticsResponse>(
            `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/statistics`,
          );
          if (isCurrentStatisticsRequest()) {
            setChatStatistics(
              normalizeChatStatistics(data, workspaceId, chatId),
            );
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
          if (
            chatStatisticsRequestIdByChatKeyRef.current.get(
              requestedChatKey,
            ) === requestId
          ) {
            chatStatisticsRequestIdByChatKeyRef.current.delete(
              requestedChatKey,
            );
          }
          const current =
            chatStatisticsSingleFlightRef.current.get(requestedChatKey);
          if (current?.promise === promise) {
            current.settled = true;
            window.setTimeout(() => {
              if (
                chatStatisticsSingleFlightRef.current.get(requestedChatKey)
                  ?.promise === promise
              ) {
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
    } catch (requestError) {
      if (abortController.signal.aborted) {
        return;
      }
      if (gitBranchesRequestIdRef.current !== requestId) {
        return;
      }
      setGitBranches(null);
    } finally {
      if (gitBranchesRequestRef.current === abortController) {
        gitBranchesRequestRef.current = null;
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
    return () => {
      remoteChatsHydrationAbortRef.current?.abort();
    };
  }, []);

  useEffect(() => {
    if (!canUseApp || !updateStatus?.autoCheckEnabled) {
      return;
    }

    const intervalId = window.setInterval(
      () => {
        void loadUpdateStatus();
      },
      10 * 60 * 1000,
    );

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
    invalidateGitDiffRequest();
    gitOperationRequestIdRef.current += 1;
    setGitDiff(null);
    setSelectedDiffPath(null);
    setDiffError(null);
    setIsLoadingDiff(false);
    setGitOperationKey(null);
    setGitCommitMessage("");

    return () => {
      invalidateGitDiffRequest();
    };
  }, [activeWorkspace?.id, invalidateGitDiffRequest]);

  useEffect(() => {
    const targetIdentity = [
      activeWorkspace?.id ?? "",
      sourceControlTargetKeyValue,
    ].join("\u0000");
    const previousTargetIdentity = sourceControlTargetIdentityRef.current;
    sourceControlTargetIdentityRef.current = targetIdentity;
    if (
      previousTargetIdentity === null ||
      previousTargetIdentity === targetIdentity
    ) {
      return;
    }

    invalidateGitDiffRequest();
    gitOperationRequestIdRef.current += 1;
    setGitDiff(null);
    setSelectedDiffPath(null);
    setDiffError(null);
    setIsLoadingDiff(false);
    setGitOperationKey(null);
    setGitCommitMessage("");
  }, [
    activeWorkspace?.id,
    invalidateGitDiffRequest,
    sourceControlTargetKeyValue,
  ]);

  useEffect(() => {
    if (
      isSourceControlTargetManual &&
      !sourceControlTargetFromKey(
        availableSourceControlTargets,
        sourceControlTargetKeyValue,
      )
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
      activeWorkspace?.id && activeChatKey
        ? parseChatRunKey(activeChatKey)
        : null;

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
  }, [
    activeChatKey,
    activeWorkspace?.id,
    contextPanelTab,
    isContextPanelOpen,
    loadTodoGraph,
  ]);

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
      (isPlanAutoRunEnabled ||
        (isContextPanelOpen && contextPanelTab === "plan")) &&
      (isPlanAutoRunBusy || activePlans.some(isAutoRunPlanInFlight));

    if (!shouldRefreshAutoRunState && !shouldRefreshRunningPlans) {
      return;
    }

    let disposed = false;
    let requestInFlight = false;
    let timeoutId: number | null = null;
    const refresh = async () => {
      if (disposed || requestInFlight || !isDocumentVisible()) {
        return;
      }
      requestInFlight = true;
      try {
        await Promise.all([
          shouldRefreshAutoRunState
            ? loadPlanAutoRunState(activeWorkspace.id)
            : undefined,
          shouldRefreshRunningPlans
            ? loadActivePlans(activeWorkspace.id)
            : undefined,
        ]);
      } finally {
        requestInFlight = false;
        if (!disposed && isDocumentVisible()) {
          timeoutId = window.setTimeout(refresh, PLAN_AUTO_RUN_REFRESH_MS);
        }
      }
    };
    const onVisibilityChange = () => {
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
      }
      if (isDocumentVisible()) {
        void refresh();
      }
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    if (isDocumentVisible()) {
      timeoutId = window.setTimeout(refresh, PLAN_AUTO_RUN_REFRESH_MS);
    }

    return () => {
      disposed = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
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
    let requestInFlight = false;
    let timeoutId: number | null = null;
    async function refreshRetryPhase() {
      if (cancelled || requestInFlight || !isDocumentVisible()) {
        return;
      }
      requestInFlight = true;
      try {
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
      } finally {
        requestInFlight = false;
      }
    }

    const schedule = () => {
      if (!cancelled && isDocumentVisible() && timeoutId === null) {
        timeoutId = window.setTimeout(async () => {
          timeoutId = null;
          await refreshRetryPhase();
          schedule();
        }, PLAN_PHASE_RETRY_REFRESH_INTERVAL_MS);
      }
    };
    const onVisibilityChange = () => {
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
      }
      if (isDocumentVisible()) {
        void refreshRetryPhase().finally(schedule);
      }
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    schedule();

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [activeWorkspace?.id, loadActivePlans, pendingPlanPhaseRetryRefresh]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      gitBranchesRequestRef.current?.abort();
      gitBranchesRequestRef.current = null;
      gitBranchesRequestIdRef.current += 1;
      setGitBranches(null);
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
            (run) =>
              run.workspaceId === tab.workspaceId && run.chatId === tab.chatId,
          ),
      );
      return next.length === current.length ? current : next;
    });

    setOpenFileTabs((current) => {
      const next = current.filter((tab) =>
        workspaces.some((workspace) => workspace.id === tab.workspaceId),
      );
      if (next.length !== current.length) {
        openFileTabsRef.current = next;
        const openEditorKeys = new Set(
          next.map((tab) => workspaceFileEditorKey(tab.workspaceId, tab.path)),
        );
        for (const key of Object.keys(workspaceFileEditorViewStatesRef.current)) {
          if (!openEditorKeys.has(key)) {
            delete workspaceFileEditorViewStatesRef.current[key];
          }
        }
        for (const key of Object.keys(workspaceMarkdownPreviewScrollTopsRef.current)) {
          if (!openEditorKeys.has(key)) {
            delete workspaceMarkdownPreviewScrollTopsRef.current[key];
          }
        }
      }
      return next.length === current.length ? current : next;
    });

    setOpenHtmlPreviewTabs((current) => {
      const next = current.filter((tab) =>
        workspaces.some((workspace) => workspace.id === tab.workspaceId),
      );
      if (next.length !== current.length) {
        openHtmlPreviewTabsRef.current = next;
      }
      return next.length === current.length ? current : next;
    });

    setOpenAgentTabs((current) => {
      const next = current.filter((tab) =>
        workspaceHasChatTab(workspaces, tab),
      );
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
    dragSessionRef: contextPanelResizeDragRef,
    isResizing: isResizingDiffPanel,
    maxHeightRatio: CONTEXT_PANEL_MAX_HEIGHT_RATIO,
    maxWidth: CONTEXT_PANEL_MAX_WIDTH,
    minHeight: CONTEXT_PANEL_MIN_HEIGHT,
    minWidth: CONTEXT_PANEL_MIN_WIDTH,
    stackedBreakpoint: CONTEXT_PANEL_STACKED_BREAKPOINT_PX,
    onHeightPreview: previewContextPanelHeight,
    onWidthPreview: previewContextPanelWidth,
    onResizeEnd: handleContextPanelResizeEnd,
  });

  useSidebarResizeEffect({
    isResizing: isResizingSidebar,
    onPointerMove: updateSidebarWidthFromClientX,
    onResizeEnd: () => setIsResizingSidebar(false),
  });


  useEffect(() => {
    return () => {
      if (workspaceChatLongPressTimeoutRef.current !== null) {
        window.clearTimeout(workspaceChatLongPressTimeoutRef.current);
      }
    };
  }, []);

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
    // Settings still loading: empty availableModels is temporary, not authoritative.
    // Clearing selection here races with active-run/message restore and leaves the
    // composer stuck on "No enabled models" after settings arrive.
    if (isLoadingSettings) {
      return;
    }

    const planModeModelId = isPlanModeEnabled
      ? settings?.plan.modeModelId?.trim() || ""
      : "";
    const planModeModel =
      planModeModelId.length > 0
        ? (availableModels.find((model) => model.id === planModeModelId) ??
          null)
        : null;

    setSelectedModelId((current) => {
      // Priority: valid manual pick; plan-mode dedicated model; default agent/first.
      if (
        hasManuallySelectedModelRef.current &&
        current &&
        availableModels.some((model) => model.id === current)
      ) {
        return current;
      }

      if (planModeModel) {
        hasManuallySelectedModelRef.current = true;
        hasManuallySelectedThinkingLevelRef.current = false;
        return planModeModel.id;
      }

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
  }, [
    availableModels,
    defaultComposerSelection.modelId,
    isLoadingSettings,
    isPlanModeEnabled,
    settings?.plan.modeModelId,
  ]);

  useEffect(() => {
    if (isLoadingSettings) {
      return;
    }

    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );
    setSelectedThinkingLevel((current) => {
      if (!selectedModel) {
        // Authoritative empty catalog only: do not clear thinking while the
        // selected model id is still catching up after settings load.
        if (!defaultComposerSelection.modelId) {
          hasManuallySelectedThinkingLevelRef.current = false;
          return "";
        }
        return current;
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
    isLoadingSettings,
    selectedModelId,
  ]);

  useEffect(() => {
    // Only prune selection against an authoritative ready catalog for the
    // current workspace. Loading/error/empty interim states must not clear
    // selectedSkillIds.
    if (skillCatalogStatus !== "ready") {
      return;
    }

    const enabledSkillIds = new Set(availableSkills.map((skill) => skill.key));

    setSelectedSkillIds((current) => {
      const next = current.filter((skillId) => enabledSkillIds.has(skillId));
      return next.length === current.length ? current : next;
    });
  }, [availableSkills, skillCatalogStatus]);

  async function handleWorkspaceSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    try {
      const isRemoteWorkspace = workspaceMode === "ssh";
      const data = await requestJson<WorkspacesResponse>(
        "/api/workspaces/add",
        {
          body: JSON.stringify({
            name: workspaceName,
            path: isRemoteWorkspace ? workspacePath : workspacePath,
            remotePath: isRemoteWorkspace ? workspacePath : null,
            serverId: isRemoteWorkspace ? workspaceServerId : null,
            terminalShell: workspaceTerminalShell || null,
            codeGraphEnabled: workspaceCodeGraphEnabled,
            contentBase64: workspaceIconDraft?.contentBase64 ?? null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      const createdWorkspace =
        data.workspaces.find(
          (workspace) => workspace.id === data.activeWorkspaceId,
        ) ?? data.workspaces[0];

      setWorkspaces(data.workspaces);
      setWorkspaceChatPaging(
        workspaceChatPagingFromWorkspaces(data.workspaces),
      );
      void loadSettings();
      const nextWorkspaceId = createdWorkspace?.id ?? data.activeWorkspaceId;
      // Clear prior chat/composer state atomically so the new workspace is never
      // paired with the previous workspace's activeChatId (messages/context-usage).
      // Keep the current view (e.g. Settings → Workspaces) so add-from-settings
      // does not navigate away. Only rewrite the browser URL when already in chat
      // so Settings stays on /settings/... after add.
      startNewWorkspaceChat(nextWorkspaceId, {
        activateChatView: false,
        updateUrl: viewMode === "chat",
      });
      if (workspaceSpecEnabled && createdWorkspace?.id) {
        try {
          await saveWorkspaceSpecSettingsRequest(
            createdWorkspace.id,
            true,
            false,
          );
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
      setWorkspaceCodeGraphEnabled(false);
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
          message:
            response.result.message ??
            (response.result.ok ? t("Ready") : t("Failed")),
          stage: "ready",
          status: response.result.ok ? "success" : "failed",
        },
      ]);
      const nextSettings = await requestJson<SettingsResponse>("/api/settings");
      settingsSkillsSnapshotRef.current = JSON.stringify(nextSettings.skills);
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
      const response = await requestJson<RemoteServerResponse>(
        "/api/remote-servers/create",
        {
          body: JSON.stringify({
            hostAlias: inlineRemoteServerHost.trim(),
            name: inlineRemoteServerName.trim(),
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      try {
        const connectResponse =
          await requestJson<RemoteServerDiagnosticResponse>(
            `/api/remote-servers/${encodeURIComponent(response.server.id)}/connect`,
            { method: "POST" },
          );
        const nextSettings =
          await requestJson<SettingsResponse>("/api/settings");
        settingsSkillsSnapshotRef.current = JSON.stringify(nextSettings.skills);
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
        const nextSettings =
          await requestJson<SettingsResponse>("/api/settings");
        settingsSkillsSnapshotRef.current = JSON.stringify(nextSettings.skills);
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
    setWorkspaceName((current) =>
      current.trim() ? current : remoteWorkspacePathBasename(path),
    );
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

  async function handleWorkspaceIconPickerSelection(
    selection: FilePickerSelection[],
  ) {
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
      target:
        workspaceMode === "ssh" && workspaceServerId
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
      target:
        workspaceMode === "ssh" && workspaceServerId
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
      allowOutsideWorkspace: true,
      mode: "file",
      multiple: true,
      readFiles: true,
      target,
      title: t("Add attachment"),
      onSelect: (selection) => {
        void handleAddSelectedFileAttachments(
          selection
            .map((item) => item.file)
            .filter((file): file is NonNullable<FilePickerSelection["file"]> =>
              Boolean(file),
            ),
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
      allowOutsideWorkspace: true,
      mode: "file",
      multiple: true,
      readFiles: true,
      target,
      title: t("Add attachment"),
      onSelect: (selection) => {
        const attachments = selection
          .map((item) => item.file)
          .filter((file): file is NonNullable<FilePickerSelection["file"]> =>
            Boolean(file),
          )
          .map(composerAttachmentFromSelectedFile);
        if (attachments.length) {
          onSelected(attachments);
        }
      },
    });
  }

  async function handleAddSelectedFileAttachments(
    files: NonNullable<FilePickerSelection["file"]>[],
  ) {
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
    const currentForKey = currentByKey[chatKey] ?? [];
    const nextForKey = resolveNext(currentForKey);
    if (nextForKey !== currentForKey) {
      advanceLiveMessageRevision(chatKey, currentForKey, nextForKey);
    }
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
        (
          updateExistingToolCall
            ? messageOwnsToolCall(message)
            : message.role === "assistant" &&
              message.id === delta.assistantMessageId
        )
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

  function createTextDeltaBuffer(
    canWrite: (chatKey: string) => boolean = () => true,
    canFlush: (chatKey: string, assistantMessageId: string) => boolean =
      () => true,
  ) {
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
        if (!canWrite(chatKey)) {
          continue;
        }
        const deferredDeltas = new Map<string, string>();
        setMessagesForChatKey(chatKey, (current) => {
          let next = current;
          for (const [assistantMessageId, delta] of messageDeltas) {
            if (!canFlush(chatKey, assistantMessageId)) {
              deferredDeltas.set(assistantMessageId, delta);
              continue;
            }
            next = appendBufferedTextDelta(next, assistantMessageId, delta);
          }
          return next;
        });
        if (deferredDeltas.size) {
          bufferedDeltasByChatKey.set(chatKey, deferredDeltas);
        }
      }
    };

    return {
      flush,
      push(chatKey: string, assistantMessageId: string, delta: string) {
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
      remapAssistantMessageId(
        chatKey: string,
        previousAssistantMessageId: string,
        canonicalAssistantMessageId: string,
      ) {
        if (
          previousAssistantMessageId === canonicalAssistantMessageId ||
          !bufferedDeltasByChatKey.has(chatKey)
        ) {
          return;
        }
        const messageDeltas = bufferedDeltasByChatKey.get(chatKey)!;
        const previousDelta = messageDeltas.get(previousAssistantMessageId);
        if (previousDelta === undefined) {
          return;
        }
        messageDeltas.delete(previousAssistantMessageId);
        messageDeltas.set(
          canonicalAssistantMessageId,
          `${messageDeltas.get(canonicalAssistantMessageId) ?? ""}${previousDelta}`,
        );
      },
    };
  }

  function createReasoningDeltaBuffer(
    canWrite: (chatKey: string) => boolean = () => true,
    canFlush: (chatKey: string, assistantMessageId: string) => boolean =
      () => true,
  ) {
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
        if (!canWrite(chatKey)) {
          continue;
        }
        const deferredDeltas = new Map<
          string,
          { delta: string; startedAtMs: number }
        >();
        setMessagesForChatKey(chatKey, (current) => {
          let next = current;
          for (const [assistantMessageId, bufferedDelta] of messageDeltas) {
            if (!canFlush(chatKey, assistantMessageId)) {
              deferredDeltas.set(assistantMessageId, bufferedDelta);
              continue;
            }
            next = appendBufferedReasoningDelta(
              next,
              assistantMessageId,
              bufferedDelta.delta,
              bufferedDelta.startedAtMs,
            );
          }
          return next;
        });
        if (deferredDeltas.size) {
          bufferedDeltasByChatKey.set(chatKey, deferredDeltas);
        }
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
      remapAssistantMessageId(
        chatKey: string,
        previousAssistantMessageId: string,
        canonicalAssistantMessageId: string,
      ) {
        if (
          previousAssistantMessageId === canonicalAssistantMessageId ||
          !bufferedDeltasByChatKey.has(chatKey)
        ) {
          return;
        }
        const messageDeltas = bufferedDeltasByChatKey.get(chatKey)!;
        const previousDelta = messageDeltas.get(previousAssistantMessageId);
        if (previousDelta === undefined) {
          return;
        }
        messageDeltas.delete(previousAssistantMessageId);
        const canonicalDelta = messageDeltas.get(canonicalAssistantMessageId);
        messageDeltas.set(canonicalAssistantMessageId, {
          delta: `${canonicalDelta?.delta ?? ""}${previousDelta.delta}`,
          startedAtMs:
            canonicalDelta?.startedAtMs ?? previousDelta.startedAtMs,
        });
      },
    };
  }

  function createToolOutputDeltaBuffer(
    canWrite: (chatKey: string) => boolean = () => true,
    canFlush: (chatKey: string, assistantMessageId: string) => boolean =
      () => true,
  ) {
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
        if (!canWrite(chatKey)) {
          continue;
        }
        const deferredDeltas = new Map<string, BufferedToolOutputDelta>();
        setMessagesForChatKey(chatKey, (current) =>
          appendBufferedToolOutputDeltas(
            current,
            Array.from(toolDeltas.values()).filter((delta) => {
              if (canFlush(chatKey, delta.assistantMessageId)) {
                return true;
              }
              deferredDeltas.set(
                `${delta.toolCallId}\u0000${delta.stream}`,
                delta,
              );
              return false;
            }),
          ),
        );
        if (deferredDeltas.size) {
          bufferedDeltasByChatKey.set(chatKey, deferredDeltas);
        }
      }
    };

    return {
      flush,
      push(chatKey: string, delta: BufferedToolOutputDelta) {
        const toolDeltas =
          bufferedDeltasByChatKey.get(chatKey) ??
          new Map<string, BufferedToolOutputDelta>();
        const key = `${delta.toolCallId}\u0000${delta.stream}`;
        const current = toolDeltas.get(key);
        toolDeltas.set(key, {
          ...delta,
          assistantMessageId:
            current?.assistantMessageId ?? delta.assistantMessageId,
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
      remapAssistantMessageId(
        chatKey: string,
        previousAssistantMessageId: string,
        canonicalAssistantMessageId: string,
      ) {
        if (
          previousAssistantMessageId === canonicalAssistantMessageId ||
          !bufferedDeltasByChatKey.has(chatKey)
        ) {
          return;
        }
        const toolDeltas = bufferedDeltasByChatKey.get(chatKey)!;
        for (const [key, delta] of toolDeltas) {
          if (delta.assistantMessageId === previousAssistantMessageId) {
            toolDeltas.set(key, {
              ...delta,
              assistantMessageId: canonicalAssistantMessageId,
            });
          }
        }
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

  function moveChatPaginationForChatKey(
    fromChatKey: string,
    toChatKey: string,
  ) {
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
    setContextUsageErrorByChatKey((current) => {
      if (!(fromChatKey in current)) {
        return current;
      }

      const { [fromChatKey]: movedError, ...next } = current;
      return { ...next, [toChatKey]: movedError };
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
    setContextUsageErrorByChatKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }
      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
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
    setContextUsageErrorByChatKey((current) => {
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
      assistantMessageId: runInfo.assistantMessageId ?? null,
      assistantSequence: runInfo.assistantSequence ?? null,
      chatId: runInfo.chatId,
      lastSequence: runInfo.lastSequence ?? null,
      queuedUserMessageId: runInfo.queuedUserMessageId ?? null,
      runId: runInfo.runId,
      workspaceId: runInfo.workspaceId,
    };
  }

  function reportChatStreamOwnershipConflict(
    kind: "duplicateWritableSession" | "assistantIdentityMismatch",
    details: {
      workspaceId: string;
      chatId: string;
      runId: string;
      canonicalAssistantMessageId: string | null;
      incomingAssistantMessageId: string | null;
      epoch: number | null;
    },
  ) {
    // Deliberately keep this identity-only: a stream ownership diagnostic must
    // never capture assistant text, tool output, or credentials in the console.
    const diagnostic = { kind, ...details };
    if (import.meta.env.DEV) {
      console.warn("[chat-stream] ownership invariant", diagnostic);
      return;
    }
    console.debug("[chat-stream] ownership conflict; reconciling", diagnostic);
  }

  function claimChatStreamSession(
    chatKey: string,
    runId: string | null,
    abortController: AbortController,
    options: { reuseSameRun: boolean },
  ): ChatStreamSession | null {
    const existing = chatStreamSessionsByChatKeyRef.current.get(chatKey);
    if (
      options.reuseSameRun &&
      existing?.runId === runId &&
      !existing.abortController.signal.aborted
    ) {
      return null;
    }

    // Invalidate before aborting: an already-buffered reader callback is allowed
    // to run after abort(), but it can no longer own state.
    existing?.abortController.abort();
    const session: ChatStreamSession = {
      abortController,
      assistantMessageId: null,
      epoch: ++chatStreamEpochRef.current,
      lastSequence: null,
      runId,
    };
    chatStreamSessionsByChatKeyRef.current.set(chatKey, session);
    return session;
  }

  function isCurrentChatStreamSession(
    chatKey: string,
    session: ChatStreamSession,
  ) {
    return chatStreamSessionsByChatKeyRef.current.get(chatKey) === session;
  }

  function releaseChatStreamSession(
    chatKey: string,
    session: ChatStreamSession,
  ) {
    if (isCurrentChatStreamSession(chatKey, session)) {
      chatStreamSessionsByChatKeyRef.current.delete(chatKey);
    }
  }

  function invalidateChatStreamSession(chatKey: string) {
    const session = chatStreamSessionsByChatKeyRef.current.get(chatKey);
    chatStreamSessionsByChatKeyRef.current.delete(chatKey);
    session?.abortController.abort();
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
        console.debug(
          "[chat-stream] refreshing running chat after recovery trigger",
          {
            chatId: runInfo.chatId,
            reason,
            workspaceId: runInfo.workspaceId,
          },
        );
        void loadChatMessages(runInfo.workspaceId, runInfo.chatId);
      }
    }
  }

  function clearWorkspaceChatActiveRun(
    workspaceId: string,
    chatId: string,
    expectedRunId: string | null = null,
  ) {
    setWorkspaces((current) => {
      let changed = false;
      const nextWorkspaces = current.map((workspace) => {
        if (workspace.id !== workspaceId) {
          return workspace;
        }

        let workspaceChanged = false;
        const nextChats = workspace.chats.map((chat) => {
          if (
            chat.id !== chatId ||
            chat.activeRun === null ||
            (expectedRunId !== null && chat.activeRun.runId !== expectedRunId)
          ) {
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

  function clearChatStreamHandoff(chatKey: string) {
    const timer = chatStreamHandoffTimersByChatKeyRef.current.get(chatKey);
    if (timer !== undefined) {
      window.clearTimeout(timer);
      chatStreamHandoffTimersByChatKeyRef.current.delete(chatKey);
    }
    chatStreamHandoffsByChatKeyRef.current.delete(chatKey);
  }

  function reconcileHandoffsFromWorkspaceSummaries(
    workspaceSummaries: WorkspaceSummary[],
  ) {
    for (const [chatKey, handoff] of chatStreamHandoffsByChatKeyRef.current) {
      if (
        chatStreamSessionsByChatKeyRef.current.has(chatKey) ||
        activeRunAbortByChatKeyRef.current.has(chatKey)
      ) {
        continue;
      }
      const workspaceChat = workspaceSummaries
        .find((workspace) => workspace.id === handoff.workspaceId)
        ?.chats.find((chat) => chat.id === handoff.chatId);
      const isDurablyWaiting =
        workspaceChat?.queuedRun?.status === "running" ||
        workspaceChat?.queuedRun?.status === "queued";
      const activeRun = normalizeActiveChatRunSummary(workspaceChat?.activeRun);
      if (isDurablyWaiting && activeRun) {
        clearChatStreamHandoff(chatKey);
        void subscribeActiveChatRun(
          {
            ...activeRun,
            lastSequence: handoff.lastSequence ?? activeRun.lastSequence ?? null,
          },
          true,
        );
        continue;
      }
      if (!isDurablyWaiting) {
        clearChatStreamHandoff(chatKey);
        finishChatRun(
          chatKey,
          handoff.runId,
          handoff.workspaceId,
          handoff.chatId,
        );
      }
    }
  }

  function reconcileChatStreamHandoff(handoff: ChatStreamHandoff) {
    const chatKey = chatRunKey(handoff.workspaceId, handoff.chatId);
    clearChatStreamHandoff(chatKey);
    chatStreamHandoffsByChatKeyRef.current.set(chatKey, handoff);

    const reconcile = async (attempt: number) => {
      if (chatStreamHandoffsByChatKeyRef.current.get(chatKey) !== handoff) {
        return;
      }
      if (
        chatStreamSessionsByChatKeyRef.current.has(chatKey) ||
        activeRunAbortByChatKeyRef.current.has(chatKey)
      ) {
        clearChatStreamHandoff(chatKey);
        return;
      }

      // Message history is authoritative for events and replay sequence, but a
      // delayed activeRun alone cannot revive a terminal session. Coordinator
      // handoff additionally requires its durable queuedRun to remain alive.
      const messageActiveRun = await loadChatMessages(
        handoff.workspaceId,
        handoff.chatId,
        undefined,
        { deferActiveRunSubscription: true },
      );
      if (chatStreamHandoffsByChatKeyRef.current.get(chatKey) !== handoff) {
        return;
      }
      if (
        chatStreamSessionsByChatKeyRef.current.has(chatKey) ||
        activeRunAbortByChatKeyRef.current.has(chatKey)
      ) {
        clearChatStreamHandoff(chatKey);
        return;
      }

      const workspaceChat = workspacesRef.current
        .find((workspace) => workspace.id === handoff.workspaceId)
        ?.chats.find((chat) => chat.id === handoff.chatId);
      const queuedStatus = workspaceChat?.queuedRun?.status;
      const isDurablyWaiting =
        queuedStatus === "running" || queuedStatus === "queued";
      const workspaceActiveRun = normalizeActiveChatRunSummary(
        workspaceChat?.activeRun,
      );
      const durableTerminalRunId =
        durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId ?? null;
      const activeRun = !isTerminalActiveRun(
        workspaceActiveRun,
        durableTerminalRunId,
      )
        ? (messageActiveRun ?? workspaceActiveRun)
        : null;
      if (isDurablyWaiting && activeRun) {
        const reattachRun = {
          ...activeRun,
          lastSequence: handoff.lastSequence ?? activeRun.lastSequence ?? null,
        };
        console.debug("[chat-stream] stream handoff reattaching active run", {
          chatId: handoff.chatId,
          lastSequence: reattachRun.lastSequence,
          result: "active-run",
          runId: reattachRun.runId,
          workspaceId: handoff.workspaceId,
        });
        clearChatStreamHandoff(chatKey);
        void subscribeActiveChatRun(reattachRun, true);
        return;
      }

      if (isDurablyWaiting) {
        if (attempt >= 20) {
          console.debug("[chat-stream] stream handoff check budget exhausted", {
            chatId: handoff.chatId,
            lastSequence: handoff.lastSequence,
            result: "waiting-check-exhausted",
            runId: handoff.runId,
            workspaceId: handoff.workspaceId,
          });
          return;
        }
        let timer: number;
        timer = window.setTimeout(() => {
          if (chatStreamHandoffTimersByChatKeyRef.current.get(chatKey) !== timer) {
            return;
          }
          chatStreamHandoffTimersByChatKeyRef.current.delete(chatKey);
          void reconcile(attempt + 1);
        }, 250);
        chatStreamHandoffTimersByChatKeyRef.current.set(chatKey, timer);
        return;
      }

      console.debug("[chat-stream] stream handoff reached durable terminal state", {
        chatId: handoff.chatId,
        lastSequence: handoff.lastSequence,
        result: "terminal",
        runId: handoff.runId,
        workspaceId: handoff.workspaceId,
      });
      clearChatStreamHandoff(chatKey);
      finishChatRun(
        chatKey,
        handoff.runId,
        handoff.workspaceId,
        handoff.chatId,
      );
    };

    let initialTimer: number;
    initialTimer = window.setTimeout(() => {
      if (
        chatStreamHandoffTimersByChatKeyRef.current.get(chatKey) !==
        initialTimer
      ) {
        return;
      }
      chatStreamHandoffTimersByChatKeyRef.current.delete(chatKey);
      void reconcile(0);
    }, 50);
    chatStreamHandoffTimersByChatKeyRef.current.set(chatKey, initialTimer);
  }

  function startChatRun(chatKey: string, runId: string | null): boolean {
    if (!runId) {
      return true;
    }

    const terminalRunId =
      durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId;
    if (terminalRunId === runId) {
      return false;
    }

    if (terminalRunId !== undefined) {
      durableRunTerminationByChatKeyRef.current.delete(chatKey);
    }
    clearChatStreamHandoff(chatKey);
    return true;
  }

  function finishChatRun(
    chatKey: string,
    runId: string | null,
    workspaceId: string,
    chatId: string | null,
    options: { durable?: boolean } = {},
  ): boolean {
    const currentRun = activeRunInfoByChatKeyRef.current[chatKey] ?? null;
    if (runId && currentRun?.runId && currentRun.runId !== runId) {
      return false;
    }

    if (runId && options.durable !== false) {
      durableRunTerminationByChatKeyRef.current.set(chatKey, { runId });
      clearChatStreamHandoff(chatKey);
    }
    setChatRunning(chatKey, false);
    setActiveRunInfoForChatKey(chatKey, null);
    clearLiveChatStatistics(chatKey);
    if (chatId) {
      clearWorkspaceChatActiveRun(workspaceId, chatId, runId);
    }
    return true;
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
      .filter((request) => request.modelId.trim() && request.providerId.trim());

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
            queuedRequest.pendingUserMessageId ??
            queuedRequest.queuedUserMessageId,
          request: queuedRequest,
          status: "queued",
        },
      ];
    });
  }

  function restoreRetryRunRequestFromFailedMessages(
    workspaceId: string,
    chatId: string,
    chatMessages: ShellMessage[],
  ) {
    const chatKey = chatRunKey(workspaceId, chatId);
    if (runningChatKeysRef.current.has(chatKey) || isSendingMessage) {
      return;
    }
    // Prefer the latest failed assistant that still has a preceding user with runConfig.
    for (let index = chatMessages.length - 1; index >= 0; index -= 1) {
      const message = chatMessages[index];
      if (message?.role !== "assistant" || message.status !== "error") {
        continue;
      }
      const hasErrorPart = message.parts.some((part) => part.type === "error");
      if (!hasErrorPart && !message.content) {
        continue;
      }
      let userMessage: ShellMessage | undefined;
      for (let userIndex = index - 1; userIndex >= 0; userIndex -= 1) {
        const candidate = chatMessages[userIndex];
        if (candidate?.role === "user") {
          userMessage = candidate;
          break;
        }
      }
      const runConfig = userMessage?.runConfig;
      if (!userMessage || !runConfig?.modelId) {
        continue;
      }
      setRetryRunRequest({
        workspaceId,
        chatId,
        content: userMessage.content,
        attachments: [],
        modelId: runConfig.modelId,
        providerId: runConfig.providerId ?? selectedProviderIdRef.current ?? "",
        thinkingLevel: runConfig.thinkingLevel ?? "",
        latencyMode: latencyModeFromValue(runConfig.latencyMode),
        skillIds: normalizeStringArray(runConfig.selectedSkillIds),
        sessionMode:
          runConfig.sessionMode ?? userMessage.sessionMode ?? undefined,
        teamModeEnabled: runConfig.teamModeEnabled ?? false,
        localChatKey: chatKey,
      });
      setChatRunFailed(chatKey, true);
      return;
    }
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
      .map((run): WorkspaceChatListItem => ({
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
      }));
    const persistedWorkspaceChats: WorkspaceChatListItem[] =
      workspace.chats.map((chat) => ({
        ...chat,
        scheduledStatus:
          chat.queuedRun?.status === "queued" ? "queued" : undefined,
      }));

    return [...scheduledChats, ...persistedWorkspaceChats].sort(
      compareWorkspaceChatListItemsByCreatedAtDesc,
    );
  }

  function scheduledWorkspaceRunsFor(workspaceId: string) {
    return scheduledWorkspaceRuns.filter(
      (run) => run.workspaceId === workspaceId,
    );
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

  function resolveCommittedPlanModeForChatKey(chatKey: string | null): boolean {
    if (!chatKey) {
      return false;
    }
    const cachedMessages = chatMessagesByKeyRef.current[chatKey];
    if (cachedMessages !== undefined) {
      return planModeEnabledFromMessages(cachedMessages);
    }
    // No message cache yet: use last committed send/load snapshot, else normal mode.
    return planModeByChatKeyRef.current[chatKey] === true;
  }

  function restorePlanModeForChatKey(chatKey: string | null) {
    const enabled = resolveCommittedPlanModeForChatKey(chatKey);
    setIsPlanModeEnabled(enabled);
    applyComposerModelForPlanMode(enabled);
  }

  function applyComposerModelForPlanMode(enabled: boolean) {
    // Settings still loading: empty availableModels is temporary. Defer to the
    // authoritative model-catalog reconciliation effect once settings arrive.
    if (isLoadingSettingsRef.current) {
      return;
    }

    // Always read the latest catalog/settings. Async callers (loadChatMessages)
    // may have started while settings were still loading and must not apply a
    // stale empty catalog after settings have already reconciled the selection.
    const availableModelsNow = availableModelsRef.current;
    const defaultComposerSelectionNow = defaultComposerSelectionRef.current;
    const settingsNow = settingsRef.current;

    if (enabled) {
      const modeModelId = settingsNow?.plan.modeModelId?.trim() || "";
      if (!modeModelId) {
        return;
      }
      const model = availableModelsNow.find(
        (candidate) => candidate.id === modeModelId,
      );
      if (!model) {
        return;
      }
      if (
        !(model.activeProviderId &&
        model.providerIds.includes(model.activeProviderId)
          ? model.activeProviderId
          : model.providerIds[0])
      ) {
        return;
      }
      hasManuallySelectedModelRef.current = true;
      hasManuallySelectedThinkingLevelRef.current = false;
      setSelectedModelId(model.id);
      setSelectedThinkingLevel(defaultThinkingLevelForModel(model));
      return;
    }

    // Authoritative empty catalog: keep the existing empty-label behavior.
    // Temporary empty catalog while settings load is handled above.
    if (
      !defaultComposerSelectionNow.modelId &&
      availableModelsNow.length === 0
    ) {
      hasManuallySelectedModelRef.current = false;
      hasManuallySelectedThinkingLevelRef.current = false;
      setSelectedModelId("");
      setSelectedThinkingLevel("");
      return;
    }

    hasManuallySelectedModelRef.current = false;
    hasManuallySelectedThinkingLevelRef.current = false;
    setSelectedModelId(defaultComposerSelectionNow.modelId);
    setSelectedThinkingLevel(defaultComposerSelectionNow.thinkingLevel);
  }

  function rememberPlanModeForChatKey(chatKey: string, value: boolean) {
    planModeByChatKeyRef.current[chatKey] = value;
  }

  function bindRequestPlanModeToChatKey(
    request: RetryRunRequest,
    chatKey: string,
  ) {
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
        const jobsResponse = await fetchWorkspaceSpecJobsList(workspaceId, 24);
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
    setActiveMainTab({
      chatId: run.chatId,
      type: "chat",
      workspaceId: run.workspaceId,
    });
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
    expectedStreamSession?: ChatStreamSession,
    options: { deferActiveRunSubscription?: boolean } = {},
  ) {
    setError(null);
    const chatKey = chatRunKey(workspaceId, chatId);
    const existingController = loadingChatControllersRef.current.get(chatKey);
    if (existingController && !existingController.signal.aborted) {
      return;
    }
    // Capture before await so a superseded response cannot flip this after a newer load
    // already wrote cache (same-chat abort → reload race).
    const hadCachedMessagesBeforeLoad =
      chatMessagesByKeyRef.current[chatKey] !== undefined;
    const liveRevisionBeforeLoad =
      liveMessageRevisionByChatKeyRef.current.get(chatKey) ?? 0;
    const liveAssistantRevisionsBeforeLoad = new Map(
      liveAssistantMessageRevisionByChatKeyRef.current.get(chatKey),
    );
    // Preserve the run identity from the instant this request starts. The
    // complete handler clears activeRunInfo before it writes the terminal
    // assistant, so response-time state alone cannot distinguish that durable
    // terminal from an unrelated ordinary history refresh.
    const liveRunIdBeforeLoad =
      expectedStreamSession?.runId ??
      chatStreamSessionsByChatKeyRef.current.get(chatKey)?.runId ??
      activeRunInfoByChatKeyRef.current[chatKey]?.runId ??
      null;
    loadingChatKeysRef.current.add(chatKey);
    const controller = new AbortController();
    loadingChatControllersRef.current.set(chatKey, controller);
    setLoadingChatKeys((current) => new Set(current).add(chatKey));

    try {
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages?limit=${CHAT_MESSAGES_INITIAL_PAGE_LIMIT}`,
        { signal: controller.signal },
      );
      // Drop superseded loads for this chatKey before any cache/state mutation.
      if (
        loadingChatControllersRef.current.get(chatKey) !== controller ||
        (expectedStreamSession &&
          !isCurrentChatStreamSession(chatKey, expectedStreamSession))
      ) {
        return;
      }
      const normalizedMessages = expandMessagesWithUserInterruptions(
        data.messages.map(normalizeChatMessageSummary),
      );
      const reportedActiveRun = normalizeActiveChatRunSummary(data.activeRun);
      const activeRun = isTerminalActiveRun(
        reportedActiveRun,
        durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId ?? null,
      )
        ? null
        : reportedActiveRun;
      if (reportedActiveRun && activeRun === null) {
        clearWorkspaceChatActiveRun(
          workspaceId,
          chatId,
          reportedActiveRun.runId,
        );
      }
      const cachedMessages = chatMessagesByKeyRef.current[chatKey] ?? [];
      const localRunInfo = activeRunInfoByChatKeyRef.current[chatKey] ?? null;
      const hasLocalActiveRun =
        Boolean(localRunInfo) && runningChatKeysRef.current.has(chatKey);
      const localStreamController =
        activeRunAbortByChatKeyRef.current.get(chatKey);
      const hasOpenLocalStream = Boolean(
        localStreamController && !localStreamController.signal.aborted,
      );
      // Same continuous local run only: matching server runId, or temporary
      // null activeRun while this client still holds a live SSE for that run.
      // A different server runId, or null without an open local stream, must
      // not keep zero-overlap cache (edit rewrites / canceled replacements).
      const sameContinuousLocalRun = isSameContinuousLocalActiveRun({
        hasLocalActiveRun,
        hasOpenLocalStream,
        localRunId: localRunInfo?.runId ?? null,
        serverActiveRunId: activeRun?.runId ?? null,
      });
      // Zero-overlap history baseline only for the same continuous local run
      // (subagent return / new attempt tail). Not for arbitrary activeRun.
      const preserveDisjointActiveRunCache = sameContinuousLocalRun;
      // Live streaming bubbles: same continuous local run, or re-attach when
      // this client has no local run but the server reports one (id-overlap
      // path only re-inserts placeholders; zero-overlap still drops orphans).
      const preserveStreamingPlaceholders =
        sameContinuousLocalRun || (!hasLocalActiveRun && Boolean(activeRun));
      const staleAfterLiveMessageRevision =
        (liveMessageRevisionByChatKeyRef.current.get(chatKey) ?? 0) >
        liveRevisionBeforeLoad;
      const liveAssistantMessageIds = new Set(
        Array.from(
          liveAssistantMessageRevisionByChatKeyRef.current.get(chatKey) ?? [],
        )
          .filter(
            ([assistantMessageId, revision]) =>
              revision >
              (liveAssistantRevisionsBeforeLoad.get(assistantMessageId) ?? 0),
          )
          .map(([assistantMessageId]) => assistantMessageId),
      );
      const durableTerminalRunId =
        durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId ?? null;
      const currentStreamRunId =
        chatStreamSessionsByChatKeyRef.current.get(chatKey)?.runId ?? null;
      const staleSnapshotBelongsToLiveRun =
        liveRunIdBeforeLoad !== null &&
        (sameContinuousLocalRun ||
          durableTerminalRunId === liveRunIdBeforeLoad ||
          (currentStreamRunId === liveRunIdBeforeLoad &&
            (activeRun?.runId === null ||
              activeRun?.runId === liveRunIdBeforeLoad)));
      const mergeResult = mergeLoadedMessagesWithStreamingPlaceholders(
        normalizedMessages,
        cachedMessages,
        {
          preserveDisjointActiveRunCache,
          // An old response may not remove live compression lifecycle parts,
          // but only while the active run proves this is still the same thread.
          preserveLiveContextCompressionParts:
            staleAfterLiveMessageRevision && sameContinuousLocalRun,
          // A resumed Agent attempt receives a new run id but continues writing
          // the same visible assistant turn. The overlay itself only merges
          // matching assistant ids. Terminal lifecycle entries are immutable
          // once emitted, so an authoritative active run is enough to retain
          // them when a later attempt reattaches before its history snapshot
          // has caught up; unlike transient compression progress, this does
          // not require the load to overlap a local live revision.
          preserveLiveAgentTaskLifecycleParts:
            (sameContinuousLocalRun || Boolean(activeRun)),
          // A stale same-run response must never regress live text, reasoning,
          // tools, or the durable terminal shape. Assistant revisions make this
          // narrow: unrelated assistant messages still use server history.
          preserveLiveAssistantMessageIds:
            staleAfterLiveMessageRevision && staleSnapshotBelongsToLiveRun
              ? liveAssistantMessageIds
              : undefined,
          preserveStreamingPlaceholders,
        },
      );
      const nextMessages = preserveCachedReasoningDurations(
        mergeResult.messages,
        cachedMessages,
      );
      const restoredQuestion = parseQuestionRequestSummary(
        data.pendingQuestion,
      );
      const serverPagination = normalizeChatMessagesPagination(data.pagination);
      const existingPagination = chatMessagePaginationByKeyRef.current[chatKey];
      const cacheWasTrimmed = trimmedChatCacheKeysRef.current.has(chatKey);
      const pagination =
        mergeResult.preservedCachePrefix &&
        existingPagination &&
        !cacheWasTrimmed
          ? existingPagination
          : serverPagination;
      if (
        loadingChatControllersRef.current.get(chatKey) !== controller ||
        (expectedStreamSession &&
          !isCurrentChatStreamSession(chatKey, expectedStreamSession))
      ) {
        return;
      }
      updateOpenChatTabTitle(workspaceId, chatId, data.chat?.title ?? null);
      setReadOnlyChatKeys((current) => {
        const readOnly = data.chat?.readOnly === true;
        if (
          (current[chatKey] === true) === readOnly &&
          (readOnly || !(chatKey in current))
        ) {
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
      setChatMessagesByKey((current) => ({
        ...current,
        [chatKey]: nextMessages,
      }));
      rememberChatCacheAccess(chatKey);
      trimmedChatCacheKeysRef.current.delete(chatKey);
      setChatMessagePaginationByKey((current) => ({
        ...current,
        [chatKey]: pagination,
      }));
      trimInactiveChatCaches();
      restoreQueuedRunRequestsForChatKey(workspaceId, chatId, nextMessages);
      restoreRetryRunRequestFromFailedMessages(
        workspaceId,
        chatId,
        nextMessages,
      );
      const planModeFromMessages = planModeEnabledFromMessages(nextMessages);
      rememberPlanModeForChatKey(chatKey, planModeFromMessages);
      if (activeChatKeyRef.current === chatKey) {
        setMessages(nextMessages);
        setPendingQuestion(
          (current) =>
            restoredQuestion ??
            (current?.workspaceId === workspaceId && current.chatId === chatId
              ? null
              : current),
        );
        if (restoredQuestion) {
          setQuestionError(null);
          setIsAnsweringQuestion(false);
        }
        // Only push Plan toggle from server when this load is filling a chat that had no
        // message cache yet (URL restore / first open). Soft reloads must not wipe a
        // draft Plan toggle while the user stays on the same chat.
        if (!hadCachedMessagesBeforeLoad) {
          setIsPlanModeEnabled(planModeFromMessages);
          applyComposerModelForPlanMode(planModeFromMessages);
        }
      }
      const expectedSessionOwnsActiveRun =
        expectedStreamSession !== undefined &&
        isCurrentChatStreamSession(chatKey, expectedStreamSession) &&
        activeRun?.runId === expectedStreamSession.runId;
      if (
        activeRun &&
        !expectedSessionOwnsActiveRun &&
        !options.deferActiveRunSubscription
      ) {
        void subscribeActiveChatRun(activeRun);
      } else if (!hasLocalActiveRun) {
        setChatRunning(chatKey, false);
        setActiveRunInfoForChatKey(chatKey, null);
        clearWorkspaceChatActiveRun(workspaceId, chatId);
      }
      return activeRun;
    } catch (requestError) {
      if (
        loadingChatControllersRef.current.get(chatKey) === controller &&
        activeChatKeyRef.current === chatKey &&
        (!expectedStreamSession ||
          isCurrentChatStreamSession(chatKey, expectedStreamSession)) &&
        !isAbortError(requestError)
      ) {
        setRunError(requestError);
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
      const beforeSequence = pagination.nextBeforeSequence;
      const params = new URLSearchParams({
        beforeSequence: String(beforeSequence),
        limit: String(CHAT_MESSAGES_HISTORY_PAGE_LIMIT),
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
        const existingIds = new Set(
          existingMessages.map((message) => message.id),
        );
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
        setRunError(requestError);
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

  async function ensureWorkspaceChatLoaded(
    workspaceId: string,
    chatId: string,
  ): Promise<boolean> {
    if (isPendingChatId(chatId)) {
      return true;
    }
    const workspace = workspaces.find(
      (candidate) => candidate.id === workspaceId,
    );
    if (!workspace) {
      return false;
    }
    if (workspace.chats.some((chat) => chat.id === chatId)) {
      return true;
    }

    try {
      const params = new URLSearchParams({
        includeChatId: chatId,
        limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE),
      });
      const data = await requestJson<WorkspaceChatsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats?${params.toString()}`,
      );
      const found = data.chats.some((chat) => chat.id === chatId);
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
      return found;
    } catch (requestError) {
      setError(errorMessage(requestError));
      // Network/API failure: keep open tabs (treat as still unknown).
      return true;
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
    for (const [
      loadingChatKey,
      controller,
    ] of loadingChatControllersRef.current) {
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
    options: { activateChatView?: boolean; updateUrl?: boolean } = {},
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
    // Adding a workspace from Settings must keep the settings view; only
    // explicit "new chat" navigation forces chat viewMode.
    if (options.activateChatView !== false) {
      setViewMode("chat");
    }
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      // chatId:null + workspace path keeps open tabs in the query without
      // re-selecting the previous active chat on refresh (see browser-route).
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
    const workspace = workspaces.find(
      (workspace) => workspace.id === workspaceId,
    );
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
      const workspace = workspaces.find((item) => item.id === tab.workspaceId);
      if (!workspace) {
        return [];
      }

      const chat = workspace.chats.find((item) => item.id === tab.chatId);
      if (!chat) {
        // Fully loaded list: skip chats that are definitely gone.
        if (!workspace.chatPagination?.hasMore) {
          return [];
        }

        // Off-page unknown: restore with fallback title and probe includeChatId.
        void ensureWorkspaceChatLoaded(tab.workspaceId, tab.chatId).then(
          (found) => {
            if (found) {
              return;
            }

            setOpenChatTabs((current) => {
              const next = current.filter(
                (openTab) =>
                  openTab.workspaceId !== tab.workspaceId ||
                  openTab.chatId !== tab.chatId,
              );
              if (next.length === current.length) {
                return current;
              }
              openChatTabsRef.current = next;
              return next;
            });
          },
        );

        return [
          {
            chatId: tab.chatId,
            fallbackTitle: t("Chat"),
            fallbackWorkspaceName: workspace.name,
            workspaceId: tab.workspaceId,
          } satisfies OpenChatTab,
        ];
      }

      return [
        {
          chatId: tab.chatId,
          fallbackTitle: chat.title,
          fallbackWorkspaceName: workspace.name,
          workspaceId: tab.workspaceId,
        } satisfies OpenChatTab,
      ];
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
    setActiveWorkspaceChatRefs(tab.workspaceId, tab.chatId, {
      syncPlanMode: true,
    });
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

    const workspace = workspaces.find(
      (workspace) => workspace.id === activeWorkspaceId,
    );
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
      setWorkspaceFilesError(
        t("Select a workspace before using file actions."),
      );
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
      const workspace = workspaces.find(
        (workspace) => workspace.id === file.workspaceId,
      );
      if (!workspace) {
        return [];
      }

      return [browserRouteFileTabToOpenFileTab(file, workspace)];
    });

    openFileTabsRef.current = nextTabs;
    setOpenFileTabs(nextTabs);
    const openEditorKeys = new Set(
      nextTabs.map((tab) => workspaceFileEditorKey(tab.workspaceId, tab.path)),
    );
    for (const key of Object.keys(workspaceFileEditorViewStatesRef.current)) {
      if (!openEditorKeys.has(key)) {
        delete workspaceFileEditorViewStatesRef.current[key];
      }
    }
    for (const key of Object.keys(workspaceMarkdownPreviewScrollTopsRef.current)) {
      if (!openEditorKeys.has(key)) {
        delete workspaceMarkdownPreviewScrollTopsRef.current[key];
      }
    }

    const selectedFile = activeFile
      ? (nextTabs.find(
          (tab) =>
            tab.workspaceId === activeFile.workspaceId &&
            tab.path === activeFile.path,
        ) ?? null)
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
    // Cross-workspace file tabs must not keep the previous workspace's chatId
    // (context-usage / stats / agent-team identity mismatch).
    if (activeWorkspaceIdRef.current !== file.workspaceId) {
      setActiveWorkspaceChatRefs(file.workspaceId, null);
      setActiveChatId(null);
      setMessages([]);
      setSelectedDiffPath(null);
    }
    setActiveWorkspaceId(file.workspaceId);
    setExpandedWorkspaceId(file.workspaceId);
    setActiveMainTab({
      path: file.path,
      type: "file",
      workspaceId: file.workspaceId,
    });
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (!isWorkspaceImageFilePath(file.path)) {
      initWorkspaceFileEditor(file.workspaceId, file.path);
    }
    if (options.updateUrl !== false) {
      updateBrowserRoute(browserRouteForActiveFile(file));
    }
  }

  function openWorkspaceHtmlPreviewTab(
    file: Pick<
      OpenFileTab,
      "workspaceId" | "path" | "name" | "workspaceName" | "workspaceLogoUrl"
    >,
    options: { updateUrl?: boolean } = {},
  ) {
    if (!isHtmlPreviewPath(file.path) && !isHtmlFilePath(file.path)) {
      return;
    }

    const previewTab: OpenHtmlPreviewTab = {
      name: file.name,
      path: file.path,
      workspaceId: file.workspaceId,
      workspaceLogoUrl: file.workspaceLogoUrl,
      workspaceName: file.workspaceName,
    };
    selectWorkspaceHtmlPreviewTab(previewTab, options);
  }

  function selectWorkspaceHtmlPreviewTab(
    preview: OpenHtmlPreviewTab,
    options: { updateUrl?: boolean } = {},
  ) {
    const nextTabs = upsertOpenHtmlPreviewTab(
      openHtmlPreviewTabsRef.current,
      preview,
    );
    openHtmlPreviewTabsRef.current = nextTabs;
    setOpenHtmlPreviewTabs(nextTabs);
    // Same as file tabs: never pair a new workspace with the prior chat identity.
    if (activeWorkspaceIdRef.current !== preview.workspaceId) {
      setActiveWorkspaceChatRefs(preview.workspaceId, null);
      setActiveChatId(null);
      setMessages([]);
      setSelectedDiffPath(null);
    }
    setActiveWorkspaceId(preview.workspaceId);
    setExpandedWorkspaceId(preview.workspaceId);
    setActiveMainTab({
      path: preview.path,
      type: "htmlPreview",
      workspaceId: preview.workspaceId,
    });
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
    if (options.updateUrl !== false) {
      updateBrowserRoute(browserRouteForActiveHtmlPreview(preview));
    }
  }

  function restoreWorkspaceHtmlPreviewTabs(
    previews: BrowserRouteHtmlPreviewTab[],
    activePreview: BrowserRouteHtmlPreviewTab | null,
  ) {
    const nextTabs = previews.flatMap((preview) => {
      if (!isHtmlPreviewPath(preview.path)) {
        return [];
      }

      const workspace = workspaces.find(
        (item) => item.id === preview.workspaceId,
      );
      if (!workspace) {
        return [];
      }

      return [browserRouteHtmlPreviewTabToOpenTab(preview, workspace)];
    });

    openHtmlPreviewTabsRef.current = nextTabs;
    setOpenHtmlPreviewTabs(nextTabs);

    const selectedPreview = activePreview
      ? (nextTabs.find(
          (tab) =>
            tab.workspaceId === activePreview.workspaceId &&
            tab.path === activePreview.path,
        ) ?? null)
      : null;
    if (!selectedPreview) {
      return false;
    }

    selectWorkspaceHtmlPreviewTab(selectedPreview, { updateUrl: false });
    return true;
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
        isMarkdownPreviewEnabled: false,
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
          isMarkdownPreviewEnabled:
            current[editorKey]?.isMarkdownPreviewEnabled ?? false,
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
          isMarkdownPreviewEnabled:
            current[editorKey]?.isMarkdownPreviewEnabled ?? false,
          isSaving: false,
          lastSavedContent: current[editorKey]?.lastSavedContent ?? "",
        },
      }));
    }
  }

  function browserRouteForActiveFile(file: OpenFileTab): BrowserRoute {
    return {
      activeFile: { path: file.path, workspaceId: file.workspaceId },
      chatId:
        activeWorkspaceIdRef.current === file.workspaceId
          ? activeChatIdRef.current
          : null,
      viewMode: "chat",
      workspaceId: file.workspaceId,
    };
  }

  function browserRouteForActiveHtmlPreview(
    preview: OpenHtmlPreviewTab,
  ): BrowserRoute {
    return {
      activePreview: { path: preview.path, workspaceId: preview.workspaceId },
      chatId:
        activeWorkspaceIdRef.current === preview.workspaceId
          ? activeChatIdRef.current
          : null,
      viewMode: "chat",
      workspaceId: preview.workspaceId,
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
          isMarkdownPreviewEnabled:
            current[editorKey]?.isMarkdownPreviewEnabled ?? false,
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

  const updateWorkspaceFileEditorMarkdownPreview = useCallback(
    (
      workspaceId: string,
      path: string,
      isMarkdownPreviewEnabled: boolean,
    ) => {
      const editorKey = workspaceFileEditorKey(workspaceId, path);
      setWorkspaceFileEditors((current) => {
        const editor = current[editorKey];
        if (
          !editor ||
          editor.isMarkdownPreviewEnabled === isMarkdownPreviewEnabled
        ) {
          return current;
        }

        return {
          ...current,
          [editorKey]: { ...editor, isMarkdownPreviewEnabled },
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
    const workspace = workspaces.find(
      (workspace) => workspace.id === workspaceId,
    );
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
    const workspace = workspaces.find(
      (workspace) => workspace.id === workspaceId,
    );
    const chat = workspace?.chats.find((chat) => chat.id === chatId);

    setOpenChatTabs((current) => {
      const pendingTab = current.find(
        (tab) =>
          tab.workspaceId === workspaceId && tab.chatId === pendingChatId,
      );
      const nextTab: OpenChatTab = {
        workspaceId,
        chatId,
        fallbackTitle: chat?.title ?? pendingTab?.fallbackTitle ?? t("Chat"),
        fallbackWorkspaceName:
          workspace?.name ??
          pendingTab?.fallbackWorkspaceName ??
          t("Workspace"),
      };
      const withoutOldTabs = current.filter(
        (tab) =>
          tab.workspaceId !== workspaceId ||
          (tab.chatId !== pendingChatId && tab.chatId !== chatId),
      );
      const pendingIndex = current.findIndex(
        (tab) =>
          tab.workspaceId === workspaceId && tab.chatId === pendingChatId,
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

    if (tab.type === "htmlPreview") {
      selectWorkspaceHtmlPreviewTab(tab);
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

    if (tab.type === "htmlPreview") {
      closeHtmlPreviewTab(tab);
      return;
    }

    const tabIndex = mainTabs.findIndex(
      (current) =>
        current.type === "file" &&
        current.workspaceId === tab.workspaceId &&
        current.path === tab.path,
    );
    const nextOpenFileTabs = openFileTabsRef.current.filter(
      (current) =>
        current.workspaceId !== tab.workspaceId || current.path !== tab.path,
    );
    openFileTabsRef.current = nextOpenFileTabs;
    setOpenFileTabs(nextOpenFileTabs);
    delete workspaceFileEditorViewStatesRef.current[
      workspaceFileEditorKey(tab.workspaceId, tab.path)
    ];
    delete workspaceMarkdownPreviewScrollTopsRef.current[
      workspaceFileEditorKey(tab.workspaceId, tab.path)
    ];
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
      updateBrowserRouteAfterTabClose(tab.workspaceId);
      return;
    }

    const nextTabs = mainTabs.filter(
      (current) =>
        !(
          current.type === "file" &&
          current.workspaceId === tab.workspaceId &&
          current.path === tab.path
        ),
    );
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveMainTab({
      chatId: null,
      type: "chat",
      workspaceId: activeWorkspaceId || tab.workspaceId,
    });
    updateBrowserRoute(
      {
        chatId: activeChatId,
        viewMode: "chat",
        workspaceId: activeWorkspaceId || tab.workspaceId,
      },
      "replace",
    );
  }

  function closeHtmlPreviewTab(tab: OpenHtmlPreviewTab) {
    const tabIndex = mainTabs.findIndex(
      (current) =>
        current.type === "htmlPreview" &&
        current.workspaceId === tab.workspaceId &&
        current.path === tab.path,
    );
    const nextOpenPreviewTabs = openHtmlPreviewTabsRef.current.filter(
      (current) =>
        current.workspaceId !== tab.workspaceId || current.path !== tab.path,
    );
    openHtmlPreviewTabsRef.current = nextOpenPreviewTabs;
    setOpenHtmlPreviewTabs(nextOpenPreviewTabs);

    if (
      activeMainTab.type !== "htmlPreview" ||
      activeMainTab.workspaceId !== tab.workspaceId ||
      activeMainTab.path !== tab.path
    ) {
      updateBrowserRouteAfterTabClose(tab.workspaceId);
      return;
    }

    const nextTabs = mainTabs.filter(
      (current) =>
        !(
          current.type === "htmlPreview" &&
          current.workspaceId === tab.workspaceId &&
          current.path === tab.path
        ),
    );
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveMainTab({
      chatId: null,
      type: "chat",
      workspaceId: activeWorkspaceId || tab.workspaceId,
    });
    updateBrowserRoute(
      {
        chatId: activeChatId,
        viewMode: "chat",
        workspaceId: activeWorkspaceId || tab.workspaceId,
      },
      "replace",
    );
  }

  function updateBrowserRouteAfterTabClose(fallbackWorkspaceId: string) {
    if (activeMainTab.type === "file" && activeFileTab) {
      updateBrowserRoute(browserRouteForActiveFile(activeFileTab), "replace");
      return;
    }
    if (activeMainTab.type === "htmlPreview" && activeHtmlPreviewTab) {
      updateBrowserRoute(
        browserRouteForActiveHtmlPreview(activeHtmlPreviewTab),
        "replace",
      );
      return;
    }
    updateBrowserRoute(
      {
        chatId: activeChatId,
        viewMode: "chat",
        workspaceId: activeWorkspaceId || fallbackWorkspaceId,
      },
      "replace",
    );
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
      (tab) =>
        !closedKeys.has(`chat:${chatRunKey(tab.workspaceId, tab.chatId)}`),
    );
    const nextOpenFileTabs = openFileTabsRef.current.filter(
      (tab) =>
        !closedKeys.has(workspaceFileEditorKey(tab.workspaceId, tab.path)),
    );
    const nextOpenHtmlPreviewTabs = openHtmlPreviewTabsRef.current.filter(
      (tab) =>
        !closedKeys.has(workspaceHtmlPreviewKey(tab.workspaceId, tab.path)),
    );

    openChatTabsRef.current = nextOpenChatTabs;
    openFileTabsRef.current = nextOpenFileTabs;
    openHtmlPreviewTabsRef.current = nextOpenHtmlPreviewTabs;
    setOpenChatTabs(nextOpenChatTabs);
    setOpenFileTabs(nextOpenFileTabs);
    setOpenHtmlPreviewTabs(nextOpenHtmlPreviewTabs);
    setOpenAgentTabs((current) => {
      const next = current.filter(
        (tab) =>
          !closedKeys.has(
            `agent:${tab.workspaceId}:${tab.chatId}:${tab.instanceId}`,
          ),
      );
      pruneAgentTabCaches(
        agentTeamSnapshotCacheRef.current,
        agentTranscriptViewCacheRef.current,
        next,
      );
      return next;
    });

    for (const tab of tabsToClose) {
      if (tab.type === "chat") {
        const chatKey = chatRunKey(tab.workspaceId, tab.chatId);
        setChatRunFailed(chatKey, false);
        removeMessagesForChatKey(chatKey);
        removeChatPaginationForChatKey(chatKey);
        removeContextUsageForChatKey(chatKey);
      }
    }

    setWorkspaceFileEditors((current) => {
      const next = { ...current };
      for (const tab of tabsToClose) {
        if (tab.type === "file") {
          const editorKey = workspaceFileEditorKey(tab.workspaceId, tab.path);
          delete next[editorKey];
          delete workspaceFileEditorViewStatesRef.current[editorKey];
          delete workspaceMarkdownPreviewScrollTopsRef.current[editorKey];
        }
      }
      return next;
    });

    const activeWasClosed = tabsToClose.some((tab) =>
      mainTabMatches(activeMainTab, tab),
    );
    if (!activeWasClosed) {
      updateBrowserRouteAfterTabClose(anchorTab.workspaceId);
      return;
    }

    const nextTab =
      nextTabs[Math.min(anchorIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    const workspaceId = activeWorkspaceId || anchorTab.workspaceId;
    setActiveWorkspaceChatRefs(workspaceId, null, { syncPlanMode: true });
    setActiveChatId(null);
    setMessages([]);
    setActiveMainTab({ chatId: null, type: "chat", workspaceId });
    updateBrowserRoute(
      {
        chatId: null,
        viewMode: "chat",
        workspaceId,
      },
      "replace",
    );
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
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);
    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    selectWorkspaceChat(tab.workspaceId, tab.chatId);
  }

  function closeChatTab(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);

    const tabIndex = mainTabs.findIndex(
      (tab) =>
        tab.type === "chat" &&
        tab.workspaceId === workspaceId &&
        tab.chatId === chatId,
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
      updateBrowserRoute(
        {
          chatId: activeChatId,
          tabs: openChatTabsToBrowserRouteTabs(nextOpenChatTabs),
          viewMode: "chat",
          workspaceId: activeWorkspaceId || workspaceId,
        },
        "replace",
      );
      return;
    }

    const nextTabs = mainTabs.filter(
      (tab) =>
        !(
          tab.type === "chat" &&
          tab.workspaceId === workspaceId &&
          tab.chatId === chatId
        ),
    );
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);

    if (nextTab) {
      selectMainTab(nextTab);
      return;
    }

    setActiveWorkspaceChatRefs(activeWorkspaceId || workspaceId, null, {
      syncPlanMode: true,
    });
    setActiveChatId(null);
    setMessages([]);
    setActiveMainTab({
      chatId: null,
      type: "chat",
      workspaceId: activeWorkspaceId || workspaceId,
    });
    updateBrowserRoute({
      chatId: null,
      viewMode: "chat",
      workspaceId: activeWorkspaceId || workspaceId,
    });
  }

  function openWorkspaceChatContextMenu(
    event: Pick<
      ReactMouseEvent<HTMLElement> | ReactPointerEvent<HTMLElement>,
      "clientX" | "clientY" | "preventDefault" | "stopPropagation"
    >,
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

  function requestDeleteWorkspaceChat(
    workspace: WorkspaceSummary,
    chat: ChatSummary,
  ) {
    const chatKey = chatRunKey(workspace.id, chat.id);
    if (
      chatSessionStatusFor(chatKey, { workspaceActiveRun: chat.activeRun })
        .kind === "running"
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
      chatSessionStatusFor(chatKey, {
        workspaceActiveRun: workspaceChat?.activeRun ?? null,
      }).kind === "running"
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

      // Explicit delete is the authoritative cleanup path: remove all open tabs
      // and client caches for this chat before workspace refresh (pagination
      // absence must not be treated as deletion).
      const nextOpenChatTabs = openChatTabsRef.current.filter(
        (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
      );
      openChatTabsRef.current = nextOpenChatTabs;
      setOpenChatTabs(nextOpenChatTabs);

      setOpenAgentTabs((current) => {
        const next = current.filter(
          (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
        );
        if (next.length !== current.length) {
          pruneAgentTabCaches(
            agentTeamSnapshotCacheRef.current,
            agentTranscriptViewCacheRef.current,
            next,
          );
        }
        return next.length === current.length ? current : next;
      });

      setChatRunFailed(chatKey, false);
      removeMessagesForChatKey(chatKey);
      removeChatPaginationForChatKey(chatKey);
      removeContextUsageForChatKey(chatKey);
      setChatRunning(chatKey, false);
      setActiveRunInfoForChatKey(chatKey, null);
      setRetryRunRequest((current) =>
        current?.chatId === chatId && current.workspaceId === workspaceId
          ? null
          : current,
      );
      setPendingDeleteChat(null);

      const activeMainMatchesDeleted =
        (activeMainTab.type === "chat" &&
          activeMainTab.workspaceId === workspaceId &&
          activeMainTab.chatId === chatId) ||
        (activeMainTab.type === "agent" &&
          activeMainTab.workspaceId === workspaceId &&
          activeMainTab.chatId === chatId);
      const activeChatMatchesDeleted =
        activeWorkspaceId === workspaceId && activeChatId === chatId;

      if (activeMainMatchesDeleted || activeChatMatchesDeleted) {
        const tabIndex = mainTabs.findIndex(
          (tab) =>
            (tab.type === "chat" &&
              tab.workspaceId === workspaceId &&
              tab.chatId === chatId) ||
            (tab.type === "agent" &&
              tab.workspaceId === workspaceId &&
              tab.chatId === chatId),
        );
        const nextMainTabs = mainTabs.filter(
          (tab) =>
            !(
              (tab.type === "chat" &&
                tab.workspaceId === workspaceId &&
                tab.chatId === chatId) ||
              (tab.type === "agent" &&
                tab.workspaceId === workspaceId &&
                tab.chatId === chatId)
            ),
        );
        const nextTab =
          tabIndex >= 0
            ? (nextMainTabs[Math.min(tabIndex, nextMainTabs.length - 1)] ??
              nextMainTabs.at(-1))
            : nextMainTabs.at(-1);

        if (nextTab) {
          selectMainTab(nextTab);
        } else {
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
      } else {
        updateBrowserRoute(
          {
            chatId: activeChatId,
            tabs: openChatTabsToBrowserRouteTabs(nextOpenChatTabs),
            viewMode: "chat",
            workspaceId: activeWorkspaceId || workspaceId,
          },
          "replace",
        );
      }

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
  function renameWorkspaceFileTab(
    workspaceId: string,
    path: string,
    newName: string,
  ) {
    const nextPath = workspaceRenamedFilePath(path, newName);
    const nextOpenFileTabs = openFileTabsRef.current.map((tab) => {
      if (tab.workspaceId !== workspaceId || tab.path !== path) {
        return tab;
      }
      return {
        ...tab,
        name: newName,
        path: nextPath,
      };
    });
    openFileTabsRef.current = nextOpenFileTabs;
    setOpenFileTabs(nextOpenFileTabs);
    const renamedPathMatches = (key: string) => {
      const prefix = `${workspaceId}:`;
      if (!key.startsWith(prefix)) {
        return false;
      }
      const filePath = key.slice(prefix.length);
      return (
        filePath === path ||
        filePath.startsWith(`${path}/`) ||
        filePath === nextPath ||
        filePath.startsWith(`${nextPath}/`)
      );
    };
    for (const key of Object.keys(workspaceFileEditorViewStatesRef.current)) {
      if (renamedPathMatches(key)) {
        delete workspaceFileEditorViewStatesRef.current[key];
      }
    }
    for (const key of Object.keys(workspaceMarkdownPreviewScrollTopsRef.current)) {
      if (renamedPathMatches(key)) {
        delete workspaceMarkdownPreviewScrollTopsRef.current[key];
      }
    }
    setWorkspaceFileEditors((current) => {
      const oldKey = workspaceFileEditorKey(workspaceId, path);
      const newKey = workspaceFileEditorKey(workspaceId, nextPath);
      if (!(oldKey in current)) {
        return current;
      }
      const next = { ...current, [newKey]: current[oldKey] };
      delete next[oldKey];
      return next;
    });
    setActiveMainTab((current) =>
      current.type === "file" &&
      current.workspaceId === workspaceId &&
      current.path === path
        ? { path: nextPath, type: "file", workspaceId }
        : current,
    );

    const stillHtml = isHtmlPreviewPath(nextPath);
    setOpenHtmlPreviewTabs((current) => {
      const next = current.flatMap((tab) => {
        if (tab.workspaceId !== workspaceId || tab.path !== path) {
          return [tab];
        }
        if (!stillHtml) {
          return [];
        }
        return [{ ...tab, name: newName, path: nextPath }];
      });
      openHtmlPreviewTabsRef.current = next;
      return next;
    });
    setActiveMainTab((current) => {
      if (
        current.type === "htmlPreview" &&
        current.workspaceId === workspaceId &&
        current.path === path
      ) {
        return stillHtml
          ? { path: nextPath, type: "htmlPreview", workspaceId }
          : { chatId: activeChatId, type: "chat", workspaceId };
      }
      return current;
    });
  }

  function closeWorkspaceFileTabsForPath(workspaceId: string, path: string) {
    const nextOpenFileTabs = openFileTabsRef.current.filter(
      (tab) =>
        tab.workspaceId !== workspaceId ||
        (tab.path !== path && !tab.path.startsWith(`${path}/`)),
    );
    openFileTabsRef.current = nextOpenFileTabs;
    setOpenFileTabs(nextOpenFileTabs);
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
    const deletedPathMatches = (key: string) => {
      const prefix = `${workspaceId}:`;
      if (!key.startsWith(prefix)) {
        return false;
      }
      const filePath = key.slice(prefix.length);
      return filePath === path || filePath.startsWith(`${path}/`);
    };
    for (const key of Object.keys(workspaceFileEditorViewStatesRef.current)) {
      if (deletedPathMatches(key)) {
        delete workspaceFileEditorViewStatesRef.current[key];
      }
    }
    for (const key of Object.keys(workspaceMarkdownPreviewScrollTopsRef.current)) {
      if (deletedPathMatches(key)) {
        delete workspaceMarkdownPreviewScrollTopsRef.current[key];
      }
    }
    setOpenHtmlPreviewTabs((current) => {
      const next = current.filter((tab) => {
        if (tab.workspaceId !== workspaceId) {
          return true;
        }
        const matches = tab.path === path || tab.path.startsWith(`${path}/`);
        return !matches;
      });
      openHtmlPreviewTabsRef.current = next;
      return next;
    });
    if (
      activeMainTab.type === "file" &&
      activeMainTab.workspaceId === workspaceId &&
      (activeMainTab.path === path || activeMainTab.path.startsWith(`${path}/`))
    ) {
      setActiveMainTab({ chatId: activeChatId, type: "chat", workspaceId });
    }
    if (
      activeMainTab.type === "htmlPreview" &&
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
      setWorkspaceFilesError(
        t("Select a workspace before using file actions."),
      );
      return;
    }

    const operationKey = `${action}:${path}`;
    setWorkspaceFileOperationKey(operationKey);
    setWorkspaceFilesError(null);

    try {
      const data = await requestJson<WorkspaceFileChildrenResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/files/${action}`,
        {
          body: JSON.stringify(
            action === "rename" ? { path, newName } : { path },
          ),
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
              root: replaceWorkspaceFileNodeChildren(
                current.root,
                data.path,
                data.children,
              ),
            }
          : current,
      );
      if (isContextPanelOpen && contextPanelTab === "git") {
        void loadGitDiff(
          activeWorkspace.id,
          selectedDiffPath,
          sourceControlTarget,
        );
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
      const loaded = await loadWorkspaceDirectoryChildren(
        activeWorkspace.id,
        node.path,
      );
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

  async function copyDiagnosticReference(diagnosticId: string) {
    try {
      await navigator.clipboard.writeText(diagnosticId);
    } catch (copyError) {
      setError(errorMessage(copyError));
    }
  }

  function workspaceFileDownloadUrl(workspaceId: string, path: string) {
    return `/api/workspaces/${encodeURIComponent(workspaceId)}/files/download?path=${encodeURIComponent(path)}`;
  }

  function downloadWorkspaceFile(node: WorkspaceFileTreeNode) {
    if (!activeWorkspace) {
      setWorkspaceFilesError(
        t("Select a workspace before using file actions."),
      );
      return;
    }
    if (!node.path) {
      setWorkspaceFilesError(
        t("Select a workspace before using file actions."),
      );
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

  function workspaceFileAbsolutePath(
    workspacePath: string,
    relativePath: string,
  ) {
    const separator = workspacePath.includes("\\") ? "\\" : "/";
    const root = workspacePath.replace(/[\\/]+$/, "");
    const normalizedRelativePath = relativePath.replace(/[\\/]+/g, separator);
    return root
      ? `${root}${separator}${normalizedRelativePath}`
      : `${separator}${normalizedRelativePath}`;
  }

  async function handleGitFileOperation(
    action: "stage" | "unstage" | "discard",
    path: string,
  ) {
    if (!activeWorkspace) {
      setDiffError(t("Select a workspace before using Git actions."));
      return;
    }

    const workspaceId = activeWorkspace.id;
    const target = sourceControlTarget;
    const targetKey = sourceControlTargetKey(target);
    invalidateGitDiffRequest();
    const requestId = gitOperationRequestIdRef.current + 1;
    gitOperationRequestIdRef.current = requestId;
    const isCurrentOperation = () =>
      gitOperationRequestIdRef.current === requestId &&
      activeWorkspaceIdRef.current === workspaceId &&
      sourceControlViewRef.current.workspaceId === workspaceId &&
      sourceControlTargetKey(sourceControlViewRef.current.target) === targetKey;
    const operationKey = `${action}:${path}`;
    setGitOperationKey(operationKey);
    setDiffError(null);

    try {
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/${action}`,
        {
          body: JSON.stringify(
            gitTargetRequestBody({ path }, target),
          ),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      if (!isCurrentOperation()) {
        return;
      }
      invalidateGitDiffRequest();
      setGitDiff(data);
      setSelectedDiffPath(
        selectedDiffPath &&
          data.files.some((file) => file.path === selectedDiffPath)
          ? selectedDiffPath
          : null,
      );
    } catch (requestError) {
      if (isCurrentOperation()) {
        setDiffError(errorMessage(requestError));
      }
    } finally {
      if (isCurrentOperation()) {
        setGitOperationKey(null);
      }
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

    const workspaceId = activeWorkspace.id;
    const target = sourceControlTarget;
    const targetKey = sourceControlTargetKey(target);
    invalidateGitDiffRequest();
    const requestId = gitOperationRequestIdRef.current + 1;
    gitOperationRequestIdRef.current = requestId;
    const isCurrentOperation = () =>
      gitOperationRequestIdRef.current === requestId &&
      activeWorkspaceIdRef.current === workspaceId &&
      sourceControlViewRef.current.workspaceId === workspaceId &&
      sourceControlTargetKey(sourceControlViewRef.current.target) === targetKey;
    setGitOperationKey("commit");
    setDiffError(null);

    try {
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/commit`,
        {
          body: JSON.stringify(
            gitTargetRequestBody({ message }, target),
          ),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      if (!isCurrentOperation()) {
        return;
      }
      invalidateGitDiffRequest();
      setGitDiff(data);
      setGitCommitMessage("");
      setSelectedDiffPath(null);
    } catch (requestError) {
      if (isCurrentOperation()) {
        setDiffError(errorMessage(requestError));
      }
    } finally {
      if (isCurrentOperation()) {
        setGitOperationKey(null);
      }
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
      setDiffError(
        t("Select an enabled model before generating a commit message."),
      );
      return;
    }

    const workspaceId = activeWorkspace.id;
    const target = sourceControlTarget;
    const targetKey = sourceControlTargetKey(target);
    invalidateGitDiffRequest();
    const requestId = gitOperationRequestIdRef.current + 1;
    gitOperationRequestIdRef.current = requestId;
    const isCurrentOperation = () =>
      gitOperationRequestIdRef.current === requestId &&
      activeWorkspaceIdRef.current === workspaceId &&
      sourceControlViewRef.current.workspaceId === workspaceId &&
      sourceControlTargetKey(sourceControlViewRef.current.target) === targetKey;
    setGitOperationKey("generate-commit-message");
    setDiffError(null);

    try {
      const data = await requestJson<GitCommitMessageResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/commit-message`,
        {
          body: JSON.stringify(
            gitTargetRequestBody(
              {
                modelId: selectedModelId,
                providerId: selectedProviderId,
              },
              target,
            ),
          ),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      if (isCurrentOperation()) {
        setGitCommitMessage(data.message);
      }
    } catch (requestError) {
      if (isCurrentOperation()) {
        setDiffError(errorMessage(requestError));
      }
    } finally {
      if (isCurrentOperation()) {
        setGitOperationKey(null);
      }
    }
  }

  function removeSelectedSkill(skillId: string) {
    setSelectedSkillIds((current) => current.filter((id) => id !== skillId));
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
      draftAttachments.reduce(
        (sum, attachment) => sum + attachment.sizeBytes,
        0,
      ) +
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

    const currentWorkspaceId =
      activeWorkspaceIdRef.current || activeWorkspace?.id || "";
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
      setError(
        message ?? t("Selected model does not support this attachment."),
      );
      return null;
    }

    const skillIds = [...effectiveSelectedSkillIds];
    return {
      attachments,
      chatId:
        currentChatId && !isPendingChatId(currentChatId) ? currentChatId : null,
      content,
      modelId: selectedModelId,
      providerId: selectedProviderId,
      skillIds,
      sessionMode: isPlanModeEnabled ? "plan" : undefined,
      teamModeEnabled:
        !isPlanModeEnabled && canUseTeamMode && isTeamModeEnabled,
      thinkingLevel: isModelThinkingLevelSupported(
        selectedModel,
        selectedThinkingLevel,
      )
        ? selectedThinkingLevel
        : "",
      latencyMode: selectedRequestLatencyMode,
      workspaceId: currentWorkspace.id,
    };
  }

  function handlePlanModeEnabledChange(value: boolean) {
    // Draft-only toggle: do not write chat history / restore cache until send or load.
    setIsPlanModeEnabled(value);
    applyComposerModelForPlanMode(value);
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
      !isGuidableActiveRun(
        runInfo,
        runningChatKeysRef.current.has(requestChatKey),
      )
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
          latencyMode: request.latencyMode ?? "standard",
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
    if (
      !workspaceId ||
      !chatId ||
      isPendingChatId(chatId) ||
      isSendingMessage
    ) {
      return false;
    }
    const runConfig = message.runConfig;
    const modelId = runConfig?.modelId ?? selectedModelIdRef.current;
    const providerId = runConfig?.providerId ?? selectedProviderIdRef.current;
    if (!modelId || !providerId) {
      setError(t("This message does not have a reusable model configuration."));
      return false;
    }
    const requestLatencyMode = latencyModeForModel(
      availableModels.find((model) => model.id === modelId),
    );
    const chatKey = chatRunKey(workspaceId, chatId);
    const previousMessages = [...(chatMessagesByKeyRef.current[chatKey] ?? [])];
    setError(null);
    setIsPreparingChatRun(true);
    try {
      const edited = await requestJson<EditChatUserMessageResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages/${encodeURIComponent(message.id)}/edit`,
        {
          body: JSON.stringify({
            attachments: attachments.map(
              ({ previewDataUrl: _previewDataUrl, ...attachment }) =>
                attachment,
            ),
            expectedContent: message.content,
            message: content,
            modelId,
            providerId,
            thinkingLevel: runConfig?.thinkingLevel || null,
            latencyMode: requestLatencyMode,
            selectedSkillIds: editedSkillIds,
            sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? null,
            teamModeEnabled: runConfig?.teamModeEnabled ?? false,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      const targetIndex = previousMessages.findIndex(
        (item) => item.id === message.id,
      );
      if (targetIndex < 0) {
        throw new Error(t("Edited message is no longer visible."));
      }
      setMessagesForChatKey(chatKey, () => [
        ...previousMessages.slice(0, targetIndex),
        {
          ...message,
          content: edited.content,
          parts: edited.parts,
          sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? null,
          runConfig: {
            modelId,
            providerId,
            thinkingLevel: runConfig?.thinkingLevel ?? null,
            latencyMode: requestLatencyMode,
            selectedSkillIds: editedSkillIds,
            sessionMode: runConfig?.sessionMode ?? message.sessionMode ?? null,
            teamModeEnabled: runConfig?.teamModeEnabled ?? false,
          },
        },
      ]);
      bindRequestPlanModeToChatKey(
        {
          attachments,
          chatId,
          content: edited.content,
          modelId,
          providerId,
          skillIds: editedSkillIds,
          sessionMode:
            runConfig?.sessionMode ?? message.sessionMode ?? undefined,
          teamModeEnabled: runConfig?.teamModeEnabled ?? false,
          thinkingLevel: runConfig?.thinkingLevel ?? "",
          latencyMode: requestLatencyMode,
          workspaceId,
        },
        chatKey,
      );
      if (activeChatKeyRef.current === chatKey) {
        const planEnabled =
          (runConfig?.sessionMode ?? message.sessionMode) === "plan";
        setIsPlanModeEnabled(planEnabled);
        applyComposerModelForPlanMode(planEnabled);
      }
      onAccepted();
      updateQueuedRunRequestsForChatKey(chatKey, () => []);
      updateScheduledWorkspaceRuns((current) =>
        current.filter((run) => run.chatKey !== chatKey),
      );
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
        latencyMode: requestLatencyMode,
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

    if (
      request.chatId &&
      readOnlyChatKeys[chatRunKey(request.workspaceId, request.chatId)]
    ) {
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
      const queued = await persistQueuedRunRequest(request, {
        deferStart: true,
      });
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

      bindRequestPlanModeToChatKey(request, runInfo.chatKey);
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
      ? (activeRunInfoByChatKeyRef.current[chatKey] ?? null)
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
      availableSkills,
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
    onRestoreWorkspaceHtmlPreviewTabs: restoreWorkspaceHtmlPreviewTabs,
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
    (
      page: number,
      filters: Partial<AiStatsFilterState> = statsRouteFiltersRef.current,
    ) => {
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
        chatId:
          activeChatId && !isPendingChatId(activeChatId) ? activeChatId : "",
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

    const modelId = runInfo?.modelId ?? "";
    const providerId = runInfo?.providerId ?? "";
    if (runInfo?.chatId && modelId && providerId) {
      void refreshContextUsage({
        chatId: runInfo.chatId,
        modelId,
        providerId,
        skillIds: [],
        thinkingLevel: selectedThinkingLevelRef.current,
        workspaceId: runInfo.workspaceId,
      });
    }

    invalidateChatStreamSession(currentChatKey);
    activeRunAbortByChatKeyRef.current.get(currentChatKey)?.abort();
    setChatRunning(currentChatKey, false);
    setActiveRunInfoForChatKey(currentChatKey, null);
    clearLiveChatStatistics(currentChatKey);
    setChatRunFailed(currentChatKey, false);
    const cancelledChat = runInfo?.chatId
      ? { chatId: runInfo.chatId, workspaceId: runInfo.workspaceId }
      : parseChatRunKey(currentChatKey);
    if (cancelledChat) {
      clearWorkspaceChatActiveRun(
        cancelledChat.workspaceId,
        cancelledChat.chatId,
      );
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
    setContextUsageErrorByChatKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }
      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
    deferStreamAuxiliaryUpdate(() => {
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
            ...(request.assistantDraft
              ? { assistantDraft: request.assistantDraft }
              : {}),
            ...(request.assistantDraftReasoning
              ? { assistantDraftReasoning: request.assistantDraftReasoning }
              : {}),
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (
        contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId
      ) {
        deferStreamAuxiliaryUpdate(() => {
          setContextUsageByChatKey((current) => ({
            ...current,
            // The endpoint returns an estimate only. Retain the exact request
            // route locally so live provider input tokens can never be paired
            // with a context window from another model/provider.
            [chatKey]: {
              ...data,
              modelId: request.modelId,
              providerId: request.providerId,
            },
          }));
          setContextUsageErrorByChatKey((current) => {
            if (!(chatKey in current)) {
              return current;
            }
            const next = { ...current };
            delete next[chatKey];
            return next;
          });
        });
      }
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException &&
        requestError.name === "AbortError";
      if (
        !wasCancelled &&
        contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId &&
        activeChatKeyRef.current === chatKey
      ) {
        setContextUsageErrorByChatKey((current) => ({
          ...current,
          [chatKey]: {
            diagnostic: errorDiagnostic(requestError),
            message: errorMessage(requestError),
          },
        }));
      }
    } finally {
      if (
        contextUsageAbortByChatKeyRef.current.get(chatKey) === abortController
      ) {
        contextUsageAbortByChatKeyRef.current.delete(chatKey);
      }
      if (
        contextUsageRequestIdByChatKeyRef.current.get(chatKey) === requestId
      ) {
        deferStreamAuxiliaryUpdate(() => {
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
    const syntheticSource = isAutomaticGuardSource(guidance.source)
      ? guidance.source
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
                ? [
                    { type: "text" as const, text: guidance.content },
                    ...guidance.parts,
                  ]
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
                  ? [
                      { type: "text" as const, text: guidance.content },
                      ...guidance.parts,
                    ]
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
      availableSkills,
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
    if (!startChatRun(chatKey, activeRun.runId)) {
      clearWorkspaceChatActiveRun(
        activeRun.workspaceId,
        activeRun.chatId,
        activeRun.runId,
      );
      return;
    }
    const existingSession = chatStreamSessionsByChatKeyRef.current.get(chatKey);
    const existingAssistantMessageId = existingSession?.assistantMessageId ?? null;
    const incomingAssistantMessageId = activeRun.assistantMessageId ?? null;
    if (
      existingSession?.runId === activeRun.runId &&
      existingAssistantMessageId &&
      incomingAssistantMessageId &&
      existingAssistantMessageId !== incomingAssistantMessageId
    ) {
      reportChatStreamOwnershipConflict("assistantIdentityMismatch", {
        canonicalAssistantMessageId: incomingAssistantMessageId,
        chatId: activeRun.chatId,
        epoch: existingSession.epoch,
        incomingAssistantMessageId: existingAssistantMessageId,
        runId: activeRun.runId,
        workspaceId: activeRun.workspaceId,
      });
      // The server summary is authoritative. Merge the retired local alias before
      // retiring its epoch so a same-run identity correction cannot leave two
      // assistant bubbles visible while the replacement subscription connects.
      setMessagesForChatKey(chatKey, (current) =>
        canonicalizeAssistantMessage(current, incomingAssistantMessageId, [
          existingAssistantMessageId,
        ]),
      );
      chatStreamSessionsByChatKeyRef.current.delete(chatKey);
      existingSession.abortController.abort();
    }
    const abortController = new AbortController();
    const session = claimChatStreamSession(
      chatKey,
      activeRun.runId,
      abortController,
      {
        // A live subscription for the same durable run already owns the UI. A
        // reconnect is only created after its previous owner has released.
        reuseSameRun: true,
      },
    );
    if (!session) {
      const existing = chatStreamSessionsByChatKeyRef.current.get(chatKey);
      reportChatStreamOwnershipConflict("duplicateWritableSession", {
        canonicalAssistantMessageId: existing?.assistantMessageId ?? null,
        chatId: activeRun.chatId,
        epoch: existing?.epoch ?? null,
        incomingAssistantMessageId: activeRun.assistantMessageId ?? null,
        runId: activeRun.runId,
        workspaceId: activeRun.workspaceId,
      });
      return;
    }
    session.assistantMessageId = activeRun.assistantMessageId ?? null;
    const ownsSession = () => isCurrentChatStreamSession(chatKey, session);
    const canFlushBufferedDelta = (
      _chatKey: string,
      assistantMessageId: string,
    ) => ownsSession() && Boolean(assistantMessageId);
    let assistantMessageId = activeRun.assistantMessageId ?? "";
    let currentAssistantMessageId = assistantMessageId;
    // After guidance, the backend keeps emitting events under the durable
    // interrupted assistant id. Map that id to the latest visible bubble so
    // consecutive recoveries do not re-route to an earlier segment.
    let interruptedAssistantMessageId: string | null = null;
    // A reconnect has no composer request to trust. Preserve a route learned
    // from this same run only; otherwise context usage remains the backend
    // estimate until an identity-bearing terminal metric arrives.
    const existingRunInfo = activeRunInfoByChatKeyRef.current[chatKey];
    const runMessageContextUsageInput = contextUsageInputFromRunMessage(
      activeRun,
      chatMessagesByKeyRef.current[chatKey] ?? [],
    );
    const runModelIdentity =
      existingRunInfo?.runId === activeRun.runId &&
      existingRunInfo.modelId &&
      existingRunInfo.providerId
        ? {
            modelId: existingRunInfo.modelId,
            providerId: existingRunInfo.providerId,
          }
        : runMessageContextUsageInput
          ? {
              modelId: runMessageContextUsageInput.modelId,
              providerId: runMessageContextUsageInput.providerId,
            }
          : null;
    // Active-run reconnect summaries omit parts of the immutable request
    // configuration. Recover them only from this run's durable queued-user
    // message; never fill gaps from the mutable composer selection.
    const runContextUsageConfig =
      runModelIdentity &&
      runMessageContextUsageInput &&
      runModelIdentity.modelId === runMessageContextUsageInput.modelId &&
      runModelIdentity.providerId === runMessageContextUsageInput.providerId
        ? runMessageContextUsageInput
        : null;
    let latestResponseUsage: ChatUsage | null = null;
    let liveStartedAtMs = Date.now();
    let liveAssistantDraft = "";
    let liveAssistantDraftReasoning = "";
    let lastLiveContextUsageRefreshAtMs = Date.now();
    let hasGuidanceTurns = false;
    let terminalContextUsageRefreshRequested = false;
    const textDeltaBuffer = createTextDeltaBuffer(
      () => ownsSession(),
      canFlushBufferedDelta,
    );
    const reasoningDeltaBuffer = createReasoningDeltaBuffer(() =>
      ownsSession(),
      canFlushBufferedDelta,
    );
    const toolOutputDeltaBuffer = createToolOutputDeltaBuffer(() =>
      ownsSession(),
      canFlushBufferedDelta,
    );
    const remapBufferedAssistantMessageId = (
      previousAssistantMessageId: string,
      canonicalAssistantMessageId: string,
    ) => {
      textDeltaBuffer.remapAssistantMessageId(
        chatKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
      reasoningDeltaBuffer.remapAssistantMessageId(
        chatKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
      toolOutputDeltaBuffer.remapAssistantMessageId(
        chatKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
    };
    const flushStreamDeltaBuffers = () => {
      textDeltaBuffer.flush();
      reasoningDeltaBuffer.flush();
      toolOutputDeltaBuffer.flush();
    };
    const refreshRunContextUsage = (
      modelIdentity: { modelId: string; providerId: string } | null =
        runModelIdentity,
    ): boolean => {
      if (!ownsSession()) {
        return false;
      }
      if (!modelIdentity) {
        return false;
      }
      if (!runContextUsageConfig) {
        return false;
      }

      void refreshContextUsage({
        chatId: activeRun.chatId,
        modelId: modelIdentity.modelId,
        providerId: modelIdentity.providerId,
        skillIds: runContextUsageConfig.skillIds,
        thinkingLevel: runContextUsageConfig.thinkingLevel,
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
      if (
        now - lastLiveContextUsageRefreshAtMs <
        LIVE_CONTEXT_USAGE_REFRESH_MS
      ) {
        return;
      }
      if (!runModelIdentity || !runContextUsageConfig) {
        return;
      }

      lastLiveContextUsageRefreshAtMs = now;
      void refreshContextUsage({
        assistantDraft: liveAssistantDraft,
        assistantDraftReasoning: liveAssistantDraftReasoning,
        chatId: activeRun.chatId,
        modelId: runModelIdentity.modelId,
        providerId: runModelIdentity.providerId,
        skillIds: runContextUsageConfig.skillIds,
        thinkingLevel: runContextUsageConfig.thinkingLevel,
        workspaceId: activeRun.workspaceId,
      });
    };

    const ensureStreamingAssistantMessage = (
      nextAssistantMessageId: string,
      memoriesUsed: ChatMemoryUsedSummary[] = [],
    ) => {
      if (!ownsSession() || !nextAssistantMessageId) {
        return;
      }
      setMessagesForChatKey(chatKey, (current) => {
        if (current.some((message) => message.id === nextAssistantMessageId)) {
          return current.map((message) =>
            message.id === nextAssistantMessageId &&
            message.role === "assistant"
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
    const finishStreamingAssistantMessage = (
      finishedAssistantMessageId: string,
    ) => {
      if (!ownsSession() || !finishedAssistantMessageId) {
        return;
      }
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
    const streamAttemptSnapshots = new Map<string, StreamAttemptSnapshot>();
    const startLiveReasoningDuration = () => {
      if (activeReasoningStartedAtMs !== null) {
        return activeReasoningStartedAtMs;
      }
      const startedAtMs = Date.now();
      activeReasoningStartedAtMs = startedAtMs;
      return startedAtMs;
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
                ? finishActiveReasoningPart(
                    message.parts,
                    startedAtMs,
                    endedAtMs,
                  )
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
      if (!currentAssistantMessageId && eventAssistantMessageId) {
        remapBufferedAssistantMessageId("", eventAssistantMessageId);
        assistantMessageId = eventAssistantMessageId;
        currentAssistantMessageId = eventAssistantMessageId;
        session.assistantMessageId = eventAssistantMessageId;
        return eventAssistantMessageId;
      }
      if (
        eventAssistantMessageId &&
        currentAssistantMessageId &&
        eventAssistantMessageId !== currentAssistantMessageId
      ) {
        reportChatStreamOwnershipConflict("assistantIdentityMismatch", {
          canonicalAssistantMessageId: currentAssistantMessageId,
          chatId: activeRun.chatId,
          epoch: session.epoch,
          incomingAssistantMessageId: eventAssistantMessageId,
          runId: activeRun.runId,
          workspaceId: activeRun.workspaceId,
        });
        void loadChatMessages(
          activeRun.workspaceId,
          activeRun.chatId,
          session,
        );
      }
      return currentAssistantMessageId;
    };

    let lastProcessedSequence = activeRun.lastSequence ?? -1;
    session.lastSequence = activeRun.lastSequence ?? null;
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
      session.lastSequence = sequence;
      const currentRunInfo = activeRunInfoByChatKeyRef.current[chatKey];
      if (currentRunInfo?.runId === activeRun.runId) {
        setActiveRunInfoForChatKey(chatKey, {
          ...currentRunInfo,
          lastSequence: sequence,
        });
      }
    };
    // Track assistants that already received live deltas/tools for this chat so
    // later Coordinator attempt `start` events (including GET reattach) keep history.
    const markAssistantLiveStreamEvent = (eventAssistantMessageId?: string) => {
      const assistantId = resolvedAssistantMessageId(eventAssistantMessageId);
      const tracked =
        liveStreamAssistantIdsByChatKeyRef.current.get(chatKey) ??
        new Set<string>();
      tracked.add(assistantId);
      liveStreamAssistantIdsByChatKeyRef.current.set(chatKey, tracked);
    };
    const hasSeenLiveStreamEventsForAssistant = (assistantId: string) =>
      liveStreamAssistantIdsByChatKeyRef.current
        .get(chatKey)
        ?.has(assistantId) ?? false;

    setChatRunning(chatKey, true);
    setChatRunFailed(chatKey, false);
    setActiveRunInfoForChatKey(chatKey, {
      acceptingGuidance: activeRun.acceptingGuidance,
      assistantMessageId: activeRun.assistantMessageId ?? null,
      assistantSequence: activeRun.assistantSequence ?? null,
      chatId: activeRun.chatId,
      chatKey,
      lastSequence: lastSequenceForState(),
      queuedUserMessageId: activeRun.queuedUserMessageId ?? null,
      runId: activeRun.runId,
      modelId: runModelIdentity?.modelId,
      providerId: runModelIdentity?.providerId,
      workspaceId: activeRun.workspaceId,
    });
    activeRunAbortByChatKeyRef.current.set(chatKey, abortController);
    let shouldReconnect = false;
    let streamEnded = false;

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
        const backendError = await responseError(response);
        const message = backendError.message;
        if (
          (response.status === 400 || response.status === 404) &&
          isStaleActiveRunError(message)
        ) {
          console.debug(
            "[chat-stream] active run stream is stale; refreshing messages",
            {
              chatId: activeRun.chatId,
              runId: activeRun.runId,
              status: response.status,
              workspaceId: activeRun.workspaceId,
            },
          );
          finishChatRun(
            chatKey,
            activeRun.runId,
            activeRun.workspaceId,
            activeRun.chatId,
          );
          await loadChatMessages(
            activeRun.workspaceId,
            activeRun.chatId,
            session,
          );
          return;
        }
        console.debug(
          "[chat-stream] active run stream returned backend error",
          {
            chatId: activeRun.chatId,
            runId: activeRun.runId,
            status: response.status,
            workspaceId: activeRun.workspaceId,
          },
        );
        throw backendError;
      }

      const termination = await readChatStream(
        response,
        (streamEvent, meta) => {
          if (!ownsSession()) {
            return;
          }
          const eventSequence = meta.id === null ? Number.NaN : Number(meta.id);
          const isReplay =
            Number.isFinite(eventSequence) && eventSequence <= lastProcessedSequence;
          if (isReplay) {
            return;
          }
          updateLastProcessedSequence(
            Number.isFinite(eventSequence) ? eventSequence : null,
          );
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
            if (!startChatRun(chatKey, activeRun.runId)) {
              return;
            }
            const previousAssistantMessageId = currentAssistantMessageId;
            if (
              previousAssistantMessageId &&
              previousAssistantMessageId !== streamEvent.assistantMessageId
            ) {
              reportChatStreamOwnershipConflict("assistantIdentityMismatch", {
                canonicalAssistantMessageId: previousAssistantMessageId,
                chatId: activeRun.chatId,
                epoch: session.epoch,
                incomingAssistantMessageId: streamEvent.assistantMessageId,
                runId: activeRun.runId,
                workspaceId: activeRun.workspaceId,
              });
              void loadChatMessages(
                activeRun.workspaceId,
                activeRun.chatId,
                session,
              );
              return;
            }
            remapBufferedAssistantMessageId(
              previousAssistantMessageId,
              streamEvent.assistantMessageId,
            );
            assistantMessageId = streamEvent.assistantMessageId;
            currentAssistantMessageId = streamEvent.assistantMessageId;
            session.assistantMessageId = streamEvent.assistantMessageId;
            setMessagesForChatKey(chatKey, (current) => {
              const canonical = canonicalizeAssistantMessage(
                current,
                streamEvent.assistantMessageId,
                [previousAssistantMessageId, activeRun.assistantMessageId],
              );
              const existing = canonical.find(
                (message) =>
                  message.role === "assistant" &&
                  message.id === streamEvent.assistantMessageId,
              );
              const preserveHistory = shouldPreserveAssistantHistoryOnStart(
                hasSeenLiveStreamEventsForAssistant(
                  streamEvent.assistantMessageId,
                ),
              );
              if (!existing) {
                return canonical;
              }
              return canonical.map((message) =>
                message.role === "assistant" &&
                message.id === streamEvent.assistantMessageId
                  ? mergeAssistantMessageOnStreamStart(
                      message,
                      streamEvent.memoriesUsed,
                      preserveHistory,
                    )
                  : message,
              );
            });
            ensureStreamingAssistantMessage(
              streamEvent.assistantMessageId,
              streamEvent.memoriesUsed,
            );
            setChatRunFailed(chatKey, false);
            setChatRunning(chatKey, true);
            setActiveRunInfoForChatKey(chatKey, {
              acceptingGuidance: true,
              assistantMessageId: streamEvent.assistantMessageId,
              chatId: streamEvent.chatId,
              chatKey,
              lastSequence: lastSequenceForState(),
              modelId: runModelIdentity?.modelId,
              providerId: runModelIdentity?.providerId,
              queuedUserMessageId: activeRun.queuedUserMessageId ?? null,
              runId: activeRun.runId,
              workspaceId: activeRun.workspaceId,
            });
            liveStartedAtMs = Date.now();
            liveAssistantDraft = "";
            liveAssistantDraftReasoning = "";
            lastLiveContextUsageRefreshAtMs = Date.now();
            if (runModelIdentity) {
              updateLiveChatStatistics(chatKey, {
                ...runModelIdentity,
                startedAtMs: liveStartedAtMs,
                usage: null,
              });
            }
            refreshActiveAgentTeamSnapshot(
              activeRun.workspaceId,
              streamEvent.chatId,
            );
            return;
          }

          if (streamEvent.type === "textDelta") {
            markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
            markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
            const snapshotKey = resolvedAssistantMessageId(
              streamEvent.assistantMessageId,
            );
            streamAttemptSnapshots.set(
              snapshotKey,
              emptyStreamingAttemptSnapshot(),
            );
            ensureStreamingAssistantMessage(
              resolvedAssistantMessageId(streamEvent.assistantMessageId),
            );
            setMessagesForChatKey(chatKey, (current) => {
              const message = current.find((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                ),
              );
              if (message) {
                streamAttemptSnapshots.set(
                  snapshotKey,
                  streamingAttemptSnapshot(message),
                );
              }
              return current;
            });
            setActiveRunInfoForChatKey(chatKey, {
              acceptingGuidance: true,
              assistantMessageId: session.assistantMessageId,
              assistantSequence: activeRun.assistantSequence ?? null,
              chatId: activeRun.chatId,
              chatKey,
              lastSequence: lastSequenceForState(),
              modelId: runModelIdentity?.modelId,
              providerId: runModelIdentity?.providerId,
              queuedUserMessageId: activeRun.queuedUserMessageId ?? null,
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
            if (runModelIdentity) {
              updateLiveChatStatistics(chatKey, {
                ...runModelIdentity,
                startedAtMs: liveStartedAtMs,
                usage: null,
              });
            }
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
                  ? resetStreamingAssistantMessage(
                      message,
                      streamEvent,
                      streamAttemptSnapshots.get(
                        resolvedAssistantMessageId(
                          streamEvent.assistantMessageId,
                        ),
                      ),
                    )
                  : message,
              ),
            );
            return;
          }

          if (streamEvent.type === "contextCompression") {
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
                  ? assistantMessageWithContextCompression(message, streamEvent)
                  : message,
              ),
            );
            if (streamEvent.status === "completed") {
              refreshRunContextUsage();
            }
            return;
          }

          if (streamEvent.type === "agentTaskLifecycle") {
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                message.role === "assistant" &&
                message.id === streamEvent.assistantMessageId
                  ? {
                      ...message,
                      parts: upsertAgentTaskLifecyclePart(
                        message.parts,
                        streamEvent.lifecycle,
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
            deferStreamAuxiliaryUpdate(() => {
              if (!ownsSession()) {
                return;
              }
              if (runModelIdentity) {
                updateLiveChatStatistics(chatKey, {
                  ...runModelIdentity,
                  startedAtMs: liveStartedAtMs,
                  usage: latestResponseUsage,
                });
              }
            });
            return;
          }

          if (streamEvent.type === "guidanceApplied") {
            finishLiveReasoningDuration(currentAssistantMessageId);
            const previousAssistantId = currentAssistantMessageId;
            const guidanceAssistantId = `${streamEvent.id}-assistant`;
            currentAssistantMessageId = guidanceAssistantId;
            // Prefer durable interrupted id from the event so consecutive
            // recoveries keep remapping backend event ids to the newest bubble.
            interruptedAssistantMessageId =
              routingInterruptedAssistantMessageId(
                previousAssistantId,
                streamEvent.interruptedAssistantId,
              );
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
            const liveStatisticsUsage =
              streamEvent.usage &&
              streamEvent.usage.inputTokens !== null &&
              streamEvent.usage.outputTokens !== null
                ? streamEvent.usage
                : latestResponseUsage;
            if (!latestResponseUsage && liveStatisticsUsage) {
              latestResponseUsage = liveStatisticsUsage;
            }
            terminalContextUsageRefreshRequested = refreshRunContextUsage({
              modelId: streamEvent.metrics.modelId,
              providerId: streamEvent.metrics.providerId,
            });
            updateLiveChatStatistics(chatKey, {
              modelId: streamEvent.metrics.modelId,
              providerId: streamEvent.metrics.providerId,
              startedAtMs: liveStartedAtMs,
              usage: liveStatisticsUsage,
            });
            void loadChatStatistics(activeRun.workspaceId, activeRun.chatId);
            void refreshWorkspaces();
            setChatRunFailed(chatKey, false);
            // The terminal refresh uses the run's actual provider route. Keep
            // the composer effect from immediately replacing it with the
            // current composer route as this run becomes inactive.
            skipComposerContextUsageRefreshAfterRunByChatKeyRef.current.add(
              chatKey,
            );
            finishChatRun(
              chatKey,
              activeRun.runId,
              activeRun.workspaceId,
              activeRun.chatId,
            );
            setRetryRunRequest(null);
            setPendingQuestion(null);
            setQuestionError(null);
            setIsAnsweringQuestion(false);
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
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
            markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
            finishLiveReasoningDuration(
              streamEvent.assistantMessageId,
              streamEvent.reasoningDurationMs,
            );
            ensureStreamingAssistantMessage(
              resolvedAssistantMessageId(streamEvent.assistantMessageId),
            );
            const messageOwnsToolCall = (message: ShellMessage) =>
              messageHasToolCall(message, streamEvent.toolCall.id);
            setMessagesForChatKey(chatKey, (current) => {
              const updateExistingToolCall = current.some(messageOwnsToolCall);
              return current.map((message) =>
                (
                  updateExistingToolCall
                    ? messageOwnsToolCall(message)
                    : isCurrentAssistantMessage(
                        message,
                        streamEvent.assistantMessageId,
                      )
                )
                  ? {
                      ...message,
                      parts: upsertToolCallPart(
                        message.parts,
                        streamEvent.toolCall,
                      ),
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
            markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
            const messageOwnsToolCall = (message: ShellMessage) =>
              messageHasToolCall(message, streamEvent.toolCallId);
            setMessagesForChatKey(chatKey, (current) => {
              const updateExistingToolCall = current.some(messageOwnsToolCall);
              return current.map((message) =>
                (
                  updateExistingToolCall
                    ? messageOwnsToolCall(message)
                    : isCurrentAssistantMessage(
                        message,
                        streamEvent.assistantMessageId,
                      )
                )
                  ? {
                      ...message,
                      parts: applyToolResultToParts(
                        message.parts,
                        streamEvent.toolCallId,
                        streamEvent.output,
                        streamEvent.isError,
                        streamEvent.startedAt,
                        streamEvent.completedAt,
                        streamEvent.terminal !== false,
                      ),
                      toolCalls: applyToolResult(
                        message.toolCalls,
                        streamEvent.toolCallId,
                        streamEvent.output,
                        streamEvent.isError,
                        streamEvent.startedAt,
                        streamEvent.completedAt,
                        streamEvent.terminal !== false,
                      ),
                    }
                  : message,
              );
            });
            return;
          }

          if (streamEvent.type === "toolOutputDelta") {
            markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
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
            const sourceControlView = sourceControlViewRef.current;
            if (
              sourceControlView.isVisible &&
              sourceControlView.workspaceId === streamEvent.workspaceId &&
              sourceControlView.chatKey === chatKey
            ) {
              void loadGitDiff(
                streamEvent.workspaceId,
                sourceControlView.selectedDiffPath,
                sourceControlView.target,
              );
            }
            deferStreamAuxiliaryUpdate(() => {
              if (!ownsSession() || !runModelIdentity) {
                return;
              }
              updateLiveChatStatistics(chatKey, {
                codeChangeStats: streamEvent.codeChangeStats,
                ...runModelIdentity,
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
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
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
                isCurrentAssistantMessage(
                  message,
                  streamEvent.assistantMessageId,
                )
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
            finishStreamingAssistantMessage(currentAssistantMessageId);
            chatStreamHandoffsByChatKeyRef.current.set(chatKey, {
              chatId: activeRun.chatId,
              lastSequence: lastSequenceForState(),
              runId: activeRun.runId,
              workspaceId: activeRun.workspaceId,
            });
            streamEnded = true;
            finishChatRun(
              chatKey,
              activeRun.runId,
              activeRun.workspaceId,
              activeRun.chatId,
              { durable: false },
            );
            refreshTerminalContextUsage();
            refreshActiveAgentTeamSnapshot(
              activeRun.workspaceId,
              activeRun.chatId,
            );
            void refreshMessagesAfterSpecJobSettles(
              activeRun.workspaceId,
              activeRun.chatId,
              activeRun.runId,
            );
            return;
          }

          if (streamEvent.type === "error") {
            console.debug(
              "[chat-stream] active run stream emitted backend error event",
              {
                chatId: activeRun.chatId,
                message: streamEvent.message,
                runId: activeRun.runId,
                workspaceId: activeRun.workspaceId,
              },
            );
            finishLiveReasoningDuration();
            setChatRunFailed(chatKey, true);
            finishChatRun(
              chatKey,
              activeRun.runId,
              activeRun.workspaceId,
              activeRun.chatId,
            );
            setError(streamEvent.message);
            setPendingQuestion(null);
            setQuestionError(null);
            setIsAnsweringQuestion(false);
            setMessagesForChatKey(chatKey, (current) =>
              current.map((message) =>
                isCurrentAssistantMessage(message)
                  ? assistantMessageWithAppendedError(
                      message,
                      streamEvent.message,
                    )
                  : message,
              ),
            );
          }
        },
        { signal: abortController.signal },
      );

      if (termination === "eof" && ownsSession()) {
        flushStreamDeltaBuffers();
        const serverActiveRun = await loadChatMessages(
          activeRun.workspaceId,
          activeRun.chatId,
          session,
        );
        if (ownsSession()) {
          if (serverActiveRun === null) {
            finishStreamingAssistantMessage(currentAssistantMessageId);
            finishChatRun(
              chatKey,
              activeRun.runId,
              activeRun.workspaceId,
              activeRun.chatId,
            );
          } else {
            // A failed reconciliation is not proof of completion. Preserve the
            // streaming message and use the ordinary active-run reconnect path.
            shouldReconnect = true;
          }
        }
      }

      if (ownsSession()) {
        await refreshWorkspaces();
      }
    } catch (requestError) {
      if (!ownsSession()) {
        return;
      }
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      const wasCancelled =
        requestError instanceof DOMException &&
        requestError.name === "AbortError";
      if (isStreamIdleError(requestError)) {
        console.debug(
          "[chat-stream] active run stream idle timeout; reconnecting",
          {
            chatId: activeRun.chatId,
            lastSequence: lastSequenceForState(),
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          },
        );
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
        setRunError(requestError);
      }
    } finally {
      if (!ownsSession()) {
        return;
      }
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      if (!shouldReconnect) {
        refreshTerminalContextUsage();
      }
      if (activeRunAbortByChatKeyRef.current.get(chatKey) === abortController) {
        activeRunAbortByChatKeyRef.current.delete(chatKey);
        if (shouldReconnect) {
          releaseChatStreamSession(chatKey, session);
          setChatRunning(chatKey, true);
          setActiveRunInfoForChatKey(chatKey, {
            acceptingGuidance: activeRun.acceptingGuidance,
            assistantMessageId: session.assistantMessageId,
            assistantSequence: activeRun.assistantSequence ?? null,
            chatId: activeRun.chatId,
            chatKey,
            lastSequence: lastSequenceForState(),
            modelId: runModelIdentity?.modelId,
            providerId: runModelIdentity?.providerId,
            queuedUserMessageId: activeRun.queuedUserMessageId ?? null,
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          });
          void subscribeActiveChatRun(activeRunWithCurrentSequence(), true);
        } else if (
          streamEnded &&
          durableRunTerminationByChatKeyRef.current.get(chatKey)?.runId !==
            activeRun.runId
        ) {
          const handoff = {
            chatId: activeRun.chatId,
            lastSequence: lastSequenceForState(),
            runId: activeRun.runId,
            workspaceId: activeRun.workspaceId,
          };
          releaseChatStreamSession(chatKey, session);
          reconcileChatStreamHandoff(handoff);
        } else if (streamEnded) {
          // A complete/error event has already established this run as durable
          // terminal. Its trailing streamEnd only closes the SSE transport; it
          // must not re-enter handoff reconciliation and reload stale history.
          releaseChatStreamSession(chatKey, session);
          clearChatStreamHandoff(chatKey);
        } else {
          finishChatRun(
            chatKey,
            activeRun.runId,
            activeRun.workspaceId,
            activeRun.chatId,
          );
          releaseChatStreamSession(chatKey, session);
        }
      }
    }
  }

  async function runChatMessage(
    initialRequest: RetryRunRequest,
  ): Promise<string | null> {
    const requestModel = availableModels.find(
      (model) => model.id === initialRequest.modelId,
    );
    let request = {
      ...initialRequest,
      latencyMode: latencyModeForModel(requestModel),
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
        openPendingChatTab(
          request.workspaceId,
          queued.chatId,
          queued.chatTitle,
        );
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
    const localAssistantId =
      request.assistantMessageId ?? `local-assistant-${runKey}`;
    const localCreatedAt = new Date().toISOString();
    const visibleUserContent = messageWithSelectedSkills(
      availableSkills,
      request.skillIds,
      request.content,
    );
    const localUserParts = userMessageParts(
      visibleUserContent,
      request.attachments,
    );
    let assistantMessageId = localAssistantId;
    let currentAssistantMessageId = localAssistantId;
    // See subscribeActiveChatRun: post-guidance events keep carrying the durable
    // interrupted assistant id and must target the latest visible bubble.
    let interruptedAssistantMessageId: string | null = null;
    let requestChatId = request.chatId;
    const pendingChatId =
      request.chatId || request.localChatKey ? null : `pending:${runKey}`;
    let runMessagesKey =
      request.localChatKey ??
      (requestChatId
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
    let streamEnded = false;
    let hasGuidanceTurns = false;
    let activeRunId: string | null = null;
    let reconnectAfterEof: ActiveChatRunSummary | null = null;
    let terminalContextUsageRefreshRequested = false;
    const abortController = new AbortController();
    const session = claimChatStreamSession(
      runMessagesKey,
      null,
      abortController,
      { reuseSameRun: false },
    );
    if (!session) {
      throw new Error("failed to claim chat stream session");
    }
    session.assistantMessageId = localAssistantId;
    const ownsSession = () =>
      isCurrentChatStreamSession(runMessagesKey, session);
    const canFlushBufferedDelta = (
      _chatKey: string,
      assistantMessageId: string,
    ) => ownsSession() && Boolean(assistantMessageId);
    const textDeltaBuffer = createTextDeltaBuffer(
      () => ownsSession(),
      canFlushBufferedDelta,
    );
    const reasoningDeltaBuffer = createReasoningDeltaBuffer(() =>
      ownsSession(),
      canFlushBufferedDelta,
    );
    const toolOutputDeltaBuffer = createToolOutputDeltaBuffer(() =>
      ownsSession(),
      canFlushBufferedDelta,
    );
    const remapBufferedAssistantMessageId = (
      previousAssistantMessageId: string,
      canonicalAssistantMessageId: string,
    ) => {
      textDeltaBuffer.remapAssistantMessageId(
        runMessagesKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
      reasoningDeltaBuffer.remapAssistantMessageId(
        runMessagesKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
      toolOutputDeltaBuffer.remapAssistantMessageId(
        runMessagesKey,
        previousAssistantMessageId,
        canonicalAssistantMessageId,
      );
    };
    const flushStreamDeltaBuffers = () => {
      textDeltaBuffer.flush();
      reasoningDeltaBuffer.flush();
      toolOutputDeltaBuffer.flush();
    };
    const refreshRunContextUsage = (
      modelIdentity: { modelId: string; providerId: string } = {
        modelId: request.modelId,
        providerId: request.providerId,
      },
    ): boolean => {
      if (!ownsSession() || !requestChatId) {
        return false;
      }

      void refreshContextUsage({
        chatId: requestChatId,
        modelId: modelIdentity.modelId,
        providerId: modelIdentity.providerId,
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
      if (
        now - lastLiveContextUsageRefreshAtMs <
        LIVE_CONTEXT_USAGE_REFRESH_MS
      ) {
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
      !request.localChatKey ||
      activeChatKeyRef.current === request.localChatKey;

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
    setChatRunning(currentRunningChatKey, true);
    setActiveRunInfoForChatKey(currentRunningChatKey, {
      acceptingGuidance: false,
      chatId: requestChatId,
      chatKey: currentRunningChatKey,
      modelId: request.modelId,
      providerId: request.providerId,
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
      if (!ownsSession()) {
        return;
      }
      setMessagesForChatKey(runMessagesKey, (current) => {
        if (current.some((message) => message.id === nextAssistantMessageId)) {
          return current.map((message) =>
            message.id === nextAssistantMessageId &&
            message.role === "assistant"
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
    const finishStreamingAssistantMessage = (
      finishedAssistantMessageId: string,
    ) => {
      if (!ownsSession()) {
        return;
      }
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
    const markAssistantLiveStreamEvent = (eventAssistantMessageId?: string) => {
      const assistantId = resolvedAssistantMessageId(eventAssistantMessageId);
      const tracked =
        liveStreamAssistantIdsByChatKeyRef.current.get(runMessagesKey) ??
        new Set<string>();
      tracked.add(assistantId);
      liveStreamAssistantIdsByChatKeyRef.current.set(runMessagesKey, tracked);
    };
    let activeReasoningStartedAtMs: number | null = null;
    const streamAttemptSnapshots = new Map<string, StreamAttemptSnapshot>();
    const startLiveReasoningDuration = () => {
      if (activeReasoningStartedAtMs !== null) {
        return activeReasoningStartedAtMs;
      }
      const startedAtMs = Date.now();
      activeReasoningStartedAtMs = startedAtMs;
      return startedAtMs;
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
                ? finishActiveReasoningPart(
                    message.parts,
                    startedAtMs,
                    endedAtMs,
                  )
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
            latencyMode: request.latencyMode ?? "standard",
          }),
          cache: "no-store",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (!response.ok) {
        throw await responseError(response);
      }

      const termination = await readChatStream(response, (streamEvent) => {
        if (!ownsSession()) {
          return;
        }
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
          remapBufferedAssistantMessageId(
            previousAssistantMessageId,
            streamEvent.assistantMessageId,
          );
          assistantMessageId = streamEvent.assistantMessageId;
          currentAssistantMessageId = streamEvent.assistantMessageId;
          session.assistantMessageId = streamEvent.assistantMessageId;
          requestChatId = streamEvent.chatId;
          currentRunningChatKey = chatRunKey(
            request.workspaceId,
            streamEvent.chatId,
          );
          // `runId` is the stable active-run identity. `llmRequestId` remains
          // a legacy fallback for local streams that predate `runId`; provider
          // attempts never update this value after the start event.
          const startedRunId =
            activeRunIdFromStartEvent(streamEvent) ?? activeRunId;
          if (!startChatRun(currentRunningChatKey, startedRunId)) {
            return;
          }
          activeRunId = startedRunId;
          session.runId = startedRunId;
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
            if (
              chatStreamSessionsByChatKeyRef.current.get(runMessagesKey) ===
              session
            ) {
              const replacingSession =
                chatStreamSessionsByChatKeyRef.current.get(
                  currentRunningChatKey,
                );
              replacingSession?.abortController.abort();
              chatStreamSessionsByChatKeyRef.current.delete(runMessagesKey);
              chatStreamSessionsByChatKeyRef.current.set(
                currentRunningChatKey,
                session,
              );
            }
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
            const pendingLiveAssistantIds =
              liveStreamAssistantIdsByChatKeyRef.current.get(runMessagesKey);
            if (pendingLiveAssistantIds?.size) {
              const nextLiveIds =
                liveStreamAssistantIdsByChatKeyRef.current.get(
                  currentRunningChatKey,
                ) ?? new Set<string>();
              for (const id of pendingLiveAssistantIds) {
                nextLiveIds.add(
                  id === localAssistantId ? streamEvent.assistantMessageId : id,
                );
              }
              liveStreamAssistantIdsByChatKeyRef.current.set(
                currentRunningChatKey,
                nextLiveIds,
              );
              liveStreamAssistantIdsByChatKeyRef.current.delete(runMessagesKey);
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
            moveMessagesForChatKey(
              runMessagesKey,
              currentRunningChatKey,
              (current) =>
                canonicalizeAssistantMessage(
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
                  streamEvent.assistantMessageId,
                  [localAssistantId, previousAssistantMessageId],
                ),
            );

            runMessagesKey = currentRunningChatKey;
          } else {
            const liveIds = liveStreamAssistantIdsByChatKeyRef.current.get(
              currentRunningChatKey,
            );
            if (liveIds?.has(localAssistantId)) {
              liveIds.delete(localAssistantId);
              liveIds.add(streamEvent.assistantMessageId);
            }
            setMessagesForChatKey(currentRunningChatKey, (current) =>
              canonicalizeAssistantMessage(
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
                streamEvent.assistantMessageId,
                [localAssistantId, previousAssistantMessageId],
              ),
            );
          }
          ensureStreamingAssistantMessage(
            streamEvent.assistantMessageId,
            streamEvent.memoriesUsed,
          );
          setChatRunning(currentRunningChatKey, true);
          setActiveRunInfoForChatKey(currentRunningChatKey, {
            acceptingGuidance: activeRunId !== null,
            assistantMessageId: streamEvent.assistantMessageId,
            chatId: streamEvent.chatId,
            chatKey: currentRunningChatKey,
            modelId: request.modelId,
            providerId: request.providerId,
            queuedUserMessageId: request.queuedUserMessageId ?? null,
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
          refreshActiveAgentTeamSnapshot(
            request.workspaceId,
            streamEvent.chatId,
          );
          const shouldActivateStartedChat =
            shouldActivateRun ||
            activeChatKeyRef.current === currentRunningChatKey ||
            activeChatKeyRef.current === request.localChatKey ||
            activeChatKeyRef.current === null ||
            Boolean(request.chatId && !request.localChatKey);
          if (shouldActivateStartedChat) {
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
          markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
          markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
          const snapshotKey = resolvedAssistantMessageId(
            streamEvent.assistantMessageId,
          );
          streamAttemptSnapshots.set(
            snapshotKey,
            emptyStreamingAttemptSnapshot(),
          );
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(runMessagesKey, (current) => {
            const message = current.find((message) =>
              isCurrentAssistantMessage(
                message,
                streamEvent.assistantMessageId,
              ),
            );
            if (message) {
              streamAttemptSnapshots.set(
                snapshotKey,
                streamingAttemptSnapshot(message),
              );
            }
            return current;
          });
          setActiveRunInfoForChatKey(runMessagesKey, {
            acceptingGuidance: activeRunId !== null,
            assistantMessageId: session.assistantMessageId,
            chatId: requestChatId,
            chatKey: runMessagesKey,
            modelId: request.modelId,
            providerId: request.providerId,
            queuedUserMessageId: request.queuedUserMessageId ?? null,
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
                      resolvedAssistantMessageId(
                        streamEvent.assistantMessageId,
                      ),
                    ),
                  )
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "contextCompression") {
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
          return;
        }

        if (streamEvent.type === "agentTaskLifecycle") {
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              message.role === "assistant" &&
              message.id === streamEvent.assistantMessageId
                ? {
                    ...message,
                    parts: upsertAgentTaskLifecyclePart(
                      message.parts,
                      streamEvent.lifecycle,
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
          deferStreamAuxiliaryUpdate(() => {
            if (!ownsSession()) {
              return;
            }
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
          // Prefer durable interrupted id from the event so consecutive
          // recoveries keep remapping backend event ids to the newest bubble.
          interruptedAssistantMessageId = routingInterruptedAssistantMessageId(
            previousAssistantId,
            streamEvent.interruptedAssistantId,
          );
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
          terminalContextUsageRefreshRequested = refreshRunContextUsage({
            modelId: streamEvent.metrics.modelId,
            providerId: streamEvent.metrics.providerId,
          });
          updateLiveChatStatistics(runMessagesKey, {
            modelId: streamEvent.metrics.modelId,
            providerId: streamEvent.metrics.providerId,
            startedAtMs: liveStartedAtMs,
            usage: liveStatisticsUsage,
          });
          finishChatRun(
            runMessagesKey,
            activeRunId,
            request.workspaceId,
            requestChatId,
          );
          if (requestChatId) {
            void loadChatStatistics(request.workspaceId, requestChatId);
          }
          void refreshWorkspaces();
          setChatRunFailed(runMessagesKey, false);
          // The terminal refresh uses the run's actual provider route. Keep
          // the composer effect from immediately replacing it with the
          // current composer route as this run becomes inactive.
          skipComposerContextUsageRefreshAfterRunByChatKeyRef.current.add(
            runMessagesKey,
          );
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
          markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
          finishLiveReasoningDuration(
            streamEvent.assistantMessageId,
            streamEvent.reasoningDurationMs,
          );
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCall.id);
          setMessagesForChatKey(runMessagesKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (
                updateExistingToolCall
                  ? messageOwnsToolCall(message)
                  : isCurrentAssistantMessage(
                      message,
                      streamEvent.assistantMessageId,
                    )
              )
                ? {
                    ...message,
                    toolCalls: upsertToolCall(
                      message.toolCalls,
                      streamEvent.toolCall,
                    ),
                    parts: upsertToolCallPart(
                      message.parts,
                      streamEvent.toolCall,
                    ),
                  }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "toolResult") {
          markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
          ensureStreamingAssistantMessage(
            resolvedAssistantMessageId(streamEvent.assistantMessageId),
          );
          const messageOwnsToolCall = (message: ShellMessage) =>
            messageHasToolCall(message, streamEvent.toolCallId);
          setMessagesForChatKey(runMessagesKey, (current) => {
            const updateExistingToolCall = current.some(messageOwnsToolCall);
            return current.map((message) =>
              (
                updateExistingToolCall
                  ? messageOwnsToolCall(message)
                  : isCurrentAssistantMessage(
                      message,
                      streamEvent.assistantMessageId,
                    )
              )
                ? {
                    ...message,
                    toolCalls: applyToolResult(
                      message.toolCalls,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                      streamEvent.terminal !== false,
                    ),
                    parts: applyToolResultToParts(
                      message.parts,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                      streamEvent.startedAt,
                      streamEvent.completedAt,
                      streamEvent.terminal !== false,
                    ),
                  }
                : message,
            );
          });
          return;
        }

        if (streamEvent.type === "toolOutputDelta") {
          markAssistantLiveStreamEvent(streamEvent.assistantMessageId);
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
          const sourceControlView = sourceControlViewRef.current;
          if (
            sourceControlView.isVisible &&
            sourceControlView.workspaceId === streamEvent.workspaceId &&
            sourceControlView.chatKey === runMessagesKey
          ) {
            void loadGitDiff(
              streamEvent.workspaceId,
              sourceControlView.selectedDiffPath,
              sourceControlView.target,
            );
          }
          deferStreamAuxiliaryUpdate(() => {
            if (!ownsSession()) {
              return;
            }
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
          finishStreamingAssistantMessage(currentAssistantMessageId);
          if (requestChatId) {
            chatStreamHandoffsByChatKeyRef.current.set(currentRunningChatKey, {
              chatId: requestChatId,
              lastSequence: session.lastSequence,
              runId: activeRunId,
              workspaceId: request.workspaceId,
            });
          }
          streamEnded = true;
          finishChatRun(
            currentRunningChatKey,
            activeRunId,
            request.workspaceId,
            requestChatId,
            { durable: false },
          );
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
          streamHadError = true;
          setChatRunFailed(runMessagesKey, true);
          finishChatRun(
            currentRunningChatKey,
            activeRunId,
            request.workspaceId,
            requestChatId,
          );
          setError(streamEvent.message);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message)
                ? assistantMessageWithAppendedError(
                    message,
                    streamEvent.message,
                  )
                : message,
            ),
          );
        }
      });

      if (termination === "eof" && ownsSession() && requestChatId) {
        flushStreamDeltaBuffers();
        const serverActiveRun = await loadChatMessages(
          request.workspaceId,
          requestChatId,
          session,
        );
        if (ownsSession()) {
          if (serverActiveRun === null) {
            finishStreamingAssistantMessage(currentAssistantMessageId);
            finishChatRun(
              currentRunningChatKey,
              activeRunId,
              request.workspaceId,
              requestChatId,
            );
          } else {
            // The server still owns a run, or the reconciliation request could
            // not determine its state. Reattach instead of completing on EOF.
            reconnectAfterEof = serverActiveRun ?? activeRunSummaryFromInfo({
              acceptingGuidance: activeRunId !== null,
              assistantMessageId: session.assistantMessageId,
              chatId: requestChatId,
              chatKey: currentRunningChatKey,
              runId: activeRunId,
              workspaceId: request.workspaceId,
            });
          }
        }
      }

      if (ownsSession()) {
        await refreshWorkspaces();
        // streamEnd can suspend a Coordinator for durable handoff; only a
        // completed stream may advance this chat's queued request locally.
        runSucceeded =
          !streamHadError && !streamEnded && reconnectAfterEof === null;
      }
    } catch (requestError) {
      if (!ownsSession()) {
        return null;
      }
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      const wasCancelled =
        requestError instanceof DOMException &&
        requestError.name === "AbortError";
      const message = wasCancelled
        ? t("Run cancelled.")
        : errorMessage(requestError);
      if (!wasCancelled) {
        setChatRunFailed(runMessagesKey, true);
      }
      if (wasCancelled) {
        setError(message);
      } else {
        setRunError(requestError);
      }
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
      if (!ownsSession()) {
        return null;
      }
      flushStreamDeltaBuffers();
      finishLiveReasoningDuration();
      if (
        activeRunAbortByChatKeyRef.current.get(currentRunningChatKey) ===
        abortController
      ) {
        activeRunAbortByChatKeyRef.current.delete(currentRunningChatKey);
        if (reconnectAfterEof) {
          releaseChatStreamSession(currentRunningChatKey, session);
          setChatRunning(currentRunningChatKey, true);
          void subscribeActiveChatRun(reconnectAfterEof, true);
        } else if (
          streamEnded &&
          requestChatId &&
          durableRunTerminationByChatKeyRef.current.get(currentRunningChatKey)
            ?.runId !== activeRunId
        ) {
          const handoff = {
            chatId: requestChatId,
            lastSequence: session.lastSequence,
            runId: activeRunId,
            workspaceId: request.workspaceId,
          };
          releaseChatStreamSession(currentRunningChatKey, session);
          reconcileChatStreamHandoff(handoff);
        } else if (streamEnded) {
          // A complete/error event has already established this run as durable
          // terminal. Its trailing streamEnd only closes the SSE transport; it
          // must not re-enter handoff reconciliation and reload stale history.
          releaseChatStreamSession(currentRunningChatKey, session);
          clearChatStreamHandoff(currentRunningChatKey);
        } else {
          refreshTerminalContextUsage();
          finishChatRun(
            currentRunningChatKey,
            activeRunId,
            request.workspaceId,
            requestChatId,
          );
          releaseChatStreamSession(currentRunningChatKey, session);
        }
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
      const params = new URLSearchParams({
        limit: String(WORKSPACE_CHAT_HISTORY_PAGE_SIZE),
      });
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
          const existingChatIds = new Set(
            workspace.chats.map((chat) => chat.id),
          );
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
      const data = await requestJson<SettingsResponse>(
        "/api/workspaces/order",
        {
          body: JSON.stringify({ workspaceIds }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      settingsSkillsSnapshotRef.current = JSON.stringify(data.skills);
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
    if (
      !sourceWorkspace ||
      !targetWorkspace ||
      sourceWorkspace.pinned !== targetWorkspace.pinned
    ) {
      return;
    }

    event.preventDefault();
    const workspaceIds = moveItemId(
      workspaceOrderPreviewRef.current ??
        workspaces.map((workspace) => workspace.id),
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

    if (
      !workspaceIds ||
      sameStringList(
        workspaceIds,
        previousWorkspaces.map((workspace) => workspace.id),
      )
    ) {
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
    setWorkspaceCodeGraphEnabled(false);
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
    const saveRevision = themeSaveRevisionRef.current + 1;
    themeSaveRevisionRef.current = saveRevision;
    setSettings((current) =>
      current
        ? { ...current, general: { ...current.general, theme: nextTheme } }
        : current,
    );
    setError(null);

    const saveRequest = themeSaveQueueRef.current.then(() =>
      requestJson<SettingsResponse>("/api/settings/general", {
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
      }),
    );
    themeSaveQueueRef.current = saveRequest.then(
      () => undefined,
      () => undefined,
    );

    try {
      const data = await saveRequest;
      if (themeSaveRevisionRef.current !== saveRevision) {
        return;
      }
      settingsSkillsSnapshotRef.current = JSON.stringify(data.skills);
      setSettings(data);
    } catch (requestError) {
      if (themeSaveRevisionRef.current !== saveRevision) {
        return;
      }
      setError(errorMessage(requestError));
      setSettings((current) =>
        current
          ? {
              ...current,
              general: { ...current.general, theme: previousTheme },
            }
          : current,
      );
    }
  }

  const handleSettingsPanelSettingsChange = useCallback(
    (data: SettingsResponse) => {
      const skillsSnapshot = JSON.stringify(data.skills);
      const skillsChanged = settingsSkillsSnapshotRef.current !== skillsSnapshot;
      settingsSkillsSnapshotRef.current = skillsSnapshot;
      const modelsSnapshot = JSON.stringify(data.configuredModels);
      const modelsChanged = settingsModelsSnapshotRef.current !== modelsSnapshot;
      settingsModelsSnapshotRef.current = modelsSnapshot;
      setSettings(data);
      setUpdateStatus(data.update);
      setIsTeamModeEnabled(data.general.defaultTeamModeEnabled);
      // Skill install/update/refresh mutates settings.skills; re-fetch the
      // workspace menu catalog so slash menu matches the effective set.
      // Soft reload keeps the last good catalog until the new response arrives.
      // Skip when skills are unchanged (e.g. Settings panel mount GET) so we
      // do not form a tight setState → request → setState loop under mocks.
      if (skillsChanged) {
        reloadWorkspaceSkillCatalog();
      }
      // Image-output model install can materialize built-in agent definitions
      // server-side. Reload only when configured models actually change — not
      // on every Settings open / non-model save.
      if (modelsChanged) {
        void loadAgentDefinitions();
      }
    },
    [loadAgentDefinitions, reloadWorkspaceSkillCatalog],
  );

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
  const handleGuideQueuedMessageForChatPanel = useStableCallback(
    (messageId: string) => void handleGuideQueuedMessage(messageId),
  );
  const handleSelectDraftAttachmentsForChatPanel = useStableCallback(() =>
    handleSelectDraftAttachments(),
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
    (event: FormEvent<HTMLFormElement>, options?: { schedule?: boolean }) =>
      void handleSendMessage(event, options),
  );
  const handleModelChangeForChatPanel = useStableCallback(
    handleChatModelChange,
  );
  const handleRemoveAttachmentForChatPanel = useStableCallback(
    handleRemoveDraftAttachment,
  );
  const handleRemoveSkillForChatPanel = useStableCallback(removeSelectedSkill);
  const handleThinkingLevelChangeForChatPanel = useStableCallback(
    handleChatThinkingLevelChange,
  );
  const handleToggleSkillForChatPanel = useStableCallback(toggleSelectedSkill);
  const handleWithdrawQueuedMessageForChatPanel = useStableCallback(
    handleWithdrawQueuedMessage,
  );
  const providersForChatPanel =
    settings?.providers ?? EMPTY_CONFIGURED_PROVIDERS;
  const refreshAgentPanelForContextPanel = useStableCallback(async () => {
    if (activeWorkspaceId && activeChatId && !isPendingChatId(activeChatId)) {
      await loadAgentTeamSnapshot(activeWorkspaceId, activeChatId, {
        silent: false,
      });
    }
  });
  const openAgentInstanceTabForContextPanel =
    useStableCallback(openAgentInstanceTab);
  const agentsPanelForContextPanel = useMemo(
    () => (
      <Suspense fallback={<PanelLoadingFallback />}>
        <AgentsRuntimePanel
          activeChatId={
            activeChatId && !isPendingChatId(activeChatId) ? activeChatId : null
          }
          error={agentTeamError}
          isLoading={isLoadingAgentTeam}
          onRefresh={refreshAgentPanelForContextPanel}
          onSelectInstance={openAgentInstanceTabForContextPanel}
          selectedInstanceId={
            activeMainTab.type === "agent"
              ? activeMainTab.instanceId
              : (agentTeamSnapshot?.team.coordinatorInstanceId ?? null)
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
  const handleSourceControlTargetChange = useStableCallback(
    (targetKey: string) => {
      if (targetKey === sourceControlTargetKeyValue) {
        return;
      }
      const target = sourceControlTargetFromKey(
        availableSourceControlTargets,
        targetKey,
      );
      if (!target) {
        return;
      }
      invalidateGitDiffRequest();
      gitOperationRequestIdRef.current += 1;
      sourceControlTargetIdentityRef.current = [
        activeWorkspace?.id ?? "",
        sourceControlTargetKey(target),
      ].join("\u0000");
      sourceControlViewRef.current = {
        ...sourceControlViewRef.current,
        selectedDiffPath: null,
        target,
      };
      setGitDiff(null);
      setDiffError(null);
      setIsLoadingDiff(false);
      setGitOperationKey(null);
      setGitCommitMessage("");
      setIsSourceControlTargetManual(true);
      setSelectedSourceControlTargetScope(sourceControlTargetScope);
      setSelectedSourceControlTarget(target);
      setSelectedDiffPath(null);
    },
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
        positioned: false,
        top: event.clientY,
        workspacePath: activeWorkspace.path,
      });
    },
  );
  const handleRefreshDiffForContextPanel = useStableCallback(() => {
    if (activeWorkspace?.id) {
      void loadGitDiff(
        activeWorkspace.id,
        selectedDiffPath,
        sourceControlTarget,
      );
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
  const handleContextPanelTabChange = useStableCallback(
    (tab: ContextPanelTab) => {
      setContextPanelTab(tab);
      setIsContextPanelOpen(true);
    },
  );
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
          label: isInstallingUpdate
            ? t("Installing update…")
            : t("Install update"),
          onClick: () => void installUpdateFromNav(),
        }
      : null;

  if (isCheckingAuth) {
    return (
      <I18nContext.Provider value={{ language, t }}>
        <main className="app-root grid place-items-center bg-[var(--surface-secondary)] text-[var(--foreground)]">
          <LoaderCircle
            aria-hidden="true"
            className="size-6 animate-spin text-[var(--accent-soft-foreground)]"
          />
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
            <div className="app-error-toast-message">
              <div>{error}</div>
              {errorDiagnosticReference ? (
                <div
                  aria-label={t(
                    "Diagnostic reference {diagnosticId}; operation {operation}; phase {phase}",
                    {
                      diagnosticId: errorDiagnosticReference.diagnosticId,
                      operation:
                        errorDiagnosticReference.operation ?? t("Unavailable"),
                      phase: errorDiagnosticReference.phase ?? t("Unavailable"),
                    },
                  )}
                  className="mt-2 flex flex-wrap items-center gap-1.5 text-[11px] font-medium"
                >
                  <span className="font-mono">
                    {t("Diagnostic reference: {diagnosticId}", {
                      diagnosticId: errorDiagnosticReference.diagnosticId,
                    })}
                  </span>
                  <Button
                    aria-label={t("Copy diagnostic reference")}
                    className="h-5 min-h-5 px-1 text-[11px]"
                    onPress={() =>
                      void copyDiagnosticReference(
                        errorDiagnosticReference.diagnosticId,
                      )
                    }
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <Copy aria-hidden="true" className="size-3" />
                  </Button>
                </div>
              ) : null}
            </div>
            <Button
              aria-label={t("Close error message")}
              className="app-error-toast-close"
              onPress={() => setError(null)}
              type="button"
              variant="ghost"
            >
              <X aria-hidden="true" className="size-4" />
            </Button>
          </section>
        ) : null}
        {updateInstallNotice ? (
          <section
            aria-live="polite"
            className="app-status-toast"
            role="status"
          >
            <CheckCircle2
              aria-hidden="true"
              className="app-status-toast-icon"
            />
            <div className="app-error-toast-message">{updateInstallNotice}</div>
            <Button
              aria-label={t("Dismiss update message")}
              className="app-status-toast-close"
              onPress={() => setUpdateInstallNotice(null)}
              type="button"
              variant="ghost"
            >
              <X aria-hidden="true" className="size-4" />
            </Button>
          </section>
        ) : null}
        {isGlobalView ? (
          <div className="global-shell">
            <FocoNavRail
              activeMode={viewMode}
              canLogout={canLogout}
              contextPanelButton={null}
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
                    activeWorkspaceId={
                      activeWorkspace?.id ?? activeWorkspaceId ?? null
                    }
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
            className={`app-shell ${showContextPanel ? "app-shell-with-context" : ""} ${
              isWorkspaceSidebarOpen ? "" : "app-shell-workspace-closed"
            }`}
            ref={appShellRef}
            style={
              {
                "--diff-panel-width": `${diffPanelWidth}px`,
                "--context-panel-min-height": `${CONTEXT_PANEL_MIN_HEIGHT}px`,
                "--context-panel-mobile-height": `${contextPanelMobileHeight}px`,
                "--sidebar-max-width": `${workspaceSidebarMaxWidth}px`,
                "--sidebar-width": `${sidebarWidth}px`,
              } as CSSProperties
            }
          >
            {isMobileWorkspaceOpen ? (
              <Button
                aria-label={t("Close")}
                className="mobile-sidebar-backdrop"
                onPress={() => setIsMobileWorkspaceOpen(false)}
                type="button"
                variant="ghost"
              />
            ) : null}
            <FocoNavRail
              activeMode={viewMode}
              canLogout={canLogout}
              onAddWorkspace={openWorkspaceDialog}
              contextPanelButton={{
                active: isContextPanelOpen,
                icon: ResponsiveContextPanelIcon,
                label: isContextPanelOpen
                  ? t("Close context panel")
                  : t("Open context panel"),
                onClick: () => setIsContextPanelOpen((current) => !current),
                selection: "toggle",
              }}
              terminalButton={{
                active: isTerminalOpen,
                disabled: !activeWorkspace,
                icon: SquareTerminal,
                label: isTerminalOpen
                  ? t("Close terminal")
                  : t("Open terminal"),
                onClick: toggleWorkspaceTerminal,
                selection: "toggle",
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
              className={`workspace-sidebar relative border-[color-mix(in_oklab,var(--border)_80%,transparent)] lg:border-r ${
                isMobileWorkspaceOpen ? "workspace-sidebar-mobile-open" : ""
              }`}
              ref={workspaceSidebarRef}
            >
              <div
                aria-label={t("Resize workspace sidebar")}
                aria-orientation="vertical"
                aria-valuemax={workspaceSidebarMaxWidth}
                aria-valuemin={workspaceSidebarMinWidth}
                aria-valuenow={Math.min(sidebarWidth, workspaceSidebarMaxWidth)}
                className={`workspace-sidebar-splitter cursor-col-resize ${
                  isResizingSidebar ? "workspace-sidebar-splitter-active" : ""
                }`}
                onKeyDown={(event) => {
                  if (event.key === "ArrowLeft") {
                    event.preventDefault();
                    setSidebarWidth((current) =>
                      clampWorkspaceSidebarWidth(current - 24),
                    );
                  }

                  if (event.key === "ArrowRight") {
                    event.preventDefault();
                    setSidebarWidth((current) =>
                      clampWorkspaceSidebarWidth(current + 24),
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
                <div className="workspace-sidebar-header flex items-center justify-between gap-2 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-4 py-2">
                  <div className="min-w-0">
                    <span className="workspace-sidebar-title">
                      {t("Workspaces")}
                    </span>
                  </div>
                  <div className="flex shrink-0 items-center gap-1.5">
                    <Button
                      aria-label={t("Refresh workspaces")}
                      isIconOnly
                      isDisabled={isLoading}
                      onPress={() => void refreshWorkspaces()}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      <RefreshCw
                        aria-hidden="true"
                        className={`size-3.5 ${isLoading ? "animate-spin" : ""}`}
                      />
                    </Button>
                    <Button
                      aria-label={t("Search chats")}
                      aria-pressed={workspaceChatSearchOpen}
                      isIconOnly
                      onPress={() =>
                        setWorkspaceChatSearchOpen((current) => !current)
                      }
                      size="sm"
                      type="button"
                      variant={workspaceChatSearchOpen ? "tertiary" : "ghost"}
                    >
                      <Search aria-hidden="true" className="size-3.5" />
                    </Button>
                    <Button
                      aria-label={t("Close")}
                      className="mobile-sidebar-close"
                      isIconOnly
                      onPress={() => setIsMobileWorkspaceOpen(false)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      <X aria-hidden="true" className="size-4" />
                    </Button>
                  </div>
                </div>

                {workspaceChatSearchOpen ? (
                  <div className="border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-3 py-2">
                    <div className="relative">
                      <TextField aria-label={t("Search chats")} className="contents">
                        <Input
                          className="workspace-chat-search-input h-9 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 pr-8 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
                          onChange={(event) =>
                            setWorkspaceChatSearchQuery(event.target.value)
                          }
                          placeholder={t("Search chats placeholder")}
                          type="search"
                          value={workspaceChatSearchQuery}
                        />
                      </TextField>
                      {workspaceChatSearchQuery.length ? (
                        <Button
                          aria-label={t("Clear search")}
                          className="absolute right-2 top-1/2 inline-flex size-5 -translate-y-1/2 items-center justify-center rounded-full text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--muted)]"
                          onPress={() => setWorkspaceChatSearchQuery("")}
                          type="button"
                          variant="ghost"
                        >
                          <X aria-hidden="true" className="size-3.5" />
                        </Button>
                      ) : null}
                    </div>
                  </div>
                ) : null}

                <nav
                  aria-label={t("Workspace list")}
                  className="workspace-nav panel-scroll min-h-0 flex-1 overflow-y-auto"
                >
                  {sidebarWorkspaces.length ? (
                    sidebarWorkspaces.map((workspace) => {
                      const isActive = workspace.id === activeWorkspace?.id;
                      const isExpanded =
                        isWorkspaceSearchActive ||
                        expandedWorkspaceId === workspace.id;
                      const workspaceChats = isWorkspaceSearchActive
                        ? workspace.chats.map(
                            (chat): WorkspaceChatListItem => ({
                              ...chat,
                              scheduledStatus:
                                chat.queuedRun?.status === "queued"
                                  ? "queued"
                                  : undefined,
                            }),
                          )
                        : workspaceChatListItemsFor(workspace);
                      const paging = workspaceChatPaging[workspace.id];
                      const visibleChats = workspaceChats;
                      const hiddenChatCount = isWorkspaceSearchActive
                        ? 0
                        : Math.max(
                            (paging?.total ?? workspace.chats.length) -
                              workspace.chats.length,
                            0,
                          );
                      const nextVisibleChatCount = Math.min(
                        WORKSPACE_CHAT_HISTORY_PAGE_SIZE,
                        hiddenChatCount,
                      );
                      const isRemoteWorkspace = workspace.serverId !== null;
                      const isRemoteReady = workspaceConnectionLooksReady(
                        workspace.connectionStatus,
                      );

                      return (
                        <div
                          className={`${draggedWorkspaceId === workspace.id ? "opacity-80" : ""}`}
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
                          <Accordion
                            className="workspace-accordion"
                            expandedKeys={isExpanded ? [workspace.id] : []}
                            hideSeparator
                            onExpandedChange={(keys) => {
                              if (isWorkspaceSearchActive) {
                                return;
                              }

                              setExpandedWorkspaceId(
                                keys.has(workspace.id) ? workspace.id : null,
                              );
                            }}
                          >
                            <Accordion.Item id={workspace.id}>
                              <Accordion.Heading className="workspace-accordion-heading">
                                <Accordion.Trigger
                                  className={workspaceItemClass(isActive)}
                                >
                                  <span className="relative mr-3 inline-flex shrink-0">
                                    <WorkspaceIcon
                                      className="size-4 shrink-0 rounded object-cover"
                                      fallbackClassName="size-4 shrink-0"
                                      isRemote={isRemoteWorkspace}
                                      logoUrl={workspace.logoUrl}
                                    />
                                    {isRemoteWorkspace ? (
                                      <span
                                        className={`absolute -bottom-0.5 -right-0.5 size-2 rounded-full border border-white ${workspaceConnectionDotClass(workspace.connectionStatus)}`}
                                      />
                                    ) : null}
                                  </span>
                                  <span className="min-w-0 flex-1 text-left">
                                    <span className="block truncate">
                                      {workspace.name}
                                    </span>
                                    <span className="block truncate text-[9px] font-medium leading-3 text-[color-mix(in_oklab,var(--muted)_70%,transparent)]">
                                      {workspace.displayPath}
                                    </span>
                                  </span>
                                  <Accordion.Indicator>
                                    <ChevronDown
                                      aria-hidden="true"
                                      className="workspace-expand-icon"
                                    />
                                  </Accordion.Indicator>
                                </Accordion.Trigger>
                                <Button
                                  aria-label={t("New chat in {name}", {
                                    name: workspace.name,
                                  })}
                                  className={workspaceNewChatButtonClass(
                                    isActive,
                                  )}
                                  isDisabled={
                                    isRemoteWorkspace && !isRemoteReady
                                  }
                                  onPress={() => {
                                    if (
                                      isRemoteWorkspace &&
                                      !isRemoteReady
                                    ) {
                                      setError(
                                        t(
                                          "Remote workspace is offline. Retry the connection before opening remote operations.",
                                        ),
                                      );
                                      return;
                                    }
                                    startNewWorkspaceChat(workspace.id);
                                  }}
                                  type="button"
                                  variant="ghost"
                                >
                                  <Plus
                                    aria-hidden="true"
                                    className="size-4"
                                  />
                                </Button>
                              </Accordion.Heading>
                              {isRemoteWorkspace && !isRemoteReady ? (
                                <div className="ml-9 mt-1 flex items-center gap-2 pr-1.5 text-[11px] leading-4 text-[var(--muted)]">
                                  {workspace.lastRemoteError ? (
                                    <span className="min-w-0 flex-1 truncate">
                                      {workspace.lastRemoteError}
                                    </span>
                                  ) : null}
                                  <Button
                                    className="inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-[var(--border)] bg-[var(--surface)] px-2 font-semibold text-[var(--accent-soft-foreground)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                                    isDisabled={
                                      retryingRemoteWorkspaceId ===
                                      workspace.id
                                    }
                                    onPress={() =>
                                      void retryRemoteWorkspace(workspace)
                                    }
                                    type="button"
                                    variant="ghost"
                                  >
                                    {retryingRemoteWorkspaceId ===
                                    workspace.id ? (
                                      <LoaderCircle
                                        aria-hidden="true"
                                        className="size-3 animate-spin"
                                      />
                                    ) : (
                                      <RefreshCw
                                        aria-hidden="true"
                                        className="size-3"
                                      />
                                    )}
                                    {t("Retry")}
                                  </Button>
                                </div>
                              ) : null}
                              <Accordion.Panel>
                                <Accordion.Body className="workspace-accordion-body">
                                  {isExpanded ? (
                                  <div className="mt-1 space-y-1 border-l border-[color-mix(in_oklab,var(--border)_80%,transparent)] pl-3 pr-1.5">
                                    {visibleChats.length > 0 ? (
                                      <>
                                        {visibleChats.map((chat) => {
                                          const chatKey = chatRunKey(
                                            workspace.id,
                                            chat.id,
                                          );
                                          const scheduledChatKey =
                                            chat.scheduledChatKey ?? null;
                                          const sessionStatus =
                                            chatSessionStatusFor(
                                              chatKey,
                                              {
                                                scheduledChatKey,
                                                scheduledStatus:
                                                  chat.scheduledStatus ?? null,
                                                workspaceActiveRun:
                                                  chat.activeRun,
                                              },
                                            );
                                          const statusDotClass =
                                            chatSessionStatusDotClass(
                                              sessionStatus.kind,
                                            );
                                          const isChatActive =
                                            activeWorkspace?.id ===
                                              workspace.id &&
                                            activeChatId === chat.id;
                                          const chatDiffStats =
                                            chat.codeChangeStats;

                                          return (
                                            <Button
                                              aria-current={
                                                isChatActive
                                                  ? "page"
                                                  : undefined
                                              }
                                              className={chatItemClass()}
                                              key={chat.id}
                                              // Long-press and context-menu state need their pointer handlers below.
                                              onClick={() => {
                                                if (
                                                  suppressNextWorkspaceChatClickRef.current
                                                ) {
                                                  suppressNextWorkspaceChatClickRef.current = false;
                                                  return;
                                                }

                                                selectWorkspaceChat(
                                                  workspace.id,
                                                  chat.id,
                                                );
                                              }}
                                              onContextMenu={(event) =>
                                                openWorkspaceChatContextMenu(
                                                  event,
                                                  workspace,
                                                  chat,
                                                )
                                              }
                                              onPointerCancel={
                                                cancelWorkspaceChatLongPress
                                              }
                                              onPointerDown={(event) =>
                                                startWorkspaceChatLongPress(
                                                  event,
                                                  workspace,
                                                  chat,
                                                )
                                              }
                                              onPointerLeave={
                                                cancelWorkspaceChatLongPress
                                              }
                                              onPointerUp={
                                                cancelWorkspaceChatLongPress
                                              }
                                              type="button"
                                              variant={
                                                isChatActive
                                                  ? "tertiary"
                                                  : "ghost"
                                              }
                                            >
                                              <span
                                                aria-hidden="true"
                                                className={`session-status-dot ${statusDotClass}`}
                                              />
                                              <span className="min-w-0 flex-1">
                                                <span className="block truncate">
                                                  {chat.title}
                                                </span>
                                                <span className="mt-0.5 flex min-w-0 items-center justify-between gap-2 text-[9px] font-normal leading-tight">
                                                  <span className="min-w-0 truncate text-[color-mix(in_oklab,var(--muted)_55%,transparent)]">
                                                    {formatChatCreatedAt(
                                                      chat.createdAt,
                                                    )}
                                                  </span>
                                                  {chatDiffStats &&
                                                  hasGitDiffStats(
                                                    chatDiffStats,
                                                  ) ? (
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
                                                        +
                                                        {
                                                          chatDiffStats.additions
                                                        }
                                                      </span>
                                                      <span className="chat-diff-delete">
                                                        -
                                                        {
                                                          chatDiffStats.deletions
                                                        }
                                                      </span>
                                                    </span>
                                                  ) : null}
                                                </span>
                                              </span>
                                            </Button>
                                          );
                                        })}
                                        {hiddenChatCount > 0 ? (
                                          <Button
                                            aria-label={t(
                                              "Show {count} more chats in {name}",
                                              {
                                                count: nextVisibleChatCount,
                                                name: workspace.name,
                                              },
                                            )}
                                            className="workspace-show-more-chats flex min-h-10 min-w-0 w-full items-center gap-2 rounded-lg border border-transparent px-2 py-1.5 text-left text-[10px] font-medium text-[var(--muted)] hover:border-[var(--border)] hover:bg-[color-mix(in_oklab,var(--surface)_80%,transparent)] hover:text-[var(--foreground)]"
                                            isDisabled={paging?.isLoading}
                                            onPress={() =>
                                              void showMoreWorkspaceChats(
                                                workspace.id,
                                              )
                                            }
                                            type="button"
                                            variant="ghost"
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
                                              <span className="mt-0.5 block truncate text-[9px] font-normal leading-tight text-[var(--muted)]">
                                                {t("{count} hidden chats", {
                                                  count: hiddenChatCount,
                                                })}
                                              </span>
                                            </span>
                                          </Button>
                                        ) : null}
                                      </>
                                    ) : (
                                      <div className="rounded-lg px-2 py-1.5 text-xs text-[var(--muted)]">
                                        {t("No chats")}
                                      </div>
                                    )}
                                  </div>
                                  ) : null}
                                </Accordion.Body>
                              </Accordion.Panel>
                            </Accordion.Item>
                          </Accordion>
                        </div>
                      );
                    })
                  ) : (
                    <div className="mx-2 rounded-lg border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_60%,transparent)] px-3 py-4 text-sm text-[var(--muted)]">
                      {isWorkspaceSearchActive
                        ? isSearchingWorkspaceChats
                          ? t("Searching chats…")
                          : (workspaceChatSearchError ?? t("No matching chats"))
                        : isLoading
                          ? t("Loading workspaces…")
                          : t("No workspaces")}
                    </div>
                  )}
                </nav>
                <ModelRoutingPanel
                  models={settings?.configuredModels ?? []}
                  onFastModeChange={updateModelFastMode}
                  onRouteChange={updateModelRoute}
                  providers={settings?.providers ?? EMPTY_CONFIGURED_PROVIDERS}
                />
              </div>
            </aside>

            {workspaceChatContextMenu ? (
              <ContextMenu
                aria-label={workspaceChatContextMenu.chat.title}
                isOpen
                items={[
                  {
                    danger: true,
                    disabled: Boolean(
                      workspaceChatContextMenu.chat.scheduledRunId,
                    ),
                    icon: <Trash2 aria-hidden="true" className="size-3.5" />,
                    id: "delete-chat",
                    label: t("Delete chat"),
                  },
                ]}
                left={workspaceChatContextMenu.left}
                top={workspaceChatContextMenu.top}
                onAction={(key) => {
                  if (key !== "delete-chat") {
                    return;
                  }
                  const { chat, workspace } = workspaceChatContextMenu;
                  setWorkspaceChatContextMenu(null);
                  requestDeleteWorkspaceChat(workspace, chat);
                }}
                onOpenChange={(open) => {
                  if (!open) {
                    setWorkspaceChatContextMenu(null);
                  }
                }}
              />
            ) : null}

            {workspaceFileContextMenu ? (
              <ContextMenu
                aria-label={workspaceFileContextMenu.node.name}
                className="workspace-file-context-menu"
                isOpen
                items={[
                  {
                    icon: <FileText aria-hidden="true" className="size-3.5" />,
                    id: "open",
                    label: t("Open"),
                  },
                  ...(
                    workspaceFileContextMenu.node.kind === "file" &&
                    isHtmlFilePath(workspaceFileContextMenu.node.path)
                      ? [
                          {
                            icon: (
                              <AppWindow
                                aria-hidden="true"
                                className="size-3.5"
                              />
                            ),
                            id: "preview",
                            label: t("Preview in new tab"),
                          },
                        ]
                      : []
                  ),
                  ...(
                    workspaceFileContextMenu.node.kind === "file"
                      ? [
                          {
                            icon: (
                              <Download
                                aria-hidden="true"
                                className="size-3.5"
                              />
                            ),
                            id: "download",
                            label: t("Download"),
                          },
                        ]
                      : []
                  ),
                  {
                    icon: <Pencil aria-hidden="true" className="size-3.5" />,
                    id: "rename",
                    label: t("Rename"),
                  },
                  {
                    danger: true,
                    icon: <Trash2 aria-hidden="true" className="size-3.5" />,
                    id: "delete",
                    label: t("Delete"),
                  },
                  {
                    icon: <Copy aria-hidden="true" className="size-3.5" />,
                    id: "copy-name",
                    label: t("Copy file name"),
                  },
                  {
                    icon: <Copy aria-hidden="true" className="size-3.5" />,
                    id: "copy-relative",
                    label: t("Copy relative path"),
                  },
                  {
                    icon: <Copy aria-hidden="true" className="size-3.5" />,
                    id: "copy-absolute",
                    label: t("Copy absolute path"),
                  },
                ]}
                left={workspaceFileContextMenu.left}
                positioned={workspaceFileContextMenu.positioned}
                top={workspaceFileContextMenu.top}
                onAction={(key) => {
                  const { node, workspacePath } = workspaceFileContextMenu;
                  setWorkspaceFileContextMenu(null);
                  switch (String(key)) {
                    case "open":
                      if (node.kind === "directory") {
                        void toggleWorkspaceFileTreePath(node);
                      } else {
                        void openWorkspaceFileTab(node);
                      }
                      break;
                    case "preview":
                      if (activeWorkspace) {
                        openWorkspaceHtmlPreviewTab({
                          name: node.name,
                          path: node.path,
                          workspaceId: activeWorkspace.id,
                          workspaceLogoUrl: activeWorkspace.logoUrl ?? null,
                          workspaceName: activeWorkspace.name,
                        });
                      }
                      break;
                    case "download":
                      downloadWorkspaceFile(node);
                      break;
                    case "rename": {
                      const nextName = window.prompt(t("Rename file"), node.name);
                      if (nextName === null) {
                        break;
                      }
                      const trimmedName = nextName.trim();
                      if (!trimmedName || trimmedName === node.name) {
                        break;
                      }
                      void handleWorkspaceFileOperation(
                        "rename",
                        node.path,
                        trimmedName,
                      );
                      break;
                    }
                    case "delete":
                      if (
                        !window.confirm(
                          t("Delete this file or folder?\n\nPath: {path}", {
                            path: node.path,
                          }),
                        )
                      ) {
                        break;
                      }
                      void handleWorkspaceFileOperation("delete", node.path);
                      break;
                    case "copy-name":
                      void copyWorkspaceFileText(node.name);
                      break;
                    case "copy-relative":
                      void copyWorkspaceFileText(node.path);
                      break;
                    case "copy-absolute":
                      void copyWorkspaceFileText(
                        workspaceFileAbsolutePath(workspacePath, node.path),
                      );
                      break;
                    default:
                      break;
                  }
                }}
                onOpenChange={(open) => {
                  if (!open) {
                    workspaceFileContextMenuRef.current = null;
                    setWorkspaceFileContextMenu(null);
                  }
                }}
              />
            ) : null}

            <section className="app-main-panel flex min-w-0 flex-col">
              <header className="app-toolbar shrink-0 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)]">
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <MainTabBar
                    activeTab={activeMainTab}
                    agentInstanceIsRunning={agentInstanceIsRunning}
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
                  onMarkdownPreviewChange={
                    updateWorkspaceFileEditorMarkdownPreview
                  }
                  onOpenHtmlPreview={
                    isHtmlFilePath(activeFileTab.path)
                      ? () => openWorkspaceHtmlPreviewTab(activeFileTab)
                      : undefined
                  }
                  onReload={reloadWorkspaceFileEditor}
                  onRestoreMarkdownPreviewScrollTop={
                    getWorkspaceMarkdownPreviewScrollTop
                  }
                  onRestoreViewState={getWorkspaceFileEditorViewState}
                  onSave={saveWorkspaceFileEditor}
                  onSaveMarkdownPreviewScrollTop={
                    saveWorkspaceMarkdownPreviewScrollTop
                  }
                  onSaveViewState={saveWorkspaceFileEditorViewState}
                />
              ) : null}
              {openHtmlPreviewTabs.map((previewTab) => {
                const isActivePreview =
                  activeMainTab.type === "htmlPreview" &&
                  activeMainTab.workspaceId === previewTab.workspaceId &&
                  activeMainTab.path === previewTab.path;
                return (
                  <div
                    className={
                      isActivePreview
                        ? "flex min-h-0 min-w-0 flex-1 flex-col"
                        : "hidden"
                    }
                    key={workspaceHtmlPreviewKey(
                      previewTab.workspaceId,
                      previewTab.path,
                    )}
                  >
                    <WorkspaceHtmlPreviewPanel tab={previewTab} />
                  </div>
                );
              })}
              {activeMainTab.type === "agent" && activeAgentTab ? (
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
                      selectWorkspaceChat(
                        activeAgentTab.workspaceId,
                        activeAgentTab.chatId,
                      )
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
                        : (agentTeamSnapshotCacheRef.current.get(
                            chatRunKey(
                              activeAgentTab.workspaceId,
                              activeAgentTab.chatId,
                            ),
                          ) ?? null)
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
              ) : activeMainTab.type !== "file" &&
                activeMainTab.type !== "htmlPreview" ? (
                <ChatPanel
                  activeWorkspaceName={activeWorkspace?.name ?? null}
                  canOpenAgentTranscript={(instanceId) => {
                    const snapshot =
                      agentTeamSnapshot?.team.chatId === activeChatId
                        ? agentTeamSnapshot
                        : activeChatKey
                          ? (agentTeamSnapshotCacheRef.current.get(activeChatKey) ?? null)
                          : null;
                    return snapshot?.instances.some(
                      (candidate) => candidate.id === instanceId,
                    ) ?? false;
                  }}
                  agentNameForInstance={(instanceId) => {
                    const snapshot =
                      agentTeamSnapshot?.team.chatId === activeChatId
                        ? agentTeamSnapshot
                        : activeChatKey
                          ? (agentTeamSnapshotCacheRef.current.get(activeChatKey) ?? null)
                          : null;
                    return (
                      snapshot?.instances.find(
                        (candidate) => candidate.id === instanceId,
                      )?.definitionSnapshot.name ?? null
                    );
                  }}
                  helpers={chatPanelHelpers}
                  availableModels={availableModels}
                  chatScrollKey={`${activeWorkspaceId}:${activeChatId ?? ""}`}
                  canGuideActiveRun={isGuidableActiveRun(
                    activeRunInfo?.chatKey === activeChatKey
                      ? activeRunInfo
                      : null,
                    activeChatKey !== null &&
                      runningChatKeys.has(activeChatKey),
                  )}
                  draftAttachments={draftAttachments}
                  draftMessage={draftMessage}
                  draftUnsupportedAttachmentMessage={
                    unsupportedDraftAttachmentMessage
                  }
                  contextUsageDiagnostic={contextUsageError?.diagnostic ?? null}
                  contextUsageError={contextUsageError?.message ?? null}
                  contextUsage={displayedContextUsage}
                  isLoadingSettings={isLoadingSettings}
                  isLoadingContextUsage={isLoadingContextUsage}
                  isLoadingMessages={isLoadingActiveChatMessages}
                  hasMoreMessagesBefore={
                    activeChatPagination?.hasMoreBefore === true
                  }
                  isLoadingMoreMessages={isLoadingOlderActiveChatMessages}
                  isSendingMessage={isSendingMessage}
                  isSelectingAttachments={isSelectingAttachments}
                  isPlanModeEnabled={isPlanModeEnabled}
                  messages={messages}
                  readOnly={activeChatReadOnly}
                  overviewRenderer={chatOverviewRenderer}
                  onAddPastedImageAttachments={
                    handleAddPastedImageAttachmentsForChatPanel
                  }
                  onDraftMessageChange={setDraftMessage}
                  onEditMessage={handleEditChatMessage}
                  onGuideQueuedMessage={handleGuideQueuedMessageForChatPanel}
                  onLoadMoreMessages={() => {
                    if (
                      !activeWorkspaceId ||
                      !activeChatId ||
                      isPendingChatId(activeChatId)
                    ) {
                      return Promise.resolve();
                    }
                    return loadOlderChatMessages(
                      activeWorkspaceId,
                      activeChatId,
                    );
                  }}
                  onSelectAttachments={handleSelectDraftAttachmentsForChatPanel}
                  onSelectEditAttachments={handleSelectEditAttachments}
                  onCancelRun={handleCancelRunForChatPanel}
                  onCopyDiagnosticReference={(diagnosticId) =>
                    void copyDiagnosticReference(diagnosticId)
                  }
                  onGuideActiveRun={handleGuideActiveRunForChatPanel}
                  onQueueActiveRun={handleQueueActiveRunForChatPanel}
                  onModelChange={handleModelChangeForChatPanel}
                  onOpenMessageApiRequests={handleOpenMessageApiRequests}
                  onOpenAgentTranscript={(instanceId) => {
                    const snapshot =
                      agentTeamSnapshot?.team.chatId === activeChatId
                        ? agentTeamSnapshot
                        : activeChatKey
                          ? (agentTeamSnapshotCacheRef.current.get(activeChatKey) ?? null)
                          : null;
                    const instance = snapshot?.instances.find(
                      (candidate) => candidate.id === instanceId,
                    );
                    if (instance) {
                      openAgentInstanceTab(instance);
                    }
                  }}
                  onRemoveAttachment={handleRemoveAttachmentForChatPanel}
                  onRemoveSkill={handleRemoveSkillForChatPanel}
                  onRetryRun={handleRetryRunForChatPanel}
                  onSubmit={handleSubmitForChatPanel}
                  onPlanModeEnabledChange={handlePlanModeEnabledChange}
                  onThinkingLevelChange={handleThinkingLevelChangeForChatPanel}
                  onToggleSkill={handleToggleSkillForChatPanel}
                  onWithdrawQueuedMessage={
                    handleWithdrawQueuedMessageForChatPanel
                  }
                  canRetryRun={retryRunRequest !== null && !isSendingMessage}
                  queuedRunCount={queuedRunRequests.length}
                  queuedMessageIds={queuedMessageIds}
                  selectedModelId={selectedModelId}
                  selectedSkillIds={selectedSkillIds}
                  selectedThinkingLevel={selectedThinkingLevel}
                  settings={settings}
                  skillCatalogError={skillCatalogError}
                  skillCatalogRefreshError={skillCatalogRefreshError}
                  skillCatalogStatus={skillCatalogStatus}
                  skills={availableSkills}
                  thinkingLevels={thinkingLevels}
                  workspaces={workspaces}
                  workspaceId={
                    activeWorkspace?.id ?? (activeWorkspaceId || null)
                  }
                />
              ) : null}
              {workspaces
                .filter((workspace) =>
                  terminalOpenWorkspaceIds.has(workspace.id),
                )
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
                contextPanelMobileHeight={contextPanelMobileHeight}
                diffResponse={gitDiff}
                files={contextPanelFiles}
                gitCommitMessage={gitCommitMessage}
                gitOperationKey={gitOperationKey}
                sourceControlTargetKey={sourceControlTargetKeyValue}
                sourceControlTargets={sourceControlTargetOptions}
                expandedFileTreePaths={expandedFileTreePaths}
                isLoadingChatStatistics={isLoadingChatStatistics}
                isLoadingDiff={isLoadingDiff}
                isLoadingContextMemories={isLoadingContextMemories}
                isLoadingPlans={isLoadingActivePlans}
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
                onGenerateGitCommitMessage={
                  handleGenerateGitCommitMessageForContextPanel
                }
                onGitCommitMessageChange={setGitCommitMessage}
                onGitFileOperation={handleGitFileOperationForContextPanel}
                onSourceControlTargetChange={handleSourceControlTargetChange}
                onRefreshWorkspaceFiles={
                  handleRefreshWorkspaceFilesForContextPanel
                }
                onToggleFileTreePath={toggleWorkspaceFileTreePath}
                onOpenWorkspaceFile={handleOpenWorkspaceFileForContextPanel}
                onOpenWorkspaceFileMenu={
                  handleOpenWorkspaceFileMenuForContextPanel
                }
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
                onPlanPhaseRetryWithOverride={(
                  planId,
                  phaseId,
                  implementationChatId,
                  override,
                ) => {
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
                onGenerateWorkspaceSpec={
                  handleGenerateWorkspaceSpecForContextPanel
                }
                onWorkspaceSpecContentChange={setWorkspaceSpecDraft}
                onWorkspaceSpecPreviewChange={setWorkspaceSpecPreviewEnabled}
                onWorkspaceSpecSettingsChange={
                  handleWorkspaceSpecSettingsChangeForContextPanel
                }
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
                onResizeStart={handleContextPanelResizeStart}
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
            codeGraphEnabled={workspaceCodeGraphEnabled}
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
            onCodeGraphEnabledChange={setWorkspaceCodeGraphEnabled}
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
            onPathChange={
              workspaceMode === "ssh"
                ? setWorkspaceRemotePath
                : setWorkspacePath
            }
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
            allowOutsideWorkspace={filePickerRequest.allowOutsideWorkspace}
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
        {settings &&
        !settings.nativeTools.ripgrep.available &&
        !isRipgrepDialogDismissed ? (
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
  const { t } = useI18n();

  return (
    <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && onClose()}>
      <Modal.Container placement="center" size="md">
        <Modal.Dialog aria-label={t("rg command was not found")}>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Icon className="bg-warning-soft text-warning-soft-foreground">
              <CircleAlert aria-hidden="true" className="size-5" />
            </Modal.Icon>
            <Modal.Heading>{t("rg command was not found")}</Modal.Heading>
            <p className="truncate text-xs font-medium text-muted">{installDir}</p>
          </Modal.Header>
          <Modal.Body className="space-y-3">
            <p className="text-sm leading-6 text-foreground">
              {t(
                "Foco uses ripgrep for full-text search. Install it into {path} so the search_text tool can run.",
                { path: installDir },
              )}
            </p>
            {error ? (
              <p className="rounded-lg border border-danger bg-danger-soft px-3 py-2 text-sm font-medium text-danger-soft-foreground">
                {error}
              </p>
            ) : null}
          </Modal.Body>
          <Modal.Footer>
            <Button aria-label={t("Cancel")} variant="tertiary" onPress={onClose}>
              {t("Cancel")}
            </Button>
            <Button
              aria-label={t("Download ripgrep")}
              isPending={isInstalling}
              onPress={onInstall}
            >
              {({ isPending }) => (
                <>
                  {isPending ? (
                    <Spinner color="current" size="sm" />
                  ) : (
                    <Download aria-hidden="true" className="size-4" />
                  )}
                  {isPending ? t("Installing ripgrep…") : t("Download ripgrep")}
                </>
              )}
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
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
    <Modal.Backdrop
      isDismissable={false}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onCancelRun();
        }
      }}
    >
      <Modal.Container placement="center" scroll="inside" size="lg">
        <Modal.Dialog aria-label={t("Foco needs your answer")}>
          <Modal.Header>
            <Modal.Icon className="bg-accent-soft text-accent-soft-foreground">
              <MessageSquare aria-hidden="true" className="size-5" />
            </Modal.Icon>
            <Modal.Heading>{t("Foco needs your answer")}</Modal.Heading>
            <p className="text-sm text-muted">{t("Waiting for your answer")}</p>
          </Modal.Header>
          <form onSubmit={submitAnswer}>
            <Modal.Body className="space-y-4">
              {question.questions.map((item, index) => {
                const draft = draftAnswers[item.id] ?? {
                  manualAnswer: "",
                  selectedOptionValue: null,
                };

                return (
                  <section
                    className="space-y-3 rounded-lg border border-border bg-surface p-3"
                    key={item.id}
                  >
                    <p className="whitespace-pre-wrap text-sm font-semibold leading-6 text-foreground">
                      {question.questions.length > 1
                        ? `${index + 1}. ${item.question}`
                        : item.question}
                    </p>

                    {item.options.length ? (
                      <RadioGroup
                        aria-label={item.question}
                        className="space-y-2"
                        name={`question-option-${item.id}`}
                        value={draft.selectedOptionValue ?? undefined}
                        onChange={(value) => {
                          setDraftAnswers((current) => ({
                            ...current,
                            [item.id]: {
                              manualAnswer: current[item.id]?.manualAnswer ?? "",
                              selectedOptionValue: value,
                            },
                          }));
                          setLocalError(null);
                        }}
                      >
                        {item.options.map((option) => (
                          <Radio key={option.value} value={option.value}>
                            <Radio.Content className="flex w-full cursor-pointer gap-3 rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground transition hover:border-accent/40 hover:bg-accent-soft/40 data-[selected=true]:border-accent data-[selected=true]:bg-accent-soft data-[selected=true]:text-accent-soft-foreground">
                              <Radio.Control className="mt-1 size-4 shrink-0">
                                <Radio.Indicator />
                              </Radio.Control>
                              <span className="min-w-0">
                                <span className="block font-semibold">
                                  {option.label}
                                </span>
                                {option.description ? (
                                  <span className="mt-0.5 block text-xs leading-5 text-muted">
                                    {option.description}
                                  </span>
                                ) : null}
                              </span>
                            </Radio.Content>
                          </Radio>
                        ))}
                      </RadioGroup>
                    ) : null}

                    {item.allowFreeText ? (
                      <TextField
                        fullWidth
                        name={`question-free-${item.id}`}
                        value={draft.manualAnswer}
                        onChange={(value) => {
                          setDraftAnswers((current) => ({
                            ...current,
                            [item.id]: {
                              manualAnswer: value,
                              selectedOptionValue: null,
                            },
                          }));
                          setLocalError(null);
                        }}
                      >
                        <Label>{t("Custom answer")}</Label>
                        <TextArea className="min-h-24" />
                      </TextField>
                    ) : null}
                  </section>
                );
              })}

              {displayedError ? (
                <div className="rounded-lg border border-danger bg-danger-soft px-3 py-2 text-sm text-danger-soft-foreground">
                  {displayedError}
                </div>
              ) : null}
            </Modal.Body>
            <Modal.Footer>
              <Button
                aria-label={t("Cancel run")}
                type="button"
                variant="tertiary"
                onPress={onCancelRun}
              >
                {t("Cancel run")}
              </Button>
              <Button
                aria-label={t("Continue run")}
                isDisabled={!canSubmit}
                isPending={isSaving}
                type="submit"
              >
                {({ isPending }) => (
                  <>
                    {isPending ? (
                      <Spinner color="current" size="sm" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Continue run")}
                  </>
                )}
              </Button>
            </Modal.Footer>
          </form>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function MainTabBar({
  activeTab,
  agentInstanceIsRunning,
  chatSessionStatusFor,
  onCloseTab,
  onCloseTabs,
  onSelectTab,
  tabs,
}: {
  activeTab: ActiveMainTab;
  agentInstanceIsRunning: (
    workspaceId: string,
    chatId: string,
    instanceId: string,
  ) => boolean;
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
  const hasTrackedTabKeysRef = useRef(false);
  const previousTabKeysRef = useRef<string[]>([]);
  const [contextMenu, setContextMenu] =
    useState<MainTabContextMenuState | null>(null);
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

    const maxScrollLeft = Math.max(
      0,
      element.scrollWidth - element.clientWidth,
    );
    const availableWidth =
      tabsContainerRef.current?.clientWidth ?? element.clientWidth;
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

    const contextMenuTabKey = mainTabKey(contextMenu.tab);
    if (!tabs.some((tab) => mainTabKey(tab) === contextMenuTabKey)) {
      setContextMenu(null);
    }
  }, [contextMenu, tabs]);


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

    const maxScrollLeft = Math.max(
      0,
      element.scrollWidth - element.clientWidth,
    );
    if (maxScrollLeft <= 0) {
      return;
    }

    const rawDelta =
      Math.abs(event.deltaX) > Math.abs(event.deltaY)
        ? event.deltaX
        : event.deltaY;
    if (rawDelta === 0) {
      return;
    }

    const deltaUnit =
      event.deltaMode === 1
        ? 16
        : event.deltaMode === 2
          ? element.clientWidth
          : 1;
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

  function handleContextMenu(
    event: ReactMouseEvent<HTMLDivElement>,
    tab: MainTabSummary,
  ) {
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

  function hasClosableTabs(
    scope: MainTabCloseScope,
    anchorTab: MainTabSummary,
  ) {
    const anchorIndex = tabs.findIndex(
      (tab) => mainTabKey(tab) === mainTabKey(anchorTab),
    );
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
    <ContextMenu
      aria-label={contextMenu.tab.title}
      isOpen
      items={contextMenuItems.map((item) => ({
        disabled: !hasClosableTabs(item.scope, contextMenu.tab),
        icon: <X aria-hidden="true" className="size-3.5" />,
        id: item.scope,
        label: t(item.label),
      }))}
      left={contextMenu.left}
      top={contextMenu.top}
      onAction={(key) => {
        closeTabsFromMenu(String(key) as MainTabCloseScope);
      }}
      onOpenChange={(open) => {
        if (!open) {
          setContextMenu(null);
        }
      }}
    />
  ) : null;

  return (
    <>
      <div
        className="chat-tabs flex min-w-0 flex-1 flex-nowrap overflow-hidden"
        ref={tabsContainerRef}
      >
        {scrollState.hasOverflow ? (
          <Button
            aria-label={t("Scroll chat tabs left")}
            className="chat-tab-scroll-button"
            isDisabled={!scrollState.canScrollLeft}
            onPress={() => scrollTabs(-1)}
            type="button"
            variant="ghost"
          >
            <ChevronLeft aria-hidden="true" className="size-4" />
          </Button>
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
                tab.type === "chat"
                  ? chatSessionStatusFor(
                      chatRunKey(tab.workspaceId, tab.chatId),
                    ).kind === "running"
                  : tab.type === "agent"
                    ? agentInstanceIsRunning(
                        tab.workspaceId,
                        tab.chatId,
                        tab.instanceId,
                      )
                    : false;
              const title =
                tab.type === "htmlPreview"
                  ? t("{name} · Preview", { name: tab.name })
                  : tab.title ||
                    t(
                      tab.type === "chat"
                        ? "Chat"
                        : tab.type === "agent"
                          ? "Agent"
                          : "Files",
                    );
              const key = mainTabKey(tab);

              return (
                <div
                  className={`chat-tab-item group flex h-12 min-w-36 max-w-64 shrink-0 items-center rounded-lg border px-2 py-1.5 transition-colors ${
                    isActive
                      ? "border-[var(--accent)] bg-[var(--surface)] text-[var(--foreground)] shadow-sm"
                      : "border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_80%,transparent)] text-[var(--muted)] hover:border-[var(--border)] hover:bg-[var(--surface)]"
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
                    data-heroui-exception="native-chat-tab"
                    onClick={() => onSelectTab(tab)}
                    role="tab"
                    title={title}
                    type="button"
                  >
                    <span className="flex min-w-0 items-center gap-1.5 truncate text-sm font-semibold leading-5">
                      {tab.type === "file" ? (
                        <FileText
                          aria-hidden="true"
                          className="size-3.5 shrink-0 text-slate-500"
                        />
                      ) : null}
                      {tab.type === "htmlPreview" ? (
                        <AppWindow
                          aria-hidden="true"
                          className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
                        />
                      ) : null}
                      {tab.type === "agent" ? (
                        <Bot
                          aria-hidden="true"
                          className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
                        />
                      ) : null}
                      {isRunning ? (
                        <span
                          aria-label={t(
                            tab.type === "agent"
                              ? "Agent is running"
                              : "Chat is running",
                          )}
                          className="inline-flex shrink-0"
                          role="status"
                        >
                          <LoaderCircle
                            aria-hidden="true"
                            className="chat-tab-running-spinner size-3.5 animate-spin text-[var(--accent-soft-foreground)]"
                          />
                        </span>
                      ) : null}
                      <span className="min-w-0 truncate">{title}</span>
                    </span>
                    <span className="flex min-w-0 items-center gap-1 text-[11px] font-medium leading-4 text-[var(--muted)]">
                      <WorkspaceIcon
                        className="size-3 shrink-0 rounded-sm object-cover"
                        fallbackClassName="size-3 shrink-0"
                        logoUrl={tab.workspaceLogoUrl}
                      />
                      <span className="min-w-0 truncate">
                        {tab.workspaceName}
                      </span>
                    </span>
                  </button>
                  <span className="ml-1 inline-flex size-7 shrink-0 items-center justify-center">
                    <Button
                      aria-label={t("Close chat tab {title}", { title })}
                      className="size-7 min-w-7 opacity-0 focus:opacity-100 group-hover:opacity-100 max-[767px]:opacity-100 max-[767px]:focus:opacity-100 max-[767px]:group-hover:opacity-100"
                      isIconOnly
                      onPress={() => onCloseTab(tab)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                    </Button>
                  </span>
                </div>
              );
            })
          ) : (
            <div className="flex h-12 min-w-0 items-center rounded-lg border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_55%,transparent)] px-3 text-sm font-medium text-[var(--muted)]">
              {t("No open chats")}
            </div>
          )}
        </div>
        {scrollState.hasOverflow ? (
          <Button
            aria-label={t("Scroll chat tabs right")}
            className="chat-tab-scroll-button"
            isDisabled={!scrollState.canScrollRight}
            onPress={() => scrollTabs(1)}
            type="button"
            variant="ghost"
          >
            <ChevronRight aria-hidden="true" className="size-4" />
          </Button>
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
  icon: (props: {
    className?: string;
    "aria-hidden"?: boolean | "true" | "false";
  }) => ReactNode;
  label: string;
  onClick: () => void;
  selection?: "action" | "page" | "toggle";
};

function FocoNavRail({
  activeMode,
  canLogout,
  contextPanelButton,
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
        <Button
          aria-label="Foco"
          className="foco-nav-logo-button"
          onPress={onReturnHome}
          type="button"
          variant="ghost"
        >
          <FocoLogoMark />
        </Button>
        <NavRailButton
          active={activeMode === "chat"}
          icon={Home}
          label={t("Home")}
          onClick={onHomeClick}
          selection="page"
        />
        <NavRailButton
          active={activeMode === "stats"}
          icon={Activity}
          label={t("API details")}
          onClick={onOpenStats}
          selection="page"
        />
        <NavRailButton
          active={activeMode === "scheduled"}
          icon={CalendarClock}
          label={t("Scheduled tasks")}
          onClick={onOpenScheduledTasks}
          selection="page"
        />
        <NavRailButton
          active={activeMode === "skill-store"}
          icon={ShoppingBag}
          label={t("Skill Store")}
          onClick={onOpenSkillStore}
          selection="page"
        />
        <NavRailButton
          active={activeMode === "settings"}
          icon={Settings}
          label={t("Settings")}
          onClick={onOpenSettings}
          selection="page"
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
          active={false}
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
  selection = "action",
}: NavRailAction) {
  return (
    <Button
      aria-current={selection === "page" && active ? "page" : undefined}
      aria-label={label}
      aria-pressed={selection === "toggle" ? active : undefined}
      className={`foco-nav-rail-button ${active ? "foco-nav-rail-button-active" : ""}`}
      isDisabled={disabled}
      isIconOnly
      onPress={onClick}
      type="button"
      variant={active ? "tertiary" : "ghost"}
    >
      <Icon aria-hidden="true" className="size-4" />
    </Button>
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
        <span className="inline-flex size-20 items-center justify-center overflow-hidden rounded-2xl text-[var(--accent-soft-foreground)]">
          <WorkspaceIcon
            className="size-20 rounded-2xl object-cover"
            fallbackClassName="size-10"
            isRemote={Boolean(selectedWorkspace?.serverId)}
            logoUrl={selectedWorkspace?.logoUrl}
          />
        </span>
        <div className="min-w-0">
          <span className="foco-eyebrow">{t("Workspace")}</span>
          <h2 className="foco-display mt-1 truncate text-3xl leading-tight text-[var(--foreground)]">
            {selectedWorkspace?.name ?? t("No workspace selected")}
          </h2>
        </div>
      </div>
    </section>
  );
}

function workspaceConnectionDotClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return "bg-[var(--success)]";
  }
  if (
    normalized === "checking" ||
    normalized === "connecting" ||
    normalized === "reconnecting"
  ) {
    return "bg-[var(--warning)]";
  }
  if (normalized === "failed" || normalized === "failedauth") {
    return "bg-[var(--danger)]";
  }
  if (normalized === "degraded") {
    return "bg-[var(--warning)]";
  }
  return "bg-[var(--default)]";
}

function PanelLoadingFallback() {
  return (
    <div className="grid h-full w-full place-items-center p-8 text-[var(--muted)]">
      <LoaderCircle aria-hidden="true" className="size-6 animate-spin" />
    </div>
  );
}

function FocoLogoMark() {
  return (
    <span
      aria-hidden="true"
      className="foco-logo-mark inline-flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-[var(--surface)] shadow-[var(--overlay-shadow)] ring-1 ring-[color-mix(in_oklab,var(--border)_80%,transparent)]"
      dangerouslySetInnerHTML={{ __html: focoLogoSvg }}
    />
  );
}

function hydrateChatTab(
  tab: OpenChatTab,
  workspaces: WorkspaceSummary[],
): ChatTabSummary {
  const workspace = workspaces.find(
    (workspace) => workspace.id === tab.workspaceId,
  );
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
  const workspace = workspaces.find(
    (workspace) => workspace.id === tab.workspaceId,
  );

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
        tab.workspaceId === nextTab.workspaceId &&
        tab.chatId === nextTab.chatId,
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
  previewTabs: OpenHtmlPreviewTab[],
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

  const routePreviews = route.previews
    ? dedupeBrowserRouteHtmlPreviewTabs(route.previews)
    : openHtmlPreviewTabsToBrowserRouteTabs(previewTabs);
  if (route.activePreview) {
    routePreviews.push(route.activePreview);
  }

  const dedupedFiles = dedupeBrowserRouteFileTabs(routeFiles);
  const dedupedPreviews = dedupeBrowserRouteHtmlPreviewTabs(routePreviews);
  return {
    ...nextRoute,
    ...(dedupedFiles.length ? { files: dedupedFiles } : {}),
    ...(route.activeFile ? { activeFile: route.activeFile } : {}),
    ...(dedupedPreviews.length ? { previews: dedupedPreviews } : {}),
    ...(route.activePreview ? { activePreview: route.activePreview } : {}),
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

function openChatTabsToBrowserRouteTabs(
  tabs: OpenChatTab[],
): BrowserRouteChatTab[] {
  return tabs.map((tab) => ({
    chatId: tab.chatId,
    workspaceId: tab.workspaceId,
  }));
}

function openFileTabsToBrowserRouteFileTabs(
  tabs: OpenFileTab[],
): BrowserRouteFileTab[] {
  return tabs.map((tab) => ({
    path: tab.path,
    workspaceId: tab.workspaceId,
  }));
}

function openHtmlPreviewTabsToBrowserRouteTabs(
  tabs: OpenHtmlPreviewTab[],
): BrowserRouteHtmlPreviewTab[] {
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

function browserRouteHtmlPreviewTabToOpenTab(
  preview: BrowserRouteHtmlPreviewTab,
  workspace: WorkspaceSummary,
): OpenHtmlPreviewTab {
  return {
    name: fileNameFromPath(preview.path),
    path: preview.path,
    workspaceId: preview.workspaceId,
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

function dedupeBrowserRouteHtmlPreviewTabs(tabs: BrowserRouteHtmlPreviewTab[]) {
  const seen = new Set<string>();
  return tabs.filter((tab) => {
    if (!isHtmlPreviewPath(tab.path)) {
      return false;
    }

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
      (tab) =>
        tab.workspaceId === nextTab.workspaceId && tab.path === nextTab.path,
    )
  ) {
    return tabs;
  }

  return [...tabs, nextTab];
}

function upsertOpenHtmlPreviewTab(
  tabs: OpenHtmlPreviewTab[],
  nextTab: OpenHtmlPreviewTab,
) {
  if (
    tabs.some(
      (tab) =>
        tab.workspaceId === nextTab.workspaceId && tab.path === nextTab.path,
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

  if (tab.type === "htmlPreview") {
    return workspaceHtmlPreviewKey(tab.workspaceId, tab.path);
  }

  return workspaceFileEditorKey(tab.workspaceId, tab.path);
}

function mainTabMatches(activeTab: ActiveMainTab, tab: MainTabSummary) {
  if (
    activeTab.type !== tab.type ||
    activeTab.workspaceId !== tab.workspaceId
  ) {
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

  if (tab.type === "htmlPreview") {
    return activeTab.type === "htmlPreview" && activeTab.path === tab.path;
  }

  return activeTab.type === "file" && activeTab.path === tab.path;
}

function workspaceFileEditorKey(workspaceId: string, path: string) {
  return `${workspaceId}:${path}`;
}

function workspaceHtmlPreviewKey(workspaceId: string, path: string) {
  return `htmlPreview:${workspaceId}:${path}`;
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

/** Presence of a chat relative to workspace summaries (first page + loaded pages only). */
type WorkspaceChatPresence = "present" | "unknown" | "missing";

/**
 * Distinguishes chats loaded in the current summary page from chats that may
 * still exist off-page (`hasMore`) vs chats that are known gone.
 */
function workspaceChatPresence(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
  options: { allowPending?: boolean } = {},
): WorkspaceChatPresence {
  const workspace = workspaces.find((item) => item.id === tab.workspaceId);
  if (!workspace) {
    return "missing";
  }

  if (options.allowPending && isPendingChatId(tab.chatId)) {
    return "present";
  }

  if (workspace.chats.some((chat) => chat.id === tab.chatId)) {
    return "present";
  }

  if (workspace.chatPagination?.hasMore) {
    return "unknown";
  }

  return "missing";
}

/** Keep client state when present or only off-page unknown; drop only when missing. */
function workspaceChatIsNotMissing(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
  options: { allowPending?: boolean } = {},
) {
  return workspaceChatPresence(workspaces, tab, options) !== "missing";
}

function workspaceHasChat(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
) {
  return workspaceChatIsNotMissing(workspaces, tab);
}

function workspaceHasChatTab(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
) {
  return workspaceChatIsNotMissing(workspaces, tab, { allowPending: true });
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
    <main className="app-root grid place-items-center bg-[var(--surface-secondary)] px-4 text-[var(--foreground)]">
      <form
        aria-label={t("Foco authentication")}
        className="w-full max-w-sm rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_90%,transparent)] px-4 py-5 shadow-[var(--overlay-shadow)]"
        onSubmit={onLogin}
      >
        <div className="flex items-center gap-3">
          <FocoLogoMark />
          <div className="min-w-0">
            <h1 className="foco-display text-2xl leading-none text-[var(--foreground)]">
              Foco
            </h1>
            <p className="foco-eyebrow mt-1.5">{t("Password required")}</p>
          </div>
        </div>
        <TextField
          className="mt-5 block"
          value={password}
          onChange={onPasswordChange}
        >
          <Label className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
            {t("Password")}
          </Label>
          <Input
            autoComplete="current-password"
            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
            type="password"
          />
        </TextField>
        {error ? (
          <div className="mt-4 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
            {error}
          </div>
        ) : null}
        <Button
          aria-label={t("Log in")}
          className="mt-4 inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
          isDisabled={isLoggingIn || !password.trim()}
          type="submit"
        >
          {isLoggingIn ? (
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          ) : (
            <Lock aria-hidden="true" className="size-4" />
          )}
          {t("Log in")}
        </Button>
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
  // A replayed running card may legitimately have no output (for example a
  // waiting `agent_wait_tasks` result), so output alone cannot establish which
  // version is newer. Keep terminal state monotonic and use the newer/equally
  // terminal update only to supplement fields that the earlier version lacked.
  const currentRank = toolCallStatusRank(currentToolCall.status);
  const nextRank = toolCallStatusRank(normalizedToolCall.status);
  const preferred = nextRank >= currentRank
    ? normalizedToolCall
    : currentToolCall;
  const supplementary = preferred === normalizedToolCall
    ? currentToolCall
    : normalizedToolCall;

  return {
    ...supplementary,
    ...preferred,
    status: preferred.status,
    output: preferred.output ?? supplementary.output,
    isError: preferred.isError,
    startedAt: preferred.startedAt ?? supplementary.startedAt,
    completedAt: preferred.completedAt ?? supplementary.completedAt,
    liveOutput:
      preferred.liveOutput ??
      (preferred.output === null
        ? supplementary.liveOutput
        : undefined),
  };
}

function toolCallStatusRank(status: string) {
  if (status === "completed" || status === "error" || status === "cancelled") {
    return 2;
  }
  return status === "running" ? 1 : 0;
}

function applyToolResult(
  toolCalls: ChatToolCallSummary[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
  startedAt?: string | null,
  completedAt?: string | null,
  terminal = true,
) {
  return toolCalls.map((toolCall) => {
    if (toolCall.id !== toolCallId) {
      return toolCall;
    }
    if (
      !terminal &&
      (toolCall.status === "completed" ||
        toolCall.status === "error" ||
        toolCall.status === "cancelled")
    ) {
      return toolCall;
    }
    return {
      ...toolCall,
      output,
      isError,
      status: terminal ? (isError ? "error" : "completed") : "running",
      startedAt: startedAt ?? toolCall.startedAt ?? null,
      completedAt: terminal ? (completedAt ?? toolCall.completedAt ?? null) : null,
      liveOutput: undefined,
    };
  });
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
        : (liveOutput?.stdout ?? ""),
    stderr:
      stream === "stderr"
        ? `${liveOutput?.stderr ?? ""}${delta}`
        : (liveOutput?.stderr ?? ""),
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
  kind: ChatContextCompressionKind,
): ChatRunBadge {
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
    snapshotId:
      streamEvent.detail?.snapshotId ?? streamEvent.snapshotId ?? null,
    compressionId:
      streamEvent.detail?.compressionId ?? streamEvent.compressionId ?? null,
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
  return (
    detail.compressionId ??
    detail.snapshotId ??
    `${kind}:${detail.startedAt ?? "pending"}`
  );
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
    return contextCompressionPartsMatch(part, nextPart);
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
  const currentCompressionId = current.detail.compressionId;
  const nextCompressionId = next.detail.compressionId;
  if (currentCompressionId && nextCompressionId) {
    return currentCompressionId === nextCompressionId;
  }
  const currentSnapshotId = current.detail.snapshotId;
  const nextSnapshotId = next.detail.snapshotId;
  if (currentSnapshotId && nextSnapshotId) {
    return currentSnapshotId === nextSnapshotId;
  }
  return (
    current.kind === next.kind &&
    Boolean(current.detail.startedAt) &&
    current.detail.startedAt === next.detail.startedAt
  );
}

function mergeContextCompressionPart(
  current: ChatContextCompressionPart,
  next: ChatContextCompressionPart,
): ChatContextCompressionPart {
  // Reconnect and history reload can replay an older event after a terminal
  // event. Preserve the terminal state (or the later attempt) so cards never
  // appear to move backwards.
  const [preferred, supplementary] = contextCompressionPartShouldReplace(
    current,
    next,
  )
    ? [next, current]
    : [current, next];
  const detail = normalizedContextCompressionDetail({
    ...supplementary.detail,
    ...preferred.detail,
    compressionId:
      preferred.detail.compressionId ??
      supplementary.detail.compressionId ??
      null,
    snapshotId:
      preferred.detail.snapshotId ?? supplementary.detail.snapshotId ?? null,
    originalTokenCount:
      preferred.detail.originalTokenCount ??
      supplementary.detail.originalTokenCount ??
      null,
    summaryTokenCount:
      preferred.detail.summaryTokenCount ??
      supplementary.detail.summaryTokenCount ??
      null,
    startedAt: preferred.detail.startedAt ?? supplementary.detail.startedAt ?? null,
    completedAt:
      preferred.detail.completedAt ?? supplementary.detail.completedAt ?? null,
    providerId: preferred.detail.providerId ?? supplementary.detail.providerId ?? null,
    modelId: preferred.detail.modelId ?? supplementary.detail.modelId ?? null,
    providerRequestId:
      preferred.detail.providerRequestId ??
      supplementary.detail.providerRequestId ??
      null,
    compressionMode:
      preferred.detail.compressionMode ??
      supplementary.detail.compressionMode ??
      null,
    attemptIndex:
      preferred.detail.attemptIndex ?? supplementary.detail.attemptIndex ?? null,
    outcome: preferred.detail.outcome ?? supplementary.detail.outcome ?? null,
    action: preferred.detail.action ?? supplementary.detail.action ?? null,
    errorMessage:
      preferred.detail.errorMessage ?? supplementary.detail.errorMessage ?? null,
  });
  return {
    ...preferred,
    id: detail.compressionId ?? detail.snapshotId ?? preferred.id,
    detail,
  };
}

function contextCompressionPartShouldReplace(
  current: ChatContextCompressionPart,
  next: ChatContextCompressionPart,
) {
  const currentTerminal = isTerminalContextCompressionStatus(current.status);
  const nextTerminal = isTerminalContextCompressionStatus(next.status);
  if (currentTerminal !== nextTerminal) {
    return nextTerminal;
  }

  const currentAttempt = current.detail.attemptIndex;
  const nextAttempt = next.detail.attemptIndex;
  if (currentAttempt != null && nextAttempt != null && currentAttempt !== nextAttempt) {
    return nextAttempt > currentAttempt;
  }

  return (
    contextCompressionStatusRank(next.status) >=
    contextCompressionStatusRank(current.status)
  );
}

function isTerminalContextCompressionStatus(status: string) {
  return (
    status === "completed" ||
    status === "skipped" ||
    status === "failed" ||
    status === "cancelled"
  );
}

function contextCompressionStatusRank(status: string) {
  if (isTerminalContextCompressionStatus(status)) {
    return 2;
  }
  if (status === "retrying") {
    return 1;
  }
  return 0;
}

function normalizedContextCompressionDetail(
  detail: ChatContextCompressionDetail,
): ChatContextCompressionDetail {
  return {
    status: detail.status,
    kind: detail.kind,
    compressionId: detail.compressionId ?? null,
    snapshotId: detail.snapshotId ?? null,
    originalTokenCount: detail.originalTokenCount ?? null,
    summaryTokenCount: detail.summaryTokenCount ?? null,
    startedAt: detail.startedAt ?? null,
    completedAt: detail.completedAt ?? null,
    providerId: detail.providerId ?? null,
    modelId: detail.modelId ?? null,
    providerRequestId: detail.providerRequestId ?? null,
    compressionMode: detail.compressionMode ?? null,
    attemptIndex: detail.attemptIndex ?? null,
    outcome: detail.outcome ?? null,
    action: detail.action ?? null,
    errorMessage: detail.errorMessage ?? null,
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

function streamingAttemptSnapshot(
  message: ShellMessage,
): StreamAttemptSnapshot {
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
  const reasoningDelta = missingFinalSuffix(
    message.reasoning ?? "",
    nextReasoning ?? "",
  );
  if (reasoningDelta) {
    parts = appendReasoningPart(parts, reasoningDelta);
  }
  if (activeReasoningStartedAtMs !== null) {
    const serverParts = finishReasoningPartWithDuration(
      parts,
      streamEvent.reasoningDurationMs,
    );
    parts =
      serverParts === parts
        ? finishActiveReasoningPart(
            parts,
            activeReasoningStartedAtMs,
            completedAtMs,
          )
        : serverParts;
  } else {
    parts = finishReasoningPartWithDuration(
      parts,
      streamEvent.reasoningDurationMs,
    );
  }
  parts = finalizedTextParts(parts, message.content, streamEvent);

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
  let parts = (() => {
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

  parts = finalizedTextParts(parts, message.content, streamEvent);

  return {
    ...message,
    metrics: streamEvent.metrics,
    memoriesUsed: streamEvent.memoriesUsed,
    extractedMemories: message.extractedMemories,
    status: undefined,
    parts: parts.length ? parts : fallbackMessageParts(message),
  };
}

/**
 * Complete is the durable authority for the final visible response, but it is
 * not permission to reorder the conversation. New servers tell us whether the
 * completion supplies the exact final provider segment. In that case replace
 * only that text block in place; a tool-only final turn instead receives its
 * truthful completion fallback after the last structural block. Older payloads
 * retain the previous suffix-compatible behavior.
 */
export function finalizedTextParts(
  parts: ChatMessagePart[],
  currentContent: string,
  streamEvent: Extract<ChatStreamEvent, { type: "complete" }>,
): ChatMessagePart[] {
  if (!streamEvent.text) {
    return parts;
  }
  if (streamEvent.finalTextSegment !== undefined) {
    if (!streamEvent.finalTextSegment) {
      return parts;
    }
    // `finalTextSegment` is itself the original explicit contract. A few
    // older producers omitted its companion boolean, but that does not turn a
    // known provider segment into a tool-only fallback.
    return streamEvent.hasFinalTextSegment === false
      ? appendOrReplaceToolOnlyFinalFallback(parts, streamEvent.finalTextSegment)
      : replaceLastTextPart(parts, streamEvent.finalTextSegment);
  }
  if (streamEvent.hasFinalTextSegment === true) {
    return replaceLastTextPart(parts, streamEvent.text);
  }
  if (streamEvent.hasFinalTextSegment === false) {
    return appendOrReplaceToolOnlyFinalFallback(parts, streamEvent.text);
  }

  const textDelta = missingFinalSuffix(currentContent, streamEvent.text);
  if (textDelta) {
    return appendTextPart(parts, textDelta);
  }
  if (
    currentContent !== streamEvent.text &&
    !currentContent.includes(streamEvent.text)
  ) {
    return replaceLastTextPart(parts, streamEvent.text);
  }
  return parts;
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

/**
 * Backend stream events after guidance keep the durable interrupted assistant
 * id. Prefer that contract field for routing aliases so consecutive recoveries
 * do not alias the previous temporary guidance bubble id. When the field is
 * absent (manual guidance / older events), fall back to the previous visible
 * segment id.
 */
export function routingInterruptedAssistantMessageId(
  previousVisibleAssistantId: string,
  interruptedAssistantId?: string | null,
): string {
  return interruptedAssistantId ?? previousVisibleAssistantId;
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

type ChatStreamSession = {
  runId: string | null;
  epoch: number;
  abortController: AbortController;
  lastSequence: number | null;
  assistantMessageId: string | null;
};

/**
 * Moves local/legacy stream aliases onto the server's durable assistant id in a
 * single state update. A reconnect can observe both forms briefly; rendering
 * either as independent messages is what produces duplicate assistant bubbles.
 */
export function canonicalizeAssistantMessage(
  current: ShellMessage[],
  canonicalId: string,
  aliases: Iterable<string | null | undefined>,
): ShellMessage[] {
  const aliasIds = new Set<string>([canonicalId]);
  for (const alias of aliases) {
    if (alias) {
      aliasIds.add(alias);
    }
  }

  const matching = current.filter(
    (message) => message.role === "assistant" && aliasIds.has(message.id),
  );
  if (!matching.length) {
    return current;
  }

  // `current` is the only ordering authority available for legacy parts. Keep
  // its assistant order while folding aliases; starting from the durable id can
  // otherwise prepend a late Complete before earlier reasoning/tool events.
  const merged = matching.slice(1).reduce<ShellMessage>((result, message) => {
    const content = mergeAssistantStreamText(result.content, message.content);
    const reasoning = mergeAssistantStreamText(
      result.reasoning ?? "",
      message.reasoning ?? "",
    );
    return {
      ...result,
      content,
      extractedMemories:
        result.extractedMemories.length >= message.extractedMemories.length
          ? result.extractedMemories
          : message.extractedMemories,
      memoriesUsed:
        result.memoriesUsed.length >= message.memoriesUsed.length
          ? result.memoriesUsed
          : message.memoriesUsed,
      metrics: message.metrics ?? result.metrics,
      parts: mergeAssistantMessageParts(result.parts, message.parts),
      reasoning: reasoning || null,
      status: mergeAssistantMessageStatus(result.status, message.status),
      toolCalls: mergeAssistantToolCalls(result.toolCalls, message.toolCalls),
    };
  }, matching[0]);
  // A temporary alias can be remapped to the durable id before its duplicate is
  // removed. Prefer an explicit terminal version already carrying that id, but
  // ignore a remapped streaming placeholder and use the monotonic merge then.
  const durableTerminal = matching.find(
    (message) => message.id === canonicalId && message.status !== "streaming",
  );
  const canonicalMessage = {
    ...merged,
    id: canonicalId,
    status: durableTerminal?.status ?? merged.status,
  };
  const firstMatchingIndex = current.findIndex((message) =>
    matching.includes(message),
  );
  const next = current.filter((message) => !matching.includes(message));
  next.splice(firstMatchingIndex, 0, canonicalMessage);
  return next;
}

/** A terminal alias must never be revived by a stale streaming placeholder. */
function mergeAssistantMessageStatus(
  current: ShellMessage["status"],
  next: ShellMessage["status"],
): ShellMessage["status"] {
  const rank = (status: ShellMessage["status"]) =>
    status === "streaming" ? 0 : 1;
  // Alias order is not an event sequence. Preserve the earlier terminal when
  // ranks tie instead of allowing a stale alias to replace it arbitrarily.
  return rank(next) > rank(current) ? next : current;
}

function mergeAssistantToolCalls(
  current: ChatToolCallSummary[],
  next: ChatToolCallSummary[],
): ChatToolCallSummary[] {
  const merged = [...current];
  const indexes = new Map<string, number>();
  for (const [index, toolCall] of merged.entries()) {
    const existingIndex = indexes.get(toolCall.id);
    if (existingIndex === undefined) {
      indexes.set(toolCall.id, index);
    } else {
      merged[existingIndex] = mergeToolCallUpdate(
        merged[existingIndex],
        toolCall,
      );
    }
  }
  // Drop duplicate legacy tool summaries after their first timeline position.
  const unique = merged.filter((toolCall, index) => indexes.get(toolCall.id) === index);
  indexes.clear();
  unique.forEach((toolCall, index) => indexes.set(toolCall.id, index));

  for (const toolCall of next) {
    const existingIndex = indexes.get(toolCall.id);
    if (existingIndex === undefined) {
      indexes.set(toolCall.id, unique.length);
      unique.push(toolCall);
    } else {
      unique[existingIndex] = mergeToolCallUpdate(
        unique[existingIndex],
        toolCall,
      );
    }
  }
  return unique;
}

function mergeAssistantStreamText(current: string, next: string) {
  if (!current) {
    return next;
  }
  if (!next || current === next || current.includes(next)) {
    return current;
  }
  if (next.includes(current)) {
    return next;
  }
  return `${current}${next}`;
}

/**
 * Keep every distinct part while collapsing the shared suffix/prefix produced
 * when a temporary local assistant and its durable server counterpart overlap.
 * Choosing the longer array is unsafe: two aliases can contain equally many,
 * but different, text, reasoning, or tool parts.
 */
export function mergeAssistantMessageParts(
  current: ChatMessagePart[],
  next: ChatMessagePart[],
): ChatMessagePart[] {
  const merged: ChatMessagePart[] = [];
  const stableIndexes = new Map<string, number>();
  for (const part of current) {
    const identity = chatMessagePartIdentity(part);
    const existingIndex = identity ? stableIndexes.get(identity) : undefined;
    if (existingIndex === undefined) {
      if (identity) {
        stableIndexes.set(identity, merged.length);
      }
      merged.push(part);
    } else {
      merged[existingIndex] = mergeAssistantMessagePart(merged[existingIndex], part);
    }
  }

  // Unkeyed text/reasoning has no protocol id in historical records. Align it
  // only while its type and content prove continuity, and only forward from the
  // last aligned position. A block that cannot be proven equivalent remains a
  // distinct timeline event instead of being silently discarded.
  let nextSearchStart = 0;
  for (const [nextIndex, part] of next.entries()) {
    const identity = chatMessagePartIdentity(part);
    const stableIndex = identity ? stableIndexes.get(identity) : undefined;
    if (stableIndex !== undefined) {
      merged[stableIndex] = mergeAssistantMessagePart(merged[stableIndex], part);
      nextSearchStart = Math.max(nextSearchStart, stableIndex + 1);
      continue;
    }
    if (identity) {
      stableIndexes.set(identity, merged.length);
      merged.push(part);
      nextSearchStart = merged.length;
      continue;
    }

    const compatibleIndex = findContinuousMessagePartIndex(
      merged,
      part,
      nextSearchStart,
      nextLegacyPartSearchEnd(next, nextIndex, stableIndexes, merged.length),
    );
    if (compatibleIndex === -1) {
      merged.push(part);
    } else {
      merged[compatibleIndex] = mergeAssistantMessagePart(
        merged[compatibleIndex],
        part,
      );
      nextSearchStart = compatibleIndex + 1;
    }
  }
  return merged;
}

function chatMessagePartIdentity(part: ChatMessagePart): string | null {
  switch (part.type) {
    case "toolCall":
      return `tool:${part.toolCall.id}`;
    case "contextCompression":
      if (part.detail.compressionId) {
        return `compression:${part.detail.compressionId}`;
      }
      if (part.detail.snapshotId) {
        return `compression:snapshot:${part.detail.snapshotId}`;
      }
      // Old history may synthesize a generic `kind:pending` id. It is not an
      // event identity, so treating it as one would discard distinct cards.
      return part.detail.startedAt
        ? `compression:legacy:${part.kind}:${part.detail.startedAt}`
        : null;
    case "agentTaskLifecycle":
      return `agent-task:${part.lifecycle.eventId}`;
    case "userInterruption":
      return `interruption:${part.id}`;
    case "attachment":
      return `attachment:${part.attachment.id}`;
    default:
      return null;
  }
}

function findContinuousMessagePartIndex(
  parts: ChatMessagePart[],
  next: ChatMessagePart,
  startIndex: number,
  endIndex: number,
): number {
  for (let index = startIndex; index < endIndex; index += 1) {
    const current = parts[index];
    if (current && partsHaveContinuousContent(current, next)) {
      return index;
    }
  }
  return -1;
}

/**
 * Legacy text and reasoning lack a protocol id. A later known structure bounds
 * the only interval in which a compatible segment can be upgraded. Without an
 * anchor, consider only the expected next timeline position: matching equal
 * text across a tool or interruption would be speculation, not deduplication.
 */
function nextLegacyPartSearchEnd(
  next: ChatMessagePart[],
  nextIndex: number,
  stableIndexes: Map<string, number>,
  mergedLength: number,
): number {
  for (let index = nextIndex + 1; index < next.length; index += 1) {
    const identity = chatMessagePartIdentity(next[index]);
    if (identity) {
      const stableIndex = stableIndexes.get(identity);
      if (stableIndex !== undefined) {
        return stableIndex;
      }
    }
  }
  return Math.min(mergedLength, nextIndex + 1);
}

function partsHaveContinuousContent(
  current: ChatMessagePart,
  next: ChatMessagePart,
): boolean {
  const hasContinuousText = (currentText: string, nextText: string) =>
    currentText === nextText ||
    currentText.startsWith(nextText) ||
    nextText.startsWith(currentText);
  if (current.type === "text" && next.type === "text") {
    return hasContinuousText(current.text, next.text);
  }
  if (current.type === "reasoning" && next.type === "reasoning") {
    return hasContinuousText(current.text, next.text);
  }
  return current.type === "error" && next.type === "error"
    ? hasContinuousText(current.text, next.text)
    : false;
}

function mergeAssistantMessagePart(
  current: ChatMessagePart,
  next: ChatMessagePart,
): ChatMessagePart {
  if (current.type !== next.type) {
    return next;
  }
  switch (current.type) {
    case "toolCall": {
      if (next.type !== "toolCall") {
        return next;
      }
      return {
        type: "toolCall",
        toolCall: mergeToolCallUpdate(current.toolCall, next.toolCall),
      };
    }
    case "contextCompression":
      if (next.type !== "contextCompression") {
        return next;
      }
      return mergeContextCompressionPart(current, next);
    case "agentTaskLifecycle":
      if (next.type !== "agentTaskLifecycle") {
        return next;
      }
      return {
        type: "agentTaskLifecycle",
        lifecycle: {
          ...current.lifecycle,
          ...next.lifecycle,
          durationMs: next.lifecycle.durationMs ?? current.lifecycle.durationMs,
          resultJson: next.lifecycle.resultJson ?? current.lifecycle.resultJson,
          resultPreview:
            next.lifecycle.resultPreview ?? current.lifecycle.resultPreview,
          errorPreview: next.lifecycle.errorPreview ?? current.lifecycle.errorPreview,
        },
      };
    case "userInterruption":
      if (next.type !== "userInterruption") {
        return next;
      }
      return {
        ...current,
        ...next,
        interruptedAssistantMetrics:
          next.interruptedAssistantMetrics ??
          current.interruptedAssistantMetrics ??
          null,
      };
    case "attachment":
      if (next.type !== "attachment") {
        return next;
      }
      return {
        type: "attachment",
        attachment: { ...current.attachment, ...next.attachment },
      };
    case "reasoning": {
      if (next.type !== "reasoning") {
        return next;
      }
      const durationMs = next.durationMs ?? current.durationMs;
      return {
        type: "reasoning",
        text: mergeAssistantStreamText(current.text, next.text),
        ...(durationMs === undefined ? {} : { durationMs }),
        ...(durationMs === undefined
          ? {
              liveDurationMs: Math.max(
                current.liveDurationMs ?? 0,
                next.liveDurationMs ?? 0,
              ),
              startedAtMs: current.startedAtMs ?? next.startedAtMs,
            }
          : {}),
      };
    }
    case "text":
      return next.type === "text"
        ? { type: "text", text: mergeAssistantStreamText(current.text, next.text) }
        : next;
    case "error":
      return next.type === "error"
        ? { type: "error", text: mergeAssistantStreamText(current.text, next.text) }
        : next;
  }
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

function appendTextPart(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
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

/**
 * A tool-only Complete describes a fallback after the provider's final tool
 * boundary. A replay is allowed to confirm that same fallback, but must not
 * use an earlier ordinary text part as proof: the model may intentionally use
 * the same text in a different provider turn.
 */
function appendOrReplaceToolOnlyFinalFallback(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  let lastToolCallIndex = -1;
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    if (parts[index]?.type === "toolCall") {
      lastToolCallIndex = index;
      break;
    }
  }
  if (lastToolCallIndex < 0) {
    return appendTextPart(parts, text);
  }

  for (let index = parts.length - 1; index > lastToolCallIndex; index -= 1) {
    const part = parts[index];
    if (part?.type === "text") {
      return part.text === text
        ? parts
        : [...parts.slice(0, index), { ...part, text }, ...parts.slice(index + 1)];
    }
  }
  return [...parts, { type: "text", text }];
}

function replaceLastTextPart(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    const part = parts[index];
    if (part?.type !== "text") {
      continue;
    }
    return [
      ...parts.slice(0, index),
      { ...part, text },
      ...parts.slice(index + 1),
    ];
  }
  return text ? [...parts, { type: "text", text }] : parts;
}

function appendErrorPart(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
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

function upsertAgentTaskLifecyclePart(
  parts: ChatMessagePart[],
  lifecycle: ChatAgentTaskLifecycle,
): ChatMessagePart[] {
  const existingIndex = parts.findIndex(
    (part) =>
      part.type === "agentTaskLifecycle" &&
      part.lifecycle.eventId === lifecycle.eventId,
  );
  const nextPart: ChatMessagePart = { type: "agentTaskLifecycle", lifecycle };

  if (existingIndex === -1) {
    return [...parts, nextPart];
  }

  return parts.map((part, index) => (index === existingIndex ? nextPart : part));
}

function parseChatAgentTaskLifecycle(
  value: unknown,
): ChatAgentTaskLifecycle | null {
  if (!isObjectRecord(value)) {
    return null;
  }
  const eventId = stringField(value, "eventId", "event_id");
  const teamId = stringField(value, "teamId", "team_id");
  const taskId = stringField(value, "taskId", "task_id");
  const parentTaskId = stringField(value, "parentTaskId", "parent_task_id");
  const instanceId = stringField(value, "instanceId", "instance_id");
  const status = stringField(value, "status");
  const completedAt = stringField(value, "completedAt", "completed_at");
  const startedAt = optionalNullableStringField(value, "startedAt", "started_at");
  const durationMs = fieldValue(value, "durationMs", "duration_ms");
  const rawResultJson = fieldValue(value, "resultJson", "result_json");
  const resultJson = isJsonValue(rawResultJson) ? rawResultJson : null;
  const resultPreview = optionalNullableStringField(
    value,
    "resultPreview",
    "result_preview",
  );
  const errorPreview = optionalNullableStringField(
    value,
    "errorPreview",
    "error_preview",
  );
  if (
    !eventId || !teamId || !taskId || !parentTaskId || !instanceId || !status ||
    !completedAt || startedAt === false || resultPreview === false ||
    errorPreview === false ||
    (durationMs !== undefined &&
      durationMs !== null &&
      typeof durationMs !== "number")
  ) {
    return null;
  }
  return {
    eventId, teamId, taskId, parentTaskId, instanceId, status, completedAt,
    startedAt: startedAt ?? null,
    durationMs: typeof durationMs === "number" ? durationMs : null,
    resultJson,
    resultPreview: resultPreview ?? null,
    errorPreview: errorPreview ?? null,
  };
}

function applyToolResultToParts(
  parts: ChatMessagePart[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
  startedAt?: string | null,
  completedAt?: string | null,
  terminal = true,
): ChatMessagePart[] {
  return parts.map((part) => {
    if (part.type !== "toolCall" || part.toolCall.id !== toolCallId) {
      return part;
    }
    if (
      !terminal &&
      (part.toolCall.status === "completed" ||
        part.toolCall.status === "error" ||
        part.toolCall.status === "cancelled")
    ) {
      return part;
    }
    return {
      type: "toolCall",
      toolCall: {
        ...part.toolCall,
        output,
        isError,
        status: terminal ? (isError ? "error" : "completed") : "running",
        startedAt: startedAt ?? part.toolCall.startedAt ?? null,
        completedAt: terminal
          ? (completedAt ?? part.toolCall.completedAt ?? null)
          : null,
        liveOutput: undefined,
      },
    } satisfies ChatMessagePart;
  });
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
      if (part.type === "agentTaskLifecycle") {
        return `agent task ${part.lifecycle.status}`.trim();
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
    // This is a UI protocol rather than an ordinary tool error. Keep its
    // structured marker intact across live updates and history normalization.
    output: isToolCallLoopGuardBlockedPayload(toolCall.output)
      ? toolCall.output
      : (toolCall.output === null ? null : normalizedJsonValue(toolCall.output)),
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

function toolCallChangeStats(
  toolCall: ChatToolCallSummary,
): ToolCallChangeStats | null {
  if (
    toolCall.isError ||
    (toolCall.name !== "edit_file" &&
      toolCall.name !== "write_file" &&
      toolCall.name !== "apply_patch")
  ) {
    return null;
  }
  if (toolCall.output === null || !isObjectRecord(toolCall.output)) {
    return null;
  }

  const linesAdded = numericField(toolCall.output, "linesAdded", "lines_added");
  const linesRemoved = numericField(
    toolCall.output,
    "linesRemoved",
    "lines_removed",
  );
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

  if (
    toolCall.name === "get_command_output" ||
    toolCall.name === "stop_command"
  ) {
    const processId = textField(input, "processId");
    if (processId) {
      return compactToolText(processId);
    }
  }

  if (toolCall.name === "run_command") {
    const command = textField(input, "command");
    const args = stringArrayField(input, "args") ?? [];
    const cwd = textField(input, "cwd");

    if (command) {
      const fullCommand = [command, ...args].map(formatCommandPart).join(" ");
      return compactToolText(
        cwd && cwd !== "." ? `${fullCommand} | cwd: ${cwd}` : fullCommand,
      );
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
  const pathIndex = parts.findIndex(
    (part) => part === textField(input, "path"),
  );
  const startLine = numberTextField(input, "startLine", "start_line");
  const endLine = numberTextField(input, "endLine", "end_line");

  if (pathIndex !== -1 && startLine && endLine) {
    parts[pathIndex] = `${parts[pathIndex]}:${startLine}-${endLine}`;
  }

  return parts.length
    ? compactToolText(parts.join(" | "))
    : compactToolJson(input);
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

function textField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "string" ? field : null;
}

function numberTextField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "number" ? String(field) : null;
}

function numericField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "number" && Number.isSafeInteger(field) && field >= 0
    ? field
    : null;
}

function stringArrayField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
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

  return /^[A-Za-z0-9_./:=@%+,\-\\]+$/.test(value)
    ? value
    : JSON.stringify(value);
}

function compactToolJson(value: JsonValue) {
  return compactToolText(JSON.stringify(value));
}

function compactToolText(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 240
    ? `${normalized.slice(0, 237)}…`
    : normalized;
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
    statistics ??
    emptyChatStatistics(workspaceId, chatId, emptyAiStatisticsSummary());
  const totalRequests = base.totalRequests + 1;
  const totalLatencyMs = base.totalLatencyMs + liveLatencyMs;
  const messageCount = messages.length || base.messageCount;
  const userMessageCount =
    countMessagesByRole(messages, "user") || base.userMessageCount;
  const assistantMessageCount =
    countMessagesByRole(messages, "assistant") || base.assistantMessageCount;
  const toolMessageCount =
    countMessagesByRole(messages, "tool") || base.toolMessageCount;

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

export function contextUsageWithLatestProviderUsage(
  usage: ContextUsageResponse,
  latestProviderUsage: LiveChatStatistics | null,
): ContextUsageResponse {
  const inputTokens = latestProviderUsage?.usage?.inputTokens;
  if (
    typeof inputTokens !== "number" ||
    inputTokens < 0 ||
    usage.contextWindow <= 0 ||
    !sameModelRoute(usage, latestProviderUsage)
  ) {
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

export function composerContextUsageRefreshAction({
  hasPendingSkip,
  isSendingMessage,
  matchesCurrentIdentity,
}: {
  hasPendingSkip: boolean;
  isSendingMessage: boolean;
  matchesCurrentIdentity: boolean;
}): "record-skip" | "refresh" | "unchanged" {
  if (isSendingMessage) {
    return matchesCurrentIdentity ? "unchanged" : "record-skip";
  }

  // Consume an old skip marker before checking identity. A switch away from
  // and back to the run route can otherwise leave the marker behind and
  // suppress a later, unrelated composer refresh.
  if (hasPendingSkip || matchesCurrentIdentity) {
    return "unchanged";
  }

  return "refresh";
}

function sameModelRoute(
  usage: Pick<ContextUsageResponse, "modelId" | "providerId">,
  live: Pick<LiveChatStatistics, "modelId" | "providerId"> | null,
): boolean {
  return Boolean(
    usage.modelId &&
      usage.providerId &&
      live?.modelId &&
      live.providerId &&
      usage.modelId === live.modelId &&
      usage.providerId === live.providerId,
  );
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
  for (const key of [
    "history",
    "compressionSnapshot",
    "toolSchema",
    "systemPrompt",
  ] as const) {
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
  const base = emptyChatStatistics(
    workspaceId,
    chatId,
    emptyAiStatisticsSummary(),
  );
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
        right.callCount - left.callCount ||
        left.toolName.localeCompare(right.toolName),
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
    year:
      date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
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
  const phase = plan.phases.find(
    (candidate) => candidate.id === target.phaseId,
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
    left.phaseId === right.phaseId
  );
}

function chatTitleForDraft(
  content: string,
  attachments: ChatAttachmentPayload[],
) {
  const normalized = content.trim().replace(/\s+/g, " ");
  if (normalized) {
    return normalized.length > 48
      ? `${normalized.slice(0, 48)}…`
      : normalized;
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

type ChatStreamTermination = "complete" | "streamEnd" | "eof";

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
  const configured = (
    globalThis as {
      __FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__?: unknown;
    }
  ).__FOCO_TEST_CHAT_STREAM_IDLE_TIMEOUT_MS__;
  return typeof configured === "number" && configured > 0
    ? configured
    : CHAT_STREAM_IDLE_TIMEOUT_MS;
}

async function readChatStream(
  response: Response,
  onEvent: (event: ChatStreamEvent, meta: ChatStreamFrameMeta) => void,
  options: { idleTimeoutMs?: number; signal?: AbortSignal } = {},
): Promise<ChatStreamTermination> {
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
        return "complete";
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
    return "streamEnd";
  }

  buffer += decoder.decode();
  readSseFrames(`${buffer}\n\n`, handleEvent);
  return shouldStopReading
    ? "streamEnd"
    : sawCompletionEvent
      ? "complete"
      : "eof";
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
    const id =
      lines
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

export function activeRunIdFromStartEvent(event: unknown): string | null {
  const value = isObjectRecord(event) ? event : {};
  const runId = typeof value.runId === "string" ? value.runId : null;
  const llmRequestId =
    typeof value.llmRequestId === "string" ? value.llmRequestId : null;
  // Remote `runId` remains stable for the whole chat run. The legacy local
  // start field is a fallback only; individual provider attempts never reach here.
  return runId ?? llmRequestId;
}

export function parseChatStreamEvent(value: unknown): ChatStreamEvent | null {
  if (!isObjectRecord(value) || typeof value.type !== "string") {
    return null;
  }

  if (isObjectRecord(value.value) && typeof value.value.type !== "string") {
    return parseChatStreamEvent({ ...value.value, type: value.type });
  }

  if (value.type === "start") {
    const chatId = stringField(value, "chatId", "chat_id");
    const userMessageId = stringField(
      value,
      "userMessageId",
      "user_message_id",
    );
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const runId = optionalStringField(value, "runId", "run_id");
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
      runId === null ||
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
      runId,
      llmRequestId,
      memoriesUsed,
    };
  }

  if (value.type === "connecting") {
    const message = optionalStringField(value, "message");
    if (message === null) {
      return null;
    }

    return message === undefined
      ? { type: "connecting" }
      : { type: "connecting", message };
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

    if (
      assistantMessageId === null ||
      delta === null ||
      reasoningDurationMs === false
    ) {
      return null;
    }

    return {
      type: "textDelta",
      assistantMessageId,
      delta,
      reasoningDurationMs,
    };
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

  if (
    value.type === "agentTaskLifecycle" ||
    value.type === "agent_task_lifecycle"
  ) {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const lifecycle = parseChatAgentTaskLifecycle(fieldValue(value, "lifecycle"));
    if (!assistantMessageId || !lifecycle) {
      return null;
    }
    return { type: "agentTaskLifecycle", assistantMessageId, lifecycle };
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
    const hasFinalTextSegment = fieldValue(
      value,
      "hasFinalTextSegment",
      "has_final_text_segment",
    );
    const finalTextSegment = fieldValue(
      value,
      "finalTextSegment",
      "final_text_segment",
    );
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

    if (
      !chatId ||
      !assistantMessageId ||
      text === null ||
      (hasFinalTextSegment !== undefined &&
        typeof hasFinalTextSegment !== "boolean") ||
      (finalTextSegment !== undefined &&
        finalTextSegment !== null &&
        typeof finalTextSegment !== "string")
    ) {
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
      ...(typeof hasFinalTextSegment === "boolean"
        ? { hasFinalTextSegment }
        : {}),
      ...(finalTextSegment === null || typeof finalTextSegment === "string"
        ? { finalTextSegment }
        : {}),
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

    return {
      type: "toolCall",
      assistantMessageId,
      reasoningDurationMs,
      toolCall,
    };
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
    const terminal = fieldValue(value, "terminal");
    const startedAt = optionalNullableStringField(
      value,
      "startedAt",
      "started_at",
    );
    const completedAt = optionalNullableStringField(
      value,
      "completedAt",
      "completed_at",
    );

    if (
      !assistantMessageId ||
      !toolCallId ||
      !isJsonValue(output) ||
      typeof isError !== "boolean" ||
      (terminal !== undefined && typeof terminal !== "boolean") ||
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
      terminal: terminal !== false,
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

  if (value.type === "hookNotification" || value.type === "hook_notification") {
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

  if (value.type === "guidanceApplied" || value.type === "guidance_applied") {
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

function parseQuestionRequestSummary(
  value: unknown,
): QuestionRequestSummary | null {
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

function parseQuestionOptionSummary(
  value: unknown,
): QuestionOptionSummary | null {
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
  const compressionId = optionalNullableStringField(
    value,
    "compressionId",
    "compression_id",
  );
  const snapshotId = optionalNullableStringField(
    value,
    "snapshotId",
    "snapshot_id",
  );
  const startedAt = optionalNullableStringField(
    value,
    "startedAt",
    "started_at",
  );
  const completedAt = optionalNullableStringField(
    value,
    "completedAt",
    "completed_at",
  );
  const providerId = optionalNullableStringField(
    value,
    "providerId",
    "provider_id",
  );
  const modelId = optionalNullableStringField(value, "modelId", "model_id");
  const providerRequestId = optionalNullableStringField(
    value,
    "providerRequestId",
    "provider_request_id",
  );
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
  const compressionMode = optionalNullableStringField(
    value,
    "compressionMode",
    "compression_mode",
  );
  const attemptIndex = optionalNumberField(value, "attemptIndex", "attempt_index");
  const outcome = optionalNullableStringField(value, "outcome");
  const action = optionalNullableStringField(value, "action");
  const errorMessage = optionalNullableStringField(
    value,
    "errorMessage",
    "error_message",
  );

  if (
    !kind ||
    status === null ||
    compressionId === false ||
    snapshotId === false ||
    startedAt === false ||
    completedAt === false ||
    providerId === false ||
    modelId === false ||
    providerRequestId === false ||
    originalTokenCount === false ||
    summaryTokenCount === false ||
    compressionMode === false ||
    attemptIndex === false ||
    outcome === false ||
    action === false ||
    errorMessage === false ||
    (compressionMode !== undefined &&
      compressionMode !== null &&
      compressionMode !== "normal" &&
      compressionMode !== "required_overflow")
  ) {
    return false;
  }

  return normalizedContextCompressionDetail({
    ...(status ? { status } : {}),
    kind,
    compressionId: compressionId ?? null,
    snapshotId: snapshotId ?? null,
    originalTokenCount: originalTokenCount ?? null,
    summaryTokenCount: summaryTokenCount ?? null,
    startedAt: startedAt ?? null,
    completedAt: completedAt ?? null,
    providerId: providerId ?? null,
    modelId: modelId ?? null,
    providerRequestId: providerRequestId ?? null,
    compressionMode: compressionMode ?? null,
    attemptIndex: attemptIndex ?? null,
    outcome: outcome ?? null,
    action: action ?? null,
    errorMessage: errorMessage ?? null,
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
  const startedAt = optionalNullableStringField(
    value,
    "startedAt",
    "started_at",
  );
  const completedAt = optionalNullableStringField(
    value,
    "completedAt",
    "completed_at",
  );

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

function parseChatSpecUpdateSummary(
  value: unknown,
): ChatSpecUpdateSummary | null {
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

/**
 * Whether a stream `start` for an existing assistant bubble should keep prior
 * content (Coordinator cross-attempt / same-bubble resume) instead of wiping it.
 *
 * Uses chat-scoped live event tracking so GET reattach after wait still preserves
 * history, while first attach (persisted fallback only, no local live events)
 * continues to clear the draft before new deltas arrive.
 */
function shouldPreserveAssistantHistoryOnStart(
  hasSeenLiveEventsForAssistant: boolean,
): boolean {
  return hasSeenLiveEventsForAssistant;
}

function mergeAssistantMessageOnStreamStart(
  message: ShellMessage,
  memoriesUsed: ChatMemoryUsedSummary[],
  preserveHistory: boolean,
): ShellMessage {
  if (preserveHistory) {
    return {
      ...message,
      memoriesUsed: message.memoriesUsed.length
        ? message.memoriesUsed
        : memoriesUsed,
      status: "streaming",
    };
  }
  return {
    ...message,
    content: "",
    reasoning: null,
    toolCalls: [],
    parts: [],
    metrics: null,
    memoriesUsed: memoriesUsed.length ? memoriesUsed : message.memoriesUsed,
    status: "streaming",
  };
}

function contextUsageInputFromRunMessage(
  activeRun: ActiveChatRunSummary,
  messages: ShellMessage[],
): {
  modelId: string;
  providerId: string;
  thinkingLevel: string;
  skillIds: string[];
} | null {
  const queuedUserMessageId = activeRun.queuedUserMessageId;
  const runConfig = queuedUserMessageId
    ? messages.find((message) => message.id === queuedUserMessageId)?.runConfig
    : null;
  if (
    !runConfig ||
    !runConfig.modelId ||
    !runConfig.providerId
  ) {
    return null;
  }

  return {
    modelId: runConfig.modelId,
    providerId: runConfig.providerId,
    thinkingLevel: runConfig.thinkingLevel ?? "",
    skillIds: normalizeStringArray(runConfig.selectedSkillIds),
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
  const assistantMessageId = stringField(
    value,
    "assistantMessageId",
    "assistant_message_id",
  );
  const assistantSequenceValue = fieldValue(
    value,
    "assistantSequence",
    "assistant_sequence",
  );
  const queuedUserMessageId = stringField(
    value,
    "queuedUserMessageId",
    "queued_user_message_id",
  );
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
    assistantMessageId: assistantMessageId ?? null,
    assistantSequence:
      typeof assistantSequenceValue === "number"
        ? assistantSequenceValue
        : null,
    queuedUserMessageId: queuedUserMessageId ?? null,
    acceptingGuidance: acceptingGuidanceValue === true,
  };
}

function normalizeChatMessageStatus(
  value: unknown,
): "error" | "streaming" | undefined {
  if (value === "error" || value === "failed") {
    return "error";
  }
  return value === "streaming" ? value : undefined;
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
  const sessionMode: "plan" | null = rawSessionMode === "plan" ? "plan" : null;
  const status = normalizeChatMessageStatus(fieldValue(message, "status"));
  const queuedRun = normalizeQueuedMessageRunSummary(message.queuedRun);
  const runConfigValue = fieldValue(message, "runConfig", "run_config");
  const runConfigRecord =
    runConfigValue && typeof runConfigValue === "object"
      ? (runConfigValue as Record<string, unknown>)
      : null;
  const runConfig = runConfigRecord
    ? {
        modelId: String(
          fieldValue(runConfigRecord, "modelId", "model_id") ?? "",
        ),
        providerId:
          typeof fieldValue(runConfigRecord, "providerId", "provider_id") ===
          "string"
            ? String(fieldValue(runConfigRecord, "providerId", "provider_id"))
            : null,
        thinkingLevel:
          typeof fieldValue(
            runConfigRecord,
            "thinkingLevel",
            "thinking_level",
          ) === "string"
            ? String(
                fieldValue(runConfigRecord, "thinkingLevel", "thinking_level"),
              )
            : null,
        latencyMode: latencyModeFromValue(
          fieldValue(runConfigRecord, "latencyMode", "latency_mode"),
        ),
        selectedSkillIds: Array.isArray(
          fieldValue(runConfigRecord, "selectedSkillIds", "selected_skill_ids"),
        )
          ? (
              fieldValue(
                runConfigRecord,
                "selectedSkillIds",
                "selected_skill_ids",
              ) as unknown[]
            ).filter((value): value is string => typeof value === "string")
          : [],
        sessionMode:
          fieldValue(runConfigRecord, "sessionMode", "session_mode") === "plan"
            ? ("plan" as const)
            : null,
        teamModeEnabled:
          fieldValue(
            runConfigRecord,
            "teamModeEnabled",
            "team_mode_enabled",
          ) === true,
      }
    : null;
  const statusFromParts =
    !status && parts.some((part) => part.type === "error")
      ? ("error" as const)
      : status;
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
    status: statusFromParts,
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
  const hasMoreBefore =
    fieldValue(value, "hasMoreBefore", "has_more_before") === true;
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
  const thinkingLevel = fieldValue(
    queuedRun,
    "thinkingLevel",
    "thinking_level",
  );
  const latencyMode = fieldValue(queuedRun, "latencyMode", "latency_mode");
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
  const sessionMode: "plan" | null = rawSessionMode === "plan" ? "plan" : null;

  return {
    status: typeof status === "string" ? status : "queued",
    modelId,
    providerId: typeof providerId === "string" ? providerId : null,
    thinkingLevel: typeof thinkingLevel === "string" ? thinkingLevel : null,
    latencyMode: latencyModeFromValue(latencyMode),
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
    const liveDurationMs = fieldValue(
      part,
      "liveDurationMs",
      "live_duration_ms",
    );
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

  if (
    part.type === "contextCompression" ||
    part.type === "context_compression"
  ) {
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
    const id =
      stringField(part, "id") ??
      contextCompressionPartId(kind, normalizedDetail);
    return {
      type: "contextCompression",
      id,
      status,
      kind,
      detail: normalizedDetail,
    };
  }

  if (
    part.type === "agentTaskLifecycle" ||
    part.type === "agent_task_lifecycle"
  ) {
    const lifecycle = parseChatAgentTaskLifecycle(fieldValue(part, "lifecycle"));
    return lifecycle ? { type: "agentTaskLifecycle", lifecycle } : null;
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
      ...(interruptedAssistantMetrics ? { interruptedAssistantMetrics } : {}),
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

function parseNullableChatUsage(
  value: unknown,
): ChatUsage | null | undefined | false {
  if (value === null) {
    return null;
  }

  return parseChatUsage(value);
}

function parseRequiredChatReplyMetrics(
  value: unknown,
): ChatReplyMetrics | false {
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
  const cacheReadTokens = fieldValue(
    value,
    "cacheReadTokens",
    "cache_read_tokens",
  );
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

  const modelId = optionalStringField(value, "modelId", "model_id");
  const providerId = optionalStringField(value, "providerId", "provider_id");
  if (modelId === null || providerId === null) {
    return false;
  }

  return {
    inputTokens,
    outputTokens,
    cacheReadTokens,
    cacheWriteTokens,
    ...(modelId === undefined ? {} : { modelId }),
    ...(providerId === undefined ? {} : { providerId }),
  };
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
  return typeof field === "undefined" || typeof field === "string"
    ? field
    : null;
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
