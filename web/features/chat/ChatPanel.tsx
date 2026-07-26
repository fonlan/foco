import {
  ArrowUp,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Copy,
  FileSearch,
  FileText,
  GitBranch,
  Globe,
  ListChecks,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCw,
  Send,
  Server,
  Shrink,
  SlidersHorizontal,
  Search,
  Terminal,
  User,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  CSSProperties,
  ClipboardEvent as ReactClipboardEvent,
  FormEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  WheelEvent as ReactWheelEvent,
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import {
  isToolCallLoopGuardBlockedPayload,
  type ToolCallLoopGuardBlockedPayload,
} from "../../api/types";
import type {
  ChatAttachmentPartSummary,
  ChatExtractedMemorySummary,
  ChatMemoryUsedSummary,
  ChatMessagePart,
  ChatReplyMetrics,
  ChatSpecUpdateSummary,
  ChatToolCallSummary,
  ChatToolLiveOutput,
  ComposerAttachment,
  ConfiguredModelSummary,
  ConfiguredSkillSummary,
  ContextUsageResponse,
  JsonValue,
  SettingsResponse,
  ShellMessage,
  ThinkingLevelSummary,
  Translate,
  WorkspaceSummary,
} from "../../api/types";
import { CHAT_BOTTOM_LOCK_THRESHOLD_PX } from "../../app/constants";
import { useI18n } from "../../shared/i18n";
import {
  Button,
  Chip,
  Description,
  Label,
  ListBox,
  Select,
  TextArea,
  TextField,
} from "../../shared/ui";
import { forwardWheelAtVerticalBoundary } from "../../shared/scroll-forwarding";
import { thinkingLevelOptionsForModel } from "../../shared/thinking-levels";
import { selectedSkillPrefix, toolDisplayName } from "./chat-helpers";
import {
  MarkdownContent,
  type SelectedSkillPrefixResolver,
} from "./MarkdownContent";
import type { WorkspaceSkillCatalogStatus } from "./use-workspace-skill-catalog";

const COMPOSER_EDITOR_MIN_HEIGHT_PX = 68;
const COMPOSER_EDITOR_KEY_STEP_PX = 24;
const COMPOSER_EDITOR_MAX_HEIGHT_RATIO = 0.55;
const CHAT_TOP_LOAD_THRESHOLD_PX = 64;
/** How long an upward input keeps auto-loading earlier history enabled. */
const UPWARD_HISTORY_INTENT_TTL_MS = 600;
const MAX_GENERATED_IMAGE_PREVIEWS = 16;
const TOOL_CALL_SCROLL_CLASS = "tool-call-scroll panel-scroll";

const TOOL_CALL_ICONS: Record<string, LucideIcon> = {
  agent_cancel_task: Server,
  agent_create_instances: Server,
  agent_delegate_task: Server,
  agent_get_task: Server,
  agent_list: Server,
  agent_send_message: Server,
  agent_transfer_task: Server,
  agent_wait_tasks: Server,
  create_plan: ListChecks,
  create_todo_graph: ListChecks,
  update_todo_graph: ListChecks,
  delete_plan: ListChecks,
  edit_file: FileText,
  finance: Globe,
  find: Search,
  find_files: Search,
  get_plans: ListChecks,
  get_todo_graph: ListChecks,
  git_branch: GitBranch,
  git_status: GitBranch,
  graph_explore: Search,
  graph_find_callees: Search,
  graph_find_callers: Search,
  graph_find_children: Search,
  graph_find_importers: Search,
  graph_find_imports: Search,
  graph_find_references: Search,
  graph_find_symbols: Search,
  graph_related_files: Search,
  image_gen: FileText,
  image_query: Search,
  memory_search: Brain,
  memory_write: Brain,
  "mcp__context7__query-docs": Globe,
  "mcp__context7__resolve-library-id": Globe,
  open: Globe,
  read_file: FileText,
  read_spec: FileText,
  get_command_output: RefreshCw,
  stop_command: X,
  run_command: Terminal,
  screenshot: Globe,
  search_query: Search,
  search_text: FileSearch,
  sports: Globe,
  time: Globe,
  update_plan: ListChecks,
  update_plan_step: ListChecks,
  update_spec: FileText,
  weather: Globe,
  web_fetch: Globe,
  write_file: FileText,
};

type ToolCallChangeStats = {
  linesAdded: number;
  linesRemoved: number;
};

type GeneratedImageFile = {
  bytes: number | null;
  mimeType: string | null;
  path: string;
};

type ComposerResizeDrag = {
  maxHeight: number;
  startHeight: number;
  startY: number;
};

export type ChatPanelHelpers = {
  activeSkillQuery: (value: string) => string | null;
  compactInlineText: (value: string) => string;
  compactToolJson: (value: JsonValue) => string;
  fallbackMessageParts: (message: ShellMessage) => ChatMessagePart[];
  formatChatCreatedAt: (value: string) => string;
  formatFileSize: (sizeBytes: number) => string;
  formatJsonValue: (value: JsonValue) => string;
  formatNullableLatencySeconds: (
    value: number | null,
    language: string,
  ) => string;
  formatReplyDuration: (value: number | null, language: string) => string;
  formatTokensPerSecond: (
    metrics: ChatReplyMetrics,
    language: string,
  ) => string;
  messageCopyText: (message: ShellMessage, parts: ChatMessagePart[]) => string;
  removeActiveSkillToken: (value: string) => string;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  skillScopeLabel: (skill: ConfiguredSkillSummary, t: Translate) => string;
  toolCallChangeStats: (
    toolCall: ChatToolCallSummary,
  ) => ToolCallChangeStats | null;
  normalizedToolInput: (value: JsonValue) => JsonValue;
  toolCallDetailText: (toolCall: ChatToolCallSummary) => string;
  toolLiveOutputText: (
    liveOutput: ChatToolLiveOutput | undefined,
  ) => string | null;
  toolStatusText: (toolCall: ChatToolCallSummary, t: Translate) => string;
};

function ChatPanelComponent({
  activeWorkspaceName,
  availableModels,
  chatScrollKey,
  canGuideActiveRun,
  canRetryRun,
  contextUsage,
  draftAttachments,
  draftMessage,
  draftUnsupportedAttachmentMessage,
  hasMoreMessagesBefore,
  helpers,
  queuedRunCount,
  readOnly,
  isLoadingContextUsage,
  isLoadingMoreMessages,
  isLoadingMessages,
  isLoadingSettings,
  isSendingMessage,
  isSelectingAttachments,
  isPlanModeEnabled,
  messages,
  onAddPastedImageAttachments,
  overviewRenderer,
  onCancelRun,
  onDraftMessageChange,
  onEditMessage,
  onGuideActiveRun,
  onSelectEditAttachments,
  onGuideQueuedMessage,
  onLoadMoreMessages,
  onModelChange,
  onOpenMessageApiRequests,
  onQueueActiveRun,
  onRemoveAttachment,
  onRemoveSkill,
  onRetryRun,
  onSelectAttachments,
  onSubmit,
  onPlanModeEnabledChange,
  onThinkingLevelChange,
  onToggleSkill,
  onWithdrawQueuedMessage,
  selectedModelId,
  selectedSkillIds,
  selectedThinkingLevel,
  settings,
  skillCatalogError,
  skillCatalogRefreshError,
  skillCatalogStatus,
  skills,
  queuedMessageIds,
  thinkingLevels,
  workspaces,
  workspaceId,
}: {
  activeWorkspaceName: string | null;
  availableModels: ConfiguredModelSummary[];
  chatScrollKey: string;
  canGuideActiveRun: boolean;
  canRetryRun: boolean;
  contextUsage: ContextUsageResponse | null;
  draftAttachments: ComposerAttachment[];
  draftMessage: string;
  draftUnsupportedAttachmentMessage: string | null;
  hasMoreMessagesBefore: boolean;
  helpers: ChatPanelHelpers;
  queuedRunCount: number;
  readOnly: boolean;
  isLoadingContextUsage: boolean;
  isLoadingMoreMessages: boolean;
  isLoadingMessages: boolean;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  isSelectingAttachments: boolean;
  isPlanModeEnabled: boolean;
  messages: ShellMessage[];
  onAddPastedImageAttachments: (files: File[]) => void;
  overviewRenderer: () => ReactNode;
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onEditMessage: (
    message: ShellMessage,
    content: string,
    selectedSkillIds: string[],
    attachments: ComposerAttachment[],
    onAccepted: () => void,
  ) => Promise<boolean>;
  onSelectEditAttachments: (
    onSelected: (attachments: ComposerAttachment[]) => void,
  ) => void;
  onGuideActiveRun: () => void;
  onGuideQueuedMessage: (messageId: string) => void;
  onLoadMoreMessages: () => Promise<void>;
  onModelChange: (value: string) => void;
  onOpenMessageApiRequests: (message: ShellMessage) => void;
  onQueueActiveRun: () => void;
  onRemoveAttachment: (attachmentId: string) => void;
  onRemoveSkill: (skillId: string) => void;
  onRetryRun: () => void;
  onSelectAttachments: () => void;
  onSubmit: (
    event: FormEvent<HTMLFormElement>,
    options?: { schedule?: boolean },
  ) => void;
  onPlanModeEnabledChange: (value: boolean) => void;
  onThinkingLevelChange: (value: string) => void;
  onToggleSkill: (skillId: string) => void;
  onWithdrawQueuedMessage: (messageId: string) => void;
  selectedModelId: string;
  selectedSkillIds: string[];
  selectedThinkingLevel: string;
  settings: SettingsResponse | null;
  skillCatalogError: string | null;
  skillCatalogRefreshError: string | null;
  skillCatalogStatus: WorkspaceSkillCatalogStatus;
  skills: ConfiguredSkillSummary[];
  queuedMessageIds: ReadonlySet<string>;
  thinkingLevels: ThinkingLevelSummary[];
  workspaces: WorkspaceSummary[];
  workspaceId: string | null;
}) {
  const { activeSkillQuery, removeActiveSkillToken, skillScopeLabel } = helpers;
  const { t } = useI18n();
  const chatPanelRef = useRef<HTMLDivElement>(null);
  const messageScrollRef = useRef<HTMLDivElement>(null);
  const messageScrollContentRef = useRef<HTMLDivElement>(null);
  const messageTextareaRef = useRef<HTMLTextAreaElement>(null);
  const composerResizeDragRef = useRef<ComposerResizeDrag | null>(null);
  const copiedMessageTimerRef = useRef<number | null>(null);
  const pendingPrependScrollHeightRef = useRef<number | null>(null);
  const previousChatScrollKeyRef = useRef(chatScrollKey);
  const previousMessageCountRef = useRef(messages.length);
  const shouldLockMessageScrollRef = useRef(true);
  const userMessageScrollIntentRef = useRef(false);
  // High-frequency scroll/intent state stays in refs so wheel/scroll handlers avoid setState.
  const lastMessageScrollTopRef = useRef(0);
  const lastUpwardHistoryIntentAtRef = useRef(0);
  // True while a primary pointer is pressed on the list (scrollbar drag / touch).
  // Cleared via element + window pointerup/cancel so release outside cannot stick.
  // Do not setPointerCapture on the list: capture retargets click away from nested
  // interactive UI (native <details>/<summary>, buttons, links).
  const pointerHistoryGestureActiveRef = useRef(false);
  const activeHistoryPointerIdRef = useRef<number | null>(null);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [editingMessageText, setEditingMessageText] = useState("");
  const [editingSkillIds, setEditingSkillIds] = useState<string[]>([]);
  const [editingAttachments, setEditingAttachments] = useState<
    ComposerAttachment[]
  >([]);
  const [isSavingEditedMessage, setIsSavingEditedMessage] = useState(false);
  const [isCtrlKeyPressed, setIsCtrlKeyPressed] = useState(false);
  const [isResizingComposer, setIsResizingComposer] = useState(false);
  const [isSendButtonTooltipOpen, setIsSendButtonTooltipOpen] = useState(false);
  const [composerEditorHeight, setComposerEditorHeight] = useState(
    COMPOSER_EDITOR_MIN_HEIGHT_PX,
  );
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = useMemo(
    () => new Set(selectedSkillIds),
    [selectedSkillIds],
  );
  const selectedSkills = useMemo(
    () =>
      selectedSkillIds
        .map((skillId) => skills.find((skill) => skill.key === skillId))
        .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill)),
    [selectedSkillIds, skills],
  );
  const workspaceName = activeWorkspaceName?.trim();
  const composerPlaceholder = workspaceName
    ? t("Ask Foco anything about {name}…", { name: workspaceName })
    : t("Ask Foco anything…");
  const modelOptions = useMemo(
    () =>
      [...availableModels]
        .sort((left, right) =>
          left.displayName.localeCompare(right.displayName),
        )
        .map((model) => ({
          label: model.displayName,
          value: model.id,
        })),
    [availableModels, t],
  );
  const selectedModel = useMemo(
    () => availableModels.find((model) => model.id === selectedModelId) ?? null,
    [availableModels, selectedModelId],
  );
  const thinkingOptions = useMemo(
    () => [
      { label: t("Model default"), value: "" },
      ...thinkingLevelOptionsForModel(selectedModel, thinkingLevels).map(
        (level) => ({
          label: t(level.label),
          value: level.value,
        }),
      ),
    ],
    [selectedModel, thinkingLevels, t],
  );
  const visibleSkills = useMemo(
    () =>
      skillQuery === null
        ? []
        : skills.filter((skill) => {
            const query = skillQuery.toLowerCase();
            return (
              skill.enabled &&
              !selectedSkillSet.has(skill.key) &&
              (skill.name.toLowerCase().includes(query) ||
                skill.id.toLowerCase().includes(query) ||
                skill.key.toLowerCase().includes(query) ||
                skill.description.toLowerCase().includes(query))
            );
          }),
    [selectedSkillSet, skillQuery, skills],
  );
  const hasComposerDraft = Boolean(
    draftMessage.trim() || draftAttachments.length,
  );
  const runningButtonSendsMessage =
    isSendingMessage && hasComposerDraft && canGuideActiveRun;
  const runningButtonLabel = runningButtonSendsMessage
    ? t("Send guidance")
    : t("Cancel run");
  const runningButtonTitle = runningButtonSendsMessage
    ? isCtrlKeyPressed
      ? t("Send to queue")
      : queuedRunCount > 0
        ? t("Send guidance. Ctrl+click queues. {count} queued.", {
            count: queuedRunCount,
          })
        : t("Send guidance. Ctrl+click queues.")
    : t("Cancel run");
  const sendButtonTitle = draftUnsupportedAttachmentMessage
    ? draftUnsupportedAttachmentMessage
    : isCtrlKeyPressed
      ? t("Send to queue")
      : t("Send");
  const showSendButtonTooltip = isSendButtonTooltipOpen && !isSendingMessage;

  function scrollMessageListToBottom() {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }

    element.scrollTop = element.scrollHeight;
    window.requestAnimationFrame(() => {
      if (shouldLockMessageScrollRef.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
  }

  useLayoutEffect(() => {
    const previousScrollHeight = pendingPrependScrollHeightRef.current;
    if (previousScrollHeight === null) {
      return;
    }

    pendingPrependScrollHeightRef.current = null;
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }
    shouldLockMessageScrollRef.current = false;
    element.scrollTop += Math.max(
      0,
      element.scrollHeight - previousScrollHeight,
    );
    lastMessageScrollTopRef.current = element.scrollTop;
  }, [messages.length]);

  useLayoutEffect(() => {
    const element = messageScrollRef.current;
    const chatChanged = previousChatScrollKeyRef.current !== chatScrollKey;
    const wasEmpty = previousMessageCountRef.current === 0;
    previousChatScrollKeyRef.current = chatScrollKey;
    previousMessageCountRef.current = messages.length;

    if (chatChanged) {
      lastUpwardHistoryIntentAtRef.current = 0;
      pointerHistoryGestureActiveRef.current = false;
      activeHistoryPointerIdRef.current = null;
    }

    if (messages.length === 0) {
      shouldLockMessageScrollRef.current = false;
      if (element) {
        element.scrollTop = 0;
        lastMessageScrollTopRef.current = 0;
      }
      return;
    }

    if (chatChanged || wasEmpty) {
      shouldLockMessageScrollRef.current = true;
      scrollMessageListToBottom();
      lastMessageScrollTopRef.current = element?.scrollTop ?? 0;
    }
  }, [chatScrollKey, messages.length]);

  useLayoutEffect(() => {
    if (!shouldLockMessageScrollRef.current) {
      return;
    }

    scrollMessageListToBottom();
    const element = messageScrollRef.current;
    if (element) {
      lastMessageScrollTopRef.current = element.scrollTop;
    }
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

  useEffect(() => {
    if (!isResizingComposer) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const drag = composerResizeDragRef.current;
      if (!drag) {
        return;
      }

      setComposerEditorHeight(
        clampComposerEditorHeight(
          drag.startHeight + drag.startY - event.clientY,
          drag.maxHeight,
        ),
      );
    }

    function handlePointerUp() {
      composerResizeDragRef.current = null;
      setIsResizingComposer(false);
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };
  }, [isResizingComposer]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.ctrlKey) {
        setIsCtrlKeyPressed(true);
      }
    }

    function handleKeyUp(event: KeyboardEvent) {
      if (event.key === "Control" || !event.ctrlKey) {
        setIsCtrlKeyPressed(false);
      }
    }

    function handleWindowBlur() {
      setIsCtrlKeyPressed(false);
    }

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleWindowBlur);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleWindowBlur);
    };
  }, []);

  function composerEditorMaxHeight() {
    const panelHeight =
      chatPanelRef.current?.getBoundingClientRect().height ??
      window.innerHeight;
    // ponytail: one shared drag ceiling for desktop/mobile; split per breakpoint if UX needs it.
    return Math.max(
      COMPOSER_EDITOR_MIN_HEIGHT_PX,
      Math.floor(panelHeight * COMPOSER_EDITOR_MAX_HEIGHT_RATIO),
    );
  }

  function clampComposerEditorHeight(
    value: number,
    maxHeight = composerEditorMaxHeight(),
  ) {
    return Math.min(Math.max(value, COMPOSER_EDITOR_MIN_HEIGHT_PX), maxHeight);
  }

  function resizeComposerEditorBy(delta: number) {
    setComposerEditorHeight((current) =>
      clampComposerEditorHeight(current + delta),
    );
  }

  function handleComposerResizePointerDown(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    event.preventDefault();
    const startHeight =
      messageTextareaRef.current?.getBoundingClientRect().height ||
      composerEditorHeight;
    const maxHeight = composerEditorMaxHeight();
    composerResizeDragRef.current = {
      maxHeight,
      startHeight: clampComposerEditorHeight(startHeight, maxHeight),
      startY: event.clientY,
    };
    setComposerEditorHeight(composerResizeDragRef.current.startHeight);
    event.currentTarget.setPointerCapture(event.pointerId);
    setIsResizingComposer(true);
  }

  useEffect(() => {
    function handleWindowPointerEnd(event: PointerEvent) {
      clearPointerHistoryGesture(event.pointerId);
    }

    window.addEventListener("pointerup", handleWindowPointerEnd);
    window.addEventListener("pointercancel", handleWindowPointerEnd);
    return () => {
      window.removeEventListener("pointerup", handleWindowPointerEnd);
      window.removeEventListener("pointercancel", handleWindowPointerEnd);
    };
  }, []);

  function requestMoreMessages() {
    if (!hasMoreMessagesBefore || isLoadingMoreMessages) {
      return;
    }

    const element = messageScrollRef.current;
    pendingPrependScrollHeightRef.current = element?.scrollHeight ?? null;
    shouldLockMessageScrollRef.current = false;
    void onLoadMoreMessages();
  }

  function markUserMessageScrollIntent() {
    userMessageScrollIntentRef.current = true;
  }

  function markUpwardHistoryIntent() {
    lastUpwardHistoryIntentAtRef.current = performance.now();
    markUserMessageScrollIntent();
  }

  function hasRecentUpwardHistoryIntent() {
    return (
      performance.now() - lastUpwardHistoryIntentAtRef.current <=
      UPWARD_HISTORY_INTENT_TTL_MS
    );
  }

  function maybeLoadEarlierMessagesFromScroll(element: HTMLDivElement) {
    if (
      element.scrollTop <= CHAT_TOP_LOAD_THRESHOLD_PX &&
      hasMoreMessagesBefore &&
      !isLoadingMoreMessages &&
      hasRecentUpwardHistoryIntent()
    ) {
      requestMoreMessages();
    }
  }

  function handleMessageScroll() {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }

    if (messages.length === 0) {
      shouldLockMessageScrollRef.current = false;
      userMessageScrollIntentRef.current = false;
      lastMessageScrollTopRef.current = 0;
      return;
    }

    const nextScrollTop = element.scrollTop;
    const previousScrollTop = lastMessageScrollTopRef.current;
    const scrollTopDecreased = nextScrollTop < previousScrollTop;
    lastMessageScrollTopRef.current = nextScrollTop;

    // Mark upward pointer/touch drag before updating bottom-lock so the first
    // scroll away from the bottom unlocks and streaming cannot snap back.
    if (scrollTopDecreased && pointerHistoryGestureActiveRef.current) {
      markUpwardHistoryIntent();
    }

    const isAtBottom =
      element.scrollHeight - nextScrollTop - element.clientHeight <=
      CHAT_BOTTOM_LOCK_THRESHOLD_PX;
    if (isAtBottom) {
      shouldLockMessageScrollRef.current = true;
    } else if (scrollTopDecreased || userMessageScrollIntentRef.current) {
      // Native scrollbar drags can emit only `scroll`; actual upward movement
      // away from the bottom is therefore sufficient evidence to unlock.
      shouldLockMessageScrollRef.current = false;
    }
    userMessageScrollIntentRef.current = false;

    // Auto-load older history only when the user is actively scrolling upward
    // near the top (not from layout, streaming growth, or programmatic scrollTop).
    if (scrollTopDecreased && hasRecentUpwardHistoryIntent()) {
      maybeLoadEarlierMessagesFromScroll(element);
    }
  }

  function handleMessageListWheel(event: ReactWheelEvent<HTMLDivElement>) {
    if (event.deltaY < 0) {
      markUpwardHistoryIntent();
    } else {
      markUserMessageScrollIntent();
    }
  }

  function handleMessageListKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (
      event.key === "ArrowUp" ||
      event.key === "PageUp" ||
      event.key === "Home"
    ) {
      markUpwardHistoryIntent();
      return;
    }
    markUserMessageScrollIntent();
  }

  function clearPointerHistoryGesture(pointerId?: number) {
    if (
      pointerId !== undefined &&
      activeHistoryPointerIdRef.current !== null &&
      activeHistoryPointerIdRef.current !== pointerId
    ) {
      return;
    }
    pointerHistoryGestureActiveRef.current = false;
    activeHistoryPointerIdRef.current = null;
  }

  function handleMessageListPointerDown(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    if (event.pointerType === "mouse" && event.button !== 0) {
      return;
    }
    pointerHistoryGestureActiveRef.current = true;
    activeHistoryPointerIdRef.current = event.pointerId;
    // Window listeners clear the gesture when the pointer is released outside the list
    // (scrollbar drag, pan past the edge). Avoid setPointerCapture so nested
    // <details>/<summary>, buttons, and links keep receiving the full click sequence.
  }

  function handleMessageListPointerEnd(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    clearPointerHistoryGesture(event.pointerId);
  }

  function handleMessageListTouchStart() {
    pointerHistoryGestureActiveRef.current = true;
  }

  function handleMessageListTouchEnd() {
    // Touch pointers also emit pointer events when supported; keep touch handlers
    // as a fallback for environments that only deliver touch events.
    if (activeHistoryPointerIdRef.current === null) {
      pointerHistoryGestureActiveRef.current = false;
    }
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

  function isQueueModifierActive(event: { ctrlKey: boolean }) {
    return event.ctrlKey || isCtrlKeyPressed;
  }

  function handleRunningRunButtonClick(event: { ctrlKey: boolean }) {
    if (!runningButtonSendsMessage) {
      onCancelRun();
      return;
    }

    if (isQueueModifierActive(event)) {
      onQueueActiveRun();
      return;
    }

    onGuideActiveRun();
  }

  function handleModelSelect(modelId: string) {
    if (modelId !== selectedModelId) {
      onModelChange(modelId);
    }
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

  const handleCopyMessage = useCallback(
    async (messageId: string, text: string) => {
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
        setCopiedMessageId((current) =>
          current === messageId ? null : current,
        );
        copiedMessageTimerRef.current = null;
      }, 1600);
    },
    [],
  );

  const beginEditingMessage = useCallback(
    (message: ShellMessage) => {
      const persistedSkillIds = message.runConfig?.selectedSkillIds;
      const legacySelectedSkills = persistedSkillIds
        ? []
        : (selectedSkillPrefix(message.content, true)?.skills ?? []);
      const legacySkillIds = legacySelectedSkills
        .map(
          (selectedSkill) =>
            skills.find(
              (skill) =>
                skill.name === selectedSkill.name ||
                skill.path === selectedSkill.path,
            )?.key,
        )
        .filter((skillId): skillId is string => Boolean(skillId));
      setEditingMessageId(message.id);
      setEditingMessageText(message.content);
      setEditingSkillIds(persistedSkillIds ?? legacySkillIds);
      setEditingAttachments([]);
    },
    [skills],
  );

  const clearEditingMessage = useCallback(() => {
    setEditingMessageId(null);
    setEditingMessageText("");
    setEditingSkillIds([]);
    setEditingAttachments([]);
  }, []);

  const cancelEditingMessage = useCallback(() => {
    if (isSavingEditedMessage) {
      return;
    }
    clearEditingMessage();
  }, [clearEditingMessage, isSavingEditedMessage]);

  const saveEditedMessage = useCallback(
    async (message: ShellMessage) => {
      const trimmed = editingMessageText.trim();
      if (!trimmed || isSavingEditedMessage) {
        return;
      }
      const messageIndex = messages.findIndex((item) => item.id === message.id);
      const removedCount =
        messageIndex < 0 ? 0 : messages.length - messageIndex - 1;
      if (
        removedCount > 0 &&
        !window.confirm(
          t(
            "Editing this message will remove {count} later messages and regenerate the reply. Continue?",
            { count: removedCount },
          ),
        )
      ) {
        return;
      }
      setIsSavingEditedMessage(true);
      let editAccepted = false;
      try {
        await onEditMessage(
          message,
          trimmed,
          editingSkillIds,
          editingAttachments,
          () => {
            if (editAccepted) {
              return;
            }
            editAccepted = true;
            clearEditingMessage();
          },
        );
      } finally {
        setIsSavingEditedMessage(false);
      }
    },
    [
      clearEditingMessage,
      editingAttachments,
      editingMessageText,
      editingSkillIds,
      isSavingEditedMessage,
      messages,
      onEditMessage,
      t,
    ],
  );

  return (
    <div
      className="chat-panel flex min-h-0 flex-1 flex-col overflow-hidden"
      ref={chatPanelRef}
      style={
        {
          "--composer-editor-height": `${composerEditorHeight}px`,
        } as CSSProperties
      }
    >
      <div
        className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4"
        onKeyDown={handleMessageListKeyDown}
        onPointerCancel={handleMessageListPointerEnd}
        onPointerDown={handleMessageListPointerDown}
        onPointerUp={handleMessageListPointerEnd}
        onScroll={handleMessageScroll}
        onTouchEnd={handleMessageListTouchEnd}
        onTouchStart={handleMessageListTouchStart}
        onWheel={handleMessageListWheel}
        ref={messageScrollRef}
        tabIndex={0}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${
            messages.length ? "max-w-5xl gap-4" : "max-w-6xl"
          }`}
          ref={messageScrollContentRef}
        >
          {messages.length ? (
            <>
              {hasMoreMessagesBefore || isLoadingMoreMessages ? (
                <div className="flex justify-center">
                  <Button
                    className="chat-toolbar-button inline-flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_80%,transparent)] px-3 py-1.5 text-xs font-semibold text-[var(--muted)]"
                    isDisabled={isLoadingMoreMessages}
                    onPress={requestMoreMessages}
                    type="button"
                    variant="ghost"
                  >
                    {isLoadingMoreMessages ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-3.5 animate-spin"
                      />
                    ) : (
                      <ArrowUp aria-hidden="true" className="size-3.5" />
                    )}
                    <span>
                      {isLoadingMoreMessages
                        ? t("Loading…")
                        : t("Load earlier messages")}
                    </span>
                  </Button>
                </div>
              ) : null}
              {messages.map((message) => (
                <MessageRow
                  canEdit={
                    !readOnly &&
                    !isSendingMessage &&
                    !message.pendingMode &&
                    message.role === "user" &&
                    !message.syntheticSource
                  }
                  editingAttachments={editingAttachments}
                  editingSkillIds={editingSkillIds}
                  editingText={editingMessageText}
                  helpers={helpers}
                  isCopied={copiedMessageId === message.id}
                  isEditing={editingMessageId === message.id}
                  isSavingEdit={
                    isSavingEditedMessage && editingMessageId === message.id
                  }
                  key={message.id}
                  message={message}
                  onBeginEdit={beginEditingMessage}
                  onCancelEdit={cancelEditingMessage}
                  onCopyMessage={handleCopyMessage}
                  onEditingAttachmentsChange={setEditingAttachments}
                  onEditingSkillIdsChange={setEditingSkillIds}
                  onEditingTextChange={setEditingMessageText}
                  onGuideQueuedMessage={onGuideQueuedMessage}
                  onOpenMessageApiRequests={onOpenMessageApiRequests}
                  onSaveEdit={saveEditedMessage}
                  onSelectEditAttachments={onSelectEditAttachments}
                  onWithdrawQueuedMessage={onWithdrawQueuedMessage}
                  queuedMessageIds={queuedMessageIds}
                  skills={skills}
                  workspaceId={workspaceId}
                />
              ))}
            </>
          ) : isLoadingMessages ? (
            <div className="flex min-h-48 items-center justify-center gap-2 text-sm font-medium text-[var(--muted)]">
              <LoaderCircle
                aria-hidden="true"
                className="size-4 animate-spin"
              />
              <span>{t("Loading…")}</span>
            </div>
          ) : readOnly ? (
            <div className="flex min-h-48 items-center justify-center text-sm font-medium text-[var(--muted)]">
              {t("No transcript records")}
            </div>
          ) : (
            overviewRenderer()
          )}
        </div>
        <div aria-hidden="true" className="h-px" />
      </div>

      {!readOnly ? (
        <>
          <div
            aria-label={t("Resize message composer")}
            aria-orientation="horizontal"
            aria-valuemax={composerEditorMaxHeight()}
            aria-valuemin={COMPOSER_EDITOR_MIN_HEIGHT_PX}
            aria-valuenow={composerEditorHeight}
            className={`composer-resize-splitter ${
              isResizingComposer ? "composer-resize-splitter-active" : ""
            }`}
            onKeyDown={(event) => {
              if (event.key === "ArrowUp") {
                event.preventDefault();
                resizeComposerEditorBy(COMPOSER_EDITOR_KEY_STEP_PX);
              }

              if (event.key === "ArrowDown") {
                event.preventDefault();
                resizeComposerEditorBy(-COMPOSER_EDITOR_KEY_STEP_PX);
              }
            }}
            onPointerDown={handleComposerResizePointerDown}
            role="separator"
            tabIndex={0}
          />

          <div className="composer-shell shrink-0 border-t border-[color-mix(in_oklab,var(--border)_80%,transparent)] bg-transparent">
            <form
              className="message-composer-form mx-auto w-full"
              onSubmit={handleComposerSubmit}
            >
              <div className="composer-surface relative rounded-xl border border-[var(--border)] bg-[var(--surface)]">
                {selectedSkills.length ? (
                  <div className="flex flex-wrap gap-1.5 px-3 pt-2">
                    {selectedSkills.map((skill) => (
                      <span
                        className="inline-flex max-w-full items-center gap-1 rounded-full border border-[var(--accent)] bg-[var(--accent-soft)] px-2 py-1 text-xs font-semibold text-[var(--accent-soft-foreground)]"
                        key={skill.key}
                      >
                        <span className="max-w-44 truncate">{skill.name}</span>
                        <Button
                          aria-label={t("Remove skill {name}", {
                            name: skill.name,
                          })}
                          className="inline-flex size-4 items-center justify-center rounded-full text-[var(--accent-soft-foreground)] hover:bg-[var(--accent-soft)]"
                          onPress={() => onRemoveSkill(skill.key)}
                          type="button"
                          variant="ghost"
                        >
                          <X aria-hidden="true" className="size-3" />
                        </Button>
                      </span>
                    ))}
                  </div>
                ) : null}
                {draftAttachments.length ? (
                  <div className="composer-attachment-list px-3 pt-2">
                    {draftAttachments.map((attachment) => (
                      <ComposerAttachmentChip
                        helpers={helpers}
                        attachment={attachment}
                        key={attachment.id}
                        onRemove={() => onRemoveAttachment(attachment.id)}
                      />
                    ))}
                  </div>
                ) : null}
                <TextField aria-label={t("Message")} className="contents">
                  <TextArea
                    className="message-composer-textarea min-h-16 w-full resize-none border-0 bg-transparent px-3 py-1.5 text-sm leading-6 text-[var(--foreground)] shadow-none outline-none placeholder:text-[var(--muted)]"
                    name="message"
                    onChange={(event) => onDraftMessageChange(event.target.value)}
                    // IME composition and modifier-aware queueing require the native keyboard event.
                    onKeyDown={(
                      event: ReactKeyboardEvent<HTMLTextAreaElement>,
                    ) => {
                      if (
                        event.key !== "Enter" ||
                        event.shiftKey ||
                        event.nativeEvent.isComposing
                      ) {
                        return;
                      }

                      event.preventDefault();
                      if (isQueueModifierActive(event)) {
                        onSubmit(event as unknown as FormEvent<HTMLFormElement>, {
                          schedule: true,
                        });
                        return;
                      }

                      event.currentTarget.form?.requestSubmit();
                    }}
                    onPaste={handlePaste}
                    placeholder={composerPlaceholder}
                    ref={messageTextareaRef}
                    value={draftMessage}
                  />
                </TextField>
                {skillQuery !== null ? (
                  <div className="absolute bottom-full left-0 z-20 mb-2 w-full overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface)] shadow-[var(--overlay-shadow)]">
                    <div className="panel-scroll max-h-64 overflow-y-auto py-1">
                      {skillCatalogStatus === "loading" && !skills.length ? (
                        <div className="px-3 py-3 text-sm text-[var(--muted)]">
                          {t("Loading skills…")}
                        </div>
                      ) : skillCatalogStatus === "error" ? (
                        <div className="px-3 py-3 text-sm text-[var(--danger)]">
                          {skillCatalogError
                            ? t("Failed to load skills: {error}", {
                                error: skillCatalogError,
                              })
                            : t("Failed to load skills")}
                        </div>
                      ) : visibleSkills.length ? (
                        <>
                          {skillCatalogRefreshError ? (
                            <div className="border-b border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-xs text-[var(--warning)]">
                              {t("Skill list refresh failed: {error}", {
                                error: skillCatalogRefreshError,
                              })}
                            </div>
                          ) : null}
                          <ListBox
                            aria-label={t("Matching skills")}
                            disabledKeys={visibleSkills
                              .filter((skill) => !skill.enabled)
                              .map((skill) => skill.key)}
                            onAction={(key) => {
                              const skill = visibleSkills.find(
                                (candidate) => candidate.key === key,
                              );
                              if (skill) {
                                handleSkillSelect(skill);
                              }
                            }}
                          >
                            {visibleSkills.map((skill) => (
                              <ListBox.Item
                                aria-label={t("Select skill {name}", {
                                  name: skill.name,
                                })}
                                className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 px-3 py-2 text-left hover:bg-[var(--surface-secondary)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                id={skill.key}
                                key={skill.key}
                                textValue={skill.name}
                              >
                                <div className="min-w-0">
                                  <Label className="block truncate text-sm font-semibold text-[var(--foreground)]">
                                    {skill.name}
                                  </Label>
                                  <Description
                                    className="mt-0.5 block truncate text-xs text-[var(--muted)]"
                                  >
                                    {skill.enabled
                                      ? skill.description
                                      : t("Skill is disabled")}
                                  </Description>
                                </div>
                                <span className="self-center rounded-md border border-[var(--border)] px-1.5 py-0.5 text-[11px] font-semibold text-[var(--muted)]">
                                  {skill.enabled
                                    ? skillScopeLabel(skill, t)
                                    : t("disabled")}
                                </span>
                              </ListBox.Item>
                            ))}
                          </ListBox>
                        </>
                      ) : (
                        <div className="px-3 py-3 text-sm text-[var(--muted)]">
                          {skillCatalogRefreshError
                            ? t("Skill list refresh failed: {error}", {
                                error: skillCatalogRefreshError,
                              })
                            : t("No matching skills")}
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
                  <Button
                    aria-label={t("Add attachment")}
                    className="composer-tool-button"
                    isIconOnly
                    isDisabled={isSelectingAttachments}
                    onPress={onSelectAttachments}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    {isSelectingAttachments ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin"
                      />
                    ) : (
                      <Plus aria-hidden="true" className="size-4" />
                    )}
                  </Button>
                  <Button
                    aria-label={t("Plan mode")}
                    aria-pressed={isPlanModeEnabled}
                    className="composer-team-toggle"
                    onPress={() => onPlanModeEnabledChange(!isPlanModeEnabled)}
                    size="sm"
                    type="button"
                    variant={isPlanModeEnabled ? "tertiary" : "ghost"}
                  >
                    <ListChecks
                      aria-hidden="true"
                      className="size-3.5 shrink-0"
                    />
                    <span className="composer-team-toggle-label">
                      {t("Plan")}
                    </span>
                  </Button>
                  <ComposerSelectMenu
                    ariaLabel={t("Model")}
                    className="composer-model-select composer-model-select-compact max-w-full"
                    disabled={isLoadingSettings || !modelOptions.length}
                    emptyLabel={t("No enabled models")}
                    icon={Bot}
                    onChange={handleModelSelect}
                    options={modelOptions}
                    selectedValue={selectedModelId}
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
                  {canRetryRun ? (
                    <Button
                      aria-label={t("Retry last run")}
                      className="composer-retry-button composer-run-button"
                      isIconOnly
                      onPress={onRetryRun}
                      size="sm"
                      type="button"
                      variant="tertiary"
                    >
                      <RefreshCw aria-hidden="true" className="size-4" />
                    </Button>
                  ) : null}
                  <span
                    aria-hidden="true"
                    className="composer-control-spacer"
                  />
                  <ContextUsageCircle
                    isLoading={isLoadingContextUsage}
                    usage={contextUsage}
                  />
                  {isSendingMessage ? (
                    <Button
                      aria-label={runningButtonLabel}
                      className="composer-run-button"
                      isDisabled={
                        runningButtonSendsMessage &&
                        (!canGuideActiveRun ||
                          !selectedModelId ||
                          Boolean(draftUnsupportedAttachmentMessage))
                      }
                      isIconOnly
                      onPress={handleRunningRunButtonClick}
                      size="sm"
                      type="button"
                      variant={runningButtonSendsMessage ? "primary" : "danger"}
                    >
                      {runningButtonSendsMessage ? (
                        <Send aria-hidden="true" className="size-4" />
                      ) : (
                        <X aria-hidden="true" className="size-4" />
                      )}
                    </Button>
                  ) : (
                    <span
                      className="composer-send-button-shell"
                      onBlur={() => setIsSendButtonTooltipOpen(false)}
                      onFocus={() => setIsSendButtonTooltipOpen(true)}
                      onMouseEnter={() => setIsSendButtonTooltipOpen(true)}
                      onMouseLeave={() => setIsSendButtonTooltipOpen(false)}
                    >
                      <Button
                        aria-describedby={
                          showSendButtonTooltip
                            ? "composer-send-button-tooltip"
                            : undefined
                        }
                        aria-label={t("Send message")}
                        className="composer-run-button"
                        isDisabled={
                          (!draftMessage.trim() && !draftAttachments.length) ||
                          !selectedModelId ||
                          Boolean(draftUnsupportedAttachmentMessage)
                        }
                        isIconOnly
                        /* Queue modifiers are pointer-specific; normal submit remains native form behavior. */
                        onClick={(event) => {
                          if (isQueueModifierActive(event)) {
                            event.preventDefault();
                            const form = (event.currentTarget as HTMLButtonElement)
                              .form;
                            if (!form) {
                              return;
                            }

                            onSubmit(
                              event as unknown as FormEvent<HTMLFormElement>,
                              {
                                schedule: true,
                              },
                            );
                          }
                        }}
                        size="sm"
                        type="submit"
                        variant="primary"
                      >
                        <Send aria-hidden="true" className="size-4" />
                      </Button>
                      {showSendButtonTooltip ? (
                        <span
                          className="composer-send-tooltip"
                          id="composer-send-button-tooltip"
                          role="tooltip"
                        >
                          {sendButtonTitle}
                        </span>
                      ) : null}
                    </span>
                  )}
                </div>
              </div>
            </form>
          </div>
        </>
      ) : null}
    </div>
  );
}

export const ChatPanel = memo(ChatPanelComponent);

const MessageRow = memo(function MessageRow({
  canEdit,
  editingAttachments,
  editingSkillIds,
  editingText,
  helpers,
  isCopied,
  isEditing,
  isSavingEdit,
  message,
  onBeginEdit,
  onCancelEdit,
  onCopyMessage,
  onEditingAttachmentsChange,
  onEditingSkillIdsChange,
  onEditingTextChange,
  onGuideQueuedMessage,
  onOpenMessageApiRequests,
  onSaveEdit,
  onSelectEditAttachments,
  onWithdrawQueuedMessage,
  queuedMessageIds,
  skills,
  workspaceId,
}: {
  canEdit: boolean;
  editingAttachments: ComposerAttachment[];
  editingSkillIds: string[];
  editingText: string;
  helpers: ChatPanelHelpers;
  isCopied: boolean;
  isEditing: boolean;
  isSavingEdit: boolean;
  message: ShellMessage;
  onBeginEdit: (message: ShellMessage) => void;
  onCancelEdit: () => void;
  onCopyMessage: (messageId: string, text: string) => void;
  onEditingAttachmentsChange: (attachments: ComposerAttachment[]) => void;
  onEditingSkillIdsChange: (skillIds: string[]) => void;
  onEditingTextChange: (value: string) => void;
  onGuideQueuedMessage: (messageId: string) => void;
  onOpenMessageApiRequests: (message: ShellMessage) => void;
  onSaveEdit: (message: ShellMessage) => void;
  onSelectEditAttachments: (
    onSelected: (attachments: ComposerAttachment[]) => void,
  ) => void;
  onWithdrawQueuedMessage: (messageId: string) => void;
  queuedMessageIds: ReadonlySet<string>;
  skills: ConfiguredSkillSummary[];
  workspaceId: string | null;
}) {
  const { fallbackMessageParts, formatChatCreatedAt, messageCopyText } =
    helpers;
  const { t } = useI18n();
  const isUser = message.role === "user";
  const parts = useMemo(
    () =>
      message.parts.length ? message.parts : fallbackMessageParts(message),
    [fallbackMessageParts, message],
  );
  const reasoningPartCount = useMemo(
    () => parts.filter((part) => part.type === "reasoning").length,
    [parts],
  );
  const copyText = useMemo(
    () => messageCopyText(message, parts),
    [message, messageCopyText, parts],
  );
  const authorLabel = isUser ? "You" : "Foco Agent";
  const createdAtLabel = formatChatCreatedAt(message.createdAt);
  const copyLabel = isCopied ? t("Copied message") : t("Copy message");
  const pendingLabel =
    message.pendingMode === "guidance"
      ? t("Guidance pending")
      : message.pendingMode === "queued"
        ? t("Queued")
        : null;
  const isPendingUserMessage = isUser && pendingLabel !== null;
  const isPlanModeMessage = isUser && message.sessionMode === "plan";
  const canManageQueuedMessage =
    isUser &&
    message.pendingMode === "queued" &&
    queuedMessageIds.has(message.id);

  return (
    <div
      className={`message-row flex ${isUser ? "message-row-user" : "message-row-agent"}`}
    >
      <div className="message-card-shell">
        <div
          className={`message-bubble flex w-full max-w-full items-start gap-3 rounded-2xl border px-4 py-3 shadow-[var(--overlay-shadow)] ${
            isUser
              ? "message-bubble-user flex-row rounded-tr-md"
              : "message-bubble-assistant flex-row rounded-tl-md"
          } ${isPendingUserMessage ? "message-bubble-pending" : ""}`}
          style={{
            backgroundColor: isPendingUserMessage
              ? "var(--surface-secondary)"
              : isUser
                ? "var(--accent-soft)"
                : "var(--surface)",
            borderColor: isPendingUserMessage
              ? "var(--border)"
              : isUser
                ? "var(--accent)"
                : "var(--border)",
          }}
        >
          <div
            className={`message-avatar mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${
              isUser
                ? "bg-[color-mix(in_oklab,var(--accent)_45%,transparent)] text-white"
                : "bg-[var(--surface-secondary)] text-[var(--muted)]"
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
                  <span className="message-pending-badge">{pendingLabel}</span>
                ) : null}
                {isPlanModeMessage ? (
                  <span className="message-run-badge">{t("Plan mode")}</span>
                ) : null}
                <time
                  className="message-created-at"
                  dateTime={message.createdAt}
                  title={message.createdAt}
                >
                  {createdAtLabel}
                </time>
                {!isUser && message.metrics ? (
                  <span
                    className="message-model-id"
                    title={`${t("Model")}: ${message.metrics.modelId}`}
                  >
                    {message.metrics.modelId}
                  </span>
                ) : null}
                {!isUser && message.runBadges?.includes("llmReconnect") ? (
                  <span
                    className="message-run-badge"
                    title={t("LLM request failed and reconnected")}
                  >
                    {t("Reconnected")}
                  </span>
                ) : null}
                {!isUser &&
                message.runBadges?.includes("contextCompressionRule") ? (
                  <span
                    className="message-run-badge"
                    title={t("Rule-based context compression was triggered")}
                  >
                    {t("Rule compressed")}
                  </span>
                ) : null}
                {!isUser &&
                message.runBadges?.includes("contextCompressionLlm") ? (
                  <span
                    className="message-run-badge"
                    title={t("LLM summary context compression was triggered")}
                  >
                    {t("LLM compressed")}
                  </span>
                ) : null}
                {!isUser &&
                message.runBadges?.includes("contextCompressionRuntime") ? (
                  <span
                    className="message-run-badge"
                    title={t("Runtime tool-state compression was triggered")}
                  >
                    {t("Tool state compressed")}
                  </span>
                ) : null}
              </span>
              <span className="message-action-group">
                {canEdit ? (
                  <Button
                    aria-label={t("Edit message")}
                    className="size-7 min-w-7"
                    isIconOnly
                    onPress={() => onBeginEdit(message)}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <Pencil aria-hidden="true" className="size-3.5" />
                  </Button>
                ) : null}
                {canManageQueuedMessage ? (
                  <>
                    <Button
                      aria-label={t("Convert queued message to guidance")}
                      className="size-7 min-w-7"
                      isIconOnly
                      onPress={() => onGuideQueuedMessage(message.id)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      <ArrowUp aria-hidden="true" className="size-3.5" />
                    </Button>
                    <Button
                      aria-label={t("Withdraw queued message")}
                      className="size-7 min-w-7"
                      isIconOnly
                      onPress={() => onWithdrawQueuedMessage(message.id)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                    </Button>
                  </>
                ) : null}
                <Button
                  aria-label={copyLabel}
                  className="size-7 min-w-7"
                  isIconOnly
                  isDisabled={!copyText}
                  onPress={() => onCopyMessage(message.id, copyText)}
                  size="sm"
                  type="button"
                  variant="ghost"
                >
                  {isCopied ? (
                    <CheckCircle2 aria-hidden="true" className="size-3.5" />
                  ) : (
                    <Copy aria-hidden="true" className="size-3.5" />
                  )}
                </Button>
              </span>
            </div>
            {isEditing ? (
              <div className="space-y-2">
                {editingSkillIds.length ? (
                  <div className="flex flex-wrap gap-1.5">
                    {editingSkillIds.map((skillId) => {
                      const skill = skills.find((item) => item.key === skillId);
                      return (
                        <Button
                          aria-label={t("Remove skill")}
                          className="rounded-full border border-[color-mix(in_oklab,var(--accent)_20%,transparent)] bg-[color-mix(in_oklab,var(--surface)_70%,transparent)] px-2 py-0.5 text-xs text-[var(--accent-soft-foreground)]"
                          key={skillId}
                          onPress={() =>
                            onEditingSkillIdsChange(
                              editingSkillIds.filter((id) => id !== skillId),
                            )
                          }
                          type="button"
                          variant="ghost"
                        >
                          {skill?.name ?? skillId} ×
                        </Button>
                      );
                    })}
                  </div>
                ) : null}
                <div className="flex flex-wrap items-center gap-1.5">
                  <Select
                    aria-label={t("Add skill")}
                    className="min-w-32"
                    placeholder={t("Add skill")}
                    selectedKey={null}
                    onSelectionChange={(key) => {
                      const skillId = String(key ?? "");
                      if (skillId && !editingSkillIds.includes(skillId)) {
                        onEditingSkillIdsChange([...editingSkillIds, skillId]);
                      }
                    }}
                  >
                    <Select.Trigger className="h-7 rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_90%,transparent)] px-2 text-xs">
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover placement="bottom start">
                      <ListBox>
                        {skills
                          .filter((skill) => !editingSkillIds.includes(skill.key))
                          .map((skill) => (
                            <ListBox.Item
                              id={skill.key}
                              key={skill.key}
                              textValue={skill.name}
                            >
                              {skill.name}
                              <ListBox.ItemIndicator />
                            </ListBox.Item>
                          ))}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                  <Button
                    className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_90%,transparent)] px-2 py-1 text-xs"
                    onPress={() =>
                      onSelectEditAttachments((attachments) =>
                        onEditingAttachmentsChange([
                          ...editingAttachments,
                          ...attachments,
                        ]),
                      )
                    }
                    type="button"
                    variant="ghost"
                  >
                    {t("Add attachment")}
                  </Button>
                </div>
                {editingAttachments.length ? (
                  <div className="flex flex-wrap gap-1.5">
                    {editingAttachments.map((attachment) => (
                      <Button
                        aria-label={t("Remove attachment {name}", {
                          name: attachment.name,
                        })}
                        className="rounded-full border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_70%,transparent)] px-2 py-0.5 text-xs"
                        key={attachment.id}
                        onPress={() =>
                          onEditingAttachmentsChange(
                            editingAttachments.filter(
                              (item) => item.id !== attachment.id,
                            ),
                          )
                        }
                        type="button"
                        variant="ghost"
                      >
                        {attachment.name} ×
                      </Button>
                    ))}
                  </div>
                ) : null}
                <TextField aria-label={t("Edit message")} className="contents">
                  <TextArea
                    autoFocus
                    className="min-h-24 w-full resize-y rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_90%,transparent)] px-3 py-2 text-sm text-[var(--foreground)] outline-none focus:border-[var(--accent)]"
                    disabled={isSavingEdit}
                    onChange={(event) => onEditingTextChange(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        event.preventDefault();
                        onCancelEdit();
                      } else if (event.key === "Enter" && !event.shiftKey) {
                        event.preventDefault();
                        onSaveEdit(message);
                      }
                    }}
                    value={editingText}
                  />
                </TextField>
                <div className="flex justify-end gap-1.5">
                  <Button
                    aria-label={t("Cancel editing")}
                    className="size-7 min-w-7"
                    isDisabled={isSavingEdit}
                    isIconOnly
                    onPress={onCancelEdit}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </Button>
                  <Button
                    aria-label={t("Save and regenerate")}
                    className="size-7 min-w-7"
                    isDisabled={isSavingEdit || !editingText.trim()}
                    isIconOnly
                    onPress={() => onSaveEdit(message)}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    {isSavingEdit ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin"
                      />
                    ) : (
                      <Send aria-hidden="true" className="size-4" />
                    )}
                  </Button>
                </div>
              </div>
            ) : null}
            {!isEditing && !isUser ? (
              <MemoriesUsedBlock memories={message.memoriesUsed} />
            ) : null}
            {!isEditing && parts.length ? (
              parts.map((part, partIndex) => (
                <MessagePartBlock
                  helpers={helpers}
                  isError={message.status === "error"}
                  isStreaming={message.status === "streaming"}
                  isStreamingTail={partIndex === parts.length - 1}
                  isUser={isUser}
                  key={`${message.id}-part-${partIndex}`}
                  part={part}
                  reasoningDurationFallbackMs={
                    reasoningPartCount === 1
                      ? (message.metrics?.totalLatencyMs ?? null)
                      : null
                  }
                  workspaceId={workspaceId}
                />
              ))
            ) : !isEditing && message.status === "streaming" ? (
              <LoaderCircle
                aria-hidden="true"
                className="message-waiting-spinner size-4 animate-spin"
              />
            ) : null}
            {!isUser ? (
              <ExtractedMemoriesBlock memories={message.extractedMemories} />
            ) : null}
            {!isUser ? (
              <SpecUpdatesBlock updates={message.specUpdates} />
            ) : null}
            {!isUser && message.metrics && message.status !== "streaming" ? (
              <ChatReplyMetricsLine
                helpers={helpers}
                metrics={message.metrics}
                onOpenApiRequests={
                  message.metrics.llmRequestIds.length
                    ? () => onOpenMessageApiRequests(message)
                    : null
                }
              />
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
});

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
  const toneClass = usage?.hasLlmCompressionPlan
    ? "context-usage-circle-critical"
    : usage && percent >= usage.compressionTriggerPercent
      ? "context-usage-circle-warn"
      : "context-usage-circle-normal";
  const ariaLabel = t("Context usage {percent}%", { percent });
  const title = usage
    ? t("Context usage {percent}% (assembled {assembledPercent}%)", {
        assembledPercent: usage.assembledUsagePercent,
        percent,
      })
    : ariaLabel;

  return (
    <div
      aria-label={ariaLabel}
      className={`context-usage-circle ${toneClass} ${
        isLoading ? "context-usage-circle-loading" : ""
      } ${className}`}
      role="status"
      style={
        {
          "--context-usage-percent": `${clampedPercent}%`,
        } as CSSProperties
      }
      title={title}
    >
      {percent}%
    </div>
  );
}

type ComposerSelectOption = {
  badge?: string;
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
  const selectedLabel =
    options.find((option) => option.value === selectedValue)?.label ?? emptyLabel;
  // Keep a stable "Label: value" accessible name for tests and screen readers.
  const triggerAriaLabel = `${ariaLabel}: ${selectedLabel}`;

  return (
    <Select
      aria-label={ariaLabel}
      className={`composer-select-menu ${className}`}
      isDisabled={disabled || options.length === 0}
      placeholder={emptyLabel}
      selectedKey={selectedValue || null}
      onSelectionChange={(key) => {
        if (key == null) {
          return;
        }
        onChange(String(key));
      }}
    >
      <Select.Trigger
        aria-label={triggerAriaLabel}
        className="composer-select-summary h-[1.75rem] min-h-[1.75rem] gap-1.5 px-2 text-[length:var(--foco-font-micro)] font-normal"
      >
        <Icon aria-hidden="true" className="size-3.5 shrink-0" />
        <Select.Value className="composer-select-label min-w-0 flex-1 truncate leading-none">
          {({ defaultChildren, isPlaceholder }) =>
            isPlaceholder ? defaultChildren : selectedLabel
          }
        </Select.Value>
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover className="composer-select-popover w-64 max-w-[min(16rem,calc(100vw-1rem))]" placement="top start">
        <ListBox>
          {options.map((option) => (
            <ListBox.Item
              id={option.value}
              key={option.value}
              textValue={option.label}
            >
              <Label className="composer-select-option-label min-w-0 flex-1 truncate">
                {option.label}
              </Label>
              {option.badge ? (
                <Chip className="ms-auto" size="sm" variant="soft">
                  {option.badge}
                </Chip>
              ) : null}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}

function ReasoningBlock({
  helpers,
  durationMs,
  isStreaming,
  reasoning,
}: {
  helpers: ChatPanelHelpers;
  durationMs: number | null;
  isStreaming: boolean;
  reasoning: string;
}) {
  const { compactInlineText, formatNullableLatencySeconds } = helpers;
  const { language, t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(isStreaming);
  const preview = compactInlineText(reasoning);
  const durationLabel = formatNullableLatencySeconds(durationMs, language);
  const durationTitle = t("Thinking duration {duration}", {
    duration: durationLabel,
  });

  useEffect(() => {
    setIsExpanded(isStreaming);
  }, [isStreaming]);

  const toggleLabel = isExpanded
    ? t("Collapse thinking")
    : t("Expand thinking");

  return (
    <div
      className="reasoning-block tool-call-block group min-w-0 text-[var(--muted)]"
      data-expanded={isExpanded ? "true" : "false"}
    >
      <Button
        aria-expanded={isExpanded}
        aria-label={toggleLabel}
        className="tool-call-summary h-auto w-full min-w-0 cursor-pointer items-center justify-start gap-1.5 p-0 text-left text-xs font-semibold text-[var(--muted)] hover:text-[var(--muted)]"
        onPress={() => setIsExpanded((current) => !current)}
        type="button"
                    variant="ghost"
                  >
        {isExpanded ? (
          <ChevronDown
            aria-hidden="true"
            className="size-3.5 shrink-0 text-[var(--muted)]"
          />
        ) : (
          <ChevronRight
            aria-hidden="true"
            className="size-3.5 shrink-0 text-[var(--muted)]"
          />
        )}
        <span className="shrink-0 font-semibold">{t("Thinking")}</span>
        {isExpanded ? null : (
          <span
            className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-[var(--muted)]"
            title={preview}
          >
            {preview}
          </span>
        )}
        {durationLabel && durationTitle ? (
          <span
            className="ml-auto shrink-0 tabular-nums text-[11px] font-semibold text-[var(--muted)]"
            title={durationTitle}
          >
            {durationLabel}
          </span>
        ) : null}
      </Button>
      {isExpanded ? (
        <div className="mt-2 text-[var(--muted)]">
          <MarkdownContent
            content={reasoning}
            isUser={false}
            renderMode={isStreaming ? "streaming" : "full"}
            selectedSkillPrefix={helpers.selectedSkillPrefix}
            variant="reasoning"
          />
        </div>
      ) : null}
    </div>
  );
}

function MessagePartBlockComponent({
  helpers,
  isError,
  isStreaming,
  isStreamingTail,
  isUser,
  part,
  reasoningDurationFallbackMs,
  workspaceId,
}: {
  helpers: ChatPanelHelpers;
  isError: boolean;
  isStreaming: boolean;
  isStreamingTail: boolean;
  isUser: boolean;
  part: ChatMessagePart;
  reasoningDurationFallbackMs: number | null;
  workspaceId: string | null;
}) {
  if (part.type === "reasoning") {
    return (
      <ReasoningBlock
        helpers={helpers}
        durationMs={
          part.liveDurationMs ?? part.durationMs ?? reasoningDurationFallbackMs
        }
        isStreaming={isStreaming && isStreamingTail}
        reasoning={part.text}
      />
    );
  }

  if (part.type === "toolCall") {
    return (
      <ToolCallBlock
        helpers={helpers}
        toolCall={part.toolCall}
        workspaceId={workspaceId}
      />
    );
  }

  if (part.type === "contextCompression") {
    return <ContextCompressionBlock compression={part} helpers={helpers} />;
  }

  if (part.type === "attachment") {
    return (
      <AttachmentPartBlock
        attachment={part.attachment}
        helpers={helpers}
        isUser={isUser}
      />
    );
  }

  if (part.type === "error") {
    return <ErrorMessagePart text={part.text} />;
  }

  if (part.type === "userInterruption") {
    // Role boundary inside assistant parts (guidance / reasoning-loop recovery).
    // Render as a user-styled block so history matches live guidance bubbles.
    return (
      <div className="message-user-interruption my-2 rounded-xl border border-[var(--border)] bg-[var(--surface-2)] px-3 py-2">
        <MarkdownContent
          content={part.content}
          isError={false}
          isUser
          renderMode="full"
          selectedSkillPrefix={helpers.selectedSkillPrefix}
        />
      </div>
    );
  }

  return (
    <MarkdownContent
      content={part.text}
      isError={isError}
      isUser={isUser}
      renderMode={
        !isUser && isStreaming && isStreamingTail ? "streaming" : "full"
      }
      selectedSkillPrefix={helpers.selectedSkillPrefix}
    />
  );
}

export const MessagePartBlock = memo(MessagePartBlockComponent);

function ErrorMessagePart({ text }: { text: string }) {
  return (
    <div className="whitespace-pre-wrap break-words rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm leading-6 text-[var(--danger)]">
      {text}
    </div>
  );
}

function ComposerAttachmentChip({
  helpers,
  attachment,
  onRemove,
}: {
  helpers: ChatPanelHelpers;
  attachment: ComposerAttachment;
  onRemove: () => void;
}) {
  const { formatFileSize } = helpers;
  const { t } = useI18n();
  const title = attachment.path
    ? `${attachment.name} 路 ${attachment.path} 路 ${formatFileSize(attachment.sizeBytes)}`
    : `${attachment.name} 路 ${formatFileSize(attachment.sizeBytes)}`;

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
      <Button
        aria-label={t("Remove attachment {name}", { name: attachment.name })}
        className="inline-flex size-5 shrink-0 items-center justify-center rounded-full text-[var(--muted)] hover:bg-[var(--default)] hover:text-[var(--foreground)]"
        onPress={onRemove}
        type="button"
        variant="ghost"
      >
        <X aria-hidden="true" className="size-3" />
      </Button>
    </span>
  );
}

function AttachmentPartBlock({
  helpers,
  attachment,
  isUser,
}: {
  helpers: ChatPanelHelpers;
  attachment: ChatAttachmentPartSummary;
  isUser: boolean;
}) {
  const { formatFileSize } = helpers;
  const title = attachment.path
    ? `${attachment.name} 路 ${attachment.path} 路 ${formatFileSize(attachment.sizeBytes)}`
    : `${attachment.name} 路 ${formatFileSize(attachment.sizeBytes)}`;

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

function ChatReplyMetricsLine({
  helpers,
  metrics,
  onOpenApiRequests,
}: {
  helpers: ChatPanelHelpers;
  metrics: ChatReplyMetrics;
  onOpenApiRequests: (() => void) | null;
}) {
  const { formatReplyDuration, formatTokensPerSecond } = helpers;
  const { language, t } = useI18n();
  const values = [
    `${t("Model")}: ${metrics.modelId}`,
    `${t("Channel")}: ${metrics.providerId}`,
    `${t("Total time")}: ${formatReplyDuration(metrics.totalLatencyMs, language)}`,
    `${t("tokens/s")}: ${formatTokensPerSecond(metrics, language)}`,
  ];

  return (
    <div className="flex items-center justify-between gap-2 border-t border-[var(--border)] pt-2 text-[11px] leading-4 text-[var(--muted)]">
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
        {values.map((value) => (
          <span className="min-w-0 break-words" key={value}>
            {value}
          </span>
        ))}
      </div>
      {onOpenApiRequests ? (
        <Button
          aria-label={t("View API requests for this reply")}
          className="inline-flex size-7 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
          onPress={onOpenApiRequests}
          type="button"
          variant="ghost"
        >
          <Server aria-hidden="true" className="size-3.5" />
        </Button>
      ) : null}
    </div>
  );
}

function memoryMetaLabel(value: string, t: Translate) {
  return t(`memory.${value}`);
}

function MemoriesUsedBlock({
  memories,
}: {
  memories: ChatMemoryUsedSummary[];
}) {
  const { t } = useI18n();
  if (!memories.length) {
    return null;
  }

  return (
    <details className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_70%,transparent)] px-3 py-2 text-xs text-[var(--muted)]">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-[var(--muted)] marker:hidden">
        <Brain aria-hidden="true" className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]" />
        <span>{t("Memories used")}</span>
        <span className="rounded-full bg-[var(--surface)] px-1.5 py-0.5 text-[10px] text-[var(--muted)]">
          {memories.length}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {memories.map((memory) => (
          <div
            className="min-w-0 rounded-md border border-[var(--border)] bg-[var(--surface)] px-2.5 py-2"
            key={`${memory.scope}-${memory.id}`}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[10px] font-semibold uppercase tracking-normal text-[var(--muted)]">
              <span>{memoryMetaLabel(memory.scope, t)}</span>
              <span>{memoryMetaLabel(memory.kind, t)}</span>
              <span>{memoryMetaLabel(memory.source, t)}</span>
              {memory.pinned ? <span>{t("Pinned")}</span> : null}
            </div>
            <div className="mt-1 line-clamp-2 break-words text-xs leading-5 text-[var(--muted)]">
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
    <details className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_70%,transparent)] px-3 py-2 text-xs text-[var(--muted)]">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-[var(--muted)] marker:hidden">
        <Brain aria-hidden="true" className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]" />
        <span>{t("Memories saved")}</span>
        <span className="rounded-full bg-[var(--surface)] px-1.5 py-0.5 text-[10px] text-[var(--muted)]">
          {memories.length}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {memories.map((memory) => (
          <div
            className="min-w-0 rounded-md border border-[var(--border)] bg-[var(--surface)] px-2.5 py-2"
            key={`${memory.scope}-${memory.id}`}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[10px] font-semibold uppercase tracking-normal text-[var(--muted)]">
              <span>{memoryMetaLabel(memory.scope, t)}</span>
              <span>{memoryMetaLabel(memory.kind, t)}</span>
              <span>{memoryMetaLabel(memory.status, t)}</span>
            </div>
            <div className="mt-1 line-clamp-2 break-words text-xs leading-5 text-[var(--muted)]">
              {memory.fact}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}

function SpecUpdatesBlock({ updates }: { updates: ChatSpecUpdateSummary[] }) {
  const { t } = useI18n();
  if (!updates.length) {
    return null;
  }

  const lineCount = updates.reduce(
    (count, update) => count + update.lines.length,
    0,
  );

  return (
    <details className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_70%,transparent)] px-3 py-2 text-xs text-[var(--muted)]">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-[var(--muted)] marker:hidden">
        <FileText
          aria-hidden="true"
          className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
        />
        <span>{t("Spec updated")}</span>
        <span className="rounded-full bg-[var(--surface)] px-1.5 py-0.5 text-[10px] text-[var(--muted)]">
          {lineCount}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {updates.map((update) => (
          <div
            className="min-w-0 overflow-hidden rounded-md border border-[var(--border)] bg-[var(--surface)]"
            key={update.id}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 border-b border-[var(--border)] px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-normal text-[var(--muted)]">
              <span>
                {t("Revision")} {update.baseRevision} -&gt; {update.revision}
              </span>
              {update.truncated ? <span>{t("Truncated")}</span> : null}
            </div>
            <div className="panel-scroll max-h-56 overflow-auto py-1 font-mono text-[11px] leading-5">
              {update.lines.map((line, index) => (
                <div
                  className={`whitespace-pre-wrap break-words px-2.5 ${
                    line.kind === "added"
                      ? "bg-[var(--success-soft)] text-[var(--success)]"
                      : "bg-[var(--danger-soft)] text-[var(--danger)]"
                  }`}
                  key={`${update.id}-${index}`}
                >
                  <span className="select-none pr-1 font-semibold">
                    {line.kind === "added" ? "+" : "-"}
                  </span>
                  <span>{line.text}</span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}

type CompactReplacementDiffLine = {
  kind: "added" | "removed";
  text: string;
};

type CompactReplacementDiff = {
  lines: CompactReplacementDiffLine[];
};

type ToolCallViewMode = "compact" | "raw";

const EMPTY_SELECTED_SKILL_PREFIX: SelectedSkillPrefixResolver = () => null;
const DIRECT_COMPACT_TEXT_FIELDS = [
  "content",
  "text",
  "message",
  "note",
  "error",
];
const ARRAY_COMPACT_SUMMARY_FIELDS = [
  "matches",
  "entries",
  "snippets",
  "symbols",
  "references",
  "files",
  "tasks",
  "questions",
  "results",
];

function replacementDiffLines(
  oldText: string,
  newText: string,
): CompactReplacementDiffLine[] {
  return [
    ...oldText.split("\n").map((text) => ({
      kind: "removed" as const,
      text,
    })),
    ...newText.split("\n").map((text) => ({
      kind: "added" as const,
      text,
    })),
  ];
}

function applyPatchDiffLines(patch: string): CompactReplacementDiffLine[] | null {
  const patchLines = patch
    .trim()
    .split("\n")
    .map((line) => line.replace(/\r$/, ""));
  if (
    patchLines.length < 2 ||
    patchLines[0]?.trim() !== "*** Begin Patch" ||
    patchLines.at(-1)?.trim() !== "*** End Patch"
  ) {
    return null;
  }

  const lines: CompactReplacementDiffLine[] = [];
  let section: "added" | "deleted" | "updated" | null = null;
  let hasFileOperation = false;

  for (const line of patchLines.slice(1, -1)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("*** Add File:")) {
      if (!trimmed.slice("*** Add File:".length).trim()) {
        return null;
      }
      section = "added";
      hasFileOperation = true;
      continue;
    }
    if (trimmed.startsWith("*** Delete File:")) {
      if (!trimmed.slice("*** Delete File:".length).trim()) {
        return null;
      }
      section = "deleted";
      hasFileOperation = true;
      continue;
    }
    if (trimmed.startsWith("*** Update File:")) {
      if (!trimmed.slice("*** Update File:".length).trim()) {
        return null;
      }
      section = "updated";
      hasFileOperation = true;
      continue;
    }
    if (trimmed.startsWith("*** Move to:")) {
      if (section !== "updated" || !trimmed.slice("*** Move to:".length).trim()) {
        return null;
      }
      continue;
    }
    if (trimmed === "*** End of File" || trimmed === "@@" || trimmed.startsWith("@@ ")) {
      if (section !== "updated") {
        return null;
      }
      continue;
    }
    if (trimmed.startsWith("***")) {
      return null;
    }

    if (line.startsWith("+")) {
      if (section !== "added" && section !== "updated") {
        return null;
      }
      lines.push({ kind: "added", text: line.slice(1) });
    } else if (line.startsWith("-")) {
      if (section !== "updated") {
        return null;
      }
      lines.push({ kind: "removed", text: line.slice(1) });
    } else if (section !== "updated") {
      return null;
    }
  }

  return hasFileOperation && lines.length > 0 ? lines : null;
}

function successfulCompactReplacementDiff(
  toolCall: ChatToolCallSummary,
  input: JsonValue,
): CompactReplacementDiff | null {
  if (
    toolCall.isError ||
    toolCall.status !== "completed" ||
    !isJsonRecord(input)
  ) {
    return null;
  }

  if (toolCall.name === "edit_file") {
    const oldStr = input.oldStr;
    const newStr = input.newStr;
    if (typeof oldStr !== "string" || typeof newStr !== "string") {
      return null;
    }

    // ponytail: this is replacement-snippet diff, not a full-file diff; upgrade when the backend returns real hunks/startLine.
    return { lines: replacementDiffLines(oldStr, newStr) };
  }

  if (toolCall.name === "apply_patch") {
    const patch = input.patch;
    if (typeof patch !== "string") {
      return null;
    }

    const lines = applyPatchDiffLines(patch);
    return lines ? { lines } : null;
  }

  if (
    toolCall.name !== "update_spec" ||
    !Array.isArray(input.edits) ||
    input.edits.length === 0
  ) {
    return null;
  }

  const lines: CompactReplacementDiffLine[] = [];
  for (const edit of input.edits) {
    if (
      !isJsonRecord(edit) ||
      typeof edit.oldText !== "string" ||
      typeof edit.newText !== "string"
    ) {
      return null;
    }
    lines.push(...replacementDiffLines(edit.oldText, edit.newText));
  }

  return { lines };
}

function isJsonRecord(
  value: JsonValue | null | undefined,
): value is { [key: string]: JsonValue } {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonEmptyText(value: JsonValue | undefined) {
  return typeof value === "string" && value.trim() ? value : null;
}

function compactRecordText(record: { [key: string]: JsonValue }) {
  for (const fieldName of DIRECT_COMPACT_TEXT_FIELDS) {
    const text = nonEmptyText(record[fieldName]);
    if (text) {
      return text;
    }
  }
  return null;
}

function compactArraySummary(
  record: { [key: string]: JsonValue },
  toolName: string,
  compactJson: (value: JsonValue) => string,
) {
  for (const fieldName of ARRAY_COMPACT_SUMMARY_FIELDS) {
    const value = record[fieldName];
    if (!Array.isArray(value) || value.length === 0) {
      continue;
    }

    const lines = value
      .map((item) =>
        compactArrayItemText(item, fieldName, toolName, compactJson),
      )
      .filter(Boolean);
    if (lines.length) {
      return lines.join("\n");
    }
  }
  return null;
}

function compactArrayItemText(
  item: JsonValue,
  fieldName: string,
  toolName: string,
  compactJson: (value: JsonValue) => string,
) {
  if (typeof item === "string") {
    return item;
  }
  if (!isJsonRecord(item)) {
    return compactJson(item);
  }

  const snippetContent = nonEmptyText(item.content);
  if (
    (fieldName === "snippets" || toolName === "graph_explore") &&
    snippetContent
  ) {
    return snippetContent;
  }

  const directText = compactRecordText(item);
  if (directText) {
    return directText;
  }

  const parts = [
    nonEmptyText(item.path),
    nonEmptyText(item.file),
    nonEmptyText(item.name),
    nonEmptyText(item.title),
    nonEmptyText(item.symbol),
    nonEmptyText(item.id),
    nonEmptyText(item.status),
    nonEmptyText(item.kind),
  ].filter(Boolean);

  return parts.length ? parts.join(" | ") : compactJson(item);
}

function commandOutputText(output: JsonValue | null) {
  if (typeof output === "string") {
    return output.trim() ? output : null;
  }
  if (!isJsonRecord(output)) {
    return null;
  }

  const parts: string[] = [];
  const stdout = nonEmptyText(output.stdout);
  const stderr = nonEmptyText(output.stderr);
  const error = nonEmptyText(output.error);
  if (stdout) {
    parts.push(`[stdout]\n${stdout}`);
  }
  if (stderr) {
    parts.push(`[stderr]\n${stderr}`);
  }
  if (!parts.length && error) {
    parts.push(error);
  }

  return parts.length ? parts.join("\n") : null;
}

type ManagedCommandChunk = {
  cursor: number | null;
  stream: "stdout" | "stderr";
  text: string;
};

type ManagedCommandPresentation = {
  availableFromCursor: number | null;
  chunks: ManagedCommandChunk[];
  command: string | null;
  cwd: string | null;
  cursorExpired: boolean;
  endedAt: number | null;
  exitCode: number | null;
  fromCursor: number | null;
  hasMore: boolean;
  isBackgroundStart: boolean;
  nextCursor: number | null;
  pid: number | null;
  processId: string | null;
  startedAt: number | null;
  status: string | null;
  terminationReason: string | null;
  toolCompletedAt: number | null;
  toolIsActive: boolean;
};

function managedCommandPresentation(
  toolCall: ChatToolCallSummary,
  input: JsonValue,
): ManagedCommandPresentation | null {
  const inputRecord = isJsonRecord(input) ? input : null;
  const output = isJsonRecord(toolCall.output) ? toolCall.output : null;
  const isBackgroundStart =
    toolCall.name === "run_command" && inputRecord?.background === true;
  const isManagedTool =
    isBackgroundStart ||
    toolCall.name === "get_command_output" ||
    toolCall.name === "stop_command" ||
    typeof output?.processId === "string";
  if (!isManagedTool) {
    return null;
  }

  const chunks = Array.isArray(output?.chunks)
    ? output.chunks.flatMap((chunk): ManagedCommandChunk[] => {
        if (!isJsonRecord(chunk)) {
          return [];
        }
        const stream = chunk.stream;
        const text = chunk.text;
        if (
          (stream !== "stdout" && stream !== "stderr") ||
          typeof text !== "string"
        ) {
          return [];
        }
        return [
          {
            cursor: finiteNumber(chunk.cursor),
            stream,
            text,
          },
        ];
      })
    : [];

  return {
    availableFromCursor: finiteNumber(output?.availableFromCursor),
    chunks,
    command: inputRecord ? nonEmptyText(inputRecord.command) : null,
    cwd: inputRecord ? nonEmptyText(inputRecord.cwd) : null,
    cursorExpired: output?.cursorExpired === true,
    endedAt: timestampMilliseconds(output?.endedAt),
    exitCode: finiteNumber(output?.exitCode),
    fromCursor: finiteNumber(output?.fromCursor),
    hasMore: output?.hasMore === true,
    isBackgroundStart,
    nextCursor: finiteNumber(output?.nextCursor),
    pid: finiteNumber(output?.pid),
    processId:
      nonEmptyText(output?.processId) ??
      (inputRecord ? nonEmptyText(inputRecord.processId) : null),
    startedAt: timestampMilliseconds(output?.startedAt),
    status: nonEmptyText(output?.status),
    terminationReason: nonEmptyText(output?.terminationReason),
    toolCompletedAt: timestampMilliseconds(toolCall.completedAt),
    toolIsActive: !toolCall.isError && toolCall.status !== "completed",
  };
}

function finiteNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function timestampMilliseconds(value: unknown) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? null : parsed;
  }
  return null;
}

function managedCommandDurationEndAt(
  presentation: ManagedCommandPresentation,
) {
  if (presentation.endedAt !== null) {
    return presentation.endedAt;
  }
  if (presentation.toolCompletedAt !== null) {
    return presentation.toolCompletedAt;
  }
  if (presentation.toolIsActive) {
    return Date.now();
  }
  return null;
}

function managedCommandDuration(presentation: ManagedCommandPresentation) {
  if (presentation.startedAt === null) {
    return null;
  }
  const endedAt = managedCommandDurationEndAt(presentation);
  if (endedAt === null) {
    return null;
  }
  const elapsed = Math.max(0, endedAt - presentation.startedAt);
  if (elapsed < 1_000) {
    return `${elapsed}ms`;
  }
  if (elapsed < 60_000) {
    return `${(elapsed / 1_000).toFixed(elapsed < 10_000 ? 1 : 0)}s`;
  }
  return `${Math.floor(elapsed / 60_000)}m ${Math.floor((elapsed % 60_000) / 1_000)}s`;
}

function managedCommandStatusLabel(
  presentation: ManagedCommandPresentation,
  t: Translate,
) {
  switch (presentation.status) {
    case "running":
      return presentation.isBackgroundStart
        ? t("Backgrounded")
        : t("Background running");
    case "exited":
      return presentation.exitCode === null
        ? t("Exited")
        : t("Exited · code {code}", { code: presentation.exitCode });
    case "stopped":
      return t("Stopped");
    case "timed_out":
      return t("Timed out");
    case "failed":
      return t("Failed to start");
    default:
      return null;
  }
}

function managedCommandStatusClass(presentation: ManagedCommandPresentation) {
  if (presentation.status === "running" && presentation.isBackgroundStart) {
    return "border-[var(--success)] bg-[var(--success-soft)] text-[var(--success)]";
  }
  switch (presentation.status) {
    case "running":
      return "border-[var(--warning)] bg-[var(--warning-soft)] text-[var(--warning)]";
    case "exited":
      return "border-[var(--success)] bg-[var(--success-soft)] text-[var(--success)]";
    case "stopped":
    case "timed_out":
      return "border-[var(--border)] bg-[var(--surface-secondary)] text-[var(--muted)]";
    case "failed":
      return "border-[var(--danger)] bg-[var(--danger-soft)] text-[var(--danger)]";
    default:
      return "border-[var(--border)] bg-[var(--surface-secondary)] text-[var(--muted)]";
  }
}

function managedCommandHeaderDetail(
  detailText: string,
  presentation: ManagedCommandPresentation,
  t: Translate,
) {
  const processStatus = managedCommandStatusLabel(presentation, t);
  if (!processStatus) {
    return detailText;
  }

  const commandId = presentation.processId ?? detailText;
  return [commandId, processStatus].filter(Boolean).join(" · ");
}

function CommandChunkLog({ chunks }: { chunks: ManagedCommandChunk[] }) {
  if (!chunks.length) {
    return null;
  }

  return (
    <div
      className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto border-l border-[var(--border)] pl-3 font-mono text-[11px] leading-5 text-[var(--muted)]`}
    >
      {chunks.map((chunk, index) => (
        <div
          className="mb-2 last:mb-0"
          key={`${chunk.cursor ?? "unknown"}-${index}`}
        >
          <span
            className={
              chunk.stream === "stderr" ? "text-[var(--danger)]" : "text-[var(--accent-soft-foreground)]"
            }
          >
            [{chunk.stream}
            {chunk.cursor === null ? "" : ` · ${chunk.cursor}`}]
          </span>
          <pre className="whitespace-pre-wrap break-words">{chunk.text}</pre>
        </div>
      ))}
    </div>
  );
}

function ManagedCommandSummary({
  presentation,
  toolName,
  t,
}: {
  presentation: ManagedCommandPresentation;
  toolName: string;
  t: Translate;
}) {
  const statusLabel = managedCommandStatusLabel(presentation, t);
  const duration = managedCommandDuration(presentation);
  const cursorRangeStart = presentation.cursorExpired
    ? (presentation.availableFromCursor ?? presentation.fromCursor)
    : presentation.fromCursor;
  const cursorRange =
    cursorRangeStart === null
      ? null
      : presentation.nextCursor === null
        ? String(cursorRangeStart)
        : `${cursorRangeStart}–${presentation.nextCursor}`;
  const terminal =
    presentation.status !== null && presentation.status !== "running";

  return (
    <div className="grid min-w-0 gap-2">
      <div
        className={`flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 rounded-md border px-2 py-1.5 text-[11px] ${managedCommandStatusClass(presentation)}`}
      >
        {presentation.processId ? (
          <span className="font-mono">{presentation.processId}</span>
        ) : null}
        {statusLabel ? (
          <span className="font-semibold">{statusLabel}</span>
        ) : null}
        {presentation.pid !== null ? <span>PID {presentation.pid}</span> : null}
        {duration ? <span>{duration}</span> : null}
        {presentation.terminationReason ? (
          <span>{presentation.terminationReason}</span>
        ) : null}
      </div>
      {presentation.command ? (
        <div className="min-w-0 font-mono text-[11px] text-[var(--muted)]">
          {presentation.command}
          {presentation.cwd ? ` · cwd: ${presentation.cwd}` : ""}
        </div>
      ) : null}
      {toolName === "get_command_output" && cursorRange ? (
        <div className="font-mono text-[11px] text-[var(--muted)]">
          cursor {cursorRange}
        </div>
      ) : null}
      {presentation.cursorExpired ? (
        <div className="rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-2 py-1.5 text-[11px] text-[var(--warning)]">
          {t("Earlier output was removed from the retained buffer.")}
        </div>
      ) : null}
      <CommandChunkLog chunks={presentation.chunks} />
      {!presentation.chunks.length && presentation.isBackgroundStart ? (
        <div className="font-mono text-[11px] text-[var(--muted)]">
          {t("Background process started, no output yet")}
        </div>
      ) : null}
      {!presentation.chunks.length && toolName === "get_command_output" ? (
        <div className="font-mono text-[11px] text-[var(--muted)]">
          {terminal
            ? t("Process ended, no more output")
            : t("Still running, no new output")}
        </div>
      ) : null}
      {presentation.hasMore ? (
        <div className="font-mono text-[11px] text-[var(--warning)]">
          {t("More output is available; continue with nextCursor {cursor}.", {
            cursor: presentation.nextCursor ?? "-",
          })}
        </div>
      ) : null}
      {toolName === "stop_command" ? (
        <div className="font-mono text-[11px] text-[var(--muted)]">
          {presentation.status === "stopped"
            ? t("Entire process tree terminated")
            : t("Process tree termination requested")}
        </div>
      ) : null}
    </div>
  );
}

function successfulSpecMarkdown(toolCall: ChatToolCallSummary) {
  if (
    (toolCall.name !== "read_spec" && toolCall.name !== "update_spec") ||
    toolCall.isError ||
    toolCall.status !== "completed"
  ) {
    return null;
  }
  if (!isJsonRecord(toolCall.output)) {
    return null;
  }

  const contentMarkdown = toolCall.output.contentMarkdown;
  return typeof contentMarkdown === "string" ? contentMarkdown : null;
}

function compactToolCallText(
  toolCall: ChatToolCallSummary,
  input: JsonValue,
  liveOutputText: string | null,
  compactJson: (value: JsonValue) => string,
) {
  const output = toolCall.output;

  // ponytail: UI-side heuristic summary; upgrade to a backend display payload when tool outputs get richer contracts.
  if (toolCall.isError && isJsonRecord(output)) {
    const errorText = compactRecordText(output);
    if (errorText) {
      return errorText;
    }
  }

  if (toolCall.name === "read_file" && isJsonRecord(output)) {
    const content = typeof output.content === "string" ? output.content : null;
    if (content !== null) {
      return content;
    }
  }

  if (toolCall.name === "write_file" && isJsonRecord(input)) {
    const content = typeof input.content === "string" ? input.content : null;
    if (content !== null) {
      return content;
    }
  }

  if (toolCall.name === "run_command") {
    return (
      commandOutputText(output) ??
      liveOutputText ??
      (output === null ? compactJson(input) : compactJson(output))
    );
  }

  if (typeof output === "string" && output.trim()) {
    return output;
  }

  if (isJsonRecord(output)) {
    const directText = compactRecordText(output);
    if (directText) {
      return directText;
    }
    const arraySummary = compactArraySummary(
      output,
      toolCall.name,
      compactJson,
    );
    if (arraySummary) {
      return arraySummary;
    }
  }

  return output === null ? compactJson(input) : compactJson(output);
}

function CompactReplacementDiffBlock({
  diff,
}: {
  diff: CompactReplacementDiff;
}) {
  return (
    <div className="min-w-0">
      <div className="mb-1 font-semibold text-[var(--muted)]">Diff</div>
      <div
        className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto rounded-md border border-[var(--border)] font-mono text-[11px] leading-5`}
      >
        {diff.lines.map((line, index) => (
          <div
            className={`edit-file-diff-line grid grid-cols-[1.5rem_minmax(0,1fr)] whitespace-pre-wrap break-words px-2 ${
              line.kind === "added"
                ? "bg-[var(--success-soft)] text-[var(--success)]"
                : "bg-[var(--danger-soft)] text-[var(--danger)]"
            }`}
            key={`${line.kind}-${index}`}
          >
            <span className="select-none font-semibold">
              {line.kind === "added" ? "+" : "-"}
            </span>
            <span>{line.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function RawToolCallView({
  formatJsonValue,
  input,
  liveOutputText,
  toolCall,
  t,
}: {
  formatJsonValue: (value: JsonValue) => string;
  input: JsonValue;
  liveOutputText: string | null;
  toolCall: ChatToolCallSummary;
  t: Translate;
}) {
  return (
    <>
      <div className="min-w-0">
        <div className="mb-1 font-semibold text-[var(--muted)]">{t("Input")}</div>
        <pre
          className={`${TOOL_CALL_SCROLL_CLASS} max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-[var(--border)] pl-3 font-mono text-[11px] leading-5`}
        >
          {formatJsonValue(input)}
        </pre>
      </div>
      {toolCall.output !== null ? (
        <div className="min-w-0">
          <div className="mb-1 font-semibold text-[var(--muted)]">{t("Output")}</div>
          <pre
            className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${
              toolCall.isError
                ? "border-[var(--danger)] text-[var(--danger)]"
                : "border-[var(--border)]"
            }`}
          >
            {formatJsonValue(toolCall.output)}
          </pre>
        </div>
      ) : liveOutputText ? (
        <div className="min-w-0">
          <div className="mb-1 font-semibold text-[var(--muted)]">
            {t("Live output")}
          </div>
          <pre
            className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto whitespace-pre-wrap break-words border-l border-[var(--border)] pl-3 font-mono text-[11px] leading-5 text-[var(--muted)]`}
          >
            {liveOutputText}
          </pre>
        </div>
      ) : null}
    </>
  );
}

function CompactToolCallView({
  compactJson,
  diff,
  input,
  liveOutputText,
  toolCall,
  t,
}: {
  compactJson: (value: JsonValue) => string;
  diff: CompactReplacementDiff | null;
  input: JsonValue;
  liveOutputText: string | null;
  toolCall: ChatToolCallSummary;
  t: Translate;
}) {
  if (diff) {
    return <CompactReplacementDiffBlock diff={diff} />;
  }

  const managedCommand = toolCall.isError
    ? null
    : managedCommandPresentation(toolCall, input);
  if (managedCommand) {
    return (
      <ManagedCommandSummary
        presentation={managedCommand}
        t={t}
        toolName={toolCall.name}
      />
    );
  }

  const specMarkdown = successfulSpecMarkdown(toolCall);
  if (specMarkdown !== null) {
    return (
      <div
        className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto border-l border-[var(--border)] pl-3`}
      >
        <MarkdownContent
          content={specMarkdown}
          isUser={false}
          selectedSkillPrefix={EMPTY_SELECTED_SKILL_PREFIX}
        />
      </div>
    );
  }

  const text = compactToolCallText(
    toolCall,
    input,
    liveOutputText,
    compactJson,
  );
  return (
    <pre
      className={`${TOOL_CALL_SCROLL_CLASS} max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${
        toolCall.isError
          ? "border-[var(--danger)] text-[var(--danger)]"
          : "border-[var(--border)] text-[var(--muted)]"
      }`}
    >
      {text}
    </pre>
  );
}

function BlockedToolCallDetails({
  formatJsonValue,
  input,
  payload,
  t,
}: {
  formatJsonValue: (value: JsonValue) => string;
  input: JsonValue;
  payload: ToolCallLoopGuardBlockedPayload;
  t: Translate;
}) {
  const fields = [
    [t("Execution"), t("Not executed at runtime")],
    [t("Reason"), payload.reason],
    [t("Blocked batch"), String(payload.blockedBatchIndex)],
    [
      t("Recovery"),
      t("{current}/{limit}", {
        current: payload.recoveryIndex,
        limit: payload.recoveryLimit,
      }),
    ],
    [
      t("Automatic recovery"),
      payload.recoveryAvailable ? t("Will continue") : t("Limit exhausted"),
    ],
  ];

  return (
    <div className="grid gap-2 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-2.5 py-2 text-[11px] text-[var(--warning)]">
      <div className="font-semibold">{t("Not executed at runtime")}</div>
      <div className="grid gap-1.5 border-l border-[var(--warning)] pl-2.5">
        {fields.map(([label, value]) => (
          <div className="flex min-w-0 gap-2" key={label}>
            <span className="w-28 shrink-0 font-semibold">{label}</span>
            <span className="min-w-0 break-words font-mono">{value}</span>
          </div>
        ))}
      </div>
      <div className="min-w-0">
        <div className="mb-1 font-semibold">{t("Input")}</div>
        <pre className={`${TOOL_CALL_SCROLL_CLASS} max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-[var(--warning)] pl-3 font-mono text-[11px] leading-5`}>
          {formatJsonValue(input)}
        </pre>
      </div>
    </div>
  );
}

function contextCompressionKindLabel(
  kind: "rule" | "llm" | "runtimeToolState",
  t: Translate,
) {
  if (kind === "llm") {
    return t("LLM");
  }
  if (kind === "runtimeToolState") {
    return t("Runtime tool state");
  }
  return t("Rule");
}

function contextCompressionStatusLabel(
  status: string,
  detail: Extract<ChatMessagePart, { type: "contextCompression" }>["detail"],
  t: Translate,
) {
  if (status === "start") {
    return t("Compressing");
  }
  if (status === "retrying") {
    return t("Retrying compression");
  }
  if (status === "completed") {
    return t("Compressed");
  }
  if (status === "skipped") {
    return t("Skipped; continuing chat");
  }
  if (status === "failed") {
    if (detail.compressionMode === "required_overflow") {
      return t("Compression failed; context is still too large");
    }
    return t("Compression failed");
  }
  if (status === "cancelled") {
    return t("Compression cancelled");
  }
  return status || "-";
}

function formatContextCompressionTokenDelta(
  originalTokenCount: number | null | undefined,
  summaryTokenCount: number | null | undefined,
  t: Translate,
) {
  if (
    typeof originalTokenCount !== "number" ||
    typeof summaryTokenCount !== "number"
  ) {
    return "-";
  }
  const savedTokens = Math.max(0, originalTokenCount - summaryTokenCount);
  return t("Saved {count} tokens", { count: savedTokens.toLocaleString() });
}

function ContextCompressionBlock({
  compression,
  helpers,
}: {
  compression: Extract<ChatMessagePart, { type: "contextCompression" }>;
  helpers: ChatPanelHelpers;
}) {
  const { formatChatCreatedAt } = helpers;
  const { t } = useI18n();
  const detail = compression.detail;
  const kindLabel = contextCompressionKindLabel(compression.kind, t);
  const statusLabel = contextCompressionStatusLabel(compression.status, detail, t);
  const isPending =
    compression.status === "start" || compression.status === "retrying";
  const degraded = detail.action === "continue_without_compression";
  const originalTokenCount = detail.originalTokenCount ?? null;
  const summaryTokenCount = detail.summaryTokenCount ?? null;
  const savedLabel = formatContextCompressionTokenDelta(
    originalTokenCount,
    summaryTokenCount,
    t,
  );
  const modelLabel =
    [detail.providerId, detail.modelId].filter(Boolean).join(" / ") || "-";
  const fields = [
    [t("Original tokens"), originalTokenCount?.toLocaleString() ?? "-"],
    [t("Compressed tokens"), summaryTokenCount?.toLocaleString() ?? "-"],
    [
      t("Started"),
      detail.startedAt ? formatChatCreatedAt(detail.startedAt) : "-",
    ],
    [
      t("Ended"),
      detail.completedAt ? formatChatCreatedAt(detail.completedAt) : "-",
    ],
    [t("Provider"), detail.providerId || "-"],
    [t("Model"), detail.modelId || "-"],
    [t("Provider request ID"), detail.providerRequestId ?? "-"],
    ["snapshotId", detail.snapshotId ?? "-"],
    [t("Compression ID"), detail.compressionId ?? "-"],
    ["kind", compression.kind],
    [t("Mode"), detail.compressionMode ?? "-"],
    [
      t("Attempt"),
      typeof detail.attemptIndex === "number" ? String(detail.attemptIndex + 1) : "-",
    ],
    [t("Outcome"), detail.outcome ?? "-"],
    [t("Action"), detail.action ?? "-"],
    [t("Degraded"), degraded ? t("Yes") : t("No")],
    ...(detail.errorMessage ? [[t("Error"), detail.errorMessage]] : []),
  ];

  return (
    <details
      aria-busy={isPending ? true : undefined}
      className="tool-call-block group min-w-0"
    >
      <summary className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-[var(--muted)] marker:hidden">
        <Shrink
          aria-hidden="true"
          className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
        />
        <span className="min-w-0 shrink-0 truncate">
          {t("Context compression")}
        </span>
        <span className="shrink-0 text-[var(--muted)]">·</span>
        <span className="shrink-0 text-[var(--muted)]">{kindLabel}</span>
        <span
          className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-[var(--muted)]"
          title={`${savedLabel} · ${modelLabel}`}
        >
          {savedLabel} · {modelLabel}
        </span>
        <span
          aria-live={isPending ? "polite" : undefined}
          className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-4 ${
            compression.status === "completed"
              ? "bg-[var(--success-soft)] text-[var(--success)]"
              : isPending
                ? "bg-[var(--warning-soft)] text-[var(--warning)]"
                : compression.status === "failed"
                  ? "bg-[var(--danger-soft)] text-[var(--danger)]"
                : "bg-[var(--surface-secondary)] text-[var(--muted)]"
          }`}
        >
          {statusLabel}
        </span>
        <span className="sr-only">
          {isPending
            ? t("Context compression in progress")
            : compression.status === "completed"
              ? t("Context compression completed")
              : statusLabel}
        </span>
      </summary>
      <div className="mt-2 grid gap-1.5 border-l border-[var(--border)] pl-3 text-[11px] text-[var(--muted)]">
        {fields.map(([label, value]) => (
          <div className="flex min-w-0 gap-2" key={label}>
            <span className="w-32 shrink-0 font-semibold text-[var(--muted)]">
              {label}
            </span>
            <span className="min-w-0 flex-1 truncate font-mono" title={value}>
              {value}
            </span>
          </div>
        ))}
      </div>
    </details>
  );
}

function ToolCallBlock({
  helpers,
  toolCall,
  workspaceId,
}: {
  helpers: ChatPanelHelpers;
  toolCall: ChatToolCallSummary;
  workspaceId: string | null;
}) {
  const {
    compactToolJson,
    formatChatCreatedAt,
    formatJsonValue,
    normalizedToolInput,
    toolCallChangeStats,
    toolCallDetailText,
    toolLiveOutputText,
    toolStatusText,
  } = helpers;
  const { language, t } = useI18n();
  const [viewMode, setViewMode] = useState<ToolCallViewMode>("compact");
  const toolCallRootRef = useRef<HTMLDivElement>(null);
  const input = normalizedToolInput(toolCall.input);
  const blockedPayload = isToolCallLoopGuardBlockedPayload(toolCall.output)
    ? toolCall.output
    : null;
  const isBlocked = blockedPayload !== null;
  const compactReplacementDiff = successfulCompactReplacementDiff(
    toolCall,
    input,
  );
  const detailText = toolCallDetailText(toolCall);
  const changeStats = toolCallChangeStats(toolCall);
  const liveOutputText = toolLiveOutputText(toolCall.liveOutput);
  const managedCommand = managedCommandPresentation(toolCall, input);
  const managedStatusLabel = managedCommand
    ? managedCommandStatusLabel(managedCommand, t)
    : null;
  const summaryDetailText =
    toolCall.name === "get_command_output" && managedCommand
      ? managedCommandHeaderDetail(detailText, managedCommand, t)
      : detailText;
  const completedManagedCommand =
    !toolCall.isError &&
    (toolCall.name === "get_command_output" || toolCall.name === "stop_command") &&
    toolCall.status === "completed";
  const summaryStatusLabel =
    isBlocked
      ? t("Tool call blocked")
      : completedManagedCommand
      ? toolStatusText(toolCall, t)
      : !toolCall.isError && managedStatusLabel
      ? managedStatusLabel
      : toolStatusText(toolCall, t);
  const summaryStatusClass = isBlocked
    ? "bg-[var(--warning-soft)] text-[var(--warning)]"
    : toolCall.isError
    ? "bg-[var(--danger-soft)] text-[var(--danger)]"
    : !completedManagedCommand && managedCommand && managedStatusLabel
      ? managedCommandStatusClass(managedCommand)
      : toolCall.status === "completed"
        ? "bg-[var(--success-soft)] text-[var(--success)]"
        : "bg-[var(--surface-secondary)] text-[var(--muted)]";
  const generatedImages = toolCall.isError
    ? []
    : generatedImageFiles(toolCall.name, toolCall.output);
  const toggleLabel = viewMode === "compact" ? t("Raw") : t("Compact");
  const displayName = toolDisplayName(toolCall.name, language);
  const ToolIcon = TOOL_CALL_ICONS[toolCall.name] ?? Wrench;

  useEffect(() => {
    const root = toolCallRootRef.current;
    if (!root) {
      return;
    }

    const handleWheel = (event: WheelEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }
      const scroller = target.closest(".tool-call-scroll");
      if (!(scroller instanceof HTMLElement) || !root.contains(scroller)) {
        return;
      }
      forwardWheelAtVerticalBoundary(event, scroller);
    };

    root.addEventListener("wheel", handleWheel, { passive: false });
    return () => {
      root.removeEventListener("wheel", handleWheel);
    };
  }, []);

  return (
    <div className="grid min-w-0 gap-2" ref={toolCallRootRef}>
      <GeneratedImageFilesBlock
        files={generatedImages}
        formatFileSize={helpers.formatFileSize}
        workspaceId={workspaceId}
      />
      <details className="tool-call-block group min-w-0">
        <summary
          aria-label={
            isBlocked
              ? `${displayName} (${toolCall.name}) · ${summaryStatusLabel}`
              : `${displayName} (${toolCall.name})`
          }
          className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-[var(--muted)] marker:hidden"
          title={toolCall.name}
        >
          <ToolIcon
            aria-hidden="true"
            className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
          />
          <span className="min-w-0 shrink-0 truncate">{displayName}</span>
          {changeStats ? (
            <span className="shrink-0 rounded bg-[var(--surface-secondary)] px-1.5 py-0.5 font-mono text-[10px] leading-4 text-[var(--muted)]">
              <span className="text-[var(--success)]">
                +{changeStats.linesAdded}
              </span>{" "}
              <span className="text-[var(--danger)]">-{changeStats.linesRemoved}</span>
            </span>
          ) : null}
          {summaryDetailText ? (
            <span className="shrink-0 text-[var(--muted)]">·</span>
          ) : null}
          {summaryDetailText ? (
            <span
              className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-[var(--muted)]"
              title={summaryDetailText}
            >
              {summaryDetailText}
            </span>
          ) : null}
          <span
            className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-4 ${summaryStatusClass}`}
          >
            {summaryStatusLabel}
          </span>
        </summary>
        <div className="mt-2 grid gap-2 text-xs text-[var(--muted)]">
          <div className="flex min-w-0 items-start justify-between gap-2 text-[11px] text-[var(--muted)]">
            <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
              <span>
                <span className="font-semibold text-[var(--muted)]">
                  {t("Started")}
                </span>{" "}
                <span>
                  {toolCall.startedAt
                    ? formatChatCreatedAt(toolCall.startedAt)
                    : "-"}
                </span>
              </span>
              <span>
                <span className="font-semibold text-[var(--muted)]">
                  {t("Ended")}
                </span>{" "}
                <span>
                  {toolCall.completedAt
                    ? formatChatCreatedAt(toolCall.completedAt)
                    : "-"}
                </span>
              </span>
            </div>
            {!isBlocked ? (
              <Button
                aria-label={toggleLabel}
                className="h-5 min-h-0 shrink-0 rounded border border-[var(--border)] bg-[var(--surface)] px-2 py-0 text-[11px] leading-4 font-semibold text-[var(--muted)] hover:border-[var(--border)] hover:bg-[var(--surface-secondary)] focus:outline-none focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
                onPress={() =>
                  setViewMode(viewMode === "compact" ? "raw" : "compact")
                }
                type="button"
                variant="ghost"
              >
                {toggleLabel}
              </Button>
            ) : null}
          </div>
          {isBlocked ? (
            <BlockedToolCallDetails
              formatJsonValue={formatJsonValue}
              input={input}
              payload={blockedPayload}
              t={t}
            />
          ) : viewMode === "compact" ? (
            <CompactToolCallView
              compactJson={compactToolJson}
              diff={compactReplacementDiff}
              input={input}
              liveOutputText={liveOutputText}
              t={t}
              toolCall={toolCall}
            />
          ) : (
            <RawToolCallView
              formatJsonValue={formatJsonValue}
              input={input}
              liveOutputText={liveOutputText}
              toolCall={toolCall}
              t={t}
            />
          )}
        </div>
      </details>
    </div>
  );
}
function GeneratedImageFilesBlock({
  files,
  formatFileSize,
  workspaceId,
}: {
  files: GeneratedImageFile[];
  formatFileSize: (sizeBytes: number) => string;
  workspaceId: string | null;
}) {
  if (!files.length) {
    return null;
  }

  return (
    <div className="generated-image-file-list">
      {files.map((file) => (
        <figure className="generated-image-file" key={file.path}>
          {workspaceId ? (
            <img
              alt={file.path}
              src={workspaceImageBlobUrl(workspaceId, file.path)}
            />
          ) : null}
          <figcaption>
            <span className="generated-image-file-path" title={file.path}>
              {file.path}
            </span>
            {file.bytes === null ? null : (
              <span className="generated-image-file-size">
                {formatFileSize(file.bytes)}
              </span>
            )}
          </figcaption>
        </figure>
      ))}
    </div>
  );
}

function workspaceImageBlobUrl(workspaceId: string, path: string) {
  return `/api/workspaces/${encodeURIComponent(workspaceId)}/files/blob?path=${encodeURIComponent(path)}`;
}

function generatedImageFiles(
  toolName: string,
  output: JsonValue | null,
): GeneratedImageFile[] {
  const files = new Map<string, GeneratedImageFile>();
  collectGeneratedImageFiles(
    output,
    files,
    toolName === "agent_wait_tasks" || toolName === "agent_delegate_task",
  );
  return Array.from(files.values()).slice(0, MAX_GENERATED_IMAGE_PREVIEWS);
}

function collectGeneratedImageFiles(
  value: JsonValue | null | undefined,
  files: Map<string, GeneratedImageFile>,
  includeTextPaths: boolean,
) {
  if (!value || files.size >= MAX_GENERATED_IMAGE_PREVIEWS) {
    return;
  }

  if (typeof value === "string") {
    if (includeTextPaths) {
      for (const path of generatedImagePathsFromText(value)) {
        files.set(path, { bytes: null, mimeType: null, path });
        if (files.size >= MAX_GENERATED_IMAGE_PREVIEWS) {
          return;
        }
      }
    }
    return;
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      collectGeneratedImageFiles(item, files, includeTextPaths);
      if (files.size >= MAX_GENERATED_IMAGE_PREVIEWS) {
        return;
      }
    }
    return;
  }

  if (typeof value !== "object") {
    return;
  }

  const directFiles = value.files;
  if (Array.isArray(directFiles)) {
    for (const item of directFiles) {
      const file = generatedImageFile(item);
      if (file) {
        files.set(file.path, file);
      }
      if (files.size >= MAX_GENERATED_IMAGE_PREVIEWS) {
        return;
      }
    }
  }

  for (const item of Object.values(value)) {
    collectGeneratedImageFiles(item, files, includeTextPaths);
    if (files.size >= MAX_GENERATED_IMAGE_PREVIEWS) {
      return;
    }
  }
}

function generatedImageFile(value: JsonValue): GeneratedImageFile | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }

  const path = value.path;
  const mimeType = value.mimeType;
  const bytes = value.bytes;

  if (typeof path !== "string" || !path.trim()) {
    return null;
  }
  if (typeof mimeType !== "string" || !mimeType.startsWith("image/")) {
    return null;
  }

  return {
    bytes: typeof bytes === "number" && Number.isFinite(bytes) ? bytes : null,
    mimeType: typeof mimeType === "string" ? mimeType : null,
    path,
  };
}

function generatedImagePathsFromText(text: string) {
  // ponytail: text fallback handles unquoted no-space image paths; use files[] for arbitrary paths.
  const matches = text.match(/[^\s`"'<>]+?\.(?:png|jpe?g|webp|gif)\b/gi) ?? [];
  return matches
    .map((path) => path.trim().replace(/^[([]+|[),.;:\]]+$/g, ""))
    .filter((path) => path && !path.includes("://"));
}
