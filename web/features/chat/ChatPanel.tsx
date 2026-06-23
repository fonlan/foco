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
  LoaderCircle,
  Plus,
  RefreshCw,
  Send,
  Server,
  SlidersHorizontal,
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
  useEffect,
  useLayoutEffect,
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
  ChatToolCallSummary,
  ChatToolLiveOutput,
  ComposerAttachment,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  ContextUsageResponse,
  GitBranchesResponse,
  JsonValue,
  SettingsResponse,
  ShellMessage,
  ThinkingLevelSummary,
  Translate,
  WorkspaceSummary,
} from "../../api/types";
import { CHAT_BOTTOM_LOCK_THRESHOLD_PX, CREATE_BRANCH_OPTION_VALUE } from "../../app/constants";
import { useI18n } from "../../shared/i18n";
import { MarkdownContent, type SelectedSkillPrefixResolver } from "./MarkdownContent";

const COMPOSER_EDITOR_MIN_HEIGHT_PX = 68;
const COMPOSER_EDITOR_KEY_STEP_PX = 24;
const COMPOSER_EDITOR_MAX_HEIGHT_RATIO = 0.55;

type ToolCallChangeStats = {
  linesAdded: number;
  linesRemoved: number;
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

export function ChatPanel({
  activeWorkspaceName,
  availableModels,
  branchError,
  chatScrollKey,
  canGuideActiveRun,
  canRetryRun,
  canUseNativePicker,
  contextUsage,
  draftAttachments,
  draftMessage,
  gitBranches,
  helpers,
  queuedRunCount,
  readOnly,
  isLoadingBranches,
  isLoadingContextUsage,
  isLoadingMessages,
  isLoadingSettings,
  isSendingMessage,
  isSelectingAttachments,
  isTeamModeEnabled,
  messages,
  onAddPastedImageAttachments,
  overviewRenderer,
  onBranchChange,
  onCancelRun,
  onDraftMessageChange,
  onGuideActiveRun,
  onGuideQueuedMessage,
  onModelChange,
  onProviderChange,
  onQueueActiveRun,
  onRemoveAttachment,
  onRemoveSkill,
  onRetryRun,
  onSelectAttachments,
  onSubmit,
  onTeamModeEnabledChange,
  onThinkingLevelChange,
  onToggleSkill,
  onWithdrawQueuedMessage,
  selectedGitBranch,
  selectedModelId,
  selectedProviderId,
  selectedSkillIds,
  selectedThinkingLevel,
  settings,
  showTeamModeToggle,
  providers,
  skills,
  queuedMessageIds,
  thinkingLevels,
  workspaces,
}: {
  activeWorkspaceName: string | null;
  availableModels: ConfiguredModelSummary[];
  branchError: string | null;
  chatScrollKey: string;
  canGuideActiveRun: boolean;
  canRetryRun: boolean;
  canUseNativePicker: boolean;
  contextUsage: ContextUsageResponse | null;
  draftAttachments: ComposerAttachment[];
  draftMessage: string;
  gitBranches: GitBranchesResponse | null;
  helpers: ChatPanelHelpers;
  queuedRunCount: number;
  readOnly: boolean;
  isLoadingBranches: boolean;
  isLoadingContextUsage: boolean;
  isLoadingMessages: boolean;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  isSelectingAttachments: boolean;
  isTeamModeEnabled: boolean;
  messages: ShellMessage[];
  onAddPastedImageAttachments: (files: File[]) => void;
  overviewRenderer: () => ReactNode;
  onBranchChange: (value: string) => void;
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onGuideActiveRun: () => void;
  onGuideQueuedMessage: (messageId: string) => void;
  onModelChange: (value: string) => void;
  onProviderChange: (value: string) => void;
  onQueueActiveRun: () => void;
  onRemoveAttachment: (attachmentId: string) => void;
  onRemoveSkill: (skillId: string) => void;
  onRetryRun: () => void;
  onSelectAttachments: (files: File[]) => void;
  onSubmit: (
    event: FormEvent<HTMLFormElement>,
    options?: { schedule?: boolean },
  ) => void;
  onTeamModeEnabledChange: (value: boolean) => void;
  onThinkingLevelChange: (value: string) => void;
  onToggleSkill: (skillId: string) => void;
  onWithdrawQueuedMessage: (messageId: string) => void;
  selectedGitBranch: string;
  selectedModelId: string;
  selectedProviderId: string;
  selectedSkillIds: string[];
  selectedThinkingLevel: string;
  settings: SettingsResponse | null;
  showTeamModeToggle: boolean;
  providers: ConfiguredProviderSummary[];
  skills: ConfiguredSkillSummary[];
  queuedMessageIds: ReadonlySet<string>;
  thinkingLevels: ThinkingLevelSummary[];
  workspaces: WorkspaceSummary[];
}) {
  const {
    activeSkillQuery,
    compactInlineText,
    compactToolJson,
    fallbackMessageParts,
    formatChatCreatedAt,
    formatFileSize,
    formatJsonValue,
    formatNullableLatencySeconds,
    formatTokensPerSecond,
    messageCopyText,
    normalizedToolInput,
    removeActiveSkillToken,
    selectedSkillPrefix,
    skillScopeLabel,
    toolCallChangeStats,
    toolCallDetailText,
    toolLiveOutputText,
    toolStatusText,
  } = helpers;
  const { t } = useI18n();
  const chatPanelRef = useRef<HTMLDivElement>(null);
  const messageScrollRef = useRef<HTMLDivElement>(null);
  const messageScrollContentRef = useRef<HTMLDivElement>(null);
  const messageScrollEndRef = useRef<HTMLDivElement>(null);
  const messageTextareaRef = useRef<HTMLTextAreaElement>(null);
  const composerResizeDragRef = useRef<ComposerResizeDrag | null>(null);
  const copiedMessageTimerRef = useRef<number | null>(null);
  const shouldLockMessageScrollRef = useRef(true);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const [isCtrlKeyPressed, setIsCtrlKeyPressed] = useState(false);
  const [isResizingComposer, setIsResizingComposer] = useState(false);
  const [isSendButtonTooltipOpen, setIsSendButtonTooltipOpen] = useState(false);
  const [composerEditorHeight, setComposerEditorHeight] = useState(
    COMPOSER_EDITOR_MIN_HEIGHT_PX,
  );
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = new Set(selectedSkillIds);
  const selectedSkills = selectedSkillIds
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill));
  const workspaceName = activeWorkspaceName?.trim();
  const composerPlaceholder = workspaceName
    ? t("Ask Foco anything about {name}...", { name: workspaceName })
    : t("Ask Foco anything...");
  const providersById = new Map(providers.map((provider) => [provider.id, provider]));
  const providerIdsForAvailableModels = Array.from(
    new Set(availableModels.flatMap((model) => model.providerIds)),
  );
  const modelProviderGroups = [
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
  const fileInputRef = useRef<HTMLInputElement | null>(null);
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
  const sendButtonTitle = isCtrlKeyPressed ? t("Send to queue") : t("Send");
  const showSendButtonTooltip = isSendButtonTooltipOpen && !isSendingMessage;

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
        onScroll={handleMessageScroll}
        ref={messageScrollRef}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${messages.length ? "max-w-5xl gap-4" : "max-w-6xl"
            }`}
          ref={messageScrollContentRef}
        >
          {messages.length ? (
            messages.map((message) => {
              const isUser = message.role === "user";
              const parts = message.parts.length
                ? message.parts
                : fallbackMessageParts(message);
              const reasoningPartCount = parts.filter(
                (part) => part.type === "reasoning",
              ).length;
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
              const canManageQueuedMessage =
                isUser &&
                message.pendingMode === "queued" &&
                queuedMessageIds.has(message.id);

              return (
                <div
                  className={`message-row flex ${isUser ? "message-row-user" : "message-row-agent"}`}
                  key={message.id}
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
                          </span>
                          <span className="message-action-group">
                            {canManageQueuedMessage ? (
                              <>
                                <button
                                  aria-label={t(
                                    "Convert queued message to guidance",
                                  )}
                                  className="message-action-menu"
                                  onClick={() => onGuideQueuedMessage(message.id)}
                                  title={t(
                                    "Convert queued message to guidance",
                                  )}
                                  type="button"
                                >
                                  <ArrowUp
                                    aria-hidden="true"
                                    className="size-3.5"
                                  />
                                </button>
                                <button
                                  aria-label={t("Withdraw queued message")}
                                  className="message-action-menu"
                                  onClick={() => onWithdrawQueuedMessage(message.id)}
                                  title={t("Withdraw queued message")}
                                  type="button"
                                >
                                  <X
                                    aria-hidden="true"
                                    className="size-3.5"
                                  />
                                </button>
                              </>
                            ) : null}
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
                                <Copy
                                  aria-hidden="true"
                                  className="size-3.5"
                                />
                              )}
                            </button>
                          </span>
                        </div>
                        {!isUser ? (
                          <MemoriesUsedBlock memories={message.memoriesUsed} />
                        ) : null}
                        {parts.length ? (
                          parts.map((part, partIndex) => {
                            return (
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
                              />
                            );
                          })
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
                          <ChatReplyMetricsLine helpers={helpers} metrics={message.metrics} />
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })
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
        <div aria-hidden="true" className="h-px" ref={messageScrollEndRef} />
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
              <input
                ref={fileInputRef}
                aria-hidden="true"
                className="sr-only"
                multiple
                tabIndex={-1}
                type="file"
                onChange={(event) => {
                  const files = Array.from(event.currentTarget.files ?? []);
                  event.currentTarget.value = "";
                  onSelectAttachments(files);
                }}
              />
              <button
                aria-label={t("Add attachment")}
                className="composer-tool-button"
                disabled={isSelectingAttachments}
                onClick={() => fileInputRef.current?.click()}
                title={t("Add attachment")}
                type="button"
              >

                {isSelectingAttachments ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <Plus aria-hidden="true" className="size-4" />
                )}
              </button>
              {showTeamModeToggle ? (
                <button
                  aria-label={t("Team mode")}
                  aria-pressed={isTeamModeEnabled}
                  className={`composer-team-toggle ${isTeamModeEnabled
                    ? "composer-team-toggle-enabled"
                    : ""
                    }`}
                  onClick={() => onTeamModeEnabledChange(!isTeamModeEnabled)}
                  title={t("Team mode")}
                  type="button"
                >
                  <Bot aria-hidden="true" className="size-3.5 shrink-0" />
                  <span className="composer-team-toggle-label">{t("Team")}</span>
                </button>
              ) : null}
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
                    className="composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                    disabled={
                      (!draftMessage.trim() && !draftAttachments.length) ||
                      !selectedModelId
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
      ref={detailsRef}
    >
      <summary
        className={`composer-select-summary flex h-[1.875rem] w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
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
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${branch === currentBranch
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
          <MarkdownContent
            content={reasoning}
            isUser={false}
            selectedSkillPrefix={helpers.selectedSkillPrefix}
            variant="reasoning"
          />
        </div>
      ) : null}
    </div>
  );
}

function MessagePartBlock({
  helpers,
  isError,
  isStreaming,
  isStreamingTail,
  isUser,
  part,
  reasoningDurationFallbackMs,
}: {
  helpers: ChatPanelHelpers;
  isError: boolean;
  isStreaming: boolean;
  isStreamingTail: boolean;
  isUser: boolean;
  part: ChatMessagePart;
  reasoningDurationFallbackMs: number | null;
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
    return <ToolCallBlock helpers={helpers} toolCall={part.toolCall} />;
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
      selectedSkillPrefix={helpers.selectedSkillPrefix}
    />
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
}: {
  helpers: ChatPanelHelpers;
  metrics: ChatReplyMetrics;
}) {
  const { formatNullableLatencySeconds, formatTokensPerSecond } = helpers;
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

function ToolCallBlock({
  helpers,
  toolCall,
}: {
  helpers: ChatPanelHelpers;
  toolCall: ChatToolCallSummary;
}) {
  const {
    formatJsonValue,
    normalizedToolInput,
    toolCallChangeStats,
    toolCallDetailText,
    toolLiveOutputText,
    toolStatusText,
  } = helpers;
  const { t } = useI18n();
  const input = normalizedToolInput(toolCall.input);
  const detailText = toolCallDetailText(toolCall);
  const changeStats = toolCallChangeStats(toolCall);
  const liveOutputText = toolLiveOutputText(toolCall.liveOutput);

  return (
    <details className="tool-call-block group min-w-0">
      <summary className="tool-call-summary flex cursor-pointer list-none items-center gap-1.5 text-xs font-semibold text-stone-700 marker:hidden">
        <Wrench aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="min-w-0 shrink-0 truncate">{toolCall.name}</span>
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
      </div>
    </details>
  );
}
