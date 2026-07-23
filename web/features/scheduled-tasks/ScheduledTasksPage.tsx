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
import {
  SettingsButton,
  SettingsInput,
  SettingsSelect,
  SettingsTextArea,
} from "../../shared/ui";
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
  thinkingLevelOptionsForModel,
} from "../../shared/thinking-levels";

type ScheduledTasksQuery = {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: ScheduledTaskStatus;
  workspaceId?: string;
};

type ScheduledTaskRunsPageState = {
  page: number;
  pageSize: number;
  totalCount: number;
  totalPages: number;
};

type ScheduledTasksPageProps = {
  agentDefinitions: AgentDefinitionSettings[];
  onOpenChat: (workspaceId: string, chatId: string) => void;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
};

type TaskStatusFilter = "all" | ScheduledTaskStatus;
type ScheduleKind = "one_shot_at" | "interval";
type IntervalUnit = "minutes" | "hours" | "days" | "weeks" | "months";
type SessionModeDraft = "create_new_chat" | "reuse_chat";
type TaskFormMode = { type: "create" } | { task: ScheduledTaskView; type: "edit" };

type ScheduledTaskFormState = {
  agentDefinitionId: string;
  collaborationToolsEnabled: boolean;
  concurrencyPolicy: "skip_if_running" | "queue_after_current" | "force_run";
  description: string;
  intervalEvery: string;
  intervalStartAt: string;
  intervalUnit: IntervalUnit;
  misfirePolicy: "skip" | "catch_up_once";
  modelId: string;
  prompt: string;
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
const FORM_TASK_STATUSES: ScheduledTaskStatus[] = ["enabled", "paused"];
const DEFAULT_TASK_PAGE_SIZE = 25;
const DEFAULT_RUN_PAGE_SIZE = 20;
const TASK_PAGE_SIZE_OPTIONS = [10, 25, 50, 100];
const RUN_PAGE_SIZE_OPTIONS = [10, 20, 50, 100];
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-coordinator";
const DEFAULT_INTERVAL_SECONDS = 86400;
const INTERVAL_UNIT_SECONDS: Record<IntervalUnit, number> = {
  minutes: 60,
  hours: 3600,
  days: 86400,
  weeks: 604800,
  // ponytail: month intervals use fixed 30-day seconds; add calendar units if real month boundaries matter.
  months: 2592000,
};

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
  if (query.page) {
    params.set("page", String(query.page));
  }
  if (query.pageSize) {
    params.set("pageSize", String(query.pageSize));
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
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState("");
  const [taskPage, setTaskPage] = useState(1);
  const [taskPageSize, setTaskPageSize] = useState(DEFAULT_TASK_PAGE_SIZE);
  const [taskTotalCount, setTaskTotalCount] = useState(0);
  const [taskTotalPages, setTaskTotalPages] = useState(0);
  const [statusCounts, setStatusCounts] = useState<Record<string, number>>({});
  const [formMode, setFormMode] = useState<TaskFormMode | null>(null);
  const [runsByTaskId, setRunsByTaskId] = useState<Record<string, ScheduledTaskRunView[]>>({});
  const [runsPageByTaskId, setRunsPageByTaskId] = useState<Record<string, ScheduledTaskRunsPageState>>({});
  const [runsLoadingTaskId, setRunsLoadingTaskId] = useState<string | null>(null);

  const enabledModels = useMemo(
    () =>
      (settings?.configuredModels ?? []).filter(
        (model) => model.enabled && model.canEnable && model.activeProviderId,
      ),
    [settings?.configuredModels],
  );
  const thinkingLevels = settings?.thinkingLevels ?? [];
  // Scheduled tasks run against local workspace SQLite only.
  const localWorkspaces = useMemo(
    () => workspaces.filter((workspace) => !workspace.serverId),
    [workspaces],
  );

  const selectedTask =
    tasks.find((task) => task.id === selectedTaskId) ?? tasks[0] ?? null;
  const selectedRuns = selectedTask ? runsByTaskId[selectedTask.id] ?? [] : [];
  const selectedRunsPage = selectedTask
    ? runsPageByTaskId[selectedTask.id] ?? {
        page: 1,
        pageSize: DEFAULT_RUN_PAGE_SIZE,
        totalCount: 0,
        totalPages: 0,
      }
    : { page: 1, pageSize: DEFAULT_RUN_PAGE_SIZE, totalCount: 0, totalPages: 0 };

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setDebouncedSearchQuery(searchQuery.trim());
      setTaskPage(1);
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchQuery]);

  useEffect(() => {
    if (
      workspaceFilter !== "all" &&
      !localWorkspaces.some((workspace) => workspace.id === workspaceFilter)
    ) {
      setWorkspaceFilter("all");
    }
  }, [localWorkspaces, workspaceFilter]);

  useEffect(() => {
    setTaskPage(1);
  }, [statusFilter, workspaceFilter, taskPageSize]);

  const loadTasks = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await listScheduledTasks({
        page: taskPage,
        pageSize: taskPageSize,
        q: debouncedSearchQuery || undefined,
        status: statusFilter === "all" ? undefined : statusFilter,
        workspaceId: workspaceFilter === "all" ? undefined : workspaceFilter,
      });
      setTasks(data.tasks);
      setTaskTotalCount(data.totalCount);
      setTaskTotalPages(data.totalPages);
      setStatusCounts(data.statusCounts ?? {});
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
  }, [debouncedSearchQuery, statusFilter, taskPage, taskPageSize, workspaceFilter]);

  const loadRuns = useCallback(async (task: ScheduledTaskView, page = 1, pageSize = DEFAULT_RUN_PAGE_SIZE) => {
    setRunsLoadingTaskId(task.id);
    setError(null);
    try {
      const params = new URLSearchParams({
        page: String(page),
        pageSize: String(pageSize),
      });
      const data = await requestJson<ScheduledTaskRunsResponse>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/runs?${params.toString()}`,
      );
      setRunsByTaskId((current) => ({ ...current, [task.id]: data.runs }));
      setRunsPageByTaskId((current) => ({
        ...current,
        [task.id]: {
          page: data.page,
          pageSize: data.pageSize,
          totalCount: data.totalCount,
          totalPages: data.totalPages,
        },
      }));
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
      await loadTasks();
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
      await requestJson<ScheduledTaskRunResponse>(
        `/api/workspaces/${encodeURIComponent(task.workspaceId)}/scheduled-tasks/${encodeURIComponent(task.id)}/run-now`,
        { method: "POST" },
      );
      const pageSize = runsPageByTaskId[task.id]?.pageSize ?? DEFAULT_RUN_PAGE_SIZE;
      await loadRuns(task, 1, pageSize);
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
      setTaskPage(1);
      setSelectedTaskId(data.task.id);
      setRunsByTaskId((current) => ({ ...current, [data.task.id]: [] }));
      await loadTasks();
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
      setRunsByTaskId((current) => {
        const next = { ...current };
        delete next[task.id];
        return next;
      });
      setRunsPageByTaskId((current) => {
        const next = { ...current };
        delete next[task.id];
        return next;
      });
      setSelectedTaskId((current) => (current === task.id ? null : current));
      await loadTasks();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setOperationKey(null);
    }
  }

  async function handleTaskSaved(task: ScheduledTaskView) {
    setSelectedTaskId(task.id);
    setRunsByTaskId((current) => {
      const next = { ...current };
      delete next[task.id];
      return next;
    });
    setRunsPageByTaskId((current) => {
      const next = { ...current };
      delete next[task.id];
      return next;
    });
    setFormMode(null);
    await loadTasks();
  }

  return (
    <div className="panel-scroll h-full min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="flex w-full min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-sm">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className="inline-flex size-10 items-center justify-center rounded-lg bg-[var(--warning-soft)] text-[var(--warning)]">
                <CalendarClock aria-hidden="true" className="size-5" />
              </span>
              <div className="min-w-0">
                <h2 className="truncate text-lg font-semibold text-[var(--foreground)]">
                  {t("Scheduled tasks")}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-[var(--muted)]">
                  {t("tasks {count}", { count: taskTotalCount })}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <SettingsButton
                className="inline-flex h-10 items-center gap-2 rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-3 text-sm font-semibold text-[var(--warning)] shadow-sm hover:bg-[var(--warning-soft)]"
                onClick={() => setFormMode({ type: "create" })}
                type="button"
              >
                <Plus aria-hidden="true" className="size-4" />
                {t("New task")}
              </SettingsButton>
              <SettingsButton
                aria-label={t("Refresh scheduled tasks")}
                className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--warning)] hover:bg-[var(--warning-soft)] hover:text-[var(--warning)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
              </SettingsButton>
            </div>
          </div>
        </section>

        <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {TASK_STATUSES.map((status) => (
            <SettingsButton
              className={`rounded-lg border px-4 py-3 text-left shadow-sm transition ${statusFilter === status
                  ? "border-[var(--warning)] bg-[var(--warning-soft)]"
                  : "border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] hover:border-[var(--warning)] hover:bg-[var(--warning-soft)]"
                }`}
              key={status}
              onClick={() => setStatusFilter(statusFilter === status ? "all" : status)}
              type="button"
            >
              <div className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                {statusLabel(status, t)}
              </div>
              <div className="mt-2 text-2xl font-semibold text-[var(--foreground)]">
                {formatNumber(statusCounts[status] ?? 0, language)}
              </div>
            </SettingsButton>
          ))}
        </section>

        {error ? (
          <div
            role="alert"
            className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--danger)]"
          >
            {error}
          </div>
        ) : null}

        <section className="grid min-h-[520px] min-w-0 gap-4 xl:grid-cols-[minmax(0,0.95fr)_minmax(380px,1.05fr)]">
          <div className="min-w-0 overflow-hidden rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-sm">
            <div className="flex flex-wrap items-center gap-3 border-b border-[var(--border)] px-4 py-3">
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-semibold text-[var(--foreground)]">
                  {t("Task list")}
                </h3>
                <p className="mt-1 text-xs text-[var(--muted)]">
                  {isLoading
                    ? t("Loading…")
                    : t("tasks {count}", { count: taskTotalCount })}
                </p>
              </div>
              <label className="relative min-w-48 flex-1 sm:max-w-64">
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--muted)]"
                />
                <SettingsInput
                  aria-label={t("Search scheduled tasks")}
                  className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] pl-9 pr-3 text-sm outline-none focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)]"
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder={t("Search")}
                  value={searchQuery}
                />
              </label>
              <SettingsSelect
                aria-label={t("Filter scheduled tasks by workspace")}
                className="h-10 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)]"
                onChange={(event) => setWorkspaceFilter(event.target.value)}
                value={workspaceFilter}
              >
                <option value="all">{t("All workspaces")}</option>
                {localWorkspaces.map((workspace) => (
                  <option key={workspace.id} value={workspace.id}>
                    {workspace.name}
                  </option>
                ))}
              </SettingsSelect>
            </div>
            <div className="panel-scroll max-h-[640px] min-w-0 overflow-y-auto">
              {tasks.length ? (
                <div className="divide-y divide-[var(--border)]">
                  {tasks.map((task) => (
                    <SettingsButton
                      className={`grid w-full grid-cols-[minmax(0,1fr)_auto] gap-3 px-4 py-3 text-left transition ${selectedTask?.id === task.id ? "bg-[var(--warning-soft)]" : "hover:bg-[var(--warning-soft)]"
                        }`}
                      key={task.id}
                      onClick={() => setSelectedTaskId(task.id)}
                      type="button"
                    >
                      <span className="min-w-0">
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="truncate font-semibold text-[var(--foreground)]">
                            {task.title}
                          </span>
                          <span
                            className={`inline-flex shrink-0 rounded-full px-2 py-0.5 text-[0.68rem] font-semibold ring-1 ${statusClass(task.status)}`}
                          >
                            {statusLabel(task.status, t)}
                          </span>
                        </span>
                        <span className="mt-1 block truncate text-xs text-[var(--muted)]">
                          {task.workspaceName} / {scheduleSummary(task.schedule, t)}
                        </span>
                        <span className="mt-2 block truncate text-xs text-[var(--muted)]">
                          {actionSummary(task.action, t)}
                        </span>
                      </span>
                      <span className="whitespace-nowrap text-right text-xs text-[var(--muted)]">
                        <span className="block font-semibold text-[var(--muted)]">
                          {t("Next run")}
                        </span>
                        <span className="mt-1 block">
                          {formatTimestamp(task.nextRunAt, language, t)}
                        </span>
                      </span>
                    </SettingsButton>
                  ))}
                </div>
              ) : (
                <div className="px-4 py-12 text-center text-sm text-[var(--muted)]">
                  {isLoading ? t("Loading…") : t("No scheduled tasks")}
                </div>
              )}
            </div>
            <PaginationControls
              language={language}
              onPageChange={setTaskPage}
              onPageSizeChange={(nextPageSize) => setTaskPageSize(nextPageSize)}
              page={taskPage}
              pageSize={taskPageSize}
              pageSizeOptions={TASK_PAGE_SIZE_OPTIONS}
              t={t}
              totalCount={taskTotalCount}
              totalPages={taskTotalPages}
            />
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
            onRefreshRuns={(task) =>
              void loadRuns(
                task,
                runsPageByTaskId[task.id]?.page ?? 1,
                runsPageByTaskId[task.id]?.pageSize ?? DEFAULT_RUN_PAGE_SIZE,
              )
            }
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
            runsPage={selectedRunsPage}
            onRunsPageChange={(page) => selectedTask && void loadRuns(selectedTask, page, selectedRunsPage.pageSize)}
            onRunsPageSizeChange={(pageSize) => selectedTask && void loadRuns(selectedTask, 1, pageSize)}
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
          t={t}
          thinkingLevels={thinkingLevels}
          workspaces={localWorkspaces}
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
  runsPage,
  onRunsPageChange,
  onRunsPageSizeChange,
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
  runsPage: ScheduledTaskRunsPageState;
  onRunsPageChange: (page: number) => void;
  onRunsPageSizeChange: (pageSize: number) => void;
  task: ScheduledTaskView | null;
  t: Translate;
}) {
  if (!task) {
    return (
      <div className="grid min-h-[360px] place-items-center rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 text-sm text-[var(--muted)] shadow-sm">
        {t("Select a scheduled task")}
      </div>
    );
  }

  const action = recordValue(task.action);
  const metadata = recordValue(task.metadata);
  const modelId = stringField(action, "model_id", "modelId");
  const agentDefinitionId = stringField(action, "agent_definition_id", "agentDefinitionId");
  const thinkingLevel = stringField(action, "thinking_level", "thinkingLevel");
  const usage = task.usage;

  return (
    <div className="min-w-0 overflow-hidden rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-sm">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--border)] px-4 py-4">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate text-base font-semibold text-[var(--foreground)]">
              {task.title}
            </h3>
            <span
              className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ring-1 ${statusClass(task.status)}`}
            >
              {statusLabel(task.status, t)}
            </span>
          </div>
          <p className="mt-1 text-xs text-[var(--muted)]">
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
          <KeyValue label={t("Thinking level")} value={thinkingLevel ?? t("None")} />
          <KeyValue
            label={t("Team mode")}
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

      <div className="border-t border-[var(--border)] px-4 py-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div>
            <h4 className="text-sm font-semibold text-[var(--foreground)]">{t("Prompt")}</h4>
            <p className="mt-1 text-xs text-[var(--muted)]">{t("Agent prompt")}</p>
          </div>
        </div>
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-sm text-[var(--muted)]">
          {actionSummary(task.action, t)}
        </pre>
      </div>

      <div className="border-t border-[var(--border)] px-4 py-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <div>
            <h4 className="text-sm font-semibold text-[var(--foreground)]">{t("Run history")}</h4>
            <p className="mt-1 text-xs text-[var(--muted)]">
              {t("runs {count}", { count: runsPage.totalCount })}
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
            <thead className="border-y border-[var(--border)] bg-[var(--surface)] text-xs font-semibold text-[var(--muted)]">
              <tr>
                <th className="px-3 py-2">{t("Scheduled time")}</th>
                <th className="px-3 py-2">{t("Trigger")}</th>
                <th className="px-3 py-2">{t("Status")}</th>
                <th className="px-3 py-2">{t("Completed")}</th>
                <th className="px-3 py-2">{t("Error")}</th>
                <th className="px-3 py-2 text-right">{t("Chat")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--border)]">
              {runs.length ? (
                runs.map((run) => (
                  <tr className="align-top" key={run.id}>
                    <td className="whitespace-nowrap px-3 py-2 text-[var(--muted)]">
                      {formatTimestamp(run.scheduledAt, language, t)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-[var(--muted)]">
                      {triggerLabel(run.triggerReason, t)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2">
                      <span
                        className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ring-1 ${runStatusClass(run.status)}`}
                      >
                        {runStatusLabel(run.status, t)}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-[var(--muted)]">
                      {formatTimestamp(run.completedAt, language, t)}
                    </td>
                    <td className="max-w-64 px-3 py-2 text-[var(--muted)]">
                      <span className="line-clamp-2">{run.errorMessage ?? ""}</span>
                    </td>
                    <td className="px-3 py-2 text-right">
                      {run.chatId ? (
                        <SettingsButton
                          className="inline-flex h-8 items-center gap-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] hover:border-[var(--warning)] hover:bg-[var(--warning-soft)] hover:text-[var(--warning)]"
                          onClick={() => onOpenChat(run.workspaceId, run.chatId!)}
                          type="button"
                        >
                          <ExternalLink aria-hidden="true" className="size-3.5" />
                          {t("Open chat")}
                        </SettingsButton>
                      ) : (
                        <span className="text-xs text-[var(--muted)]">{t("Not available")}</span>
                      )}
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td className="px-3 py-8 text-center text-sm text-[var(--muted)]" colSpan={6}>
                    {isLoadingRuns ? t("Loading…") : t("No runs")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        <PaginationControls
          language={language}
          onPageChange={onRunsPageChange}
          onPageSizeChange={onRunsPageSizeChange}
          page={runsPage.page}
          pageSize={runsPage.pageSize}
          pageSizeOptions={RUN_PAGE_SIZE_OPTIONS}
          t={t}
          totalCount={runsPage.totalCount}
          totalPages={runsPage.totalPages}
        />
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
  t: Translate;
  thinkingLevels: SettingsResponse["thinkingLevels"];
  workspaces: WorkspaceSummary[];
}) {
  const [form, setForm] = useState<ScheduledTaskFormState>(() =>
    taskFormDefaults(mode, workspaces, enabledModels, agentDefinitions),
  );
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewRuns, setPreviewRuns] = useState<string[]>([]);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  useEffect(() => {
    if (mode.type !== "create" || agentDefinitions.length === 0) {
      return;
    }

    setForm((current) => {
      if (current.agentDefinitionId) {
        return current;
      }

      const defaults = taskFormDefaults(mode, workspaces, enabledModels, agentDefinitions);
      return {
        ...current,
        agentDefinitionId: defaults.agentDefinitionId,
        modelId: defaults.modelId,
        thinkingLevel: defaults.thinkingLevel,
      };
    });
  }, [agentDefinitions, enabledModels, mode, workspaces]);

  const selectedModel = enabledModels.find((model) => model.id === form.modelId) ?? null;
  const thinkingOptions = useMemo(
    () => thinkingLevelOptionsForModel(selectedModel, thinkingLevels),
    [selectedModel, thinkingLevels],
  );

  function updateModel(modelId: string) {
    const nextModel = enabledModels.find((model) => model.id === modelId) ?? null;
    setForm((current) => ({
      ...current,
      modelId,
      thinkingLevel: defaultThinkingLevelForModel(nextModel),
    }));
  }

  function updateAgentDefinition(agentDefinitionId: string) {
    const definition =
      agentDefinitions.find((agentDefinition) => agentDefinition.id === agentDefinitionId) ??
      null;
    const model =
      enabledModels.find((candidate) => candidate.id === definition?.modelId) ?? null;
    setForm((current) => ({
      ...current,
      agentDefinitionId,
      modelId: definition?.modelId ?? current.modelId,
      thinkingLevel: isModelThinkingLevelSupported(
        model,
        definition?.modelOptions.thinkingLevel,
      )
        ? definition!.modelOptions.thinkingLevel!
        : definition
          ? ""
          : current.thinkingLevel,
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
      payload = taskFormPayload(form, mode, t, enabledModels);
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
    <div className="fixed inset-0 z-50 flex justify-end bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)]">
      <SettingsButton
        aria-label={t("Close scheduled task editor backdrop")}
        className="absolute inset-0 cursor-default"
        onClick={onClose}
        type="button"
      />
      <form
        aria-label={t("Scheduled task editor")}
        aria-modal="true"
        className="panel-scroll relative h-full w-full max-w-2xl overflow-y-auto bg-[var(--surface)] shadow-2xl"
        onSubmit={(event) => void saveTask(event)}
        role="dialog"
      >
        <div className="sticky top-0 z-10 flex items-center justify-between gap-3 border-b border-[var(--border)] bg-[var(--surface)] px-5 py-4">
          <div>
            <h3 className="text-base font-semibold text-[var(--foreground)]">
              {mode.type === "edit" ? t("Edit scheduled task") : t("New scheduled task")}
            </h3>
          </div>
          <SettingsButton
            aria-label={t("Close scheduled task editor")}
            className="inline-flex size-9 items-center justify-center rounded-lg text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)]"
            onClick={onClose}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </SettingsButton>
        </div>

        {error ? (
          <div className="mx-5 mt-4 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
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
                disabled={!isFormTaskStatus(form.status)}
                label={t("Status")}
                onChange={(status) =>
                  setForm((current) => ({
                    ...current,
                    status: status as ScheduledTaskStatus,
                  }))
                }
                value={form.status}
              >
                {(isFormTaskStatus(form.status)
                  ? FORM_TASK_STATUSES
                  : [form.status]
                ).map((status) => (
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
                  <option value="weeks">{t("Weeks")}</option>
                  <option value="months">{t("Months")}</option>
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
                <option value="force_run">{t("Force run")}</option>
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
                label={t("Thinking level")}
                onChange={(thinkingLevel) =>
                  setForm((current) => ({ ...current, thinkingLevel }))
                }
                value={form.thinkingLevel}
              >
                <option value="">{t("None")}</option>
                {thinkingOptions.map((level) => (
                  <option key={level.value} value={level.value}>
                    {t(level.label)}
                  </option>
                ))}
              </SelectField>
            </div>
            <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
              <span className="text-sm font-semibold text-[var(--muted)]">
                {t("Enable Team mode")}
              </span>
              <SettingsInput
                checked={form.collaborationToolsEnabled}
                className="size-4 accent-[var(--warning)]"
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

        <div className="sticky bottom-0 flex items-center justify-end gap-2 border-t border-[var(--border)] bg-[var(--surface)] px-5 py-4">
          <SettingsButton
            className="h-10 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-semibold text-[var(--muted)] hover:bg-[var(--surface-secondary)]"
            onClick={onClose}
            type="button"
          >
            {t("Cancel")}
          </SettingsButton>
          <SettingsButton
            className="inline-flex h-10 items-center gap-2 rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-4 text-sm font-semibold text-[var(--warning)] hover:bg-[var(--warning-soft)] disabled:cursor-not-allowed disabled:opacity-60"
            disabled={isSaving}
            type="submit"
          >
            {isSaving ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Clock3 aria-hidden="true" className="size-4" />
            )}
            {t("Save task")}
          </SettingsButton>
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
    <section className="rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-3">
      <h4 className="mb-3 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
        {title}
      </h4>
      <div className="space-y-3">{children}</div>
    </section>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 text-sm sm:grid-cols-[132px_minmax(0,1fr)]">
      <dt className="text-xs font-semibold text-[var(--muted)]">{label}</dt>
      <dd className="min-w-0 truncate text-[var(--foreground)]" title={value}>
        {value}
      </dd>
    </div>
  );
}

function PaginationControls({
  language,
  onPageChange,
  onPageSizeChange,
  page,
  pageSize,
  pageSizeOptions,
  t,
  totalCount,
  totalPages,
}: {
  language: string;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
  page: number;
  pageSize: number;
  pageSizeOptions: number[];
  t: Translate;
  totalCount: number;
  totalPages: number;
}) {
  const start = totalCount === 0 ? 0 : (page - 1) * pageSize + 1;
  const end = Math.min(totalCount, page * pageSize);
  const effectiveTotalPages = Math.max(totalPages, totalCount === 0 ? 0 : 1);

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] px-4 py-3 text-xs text-[var(--muted)]">
      <div>
        {t("Showing {start}-{end} of {total}", {
          start: formatNumber(start, language),
          end: formatNumber(end, language),
          total: formatNumber(totalCount, language),
        })}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <label className="inline-flex items-center gap-2">
          <span>{t("Page size")}</span>
          <SettingsSelect
            className="h-8 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs text-[var(--foreground)] outline-none focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)]"
            onChange={(event) => onPageSizeChange(Number(event.target.value))}
            value={pageSize}
          >
            {pageSizeOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </SettingsSelect>
        </label>
        <span>
          {t("Page {page} of {totalPages}", {
            page: formatNumber(totalCount === 0 ? 0 : page, language),
            totalPages: formatNumber(effectiveTotalPages, language),
          })}
        </span>
        <SettingsButton
          aria-label={t("Previous page")}
          className="inline-flex h-8 items-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 font-semibold text-[var(--muted)] hover:border-[var(--warning)] hover:bg-[var(--warning-soft)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
          disabled={page <= 1}
          onClick={() => onPageChange(Math.max(1, page - 1))}
          type="button"
        >
          {t("Previous page")}
        </SettingsButton>
        <SettingsButton
          aria-label={t("Next page")}
          className="inline-flex h-8 items-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 font-semibold text-[var(--muted)] hover:border-[var(--warning)] hover:bg-[var(--warning-soft)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
          disabled={totalPages === 0 || page >= totalPages}
          onClick={() => onPageChange(page + 1)}
          type="button"
        >
          {t("Next page")}
        </SettingsButton>
      </div>
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
    <SettingsButton
      aria-label={label}
      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--warning)] hover:bg-[var(--warning-soft)] hover:text-[var(--warning)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
    </SettingsButton>
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
      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
        {label}
      </span>
      <SettingsInput
        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)] disabled:bg-[var(--surface-secondary)]"
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
      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
        {label}
      </span>
      <SettingsTextArea
        className="w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)]"
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
      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
        {label}
      </span>
      <SettingsSelect
        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--warning)] focus:ring-2 focus:ring-[var(--warning)] disabled:bg-[var(--surface-secondary)]"
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {children}
      </SettingsSelect>
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
    <div className="border-t border-[var(--border)] pt-3">
      <div className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
        {t("Next five runs")}
      </div>
      {isLoading ? (
        <div className="mt-2 text-sm text-[var(--muted)]">{t("Loading…")}</div>
      ) : error ? (
        <div className="mt-2 text-sm text-[var(--danger)]">{error}</div>
      ) : runs.length ? (
        <ol className="mt-2 space-y-1 text-sm text-[var(--muted)]">
          {runs.map((runAt) => (
            <li key={runAt}>{formatTimestamp(runAt, language, t)}</li>
          ))}
        </ol>
      ) : (
        <div className="mt-2 text-sm text-[var(--muted)]">{t("No upcoming runs")}</div>
      )}
    </div>
  );
}

function taskFormDefaults(
  mode: TaskFormMode,
  workspaces: WorkspaceSummary[],
  enabledModels: ConfiguredModelSummary[],
  agentDefinitions: AgentDefinitionSettings[],
): ScheduledTaskFormState {
  if (mode.type === "edit") {
    return taskFormFromTask(mode.task);
  }

  const agentDefinition = defaultTaskAgentDefinition(agentDefinitions);
  const model = agentDefinition
    ? enabledModels.find((item) => item.id === agentDefinition.modelId) ?? null
    : enabledModels[0] ?? null;
  return {
    agentDefinitionId: agentDefinition?.id ?? "",
    collaborationToolsEnabled: true,
    concurrencyPolicy: "skip_if_running",
    description: "",
    intervalEvery: "1",
    intervalStartAt: "",
    intervalUnit: "days",
    misfirePolicy: "catch_up_once",
    modelId: agentDefinition?.modelId ?? model?.id ?? "",
    prompt: "",
    reuseChatId: "",
    runAt: dateTimeLocalFromDate(new Date(Date.now() + 60 * 60 * 1000)),
    scheduleType: "interval",
    sessionMode: "create_new_chat",
    status: "enabled",
    thinkingLevel: normalizeScheduledThinkingLevel(
      agentDefinition
        ? enabledModels.find((candidate) => candidate.id === agentDefinition.modelId)
        : model,
      agentDefinition?.modelOptions.thinkingLevel ?? model?.thinkingLevel,
    ),
    title: "",
    workspaceId: workspaces[0]?.id ?? "",
  };
}

function normalizeScheduledThinkingLevel(
  model: ConfiguredModelSummary | null | undefined,
  thinkingLevel: string | null | undefined,
) {
  return isModelThinkingLevelSupported(model, thinkingLevel) ? thinkingLevel : "";
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

function taskFormPayload(
  form: ScheduledTaskFormState,
  mode: TaskFormMode,
  t: Translate,
  enabledModels: ConfiguredModelSummary[] = [],
) {
  const title = form.title.trim();
  const prompt = form.prompt.trim();
  if (!title) {
    throw new Error(t("Title is required."));
  }
  if (!form.workspaceId) {
    throw new Error(t("Workspace is required."));
  }
  if (!prompt) {
    throw new Error(t("Prompt is required."));
  }
  if (!form.agentDefinitionId && !form.modelId) {
    throw new Error(t("Select an agent or model."));
  }

  const schedule = scheduleFromForm(form, t);
  const action: ScheduledTaskAction = {
    collaboration_tools_enabled: form.collaborationToolsEnabled,
    prompt,
    session_mode:
      form.sessionMode === "reuse_chat"
        ? { reuse_chat: { chat_id: requiredText(form.reuseChatId, t("Chat id is required.")) } }
        : "create_new_chat",
    skill_ids: [],
    type: "agent_prompt",
    ...(form.agentDefinitionId ? { agent_definition_id: form.agentDefinitionId } : {}),
    ...(form.modelId ? { model_id: form.modelId } : {}),
    ...(isModelThinkingLevelSupported(
      form.modelId ? enabledModels.find((model) => model.id === form.modelId) : null,
      form.thinkingLevel,
    )
      ? { thinking_level: form.thinkingLevel }
      : {}),
  };

  return {
    action,
    concurrencyPolicy: form.concurrencyPolicy,
    description: form.description.trim() || null,
    misfirePolicy: form.misfirePolicy,
    schedule,
    ...(mode.type === "create" || isFormTaskStatus(form.status)
      ? { status: form.status }
      : {}),
    title,
  };
}

function scheduleFromForm(
  form: ScheduledTaskFormState,
  t: Translate = (key) => key,
): ScheduledTaskSchedule {
  if (form.scheduleType === "one_shot_at") {
    return {
      run_at: localDateTimeToIso(requiredText(form.runAt, t("Run at is required.")), t),
      type: "one_shot_at",
    };
  }

  const every = Number.parseInt(form.intervalEvery, 10);
  if (!Number.isSafeInteger(every) || every <= 0) {
    throw new Error(t("Interval must be a positive whole number."));
  }
  const schedule: ScheduledTaskSchedule = {
    every_seconds: every * INTERVAL_UNIT_SECONDS[form.intervalUnit],
    type: "interval",
  };
  if (form.intervalStartAt) {
    schedule.start_at = localDateTimeToIso(form.intervalStartAt, t);
  }
  return schedule;
}

function requiredText(value: string, message: string) {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(message);
  }
  return normalized;
}

function defaultTaskAgentDefinition(agentDefinitions: AgentDefinitionSettings[]) {
  return (
    agentDefinitions.find((definition) => definition.id === DEFAULT_AGENT_DEFINITION_ID) ??
    agentDefinitions[0] ??
    null
  );
}

function isFormTaskStatus(status: ScheduledTaskStatus) {
  return status === "enabled" || status === "paused";
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
      return "bg-[var(--success-soft)] text-[var(--success-soft-foreground)] ring-[var(--success)]";
    case "paused":
      return "bg-[var(--warning-soft)] text-[var(--warning)] ring-[var(--warning)]";
    case "completed":
      return "bg-[var(--success-soft)] text-[var(--success-soft-foreground)] ring-[var(--success)]";
    case "archived":
      return "bg-[var(--surface-secondary)] text-[var(--muted)] ring-[var(--border)]";
    default:
      return "bg-[var(--surface-secondary)] text-[var(--muted)] ring-[var(--border)]";
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
      return "bg-[var(--success-soft)] text-[var(--success-soft-foreground)] ring-[var(--success)]";
    case "failed":
      return "bg-[var(--danger-soft)] text-[var(--danger)] ring-[var(--danger)]";
    case "running":
    case "queued":
    case "pending":
      return "bg-[var(--warning-soft)] text-[var(--warning)] ring-[var(--warning)]";
    case "cancelled":
    case "skipped":
      return "bg-[var(--surface-secondary)] text-[var(--muted)] ring-[var(--border)]";
    default:
      return "bg-[var(--surface-secondary)] text-[var(--muted)] ring-[var(--border)]";
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
    case "force_run":
      return t("Force run");
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
  if (seconds % 2592000 === 0) {
    return `${seconds / 2592000}mo`;
  }
  if (seconds % 604800 === 0) {
    return `${seconds / 604800}w`;
  }
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
  if (seconds % 2592000 === 0) {
    return { every: String(seconds / 2592000), unit: "months" };
  }
  if (seconds % 604800 === 0) {
    return { every: String(seconds / 604800), unit: "weeks" };
  }
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

function localDateTimeToIso(value: string, t: Translate = (key) => key) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error(t("Date/time is invalid."));
  }
  return date.toISOString();
}
