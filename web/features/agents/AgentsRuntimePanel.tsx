import {
  Activity,
  Bot,
  CheckCircle2,
  CircleAlert,
  CircleDashed,
  CirclePause,
  CircleStop,
  GitBranch,
  Hourglass,
  LoaderCircle,
  RefreshCw,
  User,
  type LucideIcon,
} from "lucide-react";

import type {
  AgentInstanceView,
  AgentTeamSnapshotResponse,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";

export function AgentsRuntimePanel({
  activeChatId,
  error,
  isLoading,
  onRefresh,
  onSelectInstance,
  selectedInstanceId,
  snapshot,
}: {
  activeChatId: string | null;
  error: string | null;
  isLoading: boolean;
  onRefresh: () => Promise<void>;
  onSelectInstance: (instance: AgentInstanceView) => void;
  selectedInstanceId: string | null;
  snapshot: AgentTeamSnapshotResponse | null;
}) {
  const { t } = useI18n();
  const instances = snapshot?.instances ?? [];

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
              {t("Current chat agent instances")}
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
        <AgentEmptyState text={t("Open a chat to view its Agent instances.")} />
      ) : null}

      {activeChatId && !snapshot && !error ? (
        <AgentEmptyState
          text={
            isLoading
              ? t("Loading agent instances...")
              : t("No agent instances in this chat yet.")
          }
        />
      ) : null}

      {activeChatId && snapshot ? (
        instances.length ? (
          <AgentInstancesList
            instances={instances}
            onSelectInstance={onSelectInstance}
            selectedInstanceId={selectedInstanceId}
          />
        ) : (
          <AgentEmptyState text={t("No agent instances in this chat yet.")} />
        )
      ) : null}
    </section>
  );
}

function AgentInstancesList({
  instances,
  onSelectInstance,
  selectedInstanceId,
}: {
  instances: AgentInstanceView[];
  onSelectInstance: (instance: AgentInstanceView) => void;
  selectedInstanceId: string | null;
}) {
  return (
    <div className="grid gap-2">
      {instances.map((instance) => (
        <AgentInstanceCard
          instance={instance}
          isSelected={instance.id === selectedInstanceId}
          key={instance.id}
          onSelect={() => onSelectInstance(instance)}
        />
      ))}
    </div>
  );
}

function AgentInstanceCard({
  instance,
  isSelected,
  onSelect,
}: {
  instance: AgentInstanceView;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const { t } = useI18n();
  const isIsolated = instance.executionWorkspaceMode === "isolated_worktree";
  const status = agentStatusPresentation(instance.status);
  const StatusIcon = status.icon;

  return (
    <button
      aria-label={t("Open agent {name}", {
        name: instance.definitionSnapshot.name,
      })}
      aria-pressed={isSelected}
      className={`w-full rounded-lg border px-3 py-3 text-left transition ${isSelected
          ? "border-teal-300 bg-teal-50 text-stone-950 shadow-sm"
          : "border-stone-200 bg-white hover:border-teal-200 hover:bg-teal-50"
        }`}
      onClick={onSelect}
      type="button"
    >
      <div className="flex min-w-0 items-start gap-2">
        <span
          aria-label={t("Agent status {status}", { status: instance.status })}
          className={`mt-0.5 inline-flex size-9 shrink-0 items-center justify-center rounded-lg border ${status.className}`}
          title={t("Agent status {status}", { status: instance.status })}
        >
          <StatusIcon
            aria-hidden="true"
            className={`size-5 ${status.animate ? "animate-spin" : ""}`}
          />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-1.5">
            <User aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
            <div className="truncate text-sm font-semibold text-stone-950">
              {instance.definitionSnapshot.name}
            </div>
          </div>
          <div className="mt-1 flex flex-wrap gap-1.5 text-[11px] font-semibold uppercase tracking-normal text-stone-500">
            <span>{instance.role}</span>
            <span className={status.textClassName}>{instance.status}</span>
            <span>{isIsolated ? t("isolated") : t("shared")}</span>
            {instance.worktreeStatus ? <span>{instance.worktreeStatus}</span> : null}
          </div>
        </div>
      </div>

      {isIsolated && instance.worktreeBranch ? (
        <div className="mt-2 flex min-w-0 items-center gap-1.5 text-xs text-stone-500">
          <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
          <span className="truncate">{instance.worktreeBranch}</span>
        </div>
      ) : null}

      {instance.executionRootPath ? (
        <div className="mt-1 truncate font-mono text-[11px] text-stone-500">
          {instance.executionRootPath}
        </div>
      ) : null}

      <dl className="mt-2 grid gap-1 text-xs text-stone-600">
        <AgentKeyValue label={t("Instance ID")} value={instance.id} />
        <AgentKeyValue label={t("Definition")} value={instance.definitionId} />
        <AgentKeyValue label={t("Created")} value={formatAgentTimestamp(instance.createdAt)} />
      </dl>
    </button>
  );
}

function AgentKeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="inline font-semibold text-stone-800">{label}: </dt>
      <dd className="inline break-all">{value}</dd>
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

function agentStatusPresentation(status: string): {
  animate: boolean;
  className: string;
  icon: LucideIcon;
  textClassName: string;
} {
  switch (status) {
    case "running":
      return {
        animate: true,
        className: "border-sky-200 bg-sky-100 text-sky-700",
        icon: LoaderCircle,
        textClassName: "text-sky-700",
      };
    case "waiting":
      return {
        animate: false,
        className: "border-violet-200 bg-violet-100 text-violet-700",
        icon: Hourglass,
        textClassName: "text-violet-700",
      };
    case "paused":
      return {
        animate: false,
        className: "border-amber-200 bg-amber-100 text-amber-700",
        icon: CirclePause,
        textClassName: "text-amber-700",
      };
    case "draining":
      return {
        animate: false,
        className: "border-orange-200 bg-orange-100 text-orange-700",
        icon: Activity,
        textClassName: "text-orange-700",
      };
    case "stopped":
      return {
        animate: false,
        className: "border-stone-200 bg-stone-100 text-stone-600",
        icon: CircleStop,
        textClassName: "text-stone-600",
      };
    case "failed":
      return {
        animate: false,
        className: "border-rose-200 bg-rose-100 text-rose-700",
        icon: CircleAlert,
        textClassName: "text-rose-700",
      };
    case "idle":
    case "active":
      return {
        animate: false,
        className: "border-emerald-200 bg-emerald-100 text-emerald-700",
        icon: CheckCircle2,
        textClassName: "text-emerald-700",
      };
    default:
      return {
        animate: false,
        className: "border-stone-200 bg-stone-100 text-stone-600",
        icon: CircleDashed,
        textClassName: "text-stone-600",
      };
  }
}

function formatAgentTimestamp(value: string) {
  return new Date(value).toLocaleString();
}
