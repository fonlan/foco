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
  GripVertical,
  ListChecks,
  LoaderCircle,
  LocateFixed,
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
  X,
  type LucideIcon,
} from "lucide-react";
import {
  memo,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ComponentProps,
  type DragEvent,
  type FormEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  type SetStateAction,
} from "react";

import type {
  AppLanguageId,
  ChatStatisticsResponse,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ContextMemoryState,
  ContextUsageResponse,
  ContextUsageSegments,
  GitDiffResponse,
  GitStatusFileSummary,
  MemoryFactRecord,
  Plan,
  PlanPhase,
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
  CONTEXT_PANEL_STACKED_BREAKPOINT_PX,
} from "../../app/constants";
import { MarkdownContent, type SelectedSkillPrefixResolver } from "../chat/MarkdownContent";
import { toolDisplayName } from "../chat/chat-helpers";
import { diffLineClass, parseGitDiffSections, type GitDiffSection } from "../git/diff-parser";
import { preloadOptionalMonaco } from "../files/WorkspaceFileEditorPanel";
import { useI18n } from "../../shared/i18n";
import {
  Button,
  Card,
  Checkbox,
  Dropdown,
  Label,
  ListBox,
  Modal,
  Select,
  Tabs,
  TextArea,
  TextField,
} from "../../shared/ui";
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
  thinkingLevelOptionsForModel,
} from "../../shared/thinking-levels";
import { moveItemId, sameStringList } from "../workspaces/workspace-helpers";

export type ContextPanelTab = "todo" | "plan" | "files" | "git" | "memory" | "stats" | "agents" | "spec";

type PanelNumberSetter = (value: SetStateAction<number>) => void;

export type PlanPhaseRetryOverride = {
  modelId: string;
  providerId: string;
  thinkingLevel: string | null;
};

type PlanAction = "mark_complete" | "pause" | "resume" | "retry_merge" | "start";

const PLAN_RETRY_DEFAULT_OPTION_KEY = "__foco-plan-retry-default__";

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
  sourceControlTargetKey,
  sourceControlTargets,
  expandedFileTreePaths,
  isLoadingChatStatistics,
  isLoadingContextMemories,
  isLoadingPlans,
  isPlanAutoRunBusy,
  isPlanAutoRunEnabled,
  planAutoRunBlockedReason,
  isPlanAutoRunToggleDisabled,
  runtimeToolStateCompressionEnabled,
  loadingWorkspaceDirectoryPaths,
  isLoadingDiff,
  isLoadingTodoGraph,
  isLoadingWorkspaceSpec,
  isLoadingWorkspaceFiles,
  onDeletePlan,
  onForgetContextMemory,
  onGenerateGitCommitMessage,
  onGenerateWorkspaceSpec,
  onGitCommit,
  onGitCommitMessageChange,
  onGitFileOperation,
  onMemoryPageChange,
  onPlanAction,
  onPlanAutoRunToggle,
  onPlanOrderChange,
  onSourceControlTargetChange,
  onPlanPhaseRetry,
  onOpenPlanPhaseChat,
  onPlanPhaseRetryWithOverride,
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
  availableModels,
  plans,
  providers,
  thinkingLevels,
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
  sourceControlTargetKey: string;
  sourceControlTargets: { key: string; label: string; description: string }[];
  expandedFileTreePaths: Set<string>;
  isLoadingChatStatistics: boolean;
  isLoadingContextMemories: boolean;
  isLoadingPlans: boolean;
  isPlanAutoRunBusy: boolean;
  isPlanAutoRunEnabled: boolean;
  planAutoRunBlockedReason: string | null;
  isPlanAutoRunToggleDisabled: boolean;
  runtimeToolStateCompressionEnabled: boolean;
  loadingWorkspaceDirectoryPaths: Set<string>;
  isLoadingDiff: boolean;
  isLoadingTodoGraph: boolean;
  isLoadingWorkspaceSpec: boolean;
  isLoadingWorkspaceFiles: boolean;
  onDeletePlan: (planId: string) => void;
  onForgetContextMemory: (memory: MemoryFactRecord) => void;
  onGenerateGitCommitMessage: () => void;
  onGenerateWorkspaceSpec: () => void;
  onGitCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGitCommitMessageChange: (message: string) => void;
  onGitFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onMemoryPageChange: (scope: "global" | "workspace", page: number) => void;
  onPlanAction: (planId: string, action: PlanAction) => void;
  onPlanAutoRunToggle: (enabled: boolean) => void;
  onPlanOrderChange: (planIds: string[]) => void;
  onSourceControlTargetChange: (targetKey: string) => void;
  onPlanPhaseRetry: (
    planId: string,
    phaseId: string,
    implementationChatId: string | null,
  ) => void;
  onPlanPhaseRetryWithOverride: (
    planId: string,
    phaseId: string,
    implementationChatId: string | null,
    override: PlanPhaseRetryOverride,
  ) => void;
  onOpenPlanPhaseChat: (chatId: string) => void;
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
  availableModels: ConfiguredModelSummary[];
  plans: Plan[];
  providers: ConfiguredProviderSummary[];
  thinkingLevels: { label: string; value: string }[];
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
      <Tabs
        className="context-panel-tabs panel-scroll"
        onSelectionChange={(key) => onTabChange(key as ContextPanelTab)}
        selectedKey={activeTab}
      >
        <Tabs.ListContainer>
          <Tabs.List aria-label={t("Context panel sections")}>
            {tabs.map((tab) => {
              const Icon = tab.icon;
              const isActive = activeTab === tab.id;

              return (
                <Tabs.Tab
                  className={`context-panel-tab ${isActive ? "context-panel-tab-active" : ""}`}
                  id={tab.id}
                  key={tab.id}
                >
                  <Icon aria-hidden="true" className="size-3.5" />
                  <span>{t(tab.label)}</span>
                  <Tabs.Indicator />
                </Tabs.Tab>
              );
            })}
          </Tabs.List>
        </Tabs.ListContainer>
      </Tabs>

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
            autoRunBusy={isPlanAutoRunBusy}
            autoRunEnabled={isPlanAutoRunEnabled}
            autoRunBlockedReason={planAutoRunBlockedReason}
            autoRunToggleDisabled={isPlanAutoRunToggleDisabled}
            error={planError}
            isLoading={isLoadingPlans}
            onAction={onPlanAction}
            onAutoRunToggle={onPlanAutoRunToggle}
            onDeletePlan={onDeletePlan}
            onOpenPhaseChat={onOpenPlanPhaseChat}
            onOrderChange={onPlanOrderChange}
            onPhaseRetry={onPlanPhaseRetry}
            onPhaseRetryWithOverride={onPlanPhaseRetryWithOverride}
            operationKey={planOperationKey}
            availableModels={availableModels}
            plans={plans}
            providers={providers}
            thinkingLevels={thinkingLevels}
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
              sourceControlTargetKey={sourceControlTargetKey}
              sourceControlTargets={sourceControlTargets}
              isLoading={isLoadingDiff}
              onCommit={onGitCommit}
              onGenerateCommitMessage={onGenerateGitCommitMessage}
              onCommitMessageChange={onGitCommitMessageChange}
              onFileOperation={onGitFileOperation}
              onRefresh={onRefreshDiff}
              onSelectFile={onSelectDiffFile}
              onTargetChange={onSourceControlTargetChange}
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
            runtimeToolStateCompressionEnabled={runtimeToolStateCompressionEnabled}
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
      <div className="flex items-center justify-between gap-3 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]">
            <Files aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Files")}</h2>
            <p className="truncate text-xs font-medium text-[var(--muted)]">
              {t("Workspace file tree")}
            </p>
          </div>
        </div>
        <Button
          aria-label={t("Refresh files")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:opacity-60"
          isDisabled={isLoading}
          onPress={onRefresh}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </Button>
      </div>

      {error ? (
        <div className="mx-4 mt-3 rounded-xl border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-xs font-medium text-[var(--danger)]">
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
          <div className="rounded-xl border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_60%,transparent)] px-3 py-4 text-sm text-[var(--muted)]">
            {isLoading ? t("Loading files…") : t("No files")}
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
        <Button
          aria-label={isExpanded ? t("Collapse folder") : t("Expand folder")}
          className="workspace-file-tree-toggle"
          isDisabled={!isDirectory}
          isIconOnly
          onPress={() => {
            if (isDirectory) {
              void onTogglePath(node);
            }
          }}
          size="sm"
          type="button"
          variant="ghost"
        >
          {isDirectory ? (
            isExpanded ? (
              <ChevronDown aria-hidden="true" className="size-3.5" />
            ) : (
              <ChevronRight aria-hidden="true" className="size-3.5" />
            )
          ) : null}
        </Button>
        {isDirectory ? (
          <Folder aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-folder-icon" />
        ) : (
          <FileText aria-hidden="true" className="workspace-file-tree-icon workspace-file-tree-file-icon" />
        )}
        <span className="workspace-file-tree-name" title={node.path || node.name}>
          {node.name}
        </span>
        {isBusy || isLoadingDirectory ? <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin text-[var(--muted)]" /> : null}
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
      <div className="flex items-center justify-between gap-3 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-[var(--warning-soft)] text-[var(--warning)]">
            <ListChecks aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("ToDo graph")}</h2>
            <p className="truncate text-xs font-medium text-[var(--muted)]">
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
            className="size-4 shrink-0 animate-spin text-[var(--muted)]"
          />
        ) : null}
      </div>
      {error ? (
        <div className="border-b border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--danger)]">
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
  autoRunBusy,
  autoRunEnabled,
  autoRunBlockedReason,
  autoRunToggleDisabled,
  error,
  isLoading,
  onAction,
  onAutoRunToggle,
  onDeletePlan,
  onOpenPhaseChat,
  onOrderChange,
  onPhaseRetry,
  onPhaseRetryWithOverride,
  operationKey,
  availableModels,
  plans,
  providers,
  thinkingLevels,
}: {
  autoRunBusy: boolean;
  autoRunEnabled: boolean;
  autoRunBlockedReason: string | null;
  autoRunToggleDisabled: boolean;
  error: string | null;
  isLoading: boolean;
  onAction: (planId: string, action: PlanAction) => void;
  onAutoRunToggle: (enabled: boolean) => void;
  onDeletePlan: (planId: string) => void;
  onOpenPhaseChat: (chatId: string) => void;
  onOrderChange: (planIds: string[]) => void;
  onPhaseRetry: (
    planId: string,
    phaseId: string,
    implementationChatId: string | null,
  ) => void;
  onPhaseRetryWithOverride: (
    planId: string,
    phaseId: string,
    implementationChatId: string | null,
    override: PlanPhaseRetryOverride,
  ) => void;
  operationKey: string | null;
  availableModels: ConfiguredModelSummary[];
  plans: Plan[];
  providers: ConfiguredProviderSummary[];
  thinkingLevels: { label: string; value: string }[];
}) {
  const { language, t } = useI18n();
  const [expandedPhaseKeys, setExpandedPhaseKeys] = useState<Set<string>>(
    () => new Set(),
  );
  const [overrideRetryPhase, setOverrideRetryPhase] = useState<{
    plan: Plan;
    phase: PlanPhase;
  } | null>(null);
  const showAutoRunBusy = autoRunEnabled && autoRunBusy;
  const autoRunBlockedLabel = autoRunBlockedReason
    ? t(
        autoRunBlockedReason === "waiting_for_ready"
          ? "Auto run waiting for plan readiness"
          : autoRunBlockedReason === "waiting_for_retry"
            ? "Auto run paused until phase retry"
            : autoRunBlockedReason === "cancelled_phase"
              ? "Auto run paused until cancelled phase retry"
              : autoRunBlockedReason === "merge_blocked"
                ? "Auto run paused until merge retry"
                : "Auto run paused after a scheduler error",
      )
    : null;
  const runningPlan = plans.find((plan) => plan.status === "running") ?? null;
  const runningPlanId = runningPlan?.id ?? null;
  const runningPlanArticleRef = useRef<HTMLElement | null>(null);
  const planListPanelRef = useRef<HTMLDivElement | null>(null);
  const lastScrolledRunningPlanId = useRef<string | null>(null);
  const [draggedPlanId, setDraggedPlanId] = useState<string | null>(null);
  const [planOrderPreview, setPlanOrderPreview] = useState<string[] | null>(null);
  const planOrderPreviewRef = useRef<string[] | null>(null);
  const planOrderDropHandledRef = useRef(false);
  const orderedPlans = useMemo(
    () => (planOrderPreview ? reorderPlansByIds(plans, planOrderPreview) : plans),
    [planOrderPreview, plans],
  );

  const clearPlanOrderDrag = () => {
    setDraggedPlanId(null);
    setPlanOrderPreview(null);
    planOrderPreviewRef.current = null;
    planOrderDropHandledRef.current = false;
  };

  const commitPlanOrderPreview = () => {
    const previewIds = planOrderPreviewRef.current;
    if (!previewIds) {
      clearPlanOrderDrag();
      return;
    }
    if (!sameStringList(previewIds, reorderablePlanIds(plans))) {
      onOrderChange(previewIds);
    }
    clearPlanOrderDrag();
  };

  const handlePlanDragStart = (event: DragEvent<HTMLButtonElement>, plan: Plan) => {
    if (!isPlanReorderable(plan)) {
      return;
    }
    const planIds = reorderablePlanIds(plans);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", plan.id);
    setDraggedPlanId(plan.id);
    setPlanOrderPreview(planIds);
    planOrderPreviewRef.current = planIds;
    planOrderDropHandledRef.current = false;
  };

  const handlePlanDragOver = (event: DragEvent<HTMLElement>, targetPlan: Plan) => {
    const sourcePlanId = draggedPlanId;
    if (!sourcePlanId || !isPlanReorderable(targetPlan)) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const currentPlanIds = planOrderPreviewRef.current ?? reorderablePlanIds(plans);
    const nextPlanIds = moveItemId(currentPlanIds, sourcePlanId, targetPlan.id);
    if (sameStringList(nextPlanIds, currentPlanIds)) {
      return;
    }
    planOrderPreviewRef.current = nextPlanIds;
    setPlanOrderPreview(nextPlanIds);
  };

  const handlePlanDrop = (event: DragEvent<HTMLElement>, targetPlan: Plan) => {
    if (!draggedPlanId || !isPlanReorderable(targetPlan)) {
      return;
    }
    event.preventDefault();
    planOrderDropHandledRef.current = true;
    commitPlanOrderPreview();
  };

  const handlePlanDragEnd = () => {
    if (!planOrderDropHandledRef.current) {
      commitPlanOrderPreview();
      return;
    }
    clearPlanOrderDrag();
  };

  useEffect(() => {
    if (!runningPlanId) {
      lastScrolledRunningPlanId.current = null;
      return;
    }
    if (lastScrolledRunningPlanId.current === runningPlanId) {
      return;
    }

    const animationFrameId = window.requestAnimationFrame(() => {
      const planListPanel = planListPanelRef.current;
      const runningPlanArticle = runningPlanArticleRef.current;
      if (!planListPanel || !runningPlanArticle) {
        return;
      }

      const containerRect = planListPanel.getBoundingClientRect();
      const articleRect = runningPlanArticle.getBoundingClientRect();
      // ponytail: one-shot centering for the plain list; switch to the virtual list API if this panel virtualizes.
      planListPanel.scrollTop = Math.max(
        0,
        planListPanel.scrollTop +
          articleRect.top -
          containerRect.top -
          (planListPanel.clientHeight - articleRect.height) / 2,
      );
      lastScrolledRunningPlanId.current = runningPlanId;
    });

    return () => window.cancelAnimationFrame(animationFrameId);
  }, [runningPlanId]);

  const locateRunningPlan = () => {
    const planListPanel = planListPanelRef.current;
    const runningPlanArticle = runningPlanArticleRef.current;
    if (!planListPanel || !runningPlanArticle) {
      return;
    }
    const containerRect = planListPanel.getBoundingClientRect();
    const articleRect = runningPlanArticle.getBoundingClientRect();
    planListPanel.scrollTop = Math.max(
      0,
      planListPanel.scrollTop +
        articleRect.top -
        containerRect.top -
        (planListPanel.clientHeight - articleRect.height) / 2,
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-3 py-2">
        <Checkbox
          className="flex min-w-0 items-start gap-2 text-xs text-[var(--muted)]"
          isDisabled={autoRunToggleDisabled}
          isSelected={autoRunEnabled}
          onChange={onAutoRunToggle}
        >
          <Checkbox.Content>
            <Checkbox.Control className="mt-0.5 size-3.5 shrink-0 rounded border-[var(--border)]">
              <Checkbox.Indicator />
            </Checkbox.Control>
            <span className="min-w-0">
              <span className="block truncate font-semibold text-[var(--foreground)]">
                {t("Auto run plans")}
              </span>
              <span className="block truncate text-[var(--muted)]">
                {t("Run every active plan in order")}
              </span>
            </span>
          </Checkbox.Content>
        </Checkbox>
        {showAutoRunBusy ? (
          <span className="inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full border border-[var(--warning)] bg-[var(--warning-soft)] px-2 text-xs font-medium text-[var(--warning)]">
            <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />
            {t("Auto running")}
          </span>
        ) : autoRunEnabled && autoRunBlockedLabel ? (
          <span className="inline-flex min-h-6 shrink-0 items-center rounded-full border border-[var(--border)] bg-[var(--surface-secondary)] px-2 py-1 text-xs font-medium text-[var(--muted)]">
            {autoRunBlockedLabel}
          </span>
        ) : null}
        <Button
          aria-label={t("Locate running plan")}
          className="plan-locate-button inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
          isDisabled={!runningPlanId}
          onPress={locateRunningPlan}
          type="button"
        >
          <LocateFixed aria-hidden="true" className="size-3.5" />
          <span className="plan-locate-button-label">{t("Locate")}</span>
        </Button>
      </div>

      <div className="context-list-panel panel-scroll" ref={planListPanelRef}>
        {isLoading && plans.length === 0 ? (
          <div className="context-empty-state">
            <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
            <h2>{t("Plan")}</h2>
            <p>{t("Loading plans…")}</p>
          </div>
        ) : null}

        {error && plans.length === 0 ? (
          <div className="context-empty-state">
            <ScrollText aria-hidden="true" className="size-5" />
            <h2>{t("Plan")}</h2>
            <p>{error}</p>
          </div>
        ) : null}

        {!isLoading && !error && plans.length === 0 ? (
          <div className="context-empty-state">
            <ScrollText aria-hidden="true" className="size-5" />
            <h2>{t("Plan")}</h2>
            <p>{t("No active plans for this workspace.")}</p>
          </div>
        ) : null}

        {error && plans.length > 0 ? (
          <div className="mb-3 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-xs font-medium text-[var(--danger)]">
            {error}
          </div>
        ) : null}

        {plans.length > 0 ? (
          <div className="space-y-3">
            {orderedPlans.map((plan) => {
              const totalSteps = plan.phases.reduce(
                (count, phase) => count + phase.steps.length,
                0,
              );
              const completedSteps = plan.phases.reduce(
                (count, phase) =>
                  count + phase.steps.filter((step) => step.status === "completed").length,
                0,
              );
              const earliestIncompletePhase = earliestIncompletePlanPhase(plan);
              const cancelledBarrierPhase =
                earliestIncompletePhase?.status === "cancelled"
                  ? earliestIncompletePhase
                  : null;
              const action = primaryPlanAction(plan);
              const actionKey = action ? `${action}:${plan.id}` : null;
              const mergedCommitId = planMergedIntoSharedWorkspace(plan);
              const mergeInProgress = planMergeInProgress(plan);
              const mergeChatId = mergeInProgress?.implementationChatId ?? null;
              const canRetryMerge = planNeedsMergeRetry(plan);
              const retryMergeKey = planRetryMergeOperationKey(plan.id);
              const isRetryingMerge = operationKey === retryMergeKey;
              const canReorderPlan = isPlanReorderable(plan);

              return (
                <article
                  className={`context-memory-item ${draggedPlanId === plan.id ? "opacity-60" : ""}`}
                  key={plan.id}
                  onDragOver={canReorderPlan ? (event) => handlePlanDragOver(event, plan) : undefined}
                  onDrop={canReorderPlan ? (event) => handlePlanDrop(event, plan) : undefined}
                  ref={plan.id === runningPlanId ? runningPlanArticleRef : undefined}
                >
                  <div className="context-memory-item-header">
                    <div className="context-memory-badges">
                      <span className={planStatusClass(plan.status)}>
                        {t(planStatusLabel(plan.status))}
                      </span>
                      <span className="context-memory-kind">
                        {completedSteps}/{totalSteps}
                      </span>
                      {mergedCommitId ? (
                        <>
                          <span
                            aria-describedby={`plan-merge-status-${plan.id}`}
                            className="context-memory-pin inline-flex items-center gap-1"
                          >
                            <CheckCircle2 aria-hidden="true" className="size-3" />
                            {mergedCommitId}
                          </span>
                          <span
                            className="sr-only"
                            id={`plan-merge-status-${plan.id}`}
                          >
                            {t("Merged into shared workspace")}
                          </span>
                        </>
                      ) : mergeInProgress ? (
                        <>
                          <span
                            className="sr-only"
                            id={`plan-merge-in-progress-status-${plan.id}`}
                          >
                            {t("Merging")}
                          </span>
                          {mergeChatId ? (
                            <span title={t("Open merge chat")}>
                              <Button
                                aria-describedby={`plan-merge-in-progress-status-${plan.id}`}
                                aria-label={t("Open merge chat")}
                                className="context-memory-pin inline-flex items-center gap-1 hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                onPress={() => onOpenPhaseChat(mergeChatId)}
                                type="button"
                              >
                                <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />
                                {t("Merging")}
                              </Button>
                            </span>
                          ) : (
                            <span
                              aria-describedby={`plan-merge-in-progress-status-${plan.id}`}
                              className="context-memory-pin inline-flex items-center gap-1"
                              title={t("Merging")}
                            >
                              <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />
                              {t("Merging")}
                            </span>
                          )}
                        </>
                      ) : canRetryMerge ? (
                        <>
                          <span
                            className="sr-only"
                            id={`plan-merge-retry-hint-${plan.id}`}
                          >
                            {t(planMergeRetryHint(plan))}
                          </span>
                          <Button
                            aria-describedby={`plan-merge-retry-hint-${plan.id}`}
                            aria-label={t("Retry Merge")}
                            className="context-memory-pin inline-flex items-center gap-1 hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                            isDisabled={operationKey !== null}
                            onPress={() => onAction(plan.id, "retry_merge")}
                            type="button"
                          >
                            {isRetryingMerge ? (
                              <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />
                            ) : (
                              <RefreshCw aria-hidden="true" className="size-3" />
                            )}
                            {t("Retry Merge")}
                          </Button>
                        </>
                      ) : null}
                    </div>
                    <div className="flex shrink-0 items-center gap-1.5">
                      {/* Native drag events must stay on this control: HeroUI Button deliberately
                          omits `draggable` and drag handlers, while plan ordering depends on them. */}
                      {canReorderPlan ? (
                        <button
                          aria-label={t("Reorder plan")}
                          className="inline-flex size-8 shrink-0 cursor-grab items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] active:cursor-grabbing disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                          data-heroui-exception="native-plan-drag"
                          draggable
                          onDragEnd={handlePlanDragEnd}
                          onDragStart={(event) => handlePlanDragStart(event, plan)}
                          title={t("Reorder plan")}
                          type="button"
                        >
                          <GripVertical aria-hidden="true" className="size-3.5" />
                        </button>
                      ) : null}
                      {action ? (
                        <Button
                          aria-label={t(planActionLabel(action))}
                          className="inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                          isDisabled={operationKey !== null}
                          onPress={() => onAction(plan.id, action)}
                          type="button"
                        >
                          {operationKey === actionKey ? (
                            <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                          ) : (
                            <CheckCircle2 aria-hidden="true" className="size-3.5" />
                          )}
                          {t(planActionLabel(action))}
                        </Button>
                      ) : null}
                      <Button
                        aria-label={t("Delete plan")}
                        className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        isDisabled={operationKey !== null}
                        onPress={() => onDeletePlan(plan.id)}
                        type="button"
                      >
                        {operationKey === `delete:${plan.id}` ? (
                          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                        ) : (
                          <Trash2 aria-hidden="true" className="size-3.5" />
                        )}
                      </Button>
                    </div>
                  </div>
                  <h3 className="break-words text-sm font-semibold text-[var(--foreground)]">
                    {plan.title}
                  </h3>
                  <p>{plan.overview}</p>
                  {cancelledBarrierPhase ? (
                    <div className="mt-2 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-2 py-1.5 text-xs text-[var(--warning)]">
                      <span className="font-semibold">{cancelledBarrierPhase.title}</span>
                      {": "}
                      {t("Retry the cancelled phase to continue this plan.")}
                    </div>
                  ) : null}
                  {plan.errorMessage ? (
                    <div
                      className="mt-2 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-2 py-1.5 text-xs text-[var(--danger)]"
                    >
                      {plan.errorMessage}
                    </div>
                  ) : null}
                  <small>
                    {t("Updated {time}", {
                      time: formatTodoGraphDate(plan.updatedAt, language),
                    })}
                  </small>
                  <div className="mt-3 space-y-2">
                    {plan.phases.map((phase) => {
                      const phaseKey = `${plan.id}:${phase.id}`;
                      const isExpanded = expandedPhaseKeys.has(phaseKey);
                      const canRetryPhase = isRetryablePlanPhase(phase);
                      const retryOperationKey = planPhaseRetryOperationKey(plan.id, phase.id);
                      const isRetrying = operationKey === retryOperationKey;
                      const implementationChatId = phase.implementationChatId;

                      return (
                        <section
                          className="rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_80%,transparent)] px-2.5 py-2"
                          key={phase.id}
                        >
                          <div className="flex min-w-0 items-start justify-between gap-2">
                            <Button
                              aria-expanded={isExpanded}
                              className="flex min-w-0 flex-1 items-start gap-2 text-left"
                              onPress={() => {
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
                              <ChevronRight
                                aria-hidden="true"
                                className={`mt-0.5 size-3.5 shrink-0 text-[var(--muted)] transition-transform ${
                                  isExpanded ? "rotate-90" : ""
                                }`}
                              />
                              <div className="min-w-0">
                                <div className="truncate text-xs font-semibold text-[var(--foreground)]">
                                  {phase.title}
                                </div>
                                {phase.summary ? (
                                  <div className="mt-0.5 line-clamp-2 text-xs text-[var(--muted)]">
                                    {phase.summary}
                                  </div>
                                ) : null}
                              </div>
                            </Button>
                            <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
                              <span className={planPhaseStatusClass(phase.status)}>
                                {t(planPhaseStatusLabel(phase.status))}
                              </span>
                              {implementationChatId ? (
                                <Button
                                  aria-label={t("Open implementation chat")}
                                  className="inline-flex size-7 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                  onPress={() => onOpenPhaseChat(implementationChatId)}
                                  type="button"
                                >
                                  <MessageSquare aria-hidden="true" className="size-3.5" />
                                </Button>
                              ) : null}
                              {canRetryPhase ? (
                                <div className="relative inline-flex h-7 shrink-0 rounded-md shadow-sm">
                                  <Button
                                    aria-label={t("Retry plan phase")}
                                    className="inline-flex h-7 max-w-[7rem] items-center justify-center gap-1 rounded-l-md border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                    isDisabled={operationKey !== null}
                                    onPress={() => {
                                      onPhaseRetry(
                                        plan.id,
                                        phase.id,
                                        phase.implementationChatId,
                                      );
                                    }}
                                    type="button"
                                  >
                                    {isRetrying ? (
                                      <LoaderCircle aria-hidden="true" className="size-3.5 shrink-0 animate-spin" />
                                    ) : (
                                      <RefreshCw aria-hidden="true" className="size-3.5 shrink-0" />
                                    )}
                                    <span className="truncate">
                                      {isRetrying ? t("Retrying…") : t("Retry")}
                                    </span>
                                  </Button>
                                  <Dropdown>
                                    <Button
                                      aria-label={t("Retry phase options")}
                                      className="inline-flex h-7 w-7 items-center justify-center rounded-r-md border border-l-0 border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                      isDisabled={operationKey !== null}
                                      type="button"
                                    >
                                      <ChevronDown aria-hidden="true" className="size-3.5" />
                                    </Button>
                                    <Dropdown.Popover className="min-w-40">
                                      <Dropdown.Menu
                                        aria-label={t("Retry phase options")}
                                        onAction={() => setOverrideRetryPhase({ plan, phase })}
                                      >
                                        <Dropdown.Item
                                          id="retry-with-another-model"
                                          textValue={t("Retry with another model")}
                                        >
                                          <Bot aria-hidden="true" className="size-3.5 shrink-0" />
                                          <Label>{t("Retry with another model…")}</Label>
                                        </Dropdown.Item>
                                      </Dropdown.Menu>
                                    </Dropdown.Popover>
                                  </Dropdown>
                                </div>
                              ) : null}
                            </div>
                          </div>
                          {isExpanded ? (
                            <div className="mt-2 space-y-2 pl-5">
                              {phase.errorMessage ? (
                                <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-2 py-1.5 text-xs text-[var(--danger)]">
                                  {phase.errorMessage}
                                </div>
                              ) : null}
                              {phase.implementationChatId ? (
                                <div className="flex min-w-0 items-center gap-1.5 text-xs text-[var(--muted)]">
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
        ) : null}
      </div>
      {overrideRetryPhase ? (
        <PlanPhaseRetryDialog
          availableModels={availableModels}
          isSubmitting={operationKey !== null}
          onClose={() => setOverrideRetryPhase(null)}
          onSubmit={(override) => {
            const phase = overrideRetryPhase.phase;
            onPhaseRetryWithOverride(
              overrideRetryPhase.plan.id,
              phase.id,
              phase.implementationChatId,
              override,
            );
            setOverrideRetryPhase(null);
          }}
          phase={overrideRetryPhase.phase}
          providers={providers}
          thinkingLevels={thinkingLevels}
        />
      ) : null}
    </div>
  );
}

function PlanPhaseRetryDialog({
  availableModels,
  isSubmitting,
  onClose,
  onSubmit,
  phase,
  providers,
  thinkingLevels,
}: {
  availableModels: ConfiguredModelSummary[];
  isSubmitting: boolean;
  onClose: () => void;
  onSubmit: (override: PlanPhaseRetryOverride) => void;
  phase: PlanPhase;
  providers: ConfiguredProviderSummary[];
  thinkingLevels: { label: string; value: string }[];
}) {
  const { t } = useI18n();
  const selectableModels = useMemo(
    () => availableModels.filter((model) => model.enabled && model.canEnable),
    [availableModels],
  );
  const [modelId, setModelId] = useState(() =>
    defaultPlanPhaseRetryModelId(phase, selectableModels),
  );
  const selectedModel =
    selectableModels.find((model) => model.id === modelId) ?? null;
  const thinkingOptions = useMemo(
    () => thinkingLevelOptionsForModel(selectedModel, thinkingLevels),
    [selectedModel, thinkingLevels],
  );
  const resolvedProviderId =
    selectedModel?.activeProviderId &&
    providers.some(
      (provider) => provider.id === selectedModel.activeProviderId && provider.enabled,
    )
      ? selectedModel.activeProviderId
      : selectedModel?.providerIds.find((id) =>
          providers.some((provider) => provider.id === id && provider.enabled),
        ) ?? "";
  const supportsThinking = thinkingOptions.length > 0;
  const [thinkingLevel, setThinkingLevel] = useState(() =>
    defaultThinkingLevelForModel(selectedModel),
  );
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    const nextModel = selectableModels.find((model) => model.id === modelId) ?? null;
    if (!nextModel) {
      const fallbackModelId = defaultPlanPhaseRetryModelId(phase, selectableModels);
      setModelId(fallbackModelId);
      return;
    }
    if (thinkingLevel && !isModelThinkingLevelSupported(nextModel, thinkingLevel)) {
      setThinkingLevel(defaultThinkingLevelForModel(nextModel));
    }
  }, [modelId, phase, selectableModels, thinkingLevel]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const submitRetry = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!modelId || !resolvedProviderId) {
      setFormError(t("Select an enabled model before retrying."));
      return;
    }
    onSubmit({
      modelId,
      providerId: resolvedProviderId,
      thinkingLevel: isModelThinkingLevelSupported(selectedModel, thinkingLevel)
        ? thinkingLevel
        : null,
    });
  };

  return (
    <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && onClose()}>
      <Modal.Container placement="center" size="md">
        <Modal.Dialog aria-label={t("Retry with another model")}>
          <Modal.CloseTrigger aria-label={t("Close retry dialog")} />
          <Modal.Header>
            <Modal.Icon className="bg-accent-soft text-accent-soft-foreground">
              <RefreshCw aria-hidden="true" className="size-4" />
            </Modal.Icon>
            <div className="min-w-0">
              <Modal.Heading>{t("Retry with another model")}</Modal.Heading>
              <p className="truncate text-xs text-[var(--muted)]">{phase.title}</p>
            </div>
          </Modal.Header>
          <form className="space-y-4" onSubmit={submitRetry}>
            <Modal.Body className="space-y-4">
          <PlanRetrySelect
            autoFocus
            disabled={isSubmitting || selectableModels.length === 0}
            label={t("Model")}
            onChange={(value) => {
              const nextModel = selectableModels.find((model) => model.id === value) ?? null;
              setModelId(value);
              setFormError(null);
              setThinkingLevel(defaultThinkingLevelForModel(nextModel));
            }}
            options={[
              { label: t("Select model"), value: "" },
              ...selectableModels.map((model) => ({
                label: model.displayName,
                value: model.id,
              })),
            ]}
            value={modelId}
          />
          {supportsThinking ? (
            <PlanRetrySelect
              disabled={isSubmitting}
              label={t("Thinking level")}
              onChange={setThinkingLevel}
              options={[
                { label: t("Model default"), value: "" },
                ...thinkingOptions.map((level) => ({
                  label: t(level.label),
                  value: level.value,
                })),
              ]}
              value={thinkingLevel}
            />
          ) : null}
          <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
            {t("Scope: this retry only")}
          </div>
          {formError ? (
            <div className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
              {formError}
            </div>
          ) : null}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="tertiary" onPress={onClose}>
                {t("Cancel")}
              </Button>
              <Button
                isDisabled={isSubmitting || !modelId || !resolvedProviderId}
                type="submit"
              >
                {isSubmitting ? (
                  <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-3.5" />
                )}
                {isSubmitting ? t("Retrying…") : t("Retry")}
              </Button>
            </Modal.Footer>
          </form>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function PlanRetrySelect({
  autoFocus,
  disabled,
  label,
  onChange,
  options,
  value,
}: {
  autoFocus?: boolean;
  disabled?: boolean;
  label: string;
  onChange: (value: string) => void;
  options: { label: string; value: string }[];
  value: string;
}) {
  return (
    <Select
      aria-label={label}
      autoFocus={autoFocus}
      isDisabled={disabled}
      selectedKey={value || PLAN_RETRY_DEFAULT_OPTION_KEY}
      onSelectionChange={(key) =>
        onChange(
          key === PLAN_RETRY_DEFAULT_OPTION_KEY ? "" : String(key ?? ""),
        )
      }
    >
      <Label>{label}</Label>
      <Select.Trigger className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)]">
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {options.map((option) => (
            <ListBox.Item
              id={option.value || PLAN_RETRY_DEFAULT_OPTION_KEY}
              key={option.value || PLAN_RETRY_DEFAULT_OPTION_KEY}
              textValue={option.label}
            >
              {option.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}

function defaultPlanPhaseRetryModelId(
  phase: PlanPhase,
  models: ConfiguredModelSummary[],
) {
  const lastModelId = [...phase.attempts]
    .reverse()
    .find((attempt) => attempt.modelId)?.modelId;
  if (lastModelId && models.some((model) => model.id === lastModelId)) {
    return lastModelId;
  }
  return models[0]?.id ?? "";
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
            ? "border-[var(--accent)] bg-[var(--accent)] text-white"
            : "border-[var(--border)] bg-[var(--surface)] text-transparent"
        }`}
      >
        <CheckCircle2 className="size-3" />
      </span>
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className={`min-w-0 break-words font-medium ${
              isComplete ? "text-[var(--muted)] line-through" : "text-[var(--foreground)]"
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
          <div className="mt-0.5 whitespace-pre-wrap text-[var(--muted)]">{step.detail}</div>
        ) : null}
        {step.acceptance.length ? (
          <ul className="mt-1 list-disc space-y-0.5 pl-4 text-[var(--muted)]">
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
          <p>{t("Loading…")}</p>
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
                <Button
                  aria-label={t("Delete memory")}
                  className="context-memory-delete-button"
                  isDisabled={deletingMemoryId === memory.id}
                  onPress={() => onForgetMemory(memory)}
                  type="button"
                >
                  {deletingMemoryId === memory.id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" />
                  ) : (
                    <Trash2 aria-hidden="true" />
                  )}
                </Button>
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
                <Button
                  aria-label={t("Previous page")}
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                  isDisabled={meta.page <= 1}
                  onPress={() => onPageChange(meta.page - 1)}
                  type="button"
                >
                  <ChevronLeft aria-hidden="true" className="size-4" />
                </Button>
                {paginationItems.map((item, index) =>
                  item === "ellipsis" ? (
                    <span
                      aria-hidden="true"
                      className="context-memory-pagination-control context-memory-pagination-ellipsis inline-flex items-center justify-center text-[var(--muted)]"
                      key={`cm-ellipsis-${index}`}
                    >
                      ...
                    </span>
                  ) : (
                    <Button
                      aria-current={
                        item === meta.page ? "page" : undefined
                      }
                      aria-label={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      className={`context-memory-pagination-control inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                        item === meta.page
                          ? "border-[var(--accent)] bg-[var(--accent)] text-white"
                          : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                        }`}
                      key={item}
                      onPress={() => onPageChange(item)}
                      type="button"
                    >
                      {formatNumber(item, language)}
                    </Button>
                  ),
                )}
                <Button
                  aria-label={t("Next page")}
                  className="context-memory-pagination-control inline-flex items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                  isDisabled={meta.totalPages === 0 || meta.page >= meta.totalPages}
                  onPress={() => onPageChange(meta.page + 1)}
                  type="button"
                >
                  <ChevronRight aria-hidden="true" className="size-4" />
                </Button>
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
        <p>{t("Loading…")}</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--background-secondary)]">
      <div className="flex items-center justify-between gap-3 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]">
            <ScrollText aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Project Spec")}</h2>
            <p className="truncate text-xs font-medium text-[var(--muted)]">
              {spec
                ? `${t("Revision")} ${formatNumber(spec.revision, language)}`
                : t("Workspace spec")}
            </p>
          </div>
        </div>
        <Button
          aria-label={t("Reload spec")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-[var(--muted)] hover:bg-[var(--default)]/80 hover:text-[var(--foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
          isDisabled={isLoading}
          onPress={onReload}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden px-3 py-3">
        {error ? (
          <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-xs font-medium text-[var(--danger)]">
            {error}
          </div>
        ) : null}

        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_2.25rem_2.25rem_2.25rem] items-center gap-2">
          <Button
            className="inline-flex min-h-9 min-w-0 items-center justify-center gap-2 rounded-md bg-[var(--accent)] px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--accent)]/45 disabled:text-white/70"
            isDisabled={!enabled || isBusy || isLoading}
            onPress={onGenerate}
            type="button"
          >
            {operationKey === "generate" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Sparkles aria-hidden="true" className="size-4" />
            )}
            <span className="truncate">{generateLabel}</span>
          </Button>
          <Button
            aria-label={t("Save")}
            className="inline-flex size-9 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
            isDisabled={!canEdit || !isDirty || isBusy || isLoading}
            onPress={onSave}
            type="button"
          >
            {operationKey === "save" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Save aria-hidden="true" className="size-4" />
            )}
          </Button>
          <Button
            aria-label={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
            aria-pressed={previewEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm ${previewEnabled
                ? "border-[var(--accent)] bg-[var(--accent)] text-white hover:bg-[var(--accent)]"
                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
              }`}
            onPress={() => onPreviewChange(!previewEnabled)}
            type="button"
          >
            {previewEnabled ? (
              <EyeOff aria-hidden="true" className="size-4" />
            ) : (
              <Eye aria-hidden="true" className="size-4" />
            )}
          </Button>
          <Button
            aria-label={t("Inject into new chats")}
            aria-pressed={injectEnabled}
            className={`inline-flex size-9 items-center justify-center rounded-md border shadow-sm disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)] ${injectEnabled
                ? "border-[var(--accent)] bg-[var(--accent)] text-white hover:bg-[var(--accent)]"
                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
              }`}
            isDisabled={!enabled || isBusy || isLoading}
            onPress={() => onSettingsChange(enabled, !injectEnabled)}
            type="button"
          >
            {operationKey === "settings" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <MessageSquare aria-hidden="true" className="size-4" />
            )}
          </Button>
        </div>

        {conflictMessage ? (
          <div className="rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-xs font-medium text-[var(--warning)]">
            <div>{conflictMessage}</div>
            <Button
              className="mt-2 inline-flex items-center gap-2 rounded-md border border-[var(--warning)] bg-[var(--surface)] px-2.5 py-1.5 text-xs font-semibold text-[var(--warning)] hover:bg-[var(--warning-soft)]"
              onPress={onReload}
              type="button"
            >
              <RefreshCw
                aria-hidden="true"
                className="context-refresh-icon size-3.5"
                data-loading={isLoading ? "true" : undefined}
              />
              {t("Reload spec")}
            </Button>
          </div>
        ) : null}

        <div className="min-h-0 flex-1">
          {previewEnabled ? (
            <div className="h-full min-h-0 overflow-y-auto rounded-md border border-[var(--border)] bg-[var(--surface)] px-4 py-3">
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
            <TextField
              aria-label={t("Project Spec Markdown")}
              className="h-full"
              isDisabled={!canEdit || isLoading}
              value={contentDraft}
              onChange={onContentChange}
            >
              <TextArea
                className="h-full min-h-0 w-full resize-none rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-[13px] leading-5 text-[var(--foreground)] shadow-inner outline-none placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                placeholder={t("Generate or paste a Project Spec Markdown document.")}
              />
            </TextField>
          )}
        </div>

        <div className="hidden rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs leading-5 text-[var(--muted)] md:block">
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
                <div className="break-words text-[var(--danger)]">{latestJob.errorMessage}</div>
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
  runtimeToolStateCompressionEnabled,
  statistics,
}: {
  contextUsage: ContextUsageResponse | null;
  error: string | null;
  isLoading: boolean;
  runtimeToolStateCompressionEnabled: boolean;
  statistics: ChatStatisticsResponse | null;
}) {
  const { language, t } = useI18n();

  if (isLoading && !statistics) {
    return (
      <div className="context-empty-state">
        <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
        <h2>{t("Stats")}</h2>
        <p>{t("Loading…")}</p>
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
  const providerChart = statistics.providerBreakdown.map((item) => ({
    id: item.providerId,
    label: item.providerId,
    value: item.requestCount,
  }));
  const contextBreakdownBySource = Array.isArray(contextUsage?.tokenBreakdown?.bySource)
    ? contextUsage.tokenBreakdown.bySource
    : [];
  const contextChart = contextUsage
    ? contextBreakdownBySource
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
          <LoaderCircle aria-label={t("Loading…")} className="size-4 animate-spin" />
        ) : null}
      </div>

      <ContextUsageTimelinePanel
        contextUsage={contextUsage}
        runtimeToolStateCompressionEnabled={runtimeToolStateCompressionEnabled}
      />

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

      <ContextStatsSection title={t("Provider calls")}>
        <ContextMiniBarChart
          data={providerChart}
          emptyLabel={t("No provider calls yet.")}
          valueFormatter={(value) => formatNumber(value, language)}
        />
      </ContextStatsSection>

      <ContextStatsSection title={t("Context mix")}>
        {contextUsage ? (
          <ContextMiniBarChart
            data={contextChart}
            emptyLabel={t("No context usage yet.")}
            valueFormatter={(value) => formatNumber(value, language)}
          />
        ) : (
          <div className="context-empty-inline">{t("Context usage unavailable.")}</div>
        )}
      </ContextStatsSection>

      <ContextStatsSection title={t("Tools and compression")}>
        <ContextStatsRows
          emptyLabel={t("No tools used yet.")}
          rows={[
            ...statistics.toolBreakdown.map((item) => ({
              label: toolDisplayName(item.toolName, language),
              value: formatNumber(item.callCount, language),
            })),
            {
              label: t("LLM compression snapshots"),
              value: formatNumber(statistics.compression.llmSnapshotCount, language),
            },
            ...(runtimeToolStateCompressionEnabled
              ? [
                  {
                    label: t("Runtime tool-state snapshots"),
                    value: formatNumber(
                      statistics.compression.runtimeToolStateSnapshotCount,
                      language,
                    ),
                  },
                ]
              : []),
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

function ContextUsageTimelinePanel({
  contextUsage,
  runtimeToolStateCompressionEnabled,
}: {
  contextUsage: ContextUsageResponse | null;
  runtimeToolStateCompressionEnabled: boolean;
}) {
  const { t } = useI18n();

  if (!contextUsage?.contextWindow) {
    return (
      <section className="context-usage-timeline-panel">
        <div className="context-empty-inline">{t("Context usage unavailable.")}</div>
      </section>
    );
  }

  const currentEntry: ContextUsageBarEntry = {
    id: "current",
    label: t("Current"),
    usagePercent: contextUsage.usagePercent,
    contextWindow: contextUsage.contextWindow,
    totalUsedTokens: contextUsage.totalUsedContextTokens,
    segments: contextUsage.segments,
    toolCompressionTriggerPercent: runtimeToolStateCompressionEnabled
      ? contextUsage.compressionTriggerPercent
      : undefined,
    llmCompressionTriggerPercent: contextUsage.llmCompressionTriggerPercent,
  };
  return (
    <section className="context-usage-timeline-panel" aria-label={t("Context usage timeline")}>
      <ContextUsageBar entry={currentEntry} isCurrent />
      <div className="context-usage-legend" aria-label={t("Context usage legend")}>
        {CONTEXT_USAGE_SEGMENT_STYLES.map((segment) => (
          <span className="context-usage-legend-item" key={segment.key}>
            <span
              aria-hidden="true"
              className="context-usage-legend-swatch"
              style={{ backgroundColor: segment.color }}
            />
            {t(segment.label)}
          </span>
        ))}
      </div>
    </section>
  );
}

type ContextUsageBarEntry = {
  id: string;
  label: string;
  meta?: string;
  usagePercent?: number;
  contextWindow: number;
  totalUsedTokens: number;
  segments: ContextUsageSegments;
  toolCompressionTriggerPercent?: number;
  llmCompressionTriggerPercent?: number;
};

const CONTEXT_USAGE_SEGMENT_STYLES = [
  { key: "promptTools", label: "Prompt/tools", color: "#2563eb" },
  { key: "history", label: "History", color: "#16a34a" },
  { key: "compressionSnapshot", label: "Compression snapshot", color: "#7c3aed" },
] as const;

function ContextUsageBar({
  entry,
  isCurrent = false,
}: {
  entry: ContextUsageBarEntry;
  isCurrent?: boolean;
}) {
  const { language, t } = useI18n();
  const contextWindow = Math.max(entry.contextWindow, 1);
  const usedPercent = (entry.totalUsedTokens / contextWindow) * 100;
  const toolCompressionTriggerPercent =
    typeof entry.toolCompressionTriggerPercent === "number"
      ? entry.toolCompressionTriggerPercent
      : null;
  const llmCompressionTriggerPercent = entry.llmCompressionTriggerPercent ?? 95;
  const isPastToolTrigger =
    toolCompressionTriggerPercent != null && usedPercent >= toolCompressionTriggerPercent;
  const displayMeta =
    typeof entry.usagePercent === "number" ? `${entry.usagePercent}%` : entry.meta;
  let usedPercentCursor = 0;
  const rawSegments = {
    promptTools: entry.segments.systemPrompt + entry.segments.toolSchema,
    history: entry.segments.history,
    compressionSnapshot: entry.segments.compressionSnapshot,
  };
  const segmentParts = CONTEXT_USAGE_SEGMENT_STYLES.flatMap((segment) => {
    const tokens = rawSegments[segment.key];
    if (tokens <= 0 || usedPercentCursor >= 100) {
      return [];
    }
    const widthPercent = Math.min((tokens / contextWindow) * 100, 100 - usedPercentCursor);
    usedPercentCursor += widthPercent;
    return [{ ...segment, tokens, widthPercent }];
  });

  return (
    <div className={`context-usage-bar-row${isCurrent ? " is-current" : ""}`}>
      <div className="context-usage-bar-topline">
        {isPastToolTrigger && toolCompressionTriggerPercent != null ? (
          <strong>
            {t("Past {percent}%", { percent: toolCompressionTriggerPercent })}
          </strong>
        ) : null}
        {toolCompressionTriggerPercent != null ? (
          <span
            className="context-usage-bar-threshold is-tool-state"
            style={{ left: `${toolCompressionTriggerPercent}%` }}
          >
            {toolCompressionTriggerPercent}%
          </span>
        ) : null}
        <span
          className="context-usage-bar-threshold is-llm"
          style={{ left: `${llmCompressionTriggerPercent}%` }}
        >
          {llmCompressionTriggerPercent}%
        </span>
      </div>
      <div className="context-usage-bar-copy">
        <span className="context-usage-bar-label" title={entry.label}>
          {entry.label}
        </span>
        {displayMeta ? (
          <span className="context-usage-bar-meta" title={displayMeta}>
            {displayMeta}
          </span>
        ) : null}
      </div>
      <div
        className="context-usage-bar-track"
        title={`${formatNumber(entry.totalUsedTokens, language)} / ${formatNumber(
          entry.contextWindow,
          language,
        )}`}
      >
        {segmentParts.map((segment, index) => (
          <span
            aria-label={`${t(segment.label)}: ${formatNumber(segment.tokens, language)}`}
            className={`context-usage-bar-segment${index === 0 ? " is-first" : ""}${
              index === segmentParts.length - 1 ? " is-last" : ""
            }`}
            key={segment.key}
            style={{ backgroundColor: segment.color, width: `${segment.widthPercent}%` }}
          />
        ))}
        {toolCompressionTriggerPercent != null ? (
          <span
            className="context-usage-trigger-marker is-tool-state"
            aria-hidden="true"
            style={{ left: `${toolCompressionTriggerPercent}%` }}
          />
        ) : null}
        <span
          className="context-usage-trigger-marker is-llm"
          aria-hidden="true"
          style={{ left: `${llmCompressionTriggerPercent}%` }}
        />
      </div>
      <div className="context-usage-bar-footer">
        <span>{formatNumber(entry.totalUsedTokens, language)}</span>
      </div>
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
        <div className="context-mini-bar-row" key={item.id} title={`${item.label}: ${valueFormatter(item.value)}`}>
          <span className="context-mini-bar-label" title={item.label}>{item.label}</span>
          <span className="context-mini-bar-track">
            <span
              className="context-mini-bar-fill"
              style={{
                backgroundColor: chartColor(index),
                width: `${Math.max(2, (item.value / chartMax) * 100)}%`,
              }}
            />
          </span>
          <span className="context-mini-bar-value" title={valueFormatter(item.value)}>{valueFormatter(item.value)}</span>
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
      {rows.map((row, index) => (
        <div className="context-stats-row" key={`${row.label}-${index}`}>
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
      <Card
        className="todo-graph-task-card gap-0 overflow-hidden p-0"
        style={{ marginLeft: level ? Math.min(level * 14, 42) : 0 }}
        variant="default"
      >
        <Button
          aria-controls={bodyId}
          aria-expanded={isExpanded}
          className="h-auto min-h-0 w-full min-w-0 items-start justify-start gap-2 rounded-none px-3 py-2 text-left"
          onPress={() => setIsExpanded((current) => !current)}
          type="button"
          variant="ghost"
        >
          {isExpanded ? (
            <ChevronDown
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-[var(--muted)]"
            />
          ) : (
            <ChevronRight
              aria-hidden="true"
              className="mt-0.5 size-3.5 shrink-0 text-[var(--muted)]"
            />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] font-semibold text-[var(--muted)]">
                {task.id}
              </span>
              <span className={taskStatusClass(task.status)}>
                {t(task.status)}
              </span>
            </div>
            <h3
              className={`mt-1 break-words text-sm font-semibold leading-snug text-[var(--foreground)] ${isExpanded ? "" : "line-clamp-2"
                }`}
            >
              {task.title}
            </h3>
            {task.summary ? (
              <p
                className={`mt-1 break-words text-xs leading-5 text-[var(--muted)] ${isExpanded ? "" : "line-clamp-2"
                  }`}
              >
                {task.summary}
              </p>
            ) : null}
          </div>
        </Button>
        {isExpanded ? (
          <Card.Content className="px-3 pb-2 pl-8" id={bodyId}>
            {task.dependsOn.length ? (
              <div className="mt-1 flex flex-wrap gap-1.5">
                {task.dependsOn.map((dependencyId) => (
                  <span
                    className="rounded-md bg-[var(--surface-secondary)] px-1.5 py-0.5 font-mono text-[11px] text-[var(--muted)]"
                    key={dependencyId}
                  >
                    {dependencyId}
                  </span>
                ))}
              </div>
            ) : null}
            {task.acceptance.length ? (
              <ul className="mt-2 space-y-1 text-xs leading-5 text-[var(--muted)]">
                {task.acceptance.map((item, index) => (
                  <li className="flex gap-2" key={`${task.id}-acceptance-${index}`}>
                    <CheckCircle2
                      aria-hidden="true"
                      className="mt-0.5 size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
                    />
                    <span className="min-w-0 break-words">{item}</span>
                  </li>
                ))}
              </ul>
            ) : null}
          </Card.Content>
        ) : null}
      </Card>
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
  sourceControlTargetKey,
  sourceControlTargets,
  onCommit,
  onGenerateCommitMessage,
  onCommitMessageChange,
  onFileOperation,
  onRefresh,
  onSelectFile,
  onTargetChange,
  selectedPath,
}: {
  diffError: string | null;
  diffResponse: GitDiffResponse | null;
  files: GitStatusFileSummary[];
  gitCommitMessage: string;
  gitOperationKey: string | null;
  isLoading: boolean;
  sourceControlTargetKey: string;
  sourceControlTargets: { key: string; label: string; description: string }[];
  onCommit: (event: FormEvent<HTMLFormElement>) => void;
  onGenerateCommitMessage: () => void;
  onCommitMessageChange: (message: string) => void;
  onFileOperation: (action: "stage" | "unstage" | "discard", path: string) => void;
  onRefresh: () => void;
  onSelectFile: (path: string | null) => void;
  onTargetChange: (targetKey: string) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();
  const diffSections = parseGitDiffSections(diffResponse);
  const stagedFiles = diffResponse?.stagedFiles ?? [];
  const isCommitting = gitOperationKey === "commit";
  const isGeneratingCommitMessage = gitOperationKey === "generate-commit-message";
  const isCommitMessageInputDisabled = isCommitting || isGeneratingCommitMessage;

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col bg-[var(--background-secondary)]">
      <div className="flex items-center justify-between gap-3 border-b border-[color-mix(in_oklab,var(--border)_80%,transparent)] px-3 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]">
            <GitCompare aria-hidden="true" className="size-4" />
          </span>
          <div className="min-w-0 flex-1">
            <span className="foco-eyebrow">{t("Source Control")}</span>
            <Select
              aria-label={t("Source Control target")}
              isDisabled={sourceControlTargets.length <= 1 || isLoading}
              selectedKey={sourceControlTargetKey}
              onSelectionChange={(key) => onTargetChange(String(key ?? ""))}
            >
              <Label className="sr-only">{t("Source Control target")}</Label>
              <Select.Trigger
                aria-label={t("Source Control target")}
                className="mt-0.5 block w-full min-w-0 max-w-full truncate rounded-md border border-[var(--border)] bg-[var(--surface)] px-2 py-1 text-xs font-medium text-[var(--muted)] shadow-sm outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
              >
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {sourceControlTargets.map((target) => (
                    <ListBox.Item
                      id={target.key}
                      key={target.key}
                      textValue={target.label}
                    >
                      {target.label}
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  ))}
                </ListBox>
              </Select.Popover>
            </Select>
          </div>
        </div>
        <Button
          aria-label={t("Refresh diff")}
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-[var(--muted)] hover:bg-[var(--default)]/80 hover:text-[var(--foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
          isDisabled={isLoading}
          onPress={onRefresh}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className="context-refresh-icon size-4"
            data-loading={isLoading ? "true" : undefined}
          />
        </Button>
      </div>

      {diffError ? (
        <div className="border-b border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--danger)]">
          {diffError}
        </div>
      ) : null}

      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
        <form className="mb-3 space-y-2 px-1" onSubmit={onCommit}>
          <div className="relative">
            <TextField
              aria-label={t("Commit message")}
              isDisabled={isCommitMessageInputDisabled}
              value={gitCommitMessage}
              onChange={onCommitMessageChange}
            >
              <TextArea
                className="min-h-20 w-full resize-none rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 pr-11 text-sm text-[var(--foreground)] shadow-inner outline-none placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
                placeholder={t("Commit message")}
              />
            </TextField>
            <Button
              aria-label={t("Generate commit message")}
              className="absolute right-2 top-2 inline-flex size-7 items-center justify-center rounded-md text-[var(--accent-soft-foreground)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)] disabled:hover:bg-transparent"
              isDisabled={isCommitMessageInputDisabled || stagedFiles.length === 0}
              onPress={onGenerateCommitMessage}
              type="button"
            >
              {isGeneratingCommitMessage ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <Sparkles aria-hidden="true" className="size-4" />
              )}
            </Button>
          </div>
          <Button
            className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-[var(--accent)] px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--default)] disabled:text-[var(--muted)]"
            isDisabled={isCommitMessageInputDisabled || !gitCommitMessage.trim() || stagedFiles.length === 0}
            type="submit"
          >
            {isCommitting ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin" /> : null}
            {t("Commit")}
          </Button>
        </form>

        <section className="mb-3">
          <div className="mb-1 flex items-center justify-between px-1 text-[11px] font-semibold uppercase tracking-wide text-[var(--muted)]">
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
              <div className="rounded-md border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_70%,transparent)] px-3 py-2 text-xs text-[var(--muted)]">
                {t("No staged changes")}
              </div>
            )}
          </div>
        </section>

        <section>
          <Button
            className={diffFileButtonClass(selectedPath === null)}
            onPress={() => onSelectFile(null)}
            type="button"
          >
            <span className="truncate text-[11px] font-semibold uppercase tracking-wide">
              {t("Changes")}
            </span>
            <span className="text-xs text-[var(--muted)]">{files.length}</span>
          </Button>
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
              <div className="rounded-md border border-dashed border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_70%,transparent)] px-3 py-2 text-xs text-[var(--muted)]">
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
        <Button
          aria-label={`${file.path} ${label}`}
          className="flex min-w-0 flex-1 items-center gap-1.5 py-0.5 text-left"
          onPress={() => onSelectFile(isExpanded ? null : file.path)}
          type="button"
        >
          {isExpanded ? (
            <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
          ) : (
            <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
          )}
          <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-left">
            <span className="min-w-0 truncate text-[13px] font-medium text-[var(--foreground)]">
              {pathParts.name}
            </span>
            {pathParts.directory ? (
              <span className="shrink truncate text-xs text-[var(--muted)]">
                {pathParts.directory}
              </span>
            ) : null}
          </span>
        </Button>
        <span className={gitStatusBadgeClass(label)}>{label}</span>
        <Button
          aria-label={t(action === "stage" ? "Stage file" : "Unstage file")}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded text-[var(--muted)] hover:bg-[var(--default)] hover:text-[var(--foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
          isDisabled={gitOperationKey !== null}
          onPress={() => {
            onFileOperation(action, file.path);
          }}
          type="button"
        >
          {isActionLoading ? (
            <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          ) : action === "stage" ? (
            <Plus aria-hidden="true" className="size-3.5" />
          ) : (
            <Minus aria-hidden="true" className="size-3.5" />
          )}
        </Button>
        {showDiscard ? (
          <Button
            aria-label={t("Discard file changes")}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded text-[var(--muted)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
            isDisabled={gitOperationKey !== null}
            onPress={() => {
              onFileOperation("discard", file.path);
            }}
            type="button"
          >
            {isDiscardLoading ? (
              <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
            ) : (
              <Undo2 aria-hidden="true" className="size-3.5" />
            )}
          </Button>
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
      <div className="ml-5 mt-1 flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-3 text-xs font-medium text-[var(--muted)]">
        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        {t("Loading…")}
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
          <div className="text-[11px] font-semibold uppercase text-[var(--muted)]">
            {t(section.kind === "staged" ? "Staged" : "Unstaged")}
          </div>
          {section.files.map((file) =>
            file.isBinary || file.lines.length === 0 ? (
              <InlineGitDiffNotice key={`${section.kind}-${file.path}`}>
                {t("Inline diff is unavailable for binary or non-text files.")}
              </InlineGitDiffNotice>
            ) : (
              <div
                className="panel-scroll max-h-[min(30rem,52dvh)] overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface)] py-2 font-mono text-[11px] leading-5 shadow-sm"
                key={`${section.kind}-${file.path}`}
              >
                {file.lines.map((line, index) => (
                  <div
                    className={diffLineClass(line.kind)}
                    key={`${section.kind}-${file.path}-${index}`}
                  >
                    <span className="select-none pr-2 text-[var(--muted)]">
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
    <div className="flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-xs font-medium text-[var(--muted)]">
      <FileText aria-hidden="true" className="size-3.5 shrink-0" />
      <span>{children}</span>
    </div>
  );
}

type ContextPanelSidebarProps = ComponentProps<typeof ContextPanel> & {
  contextPanelMobileHeight: number;
  diffPanelWidth: number;
  isResizing: boolean;
  onResizeStart: (session: {
    stacked: boolean;
    startClientX: number;
    startClientY: number;
    startHeight: number;
    startWidth: number;
  }) => void;
  setMobileHeight: PanelNumberSetter;
  setWidth: PanelNumberSetter;
};

export function ContextPanelSidebar({
  contextPanelMobileHeight,
  diffPanelWidth,
  isResizing,
  onResizeStart,
  setMobileHeight,
  setWidth,
  ...panelProps
}: ContextPanelSidebarProps) {
  const { t } = useI18n();
  const [isStackedLayout, setIsStackedLayout] = useState(
    () =>
      typeof window !== "undefined" &&
      window.innerWidth < CONTEXT_PANEL_STACKED_BREAKPOINT_PX,
  );
  const [stackedMaxHeight, setStackedMaxHeight] = useState(() =>
    typeof window !== "undefined"
      ? Math.floor(window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO)
      : CONTEXT_PANEL_MIN_HEIGHT,
  );

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    function syncStackedLayoutMetrics() {
      setIsStackedLayout(window.innerWidth < CONTEXT_PANEL_STACKED_BREAKPOINT_PX);
      setStackedMaxHeight(
        Math.floor(window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO),
      );
    }

    syncStackedLayoutMetrics();
    window.addEventListener("resize", syncStackedLayoutMetrics);
    return () => {
      window.removeEventListener("resize", syncStackedLayoutMetrics);
    };
  }, []);

  return (
    <aside className="context-sidebar diff-sidebar min-w-0 border-[color-mix(in_oklab,var(--border)_80%,transparent)] lg:border-l">
      <div className="relative flex h-full min-h-0 min-w-0 flex-col">
        <div
          aria-label={t("Resize context panel")}
          aria-orientation={isStackedLayout ? "horizontal" : "vertical"}
          aria-valuemax={isStackedLayout ? stackedMaxHeight : CONTEXT_PANEL_MAX_WIDTH}
          aria-valuemin={isStackedLayout ? CONTEXT_PANEL_MIN_HEIGHT : CONTEXT_PANEL_MIN_WIDTH}
          aria-valuenow={isStackedLayout ? contextPanelMobileHeight : diffPanelWidth}
          className={`context-sidebar-splitter ${isResizing ? "context-sidebar-splitter-active" : ""}`}
          onKeyDown={(event) => {
            if (isStackedLayout) {
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
                setMobileHeight((current) =>
                  Math.max(current - 24, CONTEXT_PANEL_MIN_HEIGHT),
                );
              }
              return;
            }

            if (event.key === "ArrowLeft") {
              event.preventDefault();
              setWidth((current) => Math.min(current + 24, CONTEXT_PANEL_MAX_WIDTH));
            }

            if (event.key === "ArrowRight") {
              event.preventDefault();
              setWidth((current) => Math.max(current - 24, CONTEXT_PANEL_MIN_WIDTH));
            }
          }}
          onPointerDown={(event) => {
            event.preventDefault();
            const stacked = window.innerWidth < CONTEXT_PANEL_STACKED_BREAKPOINT_PX;
            const maxHeight = Math.floor(
              window.innerHeight * CONTEXT_PANEL_MAX_HEIGHT_RATIO,
            );
            const startHeight = Math.min(
              Math.max(contextPanelMobileHeight, CONTEXT_PANEL_MIN_HEIGHT),
              maxHeight,
            );
            const startWidth = Math.min(
              Math.max(diffPanelWidth, CONTEXT_PANEL_MIN_WIDTH),
              CONTEXT_PANEL_MAX_WIDTH,
            );
            event.currentTarget.setPointerCapture(event.pointerId);
            onResizeStart({
              stacked,
              startClientX: event.clientX,
              startClientY: event.clientY,
              startHeight,
              startWidth,
            });
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
    return `${base} bg-[var(--success-soft)] text-[var(--success-soft-foreground)]`;
  }

  if (status === "running" || status === "ready") {
    return `${base} bg-[var(--warning-soft)] text-[var(--warning)]`;
  }

  if (status === "failed") {
    return `${base} bg-[var(--danger-soft)] text-[var(--danger)]`;
  }

  return `${base} bg-[var(--surface-secondary)] text-[var(--muted)]`;
}

function earliestIncompletePlanPhase(plan: Plan) {
  return plan.phases
    .filter((phase) => phase.status !== "completed")
    .sort((left, right) => left.sequence - right.sequence)[0] ?? null;
}

function primaryPlanAction(plan: Plan): PlanAction | null {
  const { status } = plan;
  if (status === "implemented" || status === "failed" || status === "cancelled") {
    return "mark_complete";
  }
  if (status === "paused") {
    // A cancelled earliest incomplete phase is a durable execution barrier. The
    // phase Retry action is the only UI operation that may resume this plan.
    if (earliestIncompletePlanPhase(plan)?.status === "cancelled") {
      return null;
    }
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

function isRetryablePlanPhase(phase: PlanPhase) {
  return phase.status === "failed" || phase.status === "cancelled";
}

function isPlanReorderable(plan: Plan) {
  return plan.status === "draft" || plan.status === "ready" || plan.status === "paused" || plan.status === "failed";
}

function reorderablePlanIds(plans: Plan[]) {
  return plans.filter(isPlanReorderable).map((plan) => plan.id);
}

function reorderPlansByIds(plans: Plan[], planIds: string[]) {
  const reorderablePlans = plans.filter(isPlanReorderable);
  if (reorderablePlans.length !== planIds.length) {
    return plans;
  }
  const plansById = new Map(reorderablePlans.map((plan) => [plan.id, plan]));
  const nextReorderablePlans = planIds
    .map((planId) => plansById.get(planId))
    .filter((plan): plan is Plan => Boolean(plan));
  if (nextReorderablePlans.length !== reorderablePlans.length) {
    return plans;
  }
  let nextIndex = 0;
  return plans.map((plan) => (isPlanReorderable(plan) ? nextReorderablePlans[nextIndex++] ?? plan : plan));
}

function planPhaseRetryOperationKey(planId: string, phaseId: string) {
  return `retry-phase:${planId}:${phaseId}`;
}

function planRetryMergeOperationKey(planId: string) {
  return `retry_merge:${planId}`;
}

function planNeedsMergeRetry(plan: Plan) {
  // Merge integration is separate from phase implementation: after all phases
  // complete, a failed merge leaves the plan implemented with an error and no
  // sharedMergeCommitId. Offer Retry Merge for any such state (dirty workspace,
  // LLM merge failure, cancel/interrupt), not only the dirty-workspace message.
  const errorMessage = plan.errorMessage?.trim();
  return (
    !plan.sharedMergeCommitId?.trim() &&
    !!errorMessage &&
    (plan.status === "implemented" || plan.status === "blocked")
  );
}

function isDirtySharedWorkspaceMergeError(errorMessage: string | null | undefined) {
  return (errorMessage ?? "").includes("shared workspace has uncommitted changes");
}

function planMergeRetryHint(plan: Plan) {
  return isDirtySharedWorkspaceMergeError(plan.errorMessage)
    ? "Clean the shared workspace, then retry merge"
    : "Retry merging into the shared workspace";
}

function planActionLabel(action: PlanAction) {
  switch (action) {
    case "mark_complete":
      return "Mark complete";
    case "resume":
      return "Resume";
    case "start":
      return "Start";
    case "pause":
      return "Pause";
    case "retry_merge":
      return "Retry Merge";
    default:
      return action;
  }
}

function planMergedIntoSharedWorkspace(plan: Plan) {
  const commitId = plan.sharedMergeCommitId?.trim();
  return commitId ? commitId.slice(0, 7) : null;
}

function planMergeInProgress(plan: Plan) {
  if (plan.sharedMergeCommitId?.trim()) {
    return null;
  }

  const activeMergeAttempt = plan.phases
    .flatMap((phase) => phase.attempts ?? [])
    .filter(
      (attempt) =>
        (attempt.trigger === "merge_auto" || attempt.trigger === "merge_retry") &&
        (attempt.status === "queued" || attempt.status === "running"),
    )
    .reduce<typeof plan.phases[number]["attempts"][number] | null>(
      (latestAttempt, attempt) =>
        !latestAttempt || attempt.sequence > latestAttempt.sequence
          ? attempt
          : latestAttempt,
      null,
    );
  if (activeMergeAttempt) {
    return {
      implementationChatId: activeMergeAttempt.implementationChatId?.trim() || null,
    };
  }

  const allPhasesCompleted =
    plan.phases.length > 0 &&
    plan.phases.every((phase) => phase.status === "completed");
  return plan.status === "running" && allPhasesCompleted
    ? { implementationChatId: null }
    : null;
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
    case "blocked":
      return "Blocked";
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
    return `${base} bg-[var(--success-soft)] text-[var(--success-soft-foreground)]`;
  }
  if (status === "ready" || status === "running" || status === "blocked") {
    return `${base} bg-[var(--warning-soft)] text-[var(--warning)]`;
  }
  if (status === "failed") {
    return `${base} bg-[var(--danger-soft)] text-[var(--danger)]`;
  }
  return `${base} bg-[var(--surface-secondary)] text-[var(--muted)]`;
}

function planPhaseStatusClass(status: string) {
  const base = "inline-flex shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-semibold";
  if (status === "completed" || status === "implemented") {
    return `${base} bg-[var(--success-soft)] text-[var(--success-soft-foreground)]`;
  }
  if (status === "ready" || status === "running" || status === "blocked") {
    return `${base} bg-[var(--warning-soft)] text-[var(--warning)]`;
  }
  if (status === "failed") {
    return `${base} bg-[var(--danger-soft)] text-[var(--danger)]`;
  }
  return `${base} bg-[var(--surface-secondary)] text-[var(--muted)]`;
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
    projectSpec: t("Project Spec"),
    reservedPrompt: t("Prompt"),
    runtimeAssistant: t("Runtime assistant"),
    runtimeToolState: t("Tools"),
    runtimeToolStateSnapshot: t("Tools"),
    stableInjection: t("Stable context"),
    todoGraph: t("Tools"),
    toolCalls: t("Tools"),
    turnMemory: t("Memory"),
  };

  return labels[source] ?? source;
}

function diffFileButtonClass(active: boolean) {
  return `diff-file-button flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-sm ${active
      ? "diff-file-button-active bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)] shadow-sm"
      : "text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)]"
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
      ? "bg-[var(--warning-soft)] text-[var(--warning)] border-[var(--warning)]"
      : status === "U" || status === "A"
        ? "bg-[var(--success-soft)] text-[var(--success-soft-foreground)] border-[var(--success)]"
        : status === "D"
          ? "bg-[var(--danger-soft)] text-[var(--danger)] border-[var(--danger)]"
          : status === "R"
            ? "bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)] border-[var(--accent)]"
            : "bg-[var(--surface-secondary)] text-[var(--muted)] border-[var(--border)]";

  return `shrink-0 rounded border px-1.5 py-0.5 font-mono text-[11px] font-semibold leading-none ${colorClass}`;
}

function normalizeGitStatus(status: string) {
  const trimmed = status.trim();
  if (!trimmed) {
    return "";
  }

  return trimmed === "?" ? "U" : trimmed;
}
