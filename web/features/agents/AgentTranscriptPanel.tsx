import { ArrowLeft, Bot, ListChecks, LoaderCircle, RefreshCw, User } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import type {
  AgentTranscriptItemView,
  AgentTranscriptResponse,
  AgentTeamSnapshotResponse,
} from "../../api/types";
import {
  MessagePartBlock,
  type ChatPanelHelpers,
} from "../chat/ChatPanel";
import { MarkdownContent } from "../chat/MarkdownContent";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";

type AgentTranscriptItem = AgentTranscriptItemView;

const AGENT_TRANSCRIPT_PAGE_SIZE = 25;
const noSelectedSkillPrefix = () => null;

export function AgentTranscriptPanel({
  error,
  helpers,
  instanceId,
  isLoading,
  onOpenMainChat,
  onRefresh,
  snapshot,
  workspaceId,
}: {
  error: string | null;
  helpers: ChatPanelHelpers;
  instanceId: string;
  isLoading: boolean;
  onOpenMainChat: () => void;
  onRefresh: () => Promise<void>;
  snapshot: AgentTeamSnapshotResponse | null;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const instance =
    snapshot?.instances.find((current) => current.id === instanceId) ?? null;
  const [items, setItems] = useState<AgentTranscriptItem[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [transcriptError, setTranscriptError] = useState<string | null>(null);
  const [isTranscriptLoading, setIsTranscriptLoading] = useState(false);

  const loadTranscript = useCallback(
    async (nextPage: number, mode: "replace" | "append") => {
      setIsTranscriptLoading(true);
      setTranscriptError(null);
      try {
        const data = await requestJson<AgentTranscriptResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/agent-team/instances/${encodeURIComponent(instanceId)}/transcript?page=${nextPage}&pageSize=${AGENT_TRANSCRIPT_PAGE_SIZE}`,
        );
        setItems((current) =>
          mode === "append" ? [...current, ...data.items] : data.items,
        );
        setPage(data.page);
        setHasMore(data.hasMore);
      } catch (err) {
        setTranscriptError(errorMessage(err));
        if (mode === "replace") {
          setItems([]);
          setPage(1);
          setHasMore(false);
        }
      } finally {
        setIsTranscriptLoading(false);
      }
    },
    [instanceId, workspaceId],
  );

  useEffect(() => {
    setItems([]);
    setPage(1);
    setHasMore(false);
    if (instance) {
      void loadTranscript(1, "replace");
    }
  }, [instance, loadTranscript]);

  const refreshTranscript = useCallback(async () => {
    await onRefresh();
    if (instance) {
      await loadTranscript(1, "replace");
    }
  }, [instance, loadTranscript, onRefresh]);

  const loadMoreTranscript = useCallback(async () => {
    if (!isTranscriptLoading && hasMore) {
      await loadTranscript(page + 1, "append");
    }
  }, [hasMore, isTranscriptLoading, loadTranscript, page]);

  const displayError = error ?? transcriptError;
  const loading = isLoading || isTranscriptLoading;

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

      <div className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4">
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

          {!snapshot && !displayError ? (
            <AgentTranscriptEmptyState
              text={
                loading
                  ? t("Loading agent messages...")
                  : t("No agent messages yet.")
              }
            />
          ) : null}

          {snapshot && !instance ? (
            <AgentTranscriptEmptyState text={t("Agent instance not found.")} />
          ) : null}

          {snapshot && instance && !items.length && !loading ? (
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

          {snapshot && instance && items.length && hasMore ? (
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
  const reasoningPartCount = item.parts.filter(
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
            {item.parts.length ? (
              item.parts.map((part, partIndex) => (
                <MessagePartBlock
                  helpers={helpers}
                  isError={item.status === "error"}
                  isStreaming={isStreaming}
                  isStreamingTail={partIndex === item.parts.length - 1}
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
    <div className="rounded-xl border border-dashed border-stone-200 bg-white px-3 py-10 text-center text-sm text-stone-500">
      <ListChecks aria-hidden="true" className="mx-auto mb-2 size-5 text-stone-400" />
      {text}
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
    second: "2-digit",
    year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(date);
}
