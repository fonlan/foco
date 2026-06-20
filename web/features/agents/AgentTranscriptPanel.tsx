import { ArrowLeft, Bot, ListChecks, RefreshCw, User } from "lucide-react";
import { useMemo } from "react";

import type {
  AgentInstanceView,
  AgentMessageView,
  AgentTaskView,
  AgentTeamSnapshotResponse,
  JsonValue,
} from "../../api/types";
import { MarkdownContent } from "../chat/MarkdownContent";
import { useI18n } from "../../shared/i18n";

type AgentTranscriptItem = {
  author: string;
  content: string;
  createdAt: string;
  id: string;
  kind: string;
  role: "assistant" | "user";
  taskStatus: string | null;
};

const noSelectedSkillPrefix = () => null;

export function AgentTranscriptPanel({
  error,
  instanceId,
  isLoading,
  onOpenMainChat,
  onRefresh,
  snapshot,
}: {
  error: string | null;
  instanceId: string;
  isLoading: boolean;
  onOpenMainChat: () => void;
  onRefresh: () => Promise<void>;
  snapshot: AgentTeamSnapshotResponse | null;
}) {
  const { t } = useI18n();
  const instance =
    snapshot?.instances.find((current) => current.id === instanceId) ?? null;
  const items = useMemo(
    () =>
      snapshot && instance
        ? buildAgentTranscriptItems(snapshot, instance, t("Main agent"))
        : [],
    [instance, snapshot, t],
  );

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
          disabled={isLoading}
          onClick={() => void onRefresh()}
          title={t("Refresh")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${isLoading ? "animate-spin" : ""}`}
          />
        </button>
      </header>

      <div className="message-list panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4">
        <div
          className={`message-stack mx-auto flex w-full flex-col ${
            items.length ? "max-w-5xl gap-4" : "max-w-3xl"
          }`}
        >
          {error ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}

          {!snapshot && !error ? (
            <AgentTranscriptEmptyState
              text={
                isLoading
                  ? t("Loading agent messages...")
                  : t("No agent messages yet.")
              }
            />
          ) : null}

          {snapshot && !instance ? (
            <AgentTranscriptEmptyState text={t("Agent instance not found.")} />
          ) : null}

          {snapshot && instance && !items.length ? (
            <AgentTranscriptEmptyState text={t("No agent messages yet.")} />
          ) : null}

          {items.map((item) => (
            <AgentTranscriptBubble item={item} key={item.id} />
          ))}
        </div>
      </div>
    </div>
  );
}

function AgentTranscriptBubble({ item }: { item: AgentTranscriptItem }) {
  const { t } = useI18n();
  const isUser = item.role === "user";

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
            <MarkdownContent
              content={item.content}
              isUser={isUser}
              selectedSkillPrefix={noSelectedSkillPrefix}
            />
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

function buildAgentTranscriptItems(
  snapshot: AgentTeamSnapshotResponse,
  instance: AgentInstanceView,
  mainAgentLabel: string,
) {
  const instancesById = new Map(
    snapshot.instances.map((current) => [current.id, current]),
  );
  const tasksById = new Map(snapshot.tasks.map((task) => [task.id, task]));
  const incomingMessageTaskIds = new Set(
    snapshot.messages
      .filter(
        (message) =>
          message.receiverInstanceId === instance.id && message.relatedTaskId,
      )
      .map((message) => message.relatedTaskId as string),
  );
  const items: AgentTranscriptItem[] = [];

  for (const task of snapshot.tasks) {
    if (task.ownerInstanceId !== instance.id) {
      continue;
    }

    if (!incomingMessageTaskIds.has(task.id)) {
      items.push({
        author: instanceName(
          snapshot,
          instancesById,
          task.originInstanceId,
          mainAgentLabel,
        ),
        content: taskInputContent(task),
        createdAt: task.createdAt,
        id: `task:${task.id}:input`,
        kind: "Task input",
        role: "user",
        taskStatus: task.status,
      });
    }

    const output = taskOutputContent(task);
    if (output) {
      items.push({
        author: instance.definitionSnapshot.name,
        content: output.content,
        createdAt: task.completedAt ?? task.updatedAt,
        id: `task:${task.id}:output`,
        kind: output.kind,
        role: "assistant",
        taskStatus: task.status,
      });
    }
  }

  for (const message of snapshot.messages) {
    if (
      message.senderInstanceId !== instance.id &&
      message.receiverInstanceId !== instance.id
    ) {
      continue;
    }

    const isOutgoing = message.senderInstanceId === instance.id;
    const relatedTask = message.relatedTaskId
      ? tasksById.get(message.relatedTaskId) ?? null
      : null;
    items.push({
      author: isOutgoing
        ? instance.definitionSnapshot.name
        : instanceName(
            snapshot,
            instancesById,
            message.senderInstanceId,
            mainAgentLabel,
          ),
      content: message.content,
      createdAt: message.createdAt,
      id: `message:${message.id}`,
      kind: messageKindLabel(message),
      role: isOutgoing ? "assistant" : "user",
      taskStatus: relatedTask?.status ?? null,
    });
  }

  return items.sort(compareTranscriptItems);
}

function instanceName(
  snapshot: AgentTeamSnapshotResponse,
  instancesById: Map<string, AgentInstanceView>,
  instanceId: string | null,
  mainAgentLabel: string,
) {
  if (!instanceId) {
    return mainAgentLabel;
  }

  const instance = instancesById.get(instanceId);
  if (!instance) {
    return instanceId;
  }

  if (instance.id === snapshot.team.coordinatorInstanceId) {
    return instance.definitionSnapshot.name || mainAgentLabel;
  }

  return instance.definitionSnapshot.name || instance.id;
}

function taskInputContent(task: AgentTaskView) {
  const record = jsonRecord(task.input);
  const message = record ? jsonStringField(record, "message") : null;
  if (message) {
    return message;
  }

  const delegatedInput = record?.delegatedInput;
  return delegatedInput ? formatJsonValue(delegatedInput) : formatJsonValue(task.input);
}

function taskOutputContent(task: AgentTaskView) {
  if (task.error) {
    return {
      content: jsonMessageText(task.error) ?? formatJsonValue(task.error),
      kind: "Task error",
    };
  }

  if (!task.result) {
    return null;
  }

  return {
    content: jsonMessageText(task.result) ?? formatJsonValue(task.result),
    kind: "Task result",
  };
}

function jsonMessageText(value: JsonValue) {
  const record = jsonRecord(value);
  if (!record) {
    return typeof value === "string" ? value : null;
  }

  return (
    jsonStringField(record, "text") ??
    jsonStringField(record, "message") ??
    jsonStringField(record, "content")
  );
}

function jsonStringField(record: Record<string, JsonValue>, key: string) {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function jsonRecord(value: JsonValue): Record<string, JsonValue> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value
    : null;
}

function formatJsonValue(value: JsonValue) {
  return `\`\`\`json\n${JSON.stringify(value, null, 2)}\n\`\`\``;
}

function messageKindLabel(message: AgentMessageView) {
  return message.kind === "reply" ? "Reply" : "Message";
}

function compareTranscriptItems(
  left: AgentTranscriptItem,
  right: AgentTranscriptItem,
) {
  const leftTime = transcriptTimestamp(left.createdAt);
  const rightTime = transcriptTimestamp(right.createdAt);
  if (leftTime !== rightTime) {
    return leftTime - rightTime;
  }

  return left.id.localeCompare(right.id);
}

function transcriptTimestamp(value: string) {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? 0 : timestamp;
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
