import {
  BarChart3,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ClipboardList,
  Eye,
  EyeOff,
  Files,
  FileText,
  Folder,
  GitCompare,
  ListChecks,
  LoaderCircle,
  MessageSquare,
  Minus,
  PanelBottom,
  PanelRight,
  Plus,
  RefreshCw,
  Save,
  ScrollText,
  Sparkles,
  Trash2,
  Undo2,
  type LucideIcon,
} from "lucide-react";
import {
  memo,
  useState,
  type ComponentProps,
  type FormEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  type SetStateAction,
} from "react";

import type {
  AppLanguageId,
  ChatStatisticsResponse,
  ContextMemoryState,
  ContextUsageResponse,
  GitDiffResponse,
  GitStatusFileSummary,
  MemoryFactRecord,
  Plan,
  PlanStatus,
  PlanStep,
  TaskStatus,
  TodoGraphResponse,
  TodoGraphTask,
  Translate,
  WorkspaceFilesResponse,
  WorkspaceFileTreeNode,
  WorkspaceSpecResponse,
} from "../../api/types";
import {
  chartColor,
  CONTEXT_PANEL_MAX_HEIGHT_RATIO,
  CONTEXT_PANEL_MAX_WIDTH,
  CONTEXT_PANEL_MIN_HEIGHT,
  CONTEXT_PANEL_MIN_WIDTH,
  MOBILE_BREAKPOINT_PX,
} from "../../app/constants";
import { MarkdownContent, type SelectedSkillPrefixResolver } from "../chat/MarkdownContent";
import { diffLineClass, parseGitDiffSections, type GitDiffSection } from "../git/diff-parser";
import { preloadOptionalMonaco } from "../files/WorkspaceFileEditorPanel";
import { useI18n } from "../../shared/i18n";

export type ContextPanelTab = "todo" | "plan" | "files" | "git" | "memory" | "stats" | "agents" | "spec";

type PanelNumberSetter = (value: SetStateAction<number>) => void;

export function ResponsiveContextPanelIcon({
  className,
}: {
  className?: string;
}) {
  return (
    <>
      <PanelRight aria-hidden="true" className={`${className ?? ""} hidden md:block`} />
      <PanelBottom aria-hidden="true" className={`${className ?? ""} md:hidden`} />
    </>
  );
}

const ContextPanel = memo(function ContextPanel({
  activeTab,
  agentsPanel,
  chatStatistics,
  chatStatisticsError,
  contextMemories,
  contextUsage,
  deletingContextMemoryId,
  contextMemoryError,
  diffError,
  diffResponse,
  files,
  gitCommitMessage,
  gitOperationKey,
  expandedFileTreePaths,
  isLoadingChatStatistics,
  isLoadingContextMemories,
  isLoadingPlans,
  loadingWorkspaceDirectoryPaths,
  isLoadingDiff,
  isLoadingTodoGraph,
  isLoadingWorkspaceSpec,
  isLoadingWorkspaceFiles,
  onForgetContextMemory,
  onGenerateGitCommitMessage,
  onGenerateWorkspaceSpec,
  onGitCommit,
  onGitCommitMessageChange,
  onGitFileOperation,
  onMemoryPageChange,
  onPlanAction,
  onReloadWorkspaceSpec,
  onRefreshDiff,
  onRefreshWorkspaceFiles,
  onSaveWorkspaceSpec,
  onToggleFileTreePath,
  onOpenWorkspaceFile,
  onOpenWorkspaceFileMenu,
  onSelectDiffFile,
  onTabChange,
  onWorkspaceSpecContentChange,
  onWorkspaceSpecPreviewChange,
  onWorkspaceSpecSettingsChange,
  selectedPath,
  selectedSkillPrefix,
  plans,
  planError,
  planOperationKey,
  todoGraph,
  todoGraphError,
  workspaceSpec,
  workspaceSpecConflictMessage,
  workspaceSpecDraft,
  workspaceSpecError,
  workspaceSpecOperationKey,
  workspaceSpecPreviewEnabled,
  workspaceFiles,
  workspaceFileOperationKey,
  workspaceFilesError,
}: {
  activeTab: ContextPanelTab;
  agentsPanel: ReactNode;
  chatStatistics: ChatStatisticsResponse | null;
  chatStatisticsError: string | null;
  contextMemories: ContextMemoryState;
  contextUsage: ContextUsageResponse | null;
  deletingContextMemoryId: string | null;
  contextMemoryError: string | null;
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  gitCommitMessage: string;
  gitOperationKey: string | null;
  expandedFileTreePaths: Set<string>;
  isLoadingChatStatistics: boolean;
  isLoadingContextMemories: boolean;
  isLoadingPlans: boolean;
  loadingWorkspaceDirectoryPaths: Set<string>;
  isLoadingDiff: boolean;
  isLoadingTodoGraph: boolean;
  isLoadingWorkspaceSpec: boolean;
  isLoadingWorkspaceFiles: boolean;
  onForgetContextMemory: (memory: MemoryFactRecord) => void;
  onGenerateGitCommitMessage: () => void;
  onGenerateWorkspaceSpec: () => void;
  onGitCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGitCommitMessageChange: (message: string) => void;
  onGitFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onMemoryPageChange: (scope: "global" | "workspace", page: number) => void;
  onPlanAction: (planId: string, action: string) => void;
  onReloadWorkspaceSpec: () => void;
  onRefreshDiff: () => void;
  onRefreshWorkspaceFiles: () => void;
  onSaveWorkspaceSpec: () => void;
  onToggleFileTreePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  onOpenWorkspaceFile: (node: WorkspaceFileTreeNode) => void;
  onOpenWorkspaceFileMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onSelectDiffFile: (path: string | null) => void;
  onTabChange: (tab: ContextPanelTab) => void;
  onWorkspaceSpecContentChange: (content: string) => void;
  onWorkspaceSpecPreviewChange: (enabled: boolean) => void;
  onWorkspaceSpecSettingsChange: (enabled: boolean, injectEnabled: boolean) => void;
  selectedPath: string | null;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  plans: Plan[];
  planError: string | null;
  planOperationKey: string | null;
  todoGraph: TodoGraphResponse | null;
  todoGraphError: string | null;
  workspaceSpec: WorkspaceSpecResponse | null;
  workspaceSpecConflictMessage: string | null;
  workspaceSpecDraft: string;
  workspaceSpecError: string | null;
  workspaceSpecOperationKey: "generate" | "save" | "settings" | null;
  workspaceSpecPreviewEnabled: boolean;
  workspaceFiles: WorkspaceFilesResponse | null;
  workspaceFileOperationKey: string | null;
  workspaceFilesError: string | null;
}) {
  const { t } = useI18n();
  const tabs: { id: ContextPanelTab; label: string; icon: LucideIcon }[] = [
    { id: "todo", label: "ToDo", icon: ListChecks },
    { id: "plan", label: "Plan", icon: ClipboardList },
    { id: "files", label: "Files", icon: Files },
    { id: "git", label: "Git", icon: GitCompare },
    { id: "agents", label: "Agents", icon: Bot },
    { id: "memory", label: "Memory", icon: Brain },
    { id: "spec", label: "Spec", icon: ScrollText },
    { id: "stats", label: "Stats", icon: BarChart3 },
  ];

  return (
    <section className="context-panel flex h-full min-h-0 min-w-0 flex-col">
      <div className="context-panel-tabs panel-scroll" role="tablist">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;

          return (
            <button
              aria-selected={isActive}
              className={`context-panel-tab ${isActive ? "context-panel-tab-active" : ""}`}
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              role="tab"
              type="button"
            >
              <Icon aria-hidden="true" className="size-3.5" />
              <span>{t(tab.label)}</span>
            </button>
          );
        })}
      </div>

      <div className="flex min-h-0 flex-1 flex-col">
        {activeTab === "todo" ? (
          <ContextTodoGraphTab
            error={todoGraphError}
            isLoading={isLoadingTodoGraph}
            todoGraph={todoGraph}
          />
        ) : null}

        {activeTab === "plan" ? (
          <ContextPlanTab
            error={planError}
            isLoading={isLoadingPlans}
            onAction={onPlanAction}
            operationKey={planOperationKey}
            plans={plans}
          />
        ) : null}

        {activeTab === "files" ? (
          <WorkspaceFilesTab
            error={workspaceFilesError}
            expandedPaths={expandedFileTreePaths}
            isLoading={isLoadingWorkspaceFiles}
            operationKey={workspaceFileOperationKey}
            loadingPaths={loadingWorkspaceDirectoryPaths}
            onOpenFile={onOpenWorkspaceFile}
            onOpenContextMenu={onOpenWorkspaceFileMenu}
            onRefresh={onRefreshWorkspaceFiles}
            onTogglePath={onToggleFileTreePath}
            response={workspaceFiles}
          />
        ) : null}

        {activeTab === "git" ? (
          <div className="flex min-h-0 flex-1 flex-col">
            <SourceControlPanel
              diffError={diffError}
              diffResponse={diffResponse}
              files={files}
              gitCommitMessage={gitCommitMessage}
              gitOperationKey={gitOperationKey}
              isLoading={isLoadingDiff}
              onCommit={onGitCommit}
              onGenerateCommitMessage={onGenerateGitCommitMessage}
              onCommitMessageChange={onGitCommitMessageChange}
              onFileOperation={onGitFileOperation}
              onRefresh={onRefreshDiff}
              onSelectFile={onSelectDiffFile}
              selectedPath={selectedPath}
            />
          </div>
        ) : null}

        {activeTab === "agents" ? agentsPanel : null}

        {activeTab === "memory" ? (
          <ContextMemoryTab
            deletingMemoryId={deletingContextMemoryId}
            error={contextMemoryError}
            isLoading={isLoadingContextMemories}
            memories={contextMemories}
            onForgetMemory={onForgetContextMemory}
            onPageChange={onMemoryPageChange}
          />
        ) : null}

        {activeTab === "spec" ? (
          <ContextSpecTab
            conflictMessage={workspaceSpecConflictMessage}
            contentDraft={workspaceSpecDraft}
            error={workspaceSpecError}
            isLoading={isLoadingWorkspaceSpec}
            onContentChange={onWorkspaceSpecContentChange}
            onGenerate={onGenerateWorkspaceSpec}
            onPreviewChange={onWorkspaceSpecPreviewChange}
            onReload={onReloadWorkspaceSpec}
            onSave={onSaveWorkspaceSpec}
            onSettingsChange={onWorkspaceSpecSettingsChange}
            operationKey={workspaceSpecOperationKey}
            previewEnabled={workspaceSpecPreviewEnabled}
            selectedSkillPrefix={selectedSkillPrefix}
            spec={workspaceSpec}
          />
        ) : null}

        {activeTab === "stats" ? (
          <ContextStatsTab
            contextUsage={contextUsage}
            error={chatStatisticsError}
            isLoading={isLoadingChatStatistics}
            statistics={chatStatistics}
          />
        ) : null}
      </div>
    </section>
  );
});

function WorkspaceFilesTab({
  error,
  expandedPaths,
  isLoading,
  loadingPaths,
  operationKey,
  onOpenFile,
  onOpenContextMenu,
  onRefresh,
  onTogglePath,
  response,
}: {
  error: string | null;
  expandedPaths: Set<string>;
  isLoading: boolean;
  loadingPaths: Set<string>;
  operationKey: string | null;
  onOpenFile: (node: WorkspaceFileTreeNode) => void;
  onOpenContextMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onRefresh: () => void;
  onTogglePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  response: WorkspaceFilesResponse | null;
}) {
  const { t } = useI18n();

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-sky-50 text-sky-800">
            <Files aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Files")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {t("Workspace file tree")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Refresh files")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-600 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isLoading}
          onClick={onRefresh}
          title={t("Refresh files")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </button>
      </div>

      {error ? (
        <div className="mx-4 mt-3 rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-medium text-rose-700">
          {error}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
        {response ? (
          <div
            className="workspace-file-tree"
            onFocusCapture={preloadOptionalMonaco}
            onMouseEnter={preloadOptionalMonaco}
            onPointerDown={preloadOptionalMonaco}
            role="tree"
          >
            <WorkspaceFileTreeNodeRow
              depth={0}
              expandedPaths={expandedPaths}
              loadingPaths={loadingPaths}
              node={response.root}
              onOpenFile={onOpenFile}
              onOpenContextMenu={onOpenContextMenu}
              onTogglePath={onTogglePath}
              operationKey={operationKey}
            />
          </div>
        ) : (
          <div className="rounded-xl border border-dashed border-stone-300 bg-white/60 px-3 py-4 text-sm text-stone-500">
            {isLoading ? t("Loading files...") : t("No files")}
          </div>
        )}
      </div>
    </div>
  );
}

function WorkspaceFileTreeNodeRow({
  depth,
  expandedPaths,
  loadingPaths,
  node,
  onOpenFile,
  onOpenContextMenu,
  onTogglePath,
  operationKey,
}: {
  depth: number;
  expandedPaths: Set<string>;
  loadingPaths: Set<string>;
  node: WorkspaceFileTreeNode;
  onOpenFile: (node: WorkspaceFileTreeNode) => void;
  onOpenContextMenu: (event: ReactMouseEvent, node: WorkspaceFileTreeNode) => void;
  onTogglePath: (node: WorkspaceFileTreeNode) => void | Promise<void>;
  operationKey: string | null;
}) {
  const { t } = useI18n();
  const isDirectory = node.kind === "directory";
  const isExpanded = expandedPaths.has(node.path);
  const isBusy = operationKey === `delete:${node.path}` || operationKey === `rename:${node.path}`;
  const isLoadingDirectory = loadingPaths.has(node.path);

  return (
    <div role="none">
      <div
        aria-expanded={isDirectory ? isExpanded : undefined}
        className="workspace-file-tree-row"
        onContextMenu={(event) => {
          if (node.path) {
            onOpenContextMenu(event, node);
          }
        }}
        onClick={() => {
          if (isDirectory) {
            void onTogglePath(node);
            return;
          }
          onOpenFile(node);
        }}
        role="treeitem"
        style={{ paddingLeft: `${depth * 0.875 + 0.25}rem` }}
      >
        <button
          aria-label={isExpanded ? t("Collapse folder") : t("Expand folder")}
          className="workspace-file-tree-toggle"
          disabled={!isDirectory}
          onClick={(event) => {
            event.stopPropagation();
            if (isDirectory) {
              void onTogglePath(node);
            }
          }}
          tabIndex={isDirectory ? 0 : -1}
          type="button"
        >
          {isDirectory ? (
            isExpanded ? (
              <ChevronDown aria-hidden="true" className="size-3.5" />
            ) : (
              <ChevronRight aria-hidden="true" className="size-3.5" />
            )
          ) : null}
        </button>
        {isDirectory ? (
          <Folder aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-folder-icon" />
        ) : (
          <FileText aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-file-icon" />
        )}
        <span className="workspace-file-tree-name" title={node.path || node.name}>
          {node.name}
        </span>
        {isBusy || isLoadingDirectory ? <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin text-stone-400" /> : null}
        {!isDirectory ? (
          <span className="workspace-file-tree-size">{formatFileSize(node.sizeBytes)}</span>
        ) : null}
      </div>
      {isDirectory && isExpanded
        ? node.children.map((child) => (
          <WorkspaceFileTreeNodeRow
            depth={depth + 1}
            expandedPaths={expandedPaths}
            key={child.path || child.name}
            loadingPaths={loadingPaths}
            node={child}
            onOpenFile={onOpenFile}
            onOpenContextMenu={onOpenContextMenu}
            onTogglePath={onTogglePath}
            operationKey={operationKey}
          />
        ))
        : null}
    </div>
  );
}


function TodoGraphPanel({
  error,
  isLoading,
  todoGraph,
}: {
  error: string | null;
  isLoading: boolean;
  todoGraph: TodoGraphResponse;
}) {
  const { language, t } = useI18n();

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-800">
            <ListChecks aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("ToDo graph")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {todoGraph.updatedAt
                ? t("Updated {time}", {
                  time: formatTodoGraphDate(todoGraph.updatedAt, language),
                })
                : todoGraph.chatId}
            </p>
          </div>
        </div>
        {isLoading ? (
          <LoaderCircle
            aria-hidden="true"
            className="size-4 shrink-0 animate-spin text-stone-500"
          />
        ) : null}
      </div>
      {error ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {error}
        </div>
      ) : null}
      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3">
        <div className="space-y-2">
          {todoGraph.tasks.map((task) => (
            <TodoGraphTaskItem key={task.id} level={0} task={task} />
          ))}
        </div>
      </div>
    </div>
  );
}

function ContextTodoGraphTab({
  error,
  isLoading,
  todoGraph,
}: {
  error: string | null;
  isLoading: boolean;
  todoGraph: TodoGraphResponse | null;
}) {
  const { t } = useI18n();

  if (todoGraph?.exists && todoGraph.tasks.length) {
    return (
      <TodoGraphPanel
        error={error}
        isLoading={isLoading}
        todoGraph={todoGraph}
      />
    );
  }

  return (
    <div className="context-empty-state">
      <ListChecks aria-hidden="true" className="size-5" />
      <h2>{t("ToDo graph")}</h2>
      <p>{t("No todo graph for the active session yet.")}</p>
    </div>
  );
}

function ContextPlanTab({
  error,
  isLoading,
  onAction,
  operationKey,
  plans,
}: {
  error: string | null;
  isLoading: boolean;
  onAction: (planId: string, action: string) => void;
  operationKey: string | null;
  plans: Plan[];
}) {
  const { language, t } = useI18n();
  const [expandedPhaseKeys, setExpandedPhaseKeys] = useState<Set<string>>(
    () => new Set(),
  );

  if (isLoading && plans.length === 0) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Plan")}</h2>
        <p>{t("Loading plans...")}</p>
      </div>
    );
  }

  if (error && plans.length === 0) {
    return (
      <div className="context-empty-state">
        <ScrollText aria-hidden="true" className="size-5" />
        <h2>{t("Plan")}</h2>
        <p>{error}</p>
      </div>
    );
  }

  if (!plans.length) {
    return (
      <div className="context-empty-state">
        <ScrollText aria-hidden="true" className="size-5" />
        <h2>{t("Plan")}</h2>
        <p>{t("No active plans for this workspace.")}</p>
      </div>
    );
  }

  return (
    <div className="context-list-panel panel-scroll">
      {error ? (
        <div className="mb-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-medium text-rose-700">
          {error}
        </div>
      ) : null}
      <div className="space-y-3">
        {plans.map((plan) => {
          const totalSteps = plan.phases.reduce(
            (count, phase) => count + phase.steps.length,
            0,
          );
          const completedSteps = plan.phases.reduce(
            (count, phase) =>
              count + phase.steps.filter((step) => step.status === "completed").length,
            0,
          );
          const action = primaryPlanAction(plan.status);
          const actionKey = action ? `${action}:${plan.id}` : null;
          const isMergedIntoSharedWorkspace = planMergedIntoSharedWorkspace(plan);

          return (
            <article className="context-memory-item" key={plan.id}>
              <div className="context-memory-item-header">
                <div className="context-memory-badges">
                  <span className={planStatusClass(plan.status)}>
                    {t(planStatusLabel(plan.status))}
                  </span>
                  <span className="context-memory-kind">
                    {completedSteps}/{totalSteps}
                  </span>
                  {isMergedIntoSharedWorkspace ? (
                    <span
                      className="context-memory-pin inline-flex items-center gap-1"
                      title={t("Merged into shared workspace")}
                    >
                      <CheckCircle2 aria-hidden="true" className="size-3" />
                      {t("Merged")}
                    </span>
                  ) : null}
                </div>
                {action ? (
                  <button
                    aria-label={t(planActionLabel(action))}
                    className="inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-stone-200 bg-white px-2 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                    disabled={operationKey !== null}
                    onClick={() => onAction(plan.id, action)}
                    title={t(planActionLabel(action))}
                    type="button"
                  >
                    {operationKey === actionKey ? (
                      <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-3.5" />
                    )}
                    {t(planActionLabel(action))}
                  </button>
                ) : null}
              </div>
              <h3 className="break-words text-sm font-semibold text-stone-950">
                {plan.title}
              </h3>
              <p>{plan.overview}</p>
              <small>
                {t("Updated {time}", {
                  time: formatTodoGraphDate(plan.updatedAt, language),
                })}
              </small>
              <div className="mt-3 space-y-2">
                {plan.phases.map((phase) => {
                  const phaseKey = `${plan.id}:${phase.id}`;
                  const isExpanded = expandedPhaseKeys.has(phaseKey);

                  return (
                    <section
                      className="rounded-lg border border-stone-200 bg-stone-50/80 px-2.5 py-2"
                      key={phase.id}
                    >
                      <button
                        aria-expanded={isExpanded}
                        className="flex w-full min-w-0 items-start justify-between gap-2 text-left"
                        onClick={() => {
                          setExpandedPhaseKeys((current) => {
                            const next = new Set(current);
                            if (next.has(phaseKey)) {
                              next.delete(phaseKey);
                            } else {
                              next.add(phaseKey);
                            }
                            return next;
                          });
                        }}
                        type="button"
                      >
                        <div className="flex min-w-0 items-start gap-2">
                          <ChevronRight
                            aria-hidden="true"
                            className={`mt-0.5 size-3.5 shrink-0 text-stone-500 transition-transform ${
                              isExpanded ? "rotate-90" : ""
                            }`}
                          />
                          <div className="min-w-0">
                            <div className="truncate text-xs font-semibold text-stone-900">
                              {phase.title}
                            </div>
                            {phase.summary ? (
                              <div className="mt-0.5 line-clamp-2 text-xs text-stone-500">
                                {phase.summary}
                              </div>
                            ) : null}
                          </div>
                        </div>
                        <span className={planPhaseStatusClass(phase.status)}>
                          {t(planPhaseStatusLabel(phase.status))}
                        </span>
                      </button>
                      {isExpanded ? (
                        <div className="mt-2 space-y-2 pl-5">
                          {phase.errorMessage ? (
                            <div className="rounded-md border border-rose-200 bg-rose-50 px-2 py-1.5 text-xs text-rose-700">
                              {phase.errorMessage}
                            </div>
                          ) : null}
                          {phase.implementationChatId ? (
                            <div className="flex min-w-0 items-center gap-1.5 text-xs text-stone-500">
                              <MessageSquare aria-hidden="true" className="size-3.5 shrink-0" />
                              <span className="truncate">
                                {t("Implementation chat")}: {phase.implementationChatId}
                              </span>
                            </div>
                          ) : null}
                          <div className="space-y-1.5">
                            {phase.steps.map((step) => (
                              <PlanStepRow key={step.id} step={step} />
                            ))}
                          </div>
                        </div>
                      ) : null}
                    </section>
                  );
                })}
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}

function PlanStepRow({ step }: { step: PlanStep }) {
  const { t } = useI18n();
  const isComplete = step.status === "completed";

  return (
    <div className="grid grid-cols-[auto_minmax(0,1fr)] gap-2 text-xs">
      <span
        aria-hidden="true"
        className={`mt-0.5 inline-flex size-4 items-center justify-center rounded border ${
          isComplete
            ? "border-teal-700 bg-teal-700 text-white"
            : "border-stone-300 bg-white text-transparent"
        }`}
      >
        <CheckCircle2 className="size-3" />
      </span>
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className={`min-w-0 break-words font-medium ${
              isComplete ? "text-stone-500 line-through" : "text-stone-800"
            }`}
          >
            {step.title}
          </span>
          {step.status !== "pending" && step.status !== "completed" ? (
            <span className={planPhaseStatusClass(step.status)}>
              {t(planPhaseStatusLabel(step.status))}
            </span>
          ) : null}
        </div>
        {step.detail ? (
          <div className="mt-0.5 whitespace-pre-wrap text-stone-500">{step.detail}</div>
        ) : null}
        {step.acceptance.length ? (
          <ul className="mt-1 list-disc space-y-0.5 pl-4 text-stone-500">
            {step.acceptance.map((acceptance) => (
              <li key={acceptance}>{acceptance}</li>
            ))}
          </ul>
        ) : null}
      </div>
    </div>
  );
}

function ContextMemoryTab({
  deletingMemoryId,
  error,
  isLoading,
  memories,
  onForgetMemory,
  onPageChange,
}: {
  deletingMemoryId: string | null;
  error: string | null;
  isLoading: boolean;
  memories: ContextMemoryState;
  onForgetMemory: (memory: MemoryFactRecord) => void;
  onPageChange: (scope: "global" | "workspace", page: number) => void;
}) {
  const { t } = useI18n();

  return (
    <div className="context-list-panel panel-scroll">
      {isLoading ? (
        <div className="context-empty-state">
          <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
          <h2>{t("Memory")}</h2>
          <p>{t("Loading...")}</p>
        </div>
      ) : error ? (
        <div className="context-empty-state">
          <Brain aria-hidden="true" className="size-5" />
          <h2>{t("Memory")}</h2>
          <p>{error}</p>
        </div>
      ) : (
        <>
          <ContextMemoryGroup
            deletingMemoryId={deletingMemoryId}
            emptyLabel={t("No memories")}
            label={t("Global memory")}
            memories={memories.global.memories}
            meta={{
              page: memories.global.page,
              pageSize: memories.global.pageSize,
              totalCount: memories.global.totalCount,
              totalPages: memories.global.totalPages,
            }}
            onForgetMemory={onForgetMemory}
            onPageChange={(page) => onPageChange("global", page)}
          />
          <ContextMemoryGroup
            deletingMemoryId={deletingMemoryId}
            emptyLabel={t("No memories")}
            label={t("Workspace memory")}
            memories={memories.workspace.memories}
            meta={{
              page: memories.workspace.page,
              pageSize: memories.workspace.pageSize,
              totalCount: memories.workspace.totalCount,
              totalPages: memories.workspace.totalPages,
            }}
            onForgetMemory={onForgetMemory}
            onPageChange={(page) => onPageChange("workspace", page)}
          />
        </>
      )}
    </div>
  );
}

function ContextMemoryGroup({
  deletingMemoryId,
  emptyLabel,
  label,
  meta,
  memories,
  onForgetMemory,
  onPageChange,
}: {
  deletingMemoryId: string | null;
  emptyLabel: string;
  label: string;
  meta: { page: number; pageSize: number; totalCount: number; totalPages: number };
  memories: MemoryFactRecord[];
  onForgetMemory: (memory: MemoryFactRecord) => void;
  onPageChange: (page: number) => void;
}) {
  const { language, t } = useI18n();
  const paginationItems = auditPaginationItems(meta.page, meta.totalPages);

  return (
    <div className="context-memory-group">
      <div className="context-panel-section-title">{label}</div>
      {memories.length ? (
        <>
          {memories.map((memory) => (
            <article className="context-memory-item" key={memory.id}>
              <div className="context-memory-item-header">
                <div className="context-memory-badges">
                  <span className="context-memory-kind">{memory.kind}</span>
                  {memory.pinned ? (
                    <span className="context-memory-pin">pinned</span>
                  ) : null}
                </div>
                <button
                  aria-label={t("Delete memory")}
                  className="context-memory-delete-button"
                  disabled={deletingMemoryId === memory.id}
                  onClick={() => onForgetMemory(memory)}
                  title={t("Delete memory")}
                  type="button"
                >
                  {deletingMemoryId === memory.id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" />
                  ) : (
                    <Trash2 aria-hidden="true" />
                  )}
                </button>
              </div>
              <p>{memory.fact}</p>
              <small>
                {memory.scope} 路 {formatTodoGraphDate(memory.updatedAt)}
              </small>
            </article>
          ))}
          {meta.totalPages > 1 ? (
            <div className="context-memory-pagination-shell">
              <nav
                aria-label={t("Memory pagination")}
                className="context-memory-pagination"
              >
                <button
                  aria-label={t("Previous page")}
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={meta.page <= 1}
                  onClick={() => onPageChange(meta.page - 1)}
                  title={t("Previous page")}
                  type="button"
                >
                  <ChevronLeft aria-hidden="true" className="size-4" />
                </button>
                {paginationItems.map((item, index) =>
                  item === "ellipsis" ? (
                    <span
                      aria-hidden="true"
                      className="context-memory-pagination-control context-memory-pagination-ellipsis inline-flex items-center justify-center text-stone-400"
                      key={`cm-ellipsis-${index}`}
                    >
                      ...
                    </span>
                  ) : (
                    <button
                      aria-current={
                        item === meta.page ? "page" : undefined
                      }
                      aria-label={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      className={`context-memory-pagination-control inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                        item === meta.page
                          ? "border-teal-700 bg-teal-700 text-white"
                          : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        }`}
                      key={item}
                      onClick={() => onPageChange(item)}
                      title={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      type="button"
                    >
                      {formatNumber(item, language)}
                    </button>
                  ),
                )}
                <button
                  aria-label={t("Next page")}
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={meta.totalPages === 0 || meta.page >= meta.totalPages}
                  onClick={() => onPageChange(meta.page + 1)}
                  title={t("Next page")}
                  type="button"
                >
                  <ChevronRight aria-hidden="true" className="size-4" />
                </button>
              </nav>
            </div>
          ) : null}
        </>
      ) : (
        <div className="context-empty-inline">{emptyLabel}</div>
      )}
    </div>
  );
}

function ContextSpecTab({
  conflictMessage,
  contentDraft,
  error,
  isLoading,
  onContentChange,
  onGenerate,
  onPreviewChange,
  onReload,
  onSave,
  onSettingsChange,
  operationKey,
  previewEnabled,
  selectedSkillPrefix,
  spec,
}: {
  conflictMessage: string | null;
  contentDraft: string;
  error: string | null;
  isLoading: boolean;
  onContentChange: (content: string) => void;
  onGenerate: () => void;
  onPreviewChange: (enabled: boolean) => void;
  onReload: () => void;
  onSave: () => void;
  onSettingsChange: (enabled: boolean, injectEnabled: boolean) => void;
  operationKey: "generate" | "save" | "settings" | null;
  previewEnabled: boolean;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  spec: WorkspaceSpecResponse | null;
}) {
  const { language, t } = useI18n();
  const enabled = spec?.settings.enabled ?? false;
  const injectEnabled = spec?.settings.injectEnabled ?? false;
  const isDirty = spec !== null && contentDraft !== spec.contentMarkdown;
  const isBusy = operationKey !== null;
  const latestJob = spec?.latestJob ?? null;
  const canEdit = enabled && spec !== null;
  const generateLabel = contentDraft.trim()
    ? t("Regenerate spec")
    : t("Generate spec");

  if (isLoading && !spec) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Project Spec")}</h2>
        <p>{t("Loading...")}</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--foco-canvas-raised)]">
      <div className="flex min-h-[var(--foco-header-height)] items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-teal-50 text-teal-800">
            <ScrollText aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Project Spec")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {spec
                ? `${t("Revision")} ${formatNumber(spec.revision, language)}`
                : t("Workspace spec")}
            </p>
          </div>
        </div>
        <button
          aria-label={t("Reload spec")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-stone-600 hover:bg-stone-200/80 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-400"
          disabled={isLoading}
          onClick={onReload}
          title={t("Reload spec")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </button>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden px-3 py-3">
        {error ? (
          <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-medium text-rose-700">
            {error}
          </div>
        ) : null}

        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_2.25rem_2.25rem_2.25rem] items-center gap-2">
          <button
            className="inline-flex min-h-9 min-w-0 items-center justify-center gap-2 rounded-md bg-teal-700 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-teal-900/45 disabled:text-white/70"
            disabled={!enabled || isBusy || isLoading}
            onClick={onGenerate}
            title={generateLabel}
            type="button"
          >
            {operationKey === "generate" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Sparkles aria-hidden="true" className="size-4" />
            )}
            <span className="truncate">{generateLabel}</span>
          </button>
          <button
            aria-label={t("Save")}
            className="inline-flex size-9 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
            disabled={!canEdit || !isDirty || isBusy || isLoading}
            onClick={onSave}
            title={t("Save")}
            type="button"
          >
            {operationKey === "save" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Save aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
            aria-pressed={previewEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm ${previewEnabled
                ? "border-teal-300 bg-teal-700 text-white hover:bg-teal-800"
                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
              }`}
            onClick={() => onPreviewChange(!previewEnabled)}
            title={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
            type="button"
          >
            {previewEnabled ? (
              <EyeOff aria-hidden="true" className="size-4" />
            ) : (
              <Eye aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label={t("Inject into new chats")}
            aria-pressed={injectEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400 ${injectEnabled
                ? "border-teal-300 bg-teal-700 text-white hover:bg-teal-800"
                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
              }`}
            disabled={!enabled || isBusy || isLoading}
            onClick={() => onSettingsChange(enabled, !injectEnabled)}
            title={t("Inject into new chats")}
            type="button"
          >
            {operationKey === "settings" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <MessageSquare aria-hidden="true" className="size-4" />
            )}
          </button>
        </div>

        {conflictMessage ? (
          <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-800">
            <div>{conflictMessage}</div>
            <button
              className="mt-2 inline-flex items-center gap-2 rounded-md border border-amber-300 bg-white px-2.5 py-1.5 text-xs font-semibold text-amber-900 hover:bg-amber-100"
              onClick={onReload}
              type="button"
            >
              <RefreshCw
                aria-hidden="true"
                className="context-refresh-icon size-3.5"
                data-loading={isLoading ? "true" : undefined}
              />
              {t("Reload spec")}
            </button>
          </div>
        ) : null}

        <div className="min-h-0 flex-1">
          {previewEnabled ? (
            <div className="h-full min-h-0 overflow-y-auto rounded-md border border-stone-200 bg-white px-4 py-3">
              {contentDraft.trim() ? (
                <MarkdownContent
                  content={contentDraft}
                  isUser={false}
                  selectedSkillPrefix={selectedSkillPrefix}
                />
              ) : (
                <div className="context-empty-inline">{t("No spec content")}</div>
              )}
            </div>
          ) : (
            <textarea
              aria-label={t("Project Spec Markdown")}
              className="h-full min-h-0 w-full resize-none rounded-md border border-stone-300 bg-white px-3 py-2 font-mono text-[13px] leading-5 text-stone-900 shadow-inner outline-none placeholder:text-stone-400 focus:border-teal-500 focus:ring-2 focus:ring-teal-100 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-500"
              disabled={!canEdit || isLoading}
              onChange={(event) => onContentChange(event.target.value)}
              placeholder={t("Generate or paste a Project Spec Markdown document.")}
              value={contentDraft}
            />
          )}
        </div>

        <div className="rounded-md border border-stone-200 bg-white px-3 py-2 text-xs leading-5 text-stone-600">
          {spec ? (
            <>
              <div>
                {t("Revision")} {formatNumber(spec.revision, language)}
                {spec.updatedAt ? ` · ${t("Updated")} ${formatTodoGraphDate(spec.updatedAt, language)}` : ""}
                {spec.generatedAt ? ` · ${t("Generated")} ${formatTodoGraphDate(spec.generatedAt, language)}` : ""}
              </div>
              <div>
                {latestJob
                  ? `${t("Latest job")}: ${t(workspaceSpecJobStatusLabel(latestJob.status))} · ${t(workspaceSpecTriggerLabel(latestJob.triggerType))} · ${latestJob.id}`
                  : t("No spec jobs")}
              </div>
              {latestJob?.errorMessage ? (
                <div className="break-words text-rose-700">{latestJob.errorMessage}</div>
              ) : null}
            </>
          ) : (
            t("No spec loaded")
          )}
        </div>
      </div>
    </div>
  );
}

function ContextStatsTab({
  contextUsage,
  error,
  isLoading,
  statistics,
}: {
  contextUsage: ContextUsageResponse | null;
  error: string | null;
  isLoading: boolean;
  statistics: ChatStatisticsResponse | null;
}) {
  const { language, t } = useI18n();

  if (isLoading && !statistics) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Stats")}</h2>
        <p>{t("Loading...")}</p>
      </div>
    );
  }

  if (error && !statistics) {
    return (
      <div className="context-empty-state">
        <BarChart3 aria-hidden="true" className="size-5" />
        <h2>{t("Stats")}</h2>
        <p>{error}</p>
      </div>
    );
  }

  if (!statistics) {
    return (
      <div className="context-empty-state">
        <BarChart3 aria-hidden="true" className="size-5" />
        <h2>{t("Stats")}</h2>
        <p>{t("No statistics for the active session yet.")}</p>
      </div>
    );
  }

  const tokenChart = [
    { id: "input", label: t("Input"), value: statistics.totalInputTokens },
    { id: "output", label: t("Output"), value: statistics.totalOutputTokens },
    { id: "cacheRead", label: t("Cache read"), value: statistics.totalCacheReadTokens },
    { id: "cacheWrite", label: t("Cache write"), value: statistics.totalCacheWriteTokens },
  ].filter((item) => item.value > 0);
  const modelChart = statistics.modelBreakdown.map((item) => ({
    id: item.modelId,
    label: item.modelId,
    value: item.requestCount,
  }));
  const contextChart = contextUsage
    ? contextUsage.tokenBreakdown.bySource
      .filter((item) => item.tokens > 0)
      .map((item) => ({
        id: item.source,
        label: contextSourceLabel(item.source, t),
        value: item.tokens,
      }))
    : [];

  return (
    <div className="context-stats-panel panel-scroll">
      <div className="context-stats-header">
        <div>
          <h2>{t("Session statistics")}</h2>
          <p>
            {t("Messages")}: {formatNumber(statistics.messageCount, language)}
          </p>
        </div>
        {isLoading ? (
          <LoaderCircle aria-label={t("Loading...")} className="size-4 animate-spin" />
        ) : null}
      </div>

      <div className="context-stats-metrics">
        <ContextStatMetric
          label={t("Total tokens")}
          value={formatCompactNumber(statistics.totalTokens, language)}
        />
        <ContextStatMetric
          label={t("Total time")}
          value={formatLatencySeconds(statistics.totalLatencyMs, language)}
        />
        <ContextStatMetric
          label={t("Memory refs")}
          value={formatNumber(statistics.memoryReferences, language)}
        />
        <ContextStatMetric
          label={t("New memories")}
          value={formatNumber(statistics.createdMemories, language)}
        />
        <ContextStatMetric
          label={t("LLM calls")}
          value={formatNumber(statistics.totalRequests, language)}
        />
        <ContextStatMetric
          label={t("Code changed")}
          value={`+${formatNumber(statistics.codeChangeStats.additions, language)} / -${formatNumber(statistics.codeChangeStats.deletions, language)}`}
        />
      </div>

      <ContextStatsSection title={t("Token usage")}>
        <ContextMiniBarChart
          data={tokenChart}
          emptyLabel={t("No token usage yet.")}
          valueFormatter={(value) => formatNumber(value, language)}
        />
      </ContextStatsSection>

      <ContextStatsSection title={t("Model calls")}>
        <ContextMiniBarChart
          data={modelChart}
          emptyLabel={t("No model calls yet.")}
          valueFormatter={(value) => formatNumber(value, language)}
        />
      </ContextStatsSection>

      <ContextStatsSection title={t("Context mix")}>
        {contextUsage ? (
          <>
            <div className="context-stats-inline">
              <span>{t("Context usage")}</span>
              <strong>
                {formatNumber(contextUsage.usedMessageTokens, language)} /{" "}
                {formatNumber(contextUsage.availableMessageTokens, language)}
              </strong>
            </div>
            <ContextMiniBarChart
              data={contextChart}
              emptyLabel={t("No context usage yet.")}
              valueFormatter={(value) => formatNumber(value, language)}
            />
            <ContextStatsRows
              emptyLabel={t("No context usage yet.")}
              rows={contextChart.map((item) => ({
                label: item.label,
                value: formatNumber(item.value, language),
              }))}
            />
          </>
        ) : (
          <div className="context-empty-inline">{t("Context usage unavailable.")}</div>
        )}
      </ContextStatsSection>

      <ContextStatsSection title={t("Tools and compression")}>
        <ContextStatsRows
          emptyLabel={t("No tools used yet.")}
          rows={[
            ...statistics.toolBreakdown.map((item) => ({
              label: item.toolName,
              value: formatNumber(item.callCount, language),
            })),
            {
              label: t("Rule compression snapshots"),
              value: formatNumber(statistics.compression.ruleSnapshotCount, language),
            },
            {
              label: t("LLM compression snapshots"),
              value: formatNumber(statistics.compression.llmSnapshotCount, language),
            },
            {
              label: t("Runtime tool-state snapshots"),
              value: formatNumber(
                statistics.compression.runtimeToolStateSnapshotCount,
                language,
              ),
            },
            {
              label: t("Compression snapshots"),
              value: formatNumber(statistics.compression.snapshotCount, language),
            },
            {
              label: t("Tokens saved"),
              value: formatNumber(statistics.compression.savedTokenCount, language),
            },
          ]}
        />
      </ContextStatsSection>
    </div>
  );
}

function ContextStatMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="context-stat-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ContextStatsSection({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <section className="context-stats-section">
      <div className="context-panel-section-title">{title}</div>
      {children}
    </section>
  );
}

function ContextMiniBarChart({
  data,
  emptyLabel,
  valueFormatter,
}: {
  data: { id: string; label: string; value: number }[];
  emptyLabel: string;
  valueFormatter: (value: number) => string;
}) {
  if (!data.length) {
    return <div className="context-empty-inline">{emptyLabel}</div>;
  }

  const chartMax = Math.max(...data.map((item) => item.value), 1);

  return (
    <div className="context-mini-chart context-mini-chart-bars">
      {data.map((item, index) => (
        <div className="context-mini-bar-row" key={item.id} title={valueFormatter(item.value)}>
          <span className="context-mini-bar-label">{item.label}</span>
          <span className="context-mini-bar-track">
            <span
              className="context-mini-bar-fill"
              style={{
                backgroundColor: chartColor(index),
                width: `${Math.max(2, (item.value / chartMax) * 100)}%`,
              }}
            />
          </span>
          <span className="context-mini-bar-value">{valueFormatter(item.value)}</span>
        </div>
      ))}
    </div>
  );
}

function ContextStatsRows({
  emptyLabel,
  rows,
}: {
  emptyLabel: string;
  rows: { label: string; value: string }[];
}) {
  if (!rows.length) {
    return <div className="context-empty-inline">{emptyLabel}</div>;
  }

  return (
    <div className="context-stats-rows">
      {rows.map((row) => (
        <div className="context-stats-row" key={row.label}>
          <span>{row.label}</span>
          <strong>{row.value}</strong>
        </div>
      ))}
    </div>
  );
}

function TodoGraphTaskItem({
  level,
  task,
}: {
  level: number;
  task: TodoGraphTask;
}) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(false);
  const bodyId = `todo-graph-task-${task.id}-body`;

  return (
    <div>
      <div
        className="rounded-lg border border-stone-200 bg-white shadow-sm transition hover:border-stone-300 hover:bg-stone-50"
        style={{ marginLeft: level ? Math.min(level * 14, 42) : 0 }}
      >
        <button
          aria-controls={bodyId}
          aria-expanded={isExpanded}
          className="flex w-full min-w-0 items-start gap-2 px-3 py-2 text-left focus:outline-none focus:ring-2 focus:ring-inset focus:ring-stone-300"
          onClick={() => setIsExpanded((current) => !current)}
          type="button"
        >
          {isExpanded ? (
            <ChevronDown
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-stone-500"
            />
          ) : (
            <ChevronRight
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-stone-500"
            />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] font-semibold text-stone-500">
                {task.id}
              </span>
              <span className={taskStatusClass(task.status)}>
                {t(task.status)}
              </span>
            </div>
            <h3
              className={`mt-1 break-words text-sm font-semibold leading-snug text-stone-950 ${isExpanded ? "" : "line-clamp-2"
                }`}
            >
              {task.title}
            </h3>
            {task.summary ? (
              <p
                className={`mt-1 break-words text-xs leading-5 text-stone-600 ${isExpanded ? "" : "line-clamp-2"
                  }`}
              >
                {task.summary}
              </p>
            ) : null}
          </div>
        </button>
        {isExpanded ? (
          <div className="px-3 pb-2 pl-8" id={bodyId}>
            {task.dependsOn.length ? (
              <div className="mt-1 flex flex-wrap gap-1.5">
                {task.dependsOn.map((dependencyId) => (
                  <span
                    className="rounded-md bg-stone-100 px-1.5 py-0.5 font-mono text-[11px] text-stone-600"
                    key={dependencyId}
                  >
                    {dependencyId}
                  </span>
                ))}
              </div>
            ) : null}
            {task.acceptance.length ? (
              <ul className="mt-2 space-y-1 text-xs leading-5 text-stone-600">
                {task.acceptance.map((item, index) => (
                  <li className="flex gap-2" key={`${task.id}-acceptance-${index}`}>
                    <CheckCircle2
                      aria-hidden="true"
                      className="mt-0.5 size-3.5 shrink-0 text-teal-700"
                    />
                    <span className="min-w-0 break-words">{item}</span>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
      </div>
      {task.subtasks.length ? (
        <div className="mt-2 space-y-2">
          {task.subtasks.map((subtask) => (
            <TodoGraphTaskItem
              key={subtask.id}
              level={level + 1}
              task={subtask}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function SourceControlPanel({
  diffError,
  diffResponse,
  files,
  gitCommitMessage,
  gitOperationKey,
  isLoading,
  onCommit,
  onGenerateCommitMessage,
  onCommitMessageChange,
  onFileOperation,
  onRefresh,
  onSelectFile,
  selectedPath,
}: {
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  gitCommitMessage: string;
  gitOperationKey: string | null;
  isLoading: boolean;
  onCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGenerateCommitMessage: () => void;
  onCommitMessageChange: (message: string) => void;
  onFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onRefresh: () => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();
  const diffSections = parseGitDiffSections(diffResponse);
  const stagedFiles = diffResponse?.stagedFiles ?? [];
  const isCommitting = gitOperationKey === "commit";
  const isGeneratingCommitMessage = gitOperationKey === "generate-commit-message";
  const isCommitMessageInputDisabled = isCommitting || isGeneratingCommitMessage;

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col bg-[var(--foco-canvas-raised)]">
      <div className="flex min-h-[var(--foco-header-height)] items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-teal-50 text-teal-800">
            <GitCompare aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <span className="foco-eyebrow">{t("Source Control")}</span>
            <h2 className="truncate text-sm font-semibold text-stone-950">
              {selectedPath ?? t("Workspace changes")}
            </h2>
          </div>
        </div>
        <button
          aria-label={t("Refresh diff")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-stone-600 hover:bg-stone-200/80 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-400"
          disabled={isLoading}
          onClick={onRefresh}
          title={t("Refresh diff")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </button>
      </div>

      {diffError ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {diffError}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
        <form className="mb-3 space-y-2 px-1" onSubmit={onCommit}>
          <div className="relative">
            <textarea
              className="min-h-20 w-full resize-none rounded-md border border-stone-300 bg-white px-3 py-2 pr-11 text-sm text-stone-900 shadow-inner outline-none placeholder:text-stone-400 focus:border-teal-500 focus:ring-2 focus:ring-teal-100"
              disabled={isCommitMessageInputDisabled}
              onChange={(event) => onCommitMessageChange(event.target.value)}
              placeholder={t("Commit message")}
              value={gitCommitMessage}
            />
            <button
              aria-label={t("Generate commit message")}
              className="absolute right-2 top-2 inline-flex size-7 items-center justify-center rounded-md text-teal-700 hover:bg-teal-50 hover:text-teal-900 disabled:cursor-not-allowed disabled:text-stone-300 disabled:hover:bg-transparent"
              disabled={isCommitMessageInputDisabled || stagedFiles.length === 0}
              onClick={onGenerateCommitMessage}
              title={t("Generate commit message")}
              type="button"
            >
              {isGeneratingCommitMessage ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <Sparkles aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
          <button
            className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-teal-700 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:text-stone-500"
            disabled={isCommitMessageInputDisabled || !gitCommitMessage.trim() || stagedFiles.length === 0}
            type="submit"
          >
            {isCommitting ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin" /> : null}
            {t("Commit")}
          </button>
        </form>

        <section className="mb-3">
          <div className="mb-1 flex items-center justify-between px-1 text-[11px] font-semibold uppercase tracking-wide text-stone-500">
            <span>{t("Staged Changes")}</span>
            <span>{stagedFiles.length}</span>
          </div>
          <div className="space-y-0.5">
            {stagedFiles.length ? (
              stagedFiles.map((file) => (
                <GitFileRow
                  action="unstage"
                  diffSections={diffSections}
                  file={file}
                  gitOperationKey={gitOperationKey}
                  isLoading={isLoading}
                  key={`staged-${file.path}`}
                  onFileOperation={onFileOperation}
                  onSelectFile={onSelectFile}
                  selectedPath={selectedPath}
                  showDiscard={false}
                />
              ))
            ) : (
              <div className="rounded-md border border-dashed border-stone-300 bg-white/70 px-3 py-2 text-xs text-stone-500">
                {t("No staged changes")}
              </div>
            )}
          </div>
        </section>

        <section>
          <button
            className={diffFileButtonClass(selectedPath === null)}
            onClick={() => onSelectFile(null)}
            type="button"
          >
            <span className="truncate text-[11px] font-semibold uppercase tracking-wide">
              {t("Changes")}
            </span>
            <span className="text-xs text-stone-500">{files.length}</span>
          </button>
          <div className="mt-1 space-y-0.5">
            {files.length ? (
              files.map((file) => (
                <GitFileRow
                  action="stage"
                  diffSections={diffSections}
                  file={file}
                  gitOperationKey={gitOperationKey}
                  isLoading={isLoading}
                  key={`unstaged-${file.path}`}
                  onFileOperation={onFileOperation}
                  onSelectFile={onSelectFile}
                  selectedPath={selectedPath}
                  showDiscard
                />
              ))
            ) : (
              <div className="rounded-md border border-dashed border-stone-300 bg-white/70 px-3 py-2 text-xs text-stone-500">
                {t("No changes")}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function GitFileRow({
  action,
  diffSections,
  file,
  gitOperationKey,
  isLoading,
  onFileOperation,
  onSelectFile,
  selectedPath,
  showDiscard,
}: {
  action: "stage" | "unstage";
  diffSections: GitDiffSection[];
  file: GitStatusFileSummary;
  gitOperationKey: string | null;
  isLoading: boolean;
  onFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
  showDiscard: boolean;
}) {
  const { t } = useI18n();
  const isExpanded = selectedPath === file.path;
  const label = statusLabel(file);
  const actionKey = `${action}:${file.path}`;
  const discardKey = `discard:${file.path}`;
  const isActionLoading = gitOperationKey === actionKey;
  const isDiscardLoading = gitOperationKey === discardKey;
  const pathParts = gitFilePathParts(file.path);

  return (
    <div>
      <div className={diffFileButtonClass(isExpanded)}>
        <button
          aria-label={`${file.path} ${label}`}
          className="flex min-w-0 flex-1 items-center gap-1.5 py-0.5 text-left"
          onClick={() => onSelectFile(isExpanded ? null : file.path)}
          type="button"
        >
          {isExpanded ? (
            <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
          ) : (
            <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
          )}
          <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-left">
            <span className="min-w-0 truncate text-[13px] font-medium text-stone-900">
              {pathParts.name}
            </span>
            {pathParts.directory ? (
              <span className="shrink truncate text-xs text-stone-400">
                {pathParts.directory}
              </span>
            ) : null}
          </span>
        </button>
        <span className={gitStatusBadgeClass(label)}>{label}</span>
        <button
          aria-label={t(action === "stage" ? "Stage file" : "Unstage file")}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded text-stone-500 hover:bg-stone-200 hover:text-stone-950 disabled:cursor-not-allowed disabled:text-stone-300"
          disabled={gitOperationKey !== null}
          onClick={(event) => {
            event.stopPropagation();
            onFileOperation(action, file.path);
          }}
          title={t(action === "stage" ? "Stage file" : "Unstage file")}
          type="button"
        >
          {isActionLoading ? (
            <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          ) : action === "stage" ? (
            <Plus aria-hidden="true" className="size-3.5" />
          ) : (
            <Minus aria-hidden="true" className="size-3.5" />
          )}
        </button>
        {showDiscard ? (
          <button
            aria-label={t("Discard file changes")}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded text-stone-500 hover:bg-rose-100 hover:text-rose-700 disabled:cursor-not-allowed disabled:text-stone-300"
            disabled={gitOperationKey !== null}
            onClick={(event) => {
              event.stopPropagation();
              onFileOperation("discard", file.path);
            }}
            title={t("Discard file changes")}
            type="button"
          >
            {isDiscardLoading ? (
              <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
            ) : (
              <Undo2 aria-hidden="true" className="size-3.5" />
            )}
          </button>
        ) : null}
      </div>
      {isExpanded ? (
        <InlineGitDiff isLoading={isLoading} path={file.path} sections={diffSections} />
      ) : null}
    </div>
  );
}

function InlineGitDiff({
  isLoading,
  path,
  sections,
}: {
  isLoading: boolean;
  path: string;
  sections: GitDiffSection[];
}) {
  const { t } = useI18n();
  const matchingSections = sections
    .map((section) => ({
      ...section,
      files: section.files.filter((file) => file.path === path),
    }))
    .filter((section) => section.files.length > 0);

  if (isLoading) {
    return (
      <div className="ml-5 mt-1 flex items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 py-3 text-xs font-medium text-stone-500">
        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        {t("Loading...")}
      </div>
    );
  }

  if (!matchingSections.length) {
    return (
      <div className="ml-5 mt-1">
        <InlineGitDiffNotice>
          {t("Inline diff is unavailable for binary or non-text files.")}
        </InlineGitDiffNotice>
      </div>
    );
  }

  return (
    <div className="ml-5 mt-1 space-y-2">
      {matchingSections.map((section) => (
        <div key={section.kind} className="space-y-2">
          <div className="text-[11px] font-semibold uppercase text-stone-500">
            {t(section.kind === "staged" ? "Staged" : "Unstaged")}
          </div>
          {section.files.map((file) =>
            file.isBinary || file.lines.length === 0 ? (
              <InlineGitDiffNotice key={`${section.kind}-${file.path}`}>
                {t("Inline diff is unavailable for binary or non-text files.")}
              </InlineGitDiffNotice>
            ) : (
              <div
                className="panel-scroll max-h-[min(30rem,52dvh)] overflow-auto rounded-lg border border-stone-200 bg-white py-2 font-mono text-[11px] leading-5 shadow-sm"
                key={`${section.kind}-${file.path}`}
              >
                {file.lines.map((line, index) => (
                  <div
                    className={diffLineClass(line.kind)}
                    key={`${section.kind}-${file.path}-${index}`}
                  >
                    <span className="select-none pr-2 text-stone-400">
                      {line.prefix}
                    </span>
                    <span>{line.text || " "}</span>
                  </div>
                ))}
              </div>
            ),
          )}
        </div>
      ))}
    </div>
  );
}

function InlineGitDiffNotice({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-stone-200 bg-stone-50 px-3 py-3 text-xs font-medium text-stone-500">
      <FileText aria-hidden="true" className="size-3.5 shrink-0" />
      <span>{children}</span>
    </div>
  );
}

type ContextPanelSidebarProps = ComponentProps<typeof ContextPanel> & {
  diffPanelWidth: number;
  isResizing: boolean;
  onResizeStart: () => void;
  setMobileHeight: PanelNumberSetter;
  setWidth: PanelNumberSetter;
};

export function ContextPanelSidebar({
  diffPanelWidth,
  isResizing,
  onResizeStart,
  setMobileHeight,
  setWidth,
  ...panelProps
}: ContextPanelSidebarProps) {
  const { t } = useI18n();

  return (
    <aside className="context-sidebar diff-sidebar min-w-0 border-stone-200/80 lg:border-l">
      <div className="relative flex h-full min-h-0 min-w-0 flex-col">
        <div
          aria-label={t("Resize context panel")}
          aria-orientation="vertical"
          aria-valuemax={CONTEXT_PANEL_MAX_WIDTH}
          aria-valuemin={CONTEXT_PANEL_MIN_WIDTH}
          aria-valuenow={diffPanelWidth}
          className={`context-sidebar-splitter ${isResizing ? "context-sidebar-splitter-active" : ""}`}
          onKeyDown={(event) => {
            if (event.key === "ArrowLeft") {
              event.preventDefault();
              setWidth((current) => Math.min(current + 24, CONTEXT_PANEL_MAX_WIDTH));
            }

            if (event.key === "ArrowRight") {
              event.preventDefault();
              setWidth((current) => Math.max(current - 24, CONTEXT_PANEL_MIN_WIDTH));
            }

            if (event.key === "ArrowUp") {
              event.preventDefault();
              setMobileHeight((current) =>
                Math.min(
                  current + 24,
                  Math.floor(window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO),
                ),
              );
            }

            if (event.key === "ArrowDown") {
              event.preventDefault();
              setMobileHeight((current) => Math.max(current - 24, CONTEXT_PANEL_MIN_HEIGHT));
            }
          }}
          onPointerDown={(event) => {
            event.preventDefault();
            if (window.innerWidth < MOBILE_BREAKPOINT_PX) {
              const maxHeight = Math.floor(window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO);
              const nextHeight = window.innerHeight - event.clientY;
              setMobileHeight(Math.min(Math.max(nextHeight, CONTEXT_PANEL_MIN_HEIGHT), maxHeight));
            } else {
              const nextWidth = window.innerWidth - event.clientX;
              setWidth(Math.min(Math.max(nextWidth, CONTEXT_PANEL_MIN_WIDTH), CONTEXT_PANEL_MAX_WIDTH));
            }
            event.currentTarget.setPointerCapture(event.pointerId);
            onResizeStart();
          }}
          role="separator"
          tabIndex={0}
        />
        <ContextPanel {...panelProps} />
      </div>
    </aside>
  );
}

function auditPaginationItems(
  currentPage: number,
  totalPages: number,
): Array<number | "ellipsis"> {
  if (totalPages <= 0) {
    return [];
  }

  const pages = new Set<number>([1, totalPages]);
  for (
    let page = Math.max(1, currentPage - 2);
    page <= Math.min(totalPages, currentPage + 2);
    page += 1
  ) {
    pages.add(page);
  }

  const sortedPages = Array.from(pages).sort((left, right) => left - right);
  const items: Array<number | "ellipsis"> = [];

  for (const page of sortedPages) {
    const previous = items[items.length - 1];
    if (typeof previous === "number" && page - previous > 1) {
      items.push("ellipsis");
    }
    items.push(page);
  }

  return items;
}

function taskStatusClass(status: TaskStatus) {
  const base = "inline-flex rounded-md px-2 py-0.5 text-[11px] font-semibold";

  if (status === "completed") {
    return `${base} bg-emerald-100 text-emerald-800`;
  }

  if (status === "running" || status === "ready") {
    return `${base} bg-amber-100 text-amber-800`;
  }

  if (status === "failed") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function primaryPlanAction(status: PlanStatus) {
  if (status === "implemented" || status === "failed" || status === "cancelled") {
    return "mark_complete";
  }
  if (status === "paused") {
    return "resume";
  }
  if (status === "ready" || status === "draft") {
    return "start";
  }
  if (status === "running") {
    return "pause";
  }
  return null;
}

function planActionLabel(action: string) {
  switch (action) {
    case "mark_complete":
      return "Mark complete";
    case "resume":
      return "Resume";
    case "start":
      return "Start";
    case "pause":
      return "Pause";
    default:
      return action;
  }
}

function planMergedIntoSharedWorkspace(plan: Plan) {
  return (
    plan.status === "implemented" &&
    plan.phases.length > 0 &&
    plan.phases.every(
      (phase) => phase.status === "completed" && phase.implementationChatId,
    )
  );
}

function planStatusLabel(status: string) {
  switch (status) {
    case "draft":
      return "Draft";
    case "ready":
      return "Ready";
    case "running":
      return "Running";
    case "paused":
      return "Paused";
    case "implemented":
      return "Implemented";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    default:
      return status;
  }
}

function planPhaseStatusLabel(status: string) {
  return planStatusLabel(status);
}

function planStatusClass(status: PlanStatus) {
  const base = "inline-flex rounded-md px-2 py-0.5 text-[11px] font-semibold";
  if (status === "implemented" || status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }
  if (status === "running") {
    return `${base} bg-amber-100 text-amber-800`;
  }
  if (status === "paused" || status === "draft" || status === "ready") {
    return `${base} bg-sky-100 text-sky-700`;
  }
  if (status === "failed" || status === "cancelled") {
    return `${base} bg-rose-100 text-rose-700`;
  }
  return `${base} bg-stone-100 text-stone-600`;
}

function planPhaseStatusClass(status: string) {
  const base = "inline-flex shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-semibold";
  if (status === "completed" || status === "implemented") {
    return `${base} bg-teal-100 text-teal-800`;
  }
  if (status === "running") {
    return `${base} bg-amber-100 text-amber-800`;
  }
  if (status === "failed" || status === "cancelled") {
    return `${base} bg-rose-100 text-rose-700`;
  }
  return `${base} bg-stone-100 text-stone-600`;
}

function workspaceSpecJobStatusLabel(status: string) {
  switch (status) {
    case "queued":
      return "Queued";
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "skipped":
      return "Skipped";
    case "failed":
      return "Failed";
    default:
      return status;
  }
}

function workspaceSpecTriggerLabel(triggerType: string) {
  switch (triggerType) {
    case "manual_initial":
      return "Manual initial";
    case "manual_refresh":
      return "Manual refresh";
    case "chat_completed":
      return "Chat completed";
    default:
      return triggerType;
  }
}

function formatTodoGraphDate(value: string, language: AppLanguageId = "en") {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
  }).format(date);
}

function formatFileSize(sizeBytes: number) {
  const units = ["B", "KB", "MB", "GB"];
  let value = sizeBytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const formatted =
    unitIndex === 0 || value >= 10 ? value.toFixed(0) : value.toFixed(1);
  return `${formatted} ${units[unitIndex]}`;
}

function formatLatencySeconds(value: number, language: AppLanguageId = "en") {
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: 0 }).format(value / 1000)} s`;
}

function formatNumber(value: number, language: AppLanguageId = "en") {
  return new Intl.NumberFormat(language).format(value);
}

function formatCompactNumber(value: number, _language: AppLanguageId = "en") {
  return new Intl.NumberFormat("en", {
    maximumFractionDigits: 1,
    notation: "compact",
  }).format(value);
}

function contextSourceLabel(source: string, t: Translate) {
  const labels: Record<string, string> = {
    assistantDraft: t("Assistant draft"),
    compressionSnapshot: t("Compression"),
    currentUser: t("Current user"),
    guidance: t("Guidance"),
    hookContext: t("Hook context"),
    persistedHistory: t("History"),
    reservedPrompt: t("Prompt"),
    runtimeAssistant: t("Runtime assistant"),
    runtimeToolState: t("Runtime tools"),
    runtimeToolStateSnapshot: t("Tool snapshot"),
    stableInjection: t("Stable context"),
    todoGraph: t("ToDo"),
    turnMemory: t("Memory"),
  };

  return labels[source] ?? source;
}

function diffFileButtonClass(active: boolean) {
  return `diff-file-button flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-sm ${active
      ? "diff-file-button-active bg-teal-50 text-teal-950 shadow-sm"
      : "text-stone-700 hover:bg-stone-50 hover:text-stone-950"
    }`;
}

function gitFilePathParts(path: string) {
  const separatorIndex = path.lastIndexOf("/");
  if (separatorIndex === -1) {
    return { directory: "", name: path };
  }

  return {
    directory: path.slice(0, separatorIndex),
    name: path.slice(separatorIndex + 1),
  };
}

function statusLabel(file: GitStatusFileSummary) {
  const statuses = [file.indexStatus, file.worktreeStatus]
    .map(normalizeGitStatus)
    .filter(Boolean);
  const uniqueStatuses = [...new Set(statuses)];

  return uniqueStatuses.length ? uniqueStatuses.join("") : ".";
}

function gitStatusBadgeClass(label: string) {
  const status = label[0] ?? ".";
  const colorClass =
    status === "M"
      ? "bg-amber-100 text-amber-700 border-amber-200"
      : status === "U" || status === "A"
        ? "bg-emerald-100 text-emerald-700 border-emerald-200"
        : status === "D"
          ? "bg-rose-100 text-rose-700 border-rose-200"
          : status === "R"
            ? "bg-sky-100 text-sky-700 border-sky-200"
            : "bg-stone-100 text-stone-600 border-stone-200";

  return `shrink-0 rounded border px-1.5 py-0.5 font-mono text-[11px] font-semibold leading-none ${colorClass}`;
}

function normalizeGitStatus(status: string) {
  const trimmed = status.trim();
  if (!trimmed) {
    return "";
  }

  return trimmed === "?" ? "U" : trimmed;
}
