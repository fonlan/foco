import { CalendarClock, LoaderCircle, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  ScheduledTaskStatus,
  ScheduledTasksResponse,
  ScheduledTaskView,
  Translate,
} from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";

type ScheduledTasksQuery = {
  q?: string;
  status?: ScheduledTaskStatus;
  workspaceId?: string;
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

  const search = params.toString();
  return requestJson<ScheduledTasksResponse>(
    search ? `/api/scheduled-tasks?${search}` : "/api/scheduled-tasks",
  );
}

export function ScheduledTasksPage() {
  const { language, t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [tasks, setTasks] = useState<ScheduledTaskView[]>([]);

  const statusCounts = useMemo(() => {
    return tasks.reduce<Record<string, number>>((counts, task) => {
      counts[task.status] = (counts[task.status] ?? 0) + 1;
      return counts;
    }, {});
  }, [tasks]);

  const loadTasks = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await listScheduledTasks();
      setTasks(data.tasks);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

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
        </section>

        <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {(["enabled", "paused", "completed", "archived"] as const).map((status) => (
            <div
              className="rounded-lg border border-stone-200 bg-white/85 px-4 py-3 shadow-sm"
              key={status}
            >
              <div className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                {statusLabel(status, t)}
              </div>
              <div className="mt-2 text-2xl font-semibold text-stone-950">
                {formatNumber(statusCounts[status] ?? 0, language)}
              </div>
            </div>
          ))}
        </section>

        <section className="min-w-0 overflow-hidden rounded-lg border border-stone-200 bg-white/85 shadow-sm">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
            <div>
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Task list")}
              </h3>
              <p className="mt-1 text-xs text-stone-500">
                {isLoading
                  ? t("Loading...")
                  : t("tasks {count}", { count: tasks.length })}
              </p>
            </div>
          </div>
          {error ? (
            <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div className="panel-scroll min-w-0 overflow-x-auto">
            <table className="w-full min-w-max text-left text-sm">
              <thead className="border-b border-stone-200 bg-white text-xs font-semibold text-stone-500">
                <tr>
                  <th className="px-4 py-3">{t("Task")}</th>
                  <th className="px-4 py-3">{t("Workspace")}</th>
                  <th className="px-4 py-3">{t("Status")}</th>
                  <th className="px-4 py-3">{t("Schedule")}</th>
                  <th className="px-4 py-3">{t("Next run")}</th>
                  <th className="px-4 py-3">{t("Last run")}</th>
                  <th className="px-4 py-3">{t("Prompt")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {tasks.length ? (
                  tasks.map((task) => (
                    <tr className="align-top hover:bg-amber-50/40" key={task.id}>
                      <td className="max-w-72 px-4 py-3">
                        <div className="truncate font-semibold text-stone-950">
                          {task.title}
                        </div>
                        {task.description ? (
                          <div className="mt-1 truncate text-xs text-stone-500">
                            {task.description}
                          </div>
                        ) : null}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-stone-700">
                        {task.workspaceName}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3">
                        <span
                          className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ring-1 ${statusClass(task.status)}`}
                        >
                          {statusLabel(task.status, t)}
                        </span>
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-stone-700">
                        {scheduleSummary(task.schedule, t)}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-stone-700">
                        {formatTimestamp(task.nextRunAt, language, t)}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-stone-700">
                        {formatTimestamp(task.lastRunAt, language, t)}
                      </td>
                      <td className="max-w-96 px-4 py-3 text-stone-700">
                        <div className="truncate">{actionSummary(task.action, t)}</div>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td className="px-4 py-10 text-center text-sm text-stone-500" colSpan={7}>
                      {isLoading ? t("Loading...") : t("No scheduled tasks")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>
      </div>
    </div>
  );
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

function scheduleSummary(schedule: unknown, t: Translate) {
  if (!isRecord(schedule) || typeof schedule.type !== "string") {
    return t("Custom schedule");
  }

  if (schedule.type === "one_shot_at") {
    return t("One-shot");
  }

  if (schedule.type === "interval" && typeof schedule.every_seconds === "number") {
    return t("Every {duration}", {
      duration: formatDurationSeconds(schedule.every_seconds),
    });
  }

  if (schedule.type === "cron") {
    return t("Cron");
  }

  return schedule.type;
}

function actionSummary(action: unknown, t: Translate) {
  if (!isRecord(action) || action.type !== "agent_prompt") {
    return t("Custom action");
  }

  return typeof action.prompt === "string" && action.prompt.trim()
    ? action.prompt.trim()
    : t("Agent prompt");
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
