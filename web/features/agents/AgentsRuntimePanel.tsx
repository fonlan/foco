import {
  Bot,
  CheckCircle2,
  CircleAlert,
  LoaderCircle,
  MessageSquare,
  Pause,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Square,
  Trash2,
  User,
} from "lucide-react";
import { useMemo, useState } from "react";

import type {
  AgentDefinitionSettings,
  AgentInstanceView,
  AgentTaskView,
  AgentTeamSnapshotResponse,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";

type AgentRuntimeAction =
  | "delete"
  | "drain"
  | "pause"
  | "reset_context"
  | "resume"
  | "stop";
type AgentRuntimeScope = "instance" | "team";
type AgentTaskAction = "cancel" | "retry" | "transfer";

export function AgentsRuntimePanel({
  activeChatId,
  definitions,
  error,
  isLoading,
  operationKey,
  onCreateInstances,
  onEnableTeam,
  onRefresh,
  onRuntimeAction,
  onTaskAction,
  snapshot,
}: {
  activeChatId: string | null;
  definitions: AgentDefinitionSettings[];
  error: string | null;
  isLoading: boolean;
  operationKey: string | null;
  onCreateInstances: (definitionId: string, count: number) => Promise<void>;
  onEnableTeam: (coordinatorDefinitionId: string) => Promise<void>;
  onRefresh: () => Promise<void>;
  onRuntimeAction: (
    scope: AgentRuntimeScope,
    action: AgentRuntimeAction,
    instanceId?: string,
  ) => Promise<void>;
  onTaskAction: (
    taskId: string,
    action: AgentTaskAction,
    targetInstanceId?: string,
  ) => Promise<void>;
  snapshot: AgentTeamSnapshotResponse | null;
}) {
  const { t } = useI18n();
  const [coordinatorDefinitionId, setCoordinatorDefinitionId] = useState("");
  const [workerDefinitionId, setWorkerDefinitionId] = useState("");
  const [workerCount, setWorkerCount] = useState("1");
  const sortedTasks = useMemo(
    () => [...(snapshot?.tasks ?? [])].sort((left, right) => right.sequence - left.sequence),
    [snapshot?.tasks],
  );
  const sortedEvents = useMemo(
    () => [...(snapshot?.events ?? [])].sort((left, right) => right.sequence - left.sequence),
    [snapshot?.events],
  );
  const sortedMessages = useMemo(
    () => [...(snapshot?.messages ?? [])].sort((left, right) => right.sequence - left.sequence),
    [snapshot?.messages],
  );
  const activeDefinitions = definitions.filter((definition) => definition.maxInstances > 0);
  const effectiveCoordinatorDefinitionId = coordinatorDefinitionId || activeDefinitions[0]?.id || "";
  const effectiveWorkerDefinitionId = workerDefinitionId || activeDefinitions[0]?.id || "";
  const canEnable = Boolean(activeChatId && effectiveCoordinatorDefinitionId && operationKey === null);
  const canCreateWorker = Boolean(snapshot && effectiveWorkerDefinitionId && operationKey === null);

  return (
    <section className="panel-scroll flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Bot aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-stone-950">
              {t("Agents")}
            </h3>
            <p className="truncate text-xs text-stone-500">
              {snapshot ? snapshot.team.status : t("Team is not enabled")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Refresh")}
          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-300"
          disabled={!activeChatId || isLoading}
          onClick={() => void onRefresh()}
          title={t("Refresh")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${isLoading ? "animate-spin" : ""}`}
          />
        </button>
      </div>

      {error ? (
        <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
          {error}
        </div>
      ) : null}

      {!activeChatId ? (
        <AgentEmptyState text={t("Open a chat to manage its Agent team.")} />
      ) : null}

      {activeChatId && !snapshot ? (
        <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
          <div className="flex items-center gap-2">
            <Plus aria-hidden="true" className="size-4 text-teal-700" />
            <h4 className="text-sm font-semibold text-stone-950">
              {t("Enable team")}
            </h4>
          </div>
          <div className="mt-3 grid gap-2">
            <AgentDefinitionSelect
              definitions={activeDefinitions}
              label={t("Coordinator")}
              onChange={setCoordinatorDefinitionId}
              value={effectiveCoordinatorDefinitionId}
            />
            <button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300"
              disabled={!canEnable}
              onClick={() => void onEnableTeam(effectiveCoordinatorDefinitionId)}
              type="button"
            >
              {operationKey === "agent-team-enable" ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <CheckCircle2 aria-hidden="true" className="size-4" />
              )}
              <span>{t("Enable")}</span>
            </button>
          </div>
        </div>
      ) : null}

      {snapshot ? (
        <>
          <div className="grid grid-cols-3 gap-2">
            <AgentMetric label={t("Queued")} value={snapshot.workload.queuedTasks} />
            <AgentMetric label={t("Running")} value={snapshot.workload.runningTasks} />
            <AgentMetric label={t("Waiting")} value={snapshot.workload.waitingTasks} />
          </div>

          <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h4 className="text-sm font-semibold text-stone-950">{t("Team")}</h4>
              <div className="flex flex-wrap gap-1.5">
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={Pause}
                  label={t("Pause")}
                  onClick={() => void onRuntimeAction("team", "pause")}
                />
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={Play}
                  label={t("Resume")}
                  onClick={() => void onRuntimeAction("team", "resume")}
                />
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={Square}
                  label={t("Drain")}
                  onClick={() => void onRuntimeAction("team", "drain")}
                />
                <AgentIconButton
                  danger
                  disabled={operationKey !== null}
                  icon={CircleAlert}
                  label={t("Stop")}
                  onClick={() => void onRuntimeAction("team", "stop")}
                />
              </div>
            </div>
            <div className="mt-3 grid gap-2 text-xs text-stone-600">
              <AgentKeyValue label={t("Team ID")} value={snapshot.team.id} />
              <AgentKeyValue label={t("Coordinator")} value={snapshot.team.coordinatorInstanceId} />
            </div>
          </div>

          <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
            <h4 className="text-sm font-semibold text-stone-950">{t("Create workers")}</h4>
            <div className="mt-3 grid gap-2">
              <AgentDefinitionSelect
                definitions={activeDefinitions}
                label={t("Definition")}
                onChange={setWorkerDefinitionId}
                value={effectiveWorkerDefinitionId}
              />
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("Count")}
                </span>
                <input
                  className="h-9 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm outline-none focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  min={1}
                  onChange={(event) => setWorkerCount(event.target.value)}
                  step={1}
                  type="number"
                  value={workerCount}
                />
              </label>
              <button
                className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-800 hover:border-teal-200 hover:bg-teal-50 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={!canCreateWorker}
                onClick={() =>
                  void onCreateInstances(
                    effectiveWorkerDefinitionId,
                    Math.max(1, Number.parseInt(workerCount, 10) || 1),
                  )
                }
                type="button"
              >
                <Plus aria-hidden="true" className="size-4" />
                <span>{t("Create")}</span>
              </button>
            </div>
          </div>

          <AgentInstancesSection
            instances={snapshot.instances}
            operationKey={operationKey}
            onRuntimeAction={onRuntimeAction}
          />

          <AgentTasksSection
            instances={snapshot.instances}
            operationKey={operationKey}
            onTaskAction={onTaskAction}
            tasks={sortedTasks}
          />

          <AgentObservabilitySection observability={snapshot.observability} />

          <AgentTimelineSection
            events={sortedEvents.slice(0, 16)}
            messages={sortedMessages.slice(0, 12)}
          />

          {snapshot.mutationLeaseOwners.length ? (
            <div className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-3">
              <h4 className="text-sm font-semibold text-amber-950">
                {t("Workspace mutation leases")}
              </h4>
              <div className="mt-2 grid gap-2">
                {snapshot.mutationLeaseOwners.map((owner, index) => (
                  <div className="rounded-lg bg-white/80 px-2 py-2 text-xs text-amber-900" key={`${owner.taskId ?? owner.toolCallId ?? "lease"}-${index}`}>
                    <div className="font-semibold">{owner.toolName ?? t("Tool call")}</div>
                    <div className="mt-0.5 truncate">{owner.taskId ?? owner.instanceId ?? owner.toolCallId}</div>
                    <div className="mt-0.5 text-amber-700">
                      {owner.activeMs}ms / {owner.waitMs}ms
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function AgentInstancesSection({
  instances,
  operationKey,
  onRuntimeAction,
}: {
  instances: AgentInstanceView[];
  operationKey: string | null;
  onRuntimeAction: (
    scope: AgentRuntimeScope,
    action: AgentRuntimeAction,
    instanceId?: string,
  ) => Promise<void>;
}) {
  const { t } = useI18n();
  return (
    <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
      <h4 className="text-sm font-semibold text-stone-950">{t("Instances")}</h4>
      <div className="mt-3 grid gap-2">
        {instances.map((instance) => (
          <div className="rounded-lg border border-stone-200 bg-stone-50/70 px-3 py-2" key={instance.id}>
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  <User aria-hidden="true" className="size-4 shrink-0 text-teal-700" />
                  <span className="truncate text-sm font-semibold text-stone-950">
                    {instance.definitionSnapshot.name}
                  </span>
                </div>
                <div className="mt-1 flex flex-wrap gap-1.5 text-[11px] font-semibold uppercase tracking-normal text-stone-500">
                  <span>{instance.role}</span>
                  <span>{instance.status}</span>
                </div>
              </div>
              <div className="flex shrink-0 gap-1">
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={Pause}
                  label={t("Pause")}
                  onClick={() => void onRuntimeAction("instance", "pause", instance.id)}
                />
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={Play}
                  label={t("Resume")}
                  onClick={() => void onRuntimeAction("instance", "resume", instance.id)}
                />
                <AgentIconButton
                  disabled={operationKey !== null}
                  icon={RotateCcw}
                  label={t("Reset context")}
                  onClick={() => void onRuntimeAction("instance", "reset_context", instance.id)}
                />
                <AgentIconButton
                  danger
                  disabled={operationKey !== null}
                  icon={Trash2}
                  label={t("Delete")}
                  onClick={() => void onRuntimeAction("instance", "delete", instance.id)}
                />
              </div>
            </div>
            <div className="mt-2 truncate text-xs text-stone-500">{instance.id}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function AgentTasksSection({
  instances,
  operationKey,
  onTaskAction,
  tasks,
}: {
  instances: AgentInstanceView[];
  operationKey: string | null;
  onTaskAction: (
    taskId: string,
    action: AgentTaskAction,
    targetInstanceId?: string,
  ) => Promise<void>;
  tasks: AgentTaskView[];
}) {
  const { t } = useI18n();
  const workerInstances = instances.filter((instance) => instance.role !== "coordinator");
  return (
    <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
      <h4 className="text-sm font-semibold text-stone-950">{t("Tasks")}</h4>
      <div className="mt-3 grid gap-2">
        {tasks.length ? (
          tasks.slice(0, 20).map((task) => (
            <div className="rounded-lg border border-stone-200 bg-stone-50/70 px-3 py-2" key={task.id}>
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold text-stone-950">
                    #{task.sequence} {task.status}
                  </div>
                  <div className="mt-1 truncate text-xs text-stone-500">{task.id}</div>
                </div>
                <div className="flex shrink-0 gap-1">
                  <AgentIconButton
                    danger
                    disabled={operationKey !== null}
                    icon={Square}
                    label={t("Cancel")}
                    onClick={() => void onTaskAction(task.id, "cancel")}
                  />
                  <AgentIconButton
                    disabled={operationKey !== null}
                    icon={RotateCcw}
                    label={t("Retry")}
                    onClick={() => void onTaskAction(task.id, "retry")}
                  />
                </div>
              </div>
              {workerInstances.length ? (
                <select
                  className="mt-2 h-8 w-full rounded-lg border border-stone-300 bg-white px-2 text-xs outline-none focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  disabled={operationKey !== null}
                  onChange={(event) => {
                    if (event.target.value) {
                      void onTaskAction(task.id, "transfer", event.target.value);
                      event.target.value = "";
                    }
                  }}
                  value=""
                >
                  <option value="">{t("Transfer to instance")}</option>
                  {workerInstances.map((instance) => (
                    <option key={instance.id} value={instance.id}>
                      {instance.definitionSnapshot.name} / {instance.status}
                    </option>
                  ))}
                </select>
              ) : null}
            </div>
          ))
        ) : (
          <AgentEmptyState text={t("No tasks yet.")} />
        )}
      </div>
    </div>
  );
}

function AgentObservabilitySection({
  observability,
}: {
  observability: AgentTeamSnapshotResponse["observability"];
}) {
  const { t } = useI18n();
  return (
    <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
      <h4 className="text-sm font-semibold text-stone-950">
        {t("Observability")}
      </h4>
      <div className="mt-3 grid grid-cols-2 gap-2">
        <AgentMetric label={t("Queue length")} value={observability.queueLength} />
        <AgentMetric
          label={t("Queue wait")}
          value={formatAgentMetricMs(observability.queueWaitMs.average)}
        />
        <AgentMetric
          label={t("Scheduler latency")}
          value={formatAgentMetricMs(observability.schedulerLatencyMs.average)}
        />
        <AgentMetric
          label={t("Run duration")}
          value={formatAgentMetricMs(observability.runDurationMs.average)}
        />
        <AgentMetric
          label={t("Lease wait")}
          value={formatAgentMetricMs(observability.mutationLeaseWaitMs.average)}
        />
        <AgentMetric
          label={t("Failures")}
          value={observability.failuresByType.reduce(
            (total, failure) => total + failure.count,
            0,
          )}
        />
      </div>
      {observability.failuresByType.length ? (
        <div className="mt-2 flex flex-wrap gap-1.5 text-[11px] font-semibold uppercase tracking-normal text-stone-500">
          {observability.failuresByType.map((failure) => (
            <span key={failure.kind}>
              {failure.kind}: {failure.count}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function formatAgentMetricMs(value: number | null) {
  return value === null ? "-" : `${value}ms`;
}

function AgentTimelineSection({
  events,
  messages,
}: {
  events: { eventType: string; sequence: number; taskId: string | null; createdAt: string }[];
  messages: { content: string; kind: string; sequence: number; createdAt: string }[];
}) {
  const { t } = useI18n();
  return (
    <div className="rounded-xl border border-stone-200 bg-white px-3 py-3">
      <h4 className="text-sm font-semibold text-stone-950">{t("Activity")}</h4>
      <div className="mt-3 grid gap-2">
        {events.map((event) => (
          <div className="rounded-lg bg-stone-50 px-2 py-2 text-xs" key={`event-${event.sequence}`}>
            <div className="font-semibold text-stone-900">{event.eventType}</div>
            <div className="mt-0.5 truncate text-stone-500">
              #{event.sequence} {event.taskId ?? event.createdAt}
            </div>
          </div>
        ))}
        {messages.map((message) => (
          <div className="rounded-lg bg-teal-50 px-2 py-2 text-xs" key={`message-${message.sequence}`}>
            <div className="flex items-center gap-1.5 font-semibold text-teal-950">
              <MessageSquare aria-hidden="true" className="size-3.5" />
              <span>{message.kind}</span>
            </div>
            <div className="mt-1 line-clamp-3 whitespace-pre-wrap text-teal-900">
              {message.content}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function AgentDefinitionSelect({
  definitions,
  label,
  onChange,
  value,
}: {
  definitions: AgentDefinitionSettings[];
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <select
        className="h-9 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm outline-none focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {definitions.map((definition) => (
          <option key={definition.id} value={definition.id}>
            {definition.name}
          </option>
        ))}
      </select>
    </label>
  );
}

function AgentMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="rounded-xl border border-stone-200 bg-white px-3 py-2">
      <div className="text-lg font-semibold text-stone-950">{value}</div>
      <div className="truncate text-[11px] font-semibold uppercase tracking-normal text-stone-500">
        {label}
      </div>
    </div>
  );
}

function AgentIconButton({
  danger = false,
  disabled,
  icon: Icon,
  label,
  onClick,
}: {
  danger?: boolean;
  disabled: boolean;
  icon: typeof Pause;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className={`inline-flex size-8 items-center justify-center rounded-lg border bg-white disabled:cursor-not-allowed disabled:text-stone-300 ${
        danger
          ? "border-rose-200 text-rose-700 hover:bg-rose-50"
          : "border-stone-200 text-stone-700 hover:border-teal-200 hover:bg-teal-50"
      }`}
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function AgentKeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <span className="font-semibold text-stone-800">{label}: </span>
      <span className="break-all">{value}</span>
    </div>
  );
}

function AgentEmptyState({ text }: { text: string }) {
  return (
    <div className="rounded-xl border border-dashed border-stone-200 bg-white px-3 py-6 text-center text-sm text-stone-500">
      {text}
    </div>
  );
}
