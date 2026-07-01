import { ArrowLeft, Bot, ListChecks, LoaderCircle, RefreshCw, User } from "lucide-react";
import { useMemo } from "react";

import type {
  AgentInstanceView,
  AgentMessageView,
  AgentRunEventView,
  AgentTaskView,
  AgentTeamSnapshotResponse,
  ChatMessagePart,
  ChatReplyMetrics,
  ChatToolCallSummary,
  JsonValue,
} from "../../api/types";
import {
  MessagePartBlock,
  type ChatPanelHelpers,
} from "../chat/ChatPanel";
import { MarkdownContent } from "../chat/MarkdownContent";
import { useI18n } from "../../shared/i18n";

type AgentTranscriptItem = {
  author: string;
  content: string;
  createdAt: string;
  id: string;
  kind: string;
  metrics: ChatReplyMetrics | null;
  parts: ChatMessagePart[];
  role: "assistant" | "user";
  status?: "error" | "streaming";
  taskStatus: string | null;
};

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
            <AgentTranscriptBubble
              helpers={helpers}
              item={item}
              key={item.id}
              workspaceId={workspaceId}
            />
          ))}
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
  const runEventsByTaskId = new Map<string, AgentRunEventView[]>();
  for (const event of snapshot.runEvents ?? []) {
    const events = runEventsByTaskId.get(event.runId) ?? [];
    events.push(event);
    runEventsByTaskId.set(event.runId, events);
  }
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
        metrics: null,
        parts: [],
        role: "user",
        taskStatus: task.status,
      });
    }

    const runItem = taskRunTranscriptItem(
      task,
      runEventsByTaskId.get(task.id) ?? [],
      instance.definitionSnapshot.name,
    );
    if (runItem) {
      items.push(runItem);
    } else {
      const output = taskOutputContent(task);
      if (output) {
        items.push({
          author: instance.definitionSnapshot.name,
          content: output.content,
          createdAt: task.completedAt ?? task.updatedAt,
          id: `task:${task.id}:output`,
          kind: output.kind,
          metrics: null,
          parts: [],
          role: "assistant",
          taskStatus: task.status,
        });
      }
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
      metrics: null,
      parts: [],
      role: isOutgoing ? "assistant" : "user",
      taskStatus: relatedTask?.status ?? null,
    });
  }

  return items.sort(compareTranscriptItems);
}

function taskRunTranscriptItem(
  task: AgentTaskView,
  events: AgentRunEventView[],
  author: string,
): AgentTranscriptItem | null {
  const sortedEvents = [...events].sort(compareRunEvents);
  if (!sortedEvents.length) {
    if (task.status !== "running") {
      return null;
    }
    return {
      author,
      content: "",
      createdAt: task.startedAt ?? task.updatedAt,
      id: `task:${task.id}:run`,
      kind: "Task run",
      metrics: null,
      parts: [],
      role: "assistant",
      status: "streaming",
      taskStatus: task.status,
    };
  }

  let content = "";
  let metrics: ChatReplyMetrics | null = null;
  let parts: ChatMessagePart[] = [];
  let status: AgentTranscriptItem["status"] = task.status === "running" ? "streaming" : undefined;

  for (const event of sortedEvents) {
    const payload = jsonRecord(event.payload);
    const type = payload ? jsonRawStringField(payload, "type") : null;
    if (!payload || !type) {
      continue;
    }

    if (type === "textDelta") {
      const delta = jsonRawStringField(payload, "delta") ?? "";
      content += delta;
      parts = appendTextPart(parts, delta);
      if (!status) {
        status = "streaming";
      }
      continue;
    }

    if (type === "reasoningDelta") {
      const delta = jsonRawStringField(payload, "delta") ?? "";
      parts = appendReasoningPart(parts, delta);
      if (!status) {
        status = "streaming";
      }
      continue;
    }

    if (type === "toolCall") {
      const toolCall = chatToolCallSummary(jsonField(payload, "toolCall", "tool_call"));
      if (toolCall) {
        parts = upsertToolCallPart(parts, toolCall);
        if (!status) {
          status = "streaming";
        }
      }
      continue;
    }

    if (type === "toolResult") {
      const toolCallId = jsonRawStringField(payload, "toolCallId", "tool_call_id");
      const output = payload.output;
      const isError = jsonBooleanField(payload, "isError", "is_error") ?? false;
      const startedAt = jsonRawStringField(payload, "startedAt", "started_at");
      const completedAt = jsonRawStringField(payload, "completedAt", "completed_at");
      if (toolCallId && isJsonValue(output)) {
        parts = applyToolResultToParts(
          parts,
          toolCallId,
          output,
          isError,
          startedAt,
          completedAt,
        );
      }
      continue;
    }

    if (type === "toolOutputDelta") {
      const toolCallId = jsonRawStringField(payload, "toolCallId", "tool_call_id");
      const stream = jsonRawStringField(payload, "stream");
      const delta = jsonRawStringField(payload, "delta") ?? "";
      if (toolCallId && (stream === "stdout" || stream === "stderr")) {
        parts = applyToolOutputDeltaToParts(parts, toolCallId, stream, delta);
      }
      continue;
    }

    if (type === "streamReset") {
      content = jsonRawStringField(payload, "text") ?? "";
      parts = partsFromRunSnapshot(
        content,
        optionalJsonString(payload.reasoning),
        jsonArrayField(payload, "toolCalls", "tool_calls")
          .map(chatToolCallSummary)
          .filter((toolCall): toolCall is ChatToolCallSummary => Boolean(toolCall)),
      );
      status = "streaming";
      continue;
    }

    if (type === "complete") {
      const finalText = jsonRawStringField(payload, "text") ?? content;
      const finalReasoning = optionalJsonString(payload.reasoning);
      parts = appendMissingReasoning(parts, finalReasoning);
      parts = appendMissingText(parts, content, finalText);
      content = finalText;
      metrics = chatReplyMetrics(payload.metrics);
      status = undefined;
      continue;
    }

    if (type === "error") {
      parts = appendErrorPart(
        parts,
        jsonRawStringField(payload, "message") ?? "Unknown error",
      );
      status = "error";
    }
  }

  const terminalOutput = task.status !== "running" ? taskOutputContent(task) : null;
  if (terminalOutput?.kind === "Task error" && !hasPartType(parts, "error")) {
    parts = appendErrorPart(parts, terminalOutput.content);
    status = "error";
  } else if (terminalOutput?.kind === "Task result" && !content) {
    content = terminalOutput.content;
    parts = appendTextPart(parts, terminalOutput.content);
  }

  if (!parts.length && !content && task.status !== "running") {
    return null;
  }

  return {
    author,
    content,
    createdAt: sortedEvents[0]?.createdAt ?? task.startedAt ?? task.updatedAt,
    id: `task:${task.id}:run`,
    kind: status === "streaming" ? "Task run" : status === "error" ? "Task error" : "Task result",
    metrics,
    parts,
    role: "assistant",
    status,
    taskStatus: task.status,
  };
}

function appendTextPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }
  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "text") {
    return [...parts, { type: "text", text }];
  }
  return [...parts.slice(0, -1), { ...lastPart, text: lastPart.text + text }];
}

function appendReasoningPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }
  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "reasoning") {
    return [...parts, { type: "reasoning", text }];
  }
  return [...parts.slice(0, -1), { ...lastPart, text: lastPart.text + text }];
}

function appendErrorPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }
  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "error") {
    return [...parts, { type: "error", text }];
  }
  return [...parts.slice(0, -1), { ...lastPart, text: lastPart.text + text }];
}

function appendMissingText(
  parts: ChatMessagePart[],
  current: string,
  finalText: string,
): ChatMessagePart[] {
  const suffix = missingFinalSuffix(current, finalText);
  return suffix ? appendTextPart(parts, suffix) : parts;
}

function appendMissingReasoning(
  parts: ChatMessagePart[],
  finalReasoning: string | null,
): ChatMessagePart[] {
  if (!finalReasoning) {
    return parts;
  }
  const existing = parts
    .filter((part): part is Extract<ChatMessagePart, { type: "reasoning" }> =>
      part.type === "reasoning",
    )
    .map((part) => part.text)
    .join("");
  const suffix = missingFinalSuffix(existing, finalReasoning);
  return suffix ? appendReasoningPart(parts, suffix) : parts;
}

function hasPartType(parts: ChatMessagePart[], type: ChatMessagePart["type"]) {
  return parts.some((part) => part.type === type);
}

function missingFinalSuffix(current: string, next: string) {
  if (!next || current === next) {
    return "";
  }
  return next.startsWith(current) ? next.slice(current.length) : "";
}

function partsFromRunSnapshot(
  text: string,
  reasoning: string | null,
  toolCalls: ChatToolCallSummary[],
): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [];
  if (reasoning) {
    parts.push({ type: "reasoning", text: reasoning });
  }
  if (text) {
    parts.push({ type: "text", text });
  }
  parts.push(...toolCalls.map((toolCall) => ({ type: "toolCall" as const, toolCall })));
  return parts;
}

function upsertToolCallPart(
  parts: ChatMessagePart[],
  nextToolCall: ChatToolCallSummary,
): ChatMessagePart[] {
  const existingIndex = parts.findIndex(
    (part) => part.type === "toolCall" && part.toolCall.id === nextToolCall.id,
  );
  if (existingIndex === -1) {
    return [...parts, { type: "toolCall", toolCall: nextToolCall }];
  }
  return parts.map((part, index) =>
    index === existingIndex && part.type === "toolCall"
      ? {
          type: "toolCall",
          toolCall: {
            ...part.toolCall,
            ...nextToolCall,
            liveOutput: nextToolCall.liveOutput ?? part.toolCall.liveOutput,
          },
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
      ? {
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
        }
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
      ? {
          type: "toolCall",
          toolCall: {
            ...part.toolCall,
            liveOutput: appendToolLiveOutput(part.toolCall.liveOutput, stream, delta),
          },
        }
      : part,
  );
}

function appendToolLiveOutput(
  liveOutput: ChatToolCallSummary["liveOutput"],
  stream: "stdout" | "stderr",
  delta: string,
) {
  return {
    stdout: (liveOutput?.stdout ?? "") + (stream === "stdout" ? delta : ""),
    stderr: (liveOutput?.stderr ?? "") + (stream === "stderr" ? delta : ""),
  };
}

function chatToolCallSummary(value: JsonValue | undefined): ChatToolCallSummary | null {
  if (!isJsonValue(value)) {
    return null;
  }
  const record = jsonRecord(value);
  if (!record) {
    return null;
  }
  const id = jsonRawStringField(record, "id");
  const name = jsonRawStringField(record, "name");
  if (!id || !name) {
    return null;
  }
  const output = record.output;
  const liveOutput = jsonRecord(jsonField(record, "liveOutput", "live_output"));
  const stdout = liveOutput ? jsonRawStringField(liveOutput, "stdout") ?? "" : "";
  const stderr = liveOutput ? jsonRawStringField(liveOutput, "stderr") ?? "" : "";
  const startedAt = jsonRawStringField(record, "startedAt", "started_at");
  const completedAt = jsonRawStringField(record, "completedAt", "completed_at");
  return {
    id,
    name,
    status: jsonRawStringField(record, "status") ?? "running",
    input: isJsonValue(record.input) ? record.input : {},
    output: isJsonValue(output) ? output : null,
    isError: jsonBooleanField(record, "isError", "is_error") ?? false,
    startedAt,
    completedAt,
    liveOutput: stdout || stderr ? { stdout, stderr } : undefined,
  };
}

function chatReplyMetrics(value: JsonValue | undefined): ChatReplyMetrics | null {
  if (!isJsonValue(value)) {
    return null;
  }
  const record = jsonRecord(value);
  const modelId = record ? jsonRawStringField(record, "modelId") : null;
  const providerId = record ? jsonRawStringField(record, "providerId") : null;
  if (!record || !modelId || !providerId) {
    return null;
  }
  return {
    modelId,
    providerId,
    totalLatencyMs: nullableJsonNumber(record.totalLatencyMs),
    firstTokenLatencyMs: nullableJsonNumber(record.firstTokenLatencyMs),
    outputTokens: nullableJsonNumber(record.outputTokens),
  };
}

function compareRunEvents(left: AgentRunEventView, right: AgentRunEventView) {
  if (left.sequence !== right.sequence) {
    return left.sequence - right.sequence;
  }
  return left.createdAt.localeCompare(right.createdAt);
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

function jsonStringField(record: Record<string, JsonValue>, ...keys: string[]) {
  const value = jsonField(record, ...keys);
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function jsonRawStringField(record: Record<string, JsonValue>, ...keys: string[]) {
  const value = jsonField(record, ...keys);
  return typeof value === "string" ? value : null;
}

function jsonBooleanField(record: Record<string, JsonValue>, ...keys: string[]) {
  const value = jsonField(record, ...keys);
  return typeof value === "boolean" ? value : null;
}

function jsonArrayField(record: Record<string, JsonValue>, ...keys: string[]) {
  const value = jsonField(record, ...keys);
  return Array.isArray(value) ? value : [];
}

function jsonField(record: Record<string, JsonValue>, ...keys: string[]) {
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(record, key)) {
      return record[key];
    }
  }
  return undefined;
}

function optionalJsonString(value: JsonValue | undefined) {
  return typeof value === "string" ? value : null;
}

function nullableJsonNumber(value: JsonValue | undefined) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return true;
  }
  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }
  if (typeof value === "object") {
    return Object.values(value as Record<string, unknown>).every(isJsonValue);
  }
  return false;
}

function jsonRecord(value: JsonValue | undefined): Record<string, JsonValue> | null {
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
