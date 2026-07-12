import {
  ArrowUp,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Copy,
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
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

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
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  ContextUsageResponse,
  GitBranchesResponse,
  GitWorktreeSummary,
  JsonValue,
  SettingsResponse,
  ShellMessage,
  ThinkingLevelSummary,
  Translate,
  WorkspaceSummary,
} from "../../api/types";
import { CHAT_BOTTOM_LOCK_THRESHOLD_PX, CREATE_BRANCH_OPTION_VALUE } from "../../app/constants";
import { useI18n } from "../../shared/i18n";
import { thinkingLevelOptionsForModel } from "../../shared/thinking-levels";
import { selectedSkillPrefix, toolDisplayName } from "./chat-helpers";
import { MarkdownContent, type SelectedSkillPrefixResolver } from "./MarkdownContent";

const COMPOSER_EDITOR_MIN_HEIGHT_PX = 68;
const COMPOSER_EDITOR_KEY_STEP_PX = 24;
const COMPOSER_EDITOR_MAX_HEIGHT_RATIO = 0.55;
const CHAT_TOP_LOAD_THRESHOLD_PX = 64;
const MAX_GENERATED_IMAGE_PREVIEWS = 16;

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
  run_command: Terminal,
  screenshot: Globe,
  search_query: Search,
  search_text: Search,
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
  formatNullableLatencySeconds: (value: number | null, language: string) => string;
  formatReplyDuration: (value: number | null, language: string) => string;
  formatTokensPerSecond: (metrics: ChatReplyMetrics, language: string) => string;
  messageCopyText: (message: ShellMessage, parts: ChatMessagePart[]) => string;
  removeActiveSkillToken: (value: string) => string;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  skillScopeLabel: (skill: ConfiguredSkillSummary, t: Translate) => string;
  toolCallChangeStats: (toolCall: ChatToolCallSummary) => ToolCallChangeStats | null;
  normalizedToolInput: (value: JsonValue) => JsonValue;
  toolCallDetailText: (toolCall: ChatToolCallSummary) => string;
  toolLiveOutputText: (liveOutput: ChatToolLiveOutput | undefined) => string | null;
  toolStatusText: (toolCall: ChatToolCallSummary, t: Translate) => string;
};

function ChatPanelComponent({
  activeWorkspaceName,
  availableModels,
  branchError,
  chatScrollKey,
  canGuideActiveRun,
  canRetryRun,
  contextUsage,
  draftAttachments,
  draftMessage,
  draftUnsupportedAttachmentMessage,
  gitBranches,
  hasMoreMessagesBefore,
  helpers,
  queuedRunCount,
  readOnly,
  isLoadingBranches,
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
  onBranchChange,
  onBranchMenuOpen,
  onCancelRun,
  onDraftMessageChange,
  onEditMessage,
  onGuideActiveRun,
  onSelectEditAttachments,
  onGuideQueuedMessage,
  onLoadMoreMessages,
  onModelChange,
  onOpenMessageApiRequests,
  onProviderChange,
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
  selectedGitBranch,
  selectedModelId,
  selectedProviderId,
  selectedSkillIds,
  selectedThinkingLevel,
  settings,
  providers,
  skills,
  queuedMessageIds,
  thinkingLevels,
  worktreeBranch,
  workspaces,
  workspaceId,
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
  draftUnsupportedAttachmentMessage: string | null;
  gitBranches: GitBranchesResponse | null;
  hasMoreMessagesBefore: boolean;
  helpers: ChatPanelHelpers;
  queuedRunCount: number;
  readOnly: boolean;
  isLoadingBranches: boolean;
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
  onBranchChange: (value: string) => void;
  onBranchMenuOpen: () => void;
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onEditMessage: (
    message: ShellMessage,
    content: string,
    selectedSkillIds: string[],
    attachments: ComposerAttachment[],
    onAccepted: () => void,
  ) => Promise<boolean>;
  onSelectEditAttachments: (onSelected: (attachments: ComposerAttachment[]) => void) => void;
  onGuideActiveRun: () => void;
  onGuideQueuedMessage: (messageId: string) => void;
  onLoadMoreMessages: () => Promise<void>;
  onModelChange: (value: string) => void;
  onOpenMessageApiRequests: (message: ShellMessage) => void;
  onProviderChange: (value: string) => void;
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
  selectedGitBranch: string;
  selectedModelId: string;
  selectedProviderId: string;
  selectedSkillIds: string[];
  selectedThinkingLevel: string;
  settings: SettingsResponse | null;
  providers: ConfiguredProviderSummary[];
  skills: ConfiguredSkillSummary[];
  queuedMessageIds: ReadonlySet<string>;
  thinkingLevels: ThinkingLevelSummary[];
  worktreeBranch: string | null;
  workspaces: WorkspaceSummary[];
  workspaceId: string | null;
}) {
  const {
    activeSkillQuery,
    removeActiveSkillToken,
    skillScopeLabel,
  } = helpers;
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
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [editingMessageText, setEditingMessageText] = useState("");
  const [editingSkillIds, setEditingSkillIds] = useState<string[]>([]);
  const [editingAttachments, setEditingAttachments] = useState<ComposerAttachment[]>([]);
  const [isSavingEditedMessage, setIsSavingEditedMessage] = useState(false);
  const [isCtrlKeyPressed, setIsCtrlKeyPressed] = useState(false);
  const [isResizingComposer, setIsResizingComposer] = useState(false);
  const [isSendButtonTooltipOpen, setIsSendButtonTooltipOpen] = useState(false);
  const [composerEditorHeight, setComposerEditorHeight] = useState(
    COMPOSER_EDITOR_MIN_HEIGHT_PX,
  );
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = useMemo(() => new Set(selectedSkillIds), [selectedSkillIds]);
  const selectedSkills = useMemo(
    () =>
      selectedSkillIds
        .map((skillId) => skills.find((skill) => skill.key === skillId))
        .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill)),
    [selectedSkillIds, skills],
  );
  const workspaceName = activeWorkspaceName?.trim();
  const composerPlaceholder = workspaceName
    ? t("Ask Foco anything about {name}...", { name: workspaceName })
    : t("Ask Foco anything...");
  const modelProviderGroups = useMemo(() => {
    const providersById = new Map(providers.map((provider) => [provider.id, provider]));
    const providerIdsForAvailableModels = Array.from(
      new Set(availableModels.flatMap((model) => model.providerIds)),
    );
    return [
      ...providers
        .map((provider) => provider.id)
        .filter((providerId) => providerIdsForAvailableModels.includes(providerId)),
      ...providerIdsForAvailableModels.filter(
        (providerId) => !providersById.has(providerId),
      ),
    ].map((providerId) => ({
      providerId,
      providerLabel: providersById.get(providerId)?.name ?? providerId,
      models: availableModels
        .filter((model) => model.providerIds.includes(providerId))
        .map((model) => ({
          label: model.displayName,
          value: model.id,
        })),
    }));
  }, [availableModels, providers]);
  const selectedModel = useMemo(
    () => availableModels.find((model) => model.id === selectedModelId) ?? null,
    [availableModels, selectedModelId],
  );
  const thinkingOptions = useMemo(
    () => [
      { label: t("Model default"), value: "" },
      ...thinkingLevelOptionsForModel(selectedModel, thinkingLevels).map((level) => ({
        label: t(level.label),
        value: level.value,
      })),
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
  const hasComposerDraft = Boolean(draftMessage.trim() || draftAttachments.length);
  const runningButtonSendsMessage = isSendingMessage && hasComposerDraft;
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
    element.scrollTop += Math.max(0, element.scrollHeight - previousScrollHeight);
  }, [messages.length]);

  useLayoutEffect(() => {
    const element = messageScrollRef.current;
    const chatChanged = previousChatScrollKeyRef.current !== chatScrollKey;
    const wasEmpty = previousMessageCountRef.current === 0;
    previousChatScrollKeyRef.current = chatScrollKey;
    previousMessageCountRef.current = messages.length;

    if (messages.length === 0) {
      shouldLockMessageScrollRef.current = false;
      if (element) {
        element.scrollTop = 0;
      }
      return;
    }

    if (chatChanged || wasEmpty) {
      shouldLockMessageScrollRef.current = true;
      scrollMessageListToBottom();
    }
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
      chatPanelRef.current?.getBoundingClientRect().height ?? window.innerHeight;
    // ponytail: one shared drag ceiling for desktop/mobile; split per breakpoint if UX needs it.
    return Math.max(
      COMPOSER_EDITOR_MIN_HEIGHT_PX,
      Math.floor(panelHeight * COMPOSER_EDITOR_MAX_HEIGHT_RATIO),
    );
  }

  function clampComposerEditorHeight(value: number, maxHeight = composerEditorMaxHeight()) {
    return Math.min(Math.max(value, COMPOSER_EDITOR_MIN_HEIGHT_PX), maxHeight);
  }

  function resizeComposerEditorBy(delta: number) {
    setComposerEditorHeight((current) => clampComposerEditorHeight(current + delta));
  }

  function handleComposerResizePointerDown(event: ReactPointerEvent<HTMLDivElement>) {
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

  function handleMessageScroll() {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }

    if (messages.length === 0) {
      shouldLockMessageScrollRef.current = false;
      userMessageScrollIntentRef.current = false;
      return;
    }

    const isAtBottom =
      element.scrollHeight - element.scrollTop - element.clientHeight <=
      CHAT_BOTTOM_LOCK_THRESHOLD_PX;
    if (isAtBottom || userMessageScrollIntentRef.current) {
      shouldLockMessageScrollRef.current = isAtBottom;
    }
    userMessageScrollIntentRef.current = false;

    if (
      element.scrollTop <= CHAT_TOP_LOAD_THRESHOLD_PX &&
      hasMoreMessagesBefore &&
      !isLoadingMoreMessages
    ) {
      requestMoreMessages();
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

  function handleRunningRunButtonClick(
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    const hasDraft = Boolean(draftMessage.trim() || draftAttachments.length);
    if (!hasDraft) {
      onCancelRun();
      return;
    }

    if (isQueueModifierActive(event)) {
      onQueueActiveRun();
      return;
    }

    onGuideActiveRun();
  }

  function handleModelProviderChange(providerId: string, modelId: string) {
    if (modelId !== selectedModelId) {
      onModelChange(modelId);
    }
    if (providerId !== selectedProviderId) {
      onProviderChange(providerId);
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

  const handleCopyMessage = useCallback(async (messageId: string, text: string) => {
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
  }, []);

  const beginEditingMessage = useCallback((message: ShellMessage) => {
    const persistedSkillIds = message.runConfig?.selectedSkillIds;
    const legacySelectedSkills = persistedSkillIds
      ? []
      : selectedSkillPrefix(message.content, true)?.skills ?? [];
    const legacySkillIds = legacySelectedSkills
      .map((selectedSkill) => skills.find((skill) =>
        skill.name === selectedSkill.name || skill.path === selectedSkill.path
      )?.key)
      .filter((skillId): skillId is string => Boolean(skillId));
    setEditingMessageId(message.id);
    setEditingMessageText(message.content);
    setEditingSkillIds(persistedSkillIds ?? legacySkillIds);
    setEditingAttachments([]);
  }, [skills]);

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

  const saveEditedMessage = useCallback(async (message: ShellMessage) => {
    const trimmed = editingMessageText.trim();
    if (!trimmed || isSavingEditedMessage) {
      return;
    }
    const messageIndex = messages.findIndex((item) => item.id === message.id);
    const removedCount = messageIndex < 0 ? 0 : messages.length - messageIndex - 1;
    if (
      removedCount > 0 &&
      !window.confirm(t("Editing this message will remove {count} later messages and regenerate the reply. Continue?", { count: removedCount }))
    ) {
      return;
    }
    setIsSavingEditedMessage(true);
    let editAccepted = false;
    try {
      await onEditMessage(message, trimmed, editingSkillIds, editingAttachments, () => {
        if (editAccepted) {
          return;
        }
        editAccepted = true;
        clearEditingMessage();
      });
    } finally {
      setIsSavingEditedMessage(false);
    }
  }, [clearEditingMessage, editingAttachments, editingMessageText, editingSkillIds, isSavingEditedMessage, messages, onEditMessage, t]);

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
        onKeyDown={markUserMessageScrollIntent}
        onScroll={handleMessageScroll}
        onTouchMove={markUserMessageScrollIntent}
        onWheel={markUserMessageScrollIntent}
        ref={messageScrollRef}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${messages.length ? "max-w-5xl gap-4" : "max-w-6xl"
            }`}
          ref={messageScrollContentRef}
        >
          {messages.length ? (
            <>
              {hasMoreMessagesBefore || isLoadingMoreMessages ? (
                <div className="flex justify-center">
                  <button
                    className="chat-toolbar-button inline-flex items-center gap-2 rounded-lg border border-stone-200 bg-white/80 px-3 py-1.5 text-xs font-semibold text-stone-600"
                    disabled={isLoadingMoreMessages}
                    onClick={requestMoreMessages}
                    type="button"
                  >
                    {isLoadingMoreMessages ? (
                      <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                    ) : (
                      <ArrowUp aria-hidden="true" className="size-3.5" />
                    )}
                    <span>{isLoadingMoreMessages ? t("Loading...") : t("Load earlier messages")}</span>
                  </button>
                </div>
              ) : null}
              {messages.map((message) => (
                <MessageRow
                  canEdit={
                    !readOnly &&
                    !isSendingMessage &&
                    !message.pendingMode &&
                    message.role === "user"
                  }
                  editingAttachments={editingAttachments}
                  editingSkillIds={editingSkillIds}
                  editingText={editingMessageText}
                  helpers={helpers}
                  isCopied={copiedMessageId === message.id}
                  isEditing={editingMessageId === message.id}
                  isSavingEdit={isSavingEditedMessage && editingMessageId === message.id}
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
            <div className="flex min-h-48 items-center justify-center gap-2 text-sm font-medium text-stone-500">
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              <span>{t("Loading...")}</span>
            </div>
          ) : readOnly ? (
            <div className="flex min-h-48 items-center justify-center text-sm font-medium text-stone-500">
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
            className={`composer-resize-splitter ${isResizingComposer ? "composer-resize-splitter-active" : ""
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
                    helpers={helpers}
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
              className={`message-composer-control-row ${canRetryRun ? "message-composer-actions-with-retry" : ""
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
              <button
                aria-label={t("Plan mode")}
                aria-pressed={isPlanModeEnabled}
                className={`composer-team-toggle ${isPlanModeEnabled
                    ? "composer-team-toggle-enabled"
                    : ""
                  }`}
                onClick={() => onPlanModeEnabledChange(!isPlanModeEnabled)}
                title={t("Plan mode")}
                type="button"
              >
                <ListChecks aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="composer-team-toggle-label">{t("Plan")}</span>
              </button>
              <ComposerModelProviderMenu
                ariaLabel={t("Model")}
                className="composer-model-provider-select max-w-full"
                disabled={isLoadingSettings || !modelProviderGroups.length}
                emptyLabel={t("No enabled models")}
                groups={modelProviderGroups}
                onChange={handleModelProviderChange}
                selectedModelId={selectedModelId}
                selectedProviderId={selectedProviderId}
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
                branches={worktreeBranch ? [worktreeBranch] : gitBranches?.branches ?? []}
                currentBranch={worktreeBranch ?? selectedGitBranch}
                currentWorktreeBranch={worktreeBranch}
                disabled={isSendingMessage || worktreeBranch !== null}
                isGitRepository={worktreeBranch !== null || (gitBranches?.isGitRepository ?? false)}
                isLoading={isLoadingBranches}
                onChange={onBranchChange}
                onOpen={onBranchMenuOpen}
                worktrees={gitBranches?.worktrees ?? []}
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
                      ? "composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(200,101,27,0.24)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                      : "composer-run-button inline-flex size-8 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                  }
                  disabled={
                    runningButtonSendsMessage &&
                    (!canGuideActiveRun ||
                      !selectedModelId ||
                      Boolean(draftUnsupportedAttachmentMessage))
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
                <span
                  className="composer-send-button-shell"
                  onBlur={() => setIsSendButtonTooltipOpen(false)}
                  onFocus={() => setIsSendButtonTooltipOpen(true)}
                  onMouseEnter={() => setIsSendButtonTooltipOpen(true)}
                  onMouseLeave={() => setIsSendButtonTooltipOpen(false)}
                >
                  <button
                    aria-describedby={
                      showSendButtonTooltip ? "composer-send-button-tooltip" : undefined
                    }
                    aria-label={t("Send message")}
                    className="composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(200,101,27,0.24)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                    disabled={
                      (!draftMessage.trim() && !draftAttachments.length) ||
                      !selectedModelId ||
                      Boolean(draftUnsupportedAttachmentMessage)
                    }
                    onClick={(event) => {
                      if (isQueueModifierActive(event)) {
                        event.preventDefault();
                        const form = event.currentTarget.form;
                        if (!form) {
                          return;
                        }

                        onSubmit(event as unknown as FormEvent<HTMLFormElement>, {
                          schedule: true,
                        });
                      }
                    }}
                    title={sendButtonTitle}
                    type="submit"
                  >
                    <Send aria-hidden="true" className="size-4" />
                  </button>
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
          {branchError ? (
            <div className="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {branchError}
            </div>
          ) : null}
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
  onSelectEditAttachments: (onSelected: (attachments: ComposerAttachment[]) => void) => void;
  onWithdrawQueuedMessage: (messageId: string) => void;
  queuedMessageIds: ReadonlySet<string>;
  skills: ConfiguredSkillSummary[];
  workspaceId: string | null;
}) {
  const { fallbackMessageParts, formatChatCreatedAt, messageCopyText } = helpers;
  const { t } = useI18n();
  const isUser = message.role === "user";
  const parts = useMemo(
    () => (message.parts.length ? message.parts : fallbackMessageParts(message)),
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
          className={`message-bubble flex max-w-[min(42rem,92%)] items-start gap-3 rounded-2xl border px-4 py-3 shadow-[0_18px_42px_rgba(75,63,42,0.08)] sm:max-w-[78%] ${isUser
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
            className={`message-avatar mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${isUser
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
                {!isUser && message.runBadges?.includes("contextCompressionRule") ? (
                  <span
                    className="message-run-badge"
                    title={t("Rule-based context compression was triggered")}
                  >
                    {t("Rule compressed")}
                  </span>
                ) : null}
                {!isUser && message.runBadges?.includes("contextCompressionLlm") ? (
                  <span
                    className="message-run-badge"
                    title={t("LLM summary context compression was triggered")}
                  >
                    {t("LLM compressed")}
                  </span>
                ) : null}
                {!isUser && message.runBadges?.includes("contextCompressionRuntime") ? (
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
                  <button
                    aria-label={t("Edit message")}
                    className="message-action-menu"
                    onClick={() => onBeginEdit(message)}
                    title={t("Edit message")}
                    type="button"
                  >
                    <Pencil aria-hidden="true" className="size-3.5" />
                  </button>
                ) : null}
                {canManageQueuedMessage ? (
                  <>
                    <button
                      aria-label={t("Convert queued message to guidance")}
                      className="message-action-menu"
                      onClick={() => onGuideQueuedMessage(message.id)}
                      title={t("Convert queued message to guidance")}
                      type="button"
                    >
                      <ArrowUp aria-hidden="true" className="size-3.5" />
                    </button>
                    <button
                      aria-label={t("Withdraw queued message")}
                      className="message-action-menu"
                      onClick={() => onWithdrawQueuedMessage(message.id)}
                      title={t("Withdraw queued message")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                    </button>
                  </>
                ) : null}
                <button
                  aria-label={copyLabel}
                  className="message-action-menu"
                  disabled={!copyText}
                  onClick={() => onCopyMessage(message.id, copyText)}
                  title={copyLabel}
                  type="button"
                >
                  {isCopied ? (
                    <CheckCircle2 aria-hidden="true" className="size-3.5" />
                  ) : (
                    <Copy aria-hidden="true" className="size-3.5" />
                  )}
                </button>
              </span>
            </div>
            {isEditing ? (
              <div className="space-y-2">
                {editingSkillIds.length ? (
                  <div className="flex flex-wrap gap-1.5">
                    {editingSkillIds.map((skillId) => {
                      const skill = skills.find((item) => item.key === skillId);
                      return (
                        <button
                          className="rounded-full border border-teal-800/20 bg-white/70 px-2 py-0.5 text-xs text-teal-950"
                          key={skillId}
                          onClick={() => onEditingSkillIdsChange(editingSkillIds.filter((id) => id !== skillId))}
                          title={t("Remove skill")}
                          type="button"
                        >
                          {skill?.name ?? skillId} ×
                        </button>
                      );
                    })}
                  </div>
                ) : null}
                <div className="flex flex-wrap items-center gap-1.5">
                  <select
                    aria-label={t("Add skill")}
                    className="rounded-lg border border-stone-300 bg-white/90 px-2 py-1 text-xs"
                    onChange={(event) => {
                      const skillId = event.target.value;
                      if (skillId && !editingSkillIds.includes(skillId)) {
                        onEditingSkillIdsChange([...editingSkillIds, skillId]);
                      }
                      event.target.value = "";
                    }}
                    value=""
                  >
                    <option value="">{t("Add skill")}</option>
                    {skills.filter((skill) => !editingSkillIds.includes(skill.key)).map((skill) => (
                      <option key={skill.key} value={skill.key}>{skill.name}</option>
                    ))}
                  </select>
                  <button
                    className="rounded-lg border border-stone-300 bg-white/90 px-2 py-1 text-xs"
                    onClick={() => onSelectEditAttachments((attachments) => onEditingAttachmentsChange([...editingAttachments, ...attachments]))}
                    type="button"
                  >
                    {t("Add attachment")}
                  </button>
                </div>
                {editingAttachments.length ? (
                  <div className="flex flex-wrap gap-1.5">
                    {editingAttachments.map((attachment) => (
                      <button
                        className="rounded-full border border-stone-300 bg-white/70 px-2 py-0.5 text-xs"
                        key={attachment.id}
                        onClick={() => onEditingAttachmentsChange(editingAttachments.filter((item) => item.id !== attachment.id))}
                        title={t("Remove attachment {name}", { name: attachment.name })}
                        type="button"
                      >
                        {attachment.name} ×
                      </button>
                    ))}
                  </div>
                ) : null}
                <textarea
                  aria-label={t("Edit message")}
                  autoFocus
                  className="min-h-24 w-full resize-y rounded-xl border border-stone-300 bg-white/90 px-3 py-2 text-sm text-stone-900 outline-none focus:border-teal-700"
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
                <div className="flex justify-end gap-1.5">
                  <button
                    aria-label={t("Cancel editing")}
                    className="message-action-menu"
                    disabled={isSavingEdit}
                    onClick={onCancelEdit}
                    title={t("Cancel editing")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                  <button
                    aria-label={t("Save and regenerate")}
                    className="message-action-menu"
                    disabled={isSavingEdit || !editingText.trim()}
                    onClick={() => onSaveEdit(message)}
                    title={t("Save and regenerate")}
                    type="button"
                  >
                    {isSavingEdit ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <Send aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>
              </div>
            ) : null}
            {!isEditing && !isUser ? <MemoriesUsedBlock memories={message.memoriesUsed} /> : null}
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
                      ? message.metrics?.totalLatencyMs ?? null
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
            {!isUser ? <SpecUpdatesBlock updates={message.specUpdates} /> : null}
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
      className={`context-usage-circle ${toneClass} ${isLoading ? "context-usage-circle-loading" : ""
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

type ComposerModelProviderGroup = {
  providerId: string;
  providerLabel: string;
  models: ComposerSelectOption[];
};

function ComposerModelProviderMenu({
  ariaLabel,
  className,
  disabled,
  emptyLabel,
  groups,
  onChange,
  selectedModelId,
  selectedProviderId,
}: {
  ariaLabel: string;
  className: string;
  disabled: boolean;
  emptyLabel: string;
  groups: ComposerModelProviderGroup[];
  onChange: (providerId: string, modelId: string) => void;
  selectedModelId: string;
  selectedProviderId: string;
}) {
  const selectedProvider =
    groups.find((group) => group.providerId === selectedProviderId) ?? null;
  const selectedModel =
    selectedProvider?.models.find((model) => model.value === selectedModelId) ??
    groups.flatMap((group) => group.models).find((model) => model.value === selectedModelId) ??
    null;
  const selectedLabel =
    selectedProvider && selectedModel
      ? `${selectedProvider.providerLabel} / ${selectedModel.label}`
      : selectedModel?.label ?? emptyLabel;
  const detailsRef = useCloseDetailsOnOutsidePointerDown();

  function handleSelect(
    providerId: string,
    modelId: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    detailsRef.current?.removeAttribute("open");
    onChange(providerId, modelId);
  }

  return (
    <details
      className={`composer-select-menu group relative ${className}`}
      ref={detailsRef}
    >
      <summary
        aria-disabled={disabled}
        aria-label={ariaLabel}
        className={`composer-select-summary flex h-[1.875rem] w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
          }`}
        title={selectedLabel}
      >
        <Server aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="composer-select-label min-w-0 flex-1 truncate">
          {selectedLabel}
        </span>
        <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
      </summary>
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-72 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-64 overflow-y-auto py-1">
          {groups.length ? (
            groups.map((group) => (
              <details
                className="composer-model-provider-group"
                key={group.providerId}
                open={group.providerId === selectedProviderId}
              >
                <summary
                  className={`composer-model-provider-summary flex min-h-9 w-full cursor-pointer list-none items-center gap-2 px-3 py-2 text-left text-sm font-semibold marker:hidden hover:bg-stone-50 ${group.providerId === selectedProviderId
                      ? "text-teal-900"
                      : "text-stone-700"
                    }`}
                  title={group.providerLabel}
                >
                  <Server aria-hidden="true" className="size-3.5 shrink-0" />
                  <span className="min-w-0 flex-1 truncate">
                    {group.providerLabel}
                  </span>
                  <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
                </summary>
                <div className="composer-model-provider-models border-l border-stone-100 py-1">
                  {group.models.map((model) => (
                    <button
                      aria-label={`${group.providerLabel}: ${model.label}`}
                      className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 pl-8 text-left text-sm hover:bg-stone-50 ${group.providerId === selectedProviderId && model.value === selectedModelId
                          ? "font-semibold text-teal-900"
                          : "text-stone-700"
                        }`}
                      key={model.value}
                      onClick={(event) =>
                        handleSelect(group.providerId, model.value, event)
                      }
                      type="button"
                    >
                      <Bot aria-hidden="true" className="size-3.5 shrink-0" />
                      <span className="min-w-0 flex-1 truncate">{model.label}</span>
                      {group.providerId === selectedProviderId &&
                        model.value === selectedModelId ? (
                        <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                      ) : null}
                    </button>
                  ))}
                </div>
              </details>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">{emptyLabel}</div>
          )}
        </div>
      </div>
    </details>
  );
}

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
        className={`composer-select-summary flex h-[1.875rem] w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
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
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${option.value === selectedValue
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
  currentWorktreeBranch,
  disabled,
  isGitRepository,
  isLoading,
  onChange,
  onOpen,
  worktrees,
}: {
  branches: string[];
  currentBranch: string;
  currentWorktreeBranch: string | null;
  disabled: boolean;
  isGitRepository: boolean;
  isLoading: boolean;
  onChange: (value: string) => void;
  onOpen: () => void;
  worktrees: GitWorktreeSummary[];
}) {
  const { t } = useI18n();
  const detailsRef = useCloseDetailsOnOutsidePointerDown();
  const displayedWorktrees = currentWorktreeBranch
    ? ensureCurrentWorktree(worktrees, currentWorktreeBranch)
    : worktrees;
  if (!isGitRepository) {
    return (
      <div
        aria-label={t("Git branch")}
        className="composer-branch-select inline-flex h-[1.875rem] max-w-full items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-400"
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
      className="composer-branch-select group relative max-w-full rounded-lg"
      onToggle={(event) => {
        if (event.currentTarget.open) {
          onOpen();
        }
      }}
      ref={detailsRef}
    >
      <summary
        aria-disabled={disabled}
        className={`composer-select-summary flex h-[1.875rem] w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${disabled ? "text-stone-500" : "text-stone-900 hover:border-stone-300"
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
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-72 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-64 overflow-y-auto py-1">
          <div className="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase text-stone-400">
            {t("Git branches")}
          </div>
          {branches.length ? (
            branches.map((branch) => (
              <button
                aria-label={t("Switch to branch {name}", { name: branch })}
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 disabled:cursor-not-allowed disabled:text-stone-400 disabled:hover:bg-transparent ${branch === currentBranch
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                  }`}
                disabled={disabled}
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
          <div className="mt-1 border-t border-stone-100 px-3 pb-1 pt-2 text-[11px] font-semibold uppercase text-stone-400">
            {t("Git worktrees")}
          </div>
          {displayedWorktrees.length ? (
            displayedWorktrees.map((worktree) => (
              <div
                className={`flex min-h-11 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm ${worktree.isCurrent
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                  }`}
                key={worktree.path}
                title={worktree.path}
              >
                <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{worktree.name}</span>
                  <span className="block truncate text-xs font-normal text-stone-500">
                    {worktree.branch ?? t("Detached HEAD")}
                  </span>
                </span>
                {worktree.isCurrent ? (
                  <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                ) : null}
              </div>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">
              {t("No worktrees")}
            </div>
          )}
        </div>
        <div className="border-t border-stone-100 bg-white p-1.5">
          <button
            aria-label={t("Create git branch")}
            className="flex h-9 w-full items-center gap-2 rounded-lg px-2 text-sm font-semibold text-teal-800 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-400 disabled:hover:bg-transparent"
            disabled={disabled}
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

function ensureCurrentWorktree(
  worktrees: GitWorktreeSummary[],
  branch: string,
): GitWorktreeSummary[] {
  const existingCurrent = worktrees.find((worktree) => worktree.isCurrent);
  if (existingCurrent) {
    return worktrees;
  }
  return [
    {
      branch,
      isCurrent: true,
      name: branch,
      path: "",
    },
    ...worktrees,
  ];
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
  const durationTitle = t("Thinking duration {duration}", { duration: durationLabel });

  useEffect(() => {
    setIsExpanded(isStreaming);
  }, [isStreaming]);

  const toggleLabel = isExpanded ? t("Collapse thinking") : t("Expand thinking");

  return (
    <div className="reasoning-block min-w-0 rounded-lg border border-stone-200 bg-stone-50/80 p-2 text-stone-600">
      <button
        aria-expanded={isExpanded}
        aria-label={toggleLabel}
        className="tool-call-summary flex w-full min-w-0 cursor-pointer items-center gap-1.5 text-left text-xs font-semibold text-stone-700 hover:text-stone-900"
        onClick={() => setIsExpanded((current) => !current)}
        title={toggleLabel}
        type="button"
      >
        {isExpanded ? (
          <ChevronDown aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        ) : (
          <ChevronRight aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        )}
        <span className="shrink-0 font-semibold">{t("Thinking")}</span>
        {isExpanded ? null : (
          <span
            className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-stone-500"
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
        <div className="mt-2 border-l border-stone-200 pl-3 text-stone-700">
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
          part.liveDurationMs ??
          part.durationMs ??
          reasoningDurationFallbackMs
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
    return <AttachmentPartBlock attachment={part.attachment} helpers={helpers} isUser={isUser} />;
  }

  if (part.type === "error") {
    return <ErrorMessagePart text={part.text} />;
  }

  return (
    <MarkdownContent
      content={part.text}
      isError={isError}
      isUser={isUser}
      renderMode={!isUser && isStreaming && isStreamingTail ? "streaming" : "full"}
      selectedSkillPrefix={helpers.selectedSkillPrefix}
    />
  );
}

export const MessagePartBlock = memo(MessagePartBlockComponent);

function ErrorMessagePart({ text }: { text: string }) {
  return (
    <div className="whitespace-pre-wrap break-words rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm leading-6 text-rose-700">
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
      className={`composer-attachment-chip ${attachment.previewDataUrl ? "composer-attachment-chip-image" : ""
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
      className={`message-attachment-part ${isUser ? "message-attachment-part-user" : ""
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
    <div className="flex items-center justify-between gap-2 border-t border-stone-100 pt-2 text-[11px] leading-4 text-stone-400">
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
        {values.map((value) => (
          <span className="min-w-0 break-words" key={value}>
            {value}
          </span>
        ))}
      </div>
      {onOpenApiRequests ? (
        <button
          aria-label={t("View API requests for this reply")}
          className="inline-flex size-7 shrink-0 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-500 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
          onClick={onOpenApiRequests}
          title={t("View API requests for this reply")}
          type="button"
        >
          <Server aria-hidden="true" className="size-3.5" />
        </button>
      ) : null}
    </div>
  );
}

function memoryMetaLabel(value: string, t: Translate) {
  return t(`memory.${value}`);
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
              <span>{memoryMetaLabel(memory.scope, t)}</span>
              <span>{memoryMetaLabel(memory.kind, t)}</span>
              <span>{memoryMetaLabel(memory.source, t)}</span>
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
              <span>{memoryMetaLabel(memory.scope, t)}</span>
              <span>{memoryMetaLabel(memory.kind, t)}</span>
              <span>{memoryMetaLabel(memory.status, t)}</span>
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

function SpecUpdatesBlock({ updates }: { updates: ChatSpecUpdateSummary[] }) {
  const { t } = useI18n();
  if (!updates.length) {
    return null;
  }

  const lineCount = updates.reduce((count, update) => count + update.lines.length, 0);

  return (
    <details className="rounded-lg border border-stone-100 bg-stone-50/70 px-3 py-2 text-xs text-stone-600">
      <summary className="flex cursor-pointer list-none items-center gap-2 font-semibold text-stone-600 marker:hidden">
        <FileText aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span>{t("Spec updated")}</span>
        <span className="rounded-full bg-white px-1.5 py-0.5 text-[10px] text-stone-500">
          {lineCount}
        </span>
        <ChevronDown aria-hidden="true" className="ml-auto size-3.5 shrink-0" />
      </summary>
      <div className="mt-2 space-y-2">
        {updates.map((update) => (
          <div
            className="min-w-0 overflow-hidden rounded-md border border-stone-100 bg-white"
            key={update.id}
          >
            <div className="flex min-w-0 flex-wrap items-center gap-1.5 border-b border-stone-100 px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-normal text-stone-400">
              <span>
                {t("Revision")} {update.baseRevision} -&gt; {update.revision}
              </span>
              {update.truncated ? <span>{t("Truncated")}</span> : null}
            </div>
            <div className="panel-scroll max-h-56 overflow-auto py-1 font-mono text-[11px] leading-5">
              {update.lines.map((line, index) => (
                <div
                  className={`whitespace-pre-wrap break-words px-2.5 ${line.kind === "added"
                    ? "bg-emerald-50 text-emerald-800"
                    : "bg-rose-50 text-rose-800"
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

type EditFileDiffLine = {
  kind: "added" | "removed";
  text: string;
};

type EditFileDiff = {
  lines: EditFileDiffLine[];
};

type ToolCallViewMode = "compact" | "raw";

const EMPTY_SELECTED_SKILL_PREFIX: SelectedSkillPrefixResolver = () => null;
const DIRECT_COMPACT_TEXT_FIELDS = ["content", "text", "message", "note", "error"];
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

function successfulEditFileDiff(
  toolCall: ChatToolCallSummary,
  input: JsonValue,
): EditFileDiff | null {
  if (toolCall.name !== "edit_file" || toolCall.isError || toolCall.status !== "completed") {
    return null;
  }
  if (!isJsonRecord(input)) {
    return null;
  }

  const oldStr = input.oldStr;
  const newStr = input.newStr;
  if (typeof oldStr !== "string" || typeof newStr !== "string") {
    return null;
  }

  // ponytail: this is replacement-snippet diff, not a full-file diff; upgrade when the backend returns real hunks/startLine.
  return {
    lines: [
      ...oldStr.split("\n").map((text) => ({
        kind: "removed" as const,
        text,
      })),
      ...newStr.split("\n").map((text) => ({
        kind: "added" as const,
        text,
      })),
    ],
  };
}

function isJsonRecord(value: JsonValue | null | undefined): value is { [key: string]: JsonValue } {
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
      .map((item) => compactArrayItemText(item, fieldName, toolName, compactJson))
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
  if ((fieldName === "snippets" || toolName === "graph_explore") && snippetContent) {
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
    return commandOutputText(output) ?? liveOutputText ?? (output === null ? compactJson(input) : compactJson(output));
  }

  if (typeof output === "string" && output.trim()) {
    return output;
  }

  if (isJsonRecord(output)) {
    const directText = compactRecordText(output);
    if (directText) {
      return directText;
    }
    const arraySummary = compactArraySummary(output, toolCall.name, compactJson);
    if (arraySummary) {
      return arraySummary;
    }
  }

  return output === null ? compactJson(input) : compactJson(output);
}

function EditFileDiffBlock({ diff }: { diff: EditFileDiff }) {
  return (
    <div className="min-w-0">
      <div className="mb-1 font-semibold text-stone-500">Diff</div>
      <div className="panel-scroll max-h-64 overflow-auto rounded-md border border-stone-200 font-mono text-[11px] leading-5">
        {diff.lines.map((line, index) => (
          <div
            className={`edit-file-diff-line grid grid-cols-[1.5rem_minmax(0,1fr)] whitespace-pre-wrap break-words px-2 ${line.kind === "added"
              ? "bg-emerald-50 text-emerald-800"
              : "bg-rose-50 text-rose-800"
              }`}
            key={`${line.kind}-${index}`}
          >
            <span className="select-none font-semibold">{line.kind === "added" ? "+" : "-"}</span>
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
        <div className="mb-1 font-semibold text-stone-500">{t("Input")}</div>
        <pre className="panel-scroll max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-stone-200 pl-3 font-mono text-[11px] leading-5">
          {formatJsonValue(input)}
        </pre>
      </div>
      {toolCall.output !== null ? (
        <div className="min-w-0">
          <div className="mb-1 font-semibold text-stone-500">{t("Output")}</div>
          <pre
            className={`panel-scroll max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${toolCall.isError
              ? "border-rose-200 text-rose-700"
              : "border-stone-200"
              }`}
          >
            {formatJsonValue(toolCall.output)}
          </pre>
        </div>
      ) : liveOutputText ? (
        <div className="min-w-0">
          <div className="mb-1 font-semibold text-stone-500">
            {t("Live output")}
          </div>
          <pre className="panel-scroll max-h-64 overflow-auto whitespace-pre-wrap break-words border-l border-stone-200 pl-3 font-mono text-[11px] leading-5 text-stone-700">
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
}: {
  compactJson: (value: JsonValue) => string;
  diff: EditFileDiff | null;
  input: JsonValue;
  liveOutputText: string | null;
  toolCall: ChatToolCallSummary;
}) {
  if (diff) {
    return <EditFileDiffBlock diff={diff} />;
  }

  const specMarkdown = successfulSpecMarkdown(toolCall);
  if (specMarkdown !== null) {
    return (
      <div className="panel-scroll max-h-64 overflow-auto border-l border-stone-200 pl-3">
        <MarkdownContent
          content={specMarkdown}
          isUser={false}
          selectedSkillPrefix={EMPTY_SELECTED_SKILL_PREFIX}
        />
      </div>
    );
  }

  const text = compactToolCallText(toolCall, input, liveOutputText, compactJson);
  return (
    <pre
      className={`panel-scroll max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${toolCall.isError
        ? "border-rose-200 text-rose-700"
        : "border-stone-200 text-stone-700"
        }`}
    >{text}</pre>
  );
}

function contextCompressionKindLabel(kind: "rule" | "llm" | "runtimeToolState", t: Translate) {
  if (kind === "llm") {
    return t("LLM");
  }
  if (kind === "runtimeToolState") {
    return t("Runtime tool state");
  }
  return t("Rule");
}

function contextCompressionStatusLabel(status: string, t: Translate) {
  if (status === "start") {
    return t("Compressing");
  }
  if (status === "completed") {
    return t("Compressed");
  }
  return status || "-";
}

function formatContextCompressionTokenDelta(
  originalTokenCount: number | null | undefined,
  summaryTokenCount: number | null | undefined,
  t: Translate,
) {
  if (typeof originalTokenCount !== "number" || typeof summaryTokenCount !== "number") {
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
  const statusLabel = contextCompressionStatusLabel(compression.status, t);
  const originalTokenCount = detail.originalTokenCount ?? null;
  const summaryTokenCount = detail.summaryTokenCount ?? null;
  const savedLabel = formatContextCompressionTokenDelta(
    originalTokenCount,
    summaryTokenCount,
    t,
  );
  const modelLabel = [detail.providerId, detail.modelId].filter(Boolean).join(" / ") || "-";
  const fields = [
    [t("Original tokens"), originalTokenCount?.toLocaleString() ?? "-"],
    [t("Compressed tokens"), summaryTokenCount?.toLocaleString() ?? "-"],
    [t("Started"), detail.startedAt ? formatChatCreatedAt(detail.startedAt) : "-"],
    [t("Ended"), detail.completedAt ? formatChatCreatedAt(detail.completedAt) : "-"],
    [t("Provider"), detail.providerId || "-"],
    [t("Model"), detail.modelId || "-"],
    ["snapshotId", detail.snapshotId ?? "-"],
    ["kind", compression.kind],
  ];

  return (
    <details className="tool-call-block group min-w-0">
      <summary className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-stone-700 marker:hidden">
        <Shrink aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="min-w-0 shrink-0 truncate">{t("Context compression")}</span>
        <span className="shrink-0 text-stone-300">·</span>
        <span className="shrink-0 text-stone-500">{kindLabel}</span>
        <span
          className="min-w-0 flex-1 truncate font-mono text-[11px] font-medium text-stone-500"
          title={`${savedLabel} · ${modelLabel}`}
        >
          {savedLabel} · {modelLabel}
        </span>
        <span
          className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-4 ${compression.status === "completed"
              ? "bg-emerald-50 text-emerald-700"
              : "bg-stone-100 text-stone-600"
            }`}
        >
          {statusLabel}
        </span>
      </summary>
      <div className="mt-2 grid gap-1.5 border-l border-stone-200 pl-3 text-[11px] text-stone-600">
        {fields.map(([label, value]) => (
          <div className="flex min-w-0 gap-2" key={label}>
            <span className="w-32 shrink-0 font-semibold text-stone-500">{label}</span>
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
  const input = normalizedToolInput(toolCall.input);
  const editFileDiff = successfulEditFileDiff(toolCall, input);
  const detailText = toolCallDetailText(toolCall);
  const changeStats = toolCallChangeStats(toolCall);
  const liveOutputText = toolLiveOutputText(toolCall.liveOutput);
  const generatedImages = toolCall.isError
    ? []
    : generatedImageFiles(toolCall.name, toolCall.output);
  const toggleLabel = viewMode === "compact" ? t("Raw") : t("Compact");
  const displayName = toolDisplayName(toolCall.name, language);
  const ToolIcon = TOOL_CALL_ICONS[toolCall.name] ?? Wrench;

  return (
    <div className="grid min-w-0 gap-2">
      <GeneratedImageFilesBlock
        files={generatedImages}
        formatFileSize={helpers.formatFileSize}
        workspaceId={workspaceId}
      />
      <details className="tool-call-block group min-w-0">
        <summary
          aria-label={`${displayName} (${toolCall.name})`}
          className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-stone-700 marker:hidden"
          title={toolCall.name}
        >
          <ToolIcon aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
          <span className="min-w-0 shrink-0 truncate">{displayName}</span>
          {changeStats ? (
            <span className="shrink-0 rounded bg-stone-100 px-1.5 py-0.5 font-mono text-[10px] leading-4 text-stone-600">
              <span className="text-emerald-700">+{changeStats.linesAdded}</span>{" "}
              <span className="text-rose-700">-{changeStats.linesRemoved}</span>
            </span>
          ) : null}
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
            className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-4 ${toolCall.isError
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
          <div className="flex min-w-0 items-start justify-between gap-2 text-[11px] text-stone-500">
            <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
              <span>
                <span className="font-semibold text-stone-500">{t("Started")}</span>{" "}
                <span>{toolCall.startedAt ? formatChatCreatedAt(toolCall.startedAt) : "-"}</span>
              </span>
              <span>
                <span className="font-semibold text-stone-500">{t("Ended")}</span>{" "}
                <span>{toolCall.completedAt ? formatChatCreatedAt(toolCall.completedAt) : "-"}</span>
              </span>
            </div>
            <button
              aria-label={toggleLabel}
              className="shrink-0 rounded border border-stone-200 bg-white px-2 py-0.5 text-[11px] font-semibold text-stone-600 hover:border-stone-300 hover:bg-stone-50 focus:outline-none focus:ring-2 focus:ring-teal-200"
              onClick={() => setViewMode(viewMode === "compact" ? "raw" : "compact")}
              type="button"
            >
              {toggleLabel}
            </button>
          </div>
          {viewMode === "compact" ? (
            <CompactToolCallView
              compactJson={compactToolJson}
              diff={editFileDiff}
              input={input}
              liveOutputText={liveOutputText}
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
