import { ArrowLeft, Bot, ListChecks, LoaderCircle, RefreshCw, User } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { CHAT_BOTTOM_LOCK_THRESHOLD_PX } from "../../app/constants";
import type {
  AgentTranscriptItemView,
  AgentTranscriptResponse,
  AgentTeamSnapshotResponse,
  ChatMessagePart,
} from "../../api/types";
import {
  MessagePartBlock,
  type ChatPanelHelpers,
} from "../chat/ChatPanel";
import { MarkdownContent } from "../chat/MarkdownContent";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";
import { Button } from "../../shared/ui";

type AgentTranscriptItem = AgentTranscriptItemView;
type AgentTranscriptWirePart = ChatMessagePart | {
  type?: string;
  text?: string;
  durationMs?: number;
  duration_ms?: number;
  liveDurationMs?: number;
  live_duration_ms?: number;
  startedAtMs?: number;
  started_at_ms?: number;
  tool_call?: Extract<ChatMessagePart, { type: "toolCall" }>["toolCall"];
  toolCall?: Extract<ChatMessagePart, { type: "toolCall" }>["toolCall"];
};

const AGENT_TRANSCRIPT_PAGE_SIZE = 25;
const noSelectedSkillPrefix = () => null;
type AgentTranscriptLoadMode = "append" | "hard" | "refresh" | "soft";

export type AgentTranscriptViewCacheEntry = {
  hasBaseline: boolean;
  hasMore: boolean;
  items: AgentTranscriptItemView[];
  page: number;
  scrollTop: number;
  stickToBottom: boolean;
};

function readOptionalFiniteNumber(...candidates: unknown[]): number | undefined {
  for (const value of candidates) {
    // Reject NaN/Infinity and negative durations so render falls back safely.
    if (typeof value === "number" && Number.isFinite(value) && value >= 0) {
      return value;
    }
  }
  return undefined;
}

function normalizeAgentTranscriptPart(
  part: AgentTranscriptWirePart,
  itemId: string,
  partIndex: number,
): ChatMessagePart {
  if (part.type === "reasoning") {
    const wire = part as {
      type: "reasoning";
      text?: string;
      durationMs?: number;
      duration_ms?: number;
      liveDurationMs?: number;
      live_duration_ms?: number;
      startedAtMs?: number;
      started_at_ms?: number;
    };
    const text = typeof wire.text === "string" ? wire.text : "";
    // Rust ChatMessagePart::Reasoning serializes duration as duration_ms; accept camelCase too.
    const durationMs = readOptionalFiniteNumber(wire.durationMs, wire.duration_ms);
    const liveDurationMs = readOptionalFiniteNumber(
      wire.liveDurationMs,
      wire.live_duration_ms,
    );
    const startedAtMs = readOptionalFiniteNumber(wire.startedAtMs, wire.started_at_ms);
    return {
      type: "reasoning",
      text,
      ...(durationMs !== undefined ? { durationMs } : {}),
      ...(liveDurationMs !== undefined ? { liveDurationMs } : {}),
      ...(startedAtMs !== undefined ? { startedAtMs } : {}),
    };
  }

  if (part.type !== "toolCall") {
    return part as ChatMessagePart;
  }
  if (part.toolCall) {
    return part as ChatMessagePart;
  }
  const legacyToolCall = (part as {
    tool_call?: Extract<ChatMessagePart, { type: "toolCall" }>["toolCall"];
  }).tool_call;
  if (legacyToolCall) {
    return { ...part, toolCall: legacyToolCall, type: "toolCall" } as ChatMessagePart;
  }

  // ponytail: transcript-only guard; extract a shared schema normalizer if more APIs need runtime validation.
  console.warn("Malformed agent transcript toolCall part", { itemId, partIndex });
  return { type: "error", text: "Malformed tool call omitted." };
}

export function AgentTranscriptPanel({
  chatId,
  error,
  helpers,
  instanceId,
  isLoading,
  onOpenMainChat,
  onRefresh,
  readTranscriptCache,
  snapshot,
  writeTranscriptCache,
  workspaceId,
}: {
  chatId: string;
  error: string | null;
  helpers: ChatPanelHelpers;
  instanceId: string;
  isLoading: boolean;
  onOpenMainChat: () => void;
  onRefresh: () => Promise<void>;
  readTranscriptCache: () => AgentTranscriptViewCacheEntry | null;
  snapshot: AgentTeamSnapshotResponse | null;
  writeTranscriptCache: (entry: AgentTranscriptViewCacheEntry) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const initialCacheRef = useRef<AgentTranscriptViewCacheEntry | null | undefined>(undefined);
  if (initialCacheRef.current === undefined) {
    initialCacheRef.current = readTranscriptCache();
  }
  const initialCache = initialCacheRef.current;

  const readTranscriptCacheRef = useRef(readTranscriptCache);
  const writeTranscriptCacheRef = useRef(writeTranscriptCache);
  readTranscriptCacheRef.current = readTranscriptCache;
  writeTranscriptCacheRef.current = writeTranscriptCache;

  const instance =
    snapshot?.instances.find((current) => current.id === instanceId) ?? null;
  const [items, setItems] = useState<AgentTranscriptItem[]>(
    () => initialCache?.items ?? [],
  );
  const [page, setPage] = useState(() => initialCache?.page ?? 1);
  const [hasMore, setHasMore] = useState(() => initialCache?.hasMore ?? false);
  const [transcriptError, setTranscriptError] = useState<string | null>(null);
  const [isTranscriptLoading, setIsTranscriptLoading] = useState(false);
  const activeTranscriptRequestRef = useRef<AbortController | null>(null);
  const transcriptRequestGenerationRef = useRef(0);
  const hasTranscriptBaselineRef = useRef(initialCache?.hasBaseline ?? false);
  const snapshotRef = useRef(snapshot);
  const lastHandledSnapshotRef = useRef<AgentTeamSnapshotResponse | null>(null);
  const itemsRef = useRef(items);
  const pageRef = useRef(page);
  const hasMoreRef = useRef(hasMore);
  const identityKey = `${workspaceId}:${chatId}:${instanceId}`;
  const messageScrollRef = useRef<HTMLDivElement | null>(null);
  const messageScrollContentRef = useRef<HTMLDivElement | null>(null);
  const scrollTopRef = useRef(initialCache?.scrollTop ?? 0);
  const shouldLockToBottomRef = useRef(initialCache?.stickToBottom ?? true);
  const userScrollIntentRef = useRef(false);
  const previousIdentityKeyRef = useRef(identityKey);
  const previousItemCountRef = useRef(items.length);
  const pendingScrollRestoreRef = useRef<"bottom" | "top" | null>(
    initialCache?.hasBaseline
      ? initialCache.stickToBottom
        ? "bottom"
        : "top"
      : null,
  );
  const identityKeyRef = useRef(identityKey);
  const snapshotMatchesChat = snapshot?.team.chatId === chatId;
  const instanceExists = instance !== null && snapshotMatchesChat;

  snapshotRef.current = snapshot;
  itemsRef.current = items;
  pageRef.current = page;
  hasMoreRef.current = hasMore;

  const cancelActiveTranscriptRequest = useCallback(() => {
    activeTranscriptRequestRef.current?.abort();
    activeTranscriptRequestRef.current = null;
    transcriptRequestGenerationRef.current += 1;
  }, []);

  const scrollMessageListToBottom = useCallback(() => {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }
    element.scrollTop = element.scrollHeight;
    window.requestAnimationFrame(() => {
      if (shouldLockToBottomRef.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
  }, []);

  const persistTranscriptCache = useCallback(() => {
    if (!hasTranscriptBaselineRef.current && itemsRef.current.length === 0) {
      return;
    }
    writeTranscriptCacheRef.current({
      hasBaseline: hasTranscriptBaselineRef.current,
      hasMore: hasMoreRef.current,
      items: itemsRef.current,
      page: pageRef.current,
      scrollTop: messageScrollRef.current?.scrollTop ?? scrollTopRef.current,
      stickToBottom: shouldLockToBottomRef.current,
    });
  }, []);

  const loadTranscript = useCallback(
    async (nextPage: number, mode: AgentTranscriptLoadMode) => {
      activeTranscriptRequestRef.current?.abort();
      const controller = new AbortController();
      const requestGeneration = transcriptRequestGenerationRef.current + 1;
      transcriptRequestGenerationRef.current = requestGeneration;
      activeTranscriptRequestRef.current = controller;
      const isSoft = mode === "soft";

      setIsTranscriptLoading(!isSoft);
      if (!isSoft) {
        setTranscriptError(null);
      }
      try {
        const data = await requestJson<AgentTranscriptResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/agent-team/instances/${encodeURIComponent(instanceId)}/transcript?page=${nextPage}&pageSize=${AGENT_TRANSCRIPT_PAGE_SIZE}`,
          { signal: controller.signal },
        );
        if (transcriptRequestGenerationRef.current !== requestGeneration) {
          return;
        }
        const nextItems =
          mode === "append" ? [...itemsRef.current, ...data.items] : data.items;
        setItems(nextItems);
        setPage(data.page);
        setHasMore(data.hasMore);
        hasTranscriptBaselineRef.current = true;
        setTranscriptError(null);
        writeTranscriptCacheRef.current({
          hasBaseline: true,
          hasMore: data.hasMore,
          items: nextItems,
          page: data.page,
          scrollTop: messageScrollRef.current?.scrollTop ?? scrollTopRef.current,
          stickToBottom: shouldLockToBottomRef.current,
        });
      } catch (err) {
        if (
          controller.signal.aborted ||
          transcriptRequestGenerationRef.current !== requestGeneration
        ) {
          return;
        }
        if (!isSoft) {
          setTranscriptError(errorMessage(err));
        }
      } finally {
        if (transcriptRequestGenerationRef.current === requestGeneration) {
          activeTranscriptRequestRef.current = null;
          setIsTranscriptLoading(false);
        }
      }
    },
    [instanceId, workspaceId],
  );

  useEffect(() => {
    const isIdentityChange = identityKeyRef.current !== identityKey;
    identityKeyRef.current = identityKey;

    cancelActiveTranscriptRequest();
    setIsTranscriptLoading(false);

    // Only apply cache scroll/items restore on true identity changes.
    // First mount already seeds state + pendingScrollRestoreRef from lazy init;
    // re-queuing pending here would clobber live unlock after soft refresh.
    if (isIdentityChange) {
      const cache = readTranscriptCacheRef.current();
      if (cache?.hasBaseline) {
        setItems(cache.items);
        setPage(cache.page);
        setHasMore(cache.hasMore);
        setTranscriptError(null);
        hasTranscriptBaselineRef.current = true;
        scrollTopRef.current = cache.scrollTop;
        shouldLockToBottomRef.current = cache.stickToBottom;
        pendingScrollRestoreRef.current = cache.stickToBottom ? "bottom" : "top";
      } else {
        setItems([]);
        setPage(1);
        setHasMore(false);
        setTranscriptError(null);
        hasTranscriptBaselineRef.current = false;
        scrollTopRef.current = 0;
        shouldLockToBottomRef.current = true;
        pendingScrollRestoreRef.current = null;
      }
    }

    const currentSnapshot = snapshotRef.current;
    const exists =
      currentSnapshot?.team.chatId === chatId &&
      currentSnapshot.instances.some((current) => current.id === instanceId);
    lastHandledSnapshotRef.current = currentSnapshot;
    if (exists) {
      void loadTranscript(1, hasTranscriptBaselineRef.current ? "soft" : "hard");
    }

    return () => {
      persistTranscriptCache();
      cancelActiveTranscriptRequest();
    };
  }, [
    cancelActiveTranscriptRequest,
    chatId,
    identityKey,
    instanceId,
    loadTranscript,
    persistTranscriptCache,
    workspaceId,
  ]);

  useEffect(() => {
    if (lastHandledSnapshotRef.current === snapshot) {
      return;
    }

    lastHandledSnapshotRef.current = snapshot;

    // Temporary null or snapshot for another chat must not hard-reset the view.
    if (!snapshot || snapshot.team.chatId !== chatId) {
      return;
    }

    const exists = snapshot.instances.some((current) => current.id === instanceId);
    if (!exists) {
      cancelActiveTranscriptRequest();
      setIsTranscriptLoading(false);
      setItems([]);
      setPage(1);
      setHasMore(false);
      setTranscriptError(null);
      hasTranscriptBaselineRef.current = false;
      scrollTopRef.current = 0;
      shouldLockToBottomRef.current = true;
      pendingScrollRestoreRef.current = null;
      writeTranscriptCacheRef.current({
        hasBaseline: false,
        hasMore: false,
        items: [],
        page: 1,
        scrollTop: 0,
        stickToBottom: true,
      });
      return;
    }

    void loadTranscript(1, hasTranscriptBaselineRef.current ? "soft" : "hard");
  }, [
    cancelActiveTranscriptRequest,
    chatId,
    instanceId,
    loadTranscript,
    snapshot,
  ]);

  useLayoutEffect(() => {
    const element = messageScrollRef.current;
    const identityChanged = previousIdentityKeyRef.current !== identityKey;
    const wasEmpty = previousItemCountRef.current === 0;
    previousIdentityKeyRef.current = identityKey;
    previousItemCountRef.current = items.length;

    const pendingRestore = pendingScrollRestoreRef.current;
    if (pendingRestore) {
      // Keep pending until the scroll container is mounted so layout can retry.
      if (!element) {
        return;
      }
      pendingScrollRestoreRef.current = null;
      if (pendingRestore === "bottom" || items.length === 0) {
        shouldLockToBottomRef.current = items.length > 0;
        if (items.length > 0) {
          scrollMessageListToBottom();
        } else {
          element.scrollTop = 0;
        }
        return;
      }
      shouldLockToBottomRef.current = false;
      element.scrollTop = scrollTopRef.current;
      return;
    }

    if (items.length === 0) {
      shouldLockToBottomRef.current = false;
      if (element) {
        element.scrollTop = 0;
      }
      return;
    }

    if (identityChanged || wasEmpty) {
      shouldLockToBottomRef.current = true;
      scrollMessageListToBottom();
    }
  }, [identityKey, items.length, scrollMessageListToBottom]);

  useLayoutEffect(() => {
    if (!shouldLockToBottomRef.current) {
      return;
    }
    scrollMessageListToBottom();
  }, [items, scrollMessageListToBottom]);

  useLayoutEffect(() => {
    const container = messageScrollRef.current;
    const content = messageScrollContentRef.current;
    if (!container || !content) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldLockToBottomRef.current) {
        scrollMessageListToBottom();
      }
    });
    observer.observe(container);
    observer.observe(content);

    return () => observer.disconnect();
  }, [scrollMessageListToBottom]);

  const refreshTranscript = useCallback(async () => {
    await onRefresh();
    if (instanceExists) {
      await loadTranscript(1, "refresh");
    }
  }, [instanceExists, loadTranscript, onRefresh]);

  const loadMoreTranscript = useCallback(async () => {
    if (!isTranscriptLoading && hasMore) {
      await loadTranscript(page + 1, "append");
    }
  }, [hasMore, isTranscriptLoading, loadTranscript, page]);

  const markUserScrollIntent = useCallback(() => {
    userScrollIntentRef.current = true;
  }, []);

  const handleScroll = useCallback(() => {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }
    scrollTopRef.current = element.scrollTop;

    if (itemsRef.current.length === 0) {
      shouldLockToBottomRef.current = false;
      userScrollIntentRef.current = false;
      return;
    }

    const isAtBottom =
      element.scrollHeight - element.scrollTop - element.clientHeight <=
      CHAT_BOTTOM_LOCK_THRESHOLD_PX;
    if (isAtBottom || userScrollIntentRef.current) {
      shouldLockToBottomRef.current = isAtBottom;
    }
    userScrollIntentRef.current = false;
  }, []);

  const displayError = error ?? transcriptError;
  const loading = isLoading || isTranscriptLoading;
  const showLoadingEmpty =
    !items.length && !displayError && !snapshotMatchesChat && loading;
  const showNoMessagesWhileWaiting =
    !items.length && !displayError && !snapshotMatchesChat && !loading;
  const showInstanceMissing =
    !items.length && snapshotMatchesChat && !instance;
  const showEmptyTranscript =
    !items.length && snapshotMatchesChat && instance && !loading;

  return (
    <div className="chat-panel flex min-h-0 flex-1 flex-col overflow-hidden">
      <header className="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--border)] bg-[var(--surface)] px-4 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <Button
            aria-label={t("Main chat")}
            className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)]"
            onPress={onOpenMainChat}
          >
            <ArrowLeft aria-hidden="true" className="size-4" />
          </Button>
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <Bot aria-hidden="true" className="size-4 shrink-0 text-[var(--accent-soft-foreground)]" />
              <h2 className="truncate text-sm font-semibold text-[var(--foreground)]">
                {instance?.definitionSnapshot.name ?? t("Agent transcript")}
              </h2>
              <span className="rounded-full border border-[var(--border)] bg-[var(--surface-secondary)] px-2 py-0.5 text-[11px] font-semibold text-[var(--muted)]">
                {t("Read-only")}
              </span>
            </div>
            <div className="mt-1 flex min-w-0 flex-wrap gap-1.5 text-[11px] font-semibold uppercase tracking-normal text-[var(--muted)]">
              <span>{instance?.role ?? t("Agent")}</span>
              {instance ? <span>{instance.status}</span> : null}
              {instance ? (
                <span>
                  {instance.executionWorkspaceMode === "isolated_worktree"
                    ? t("isolated")
                    : t("shared")}
                </span>
              ) : null}
            </div>
          </div>
        </div>
        <Button
          aria-label={t("Refresh")}
          className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
          isDisabled={loading}
          onPress={() => void refreshTranscript()}
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${loading ? "animate-spin" : ""}`}
          />
        </Button>
      </header>

      <div
        className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4"
        onKeyDown={markUserScrollIntent}
        onScroll={handleScroll}
        onTouchMove={markUserScrollIntent}
        onWheel={markUserScrollIntent}
        ref={messageScrollRef}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${
            items.length ? "max-w-5xl gap-4" : "max-w-3xl"
          }`}
          ref={messageScrollContentRef}
        >
          {displayError ? (
            <div className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
              {displayError}
            </div>
          ) : null}

          {showLoadingEmpty ? (
            <AgentTranscriptEmptyState text={t("Loading agent messages…")} />
          ) : null}

          {showNoMessagesWhileWaiting ? (
            <AgentTranscriptEmptyState text={t("No agent messages yet.")} />
          ) : null}

          {showInstanceMissing ? (
            <AgentTranscriptEmptyState text={t("Agent instance not found.")} />
          ) : null}

          {showEmptyTranscript ? (
            <AgentTranscriptEmptyState text={t("No agent messages yet.")} />
          ) : null}

          {items.map((item) => (
            <AgentTranscriptBubble
              helpers={helpers}
              item={item}
              key={item.id}
              workspaceId={workspaceId}
            />
          ))}

          {snapshotMatchesChat && instance && items.length && hasMore ? (
            <div className="flex justify-center pt-1">
              <Button
                className="inline-flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs font-semibold text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                isDisabled={loading}
                onPress={() => void loadMoreTranscript()}
              >
                {loading ? (
                  <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                ) : null}
                {t("Load more")}
              </Button>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function AgentTranscriptBubble({
  helpers,
  item,
  workspaceId,
}: {
  helpers: ChatPanelHelpers;
  item: AgentTranscriptItem;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const isUser = item.role === "user";
  const isStreaming = item.status === "streaming";
  const parts = item.parts.map((part, partIndex) =>
    normalizeAgentTranscriptPart(part as AgentTranscriptWirePart, item.id, partIndex),
  );
  const reasoningPartCount = parts.filter(
    (part) => part.type === "reasoning",
  ).length;

  return (
    <div
      className={`message-row flex ${
        isUser ? "message-row-user" : "message-row-agent"
      }`}
    >
      <div className="message-card-shell">
        <div
          className={`message-bubble flex max-w-[min(42rem,92%)] items-start gap-3 rounded-2xl border px-4 py-3 shadow-[var(--overlay-shadow)] sm:max-w-[78%] ${
            isUser
              ? "message-bubble-user flex-row rounded-tr-md"
              : "message-bubble-assistant flex-row rounded-tl-md"
          }`}
          style={{
            backgroundColor: isUser
              ? "var(--accent-soft)"
              : "var(--surface)",
            borderColor: isUser
              ? "var(--accent)"
              : "var(--border)",
          }}
        >
          <div
            className={`message-avatar mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${
              isUser ? "bg-[color-mix(in_oklab,var(--accent)_45%,transparent)] text-white" : "bg-[var(--surface-secondary)] text-[var(--muted)]"
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
                <span>{item.author}</span>
                <span className="message-run-badge">{t(item.kind)}</span>
                {item.taskStatus ? (
                  <span className="message-run-badge">
                    {t("Task")} {item.taskStatus}
                  </span>
                ) : null}
                <time
                  className="message-created-at"
                  dateTime={item.createdAt}
                  title={item.createdAt}
                >
                  {formatAgentTimestamp(item.createdAt)}
                </time>
              </span>
            </div>
            {parts.length ? (
              parts.map((part, partIndex) => (
                <MessagePartBlock
                  helpers={helpers}
                  isError={item.status === "error"}
                  isStreaming={isStreaming}
                  isStreamingTail={partIndex === parts.length - 1}
                  isUser={isUser}
                  key={`${item.id}-part-${partIndex}`}
                  part={part}
                  reasoningDurationFallbackMs={
                    reasoningPartCount === 1
                      ? item.metrics?.totalLatencyMs ?? null
                      : null
                  }
                  workspaceId={workspaceId}
                />
              ))
            ) : isStreaming ? (
              <LoaderCircle
                aria-hidden="true"
                className="size-4 animate-spin"
              />
            ) : (
              <MarkdownContent
                content={item.content}
                isUser={isUser}
                selectedSkillPrefix={noSelectedSkillPrefix}
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function AgentTranscriptEmptyState({ text }: { text: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_80%,transparent)] px-6 py-16 text-center">
      <div className="inline-flex size-12 items-center justify-center rounded-2xl border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)]">
        <ListChecks aria-hidden="true" className="size-5" />
      </div>
      <p className="max-w-sm text-sm text-[var(--muted)]">{text}</p>
    </div>
  );
}

function formatAgentTimestamp(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
  }).format(date);
}
