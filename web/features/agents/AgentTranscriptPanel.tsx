import { ArrowLeft, Bot, ListChecks, LoaderCircle, RefreshCw, User } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

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

type AgentTranscriptItem = AgentTranscriptItemView;
type AgentTranscriptWirePart = ChatMessagePart | {
  type?: string;
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

function normalizeAgentTranscriptPart(
  part: AgentTranscriptWirePart,
  itemId: string,
  partIndex: number,
): ChatMessagePart {
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
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);
  const scrollTopRef = useRef(initialCache?.scrollTop ?? 0);
  const stickToBottomRef = useRef(initialCache?.stickToBottom ?? true);
  const identityKey = `${workspaceId}:${chatId}:${instanceId}`;
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

  const persistTranscriptCache = useCallback(() => {
    if (!hasTranscriptBaselineRef.current && itemsRef.current.length === 0) {
      return;
    }
    writeTranscriptCacheRef.current({
      hasBaseline: hasTranscriptBaselineRef.current,
      hasMore: hasMoreRef.current,
      items: itemsRef.current,
      page: pageRef.current,
      scrollTop: scrollContainerRef.current?.scrollTop ?? scrollTopRef.current,
      stickToBottom: stickToBottomRef.current,
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
          scrollTop: scrollContainerRef.current?.scrollTop ?? scrollTopRef.current,
          stickToBottom: stickToBottomRef.current,
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

    const cache = isIdentityChange
      ? readTranscriptCacheRef.current()
      : initialCacheRef.current;
    if (cache?.hasBaseline) {
      setItems(cache.items);
      setPage(cache.page);
      setHasMore(cache.hasMore);
      setTranscriptError(null);
      hasTranscriptBaselineRef.current = true;
      scrollTopRef.current = cache.scrollTop;
      stickToBottomRef.current = cache.stickToBottom;
      requestAnimationFrame(() => {
        if (scrollContainerRef.current) {
          scrollContainerRef.current.scrollTop = cache.scrollTop;
        }
      });
    } else {
      setItems([]);
      setPage(1);
      setHasMore(false);
      setTranscriptError(null);
      hasTranscriptBaselineRef.current = false;
      scrollTopRef.current = 0;
      stickToBottomRef.current = true;
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
      stickToBottomRef.current = true;
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

  const handleScroll = useCallback(() => {
    const node = scrollContainerRef.current;
    if (!node) {
      return;
    }
    scrollTopRef.current = node.scrollTop;
    const distanceFromBottom = node.scrollHeight - node.scrollTop - node.clientHeight;
    stickToBottomRef.current = distanceFromBottom <= 48;
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
      <header className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-200 bg-white px-4 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <button
            aria-label={t("Main chat")}
            className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50"
            onClick={onOpenMainChat}
            title={t("Main chat")}
            type="button"
          >
            <ArrowLeft aria-hidden="true" className="size-4" />
          </button>
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <Bot aria-hidden="true" className="size-4 shrink-0 text-teal-700" />
              <h2 className="truncate text-sm font-semibold text-stone-950">
                {instance?.definitionSnapshot.name ?? t("Agent transcript")}
              </h2>
              <span className="rounded-full border border-stone-200 bg-stone-50 px-2 py-0.5 text-[11px] font-semibold text-stone-500">
                {t("Read-only")}
              </span>
            </div>
            <div className="mt-1 flex min-w-0 flex-wrap gap-1.5 text-[11px] font-semibold uppercase tracking-normal text-stone-500">
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
        <button
          aria-label={t("Refresh")}
          className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-300"
          disabled={loading}
          onClick={() => void refreshTranscript()}
          title={t("Refresh")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${loading ? "animate-spin" : ""}`}
          />
        </button>
      </header>

      <div
        className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4"
        onScroll={handleScroll}
        ref={scrollContainerRef}
      >
        <div
          className={`message-stack mx-auto flex w-full flex-col ${
            items.length ? "max-w-5xl gap-4" : "max-w-3xl"
          }`}
        >
          {displayError ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {displayError}
            </div>
          ) : null}

          {showLoadingEmpty ? (
            <AgentTranscriptEmptyState text={t("Loading agent messages...")} />
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
              <button
                className="inline-flex items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs font-semibold text-stone-700 hover:border-teal-200 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={loading}
                onClick={() => void loadMoreTranscript()}
                type="button"
              >
                {loading ? (
                  <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                ) : null}
                {t("Load more")}
              </button>
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
          className={`message-bubble flex max-w-[min(42rem,92%)] items-start gap-3 rounded-2xl border px-4 py-3 shadow-[0_18px_42px_rgba(75,63,42,0.08)] sm:max-w-[78%] ${
            isUser
              ? "message-bubble-user flex-row rounded-tr-md"
              : "message-bubble-assistant flex-row rounded-tl-md"
          }`}
          style={{
            backgroundColor: isUser
              ? "var(--foco-user-surface)"
              : "var(--foco-panel)",
            borderColor: isUser
              ? "var(--foco-user-border)"
              : "var(--foco-border)",
          }}
        >
          <div
            className={`message-avatar mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${
              isUser ? "bg-teal-950/45 text-white" : "bg-stone-100 text-stone-700"
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
    <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-stone-200 bg-stone-50/80 px-6 py-16 text-center">
      <div className="inline-flex size-12 items-center justify-center rounded-2xl border border-stone-200 bg-white text-stone-500">
        <ListChecks aria-hidden="true" className="size-5" />
      </div>
      <p className="max-w-sm text-sm text-stone-600">{text}</p>
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
