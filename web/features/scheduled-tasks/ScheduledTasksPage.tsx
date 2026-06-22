import {
  Archive,
  CalendarClock,
  Clock3,
  Copy,
  ExternalLink,
  LoaderCircle,
  Pause,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  FormEvent,
  HTMLAttributes,
  HTMLInputTypeAttribute,
  ReactNode,
} from "react";
import type { LucideIcon } from "lucide-react";

import type {
  AgentDefinitionSettings,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  JsonValue,
  ScheduledTaskAction,
  ScheduledTaskPreviewNextRunResponse,
  ScheduledTaskRunResponse,
  ScheduledTaskRunsResponse,
  ScheduledTaskRunStatus,
  ScheduledTaskRunView,
  ScheduledTaskSchedule,
  ScheduledTaskStatus,
  ScheduledTaskView,
  ScheduledTasksResponse,
  SettingsResponse,
  Translate,
  WorkspaceSummary,
} from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";

type ScheduledTasksQuery = {
  q?: string;
  status?: ScheduledTaskStatus;
  workspaceId?: string;
};

type ScheduledTasksPageProps = {
  agentDefinitions: AgentDefinitionSettings[];
  onOpenChat: (workspaceId: string, chatId: string) => void;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
};

type TaskStatusFilter = "all" | ScheduledTaskStatus;
type ScheduleKind = "one_shot_at" | "interval";
type IntervalUnit = "minutes" | "hours" | "days";
type SessionModeDraft = "create_new_chat" | "reuse_chat";
type TaskFormMode = { type: "create" } | { task: ScheduledTaskView; type: "edit" };

type ScheduledTaskFormState = {
  agentDefinitionId: string;
  collaborationToolsEnabled: boolean;
  concurrencyPolicy: "skip_if_running" | "queue_after_current";
  description: string;
  intervalEvery: string;
  intervalStartAt: string;
  intervalUnit: IntervalUnit;
  misfirePolicy: "skip" | "catch_up_once";
  modelId: string;
  prompt: string;
  providerId: string;
  reuseChatId: string;
  runAt: string;
  scheduleType: ScheduleKind;
  sessionMode: SessionModeDraft;
  status: ScheduledTaskStatus;
  thinkingLevel: string;
  title: string;
  workspaceId: string;
};

const TASK_STATUSES: ScheduledTaskStatus[] = [
  "enabled",
  "paused",
  "completed",
  "archived",
];
const DEFAULT_INTERVAL_SECONDS = 86400;

export async function listScheduledTasks(query: ScheduledTasksQuery = {}) {
  const params = new URLSearchParams();
  if (query.workspaceId) {
    params.set("workspaceId", query.workspaceId);
  }
  if (query.status) {
    params.set("status", query.status);
  }
  if (query.q) {
    params.set("q", query.q);
  }

  const search = params.toString();
  return requestJson<ScheduledTasksResponse>(
    search ? `/api/scheduled-tasks?${search}` : "/api/scheduled-tasks",
  );
}

export function ScheduledTasksPage({
  agentDefinitions,
  onOpenChat,
  settings,
  workspaces,
}: ScheduledTasksPageProps) {
  const { language, t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [operationKey, setOperationKey] = useState<string | null>(null);
  const [tasks, setTasks] = useState<ScheduledTaskView[]>([]);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<TaskStatusFilter>("all");
  const [workspaceFilter, setWorkspaceFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [formMode, setFormMode] = useState<TaskFormMode | null>(null);
  const [runsByTaskId, setRunsByTaskId] = useState<Record<string, ScheduledTaskRunView[]>>({});
  const [runsLoadingTaskId, setRunsLoadingTaskId] = useState<string | null>(null);

  const enabledModels = useMemo(
    () =>
      (settings?.configuredModels ?? []).filter(
        (model) => model.enabled && model.canEnable && model.activeProviderId,
      ),
    [settings?.configuredModels],
  );
  const providers = settings?.providers ?? [];
  const thinkingLevels = settings?.thinkingLevels ?? [];

  const statusCounts = useMemo(() => {
    return tasks.reduce<Record<string, number>>((counts, task) => {
      counts[task.status] = (counts[task.status] ?? 0) + 1;
      return counts;
    }, {});
  }, [tasks]);

  const filteredTasks = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    return tasks.filter((task) => {
      if (statusFilter !== "all" && task.status !== statusFilter) {
        return false;
      }
      if (workspaceFilter !== "all" && task.workspaceId !== workspaceFilter) {
        return false;
      }
      if (!query) {
        return true;
      }
      return [
        task.title,
        task.description ?? "",
        task.workspaceName,
        task.id,
        actionPrompt(task.action),
      ]
        .join(" ")
        .toLowerCase()
        .includes(query);
    });
  }, [searchQuery, statusFilter, tasks, workspaceFilter]);

  const selectedTask =
    filteredTasks.find((task) => task.id === selectedTaskId) ??
    filteredTasks[0] ??
    tasks.find((task) => task.id === selectedTaskId) ??
    null;
  const selectedRuns = selectedTask ? runsByTaskId[selectedTask.id] ?? [] : [];

  const loadTasks = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await listScheduledTasks();
      setTasks(data.tasks);
      setSelectedTaskId((current) =>
        current && data.tasks.some((task) => task.id === current)
          ? current
          : data.tasks[0]?.id ?? null,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadRuns = useCallback(async (task: ScheduledTaskView) => {
    setRunsLoadingTaskId(task.id);
    setError(null);
    try {
      const data = await requestJson<ScheduledTaskRunsResponse>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/runs`,
      );
      setRunsByTaskId((current) => ({ ...current, [task.id]: data.runs }));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setRunsLoadingTaskId(null);
    }
  }, []);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  useEffect(() => {
    if (!selectedTask || runsByTaskId[selectedTask.id]) {
      return;
    }
    void loadRuns(selectedTask);
  }, [loadRuns, runsByTaskId, selectedTask]);

  async function mutateTask(
    key: string,
    task: ScheduledTaskView,
    path: string,
    init: RequestInit = { method: "POST" },
  ) {
    setOperationKey(key);
    setError(null);
    try {
      const data = await requestJson<{ task: ScheduledTaskView }>(path, init);
      setTasks((current) =>
        current.map((item) => (item.id === data.task.id ? data.task : item)),
      );
      setSelectedTaskId(data.task.id);
      return data.task;
    } catch (requestError) {
      setError(errorMessage(requestError));
      return null;
    } finally {
      setOperationKey(null);
    }
  }

  async function runTaskNow(task: ScheduledTaskView) {
    setOperationKey(`run:${task.id}`);
    setError(null);
    try {
      const data = await requestJson<ScheduledTaskRunResponse>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/run-now`,
        { method: "POST" },
      );
      setRunsByTaskId((current) => ({
        ...current,
        [task.id]: [data.run, ...(current[task.id] ?? [])],
      }));
      await loadTasks();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setOperationKey(null);
    }
  }

  async function duplicateTask(task: ScheduledTaskView) {
    setOperationKey(`duplicate:${task.id}`);
    setError(null);
    try {
      const data = await requestJson<{ task: ScheduledTaskView }>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/duplicate`,
        { method: "POST" },
      );
      setTasks((current) => [data.task, ...current]);
      setSelectedTaskId(data.task.id);
      setRunsByTaskId((current) => ({ ...current, [data.task.id]: [] }));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setOperationKey(null);
    }
  }

  async function deleteTask(task: ScheduledTaskView) {
    if (!window.confirm(t("Delete scheduled task?"))) {
      return;
    }
    setOperationKey(`delete:${task.id}`);
    setError(null);
    try {
      await requestJson<{ task: ScheduledTaskView }>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}`,
        { method: "DELETE" },
      );
      setTasks((current) => current.filter((item) => item.id !== task.id));
      setRunsByTaskId((current) => {
        const next = { ...current };
        delete next[task.id];
        return next;
      });
      setSelectedTaskId((current) => (current === task.id ? null : current));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setOperationKey(null);
    }
  }

  async function handleTaskSaved(task: ScheduledTaskView) {
    setTasks((current) => {
      const exists = current.some((item) => item.id === task.id);
      return exists
        ? current.map((item) => (item.id === task.id ? task : item))
        : [task, ...current];
    });
    setSelectedTaskId(task.id);
    setRunsByTaskId((current) => {
      const next = { ...current };
      delete next[task.id];
      return next;
    });
    setFormMode(null);
  }

  return (
    <div className="panel-scroll h-full min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="flex w-full min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-stone-200 bg-white/85 px-4 py-4 shadow-sm">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className="inline-flex size-10 items-center justify-center rounded-lg bg-amber-50 text-amber-800">
                <CalendarClock aria-hidden="true" className="size-5" />
              </span>
              <div className="min-w-0">
                <h2 className="truncate text-lg font-semibold text-stone-950">
                  {t("Scheduled tasks")}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {t("tasks {count}", { count: tasks.length })}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                className="inline-flex h-10 items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 text-sm font-semibold text-amber-900 shadow-sm hover:bg-amber-100"
                onClick={() => setFormMode({ type: "create" })}
                type="button"
              >
                <Plus aria-hidden="true" className="size-4" />
                {t("New task")}
              </button>
              <button
                aria-label={t("Refresh scheduled tasks")}
                className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-amber-200 hover:bg-amber-50 hover:text-amber-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                disabled={isLoading}
                onClick={() => void loadTasks()}
                title={t("Refresh scheduled tasks")}
                type="button"
              >
                {isLoading ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>
          </div>
        </section>

        <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {TASK_STATUSES.map((status) => (
            <button
              className={`rounded-lg border px-4 py-3 text-left shadow-sm transition ${statusFilter === status
                  ? "border-amber-300 bg-amber-50"
                  : "border-stone-200 bg-white/85 hover:border-amber-200 hover:bg-amber-50/60"
                }`}
              key={status}
              onClick={() => setStatusFilter(statusFilter === status ? "all" : status)}
              type="button"
            >
              <div className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                {statusLabel(status, t)}
              </div>
              <div className="mt-2 text-2xl font-semibold text-stone-950">
                {formatNumber(statusCounts[status] ?? 0, language)}
              </div>
            </button>
          ))}
        </section>

        {error ? (
          <div className="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        <section className="grid min-h-[520px] min-w-0 gap-4 xl:grid-cols-[minmax(0,0.95fr)_minmax(380px,1.05fr)]">
          <div className="min-w-0 overflow-hidden rounded-lg border border-stone-200 bg-white/85 shadow-sm">
            <div className="flex flex-wrap items-center gap-3 border-b border-stone-200 px-4 py-3">
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-semibold text-stone-950">
                  {t("Task list")}
                </h3>
                <p className="mt-1 text-xs text-stone-500">
                  {isLoading
                    ? t("Loading...")
                    : t("tasks {count}", { count: filteredTasks.length })}
                </p>
              </div>
              <label className="relative min-w-48 flex-1 sm:max-w-64">
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-stone-400"
                />
                <input
                  aria-label={t("Search scheduled tasks")}
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white pl-9 pr-3 text-sm outline-none focus:border-amber-700 focus:ring-2 focus:ring-amber-100"
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder={t("Search")}
                  value={searchQuery}
                />
              </label>
              <select
                aria-label={t("Filter scheduled tasks by workspace")}
                className="h-10 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none focus:border-amber-700 focus:ring-2 focus:ring-amber-100"
                onChange={(event) => setWorkspaceFilter(event.target.value)}
                value={workspaceFilter}
              >
                <option value="all">{t("All workspaces")}</option>
                {workspaces.map((workspace) => (
                  <option key={workspace.id} value={workspace.id}>
                    {workspace.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="panel-scroll max-h-[640px] min-w-0 overflow-y-auto">
              {filteredTasks.length ? (
                <div className="divide-y divide-stone-100">
                  {filteredTasks.map((task) => (
                    <button
                      className={`grid w-full grid-cols-[minmax(0,1fr)_auto] gap-3 px-4 py-3 text-left transition ${selectedTask?.id === task.id ? "bg-amber-50" : "hover:bg-amber-50/40"
                        }`}
                      key={task.id}
                      onClick={() => setSelectedTaskId(task.id)}
                      type="button"
                    >
                      <span className="min-w-0">
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="truncate font-semibold text-stone-950">
                            {task.title}
                          </span>
                          <span
                            className={`inline-flex shrink-0 rounded-full px-2 py-0.5 text-[0.68rem] font-semibold ring-1 ${statusClass(task.status)}`}
                          >
                            {statusLabel(task.status, t)}
                          </span>
                        </span>
                        <span className="mt-1 block truncate text-xs text-stone-500">
                          {task.workspaceName} / {scheduleSummary(task.schedule, t)}
                        </span>
                        <span className="mt-2 block truncate text-xs text-stone-600">
                          {actionSummary(task.action, t)}
                        </span>
                      </span>
                      <span className="whitespace-nowrap text-right text-xs text-stone-500">
                        <span className="block font-semibold text-stone-700">
                          {t("Next run")}
                        </span>
                        <span className="mt-1 block">
                          {formatTimestamp(task.nextRunAt, language, t)}
                        </span>
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="px-4 py-12 text-center text-sm text-stone-500">
                  {isLoading ? t("Loading...") : t("No scheduled tasks")}
                </div>
              )}
            </div>
          </div>

          <TaskDetails
            isLoadingRuns={runsLoadingTaskId === selectedTask?.id}
            language={language}
            onArchive={(task) =>
              void mutateTask(
                `archive:${task.id}`,
                task,
                `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/archive`,
              )
            }
            onDelete={(task) => void deleteTask(task)}
            onDuplicate={(task) => void duplicateTask(task)}
            onEdit={(task) => setFormMode({ task, type: "edit" })}
            onOpenChat={onOpenChat}
            onPause={(task) =>
              void mutateTask(
                `pause:${task.id}`,
                task,
                `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/pause`,
              )
            }
            onRefreshRuns={(task) => void loadRuns(task)}
            onResume={(task) =>
              void mutateTask(
                `resume:${task.id}`,
                task,
                `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/resume`,
              )
            }
            onRunNow={(task) => void runTaskNow(task)}
            operationKey={operationKey}
            runs={selectedRuns}
            task={selectedTask}
            t={t}
          />
        </section>
      </div>

      {formMode ? (
        <ScheduledTaskDrawer
          agentDefinitions={agentDefinitions}
          enabledModels={enabledModels}
          language={language}
          mode={formMode}
          onClose={() => setFormMode(null)}
          onSaved={(task) => void handleTaskSaved(task)}
          providers={providers}
          t={t}
          thinkingLevels={thinkingLevels}
          workspaces={workspaces}
        />
      ) : null}
    </div>
  );
}

function TaskDetails({
  isLoadingRuns,
  language,
  onArchive,
  onDelete,
  onDuplicate,
  onEdit,
  onOpenChat,
  onPause,
  onRefreshRuns,
  onResume,
  onRunNow,
  operationKey,
  runs,
  task,
  t,
}: {
  isLoadingRuns: boolean;
  language: string;
  onArchive: (task: ScheduledTaskView) => void;
  onDelete: (task: ScheduledTaskView) => void;
  onDuplicate: (task: ScheduledTaskView) => void;
  onEdit: (task: ScheduledTaskView) => void;
  onOpenChat: (workspaceId: string, chatId: string) => void;
  onPause: (task: ScheduledTaskView) => void;
  onRefreshRuns: (task: ScheduledTaskView) => void;
  onResume: (task: ScheduledTaskView) => void;
  onRunNow: (task: ScheduledTaskView) => void;
  operationKey: string | null;
  runs: ScheduledTaskRunView[];
  task: ScheduledTaskView | null;
  t: Translate;
}) {
  if (!task) {
    return (
      <div className="grid min-h-[360px] place-items-center rounded-lg border border-stone-200 bg-white/85 px-4 text-sm text-stone-500 shadow-sm">
        {t("Select a scheduled task")}
      </div>
    );
  }

  const action = recordValue(task.action);
  const metadata = recordValue(task.metadata);
  const modelId = stringField(action, "model_id", "modelId");
  const providerId = stringField(action, "provider_id", "providerId");
  const agentDefinitionId = stringField(action, "agent_definition_id", "agentDefinitionId");
  const thinkingLevel = stringField(action, "thinking_level", "thinkingLevel");
  const usage = task.usage;

  return (
    <div className="min-w-0 overflow-hidden rounded-lg border border-stone-200 bg-white/85 shadow-sm">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-stone-200 px-4 py-4">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate text-base font-semibold text-stone-950">
              {task.title}
            </h3>
            <span
              className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ring-1 ${statusClass(task.status)}`}
            >
              {statusLabel(task.status, t)}
            </span>
          </div>
          <p className="mt-1 text-xs text-stone-500">
            {task.workspaceName} / {task.id}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <IconButton
            busy={operationKey === `run:${task.id}`}
            icon={Play}
            label={t("Run task now")}
            onClick={() => onRunNow(task)}
          />
          {task.status === "paused" ? (
            <IconButton
              busy={operationKey === `resume:${task.id}`}
              icon={Play}
              label={t("Resume task")}
              onClick={() => onResume(task)}
            />
          ) : (
            <IconButton
              busy={operationKey === `pause:${task.id}`}
              disabled={task.status === "archived"}
              icon={Pause}
              label={t("Pause task")}
              onClick={() => onPause(task)}
            />
          )}
          <IconButton
            icon={Pencil}
            label={t("Edit task")}
            onClick={() => onEdit(task)}
          />
          <IconButton
            busy={operationKey === `duplicate:${task.id}`}
            icon={Copy}
            label={t("Duplicate task")}
            onClick={() => onDuplicate(task)}
          />
          <IconButton
            busy={operationKey === `archive:${task.id}`}
            disabled={task.status === "archived"}
            icon={Archive}
            label={t("Archive task")}
            onClick={() => onArchive(task)}
          />
          <IconButton
            busy={operationKey === `delete:${task.id}`}
            icon={Trash2}
            label={t("Delete task")}
            onClick={() => onDelete(task)}
          />
        </div>
      </div>

      <div className="grid gap-4 px-4 py-4 lg:grid-cols-2">
        <DetailBlock title={t("Schedule")}>
          <KeyValue label={t("Schedule")} value={scheduleSummary(task.schedule, t)} />
          <KeyValue
            label={t("Next run")}
            value={formatTimestamp(task.nextRunAt, language, t)}
          />
          <KeyValue
            label={t("Last run")}
            value={formatTimestamp(task.lastRunAt, language, t)}
          />
          <KeyValue
            label={t("Concurrency")}
            value={policyLabel(
              stringField(metadata, "concurrencyPolicy", "concurrency_policy") ??
              "skip_if_running",
              t,
            )}
          />
          <KeyValue
            label={t("Misfire")}
            value={policyLabel(
              stringField(metadata, "misfirePolicy", "misfire_policy") ??
              "catch_up_once",
              t,
            )}
          />
        </DetailBlock>

        <DetailBlock title={t("Action")}>
          <KeyValue label={t("Agent")} value={agentDefinitionId ?? t("None")} />
          <KeyValue label={t("Model")} value={modelId ?? t("Model default")} />
          <KeyValue label={t("Provider")} value={providerId ?? t("Model default")} />
          <KeyValue label={t("Thinking level")} value={thinkingLevel ?? t("None")} />
          <KeyValue
            label={t("Collaboration tools")}
            value={booleanField(action, "collaboration_tools_enabled", "collaborationToolsEnabled")
              ? t("Enabled")
              : t("Disabled")}
          />
        </DetailBlock>

        <DetailBlock title={t("Usage")}>
          <KeyValue
            label={t("Recorded requests")}
            value={formatNumber(usage.totalRequests, language)}
          />
          <KeyValue
            label={t("Failed requests")}
            value={formatNumber(usage.failedRequests, language)}
          />
          <KeyValue
            label={t("Total tokens")}
            value={formatNumber(usage.totalTokens, language)}
          />
          <KeyValue
            label={t("Input tokens")}
            value={formatNumber(usage.totalInputTokens, language)}
          />
          <KeyValue
            label={t("Output tokens")}
            value={formatNumber(usage.totalOutputTokens, language)}
          />
          <KeyValue
            label={t("Total time")}
            value={formatLatencyMs(usage.totalLatencyMs, language, t)}
          />
          <KeyValue
            label={t("Average latency")}
            value={formatLatencyMs(usage.averageLatencyMs, language, t)}
          />
        </DetailBlock>
      </div>

      <div className="border-t border-stone-200 px-4 py-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div>
            <h4 className="text-sm font-semibold text-stone-950">{t("Prompt")}</h4>
            <p className="mt-1 text-xs text-stone-500">{t("Agent prompt")}</p>
          </div>
        </div>
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-stone-200 bg-stone-50 px-3 py-3 text-sm text-stone-700">
          {actionSummary(task.action, t)}
        </pre>
      </div>

      <div className="border-t border-stone-200 px-4 py-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <div>
            <h4 className="text-sm font-semibold text-stone-950">{t("Run history")}</h4>
            <p className="mt-1 text-xs text-stone-500">
              {t("runs {count}", { count: runs.length })}
            </p>
          </div>
          <IconButton
            busy={isLoadingRuns}
            icon={RefreshCw}
            label={t("Refresh runs")}
            onClick={() => onRefreshRuns(task)}
          />
        </div>
        <div className="panel-scroll overflow-x-auto">
          <table className="w-full min-w-[680px] text-left text-sm">
            <thead className="border-y border-stone-200 bg-white text-xs font-semibold text-stone-500">
              <tr>
                <th className="px-3 py-2">{t("Scheduled time")}</th>
                <th className="px-3 py-2">{t("Trigger")}</th>
                <th className="px-3 py-2">{t("Status")}</th>
                <th className="px-3 py-2">{t("Completed")}</th>
                <th className="px-3 py-2">{t("Error")}</th>
                <th className="px-3 py-2 text-right">{t("Chat")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-stone-100">
              {runs.length ? (
                runs.map((run) => (
                  <tr className="align-top" key={run.id}>
                    <td className="whitespace-nowrap px-3 py-2 text-stone-700">
                      {formatTimestamp(run.scheduledAt, language, t)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-stone-700">
                      {triggerLabel(run.triggerReason, t)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2">
                      <span
                        className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ring-1 ${runStatusClass(run.status)}`}
                      >
                        {runStatusLabel(run.status, t)}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-stone-700">
                      {formatTimestamp(run.completedAt, language, t)}
                    </td>
                    <td className="max-w-64 px-3 py-2 text-stone-600">
                      <span className="line-clamp-2">{run.errorMessage ?? ""}</span>
                    </td>
                    <td className="px-3 py-2 text-right">
                      {run.chatId ? (
                        <button
                          className="inline-flex h-8 items-center gap-1 rounded-lg border border-stone-200 bg-white px-2 text-xs font-semibold text-stone-700 hover:border-amber-200 hover:bg-amber-50 hover:text-amber-800"
                          onClick={() => onOpenChat(run.workspaceId, run.chatId!)}
                          type="button"
                        >
                          <ExternalLink aria-hidden="true" className="size-3.5" />
                          {t("Open chat")}
                        </button>
                      ) : (
                        <span className="text-xs text-stone-400">{t("Not available")}</span>
                      )}
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td className="px-3 py-8 text-center text-sm text-stone-500" colSpan={6}>
                    {isLoadingRuns ? t("Loading...") : t("No runs")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function ScheduledTaskDrawer({
  agentDefinitions,
  enabledModels,
  language,
  mode,
  onClose,
  onSaved,
  providers,
  t,
  thinkingLevels,
  workspaces,
}: {
  agentDefinitions: AgentDefinitionSettings[];
  enabledModels: ConfiguredModelSummary[];
  language: string;
  mode: TaskFormMode;
  onClose: () => void;
  onSaved: (task: ScheduledTaskView) => void;
  providers: ConfiguredProviderSummary[];
  t: Translate;
  thinkingLevels: SettingsResponse["thinkingLevels"];
  workspaces: WorkspaceSummary[];
}) {
  const [form, setForm] = useState<ScheduledTaskFormState>(() =>
    taskFormDefaults(mode, workspaces, enabledModels),
  );
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewRuns, setPreviewRuns] = useState<string[]>([]);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  const selectedModel = enabledModels.find((model) => model.id === form.modelId) ?? null;
  const selectableProviders = selectedModel
    ? providers.filter((provider) => selectedModel.providerIds.includes(provider.id))
    : [];

  function updateModel(modelId: string) {
    const nextModel = enabledModels.find((model) => model.id === modelId) ?? null;
    setForm((current) => ({
      ...current,
      modelId,
      providerId: nextModel?.activeProviderId ?? nextModel?.providerIds[0] ?? "",
      thinkingLevel: nextModel?.thinkingLevel ?? current.thinkingLevel,
    }));
  }

  function updateAgentDefinition(agentDefinitionId: string) {
    const definition =
      agentDefinitions.find((agentDefinition) => agentDefinition.id === agentDefinitionId) ??
      null;
    setForm((current) => ({
      ...current,
      agentDefinitionId,
      modelId: definition ? "" : current.modelId,
      providerId: definition ? "" : current.providerId,
      thinkingLevel:
        definition?.modelOptions.thinkingLevel ?? (definition ? "" : current.thinkingLevel),
    }));
  }

  useEffect(() => {
    let schedule: ScheduledTaskSchedule;
    try {
      schedule = scheduleFromForm(form);
    } catch {
      setPreviewError(null);
      setPreviewRuns([]);
      setIsPreviewLoading(false);
      return;
    }

    let cancelled = false;
    setIsPreviewLoading(true);
    const timeout = window.setTimeout(() => {
      void requestJson<ScheduledTaskPreviewNextRunResponse>(
        "/api/scheduled-tasks/preview-next-run",
        {
          body: JSON.stringify({ count: 5, schedule }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      )
        .then((data) => {
          if (!cancelled) {
            setPreviewError(null);
            setPreviewRuns(data.nextRuns);
          }
        })
        .catch((requestError) => {
          if (!cancelled) {
            setPreviewError(errorMessage(requestError));
            setPreviewRuns([]);
          }
        })
        .finally(() => {
          if (!cancelled) {
            setIsPreviewLoading(false);
          }
        });
    }, 250);

    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [
    form.intervalEvery,
    form.intervalStartAt,
    form.intervalUnit,
    form.runAt,
    form.scheduleType,
  ]);

  async function saveTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    let payload: ReturnType<typeof taskFormPayload>;
    try {
      payload = taskFormPayload(form);
    } catch (validationError) {
      setError(errorMessage(validationError));
      return;
    }

    setIsSaving(true);
    try {
      const path =
        mode.type === "edit"
          ? `/api/workspaces/${encodeURIComponent(mode.task.workspaceId)}/scheduled-tasks/${encodeURIComponent(mode.task.id)}`
          : `/api/workspaces/${encodeURIComponent(form.workspaceId)}/scheduled-tasks`;
      const data = await requestJson<{ task: ScheduledTaskView }>(path, {
        body: JSON.stringify(payload),
        headers: { "Content-Type": "application/json" },
        method: mode.type === "edit" ? "PATCH" : "POST",
      });
      onSaved(data.task);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex justify-end bg-stone-950/30">
      <button
        aria-label={t("Close scheduled task editor backdrop")}
        className="absolute inset-0 cursor-default"
        onClick={onClose}
        type="button"
      />
      <form
        aria-label={t("Scheduled task editor")}
        aria-modal="true"
        className="panel-scroll relative h-full w-full max-w-2xl overflow-y-auto bg-white shadow-2xl"
        onSubmit={(event) => void saveTask(event)}
        role="dialog"
      >
        <div className="sticky top-0 z-10 flex items-center justify-between gap-3 border-b border-stone-200 bg-white px-5 py-4">
          <div>
            <h3 className="text-base font-semibold text-stone-950">
              {mode.type === "edit" ? t("Edit scheduled task") : t("New scheduled task")}
            </h3>
          </div>
          <button
            aria-label={t("Close scheduled task editor")}
            className="inline-flex size-9 items-center justify-center rounded-lg text-stone-500 hover:bg-stone-100 hover:text-stone-900"
            onClick={onClose}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        {error ? (
          <div className="mx-5 mt-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        <div className="space-y-5 px-5 py-5">
          <DetailBlock title={t("Task")}>
            <TextField
              label={t("Title")}
              onChange={(title) => setForm((current) => ({ ...current, title }))}
              value={form.title}
            />
            <TextArea
              label={t("Description")}
              onChange={(description) =>
                setForm((current) => ({ ...current, description }))
              }
              rows={2}
              value={form.description}
            />
            <SelectField
              disabled={mode.type === "edit"}
              label={t("Workspace")}
              onChange={(workspaceId) =>
                setForm((current) => ({ ...current, workspaceId }))
              }
              value={form.workspaceId}
            >
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </SelectField>
          </DetailBlock>

          <DetailBlock title={t("Schedule")}>
            <div className="grid gap-3 sm:grid-cols-2">
              <SelectField
                label={t("Schedule type")}
                onChange={(scheduleType) =>
                  setForm((current) => ({
                    ...current,
                    scheduleType: scheduleType as ScheduleKind,
                  }))
                }
                value={form.scheduleType}
              >
                <option value="one_shot_at">{t("One-shot")}</option>
                <option value="interval">{t("Interval")}</option>
              </SelectField>
              <SelectField
                label={t("Status")}
                onChange={(status) =>
                  setForm((current) => ({
                    ...current,
                    status: status as ScheduledTaskStatus,
                  }))
                }
                value={form.status}
              >
                {TASK_STATUSES.map((status) => (
                  <option key={status} value={status}>
                    {statusLabel(status, t)}
                  </option>
                ))}
              </SelectField>
            </div>
            {form.scheduleType === "one_shot_at" ? (
              <TextField
                label={t("Run at")}
                onChange={(runAt) => setForm((current) => ({ ...current, runAt }))}
                type="datetime-local"
                value={form.runAt}
              />
            ) : (
              <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_160px]">
                <TextField
                  inputMode="numeric"
                  label={t("Every")}
                  onChange={(intervalEvery) =>
                    setForm((current) => ({ ...current, intervalEvery }))
                  }
                  value={form.intervalEvery}
                />
                <SelectField
                  label={t("Unit")}
                  onChange={(intervalUnit) =>
                    setForm((current) => ({
                      ...current,
                      intervalUnit: intervalUnit as IntervalUnit,
                    }))
                  }
                  value={form.intervalUnit}
                >
                  <option value="minutes">{t("Minutes")}</option>
                  <option value="hours">{t("Hours")}</option>
                  <option value="days">{t("Days")}</option>
                </SelectField>
                <div className="sm:col-span-2">
                  <TextField
                    label={t("Start at")}
                    onChange={(intervalStartAt) =>
                      setForm((current) => ({ ...current, intervalStartAt }))
                    }
                    type="datetime-local"
                    value={form.intervalStartAt}
                  />
                </div>
              </div>
            )}
            <div className="grid gap-3 sm:grid-cols-2">
              <SelectField
                label={t("Concurrency")}
                onChange={(concurrencyPolicy) =>
                  setForm((current) => ({
                    ...current,
                    concurrencyPolicy:
                      concurrencyPolicy as ScheduledTaskFormState["concurrencyPolicy"],
                  }))
                }
                value={form.concurrencyPolicy}
              >
                <option value="skip_if_running">{t("Skip if running")}</option>
                <option value="queue_after_current">{t("Queue after current")}</option>
              </SelectField>
              <SelectField
                label={t("Misfire")}
                onChange={(misfirePolicy) =>
                  setForm((current) => ({
                    ...current,
                    misfirePolicy:
                      misfirePolicy as ScheduledTaskFormState["misfirePolicy"],
                  }))
                }
                value={form.misfirePolicy}
              >
                <option value="catch_up_once">{t("Catch up once")}</option>
                <option value="skip">{t("Skip")}</option>
              </SelectField>
            </div>
            <RunPreview
              error={previewError}
              isLoading={isPreviewLoading}
              language={language}
              runs={previewRuns}
              t={t}
            />
          </DetailBlock>

          <DetailBlock title={t("Action")}>
            <TextArea
              label={t("Prompt")}
              onChange={(prompt) => setForm((current) => ({ ...current, prompt }))}
              rows={5}
              value={form.prompt}
            />
            <div className="grid gap-3 sm:grid-cols-2">
              <SelectField
                label={t("Session")}
                onChange={(sessionMode) =>
                  setForm((current) => ({
                    ...current,
                    sessionMode: sessionMode as SessionModeDraft,
                  }))
                }
                value={form.sessionMode}
              >
                <option value="create_new_chat">{t("Create new chat")}</option>
                <option value="reuse_chat">{t("Reuse chat")}</option>
              </SelectField>
              {form.sessionMode === "reuse_chat" ? (
                <TextField
                  label={t("Chat id")}
                  onChange={(reuseChatId) =>
                    setForm((current) => ({ ...current, reuseChatId }))
                  }
                  value={form.reuseChatId}
                />
              ) : null}
            </div>
            <SelectField
              label={t("Agent")}
              onChange={updateAgentDefinition}
              value={form.agentDefinitionId}
            >
              <option value="">{t("None")}</option>
              {agentDefinitions.map((definition) => (
                <option key={definition.id} value={definition.id}>
                  {definition.name}
                </option>
              ))}
            </SelectField>
            <div className="grid gap-3 sm:grid-cols-2">
              <SelectField
                label={t("Model")}
                onChange={updateModel}
                value={form.modelId}
              >
                <option value="">{t("Model default")}</option>
                {enabledModels.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName}
                  </option>
                ))}
              </SelectField>
              <SelectField
                disabled={!selectedModel}
                label={t("Provider")}
                onChange={(providerId) =>
                  setForm((current) => ({ ...current, providerId }))
                }
                value={form.providerId}
              >
                <option value="">{t("Model default")}</option>
                {selectableProviders.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.name}
                  </option>
                ))}
              </SelectField>
            </div>
            <SelectField
              label={t("Thinking level")}
              onChange={(thinkingLevel) =>
                setForm((current) => ({ ...current, thinkingLevel }))
              }
              value={form.thinkingLevel}
            >
              <option value="">{t("None")}</option>
              {thinkingLevels.map((level) => (
                <option key={level.value} value={level.value}>
                  {t(level.label)}
                </option>
              ))}
            </SelectField>
            <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
              <span className="text-sm font-semibold text-stone-700">
                {t("Collaboration tools")}
              </span>
              <input
                checked={form.collaborationToolsEnabled}
                className="size-4 accent-amber-700"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    collaborationToolsEnabled: event.target.checked,
                  }))
                }
                type="checkbox"
              />
            </label>
          </DetailBlock>
        </div>

        <div className="sticky bottom-0 flex items-center justify-end gap-2 border-t border-stone-200 bg-white px-5 py-4">
          <button
            className="h-10 rounded-lg border border-stone-200 bg-white px-4 text-sm font-semibold text-stone-700 hover:bg-stone-50"
            onClick={onClose}
            type="button"
          >
            {t("Cancel")}
          </button>
          <button
            className="inline-flex h-10 items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-4 text-sm font-semibold text-amber-900 hover:bg-amber-100 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={isSaving}
            type="submit"
          >
            {isSaving ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Clock3 aria-hidden="true" className="size-4" />
            )}
            {t("Save task")}
          </button>
        </div>
      </form>
    </div>
  );
}

function DetailBlock({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <section className="rounded-lg border border-stone-200 bg-white px-3 py-3">
      <h4 className="mb-3 text-xs font-semibold uppercase tracking-wide text-stone-500">
        {title}
      </h4>
      <div className="space-y-3">{children}</div>
    </section>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 text-sm sm:grid-cols-[132px_minmax(0,1fr)]">
      <dt className="text-xs font-semibold text-stone-500">{label}</dt>
      <dd className="min-w-0 truncate text-stone-800" title={value}>
        {value}
      </dd>
    </div>
  );
}

function IconButton({
  busy = false,
  disabled = false,
  icon: Icon,
  label,
  onClick,
}: {
  busy?: boolean;
  disabled?: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-amber-200 hover:bg-amber-50 hover:text-amber-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
      disabled={busy || disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      {busy ? (
        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
      ) : (
        <Icon aria-hidden="true" className="size-4" />
      )}
    </button>
  );
}

function TextField({
  disabled = false,
  inputMode,
  label,
  onChange,
  type = "text",
  value,
}: {
  disabled?: boolean;
  inputMode?: HTMLAttributes<HTMLInputElement>["inputMode"];
  label: string;
  onChange: (value: string) => void;
  type?: HTMLInputTypeAttribute;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <input
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-amber-700 focus:ring-2 focus:ring-amber-100 disabled:bg-stone-100"
        disabled={disabled}
        inputMode={inputMode}
        onChange={(event) => onChange(event.target.value)}
        type={type}
        value={value}
      />
    </label>
  );
}

function TextArea({
  label,
  onChange,
  rows,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  rows: number;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <textarea
        className="w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition focus:border-amber-700 focus:ring-2 focus:ring-amber-100"
        onChange={(event) => onChange(event.target.value)}
        rows={rows}
        value={value}
      />
    </label>
  );
}

function SelectField({
  children,
  disabled = false,
  label,
  onChange,
  value,
}: {
  children: ReactNode;
  disabled?: boolean;
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
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-amber-700 focus:ring-2 focus:ring-amber-100 disabled:bg-stone-100"
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {children}
      </select>
    </label>
  );
}

function RunPreview({
  error,
  isLoading,
  language,
  runs,
  t,
}: {
  error: string | null;
  isLoading: boolean;
  language: string;
  runs: string[];
  t: Translate;
}) {
  return (
    <div className="border-t border-stone-200 pt-3">
      <div className="text-xs font-semibold uppercase tracking-wide text-stone-500">
        {t("Next five runs")}
      </div>
      {isLoading ? (
        <div className="mt-2 text-sm text-stone-500">{t("Loading...")}</div>
      ) : error ? (
        <div className="mt-2 text-sm text-rose-700">{error}</div>
      ) : runs.length ? (
        <ol className="mt-2 space-y-1 text-sm text-stone-700">
          {runs.map((runAt) => (
            <li key={runAt}>{formatTimestamp(runAt, language, t)}</li>
          ))}
        </ol>
      ) : (
        <div className="mt-2 text-sm text-stone-500">{t("No upcoming runs")}</div>
      )}
    </div>
  );
}

function taskFormDefaults(
  mode: TaskFormMode,
  workspaces: WorkspaceSummary[],
  enabledModels: ConfiguredModelSummary[],
): ScheduledTaskFormState {
  if (mode.type === "edit") {
    return taskFormFromTask(mode.task);
  }

  const model = enabledModels[0] ?? null;
  return {
    agentDefinitionId: "",
    collaborationToolsEnabled: false,
    concurrencyPolicy: "skip_if_running",
    description: "",
    intervalEvery: "1",
    intervalStartAt: "",
    intervalUnit: "days",
    misfirePolicy: "catch_up_once",
    modelId: model?.id ?? "",
    prompt: "",
    providerId: model?.activeProviderId ?? model?.providerIds[0] ?? "",
    reuseChatId: "",
    runAt: dateTimeLocalFromDate(new Date(Date.now() + 60 * 60 * 1000)),
    scheduleType: "interval",
    sessionMode: "create_new_chat",
    status: "enabled",
    thinkingLevel: model?.thinkingLevel ?? "",
    title: "",
    workspaceId: workspaces[0]?.id ?? "",
  };
}

function taskFormFromTask(task: ScheduledTaskView): ScheduledTaskFormState {
  const schedule = recordValue(task.schedule);
  const action = recordValue(task.action);
  const metadata = recordValue(task.metadata);
  const scheduleType =
    stringField(schedule, "type") === "one_shot_at" ? "one_shot_at" : "interval";
  const everySeconds = numberField(schedule, "every_seconds", "everySeconds") ??
    DEFAULT_INTERVAL_SECONDS;
  const interval = intervalDraft(everySeconds);
  const session = action["session_mode"] ?? action["sessionMode"];
  const reuseChatId = reuseChatIdFromSession(session);

  return {
    agentDefinitionId: stringField(action, "agent_definition_id", "agentDefinitionId") ?? "",
    collaborationToolsEnabled:
      booleanField(action, "collaboration_tools_enabled", "collaborationToolsEnabled") ??
      false,
    concurrencyPolicy:
      (stringField(metadata, "concurrencyPolicy", "concurrency_policy") as
        | ScheduledTaskFormState["concurrencyPolicy"]
        | null) ?? "skip_if_running",
    description: task.description ?? "",
    intervalEvery: interval.every,
    intervalStartAt: dateTimeLocalFromString(
      stringField(schedule, "start_at", "startAt"),
    ),
    intervalUnit: interval.unit,
    misfirePolicy:
      (stringField(metadata, "misfirePolicy", "misfire_policy") as
        | ScheduledTaskFormState["misfirePolicy"]
        | null) ?? "catch_up_once",
    modelId: stringField(action, "model_id", "modelId") ?? "",
    prompt: stringField(action, "prompt") ?? "",
    providerId: stringField(action, "provider_id", "providerId") ?? "",
    reuseChatId,
    runAt: dateTimeLocalFromString(stringField(schedule, "run_at", "runAt")),
    scheduleType,
    sessionMode: reuseChatId ? "reuse_chat" : "create_new_chat",
    status: task.status,
    thinkingLevel: stringField(action, "thinking_level", "thinkingLevel") ?? "",
    title: task.title,
    workspaceId: task.workspaceId,
  };
}

function taskFormPayload(form: ScheduledTaskFormState) {
  const title = form.title.trim();
  const prompt = form.prompt.trim();
  if (!title) {
    throw new Error("Title is required.");
  }
  if (!form.workspaceId) {
    throw new Error("Workspace is required.");
  }
  if (!prompt) {
    throw new Error("Prompt is required.");
  }
  if (!form.agentDefinitionId && !form.modelId) {
    throw new Error("Select an agent or model.");
  }

  const schedule = scheduleFromForm(form);
  const action: ScheduledTaskAction = {
    collaboration_tools_enabled: form.collaborationToolsEnabled,
    prompt,
    session_mode:
      form.sessionMode === "reuse_chat"
        ? { reuse_chat: { chat_id: requiredText(form.reuseChatId, "Chat id") } }
        : "create_new_chat",
    skill_ids: [],
    type: "agent_prompt",
    ...(form.agentDefinitionId ? { agent_definition_id: form.agentDefinitionId } : {}),
    ...(form.modelId ? { model_id: form.modelId } : {}),
    ...(form.modelId && form.providerId ? { provider_id: form.providerId } : {}),
    ...(form.thinkingLevel ? { thinking_level: form.thinkingLevel } : {}),
  };

  return {
    action,
    concurrencyPolicy: form.concurrencyPolicy,
    description: form.description.trim() || null,
    misfirePolicy: form.misfirePolicy,
    schedule,
    status: form.status,
    title,
  };
}

function scheduleFromForm(form: ScheduledTaskFormState): ScheduledTaskSchedule {
  if (form.scheduleType === "one_shot_at") {
    return {
      run_at: localDateTimeToIso(requiredText(form.runAt, "Run at")),
      type: "one_shot_at",
    };
  }

  const every = Number.parseInt(form.intervalEvery, 10);
  if (!Number.isSafeInteger(every) || every <= 0) {
    throw new Error("Interval must be a positive whole number.");
  }
  const multiplier =
    form.intervalUnit === "days" ? 86400 : form.intervalUnit === "hours" ? 3600 : 60;
  const schedule: ScheduledTaskSchedule = {
    every_seconds: every * multiplier,
    type: "interval",
  };
  if (form.intervalStartAt) {
    schedule.start_at = localDateTimeToIso(form.intervalStartAt);
  }
  return schedule;
}

function requiredText(value: string, label: string) {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(`${label} is required.`);
  }
  return normalized;
}

function statusLabel(status: string, t: Translate) {
  switch (status) {
    case "enabled":
      return t("Enabled");
    case "paused":
      return t("Paused");
    case "completed":
      return t("Completed");
    case "archived":
      return t("Archived");
    default:
      return status;
  }
}

function statusClass(status: string) {
  switch (status) {
    case "enabled":
      return "bg-emerald-50 text-emerald-700 ring-emerald-100";
    case "paused":
      return "bg-amber-50 text-amber-700 ring-amber-100";
    case "completed":
      return "bg-sky-50 text-sky-700 ring-sky-100";
    case "archived":
      return "bg-stone-100 text-stone-600 ring-stone-200";
    default:
      return "bg-stone-100 text-stone-700 ring-stone-200";
  }
}

function runStatusLabel(status: ScheduledTaskRunStatus | string, t: Translate) {
  switch (status) {
    case "pending":
      return t("Pending");
    case "queued":
      return t("Queued");
    case "running":
      return t("Running");
    case "succeeded":
      return t("Succeeded");
    case "failed":
      return t("Failed");
    case "cancelled":
      return t("Cancelled");
    case "skipped":
      return t("Skipped");
    default:
      return status;
  }
}

function runStatusClass(status: string) {
  switch (status) {
    case "succeeded":
      return "bg-emerald-50 text-emerald-700 ring-emerald-100";
    case "failed":
      return "bg-rose-50 text-rose-700 ring-rose-100";
    case "running":
      return "bg-sky-50 text-sky-700 ring-sky-100";
    case "queued":
    case "pending":
      return "bg-amber-50 text-amber-700 ring-amber-100";
    case "cancelled":
    case "skipped":
      return "bg-stone-100 text-stone-600 ring-stone-200";
    default:
      return "bg-stone-100 text-stone-700 ring-stone-200";
  }
}

function triggerLabel(trigger: string, t: Translate) {
  switch (trigger) {
    case "scheduled":
      return t("Scheduled");
    case "manual":
      return t("Manual");
    case "retry":
      return t("Retry");
    case "misfire_catch_up":
      return t("Catch-up");
    default:
      return trigger;
  }
}

function policyLabel(value: string, t: Translate) {
  switch (value) {
    case "skip_if_running":
      return t("Skip if running");
    case "queue_after_current":
      return t("Queue after current");
    case "catch_up_once":
      return t("Catch up once");
    case "skip":
      return t("Skip");
    default:
      return value;
  }
}

function scheduleSummary(schedule: unknown, t: Translate) {
  const record = recordValue(schedule);
  const type = stringField(record, "type");
  if (!type) {
    return t("Custom schedule");
  }

  if (type === "one_shot_at") {
    return t("One-shot");
  }

  const seconds = numberField(record, "every_seconds", "everySeconds");
  if (type === "interval" && typeof seconds === "number") {
    return t("Every {duration}", {
      duration: formatDurationSeconds(seconds),
    });
  }

  if (type === "cron") {
    return t("Cron");
  }

  return type;
}

function actionSummary(action: unknown, t: Translate) {
  return actionPrompt(action) || t("Agent prompt");
}

function actionPrompt(action: unknown) {
  const record = recordValue(action);
  const prompt = stringField(record, "prompt");
  return prompt?.trim() ?? "";
}

function formatDurationSeconds(seconds: number) {
  if (seconds % 86400 === 0) {
    return `${seconds / 86400}d`;
  }
  if (seconds % 3600 === 0) {
    return `${seconds / 3600}h`;
  }
  if (seconds % 60 === 0) {
    return `${seconds / 60}m`;
  }
  return `${seconds}s`;
}

function formatTimestamp(value: string | null, language: string, t: Translate) {
  if (!value) {
    return t("Not scheduled");
  }

  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(timestamp);
}

function formatNumber(value: number, language: string) {
  return new Intl.NumberFormat(language).format(value);
}

function formatLatencyMs(value: number | null, language: string, t: Translate) {
  if (value === null) {
    return t("Not available");
  }
  if (value >= 1000) {
    return `${new Intl.NumberFormat(language, {
      maximumFractionDigits: 1,
      minimumFractionDigits: 0,
    }).format(value / 1000)}s`;
  }
  return `${formatNumber(value, language)}ms`;
}

function recordValue(value: unknown): Record<string, JsonValue> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, JsonValue>)
    : {};
}

function stringField(record: Record<string, JsonValue>, ...keys: string[]) {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string") {
      return value;
    }
  }
  return null;
}

function numberField(record: Record<string, JsonValue>, ...keys: string[]) {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number") {
      return value;
    }
  }
  return null;
}

function booleanField(record: Record<string, JsonValue>, ...keys: string[]) {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "boolean") {
      return value;
    }
  }
  return null;
}

function reuseChatIdFromSession(value: JsonValue | undefined) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return "";
  }
  const reuseChat = (value as Record<string, JsonValue>).reuse_chat;
  if (!reuseChat || typeof reuseChat !== "object" || Array.isArray(reuseChat)) {
    return "";
  }
  const chatId = (reuseChat as Record<string, JsonValue>).chat_id;
  return typeof chatId === "string" ? chatId : "";
}

function intervalDraft(seconds: number): { every: string; unit: IntervalUnit } {
  if (seconds % 86400 === 0) {
    return { every: String(seconds / 86400), unit: "days" };
  }
  if (seconds % 3600 === 0) {
    return { every: String(seconds / 3600), unit: "hours" };
  }
  if (seconds % 60 === 0) {
    return { every: String(seconds / 60), unit: "minutes" };
  }
  return { every: String(Math.max(1, Math.round(seconds / 60))), unit: "minutes" };
}

function dateTimeLocalFromString(value: string | null) {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return dateTimeLocalFromDate(date);
}

function dateTimeLocalFromDate(date: Date) {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function localDateTimeToIso(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error("Date/time is invalid.");
  }
  return date.toISOString();
}
