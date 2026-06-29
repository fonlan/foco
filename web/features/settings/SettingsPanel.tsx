import {
  ArrowDown,
  ArrowUp,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Code2,
  Database,
  Eye,
  EyeOff,
  FileText,
  Folder,
  FolderSearch,
  Globe,
  GripVertical,
  KeyRound,
  ListChecks,
  LoaderCircle,
  Lock,
  Pencil,
  Play,
  PlugZap,
  Plus,
  Redo2,
  RefreshCw,
  ScrollText,
  Search,
  Server,
  SlidersHorizontal,
  Sparkles,
  Terminal,
  Trash2,
  Upload,
  Webhook,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  ChangeEvent as ReactChangeEvent,
  DragEvent as ReactDragEvent,
  FormEvent,
  WheelEvent as ReactWheelEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type {
  AgentDefinitionInput,
  AgentDefinitionSettings,
  AppLanguageId,
  AppThemeId,
  ClearMemoriesResponse,
  ConfiguredMcpServerSummary,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  ConfiguredWorkspaceSummary,
  EffectiveHookSummary,
  GeneralFormState,
  HookConfig,
  HookHandler,
  HookHandlerFormState,
  HookHandlerType,
  HookMatcherGroup,
  HookRunDetail,
  HookRunDetailResponse,
  HookRunSummary,
  HookRunSummaryRow,
  HookRunsResponse,
  HookScope,
  HooksSettingsResponse,
  ImportClaudeHooksResponse,
  JsonValue,
  ManualMemoryFormState,
  McpServerFormState,
  MemoryDialogMode,
  MemoryDreamChangeSummary,
  MemoryDreamChangesResponse,
  MemoryDreamJobSummary,
  MemoryDreamJobsResponse,
  MemoryDreamRunMode,
  MemoryDreamRunResponse,
  MemoryDreamScope,
  MemoryExtractionJobSummary,
  MemoryFactRecord,
  MemoryFilterState,
  MemoryListMeta,
  MemoryListResponse,
  MemoryMutationResponse,
  MemorySettingsFormState,
  MemorySourceFormState,
  MemorySourceRecord,
  MemorySourcesResponse,
  ModelFormState,
  ModelMetadataRecord,
  ModelMetadataResponse,
  NativeSelectedFile,
  Plan,
  PlanResponse,
  PlansResponse,
  PromptSettingsFormState,
  PromptSettingsSummary,
  ProviderFormState,
  ProviderModelsRefreshResponse,
  ProviderModelsResponse,
  ProviderRequestOverrideFormState,
  ProviderRequestOverrideTarget,
  ProviderRequestOverrideValueType,
  ProviderTestResponse,
  ProviderTestState,
  SettingsResponse,
  SettingsSection,
  SpecSettingsFormState,
  SystemPromptSummary,
  TerminalShellSummary,
  Translate,
  WebSearchFormState,
  WorkspaceCommonCommandSummary,
  WorkspaceFormState,
  WorkspaceSpecResponse,
} from "../../api/types";
import {
  DEFAULT_SYSTEM_PROMPT_NAME,
  IMAGE_AGENT_SYSTEM_PROMPT_NAME,
  PLAN_MODE_SYSTEM_PROMPT_NAME,
  MEMORY_KIND_OPTIONS,
  REVIEW_SYSTEM_PROMPT_NAME,
  SAVED_PASSWORD_MASK,
} from "../../app/constants";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";
import { AgentsSettingsPanel } from "../agents/AgentsSettingsPanel";
import { WorkspaceIcon } from "../workspaces/WorkspaceIcon";
import {
  moveItemId,
  sameStringList,
  workspaceNameFromPath,
} from "../workspaces/workspace-helpers";

type ProviderModelListState = {
  message: string | null;
  models: string[];
  status: "error" | "loading" | "ok";
};

const OPENAI_RESPONSES_PROVIDER_KIND = "openai-responses";

const MODEL_DEVELOPERS = [
  "deepseek",
  "alibaba",
  "zai",
  "openai",
  "moonshot",
  "anthropic",
  "google",
  "minimax",
  "xiaomi",
  "longcat",
  "mistral",
  "nvidia",
  "xai",
  "bytedance",
  "stepfun",
  "meta",
];

function saveWorkspaceSpecSettingsRequest(
  workspaceId: string,
  enabled: boolean,
  injectEnabled: boolean,
) {
  return requestJson<WorkspaceSpecResponse>(
    `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/settings`,
    {
      body: JSON.stringify({ enabled, injectEnabled }),
      headers: { "Content-Type": "application/json" },
      method: "PUT",
    },
  );
}

type ProviderServicePreset = {
  id: string;
  label: string;
  kindIds: string[];
  defaultKindId: string;
};

const PROVIDER_SERVICE_PRESETS: ProviderServicePreset[] = [
  {
    id: "openai",
    label: "OpenAI",
    kindIds: [OPENAI_RESPONSES_PROVIDER_KIND, "openai-chat"],
    defaultKindId: OPENAI_RESPONSES_PROVIDER_KIND,
  },
  {
    id: "anthropic",
    label: "Anthropic",
    kindIds: ["anthropic"],
    defaultKindId: "anthropic",
  },
  { id: "gemini", label: "Gemini", kindIds: ["gemini"], defaultKindId: "gemini" },
  { id: "xai", label: "xAI", kindIds: ["xai"], defaultKindId: "xai" },
  {
    id: "deepseek",
    label: "DeepSeek",
    kindIds: ["deepseek"],
    defaultKindId: "deepseek",
  },
  { id: "groq", label: "Groq", kindIds: ["groq"], defaultKindId: "groq" },
  {
    id: "open-router",
    label: "OpenRouter",
    kindIds: ["open-router"],
    defaultKindId: "open-router",
  },
  {
    id: "fireworks",
    label: "Fireworks",
    kindIds: ["fireworks"],
    defaultKindId: "fireworks",
  },
  {
    id: "together",
    label: "Together",
    kindIds: ["together"],
    defaultKindId: "together",
  },
  {
    id: "moonshot",
    label: "Moonshot",
    kindIds: ["moonshot"],
    defaultKindId: "moonshot",
  },
  { id: "zai", label: "ZAI", kindIds: ["zai"], defaultKindId: "zai" },
  {
    id: "bigmodel",
    label: "BigModel",
    kindIds: ["bigmodel"],
    defaultKindId: "bigmodel",
  },
  { id: "aliyun", label: "Aliyun", kindIds: ["aliyun"], defaultKindId: "aliyun" },
  { id: "baidu", label: "Baidu", kindIds: ["baidu"], defaultKindId: "baidu" },
  { id: "cohere", label: "Cohere", kindIds: ["cohere"], defaultKindId: "cohere" },
  { id: "ollama", label: "Ollama", kindIds: ["ollama"], defaultKindId: "ollama" },
  {
    id: "ollama-cloud",
    label: "Ollama Cloud",
    kindIds: ["ollama-cloud"],
    defaultKindId: "ollama-cloud",
  },
  { id: "vertex", label: "Vertex AI", kindIds: ["vertex"], defaultKindId: "vertex" },
  {
    id: "github-copilot",
    label: "GitHub Copilot",
    kindIds: ["github-copilot"],
    defaultKindId: "github-copilot",
  },
  {
    id: "opencode-go",
    label: "OpenCode Go",
    kindIds: ["opencode-go"],
    defaultKindId: "opencode-go",
  },
  {
    id: "bedrock-api",
    label: "Bedrock API",
    kindIds: ["bedrock-api"],
    defaultKindId: "bedrock-api",
  },
  {
    id: "aihubmix",
    label: "AIHubMix",
    kindIds: ["aihubmix"],
    defaultKindId: "aihubmix",
  },
  { id: "mimo", label: "Mimo", kindIds: ["mimo"], defaultKindId: "mimo" },
  { id: "nebius", label: "Nebius", kindIds: ["nebius"], defaultKindId: "nebius" },
  { id: "minimax", label: "MiniMax", kindIds: ["minimax"], defaultKindId: "minimax" },
];

const MEMORY_DREAM_DEFAULT_PAGE_SIZE = 10;

const MEMORY_DREAM_MAX_PAGE_SIZE = 200;

export function SettingsPanel({
  activeSection,
  activeWorkspaceId,
  agentDefinitionOperationKey,
  agentDefinitions,
  agentDefinitionsError,
  defaultAgentRolePrompts,
  canLogout,
  canUseNativePicker,
  isLoadingAgentDefinitions,
  nativeBrowserToken,
  onAddWorkspace,
  onActiveSectionChange,
  onCreateAgentDefinition,
  onDeleteAgentDefinition,
  onUpdateAgentDefinition,
  onLogout,
  onOpenChat,
  onSettingsChange,
  onWorkspacesChange,
  workspaceDialogRevision,
}: {
  activeSection: SettingsSection;
  activeWorkspaceId: string | null;
  agentDefinitionOperationKey: string | null;
  agentDefinitions: AgentDefinitionSettings[];
  agentDefinitionsError: string | null;
  defaultAgentRolePrompts: Record<string, string>;
  canLogout: boolean;
  canUseNativePicker: boolean;
  isLoadingAgentDefinitions: boolean;
  nativeBrowserToken: string | null;
  onAddWorkspace: () => void;
  onActiveSectionChange: (section: SettingsSection) => void;
  onCreateAgentDefinition: (definition: AgentDefinitionInput) => Promise<boolean>;
  onDeleteAgentDefinition: (id: string) => Promise<void>;
  onUpdateAgentDefinition: (
    id: string,
    definition: AgentDefinitionInput,
  ) => Promise<boolean>;
  onLogout: () => Promise<void>;
  onOpenChat: (workspaceId: string, chatId: string) => void;
  onSettingsChange: (settings: SettingsResponse) => void;
  onWorkspacesChange: () => Promise<void>;
  workspaceDialogRevision: number;
}) {
  const { language, t } = useI18n();
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [isProviderDialogOpen, setIsProviderDialogOpen] = useState(false);
  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [isMcpDialogOpen, setIsMcpDialogOpen] = useState(false);
  const [metadata, setMetadata] = useState<ModelMetadataResponse | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedMetadataKey, setSelectedMetadataKey] = useState("");
  const [selectedModelDeveloper, setSelectedModelDeveloper] = useState("");
  const [form, setForm] = useState<ModelFormState>(() => emptyModelForm());
  const [providerForm, setProviderForm] = useState<ProviderFormState>(() =>
    emptyProviderForm(),
  );
  const [generalForm, setGeneralForm] = useState<GeneralFormState>(() =>
    emptyGeneralForm(),
  );
  const [webSearchForm, setWebSearchForm] = useState<WebSearchFormState>(() =>
    emptyWebSearchForm(),
  );
  const [promptSettingsForm, setPromptSettingsForm] =
    useState<PromptSettingsFormState>(() => emptyPromptSettingsForm());
  const [specSettingsForm, setSpecSettingsForm] =
    useState<SpecSettingsFormState>(() => emptySpecSettingsForm());
  const [planMergeAutomationMode, setPlanMergeAutomationMode] =
    useState("isolated_auto_once");
  const [planHistory, setPlanHistory] = useState<Plan[]>([]);
  const [planHistoryPage, setPlanHistoryPage] = useState(1);
  const [planHistoryPageSize, setPlanHistoryPageSize] = useState(20);
  const [planHistoryStatus, setPlanHistoryStatus] = useState("");
  const [planHistoryWorkspaceId, setPlanHistoryWorkspaceId] = useState("");
  const [planHistoryTotalCount, setPlanHistoryTotalCount] = useState(0);
  const [planHistoryTotalPages, setPlanHistoryTotalPages] = useState(0);
  const [memorySettingsForm, setMemorySettingsForm] =
    useState<MemorySettingsFormState>(() => emptyMemorySettingsForm());
  const [memoryFilter, setMemoryFilter] = useState<MemoryFilterState>(() =>
    emptyMemoryFilter(),
  );
  const [manualMemoryForm, setManualMemoryForm] =
    useState<ManualMemoryFormState>(() => emptyManualMemoryForm());
  const [memorySourceForms, setMemorySourceForms] = useState<MemorySourceFormState[]>(
    [],
  );
  const [expandedMemoryJsonIds, setExpandedMemoryJsonIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [memoryDialogMode, setMemoryDialogMode] =
    useState<MemoryDialogMode>("create");
  const [isMemoryDialogOpen, setIsMemoryDialogOpen] = useState(false);
  const [memories, setMemories] = useState<MemoryFactRecord[]>([]);
  const [memoryListMeta, setMemoryListMeta] = useState<MemoryListMeta>({
    page: 1,
    pageSize: 20,
    totalCount: 0,
    totalPages: 0,
  });
  const [memoryExtractionJobs, setMemoryExtractionJobs] = useState<
    MemoryExtractionJobSummary[]
  >([]);
  const [memoryDreamJobs, setMemoryDreamJobs] = useState<MemoryDreamJobSummary[]>(
    [],
  );
  const [memoryDreamPage, setMemoryDreamPage] = useState(1);
  const [memoryDreamPageSize, setMemoryDreamPageSize] = useState(
    MEMORY_DREAM_DEFAULT_PAGE_SIZE,
  );
  const [memoryDreamChanges, setMemoryDreamChanges] = useState<
    MemoryDreamChangeSummary[]
  >([]);
  const [memoryDreamDetailJobId, setMemoryDreamDetailJobId] = useState<
    string | null
  >(null);
  const [memoryDreamError, setMemoryDreamError] = useState<string | null>(null);
  const [selectedMemoryId, setSelectedMemoryId] = useState<string | null>(null);
  const [memorySources, setMemorySources] = useState<MemorySourceRecord[]>([]);
  const [workspaceForm, setWorkspaceForm] = useState<WorkspaceFormState>(() =>
    emptyWorkspaceForm(),
  );
  const [isLoadingWorkspaceSpecSettings, setIsLoadingWorkspaceSpecSettings] =
    useState(false);
  const [isWorkspaceSpecSettingsLoaded, setIsWorkspaceSpecSettingsLoaded] =
    useState(false);
  const [mcpForm, setMcpForm] = useState<McpServerFormState>(() =>
    emptyMcpServerForm(),
  );
  const [hookSettings, setHookSettings] = useState<HooksSettingsResponse | null>(
    null,
  );
  const [hookScope, setHookScope] = useState<HookScope>("global");
  const [hookWorkspaceId, setHookWorkspaceId] = useState("");
  const [hookForm, setHookForm] = useState<HookHandlerFormState>(() =>
    emptyHookHandlerForm(),
  );
  const [isHookDialogOpen, setIsHookDialogOpen] = useState(false);
  const [isLoadingHooks, setIsLoadingHooks] = useState(false);
  const [isSavingHooks, setIsSavingHooks] = useState(false);
  const [isImportingHooks, setIsImportingHooks] = useState(false);
  const [isTestingHooks, setIsTestingHooks] = useState(false);
  const [isRefreshingHookRuns, setIsRefreshingHookRuns] = useState(false);
  const [hookImportResult, setHookImportResult] =
    useState<ImportClaudeHooksResponse | null>(null);
  const [hookTestResult, setHookTestResult] = useState<HookRunSummary | null>(
    null,
  );
  const [hookRunDetail, setHookRunDetail] = useState<HookRunDetail | null>(null);
  const [hookTestEvent, setHookTestEvent] = useState("PreToolUse");
  const [hookTestMatcher, setHookTestMatcher] = useState("run_command");
  const [hookTestPayload, setHookTestPayload] = useState(
    '{\n  "toolInput": {\n    "command": "git status"\n  }\n}',
  );
  const [enabledSkillIds, setEnabledSkillIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSavingGeneral, setIsSavingGeneral] = useState(false);
  const [isSavingWebSearch, setIsSavingWebSearch] = useState(false);
  const [isSavingPromptSettings, setIsSavingPromptSettings] = useState(false);
  const [isSavingSpecSettings, setIsSavingSpecSettings] = useState(false);
  const [isSavingPlanSettings, setIsSavingPlanSettings] = useState(false);
  const [isLoadingPlanHistory, setIsLoadingPlanHistory] = useState(false);
  const [planHistoryError, setPlanHistoryError] = useState<string | null>(null);
  const [planHistoryOperationKey, setPlanHistoryOperationKey] = useState<string | null>(null);
  const [isSelectingPromptFile, setIsSelectingPromptFile] = useState(false);
  const [isSavingMemorySettings, setIsSavingMemorySettings] = useState(false);
  const [isLoadingMemories, setIsLoadingMemories] = useState(false);
  const [isLoadingMemoryDreamJobs, setIsLoadingMemoryDreamJobs] = useState(false);
  const [isLoadingMemoryDreamChanges, setIsLoadingMemoryDreamChanges] =
    useState(false);
  const [isSavingMemory, setIsSavingMemory] = useState(false);
  const [memoryDreamRunKey, setMemoryDreamRunKey] = useState<string | null>(null);
  const [isClearingPassword, setIsClearingPassword] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingTheme, setIsSavingTheme] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSavingWorkspaceOrder, setIsSavingWorkspaceOrder] = useState(false);
  const [isSavingWorkspaceLogo, setIsSavingWorkspaceLogo] = useState(false);
  const [isSelectingWorkspaceFormPath, setIsSelectingWorkspaceFormPath] =
    useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [isRefreshingProviderModels, setIsRefreshingProviderModels] =
    useState(false);
  const [isSavingMcpServer, setIsSavingMcpServer] = useState(false);
  const [isSavingModelOrder, setIsSavingModelOrder] = useState(false);
  const [isSavingSkills, setIsSavingSkills] = useState(false);
  const [isRefreshingSkills, setIsRefreshingSkills] = useState(false);
  const [draggedModelId, setDraggedModelId] = useState<string | null>(null);
  const [modelOrderPreview, setModelOrderPreview] = useState<string[] | null>(
    null,
  );
  const [draggedWorkspaceId, setDraggedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceOrderPreview, setWorkspaceOrderPreview] = useState<
    string[] | null
  >(null);
  const workspaceLogoInputRef = useRef<HTMLInputElement | null>(null);
  const [providerTests, setProviderTests] = useState<
    Record<string, ProviderTestState>
  >({});
  const [expandedProviderIds, setExpandedProviderIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [providerModelLists, setProviderModelLists] = useState<
    Record<string, ProviderModelListState>
  >({});
  const [error, setError] = useState<string | null>(null);
  const [isGeneralPasswordVisible, setIsGeneralPasswordVisible] = useState(false);
  const [isProviderApiKeyVisible, setIsProviderApiKeyVisible] = useState(false);
  const [isEditingGeneralPassword, setIsEditingGeneralPassword] = useState(false);

  const selectedMetadata = useMemo(
    () =>
      metadata?.models.find((model) => model.key === selectedMetadataKey) ??
      null,
    [metadata, selectedMetadataKey],
  );
  const modelDeveloperOptions = MODEL_DEVELOPERS;
  const inputModalityOptions = useMemo(
    () =>
      modelModalityOptions(
        metadata?.models ?? [],
        "inputModalities",
        form.inputModalities,
      ),
    [form.inputModalities, metadata],
  );
  const outputModalityOptions = useMemo(
    () =>
      modelModalityOptions(
        metadata?.models ?? [],
        "outputModalities",
        form.outputModalities,
      ),
    [form.outputModalities, metadata],
  );
  const developerModels = useMemo(
    () => modelsForDeveloper(metadata?.models ?? [], selectedModelDeveloper).slice(0, 200),
    [metadata, selectedModelDeveloper],
  );
  const developerModelOptions = useMemo(
    () =>
      developerModels.map((model) => ({
        key: model.key,
        value: modelIdForDeveloper(model, selectedModelDeveloper),
      })),
    [developerModels, selectedModelDeveloper],
  );
  const modelOutputsText = form.outputModalities.includes("text");
  const enabledNeedsLimits =
    form.enabled &&
    modelOutputsText &&
    (!form.contextWindow.trim() || !form.maxOutputTokens.trim());
  const providerKinds = settings?.providerKinds ?? [];
  const providers = settings?.providers ?? [];
  const workspaces = settings?.workspaces ?? [];
  const memoryWorkspace =
    workspaces.find((workspace) => workspace.id === memoryFilter.workspaceId) ??
    workspaces[0] ??
    null;
  const memoryDialogWorkspace =
    workspaces.find((workspace) => workspace.id === manualMemoryForm.workspaceId) ??
    workspaces[0] ??
    null;
  const selectedMemory =
    memories.find((memory) => memory.id === selectedMemoryId) ?? null;
  const effectivePlanHistoryWorkspaceId =
    planHistoryWorkspaceId || activeWorkspaceId || "";
  const planWorkspace =
    settings?.workspaces.find(
      (workspace) => workspace.id === effectivePlanHistoryWorkspaceId,
    ) ?? null;
  const memoryPaginationItems = auditPaginationItems(
    memoryListMeta.page,
    memoryListMeta.totalPages,
  );
  const memoryPageStart = memories.length
    ? (memoryListMeta.page - 1) * memoryListMeta.pageSize + 1
    : 0;
  const memoryPageEnd = memories.length
    ? Math.min(memoryListMeta.totalCount, memoryPageStart + memories.length - 1)
    : 0;
  const memoryDreamWorkspaceId = memoryFilter.workspaceId || memoryWorkspace?.id || "";
  const sortedMemoryDreamJobs = useMemo(
    () =>
      [...memoryDreamJobs].sort(
        (left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt),
      ),
    [memoryDreamJobs],
  );
  const memoryDreamTotalPages = sortedMemoryDreamJobs.length
    ? Math.ceil(sortedMemoryDreamJobs.length / memoryDreamPageSize)
    : 0;
  const currentMemoryDreamPage =
    memoryDreamTotalPages === 0
      ? 1
      : Math.min(memoryDreamPage, memoryDreamTotalPages);
  const paginatedMemoryDreamJobs = sortedMemoryDreamJobs.slice(
    (currentMemoryDreamPage - 1) * memoryDreamPageSize,
    currentMemoryDreamPage * memoryDreamPageSize,
  );
  const memoryDreamPaginationItems = auditPaginationItems(
    currentMemoryDreamPage,
    memoryDreamTotalPages,
  );
  const memoryDreamPageStart = paginatedMemoryDreamJobs.length
    ? (currentMemoryDreamPage - 1) * memoryDreamPageSize + 1
    : 0;
  const memoryDreamPageEnd = paginatedMemoryDreamJobs.length
    ? Math.min(
      sortedMemoryDreamJobs.length,
      memoryDreamPageStart + paginatedMemoryDreamJobs.length - 1,
    )
    : 0;
  const planHistoryPaginationItems = auditPaginationItems(
    planHistoryPage,
    planHistoryTotalPages,
  );
  const planHistoryPageStart = planHistory.length
    ? (planHistoryPage - 1) * planHistoryPageSize + 1
    : 0;
  const planHistoryPageEnd = planHistory.length
    ? Math.min(planHistoryTotalCount, planHistoryPageStart + planHistory.length - 1)
    : 0;
  const memoryDreamDetailJob =
    sortedMemoryDreamJobs.find((job) => job.id === memoryDreamDetailJobId) ??
    null;
  const activeMemoryDreamJobKeys = useMemo(
    () =>
      new Set(
        sortedMemoryDreamJobs
          .filter((job) => isActiveMemoryDreamStatus(job.status))
          .map((job) => memoryDreamJobKey(job.scope, job.workspaceId)),
      ),
    [sortedMemoryDreamJobs],
  );
  const globalDreamRunKey = memoryDreamJobKey("global", null);
  const workspaceDreamRunKey = memoryDreamJobKey(
    "workspace",
    memoryDreamWorkspaceId,
  );
  const latestSuccessfulMemoryDreamJob =
    sortedMemoryDreamJobs.find((job) => job.status === "completed") ?? null;
  const latestFailedMemoryDreamJob =
    sortedMemoryDreamJobs.find((job) => job.status === "failed") ?? null;
  const memoryDreamNextRunEstimate = nextMemoryDreamRunEstimate(
    latestSuccessfulMemoryDreamJob,
    memorySettingsForm.dream,
    language,
    t,
  );
  const latestMemoryDreamChangeCount = latestSuccessfulMemoryDreamJob
    ? memoryDreamAppliedChangeCount(latestSuccessfulMemoryDreamJob)
    : 0;
  const memoryDreamChangesByOperation = useMemo(
    () => groupMemoryDreamChanges(memoryDreamChanges),
    [memoryDreamChanges],
  );
  const isMemoryDreamRunnable =
    memorySettingsForm.enabled && memorySettingsForm.dream.enabled;
  const canClearFilteredMemories =
    memoryFilter.scope !== "global" &&
    (memoryFilter.scope !== "chat" || Boolean(memoryFilter.chatId.trim()));
  const isMemoryFilterReady =
    memoryFilter.scope !== "chat" || Boolean(memoryFilter.chatId.trim());
  const clearFilteredMemoryLabel =
    memoryFilter.scope === "chat"
      ? t("Clear filtered chat memories")
      : t("Clear filtered workspace memories");
  const selectedHookWorkspace =
    workspaces.find((workspace) => workspace.id === hookWorkspaceId) ??
    workspaces[0] ??
    null;
  const activeHookConfig =
    hookScope === "global"
      ? hookSettings?.global.config
      : hookSettings?.workspace.config;
  const activeHookPath =
    hookScope === "global"
      ? hookSettings?.global.path
      : hookSettings?.workspace.path;
  const activeHookGroups = hookConfigEntries(activeHookConfig);
  const terminalShells = settings?.terminalShells ?? [];
  const mcpTransports = settings?.mcpTransports ?? [];
  const mcpServers = settings?.mcpServers ?? [];
  const skills = settings?.skills;
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const configuredModels =
    settings?.configuredModels ?? metadata?.configuredModels ?? [];
  const configuredModelsByName = useMemo(
    () =>
      [...configuredModels].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    [configuredModels],
  );
  const passwordInputValue =
    generalForm.password ||
    (settings?.general.webServer.passwordEnabled && !isEditingGeneralPassword
      ? SAVED_PASSWORD_MASK
      : "");
  const orderedConfiguredModels = useMemo(() => {
    const sortedConfiguredModels = [...configuredModels].sort((left, right) =>
      left.displayName.localeCompare(right.displayName),
    );

    if (!modelOrderPreview) {
      return sortedConfiguredModels;
    }

    const modelsById = new Map(
      sortedConfiguredModels.map((model) => [model.id, model]),
    );
    const previewModels = modelOrderPreview
      .map((modelId) => modelsById.get(modelId))
      .filter((model): model is ConfiguredModelSummary => Boolean(model));

    return previewModels.length === sortedConfiguredModels.length
      ? previewModels
      : sortedConfiguredModels;
  }, [configuredModels, modelOrderPreview]);
  const orderedWorkspaces = useMemo(() => {
    if (!workspaceOrderPreview) {
      return workspaces;
    }

    const workspacesById = new Map(
      workspaces.map((workspace) => [workspace.id, workspace]),
    );
    const previewWorkspaces = workspaceOrderPreview
      .map((workspaceId) => workspacesById.get(workspaceId))
      .filter(
        (workspace): workspace is ConfiguredWorkspaceSummary =>
          Boolean(workspace),
      );

    return previewWorkspaces.length === workspaces.length
      ? previewWorkspaces
      : workspaces;
  }, [workspaceOrderPreview, workspaces]);
  const editingModel =
    configuredModels.find((model) => model.id === form.modelId) ?? null;
  const modelThinkingEnabled = selectedMetadata
    ? selectedMetadata.reasoning || Boolean(editingModel?.supportsThinking)
    : Boolean(editingModel?.supportsThinking);
  const editingWorkspace =
    workspaces.find((workspace) => workspace.id === workspaceForm.id) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const providerServices = useMemo(
    () => providerServicesForKinds(providerKinds),
    [providerKinds],
  );
  const selectedProviderServiceId =
    providerForm.serviceId ||
    providerServiceIdForKind(providerForm.kind) ||
    providerServiceIdForKind(defaultProviderKind(providerKinds)) ||
    providerServices[0]?.id ||
    "";
  const selectedProviderService =
    providerServices.find((service) => service.id === selectedProviderServiceId) ??
    null;
  const providerProtocolKinds = selectedProviderService
    ? providerKinds.filter((kind) =>
      selectedProviderService.kindIds.includes(kind.kind),
    )
    : providerKinds;
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const apiProxyTypes = editingProvider?.apiProxy.supportedTypes ??
    providers[0]?.apiProxy.supportedTypes ?? [
      { label: "HTTP", proxyType: "http" },
      { label: "SOCKS", proxyType: "socks" },
    ];
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const hasProviderKeyClearButton = hasSavedProviderKey || providerForm.clearApiKey;
  const modelSupportMetadata = selectedMetadata ?? modelMetadataForInput(form.modelId);
  const supportedModelProviderIds = providers
    .filter((provider) =>
      providerSupportsModel(provider, form.modelId, modelSupportMetadata, editingModel),
    )
    .map((provider) => provider.id);
  const supportedModelProviderIdSet = new Set(supportedModelProviderIds);
  const modelProviderIds = form.providerIds.filter((providerId) =>
    supportedModelProviderIdSet.has(providerId),
  );
  const selectedProviderIds = new Set(modelProviderIds);
  const activeModelProviderId =
    !form.activeProviderId || modelProviderIds.includes(form.activeProviderId)
      ? form.activeProviderId
      : modelProviderIds[0] ?? "";
  const systemPrompts = promptSettingsForm.systemPrompts.length
    ? promptSettingsForm.systemPrompts
    : settings
      ? normalizedSystemPromptSummaries(settings.prompts)
      : [];
  const savedSystemPrompts = settings
    ? normalizedSystemPromptSummaries(settings.prompts)
    : systemPrompts;
  const activeSystemPrompt =
    systemPrompts.find(
      (prompt) => prompt.name === promptSettingsForm.activeSystemPromptName,
    ) ??
    systemPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME) ??
    systemPrompts[0] ??
    null;

  function syncSkillsForm(data: SettingsResponse) {
    setEnabledSkillIds(
      new Set(
        data.skills.detected
          .filter((skill) => skill.enabled)
          .map((skill) => skill.key),
      ),
    );
  }

  function syncGeneralForm(data: SettingsResponse) {
    setIsEditingGeneralPassword(false);
    setIsGeneralPasswordVisible(false);
    setGeneralForm({
      apiRequestDetailRetentionDays: String(
        data.general.apiAudit.requestDetailRetentionDays,
      ),
      apiSaveRequestResponseDetails:
        data.general.apiAudit.saveRequestResponseDetails,
      autoStartEnabled: data.general.autoStartEnabled,
      hookAuditEnabled: data.general.hookAuditEnabled,
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
      llmRequestRetryCount: String(data.general.llmRequestRetryCount),
      password: "",
      theme: data.general.theme,
    });
  }

  function syncWebSearchForm(data: SettingsResponse) {
    setWebSearchForm({
      activeProvider:
        data.webSearch.activeProvider ||
        data.webSearch.providers[0]?.provider ||
        "tavily",
      apiProxyEnabled: data.webSearch.apiProxy.enabled,
      apiProxyType:
        data.webSearch.apiProxy.proxyType ||
        data.webSearch.apiProxy.supportedTypes[0]?.proxyType ||
        "http",
      apiProxyUrl: data.webSearch.apiProxy.url,
      braveApiKey: "",
      clearBraveApiKey: false,
      clearTavilyApiKey: false,
      enabled: data.webSearch.enabled,
      tavilyApiKey: "",
    });
  }

  function syncPromptSettingsForm(data: SettingsResponse) {
    const systemPrompts = normalizedSystemPromptSummaries(data.prompts);
    setPromptSettingsForm({
      activeSystemPromptName:
        systemPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)
          ?.name ??
        systemPrompts[0]?.name ??
        DEFAULT_SYSTEM_PROMPT_NAME,
      extraText: data.prompts.extraText,
      files: data.prompts.files,
      pendingFile: "",
      pendingSystemPromptName: "",
      pendingSystemPromptRename: "",
      renamingSystemPromptName: null,
      systemPrompts,
    });
  }

  function syncSpecSettingsForm(data: SettingsResponse) {
    setSpecSettingsForm({
      autoEnabled: data.spec.autoEnabled,
      generationModelId: data.spec.generationModelId ?? "",
      generationSystemPrompt: data.spec.generationSystemPrompt ?? "",
      updateSystemPrompt: data.spec.updateSystemPrompt ?? "",
    });
  }

  function syncPlanSettingsForm(data: SettingsResponse) {
    setPlanMergeAutomationMode(
      data.plan.mergeAutomationMode || "isolated_auto_once",
    );
  }

  function syncMemorySettingsForm(data: SettingsResponse) {
    setMemorySettingsForm({
      enabled: data.memory.enabled,
      extractionMode: data.memory.extractionMode,
      retrievalMode: data.memory.retrievalMode,
      extractionModelId: data.memory.extractionModelId ?? "",
      retrievalModelId: data.memory.retrievalModelId ?? "",
      retentionDays:
        data.memory.retentionDays === null ? "" : String(data.memory.retentionDays),
      dream: {
        enabled: data.memory.dream.enabled,
        autoEnabled: data.memory.dream.autoEnabled,
        mode: data.memory.dream.mode,
        modelId: data.memory.dream.modelId ?? "",
        workspaceIntervalDays: String(data.memory.dream.workspaceIntervalDays),
        globalIntervalDays: String(data.memory.dream.globalIntervalDays),
        createTranscriptChat: data.memory.dream.createTranscriptChat,
        maxFactsPerRun: String(data.memory.dream.maxFactsPerRun),
        maxChangesPerRun: String(data.memory.dream.maxChangesPerRun),
        schedulerScanMinutes: String(data.memory.dream.schedulerScanMinutes),
      },
    });
    setMemoryFilter((current) => ({
      ...current,
      workspaceId: current.workspaceId || data.workspaces[0]?.id || "",
    }));
  }

  const loadMetadata = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/model-metadata",
      );
      setMetadata(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadSettings = useCallback(async () => {
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      setSettings(data);
      onSettingsChange(data);
      setHookWorkspaceId((current) => current || data.workspaces[0]?.id || "");
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
      setDraggedModelId(null);
      setModelOrderPreview(null);
      syncGeneralForm(data);
      syncWebSearchForm(data);
      syncPromptSettingsForm(data);
      syncSpecSettingsForm(data);
      syncPlanSettingsForm(data);
      syncMemorySettingsForm(data);
      setProviderForm((current) => ({
        ...current,
        kind: current.kind || defaultProviderKind(data.providerKinds),
        serviceId:
          current.serviceId ||
          providerServiceIdForKind(
            current.kind || defaultProviderKind(data.providerKinds),
          ) ||
          "",
      }));
      setMcpForm((current) => ({
        ...current,
        transport: current.transport || data.mcpTransports[0]?.transport || "stdio",
      }));
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, [onSettingsChange]);

  const loadHooks = useCallback(async (workspaceId: string) => {
    if (!workspaceId) {
      return;
    }

    setIsLoadingHooks(true);
    setError(null);

    try {
      const data = await requestJson<HooksSettingsResponse>(
        `/api/hooks?workspaceId=${encodeURIComponent(workspaceId)}`,
      );
      setHookSettings(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingHooks(false);
    }
  }, []);

  const loadMemories = useCallback(async () => {
    setIsLoadingMemories(true);
    setError(null);

    try {
      const chatId = memoryFilter.chatId.trim();
      if (memoryFilter.scope === "chat" && !chatId) {
        setMemories([]);
        setMemoryExtractionJobs([]);
        setMemoryListMeta({
          page: 1,
          pageSize: memoryFilter.pageSize,
          totalCount: 0,
          totalPages: 0,
        });
        setSelectedMemoryId(null);
        return;
      }

      const params = new URLSearchParams({
        page: String(memoryFilter.page),
        pageSize: String(memoryFilter.pageSize),
        scope: memoryFilter.scope,
        status: memoryFilter.status,
      });
      if (memoryFilter.workspaceId) {
        params.set("workspaceId", memoryFilter.workspaceId);
      }
      if (chatId) {
        params.set("chatId", chatId);
      }
      if (memoryFilter.kind) {
        params.set("kind", memoryFilter.kind);
      }
      if (memoryFilter.query.trim()) {
        params.set("query", memoryFilter.query.trim());
      }
      const data = await requestJson<MemoryListResponse>(
        `/api/memory?${params.toString()}`,
      );
      if (data.totalPages > 0 && data.page > data.totalPages) {
        setMemoryFilter((current) =>
          current.page === data.page ? { ...current, page: data.totalPages } : current,
        );
        return;
      }
      setMemories(data.memories);
      setMemoryExtractionJobs(data.extractionJobs ?? []);
      setMemoryListMeta({
        page: data.page,
        pageSize: data.pageSize,
        totalCount: data.totalCount,
        totalPages: data.totalPages,
      });
      setSelectedMemoryId((current) =>
        current && data.memories.some((memory) => memory.id === current)
          ? current
          : data.memories[0]?.id ?? null,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingMemories(false);
    }
  }, [memoryFilter]);

  const loadMemoryDreamJobs = useCallback(async () => {
    setIsLoadingMemoryDreamJobs(true);
    setMemoryDreamError(null);

    try {
      const data = await requestJson<MemoryDreamJobsResponse>(
        "/api/memory/dream/jobs",
      );
      setMemoryDreamJobs(data.jobs);
      setMemoryDreamDetailJobId((current) =>
        current && data.jobs.some((job) => job.id === current) ? current : null,
      );
    } catch (requestError) {
      setMemoryDreamJobs([]);
      setMemoryDreamChanges([]);
      setMemoryDreamDetailJobId(null);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamJobs(false);
    }
  }, []);

  const loadMemoryDreamChanges = useCallback(async (jobId: string) => {
    setIsLoadingMemoryDreamChanges(true);
    setMemoryDreamError(null);

    try {
      const data = await requestJson<MemoryDreamChangesResponse>(
        `/api/memory/dream/jobs/${encodeURIComponent(jobId)}/changes`,
      );
      setMemoryDreamChanges(data.changes);
    } catch (requestError) {
      setMemoryDreamChanges([]);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamChanges(false);
    }
  }, []);

  const loadPlanHistory = useCallback(async () => {
    if (!effectivePlanHistoryWorkspaceId) {
      setPlanHistory([]);
      setPlanHistoryTotalCount(0);
      setPlanHistoryTotalPages(0);
      setPlanHistoryError(null);
      return;
    }

    setIsLoadingPlanHistory(true);
    setPlanHistoryError(null);

    try {
      const params = new URLSearchParams({
        page: String(planHistoryPage),
        pageSize: String(planHistoryPageSize),
        view: "all",
      });
      if (planHistoryStatus) {
        params.set("status", planHistoryStatus);
      }
      const data = await requestJson<PlansResponse>(
        `/api/workspaces/${encodeURIComponent(effectivePlanHistoryWorkspaceId)}/plans?${params.toString()}`,
      );
      if (data.totalPages > 0 && data.page > data.totalPages) {
        setPlanHistoryPage(data.totalPages);
        return;
      }
      setPlanHistory(data.plans);
      setPlanHistoryPage(data.page);
      setPlanHistoryPageSize(data.pageSize);
      setPlanHistoryTotalCount(data.totalCount);
      setPlanHistoryTotalPages(data.totalPages);
    } catch (requestError) {
      setPlanHistory([]);
      setPlanHistoryTotalCount(0);
      setPlanHistoryTotalPages(0);
      setPlanHistoryError(errorMessage(requestError));
    } finally {
      setIsLoadingPlanHistory(false);
    }
  }, [
    effectivePlanHistoryWorkspaceId,
    planHistoryPage,
    planHistoryPageSize,
    planHistoryStatus,
  ]);

  function closeMemoryDreamDetailDialog() {
    setMemoryDreamDetailJobId(null);
    setMemoryDreamChanges([]);
  }

  useEffect(() => {
    void loadMetadata();
    void loadSettings();
  }, [loadMetadata, loadSettings]);

  useEffect(() => {
    if (hookWorkspaceId) {
      void loadHooks(hookWorkspaceId);
    }
  }, [hookWorkspaceId, loadHooks]);

  useEffect(() => {
    if (activeSection === "memory") {
      void loadMemories();
    }
  }, [activeSection, loadMemories]);

  useEffect(() => {
    if (activeSection === "memory") {
      void loadMemoryDreamJobs();
    }
  }, [activeSection, loadMemoryDreamJobs]);

  useEffect(() => {
    if (activeSection === "plan") {
      void loadPlanHistory();
    }
  }, [activeSection, loadPlanHistory]);

  useEffect(() => {
    if (activeSection !== "memory" || !memoryDreamDetailJobId) {
      setMemoryDreamChanges([]);
      return;
    }

    if (!memoryDreamJobs.some((job) => job.id === memoryDreamDetailJobId)) {
      closeMemoryDreamDetailDialog();
      return;
    }

    void loadMemoryDreamChanges(memoryDreamDetailJobId);
  }, [
    activeSection,
    loadMemoryDreamChanges,
    memoryDreamDetailJobId,
    memoryDreamJobs,
  ]);

  useEffect(() => {
    if (activeSection !== "memory" || !selectedMemory) {
      setMemorySources([]);
      setMemorySourceForms([]);
      return;
    }
    const memoryForSources = selectedMemory;

    async function loadMemorySources() {
      try {
        const params = new URLSearchParams({
          memoryId: memoryForSources.id,
          scope: memoryFilter.scope,
        });
        if (memoryFilter.workspaceId) {
          params.set("workspaceId", memoryFilter.workspaceId);
        }
        const data = await requestJson<MemorySourcesResponse>(
          `/api/memory/sources?${params.toString()}`,
        );
        setMemorySources(data.sources);
        if (isMemoryDialogOpen && memoryDialogMode === "edit") {
          setMemorySourceForms(memorySourceRecordsToForm(data.sources));
        }
      } catch (requestError) {
        setError(errorMessage(requestError));
      }
    }

    void loadMemorySources();
  }, [
    activeSection,
    isMemoryDialogOpen,
    memoryFilter.scope,
    memoryFilter.workspaceId,
    memoryDialogMode,
    selectedMemory?.id,
  ]);

  useEffect(() => {
    if (workspaceDialogRevision > 0) {
      void loadSettings();
    }
  }, [loadSettings, workspaceDialogRevision]);

  function providerModelListMatches(providerId: string, modelId: string) {
    const normalizedModelId = modelId.trim();
    const modelList = providerModelLists[providerId];

    return (
      Boolean(normalizedModelId) &&
      modelList?.status === "ok" &&
      modelList.models.some((providerModelId) => providerModelId === normalizedModelId)
    );
  }

  function supportedProviderIdsForModel(
    modelId: string,
    metadataModel: ModelMetadataRecord | null = null,
    configuredModel: ConfiguredModelSummary | null = null,
  ) {
    const normalizedModelId = modelId.trim();

    if (!normalizedModelId) {
      return providers.map((provider) => provider.id);
    }

    const providerIdsFromLoadedLists = providers
      .filter((provider) => providerModelListMatches(provider.id, normalizedModelId))
      .map((provider) => provider.id);
    if (providerIdsFromLoadedLists.length) {
      return providerIdsFromLoadedLists;
    }

    if (configuredModel?.providerIds.length) {
      return configuredModel.providerIds;
    }

    if (metadataModel) {
      return [metadataModel.providerId];
    }

    return providers.map((provider) => provider.id);
  }

  function providerSupportsModel(
    provider: ConfiguredProviderSummary,
    modelId: string,
    metadataModel: ModelMetadataRecord | null,
    configuredModel: ConfiguredModelSummary | null,
  ) {
    return supportedProviderIdsForModel(modelId, metadataModel, configuredModel).includes(provider.id);
  }

  function matchedProviderIdsForModel(
    modelId: string,
    metadataModel: ModelMetadataRecord | null = null,
    configuredModel: ConfiguredModelSummary | null = null,
  ) {
    return providers
      .filter((provider) =>
        providerSupportsModel(provider, modelId, metadataModel, configuredModel),
      )
      .map((provider) => provider.id);
  }

  function formForMetadataModel(
    model: ModelMetadataRecord,
    current: ModelFormState,
  ): ModelFormState {
    const inputModalities = defaultModalities(model.inputModalities);
    const outputModalities = defaultModalities(model.outputModalities);
    const modelId = modelIdForDeveloper(
      model,
      selectedModelDeveloper || model.providerId,
    );
    const providerIds = matchedProviderIdsForModel(modelId, model);
    const nextProviderIds = providerIds.length ? providerIds : current.providerIds;
    const activeProviderId = nextProviderIds.includes(current.activeProviderId)
      ? current.activeProviderId
      : nextProviderIds[0] ?? "";

    return {
      ...current,
      displayName: model.name,
      enabled: outputModalitiesRequireLimits(outputModalities)
        ? model.contextWindow !== null && model.maxOutputTokens !== null
        : true,
      modelId,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: nextProviderIds,
      activeProviderId,
      inputModalities,
      outputModalities,
      thinkingLevel: model.reasoning ? current.thinkingLevel || thinkingLevels[0]?.value || "" : "",
      systemPromptName: current.systemPromptName || DEFAULT_SYSTEM_PROMPT_NAME,
    };
  }

  function selectModelDeveloper(developer: string) {
    setSelectedModelDeveloper(developer);
    setSelectedMetadataKey("");
    setForm((current) => ({
      ...emptyModelForm(),
      systemPromptName: current.systemPromptName || DEFAULT_SYSTEM_PROMPT_NAME,
    }));
  }

  function modelMetadataForInput(modelId: string) {
    const normalizedModelId = modelId.trim();

    if (!normalizedModelId) {
      return null;
    }

    const models = selectedModelDeveloper ? developerModels : metadata?.models ?? [];

    return (
      models.find((model) => model.key === normalizedModelId) ??
      models.find(
        (model) =>
          modelIdForDeveloper(model, selectedModelDeveloper || model.providerId) ===
          normalizedModelId,
      ) ??
      null
    );
  }

  function updateModelId(modelId: string) {
    const model = modelMetadataForInput(modelId);
    setSelectedMetadataKey(model?.key ?? "");
    if (model) {
      setSelectedModelDeveloper(model.providerId);
    }

    setForm((current) => {
      if (!model) {
        return {
          ...current,
          modelId,
          thinkingLevel: "",
        };
      }

      return formForMetadataModel(model, current);
    });
  }
  function editConfiguredModel(model: ConfiguredModelSummary) {
    setSelectedMetadataKey(model.metadataKey ?? "");
    const metadataModel = metadata?.models.find((item) => item.key === model.metadataKey);
    setSelectedModelDeveloper(metadataModel?.providerId ?? "");
    setForm({
      displayName: model.displayName,
      enabled: model.enabled,
      modelId: model.id,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: model.providerIds,
      activeProviderId: model.activeProviderId ?? "",
      inputModalities: defaultModalities(model.inputModalities),
      outputModalities: defaultModalities(model.outputModalities),
      thinkingLevel: model.supportsThinking
        ? model.thinkingLevel ?? thinkingLevels[0]?.value ?? ""
        : "",
      systemPromptName: model.systemPromptName || DEFAULT_SYSTEM_PROMPT_NAME,
    });
    setIsModelDialogOpen(true);
  }

  function startAddingModel() {
    setSelectedMetadataKey("");
    setSelectedModelDeveloper("");
    setForm(emptyModelForm());
    setIsModelDialogOpen(true);
  }

  function startAddingProviderFromModel() {
    setIsModelDialogOpen(false);
    onActiveSectionChange("providers");
    startAddingProvider();
  }

  function editConfiguredProvider(provider: ConfiguredProviderSummary) {
    setProviderForm({
      apiKey: "",
      apiProxyEnabled: provider.apiProxy.enabled,
      apiProxyType:
        provider.apiProxy.proxyType ||
        provider.apiProxy.supportedTypes[0]?.proxyType ||
        "http",
      apiProxyUrl: provider.apiProxy.url,
      baseUrl: provider.baseUrl ?? "",
      clearApiKey: false,
      enabled: provider.enabled,
      id: provider.id,
      kind: provider.kind,
      autoSyncModels: provider.autoSyncModels,
      modelSyncFilterRegex: provider.modelSyncFilterRegex ?? "",
      name: provider.name,
      requestOverrides: provider.requestOverrides.map((overrideRule) => ({
        target: overrideRule.target,
        name: overrideRule.name,
        valueType: overrideRule.valueType,
        value:
          overrideRule.valueType === "boolean"
            ? Boolean(overrideRule.value)
            : String(overrideRule.value),
      })),
      serviceId: providerServiceIdForKind(provider.kind) || "",
    });
    setIsProviderApiKeyVisible(false);
    setIsProviderDialogOpen(true);
  }

  function startAddingProvider() {
    const kind = defaultProviderKind(providerKinds);
    setProviderForm({
      ...emptyProviderForm(),
      baseUrl: providerKindDefaultBaseUrl(providerKinds, kind),
      kind,
      serviceId: providerServiceIdForKind(kind) || "",
    });
    setIsProviderApiKeyVisible(false);
    setIsProviderDialogOpen(true);
  }

  function applyProviderService(serviceId: string) {
    const service = providerServices.find((item) => item.id === serviceId);

    if (!service) {
      return;
    }

    const kind = providerDefaultKindForService(service, providerKinds);
    const baseUrl = providerKindDefaultBaseUrl(providerKinds, kind);

    setProviderForm((current) => {
      const previousService = providerServices.find(
        (item) => item.id === current.serviceId,
      );
      const shouldFillName =
        !current.name.trim() || current.name === previousService?.label;

      return {
        ...current,
        baseUrl,
        kind,
        name: shouldFillName ? service.label : current.name,
        serviceId,
      };
    });
  }

  function updateProviderProtocol(kind: string) {
    setProviderForm((current) => ({
      ...current,
      baseUrl: providerKindDefaultBaseUrl(providerKinds, kind),
      kind,
      serviceId: providerServiceIdForKind(kind) || current.serviceId,
    }));
  }

  function editConfiguredMcpServer(server: ConfiguredMcpServerSummary) {
    setMcpForm({
      argsText: server.args.join("\n"),
      command: server.command ?? "",
      enabled: server.enabled,
      id: server.id,
      name: server.name,
      transport: server.transport,
      url: server.url ?? "",
    });
    setIsMcpDialogOpen(true);
  }

  async function editConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setWorkspaceForm({
      commonCommands: workspace.commonCommands.map((command) => ({ ...command })),
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
      pinned: workspace.pinned,
      specEnabled: false,
      specInjectEnabled: false,
      terminalShell: workspace.terminalShell,
    });
    setIsWorkspaceSpecSettingsLoaded(false);
    setIsWorkspaceDialogOpen(true);
    setIsLoadingWorkspaceSpecSettings(true);
    try {
      const data = await requestJson<WorkspaceSpecResponse>(
        `/api/workspaces/${encodeURIComponent(workspace.id)}/spec`,
      );
      setWorkspaceForm((current) =>
        current.id === workspace.id
          ? {
            ...current,
            specEnabled: data.settings.enabled,
            specInjectEnabled: data.settings.injectEnabled,
          }
          : current,
      );
      setIsWorkspaceSpecSettingsLoaded(true);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingWorkspaceSpecSettings(false);
    }
  }

  function startAddingMcpServer() {
    setMcpForm({
      ...emptyMcpServerForm(),
      transport: mcpTransports[0]?.transport || "stdio",
    });
    setIsMcpDialogOpen(true);
  }

  function startAddingHookHandler() {
    setHookForm({
      ...emptyHookHandlerForm(),
      event: hookSettings?.supportedEvents[0] ?? "PreToolUse",
    });
    setIsHookDialogOpen(true);
  }

  function editHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    group: HookMatcherGroup,
    handler: HookHandler,
  ) {
    setHookForm(hookHandlerFormFromConfig(event, groupIndex, handlerIndex, group, handler));
    setIsHookDialogOpen(true);
  }

  async function refreshMetadata() {
    setIsRefreshing(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/model-metadata/refresh",
        { method: "POST" },
      );
      setMetadata(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshing(false);
    }
  }

  async function saveGeneralSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingGeneral(true);
    setError(null);

    try {
      const password = generalForm.password;
      const shouldSaveAutoStart =
        generalForm.autoStartEnabled ||
        Boolean(
          settings &&
          generalForm.autoStartEnabled !== settings.general.autoStartEnabled,
        );
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          apiAudit: {
            requestDetailRetentionDays: optionalPositiveInteger(
              generalForm.apiRequestDetailRetentionDays,
              t("API request detail retention days"),
            ),
            saveRequestResponseDetails: generalForm.apiSaveRequestResponseDetails,
          },
          ...(shouldSaveAutoStart
            ? { autoStartEnabled: generalForm.autoStartEnabled }
            : {}),
          clearPassword: false,
          listenHost: generalForm.listenHost,
          listenPort: optionalPositiveInteger(
            generalForm.listenPort,
            t("Listen port"),
          ),
          llmRequestRetryCount: optionalPositiveInteger(
            generalForm.llmRequestRetryCount,
            t("LLM request retries"),
          ),
          hookAuditEnabled: generalForm.hookAuditEnabled,
          language: generalForm.language,
          password: password.trim() ? password : null,
          theme: generalForm.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingGeneral(false);
    }
  }

  async function saveWebSearchSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWebSearch(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/web-search", {
        body: JSON.stringify({
          activeProvider: webSearchForm.activeProvider,
          apiProxy: {
            enabled: webSearchForm.apiProxyEnabled,
            proxyType: webSearchForm.apiProxyType,
            url: webSearchForm.apiProxyUrl,
          },
          braveApiKey: webSearchForm.braveApiKey.trim() || null,
          clearBraveApiKey: webSearchForm.clearBraveApiKey,
          clearTavilyApiKey: webSearchForm.clearTavilyApiKey,
          enabled: webSearchForm.enabled,
          tavilyApiKey: webSearchForm.tavilyApiKey.trim() || null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncWebSearchForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWebSearch(false);
    }
  }

  async function saveLanguageSetting(language: string) {
    setGeneralForm((current) => ({
      ...current,
      language,
    }));

    if (!settings) {
      return;
    }

    setIsSavingLanguage(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setGeneralForm((current) => ({
        ...current,
        language: data.general.language,
        theme: data.general.theme,
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        language: settings.general.language,
        theme: settings.general.theme,
      }));
    } finally {
      setIsSavingLanguage(false);
    }
  }

  async function saveThemeSetting(theme: AppThemeId) {
    setGeneralForm((current) => ({
      ...current,
      theme,
    }));

    if (!settings) {
      return;
    }

    setIsSavingTheme(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        theme: settings.general.theme,
      }));
    } finally {
      setIsSavingTheme(false);
    }
  }

  async function savePromptSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingPromptSettings(true);
    setError(null);

    try {
      const files = promptSettingsForm.files.map((file) => file.trim());
      const data = await requestJson<SettingsResponse>("/api/settings/prompts", {
        body: JSON.stringify({
          extraText: promptSettingsForm.extraText,
          files,
          systemPrompts: promptSettingsForm.systemPrompts,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncPromptSettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingPromptSettings(false);
    }
  }

  async function saveSpecSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingSpecSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/spec", {
        body: JSON.stringify({
          autoEnabled: specSettingsForm.autoEnabled,
          generationModelId: specSettingsForm.generationModelId.trim() || null,
          generationSystemPrompt:
            specSettingsForm.generationSystemPrompt.trim() || null,
          updateSystemPrompt: specSettingsForm.updateSystemPrompt.trim() || null,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSpecSettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingSpecSettings(false);
    }
  }

  async function savePlanSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingPlanSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/plan", {
        body: JSON.stringify({
          mergeAutomationMode: planMergeAutomationMode,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncPlanSettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingPlanSettings(false);
    }
  }

  function addSystemPrompt(name: string) {
    const nextName = name.trim();
    if (!nextName) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      if (currentSystemPrompts.some((prompt) => prompt.name === nextName)) {
        return {
          ...current,
          activeSystemPromptName: nextName,
          pendingSystemPromptName: "",
          systemPrompts: currentSystemPrompts,
        };
      }

      return {
        ...current,
        activeSystemPromptName: nextName,
        pendingSystemPromptName: "",
        systemPrompts: [
          ...currentSystemPrompts,
          {
            name: nextName,
            content: "",
          },
        ],
      };
    });
  }

  function removeSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      const systemPrompts = currentSystemPrompts.filter(
        (prompt) => prompt.name !== name,
      );
      return {
        ...current,
        activeSystemPromptName:
          current.activeSystemPromptName === name
            ? DEFAULT_SYSTEM_PROMPT_NAME
            : current.activeSystemPromptName,
        pendingSystemPromptRename:
          current.renamingSystemPromptName === name
            ? ""
            : current.pendingSystemPromptRename,
        renamingSystemPromptName:
          current.renamingSystemPromptName === name
            ? null
            : current.renamingSystemPromptName,
        systemPrompts,
      };
    });
  }

  function startRenameSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => ({
      ...current,
      activeSystemPromptName: name,
      pendingSystemPromptRename: name,
      renamingSystemPromptName: name,
    }));
  }

  function cancelRenameSystemPrompt() {
    setPromptSettingsForm((current) => ({
      ...current,
      pendingSystemPromptRename: "",
      renamingSystemPromptName: null,
    }));
  }

  function submitRenameSystemPrompt(name: string) {
    if (isSystemPromptFixed(name)) {
      return;
    }

    setPromptSettingsForm((current) => {
      const nextName = current.pendingSystemPromptRename.trim();
      if (!nextName) {
        return current;
      }

      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      if (
        currentSystemPrompts.some(
          (prompt) => prompt.name !== name && prompt.name === nextName,
        )
      ) {
        return current;
      }

      return {
        ...current,
        activeSystemPromptName: nextName,
        pendingSystemPromptRename: "",
        renamingSystemPromptName: null,
        systemPrompts: currentSystemPrompts.map((prompt) =>
          prompt.name === name
            ? {
              ...prompt,
              name: nextName,
            }
            : prompt,
        ),
      };
    });
  }

  function updateActiveSystemPromptContent(content: string) {
    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      return {
        ...current,
        systemPrompts: currentSystemPrompts.map((prompt) =>
          prompt.name === current.activeSystemPromptName
            ? {
              ...prompt,
              content,
            }
            : prompt,
        ),
      };
    });
  }

  function defaultSystemPromptContent(name: string) {
    if (!settings) {
      return null;
    }
    if (name === DEFAULT_SYSTEM_PROMPT_NAME) {
      return settings.prompts.defaultSystemPrompt;
    }
    if (name === PLAN_MODE_SYSTEM_PROMPT_NAME) {
      return settings.prompts.defaultPlanModeSystemPrompt ?? null;
    }
    if (name === REVIEW_SYSTEM_PROMPT_NAME) {
      return settings.prompts.defaultReviewSystemPrompt ?? null;
    }
    return null;
  }

  function restoreSystemPromptDefault(name: string) {
    const defaultContent = defaultSystemPromptContent(name);
    if (defaultContent === null) {
      return;
    }

    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      const hasPrompt = currentSystemPrompts.some((prompt) => prompt.name === name);
      const systemPrompts = hasPrompt
        ? currentSystemPrompts.map((prompt) =>
          prompt.name === name
            ? {
              ...prompt,
              content: defaultContent,
            }
            : prompt,
          )
        : [
          ...currentSystemPrompts,
          {
            content: defaultContent,
            name,
          },
        ];

      return {
        ...current,
        activeSystemPromptName: name,
        systemPrompts,
      };
    });
  }

  function addPromptFilePath(path: string) {
    const nextPath = path.trim();
    if (!nextPath) {
      return;
    }

    setPromptSettingsForm((current) => {
      if (current.files.includes(nextPath)) {
        return {
          ...current,
          pendingFile: "",
        };
      }

      return {
        ...current,
        files: [...current.files, nextPath],
        pendingFile: "",
      };
    });
  }

  function removePromptFilePath(path: string) {
    setPromptSettingsForm((current) => ({
      ...current,
      files: current.files.filter((file) => file !== path),
    }));
  }

  async function selectPromptFile() {
    if (!nativeBrowserToken) {
      setError(
        t(
          "Native file browsing is only available from a browser running on the Foco computer.",
        ),
      );
      return;
    }

    setIsSelectingPromptFile(true);
    setError(null);

    try {
      const data = await requestJson<{ files: NativeSelectedFile[] }>(
        "/api/native/select-files",
        nativePickerRequestInit(nativeBrowserToken),
      );
      setPromptSettingsForm((current) => {
        const files = [...current.files];
        for (const file of data.files) {
          if (!files.includes(file.path)) {
            files.push(file.path);
          }
        }
        return {
          ...current,
          files,
        };
      });
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingPromptFile(false);
    }
  }

  async function clearBrowserPassword() {
    if (!settings?.general.webServer.passwordEnabled) {
      return;
    }

    setIsClearingPassword(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: true,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsClearingPassword(false);
    }
  }

  async function saveMemorySettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingMemorySettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/memory", {
        body: JSON.stringify({
          enabled: memorySettingsForm.enabled,
          extractionMode: memorySettingsForm.extractionMode,
          retrievalMode: memorySettingsForm.retrievalMode,
          extractionModelId: memorySettingsForm.extractionModelId.trim() || null,
          retrievalModelId: memorySettingsForm.retrievalModelId.trim() || null,
          retentionDays: optionalPositiveInteger(
            memorySettingsForm.retentionDays,
            t("Retention days"),
          ),
          dream: {
            enabled: memorySettingsForm.dream.enabled,
            autoEnabled: memorySettingsForm.dream.autoEnabled,
            mode: memorySettingsForm.dream.mode,
            modelId: memorySettingsForm.dream.modelId.trim() || null,
            workspaceIntervalDays: requiredPositiveInteger(
              memorySettingsForm.dream.workspaceIntervalDays,
              t("Workspace interval days"),
            ),
            globalIntervalDays: requiredPositiveInteger(
              memorySettingsForm.dream.globalIntervalDays,
              t("Global interval days"),
            ),
            createTranscriptChat: memorySettingsForm.dream.createTranscriptChat,
            maxFactsPerRun: requiredPositiveInteger(
              memorySettingsForm.dream.maxFactsPerRun,
              t("Max facts per run"),
            ),
            maxChangesPerRun: requiredPositiveInteger(
              memorySettingsForm.dream.maxChangesPerRun,
              t("Max changes per run"),
            ),
            schedulerScanMinutes: requiredPositiveInteger(
              memorySettingsForm.dream.schedulerScanMinutes,
              t("Scheduler scan minutes"),
            ),
          },
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncMemorySettingsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemorySettings(false);
    }
  }

  async function runMemoryDream(scope: MemoryDreamScope) {
    const workspaceId = scope === "workspace" ? memoryDreamWorkspaceId : null;
    const runKey = memoryDreamJobKey(scope, workspaceId);
    setMemoryDreamRunKey(runKey);
    setMemoryDreamError(null);

    try {
      await requestJson<MemoryDreamRunResponse>("/api/memory/dream/run", {
        body: JSON.stringify({
          scope,
          ...(workspaceId ? { workspaceId } : {}),
          triggerType: "manual",
          mode: memorySettingsForm.dream.mode,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemoryDreamJobs();
    } catch (requestError) {
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setMemoryDreamRunKey(null);
    }
  }

  function updateMemoryDreamForm(
    patch: Partial<MemorySettingsFormState["dream"]>,
  ) {
    setMemorySettingsForm((current) => ({
      ...current,
      dream: {
        ...current.dream,
        ...patch,
      },
    }));
  }

  function updateMemoryFilter(patch: Partial<MemoryFilterState>) {
    setMemoryFilter((current) => ({
      ...current,
      ...patch,
      page: 1,
    }));
  }

  function goToMemoryDreamPage(page: number) {
    const maxPage = memoryDreamTotalPages || 1;
    setMemoryDreamPage(Math.min(Math.max(1, page), maxPage));
  }

  function updateMemoryDreamPageSize(value: string) {
    setMemoryDreamPage(1);
    setMemoryDreamPageSize((current) =>
      Math.min(
        MEMORY_DREAM_MAX_PAGE_SIZE,
        positiveIntegerText(value, current),
      ),
    );
  }

  function goToPlanHistoryPage(page: number) {
    const maxPage = planHistoryTotalPages || 1;
    setPlanHistoryPage(Math.min(Math.max(1, page), maxPage));
  }

  function updatePlanHistoryPageSize(value: string) {
    setPlanHistoryPage(1);
    setPlanHistoryPageSize((current) =>
      Math.min(100, positiveIntegerText(value, current)),
    );
  }

  async function runPlanHistoryAction(planId: string, action: string) {
    if (!effectivePlanHistoryWorkspaceId) {
      setPlanHistoryError(t("Select a workspace first."));
      return;
    }

    const operationKey = `${action}:${planId}`;
    setPlanHistoryOperationKey(operationKey);
    setPlanHistoryError(null);

    try {
      const response = await requestJson<PlanResponse>(
        `/api/workspaces/${encodeURIComponent(effectivePlanHistoryWorkspaceId)}/plans/${encodeURIComponent(planId)}/action`,
        {
          body: JSON.stringify({ action }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      await loadPlanHistory();
      await onWorkspacesChange();
      const implementationChatId =
        action === "start" || action === "resume"
          ? response.plan.phases.find(
            (phase) => phase.id === response.plan.activePhaseId,
          )?.implementationChatId ?? null
          : null;
      if (implementationChatId) {
        onOpenChat(effectivePlanHistoryWorkspaceId, implementationChatId);
      }
    } catch (requestError) {
      setPlanHistoryError(errorMessage(requestError));
    } finally {
      setPlanHistoryOperationKey((current) =>
        current === operationKey ? null : current,
      );
    }
  }

  function handleMemoryDreamHistoryWheel(event: ReactWheelEvent<HTMLDivElement>) {
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) {
      return;
    }

    const deltaUnit =
      event.deltaMode === 1
        ? 16
        : event.deltaMode === 2
          ? event.currentTarget.clientHeight
          : 1;
    let node: HTMLElement | null = event.currentTarget.parentElement;
    while (node) {
      const overflowY = window.getComputedStyle(node).overflowY;
      if (/(auto|scroll)/.test(overflowY) && node.scrollHeight > node.clientHeight) {
        node.scrollTop += event.deltaY * deltaUnit;
        event.preventDefault();
        return;
      }
      node = node.parentElement;
    }
  }

  function goToMemoryPage(page: number) {
    if (!isMemoryFilterReady) {
      return;
    }

    setMemoryFilter((current) => ({
      ...current,
      page,
    }));
  }

  function updateMemoryPageSize(value: string) {
    setMemoryFilter((current) => ({
      ...current,
      page: 1,
      pageSize: Math.min(200, positiveIntegerText(value, current.pageSize)),
    }));
  }

  function openCreateMemoryDialog() {
    setMemoryDialogMode("create");
    setMemorySourceForms([]);
    setExpandedMemoryJsonIds(new Set());
    setManualMemoryForm({
      ...emptyManualMemoryForm(),
      chatId: memoryFilter.chatId,
      scope: memoryFilter.scope,
      workspaceId: memoryFilter.workspaceId || workspaces[0]?.id || "",
    });
    setIsMemoryDialogOpen(true);
  }

  function openEditMemoryDialog(memory: MemoryFactRecord) {
    const isCurrentSelection = selectedMemoryId === memory.id;
    setSelectedMemoryId(memory.id);
    setMemoryDialogMode("edit");
    setMemorySourceForms(
      isCurrentSelection ? memorySourceRecordsToForm(memorySources) : [],
    );
    setExpandedMemoryJsonIds(new Set());
    setManualMemoryForm({
      chatId: memory.chatId ?? "",
      confidence: memory.confidence === null ? "" : String(memory.confidence),
      fact: memory.fact,
      kind: memory.kind,
      metadataText: prettyJsonText(memory.metadataJson),
      pinned: memory.pinned,
      scope: memory.scope as ManualMemoryFormState["scope"],
      workspaceId: memoryFilter.workspaceId || workspaces[0]?.id || "",
    });
    setIsMemoryDialogOpen(true);
  }

  function closeMemoryDialog() {
    setIsMemoryDialogOpen(false);
    setMemoryDialogMode("create");
    setManualMemoryForm(emptyManualMemoryForm());
    setMemorySourceForms([]);
    setExpandedMemoryJsonIds(new Set());
  }

  function updateMemorySourceForm(
    sourceId: string,
    field: keyof Omit<MemorySourceFormState, "id">,
    value: string,
  ) {
    setMemorySourceForms((current) =>
      current.map((source) =>
        source.id === sourceId ? { ...source, [field]: value } : source,
      ),
    );
  }

  function toggleMemoryJson(id: string) {
    setExpandedMemoryJsonIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function promoteMemoryOneLevel(memory: MemoryFactRecord) {
    if (memory.scope === "chat") {
      void promoteMemory(memory.id, "workspace");
    } else if (memory.scope === "workspace") {
      void promoteMemory(memory.id, "global");
    }
  }

  async function saveMemoryDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (memoryDialogMode === "edit" && !selectedMemory) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      const scope = manualMemoryForm.scope;
      const workspaceId =
        scope === "global" ? null : manualMemoryForm.workspaceId || memoryFilter.workspaceId;
      const metadata = parseJsonText(manualMemoryForm.metadataText || "{}", t("Memory metadata"));
      const payload =
        memoryDialogMode === "create"
          ? {
            chatId: scope === "chat" ? manualMemoryForm.chatId : null,
            confidence: optionalNumber(manualMemoryForm.confidence, t("Confidence")),
            fact: manualMemoryForm.fact,
            kind: manualMemoryForm.kind,
            metadata,
            pinned: manualMemoryForm.pinned,
            scope,
            workspaceId,
          }
          : {
            confidence: optionalNumber(manualMemoryForm.confidence, t("Confidence")),
            fact: manualMemoryForm.fact,
            kind: manualMemoryForm.kind,
            memoryId: selectedMemory?.id,
            metadata,
            pinned: manualMemoryForm.pinned,
            scope: memoryFilter.scope,
            sources: memorySourceForms.map((source) => ({
              content: source.content,
              id: source.id,
              metadata: parseJsonText(
                source.metadataText || "{}",
                `${t("Source metadata")} ${source.id}`,
              ),
              title: source.title,
            })),
            workspaceId:
              memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
          };
      const endpoint =
        memoryDialogMode === "create" ? "/api/memory/manual" : "/api/memory/edit";
      await requestJson<MemoryMutationResponse>(endpoint, {
        body: JSON.stringify(payload),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      closeMemoryDialog();
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function setMemoryStatus(memoryId: string, status: string) {
    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/status", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          status,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function forgetMemory(memoryId: string) {
    if (!window.confirm(t("Delete memory confirmation"))) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/forget", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function clearFilteredMemories() {
    if (!canClearFilteredMemories) {
      return;
    }
    if (!window.confirm(t("Clear filtered memories confirmation"))) {
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<ClearMemoriesResponse>("/api/memory/clear", {
        body: JSON.stringify({
          chatId: memoryFilter.scope === "chat" ? memoryFilter.chatId : null,
          kind: memoryFilter.kind || null,
          query: memoryFilter.query.trim() || null,
          scope: memoryFilter.scope,
          status: memoryFilter.status,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const shouldReload = memoryFilter.page === 1;
      setMemoryFilter((current) => ({
        ...current,
        page: 1,
      }));
      if (shouldReload) {
        await loadMemories();
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function promoteMemory(memoryId: string, targetScope: "workspace" | "global") {
    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<MemoryMutationResponse>("/api/memory/promote", {
        body: JSON.stringify({
          memoryId,
          scope: memoryFilter.scope,
          targetChatId: null,
          targetScope,
          targetWorkspaceId:
            targetScope === "global" ? null : memoryFilter.workspaceId,
          workspaceId:
            memoryFilter.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function updateMemoryExtractionJob(jobId: string, action: "retry" | "skip") {
    if (!memoryFilter.workspaceId) {
      setError(t("Workspace is required"));
      return;
    }

    setIsSavingMemory(true);
    setError(null);

    try {
      await requestJson<{ job: MemoryExtractionJobSummary }>(
        `/api/memory/extraction/${action}`,
        {
          body: JSON.stringify({
            jobId,
            workspaceId: memoryFilter.workspaceId,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      await loadMemories();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function saveModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>(
        "/api/models/manual",
        {
          body: JSON.stringify({
            displayName: form.displayName,
            enabled: form.enabled,
            metadataKey: selectedMetadataKey || null,
            modelId: form.modelId,
            contextWindow: optionalModelLimit(
              form.contextWindow,
              "Context window",
              modelOutputsText,
            ),
            maxOutputTokens: optionalModelLimit(
              form.maxOutputTokens,
              "Max output tokens",
              modelOutputsText,
            ),
            providerIds: modelProviderIds,
            activeProviderId: activeModelProviderId,
            inputModalities: normalizeModalities(form.inputModalities),
            outputModalities: normalizeModalities(form.outputModalities),
            thinkingLevel: form.thinkingLevel || null,
            clearThinkingLevel: !form.thinkingLevel,
            systemPromptName: form.systemPromptName,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setMetadata(data);
      await loadSettings();
      setIsModelDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  async function toggleConfiguredModelEnabled(
    model: ConfiguredModelSummary,
    enabled: boolean,
  ) {
    setIsSaving(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>("/api/models/manual", {
        body: JSON.stringify({
          displayName: model.displayName,
          enabled,
          metadataKey: model.metadataKey,
          modelId: model.id,
          contextWindow: model.contextWindow,
          maxOutputTokens: model.maxOutputTokens,
          providerIds: model.providerIds,
          activeProviderId: model.activeProviderId ?? "",
          inputModalities: model.inputModalities,
          outputModalities: model.outputModalities,
          thinkingLevel: model.thinkingLevel,
          clearThinkingLevel: !model.thinkingLevel,
          systemPromptName: model.systemPromptName,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setMetadata(data);
      await loadSettings();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  async function saveModelOrder(modelIds: string[]) {
    setIsSavingModelOrder(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/models/order", {
        body: JSON.stringify({ modelIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setDraggedModelId(null);
      setModelOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
    } finally {
      setIsSavingModelOrder(false);
    }
  }

  async function saveWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    const shouldSaveOrder = editingWorkspace?.pinned !== workspaceForm.pinned;
    const workspaceIds = shouldSaveOrder
      ? groupedWorkspaceIds(
        orderedWorkspaces.map((workspace) =>
          workspace.id === workspaceForm.id
            ? { ...workspace, pinned: workspaceForm.pinned }
            : workspace,
        ),
      )
      : null;

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/manual", {
        body: JSON.stringify({
          id: workspaceForm.id,
          name: workspaceForm.name,
          path: workspaceForm.path,
          pinned: workspaceForm.pinned,
          terminalShell: workspaceForm.terminalShell,
          commonCommands: workspaceForm.commonCommands,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const finalData = workspaceIds
        ? await requestJson<SettingsResponse>("/api/workspaces/order", {
          body: JSON.stringify({ workspaceIds }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        })
        : data;
      setSettings(finalData);
      onSettingsChange(finalData);
      await onWorkspacesChange();
      if (editingWorkspace && isWorkspaceSpecSettingsLoaded) {
        try {
          await saveWorkspaceSpecSettingsRequest(
            workspaceForm.id,
            workspaceForm.specEnabled,
            workspaceForm.specEnabled ? workspaceForm.specInjectEnabled : false,
          );
        } catch (specError) {
          setError(errorMessage(specError));
        }
      }
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  function addWorkspaceCommonCommand() {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: [...current.commonCommands, { name: "", command: "" }],
    }));
  }

  function updateWorkspaceCommonCommand(
    index: number,
    field: keyof WorkspaceCommonCommandSummary,
    value: string,
  ) {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: current.commonCommands.map((command, commandIndex) =>
        commandIndex === index ? { ...command, [field]: value } : command,
      ),
    }));
  }

  function removeWorkspaceCommonCommand(index: number) {
    setWorkspaceForm((current) => ({
      ...current,
      commonCommands: current.commonCommands.filter(
        (_command, commandIndex) => commandIndex !== index,
      ),
    }));
  }

  async function saveWorkspaceOrder(workspaceIds: string[]) {
    setIsSavingWorkspaceOrder(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/order", {
        body: JSON.stringify({ workspaceIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
      await onWorkspacesChange();
    } finally {
      setIsSavingWorkspaceOrder(false);
    }
  }

  async function toggleWorkspacePinned(
    workspace: ConfiguredWorkspaceSummary,
    pinned: boolean,
  ) {
    setIsSavingWorkspaceOrder(true);
    setError(null);

    const nextWorkspaces = orderedWorkspaces.map((item) =>
      item.id === workspace.id ? { ...item, pinned } : item,
    );
    const workspaceIds = groupedWorkspaceIds(nextWorkspaces);

    try {
      await requestJson<SettingsResponse>("/api/workspaces/manual", {
        body: JSON.stringify({
          id: workspace.id,
          name: workspace.name,
          path: workspace.path,
          pinned,
          terminalShell: workspace.terminalShell,
          commonCommands: workspace.commonCommands,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const orderData = await requestJson<SettingsResponse>("/api/workspaces/order", {
        body: JSON.stringify({ workspaceIds }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(orderData);
      onSettingsChange(orderData);
      setWorkspaceForm((current) =>
        current.id === workspace.id ? { ...current, pinned } : current,
      );
      await onWorkspacesChange();
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
      await loadSettings();
      await onWorkspacesChange();
    } finally {
      setIsSavingWorkspaceOrder(false);
    }
  }

  async function selectWorkspaceFormPath() {
    if (!nativeBrowserToken) {
      setError(
        t(
          "Native file browsing is only available from a browser running on the Foco computer.",
        ),
      );
      return;
    }

    setIsSelectingWorkspaceFormPath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        nativePickerRequestInit(nativeBrowserToken),
      );

      if (data.path) {
        setWorkspaceForm((current) => ({
          ...current,
          name: current.name.trim()
            ? current.name
            : workspaceNameFromPath(data.path ?? ""),
          path: data.path ?? current.path,
        }));
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingWorkspaceFormPath(false);
    }
  }

  async function saveWorkspaceLogo(contentBase64: string) {
    if (!editingWorkspace) {
      return;
    }

    setIsSavingWorkspaceLogo(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        `/api/workspaces/${encodeURIComponent(editingWorkspace.id)}/logo`,
        {
          body: JSON.stringify({ contentBase64 }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspaceLogo(false);
    }
  }

  async function clearWorkspaceLogo() {
    if (!editingWorkspace?.logoUrl) {
      return;
    }

    setIsSavingWorkspaceLogo(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        `/api/workspaces/${encodeURIComponent(editingWorkspace.id)}/logo`,
        { method: "DELETE" },
      );
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspaceLogo(false);
    }
  }

  async function uploadWorkspaceLogoFile(file: File) {
    try {
      const contentBase64 = await fileToBase64(file);
      await saveWorkspaceLogo(contentBase64);
    } catch (readError) {
      setError(errorMessage(readError));
    }
  }

  function handleWorkspaceLogoFileChange(
    event: ReactChangeEvent<HTMLInputElement>,
  ) {
    const file = event.target.files?.[0] ?? null;
    event.target.value = "";
    if (!file) {
      return;
    }

    void uploadWorkspaceLogoFile(file);
  }

  function handleWorkspaceDragStart(
    event: ReactDragEvent<HTMLElement>,
    workspaceId: string,
  ) {
    setDraggedWorkspaceId(workspaceId);
    setWorkspaceOrderPreview(orderedWorkspaces.map((workspace) => workspace.id));
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", workspaceId);
  }

  function handleWorkspaceDragOver(
    event: ReactDragEvent<HTMLDivElement>,
    targetWorkspaceId: string,
  ) {
    event.preventDefault();

    const sourceWorkspaceId = draggedWorkspaceId;
    if (!sourceWorkspaceId || sourceWorkspaceId === targetWorkspaceId) {
      return;
    }

    const sourceWorkspace = orderedWorkspaces.find(
      (workspace) => workspace.id === sourceWorkspaceId,
    );
    const targetWorkspace = orderedWorkspaces.find(
      (workspace) => workspace.id === targetWorkspaceId,
    );
    if (!sourceWorkspace || !targetWorkspace || sourceWorkspace.pinned !== targetWorkspace.pinned) {
      return;
    }

    const workspaceIds = moveItemId(
      workspaceOrderPreview ?? orderedWorkspaces.map((workspace) => workspace.id),
      sourceWorkspaceId,
      targetWorkspaceId,
    );
    setWorkspaceOrderPreview(workspaceIds);
  }

  async function handleWorkspaceDrop(event: ReactDragEvent<HTMLDivElement>) {
    event.preventDefault();

    const workspaceIds = workspaceOrderPreview;
    setDraggedWorkspaceId(null);

    if (!workspaceIds || sameStringList(workspaceIds, workspaces.map((workspace) => workspace.id))) {
      setWorkspaceOrderPreview(null);
      return;
    }

    await saveWorkspaceOrder(workspaceIds);
  }

  function handleWorkspaceDragEnd() {
    setDraggedWorkspaceId(null);
    setWorkspaceOrderPreview(null);
  }

  function addProviderRequestOverride() {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: [
        ...current.requestOverrides,
        emptyProviderRequestOverride(),
      ],
    }));
  }

  function updateProviderRequestOverride(
    index: number,
    patch: Partial<ProviderRequestOverrideFormState>,
  ) {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: current.requestOverrides.map((overrideRule, overrideIndex) => {
        if (overrideIndex !== index) {
          return overrideRule;
        }

        const nextRule = { ...overrideRule, ...patch };
        if (patch.valueType === "boolean" && typeof nextRule.value !== "boolean") {
          nextRule.value = true;
        } else if (patch.valueType === "string" && typeof nextRule.value !== "string") {
          nextRule.value = String(nextRule.value);
        } else if (patch.valueType === "number" && typeof nextRule.value !== "number") {
          nextRule.value = "";
        }

        return nextRule;
      }),
    }));
  }

  function deleteProviderRequestOverride(index: number) {
    setProviderForm((current) => ({
      ...current,
      requestOverrides: current.requestOverrides.filter(
        (_overrideRule, overrideIndex) => overrideIndex !== index,
      ),
    }));
  }

  async function saveProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingProvider(true);
    setError(null);

    try {
      const providerId =
        providerForm.id ||
        nextProviderId(providerForm.name, providerForm.kind, providers);
      const data = await requestJson<SettingsResponse>(
        "/api/providers/manual",
        {
          body: JSON.stringify({
            apiKey: providerForm.apiKey || null,
            apiProxy: {
              enabled: providerForm.apiProxyEnabled,
              proxyType: providerForm.apiProxyType,
              url: providerForm.apiProxyUrl,
            },
            baseUrl: providerForm.baseUrl || null,
            clearApiKey: providerForm.clearApiKey,
            enabled: providerForm.enabled,
            id: providerId,
            kind: providerForm.kind,
            autoSyncModels: providerForm.autoSyncModels,
            modelSyncFilterRegex: providerForm.modelSyncFilterRegex || null,
            name: providerForm.name,
            requestOverrides: providerForm.requestOverrides.map((overrideRule) => ({
              ...overrideRule,
              value:
                overrideRule.valueType === "number"
                  ? Number(overrideRule.value)
                  : overrideRule.value,
            })),
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setExpandedProviderIds((current) => {
        const next = new Set(current);
        next.delete(providerId);
        return next;
      });
      setProviderModelLists((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      setProviderForm((current) => ({
        ...current,
        apiKey: "",
        clearApiKey: false,
      }));
      setIsProviderApiKeyVisible(false);
      setIsProviderDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingProvider(false);
    }
  }

  async function saveMcpServer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingMcpServer(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/mcp/servers/manual",
        {
          body: JSON.stringify({
            args: mcpForm.argsText
              .split(/\r?\n/)
              .map((arg) => arg.trim())
              .filter(Boolean),
            command: mcpForm.command || null,
            enabled: mcpForm.enabled,
            id:
              mcpForm.id ||
              nextMcpServerId(mcpForm.name, mcpForm.transport, mcpServers),
            name: mcpForm.name,
            transport: mcpForm.transport,
            url: mcpForm.url || null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setIsMcpDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMcpServer(false);
    }
  }

  async function deleteProvider(providerId: string) {
    setIsSavingProvider(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/providers/delete", {
        body: JSON.stringify({ id: providerId }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setExpandedProviderIds((current) => {
        const next = new Set(current);
        next.delete(providerId);
        return next;
      });
      setProviderModelLists((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      setProviderForm({
        ...emptyProviderForm(),
        kind: defaultProviderKind(data.providerKinds),
      });
      setIsProviderApiKeyVisible(false);
      setIsProviderDialogOpen(false);
      setForm((current) => ({
        ...current,
        activeProviderId:
          current.activeProviderId === providerId
            ? ""
            : current.activeProviderId,
        providerIds: current.providerIds.filter((id) => id !== providerId),
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingProvider(false);
    }
  }

  async function deleteMcpServer(serverId: string) {
    setIsSavingMcpServer(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/mcp/servers/delete",
        {
          body: JSON.stringify({ id: serverId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setMcpForm({
        ...emptyMcpServerForm(),
        transport: data.mcpTransports[0]?.transport || "stdio",
      });
      setIsMcpDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingMcpServer(false);
    }
  }

  async function deleteModel(modelId: string) {
    if (!window.confirm(t("Delete model confirmation"))) {
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const data = await requestJson<ModelMetadataResponse>("/api/models/delete", {
        body: JSON.stringify({ id: modelId }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setMetadata(data);
      await loadSettings();
      setSelectedMetadataKey("");
      setForm(emptyModelForm());
      setIsModelDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSaving(false);
    }
  }

  async function saveSkills(nextEnabledSkillIds: Set<string>) {
    setIsSavingSkills(true);
    setError(null);

    try {
      const disabledSkillIds = (skills?.detected ?? [])
        .filter((skill) => !nextEnabledSkillIds.has(skill.key))
        .map((skill) => skill.key);
      const data = await requestJson<SettingsResponse>("/api/skills/manual", {
        body: JSON.stringify({
          disabled: disabledSkillIds,
          enabled: Array.from(nextEnabledSkillIds),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingSkills(false);
    }
  }

  async function refreshSkills() {
    setIsRefreshingSkills(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/skills/refresh", {
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncSkillsForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingSkills(false);
    }
  }

  async function saveHookAuditEnabled(hookAuditEnabled: boolean) {
    if (!settings) {
      return;
    }

    setGeneralForm((current) => ({
      ...current,
      hookAuditEnabled,
    }));
    setIsSavingGeneral(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      syncGeneralForm(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        hookAuditEnabled: settings.general.hookAuditEnabled,
      }));
    } finally {
      setIsSavingGeneral(false);
    }
  }

  async function saveDefaultTeamModeEnabled(defaultTeamModeEnabled: boolean) {
    if (!settings) {
      return;
    }

    setIsSavingGeneral(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          defaultTeamModeEnabled,
          hookAuditEnabled: settings.general.hookAuditEnabled,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
          theme: settings.general.theme,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingGeneral(false);
    }
  }

  async function refreshHookRuns() {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsRefreshingHookRuns(true);
    setError(null);

    try {
      const data = await requestJson<HookRunsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/hooks/runs?limit=50`,
      );
      setHookSettings((current) =>
        current ? { ...current, recentRuns: data.runs } : current,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingHookRuns(false);
    }
  }

  async function saveHookConfig(nextConfig: HookConfig) {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsSavingHooks(true);
    setError(null);
    setHookImportResult(null);

    try {
      const url =
        hookScope === "global" ? "/api/hooks/global" : "/api/hooks/workspace";
      const body =
        hookScope === "global"
          ? { config: nextConfig }
          : { workspaceId, config: nextConfig };
      const data = await requestJson<HooksSettingsResponse>(url, {
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setHookSettings(data);
      setIsHookDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingHooks(false);
    }
  }

  async function submitHookForm(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    try {
      const currentConfig = activeHookConfig ?? emptyHookConfig();
      const nextConfig = upsertHookHandlerInConfig(currentConfig, hookForm);
      await saveHookConfig(nextConfig);
    } catch (formError) {
      setError(errorMessage(formError));
    }
  }

  function updateHookConfig(nextConfig: HookConfig) {
    void saveHookConfig(nextConfig);
  }

  function deleteHookHandler(event: string, groupIndex: number, handlerIndex: number) {
    const nextConfig = deleteHookHandlerFromConfig(
      activeHookConfig ?? emptyHookConfig(),
      event,
      groupIndex,
      handlerIndex,
    );
    updateHookConfig(nextConfig);
  }

  function toggleHookGroup(event: string, groupIndex: number, enabled: boolean) {
    updateHookConfig(
      updateHookGroupInConfig(activeHookConfig ?? emptyHookConfig(), event, groupIndex, {
        enabled,
      }),
    );
  }

  function toggleHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    enabled: boolean,
  ) {
    updateHookConfig(
      updateHookHandlerInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        handlerIndex,
        { enabled },
      ),
    );
  }

  function moveHookGroup(event: string, groupIndex: number, direction: -1 | 1) {
    updateHookConfig(
      moveHookGroupInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        direction,
      ),
    );
  }

  function moveHookHandler(
    event: string,
    groupIndex: number,
    handlerIndex: number,
    direction: -1 | 1,
  ) {
    updateHookConfig(
      moveHookHandlerInConfig(
        activeHookConfig ?? emptyHookConfig(),
        event,
        groupIndex,
        handlerIndex,
        direction,
      ),
    );
  }

  async function importClaudeHooks(target: HookScope) {
    const workspaceId = selectedHookWorkspace?.id;
    if (target === "workspace" && !workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsImportingHooks(true);
    setError(null);
    setHookImportResult(null);

    try {
      const data = await requestJson<ImportClaudeHooksResponse>(
        "/api/hooks/import-claude",
        {
          body: JSON.stringify({
            target,
            workspaceId: target === "workspace" ? workspaceId : null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setHookImportResult(data);
      await loadHooks(workspaceId);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsImportingHooks(false);
    }
  }

  async function testHooks(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      setError(t("Select a workspace first."));
      return;
    }

    setIsTestingHooks(true);
    setError(null);
    setHookTestResult(null);

    try {
      const parsedPayload = parseJsonText(hookTestPayload, t("Sample payload"));
      const data = await requestJson<HookRunSummary>("/api/hooks/test", {
        body: JSON.stringify({
          event: hookTestEvent,
          matchValue: hookTestMatcher.trim() || null,
          payload: parsedPayload,
          workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setHookTestResult(data);
      await loadHooks(workspaceId);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsTestingHooks(false);
    }
  }

  async function openHookRunDetail(runId: string) {
    const workspaceId = selectedHookWorkspace?.id;
    if (!workspaceId) {
      return;
    }

    setError(null);
    try {
      const data = await requestJson<HookRunDetailResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/hooks/runs/${encodeURIComponent(runId)}`,
      );
      setHookRunDetail(data.run);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  async function testProvider(providerId: string) {
    setProviderTests((current) => ({
      ...current,
      [providerId]: { message: t("Testing connection..."), status: "testing" },
    }));
    setError(null);

    try {
      const data = await requestJson<ProviderTestResponse>(
        "/api/providers/test",
        {
          body: JSON.stringify({ providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setProviderTests((current) => ({
        ...current,
        [providerId]: { message: data.message, status: data.ok ? "ok" : "error" },
      }));
    } catch (requestError) {
      setProviderTests((current) => ({
        ...current,
        [providerId]: {
          message: errorMessage(requestError),
          status: "error",
        },
      }));
    }
  }

  async function loadProviderModels(providerId: string) {
    setProviderModelLists((current) => ({
      ...current,
      [providerId]: { message: null, models: [], status: "loading" },
    }));
    setError(null);

    try {
      const data = await requestJson<ProviderModelsResponse>(
        "/api/providers/models",
        {
          body: JSON.stringify({ providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setProviderModelLists((current) => ({
        ...current,
        [providerId]: {
          message: null,
          models: data.models,
          status: "ok",
        },
      }));
    } catch (requestError) {
      setProviderModelLists((current) => ({
        ...current,
        [providerId]: {
          message: errorMessage(requestError),
          models: [],
          status: "error",
        },
      }));
    }
  }

  async function refreshProviderModels() {
    setIsRefreshingProviderModels(true);
    setError(null);

    try {
      const data = await requestJson<ProviderModelsRefreshResponse>(
        "/api/providers/models/refresh",
        { method: "POST" },
      );
      setSettings(data.settings);
      onSettingsChange(data.settings);
      setProviderTests({});
      setProviderModelLists((current) => {
        const next = { ...current };
        for (const provider of data.providers) {
          next[provider.providerId] = {
            message: null,
            models: provider.models,
            status: "ok",
          };
        }
        return next;
      });
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingProviderModels(false);
    }
  }

  function toggleProviderModels(providerId: string) {
    const shouldExpand = !expandedProviderIds.has(providerId);
    setExpandedProviderIds((current) => {
      const next = new Set(current);
      if (next.has(providerId)) {
        next.delete(providerId);
      } else {
        next.add(providerId);
      }
      return next;
    });

    if (shouldExpand && !providerModelLists[providerId]) {
      void loadProviderModels(providerId);
    }
  }

  function toggleModelProvider(providerId: string, checked: boolean) {
    setForm((current) => {
      const metadataModel = modelMetadataForInput(current.modelId);
      const configuredModel = configuredModels.find((model) => model.id === current.modelId) ?? null;
      const matchedProviderIds = matchedProviderIdsForModel(
        current.modelId,
        metadataModel,
        configuredModel,
      );
      if (checked && matchedProviderIds.length && !matchedProviderIds.includes(providerId)) {
        return current;
      }
      const baseProviderIds = matchedProviderIds.length
        ? current.providerIds.filter((id) => matchedProviderIds.includes(id))
        : current.providerIds;
      const providerIds = checked
        ? [...baseProviderIds, providerId].filter(uniqueString)
        : baseProviderIds.filter((id) => id !== providerId);
      const activeProviderId = providerIds.includes(current.activeProviderId)
        ? current.activeProviderId
        : providerIds[0] ?? "";

      return {
        ...current,
        activeProviderId,
        providerIds,
      };
    });
  }

  function toggleModelModality(
    field: "inputModalities" | "outputModalities",
    modality: string,
    checked: boolean,
  ) {
    setForm((current) => {
      const values = checked
        ? [...current[field], modality]
        : current[field].filter((value) => value !== modality);

      return {
        ...current,
        [field]: normalizeModalities(values),
      };
    });
  }

  function toggleSkill(skillId: string, checked: boolean) {
    const next = new Set(enabledSkillIds);

    if (checked) {
      next.add(skillId);
    } else {
      next.delete(skillId);
    }

    setEnabledSkillIds(next);
    void saveSkills(next);
  }

  function handleModelDragStart(
    event: ReactDragEvent<HTMLDivElement>,
    modelId: string,
  ) {
    setDraggedModelId(modelId);
    setModelOrderPreview(orderedConfiguredModels.map((model) => model.id));
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", modelId);
  }

  function handleModelDragOver(
    event: ReactDragEvent<HTMLDivElement>,
    targetModelId: string,
  ) {
    event.preventDefault();

    const sourceModelId = draggedModelId;
    if (!sourceModelId || sourceModelId === targetModelId) {
      return;
    }

    const modelIds = moveItemId(
      modelOrderPreview ?? orderedConfiguredModels.map((model) => model.id),
      sourceModelId,
      targetModelId,
    );
    setModelOrderPreview(modelIds);
  }

  async function handleModelDrop(event: ReactDragEvent<HTMLDivElement>) {
    event.preventDefault();

    const modelIds = modelOrderPreview;
    setDraggedModelId(null);

    if (!modelIds || sameStringList(modelIds, configuredModels.map((model) => model.id))) {
      setModelOrderPreview(null);
      return;
    }

    await saveModelOrder(modelIds);
  }

  function handleModelDragEnd() {
    setDraggedModelId(null);
    setModelOrderPreview(null);
  }

  return (
    <div className="settings-shell panel-scroll min-h-0 flex-1 overflow-y-auto">
      <div className="settings-layout grid">
        <aside className="settings-section-nav-card flex min-h-0 flex-col border-stone-200 bg-white p-2">
          <div className="settings-sidebar-header workspace-sidebar-header flex items-center justify-between gap-2 border-b border-stone-200/80 px-4 py-2">
            <div className="min-w-0">
              <span className="workspace-sidebar-title">{t("Settings")}</span>
            </div>
          </div>
          <nav
            aria-label={t("Settings")}
            className="settings-section-nav flex flex-col gap-1.5"
          >
            <SettingsNavButton
              active={activeSection === "general"}
              icon={Globe}
              label={t("General")}
              onClick={() => onActiveSectionChange("general")}
            />
            <SettingsNavButton
              active={activeSection === "agents"}
              icon={Bot}
              label={t("Agents")}
              onClick={() => onActiveSectionChange("agents")}
            />
            <SettingsNavButton
              active={activeSection === "prompts"}
              icon={ScrollText}
              label={t("Prompts")}
              onClick={() => onActiveSectionChange("prompts")}
            />
            <SettingsNavButton
              active={activeSection === "spec"}
              icon={FileText}
              label={t("Spec")}
              onClick={() => onActiveSectionChange("spec")}
            />
            <SettingsNavButton
              active={activeSection === "plan"}
              icon={ListChecks}
              label={t("Plan settings")}
              onClick={() => onActiveSectionChange("plan")}
            />
            <SettingsNavButton
              active={activeSection === "web-search"}
              icon={Search}
              label={t("Web Search")}
              onClick={() => onActiveSectionChange("web-search")}
            />
            <SettingsNavButton
              active={activeSection === "workspaces"}
              icon={Folder}
              label={t("Workspaces")}
              onClick={() => onActiveSectionChange("workspaces")}
            />
            <SettingsNavButton
              active={activeSection === "hooks"}
              icon={Webhook}
              label={t("Hooks")}
              onClick={() => onActiveSectionChange("hooks")}
            />
            <SettingsNavButton
              active={activeSection === "memory"}
              icon={Brain}
              label={t("Memory")}
              onClick={() => onActiveSectionChange("memory")}
            />
            <SettingsNavButton
              active={activeSection === "providers"}
              icon={PlugZap}
              label={t("Providers")}
              onClick={() => onActiveSectionChange("providers")}
            />
            <SettingsNavButton
              active={activeSection === "models"}
              icon={SlidersHorizontal}
              label={t("Models")}
              onClick={() => onActiveSectionChange("models")}
            />
            <SettingsNavButton
              active={activeSection === "mcp"}
              icon={Server}
              label={t("MCP")}
              onClick={() => onActiveSectionChange("mcp")}
            />
            <SettingsNavButton
              active={activeSection === "skills"}
              icon={Wrench}
              label={t("Skills")}
              onClick={() => onActiveSectionChange("skills")}
            />
          </nav>
        </aside>

        <div className="min-w-0 flex flex-col gap-5">
          <section className="rounded-2xl border border-stone-200 bg-white/75 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="text-lg font-semibold text-stone-950">
                  {settingsSectionTitle(activeSection, t)}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {activeSection === "models"
                    ? metadata?.fetchedAt
                      ? t("Fetched {time} from {source}", {
                        time: metadata.fetchedAt,
                        source: metadata.sourceUrl ?? "",
                      })
                      : t("Model metadata has not been refreshed")
                    : settingsSectionSubtitle(activeSection, t)}
                </p>
              </div>
              {activeSection === "models" ? (
                <button
                  aria-label={t("Refresh model metadata")}
                  className="inline-flex size-10 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                  disabled={isRefreshing}
                  onClick={() => void refreshMetadata()}
                  title={t("Refresh model metadata")}
                  type="button"
                >
                  {isRefreshing ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw aria-hidden="true" className="size-4" />
                  )}
                </button>
              ) : null}
            </div>
          </section>

          {error ? (
            <div className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}

          {activeSection === "general" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveGeneralSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <Globe aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Web service")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <TextField
                    label={t("Listen address")}
                    onChange={(value) =>
                      setGeneralForm((current) => ({
                        ...current,
                        listenHost: value,
                      }))
                    }
                    placeholder="127.0.0.1"
                    value={generalForm.listenHost}
                  />
                  <TextField
                    inputMode="numeric"
                    label={t("Listen port")}
                    onChange={(value) =>
                      setGeneralForm((current) => ({
                        ...current,
                        listenPort: value,
                      }))
                    }
                    placeholder="3210"
                    value={generalForm.listenPort}
                  />
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("LLM request retries")}
                    </span>
                    <input
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      inputMode="numeric"
                      min={1}
                      onChange={(event) =>
                        setGeneralForm((current) => ({
                          ...current,
                          llmRequestRetryCount: event.target.value,
                        }))
                      }
                      placeholder={String(settings?.general.llmRequestRetryCount ?? 3)}
                      step={1}
                      type="number"
                      value={generalForm.llmRequestRetryCount}
                    />
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("API request detail retention days")}
                    </span>
                    <input
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      inputMode="numeric"
                      min={1}
                      onChange={(event) =>
                        setGeneralForm((current) => ({
                          ...current,
                          apiRequestDetailRetentionDays: event.target.value,
                        }))
                      }
                      placeholder={String(
                        settings?.general.apiAudit.requestDetailRetentionDays ?? 3,
                      )}
                      step={1}
                      type="number"
                      value={generalForm.apiRequestDetailRetentionDays}
                    />
                  </label>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("API request details")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Database
                          aria-hidden="true"
                          className="size-4 shrink-0 text-teal-700"
                        />
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Save request and response bodies")}
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <CapabilityPill
                          label={
                            generalForm.apiSaveRequestResponseDetails
                              ? t("enabled")
                              : t("disabled")
                          }
                          ok={generalForm.apiSaveRequestResponseDetails}
                        />
                        <label
                          aria-label={t("Save request and response bodies")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                        >
                          <input
                            checked={generalForm.apiSaveRequestResponseDetails}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setGeneralForm((current) => ({
                                ...current,
                                apiSaveRequestResponseDetails: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                      </div>
                    </div>
                  </fieldset>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Startup")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Play
                          aria-hidden="true"
                          className="size-4 shrink-0 fill-current text-teal-700"
                        />
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Start Foco when Windows starts")}
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <CapabilityPill
                          label={
                            generalForm.autoStartEnabled ? t("enabled") : t("disabled")
                          }
                          ok={generalForm.autoStartEnabled}
                        />
                        <label
                          aria-label={t("Start Foco when Windows starts")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                        >
                          <input
                            checked={generalForm.autoStartEnabled}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setGeneralForm((current) => ({
                                ...current,
                                autoStartEnabled: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                      </div>
                    </div>
                  </fieldset>
                </div>
                <div className="mt-4 border-t border-stone-200 pt-4">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2">
                      <Lock aria-hidden="true" className="size-4 text-teal-700" />
                      <h4 className="text-sm font-semibold text-stone-950">
                        {t("Browser authentication")}
                      </h4>
                    </div>
                    <CapabilityPill
                      label={
                        settings?.general.webServer.passwordEnabled
                          ? t("Password is enabled")
                          : t("Password is disabled")
                      }
                      ok={Boolean(settings?.general.webServer.passwordEnabled)}
                    />
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {canLogout ? (
                      <button
                        aria-label={t("Log out")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={() => void onLogout()}
                        title={t("Log out")}
                        type="button"
                      >
                        <Lock aria-hidden="true" className="size-4" />
                        {t("Log out")}
                      </button>
                    ) : null}
                    <button
                      aria-label={t("Clear browser password")}
                      className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-sm font-semibold text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:border-stone-200 disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={
                        isClearingPassword ||
                        !settings?.general.webServer.passwordEnabled
                      }
                      onClick={() => void clearBrowserPassword()}
                      title={t("Clear browser password")}
                      type="button"
                    >
                      {isClearingPassword ? (
                        <LoaderCircle
                          aria-hidden="true"
                          className="size-4 animate-spin"
                        />
                      ) : (
                        <X aria-hidden="true" className="size-4" />
                      )}
                      {t("Clear browser password")}
                    </button>
                  </div>
                  <div className="mt-3 grid gap-3">
                    <div className="grid gap-2">
                      <label className="block min-w-0">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Authentication password")}
                        </span>
                        <span className="relative block">
                          <input
                            autoComplete="new-password"
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 pr-10 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setGeneralForm((current) => ({
                                ...current,
                                password: event.target.value,
                              }))
                            }
                            onBlur={() => {
                              if (!generalForm.password) {
                                setIsEditingGeneralPassword(false);
                              }
                            }}
                            onFocus={() => setIsEditingGeneralPassword(true)}
                            placeholder={
                              settings?.general.webServer.passwordEnabled
                                ? t("New password is kept empty unless changed.")
                                : t("Set a password to require browser login.")
                            }
                            type={isGeneralPasswordVisible ? "text" : "password"}
                            value={passwordInputValue}
                          />
                          <button
                            aria-label={
                              isGeneralPasswordVisible
                                ? t("Hide password")
                                : t("Show password")
                            }
                            className="absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                            disabled={!generalForm.password}
                            onClick={() =>
                              setIsGeneralPasswordVisible((current) => !current)
                            }
                            title={
                              isGeneralPasswordVisible
                                ? t("Hide password")
                                : t("Show password")
                            }
                            type="button"
                          >
                            {isGeneralPasswordVisible ? (
                              <EyeOff aria-hidden="true" className="size-4" />
                            ) : (
                              <Eye aria-hidden="true" className="size-4" />
                            )}
                          </button>
                        </span>
                        {settings?.general.webServer.passwordEnabled ? (
                          <span className="mt-1 block text-xs text-stone-500">
                            {t(
                              "Saved password cannot be revealed; type a new password to preview it.",
                            )}
                          </span>
                        ) : null}
                      </label>
                    </div>
                  </div>
                </div>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Language")}
                  </span>
                  <select
                    className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    disabled={isSavingLanguage || isLoadingSettings}
                    onChange={(event) => void saveLanguageSetting(event.target.value)}
                    value={generalForm.language}
                  >
                    {(settings?.general.supportedLanguages ?? []).map((language) => (
                      <option key={language.id} value={language.id}>
                        {language.name}
                      </option>
                    ))}
                  </select>
                  <span className="mt-1 block text-xs text-stone-500">
                    {t("Language changes apply immediately after saving.")}
                  </span>
                </label>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Theme")}
                  </span>
                  <select
                    className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    disabled={isSavingTheme || isLoadingSettings}
                    onChange={(event) =>
                      void saveThemeSetting(event.target.value as AppThemeId)
                    }
                    value={generalForm.theme}
                  >
                    {(settings?.general.supportedThemes ?? []).map((theme) => (
                      <option key={theme.id} value={theme.id}>
                        {t(theme.name)}
                      </option>
                    ))}
                  </select>
                  <span className="mt-1 block text-xs text-stone-500">
                    {t("Theme changes apply immediately after saving.")}
                  </span>
                </label>
                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    aria-label={t("Save general settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={
                      isSavingGeneral ||
                      !generalForm.listenHost.trim() ||
                      !generalForm.listenPort.trim() ||
                      !generalForm.apiRequestDetailRetentionDays.trim()
                    }
                    title={t("Save general settings")}
                    type="submit"
                  >
                    {isSavingGeneral ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                  <button
                    aria-label={t("Reload general settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isLoadingSettings}
                    onClick={() => void loadSettings()}
                    title={t("Reload settings")}
                    type="button"
                  >
                    {isLoadingSettings ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <RefreshCw aria-hidden="true" className="size-4" />
                    )}
                    {t("Reload")}
                  </button>
                </div>
              </form>

              <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Saved bind")}
                  </h3>
                  <CapabilityPill label={t("restart required")} ok={false} />
                </div>
                <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                  <div className="break-all text-sm font-semibold text-stone-950">
                    {settings
                      ? `${settings.general.webServer.listenHost}:${settings.general.webServer.listenPort}`
                      : t("Loading...")}
                  </div>
                  <div className="mt-2 text-xs text-stone-500">
                    {t("Saved host and port are used the next time the backend starts.")}
                  </div>
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "web-search" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveWebSearchSettings(event)}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Search aria-hidden="true" className="size-5 text-teal-700" />
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("Web search")}
                    </h3>
                  </div>
                  <CapabilityPill
                    label={webSearchForm.enabled ? t("enabled") : t("disabled")}
                    ok={webSearchForm.enabled}
                  />
                </div>
                <div className="mt-4 grid gap-4">
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Runtime tool")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Expose web_search to chat runs")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-stone-500">
                          {t(
                            "web_fetch is available for known URLs; web_search requires an enabled search API.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Expose web_search to chat runs")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={webSearchForm.enabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Search API")}
                    </span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setWebSearchForm((current) => ({
                          ...current,
                          activeProvider: event.target.value,
                        }))
                      }
                      value={webSearchForm.activeProvider}
                    >
                      {(settings?.webSearch.providers ?? []).map((provider) => (
                        <option key={provider.provider} value={provider.provider}>
                          {provider.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Web search proxy")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Proxy search API requests")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-stone-500">
                          {t("Applies only to web_search requests sent to the configured search API.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable web search proxy")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={webSearchForm.apiProxyEnabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              apiProxyEnabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                    <div className="mt-3 grid gap-3 lg:grid-cols-[180px_1fr]">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Proxy type")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setWebSearchForm((current) => ({
                              ...current,
                              apiProxyType: event.target.value,
                            }))
                          }
                          value={webSearchForm.apiProxyType}
                        >
                          {(settings?.webSearch.apiProxy.supportedTypes ?? apiProxyTypes).map(
                            (proxyType) => (
                              <option
                                key={proxyType.proxyType}
                                value={proxyType.proxyType}
                              >
                                {proxyType.label}
                              </option>
                            ),
                          )}
                        </select>
                      </label>
                      <TextField
                        label={t("Proxy server")}
                        onChange={(value) =>
                          setWebSearchForm((current) => ({
                            ...current,
                            apiProxyUrl: value,
                          }))
                        }
                        placeholder="127.0.0.1:7890"
                        value={webSearchForm.apiProxyUrl}
                      />
                    </div>
                  </fieldset>
                  <div className="grid gap-3 lg:grid-cols-2">
                    {(settings?.webSearch.providers ?? []).map((provider) => {
                      const keyField =
                        provider.provider === "brave"
                          ? "braveApiKey"
                          : "tavilyApiKey";
                      const clearField =
                        provider.provider === "brave"
                          ? "clearBraveApiKey"
                          : "clearTavilyApiKey";

                      return (
                        <div
                          className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3"
                          key={provider.provider}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-sm font-semibold text-stone-900">
                              {provider.label}
                            </span>
                            <CapabilityPill
                              label={provider.hasApiKey ? t("saved") : t("missing")}
                              ok={provider.hasApiKey}
                            />
                          </div>
                          <label className="mt-3 block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("API token")}
                            </span>
                            <input
                              autoComplete="off"
                              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                setWebSearchForm((current) => ({
                                  ...current,
                                  [keyField]: event.target.value,
                                }))
                              }
                              placeholder={
                                provider.hasApiKey
                                  ? t("Saved token is kept unless changed.")
                                  : t("Paste API token")
                              }
                              type="password"
                              value={String(webSearchForm[keyField])}
                            />
                          </label>
                          {provider.hasApiKey ? (
                            <label className="mt-3 flex items-center gap-2 text-xs font-semibold text-stone-600">
                              <input
                                checked={Boolean(webSearchForm[clearField])}
                                className="size-4 accent-teal-700"
                                onChange={(event) =>
                                  setWebSearchForm((current) => ({
                                    ...current,
                                    [clearField]: event.target.checked,
                                  }))
                                }
                                type="checkbox"
                              />
                              {t("Clear saved token")}
                            </label>
                          ) : null}
                        </div>
                      );
                    })}
                  </div>
                </div>
                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    aria-label={t("Save web search settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={isSavingWebSearch || !webSearchForm.activeProvider}
                    title={t("Save web search settings")}
                    type="submit"
                  >
                    {isSavingWebSearch ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                  <button
                    aria-label={t("Reload web search settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isLoadingSettings}
                    onClick={() => void loadSettings()}
                    title={t("Reload settings")}
                    type="button"
                  >
                    {isLoadingSettings ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <RefreshCw aria-hidden="true" className="size-4" />
                    )}
                    {t("Reload")}
                  </button>
                </div>
              </form>
            </section>
          ) : null}

          {activeSection === "agents" ? (
            <AgentsSettingsPanel
              agentTools={settings?.agentTools ?? []}
              defaultTeamModeEnabled={settings?.general.defaultTeamModeEnabled ?? false}
              defaultRolePrompts={defaultAgentRolePrompts}
              definitions={agentDefinitions}
              error={agentDefinitionsError}
              isLoading={isLoadingAgentDefinitions}
              isSavingDefaultTeamMode={isSavingGeneral}
              models={configuredModelsByName}
              onCreateDefinition={onCreateAgentDefinition}
              onDefaultTeamModeEnabledChange={saveDefaultTeamModeEnabled}
              onDeleteDefinition={onDeleteAgentDefinition}
              onUpdateDefinition={onUpdateAgentDefinition}
              operationKey={agentDefinitionOperationKey}
              providers={providers}
              thinkingLevels={thinkingLevels}
            />
          ) : null}

          {activeSection === "prompts" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void savePromptSettings(event)}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Bot aria-hidden="true" className="size-5 text-teal-700" />
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("System prompt")}
                    </h3>
                  </div>
                  <span className="rounded-full border border-stone-200 bg-stone-50 px-2.5 py-1 text-xs font-semibold text-stone-600">
                    {activeSystemPrompt?.name ?? DEFAULT_SYSTEM_PROMPT_NAME}
                  </span>
                </div>
                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(180px,240px)_minmax(0,1fr)]">
                  <div className="grid content-start gap-2">
                    <div className="overflow-hidden rounded-xl border border-stone-200 bg-stone-50/80">
                      {systemPrompts.map((prompt) => {
                        const isActive =
                          prompt.name === promptSettingsForm.activeSystemPromptName;
                        const isRenaming =
                          prompt.name === promptSettingsForm.renamingSystemPromptName;
                        const isFixed = isSystemPromptFixed(prompt.name);

                        return (
                          <div
                            className={`flex items-center gap-2 px-3 py-2 ${isActive
                                ? "bg-teal-50"
                                : "hover:bg-white"
                              }`}
                            key={prompt.name}
                          >
                            {isRenaming ? (
                              <>
                                <input
                                  aria-label={t("System prompt name")}
                                  autoComplete="off"
                                  autoFocus
                                  className="h-8 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-2 text-sm font-semibold text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                  onChange={(event) =>
                                    setPromptSettingsForm((current) => ({
                                      ...current,
                                      pendingSystemPromptRename: event.target.value,
                                    }))
                                  }
                                  onKeyDown={(event) => {
                                    if (event.key === "Enter") {
                                      event.preventDefault();
                                      submitRenameSystemPrompt(prompt.name);
                                    }
                                    if (event.key === "Escape") {
                                      event.preventDefault();
                                      cancelRenameSystemPrompt();
                                    }
                                  }}
                                  value={promptSettingsForm.pendingSystemPromptRename}
                                />
                                <button
                                  aria-label={t("Save system prompt name")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                                  disabled={
                                    !promptSettingsForm.pendingSystemPromptRename.trim()
                                  }
                                  onClick={() => submitRenameSystemPrompt(prompt.name)}
                                  title={t("Save system prompt name")}
                                  type="button"
                                >
                                  <CheckCircle2 aria-hidden="true" className="size-4" />
                                </button>
                                <button
                                  aria-label={t("Cancel system prompt rename")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                  onClick={cancelRenameSystemPrompt}
                                  title={t("Cancel system prompt rename")}
                                  type="button"
                                >
                                  <X aria-hidden="true" className="size-4" />
                                </button>
                              </>
                            ) : (
                              <>
                                <button
                                  className={`min-w-0 flex-1 truncate text-left text-sm font-semibold ${isActive
                                      ? "text-teal-900"
                                      : "text-stone-700"
                                    }`}
                                  onClick={() =>
                                    setPromptSettingsForm((current) => ({
                                      ...current,
                                      activeSystemPromptName: prompt.name,
                                    }))
                                  }
                                  type="button"
                                >
                                  {prompt.name}
                                </button>
                                {defaultSystemPromptContent(prompt.name) !== null ? (
                                  <button
                                    aria-label={t("Restore default system prompt")}
                                    className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                    disabled={isLoadingSettings || !settings}
                                    onClick={() => restoreSystemPromptDefault(prompt.name)}
                                    title={t("Restore default system prompt")}
                                    type="button"
                                  >
                                    <RefreshCw aria-hidden="true" className="size-4" />
                                  </button>
                                ) : isFixed ? null : (
                                  <>
                                    <button
                                      aria-label={t("Rename system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                      onClick={() => startRenameSystemPrompt(prompt.name)}
                                      title={t("Rename system prompt")}
                                      type="button"
                                    >
                                      <Pencil aria-hidden="true" className="size-4" />
                                    </button>
                                    <button
                                      aria-label={t("Remove system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                                      onClick={() => removeSystemPrompt(prompt.name)}
                                      title={t("Remove system prompt")}
                                      type="button"
                                    >
                                      <Trash2 aria-hidden="true" className="size-4" />
                                    </button>
                                  </>
                                )}
                              </>
                            )}
                          </div>
                        );
                      })}
                    </div>
                    <div className="flex gap-2">
                      <input
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          setPromptSettingsForm((current) => ({
                            ...current,
                            pendingSystemPromptName: event.target.value,
                          }))
                        }
                        placeholder={t("Prompt name")}
                        value={promptSettingsForm.pendingSystemPromptName}
                      />
                      <button
                        aria-label={t("Add system prompt")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={!promptSettingsForm.pendingSystemPromptName.trim()}
                        onClick={() =>
                          addSystemPrompt(promptSettingsForm.pendingSystemPromptName)
                        }
                        title={t("Add system prompt")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("System prompt")}
                    </span>
                    <textarea
                      className="min-h-72 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateActiveSystemPromptContent(event.target.value)
                      }
                      value={activeSystemPrompt?.content ?? ""}
                    />
                  </label>
                </div>
                <div className="mt-6 flex items-center gap-2 border-t border-stone-200 pt-4">
                  <ScrollText aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Prompt files")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Prompt file path")}
                    </span>
                    <div className="flex gap-2">
                      <input
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        name="prompt-file-path"
                        onChange={(event) =>
                          setPromptSettingsForm((current) => ({
                            ...current,
                            pendingFile: event.target.value,
                          }))
                        }
                        placeholder="C:/Users/name/.codex/AGENTS.md"
                        value={promptSettingsForm.pendingFile}
                      />
                      <button
                        aria-label={t("Add prompt file")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={!promptSettingsForm.pendingFile.trim()}
                        onClick={() => addPromptFilePath(promptSettingsForm.pendingFile)}
                        title={t("Add prompt file")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                      <button
                        aria-label={t("Choose prompt file")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={isSelectingPromptFile || !canUseNativePicker}
                        onClick={() => void selectPromptFile()}
                        title={
                          canUseNativePicker
                            ? t("Choose prompt file")
                            : t("Local Foco browser required")
                        }
                        type="button"
                      >
                        {isSelectingPromptFile ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <FolderSearch aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </div>
                  </label>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80">
                    {promptSettingsForm.files.length ? (
                      <div className="divide-y divide-stone-200">
                        {promptSettingsForm.files.map((file) => (
                          <div
                            className="flex min-w-0 items-center justify-between gap-3 px-3 py-2"
                            key={file}
                          >
                            <div className="min-w-0 break-all text-sm font-semibold text-stone-800">
                              {file}
                            </div>
                            <button
                              aria-label={t("Remove prompt file {path}", { path: file })}
                              className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                              onClick={() => removePromptFilePath(file)}
                              title={t("Remove prompt file")}
                              type="button"
                            >
                              <Trash2 aria-hidden="true" className="size-4" />
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="px-3 py-6 text-center text-sm font-medium text-stone-500">
                        {t("No prompt files")}
                      </div>
                    )}
                  </div>
                </div>

                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Extra prompt")}
                  </span>
                  <textarea
                    className="min-h-36 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        extraText: event.target.value,
                      }))
                    }
                    placeholder={t("Extra prompt")}
                    value={promptSettingsForm.extraText}
                  />
                </label>

                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    aria-label={t("Save prompt settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={isSavingPromptSettings}
                    title={t("Save prompt settings")}
                    type="submit"
                  >
                    {isSavingPromptSettings ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                    )}
                    {t("Save")}
                  </button>
                </div>
              </form>
            </section>
          ) : null}

          {activeSection === "spec" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveSpecSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <FileText aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Auto Spec")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Automation")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Enable Auto Spec")}
                        </p>
                        <p className="mt-1 text-xs text-stone-500">
                          {t("Updates enabled workspace specs after successful chat turns.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable Auto Spec")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={specSettingsForm.autoEnabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setSpecSettingsForm((current) => ({
                              ...current,
                              autoEnabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec generation model")}
                    </span>
                    <select
                      aria-label={t("Spec generation model")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          generationModelId: event.target.value,
                        }))
                      }
                      value={specSettingsForm.generationModelId}
                    >
                      <option value="">{t("Automatic")}</option>
                      {configuredModelsByName.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.displayName}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec generation system prompt")}
                    </span>
                    <textarea
                      aria-label={t("Spec generation system prompt")}
                      className="min-h-44 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          generationSystemPrompt: event.target.value,
                        }))
                      }
                      placeholder={settings?.spec.defaultGenerationSystemPrompt ?? ""}
                      value={specSettingsForm.generationSystemPrompt}
                    />
                  </label>

                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Spec update system prompt")}
                    </span>
                    <textarea
                      aria-label={t("Spec update system prompt")}
                      className="min-h-44 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setSpecSettingsForm((current) => ({
                          ...current,
                          updateSystemPrompt: event.target.value,
                        }))
                      }
                      placeholder={settings?.spec.defaultUpdateSystemPrompt ?? ""}
                      value={specSettingsForm.updateSystemPrompt}
                    />
                  </label>
                </div>

                <button
                  aria-label={t("Save spec settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={isSavingSpecSettings}
                  title={t("Save spec settings")}
                  type="submit"
                >
                  {isSavingSpecSettings ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                  {t("Save")}
                </button>
              </form>
            </section>
          ) : null}

          {activeSection === "plan" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void savePlanSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <ListChecks aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Plan automation")}
                  </h3>
                </div>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                    {t("Merge automation")}
                  </span>
                  <select
                    aria-label={t("Merge automation")}
                    className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) => setPlanMergeAutomationMode(event.target.value)}
                    value={planMergeAutomationMode}
                  >
                    {(settings?.plan.mergeAutomationModes ?? []).map((mode) => (
                      <option key={mode.value} value={mode.value}>
                        {t(mode.label)}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  aria-label={t("Save plan settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={isSavingPlanSettings}
                  title={t("Save plan settings")}
                  type="submit"
                >
                  {isSavingPlanSettings ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                  {t("Save")}
                </button>
              </form>

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("Plan history")}
                    </h3>
                    <p className="mt-1 truncate text-xs text-stone-500">
                      {planWorkspace?.name ?? t("No workspace selected")}
                    </p>
                  </div>
                  <button
                    aria-label={t("Refresh plan history")}
                    className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isLoadingPlanHistory || !effectivePlanHistoryWorkspaceId}
                    onClick={() => void loadPlanHistory()}
                    title={t("Refresh plan history")}
                    type="button"
                  >
                    {isLoadingPlanHistory ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <RefreshCw aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>

                <div className="grid gap-3 border-b border-stone-200 px-4 py-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Workspace")}
                    </span>
                    <select
                      aria-label={t("Workspace")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) => {
                        setPlanHistoryPage(1);
                        setPlanHistoryWorkspaceId(event.target.value);
                      }}
                      value={effectivePlanHistoryWorkspaceId}
                    >
                      {workspaces.map((workspace) => (
                        <option key={workspace.id} value={workspace.id}>
                          {workspace.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Plan status")}
                    </span>
                    <select
                      aria-label={t("Plan status")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) => {
                        setPlanHistoryPage(1);
                        setPlanHistoryStatus(event.target.value);
                      }}
                      value={planHistoryStatus}
                    >
                      <option value="">{t("All statuses")}</option>
                      {[
                        "draft",
                        "ready",
                        "running",
                        "paused",
                        "implemented",
                        "completed",
                        "failed",
                        "cancelled",
                      ].map((status) => (
                        <option key={status} value={status}>
                          {t(planStatusLabel(status))}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                {planHistoryError ? (
                  <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
                    {planHistoryError}
                  </div>
                ) : null}

                <div className="divide-y divide-stone-100">
                  {!effectivePlanHistoryWorkspaceId ? (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No workspace selected")}
                    </div>
                  ) : planHistory.length ? (
                    planHistory.map((plan) => {
                      const action = planHistoryAction(plan.status);
                      const operationKey = action ? `${action}:${plan.id}` : null;
                      const totalSteps = plan.phases.reduce(
                        (count, phase) => count + phase.steps.length,
                        0,
                      );
                      const completedSteps = plan.phases.reduce(
                        (count, phase) =>
                          count + phase.steps.filter((step) => step.status === "completed").length,
                        0,
                      );

                      return (
                        <article className="px-4 py-3" key={plan.id}>
                          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                            <div className="min-w-0">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="text-sm font-semibold text-stone-950">
                                  {plan.title}
                                </span>
                                <CapabilityPill
                                  label={t(planStatusLabel(plan.status))}
                                  ok={plan.status === "completed" || plan.status === "implemented"}
                                  tone={planStatusTone(plan.status)}
                                />
                                <CapabilityPill
                                  label={`${completedSteps}/${totalSteps}`}
                                  ok={completedSteps === totalSteps && totalSteps > 0}
                                />
                              </div>
                              <p className="mt-1 line-clamp-2 text-xs leading-5 text-stone-500">
                                {plan.overview}
                              </p>
                              <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-stone-500">
                                <span>{formatAuditDate(plan.updatedAt, language)}</span>
                                {plan.completedByUserAt ? (
                                  <span>{t("Archived")}: {formatAuditDate(plan.completedByUserAt, language)}</span>
                                ) : null}
                              </div>
                            </div>
                            {action ? (
                              <button
                                aria-label={t(planActionLabel(action))}
                                className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                disabled={planHistoryOperationKey !== null}
                                onClick={() => void runPlanHistoryAction(plan.id, action)}
                                title={t(planActionLabel(action))}
                                type="button"
                              >
                                {planHistoryOperationKey === operationKey ? (
                                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                                ) : (
                                  <CheckCircle2 aria-hidden="true" className="size-4" />
                                )}
                                {t(planActionLabel(action))}
                              </button>
                            ) : null}
                          </div>
                        </article>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {isLoadingPlanHistory ? t("Loading plans...") : t("No plans")}
                    </div>
                  )}
                </div>

                <div className="flex flex-col gap-3 border-t border-stone-200 px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
                  <div className="text-xs font-medium text-stone-500">
                    {planHistoryTotalCount
                      ? t("Showing {start}-{end} of {total}", {
                        end: formatNumber(planHistoryPageEnd, language),
                        start: formatNumber(planHistoryPageStart, language),
                        total: formatNumber(planHistoryTotalCount, language),
                      })
                      : t("No plans")}
                  </div>
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between lg:justify-end">
                    <label className="block w-full sm:w-32">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Page size")}
                      </span>
                      <input
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        inputMode="numeric"
                        onChange={(event) => updatePlanHistoryPageSize(event.target.value)}
                        value={planHistoryPageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Plan history pagination")}
                      className="flex flex-wrap items-center gap-1.5"
                    >
                      <button
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={isLoadingPlanHistory || planHistoryPage <= 1}
                        onClick={() => goToPlanHistoryPage(planHistoryPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </button>
                      {planHistoryPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-stone-400"
                            key={`plan-history-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <button
                            aria-current={item === planHistoryPage ? "page" : undefined}
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === planHistoryPage
                                ? "border-teal-700 bg-teal-700 text-white"
                                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            disabled={isLoadingPlanHistory}
                            key={item}
                            onClick={() => goToPlanHistoryPage(item)}
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
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={
                          isLoadingPlanHistory ||
                          planHistoryTotalPages === 0 ||
                          planHistoryPage >= planHistoryTotalPages
                        }
                        onClick={() => goToPlanHistoryPage(planHistoryPage + 1)}
                        title={t("Next page")}
                        type="button"
                      >
                        <ChevronRight aria-hidden="true" className="size-4" />
                      </button>
                    </nav>
                  </div>
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "memory" ? (
            <section className="grid gap-4">
              {isMemoryDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close memory dialog backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={closeMemoryDialog}
                    type="button"
                  />
                  <form
                    aria-label={
                      memoryDialogMode === "create"
                        ? t("Create memory")
                        : t("Edit memory")
                    }
                    className={`fixed left-1/2 top-1/2 z-50 max-h-[88vh] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)] ${memoryDialogMode === "edit" ? "w-[min(94vw,72rem)]" : "w-[min(92vw,34rem)]"
                      }`}
                    onSubmit={(event) => void saveMemoryDialog(event)}
                    role="dialog"
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          {memoryDialogMode === "create" ? (
                            <Plus aria-hidden="true" className="size-5 text-teal-700" />
                          ) : (
                            <Pencil aria-hidden="true" className="size-5 text-teal-700" />
                          )}
                          <h3 className="text-sm font-semibold text-stone-950">
                            {memoryDialogMode === "create"
                              ? t("Create memory")
                              : t("Edit memory")}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          {memoryScopeLabel(manualMemoryForm.scope, t)}
                        </div>
                      </div>
                      <button
                        aria-label={t("Close memory dialog")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={closeMemoryDialog}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    <div
                      className={
                        memoryDialogMode === "edit"
                          ? "grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]"
                          : "grid gap-3"
                      }
                    >
                      <div className="grid min-w-0 gap-3">
                        {memoryDialogMode === "create" ? (
                          <>
                            <label className="block">
                              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                {t("Memory scope")}
                              </span>
                              <select
                                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                onChange={(event) =>
                                  setManualMemoryForm((current) => ({
                                    ...current,
                                    scope: event.target.value as ManualMemoryFormState["scope"],
                                  }))
                                }
                                value={manualMemoryForm.scope}
                              >
                                <option value="global">{t("Global memory")}</option>
                                <option value="workspace">{t("Workspace memory")}</option>
                                <option value="chat">{t("Chat memory")}</option>
                              </select>
                            </label>
                            {manualMemoryForm.scope !== "global" ? (
                              <label className="block">
                                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                  {t("Workspace")}
                                </span>
                                <select
                                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                  onChange={(event) =>
                                    setManualMemoryForm((current) => ({
                                      ...current,
                                      workspaceId: event.target.value,
                                    }))
                                  }
                                  value={
                                    manualMemoryForm.workspaceId ||
                                    memoryDialogWorkspace?.id ||
                                    ""
                                  }
                                >
                                  {workspaces.map((workspace) => (
                                    <option key={workspace.id} value={workspace.id}>
                                      {workspace.name}
                                    </option>
                                  ))}
                                </select>
                              </label>
                            ) : null}
                            {manualMemoryForm.scope === "chat" ? (
                              <TextField
                                label={t("Chat ID")}
                                onChange={(value) =>
                                  setManualMemoryForm((current) => ({
                                    ...current,
                                    chatId: value,
                                  }))
                                }
                                placeholder="chat-..."
                                value={manualMemoryForm.chatId}
                              />
                            ) : null}
                          </>
                        ) : null}
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Memory kind")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setManualMemoryForm((current) => ({
                                ...current,
                                kind: event.target.value,
                              }))
                            }
                            value={manualMemoryForm.kind}
                          >
                            {MEMORY_KIND_OPTIONS.map((kind) => (
                              <option key={kind} value={kind}>
                                {memoryKindLabel(kind, t)}
                              </option>
                            ))}
                          </select>
                        </label>
                        <div className="grid gap-3 sm:grid-cols-2">
                          <TextField
                            inputMode="numeric"
                            label={t("Confidence")}
                            onChange={(value) =>
                              setManualMemoryForm((current) => ({
                                ...current,
                                confidence: value,
                              }))
                            }
                            placeholder="0.8"
                            value={manualMemoryForm.confidence}
                          />
                          <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2 sm:mt-6">
                            <span className="text-sm font-semibold text-stone-700">
                              {t("Pinned memory")}
                            </span>
                            <input
                              checked={manualMemoryForm.pinned}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                setManualMemoryForm((current) => ({
                                  ...current,
                                  pinned: event.target.checked,
                                }))
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Memory fact")}
                          </span>
                          <textarea
                            className="min-h-32 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setManualMemoryForm((current) => ({
                                ...current,
                                fact: event.target.value,
                              }))
                            }
                            value={manualMemoryForm.fact}
                          />
                        </label>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Memory metadata")}
                          </span>
                          <textarea
                            className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setManualMemoryForm((current) => ({
                                ...current,
                                metadataText: event.target.value,
                              }))
                            }
                            spellCheck={false}
                            value={manualMemoryForm.metadataText}
                          />
                        </label>
                        {memoryDialogMode === "edit" && selectedMemory ? (
                          <div className="grid gap-2 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3 text-xs text-stone-600">
                            <div className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                              {t("Memory details")}
                            </div>
                            <div className="grid gap-2 sm:grid-cols-2">
                              <div className="break-all">
                                <span className="font-semibold text-stone-700">ID: </span>
                                {selectedMemory.id}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Memory status")}:{" "}
                                </span>
                                {memoryStatusLabel(selectedMemory.status, t)}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Memory scope")}:{" "}
                                </span>
                                {memoryScopeLabel(selectedMemory.scope, t)}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Chat ID")}:{" "}
                                </span>
                                {selectedMemory.chatId ?? "-"}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Latest")}:{" "}
                                </span>
                                {selectedMemory.isLatest ? t("Yes") : t("No")}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Expires at")}:{" "}
                                </span>
                                {selectedMemory.expiresAt ?? "-"}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Created")}:{" "}
                                </span>
                                {selectedMemory.createdAt}
                              </div>
                              <div>
                                <span className="font-semibold text-stone-700">
                                  {t("Updated")}:{" "}
                                </span>
                                {selectedMemory.updatedAt}
                              </div>
                            </div>
                          </div>
                        ) : null}
                      </div>
                      {memoryDialogMode === "edit" ? (
                        <div className="grid min-w-0 gap-2 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-2">
                          <div className="flex items-center justify-between gap-2">
                            <h4 className="text-xs font-semibold text-stone-600">
                              {t("Memory source details")}
                            </h4>
                            <span className="font-mono text-[11px] font-semibold text-stone-400">
                              {memorySourceForms.length}
                            </span>
                          </div>
                          {memorySourceForms.length === 0 ? (
                            <div className="rounded-lg border border-dashed border-stone-300 bg-white px-3 py-6 text-center text-sm font-medium text-stone-500">
                              {t("No memory sources")}
                            </div>
                          ) : (
                            <div className="grid max-h-[58vh] gap-3 overflow-y-auto pr-1">
                              {memorySourceForms.map((source, index) => (
                                <div
                                  className="grid gap-3 rounded-xl border border-stone-200 bg-white px-3 py-3"
                                  key={source.id}
                                >
                                  <div className="flex flex-wrap items-center justify-between gap-2">
                                    <div className="min-w-0">
                                      <div className="text-xs font-semibold text-stone-500">
                                        {t("Memory sources")} #{index + 1}
                                      </div>
                                      <div className="mt-1 break-all font-mono text-[11px] text-stone-400">
                                        {source.id}
                                      </div>
                                    </div>
                                  </div>
                                  <TextField
                                    label={t("Source title")}
                                    onChange={(value) =>
                                      updateMemorySourceForm(source.id, "title", value)
                                    }
                                    placeholder={t("Source title")}
                                    value={source.title}
                                  />
                                  <MemorySourceReadonlyDetails
                                    source={memorySources.find((item) => item.id === source.id)}
                                    t={t}
                                  />
                                  <div className="grid gap-1.5">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Source content")}
                                    </span>
                                    <SourceValueEditor
                                      id={`${source.id}:content`}
                                      isExpanded={expandedMemoryJsonIds.has(`${source.id}:content`)}
                                      minHeightClass="min-h-28"
                                      onChange={(value) =>
                                        updateMemorySourceForm(source.id, "content", value)
                                      }
                                      onToggle={toggleMemoryJson}
                                      t={t}
                                      title={t("Source content")}
                                      value={source.content}
                                    />
                                  </div>
                                  <div className="grid gap-1.5">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Source metadata")}
                                    </span>
                                    <SourceValueEditor
                                      id={`${source.id}:metadata`}
                                      isExpanded={expandedMemoryJsonIds.has(`${source.id}:metadata`)}
                                      minHeightClass="min-h-24"
                                      onChange={(value) =>
                                        updateMemorySourceForm(source.id, "metadataText", value)
                                      }
                                      onToggle={toggleMemoryJson}
                                      t={t}
                                      title={t("Source metadata")}
                                      value={source.metadataText}
                                    />
                                  </div>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      ) : null}
                      <button
                        aria-label={
                          memoryDialogMode === "create"
                            ? t("Create memory")
                            : t("Save memory")
                        }
                        className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 xl:col-span-2"
                        disabled={
                          isSavingMemory ||
                          !manualMemoryForm.fact.trim() ||
                          (manualMemoryForm.scope === "chat" &&
                            !manualMemoryForm.chatId.trim())
                        }
                        title={
                          memoryDialogMode === "create"
                            ? t("Create memory")
                            : t("Save memory")
                        }
                        type="submit"
                      >
                        {isSavingMemory ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : memoryDialogMode === "create" ? (
                          <Plus aria-hidden="true" className="size-4" />
                        ) : (
                          <CheckCircle2 aria-hidden="true" className="size-4" />
                        )}
                        {memoryDialogMode === "create"
                          ? t("Create memory")
                          : t("Save memory")}
                      </button>
                    </div>
                  </form>
                </>
              ) : null}

              <form
                className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                onSubmit={(event) => void saveMemorySettings(event)}
              >
                <div className="flex items-center gap-2">
                  <Bot aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Memory controls")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("General memory control")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <p className="text-sm font-semibold text-stone-800">
                          {t("Enable memory")}
                        </p>
                        <p className="mt-1 text-xs text-stone-500">
                          {t(
                            "Controls whether memory tools, retrieval, and extraction are available.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable memory")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                      >
                        <input
                          checked={memorySettingsForm.enabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setMemorySettingsForm((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>

                  <div className="grid gap-3 xl:grid-cols-2">
                    <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                      <legend className="px-1 text-xs font-semibold text-stone-600">
                        {t("Memory extraction")}
                      </legend>
                      <div className="mb-3 flex items-start gap-2">
                        <SlidersHorizontal
                          aria-hidden="true"
                          className="mt-0.5 size-4 shrink-0 text-teal-700"
                        />
                        <p className="text-xs text-stone-500">
                          {t(
                            "Controls how new facts are extracted and how long they are retained.",
                          )}
                        </p>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Extraction mode")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                extractionMode: event.target.value,
                              }))
                            }
                            value={memorySettingsForm.extractionMode}
                          >
                            {(settings?.memory.extractionModes ?? []).map((mode) => (
                              <option key={mode.value} value={mode.value}>
                                {t(mode.label)}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Extraction model")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                extractionModelId: event.target.value,
                              }))
                            }
                            value={memorySettingsForm.extractionModelId}
                          >
                            <option value="">{t("Current chat model")}</option>
                            {configuredModelsByName.map((model) => (
                              <option key={model.id} value={model.id}>
                                {model.displayName}
                              </option>
                            ))}
                          </select>
                        </label>
                        <div className="sm:col-span-2">
                          <TextField
                            inputMode="numeric"
                            label={t("Retention days")}
                            onChange={(value) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                retentionDays: value,
                              }))
                            }
                            placeholder="90"
                            value={memorySettingsForm.retentionDays}
                          />
                        </div>
                      </div>
                    </fieldset>

                    <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                      <legend className="px-1 text-xs font-semibold text-stone-600">
                        {t("Memory retrieval")}
                      </legend>
                      <div className="mb-3 flex items-start gap-2">
                        <Brain
                          aria-hidden="true"
                          className="mt-0.5 size-4 shrink-0 text-teal-700"
                        />
                        <p className="text-xs text-stone-500">
                          {t("Controls how existing memory is matched into chat context.")}
                        </p>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Memory matching")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                retrievalMode: event.target.value,
                              }))
                            }
                            value={memorySettingsForm.retrievalMode}
                          >
                            {(settings?.memory.retrievalModes ?? []).map((mode) => (
                              <option key={mode.value} value={mode.value}>
                                {t(mode.label)}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Matching model")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                retrievalModelId: event.target.value,
                              }))
                            }
                            value={memorySettingsForm.retrievalModelId}
                          >
                            <option value="">{t("Current chat model")}</option>
                            {configuredModelsByName.map((model) => (
                              <option key={model.id} value={model.id}>
                                {model.displayName}
                              </option>
                            ))}
                          </select>
                        </label>
                      </div>
                    </fieldset>
                  </div>

                  <fieldset className="rounded-xl border border-stone-200 bg-white/75 px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-stone-600">
                      {t("Dream")}
                    </legend>
                    <div className="mb-3 flex items-start gap-2">
                      <Sparkles
                        aria-hidden="true"
                        className="mt-0.5 size-4 shrink-0 text-teal-700"
                      />
                      <p className="text-xs text-stone-500">
                        {t(
                          "Consolidates stale, duplicate, and pending memories without creating scheduled task rows.",
                        )}
                      </p>
                    </div>
                    <div className="grid gap-3 md:grid-cols-3">
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Enable Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.enabled}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  enabled: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Enable Auto Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Auto Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.autoEnabled}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  autoEnabled: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-800">
                            {t("Create transcript chat")}
                          </span>
                          <label
                            aria-label={t("Create transcript chat")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white"
                          >
                            <input
                              checked={memorySettingsForm.dream.createTranscriptChat}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                updateMemoryDreamForm({
                                  createTranscriptChat: event.target.checked,
                                })
                              }
                              type="checkbox"
                            />
                          </label>
                        </div>
                      </div>
                    </div>
                    <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Dream mode")}
                        </span>
                        <select
                          aria-label={t("Dream mode")}
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            updateMemoryDreamForm({
                              mode: event.target.value as MemoryDreamRunMode,
                            })
                          }
                          value={memorySettingsForm.dream.mode}
                        >
                          <option value="deterministic_only">
                            {t("Deterministic only")}
                          </option>
                          <option value="llm">{t("LLM")}</option>
                        </select>
                      </label>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Dream model")}
                        </span>
                        <select
                          aria-label={t("Dream model")}
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            updateMemoryDreamForm({
                              modelId: event.target.value,
                            })
                          }
                          value={memorySettingsForm.dream.modelId}
                        >
                          <option value="">{t("Fallback model")}</option>
                          {configuredModelsByName.map((model) => (
                            <option key={model.id} value={model.id}>
                              {model.displayName}
                            </option>
                          ))}
                        </select>
                      </label>
                      <TextField
                        inputMode="numeric"
                        label={t("Workspace interval days")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            workspaceIntervalDays: value,
                          })
                        }
                        placeholder="7"
                        value={memorySettingsForm.dream.workspaceIntervalDays}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Global interval days")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            globalIntervalDays: value,
                          })
                        }
                        placeholder="30"
                        value={memorySettingsForm.dream.globalIntervalDays}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Max facts per run")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            maxFactsPerRun: value,
                          })
                        }
                        placeholder="200"
                        value={memorySettingsForm.dream.maxFactsPerRun}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Max changes per run")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            maxChangesPerRun: value,
                          })
                        }
                        placeholder="50"
                        value={memorySettingsForm.dream.maxChangesPerRun}
                      />
                      <TextField
                        inputMode="numeric"
                        label={t("Scheduler scan minutes")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            schedulerScanMinutes: value,
                          })
                        }
                        placeholder="60"
                        value={memorySettingsForm.dream.schedulerScanMinutes}
                      />
                    </div>
                  </fieldset>
                </div>
                <button
                  aria-label={t("Save memory settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                  disabled={isSavingMemorySettings}
                  title={t("Save memory settings")}
                  type="submit"
                >
                  {isSavingMemorySettings ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                  {t("Save")}
                </button>
              </form>

              <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Sparkles aria-hidden="true" className="size-5 text-teal-700" />
                      <h3 className="text-sm font-semibold text-stone-950">
                        {t("Dream history")}
                      </h3>
                    </div>
                    <p className="mt-1 truncate text-xs text-stone-500">
                      {t("Memory maintenance jobs and applied changes")}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    <button
                      aria-label={t("Run workspace Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300"
                      disabled={
                        !isMemoryDreamRunnable ||
                        !memoryDreamWorkspaceId ||
                        activeMemoryDreamJobKeys.has(workspaceDreamRunKey) ||
                        memoryDreamRunKey === workspaceDreamRunKey
                      }
                      onClick={() => void runMemoryDream("workspace")}
                      title={t("Run workspace Dream now")}
                      type="button"
                    >
                      {memoryDreamRunKey === workspaceDreamRunKey ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <Play aria-hidden="true" className="size-4" />
                      )}
                      {t("Run workspace Dream now")}
                    </button>
                    <button
                      aria-label={t("Run global Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={
                        !isMemoryDreamRunnable ||
                        activeMemoryDreamJobKeys.has(globalDreamRunKey) ||
                        memoryDreamRunKey === globalDreamRunKey
                      }
                      onClick={() => void runMemoryDream("global")}
                      title={t("Run global Dream now")}
                      type="button"
                    >
                      {memoryDreamRunKey === globalDreamRunKey ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <Globe aria-hidden="true" className="size-4" />
                      )}
                      {t("Run global Dream now")}
                    </button>
                    <button
                      aria-label={t("Refresh Dream history")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={() => void loadMemoryDreamJobs()}
                      title={t("Refresh Dream history")}
                      type="button"
                    >
                      {isLoadingMemoryDreamJobs ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>

                <div className="mt-4 grid gap-3 md:grid-cols-4">
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Last successful run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {latestSuccessfulMemoryDreamJob
                        ? formatAuditDate(
                          latestSuccessfulMemoryDreamJob.completedAt ??
                          latestSuccessfulMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Last failed run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {latestFailedMemoryDreamJob
                        ? formatAuditDate(
                          latestFailedMemoryDreamJob.completedAt ??
                          latestFailedMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Next automatic run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {memoryDreamNextRunEstimate}
                    </div>
                  </div>
                  <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="text-xs font-semibold text-stone-500">
                      {t("Latest applied changes")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-stone-900">
                      {formatNumber(latestMemoryDreamChangeCount, language)}
                    </div>
                  </div>
                </div>

                {memoryDreamError ? (
                  <div className="mt-4 rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                    {memoryDreamError}
                  </div>
                ) : null}

                <div
                  className="panel-scroll mt-4 overflow-x-auto rounded-xl border border-stone-200 bg-white"
                  onWheel={handleMemoryDreamHistoryWheel}
                >
                  <table className="min-w-full divide-y divide-stone-200 text-left text-sm">
                    <thead className="bg-stone-50 text-xs font-semibold uppercase tracking-wide text-stone-500">
                      <tr>
                        <th className="px-3 py-2">{t("Created")}</th>
                        <th className="px-3 py-2">{t("Scope")}</th>
                        <th className="px-3 py-2">{t("Trigger")}</th>
                        <th className="px-3 py-2">{t("Model")}</th>
                        <th className="px-3 py-2">{t("Status")}</th>
                        <th className="px-3 py-2">{t("Changes")}</th>
                        <th className="px-3 py-2 text-right">{t("Actions")}</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-stone-100">
                      {sortedMemoryDreamJobs.length === 0 ? (
                        <tr>
                          <td
                            className="px-3 py-6 text-center text-sm font-medium text-stone-500"
                            colSpan={7}
                          >
                            {isLoadingMemoryDreamJobs
                              ? t("Loading Dream history...")
                              : t("No Dream jobs")}
                          </td>
                        </tr>
                      ) : (
                        paginatedMemoryDreamJobs.map((job) => {
                          const transcriptWorkspaceId =
                            job.transcriptWorkspaceId ?? job.workspaceId;
                          const scopeLabel =
                            job.scope === "workspace"
                              ? workspaces.find((workspace) => workspace.id === job.workspaceId)
                                  ?.name ??
                                job.workspaceId ??
                                memoryDreamScopeLabel(job.scope, t)
                              : memoryDreamScopeLabel(job.scope, t);
                          return (
                            <tr
                              className="bg-white hover:bg-stone-50"
                              key={job.id}
                            >
                              <td className="px-3 py-2 align-top">
                                <span className="text-xs font-semibold text-stone-700">
                                  {formatAuditDate(job.createdAt, language)}
                                </span>
                              </td>
                              <td className="px-3 py-2 align-top">
                                {scopeLabel}
                              </td>
                              <td className="px-3 py-2 align-top">
                                {memoryDreamTriggerLabel(job.triggerType, t)}
                              </td>
                              <td className="px-3 py-2 align-top">
                                {job.modelId ?? t("Default")}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <CapabilityPill
                                  label={memoryDreamStatusLabel(job.status, t)}
                                  ok={job.status === "completed"}
                                  tone={memoryDreamStatusTone(job.status)}
                                />
                              </td>
                              <td className="px-3 py-2 align-top text-xs text-stone-600">
                                {t("added {count}", {
                                  count: job.changeCounts.added,
                                })}
                                {", "}
                                {t("updated {count}", {
                                  count: job.changeCounts.updated,
                                })}
                                {", "}
                                {t("superseded {count}", {
                                  count: job.changeCounts.superseded,
                                })}
                                {", "}
                                {t("expired {count}", {
                                  count: job.changeCounts.expired,
                                })}
                                {", "}
                                {t("rejected {count}", {
                                  count: job.changeCounts.rejected,
                                })}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <div className="flex items-center justify-end gap-1">
                                  <button
                                    aria-label={t("View details")}
                                    className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      setMemoryDreamDetailJobId(job.id);
                                    }}
                                    title={t("View details")}
                                    type="button"
                                  >
                                    <Eye aria-hidden="true" className="size-4" />
                                  </button>
                                  {job.transcriptChatId && transcriptWorkspaceId ? (
                                    <button
                                      aria-label={t("Open transcript")}
                                      className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                      onClick={(event) => {
                                        event.stopPropagation();
                                        onOpenChat(transcriptWorkspaceId, job.transcriptChatId!);
                                      }}
                                      title={t("Open transcript")}
                                      type="button"
                                    >
                                      <ScrollText aria-hidden="true" className="size-4" />
                                    </button>
                                  ) : job.transcriptChatId ? (
                                    <span className="text-xs text-stone-500">
                                      {job.transcriptChatId}
                                    </span>
                                  ) : null}
                                </div>
                              </td>
                            </tr>
                          );
                        })
                      )}
                    </tbody>
                  </table>
                </div>

                <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 pt-3 text-sm">
                  <div className="text-stone-500">
                    {t("Showing {start}-{end} of {total}", {
                      end: formatNumber(memoryDreamPageEnd, language),
                      start: formatNumber(memoryDreamPageStart, language),
                      total: formatNumber(sortedMemoryDreamJobs.length, language),
                    })}
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                      <span>{t("Page size")}</span>
                      <input
                        className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        max={MEMORY_DREAM_MAX_PAGE_SIZE}
                        min={1}
                        onChange={(event) => updateMemoryDreamPageSize(event.target.value)}
                        type="number"
                        value={memoryDreamPageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Dream history pagination")}
                      className="flex items-center gap-1"
                    >
                      <button
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={isLoadingMemoryDreamJobs || currentMemoryDreamPage <= 1}
                        onClick={() => goToMemoryDreamPage(currentMemoryDreamPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </button>
                      {memoryDreamPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-stone-400"
                            key={`memory-dream-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <button
                            aria-current={
                              item === currentMemoryDreamPage ? "page" : undefined
                            }
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === currentMemoryDreamPage
                                ? "border-teal-700 bg-teal-700 text-white"
                                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            disabled={isLoadingMemoryDreamJobs}
                            key={item}
                            onClick={() => goToMemoryDreamPage(item)}
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
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={
                          isLoadingMemoryDreamJobs ||
                          memoryDreamTotalPages === 0 ||
                          currentMemoryDreamPage >= memoryDreamTotalPages
                        }
                        onClick={() => goToMemoryDreamPage(currentMemoryDreamPage + 1)}
                        title={t("Next page")}
                        type="button"
                      >
                        <ChevronRight aria-hidden="true" className="size-4" />
                      </button>
                    </nav>
                  </div>
                </div>

                {memoryDreamDetailJob ? (
                  <>
                    <button
                      aria-label={t("Close Dream job details backdrop")}
                      className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                      onClick={closeMemoryDreamDetailDialog}
                      type="button"
                    />
                    <div
                      aria-labelledby="memory-dream-detail-title"
                      aria-modal="true"
                      className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(94vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                      role="dialog"
                    >
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <Sparkles aria-hidden="true" className="size-5 text-teal-700" />
                            <h4
                              className="text-sm font-semibold text-stone-950"
                              id="memory-dream-detail-title"
                            >
                              {t("Dream job details")}
                            </h4>
                            <CapabilityPill
                              label={memoryDreamStatusLabel(memoryDreamDetailJob.status, t)}
                              ok={memoryDreamDetailJob.status === "completed"}
                              tone={memoryDreamStatusTone(memoryDreamDetailJob.status)}
                            />
                            <CapabilityPill
                              label={memoryDreamScopeLabel(memoryDreamDetailJob.scope, t)}
                              ok={memoryDreamDetailJob.scope === "workspace"}
                            />
                          </div>
                          <div className="mt-1 text-xs text-stone-500">
                            {formatAuditDate(memoryDreamDetailJob.createdAt, language)}
                          </div>
                        </div>
                        <button
                          aria-label={t("Close Dream job details")}
                          className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                          onClick={closeMemoryDreamDetailDialog}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                      <p className="text-sm text-stone-600">
                        {memoryDreamDetailJob.summary ||
                          memoryDreamDetailJob.errorMessage ||
                          t("No summary")}
                      </p>
                      <div className="mt-3 grid gap-3">
                        {isLoadingMemoryDreamChanges ? (
                          <div className="text-sm text-stone-500">
                            {t("Loading Dream changes...")}
                          </div>
                        ) : memoryDreamChangesByOperation.length === 0 ? (
                          <div className="text-sm text-stone-500">
                            {t("No Dream changes")}
                          </div>
                        ) : (
                          memoryDreamChangesByOperation.map((group) => (
                            <div
                              className="rounded-lg border border-stone-200 bg-white px-3 py-3"
                              key={group.operation}
                            >
                              <div className="mb-3 flex flex-wrap items-center gap-2">
                                <h5 className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                                  {memoryDreamChangeOperationLabel(group.operation, t)}
                                </h5>
                                <CapabilityPill
                                  label={formatNumber(group.changes.length, language)}
                                  ok
                                />
                              </div>
                              <div className="grid gap-3">
                                {group.changes.map((change) => (
                                  <div
                                    className="rounded-lg border border-stone-100 bg-stone-50/70 px-3 py-3"
                                    key={change.id}
                                  >
                                    <div className="flex flex-wrap items-center gap-2">
                                      <CapabilityPill
                                        label={memoryDreamChangeStatusLabel(change.status, t)}
                                        ok={change.status === "applied"}
                                      />
                                      <CapabilityPill
                                        label={memoryDreamRiskLabel(change.riskLevel, t)}
                                        ok={change.riskLevel === "low"}
                                      />
                                      {change.confidence !== null ? (
                                        <span className="text-xs font-semibold text-stone-500">
                                          {formatNumber(
                                            Math.round(change.confidence * 100),
                                            language,
                                          )}
                                          %
                                        </span>
                                      ) : null}
                                    </div>
                                    <div className="mt-2 text-sm font-semibold text-stone-900">
                                      {change.reason}
                                    </div>
                                    {change.targetFactIds.length ? (
                                      <div className="mt-1 break-all text-xs text-stone-500">
                                        {change.targetFactIds.join(", ")}
                                      </div>
                                    ) : null}
                                    {change.errorMessage ? (
                                      <div className="mt-2 text-sm text-rose-700">
                                        {change.errorMessage}
                                      </div>
                                    ) : null}
                                    <div className="mt-3 grid gap-3 lg:grid-cols-3">
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("Before JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Memory state before this Dream change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.beforeJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("After JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Memory state Dream wrote or proposed.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.afterJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-stone-700">
                                          {t("Evidence JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-stone-500">
                                          {t("Sources Dream used to justify the change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700">
                                          {memoryDreamJsonText(change.evidence)}
                                        </pre>
                                      </div>
                                    </div>
                                  </div>
                                ))}
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </>
                ) : null}
              </section>

              <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("Memory list")}
                    </h3>
                    <p className="mt-1 truncate text-xs text-stone-500">
                      {memoryScopeLabel(memoryFilter.scope, t)}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    {memoryFilter.scope !== "global" ? (
                      <button
                        aria-label={clearFilteredMemoryLabel}
                        className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-sm font-semibold text-rose-700 hover:bg-rose-50 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={!canClearFilteredMemories || isSavingMemory}
                        onClick={() => void clearFilteredMemories()}
                        title={clearFilteredMemoryLabel}
                        type="button"
                      >
                        <Trash2 aria-hidden="true" className="size-4" />
                        {clearFilteredMemoryLabel}
                      </button>
                    ) : null}
                    <button
                      aria-label={t("Create memory")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-3 text-sm font-semibold text-white hover:bg-teal-900"
                      onClick={openCreateMemoryDialog}
                      title={t("Create memory")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                      {t("Create memory")}
                    </button>
                  </div>
                </div>
                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(9rem,0.8fr)_minmax(9rem,0.8fr)_minmax(8rem,0.7fr)_minmax(0,1.4fr)_auto]">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory scope")}
                    </span>
                    <select
                      aria-label={t("Memory scope")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateMemoryFilter({
                          scope: event.target.value as MemoryFilterState["scope"],
                          workspaceId:
                            event.target.value === "global"
                              ? ""
                              : memoryFilter.workspaceId || memoryWorkspace?.id || "",
                        })
                      }
                      value={memoryFilter.scope}
                    >
                      <option value="global">{t("Global memory")}</option>
                      <option value="workspace">{t("Workspace memory")}</option>
                      <option value="chat">{t("Chat memory")}</option>
                    </select>
                  </label>
                  {memoryFilter.scope !== "global" ? (
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Workspace")}
                      </span>
                      <select
                        aria-label={t("Workspace")}
                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          updateMemoryFilter({
                            workspaceId: event.target.value,
                          })
                        }
                        value={memoryFilter.workspaceId || memoryWorkspace?.id || ""}
                      >
                        {workspaces.map((workspace) => (
                          <option key={workspace.id} value={workspace.id}>
                            {workspace.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  ) : null}
                  {memoryFilter.scope === "chat" ? (
                    <TextField
                      label={t("Chat ID")}
                      onChange={(value) =>
                        updateMemoryFilter({
                          chatId: value,
                        })
                      }
                      placeholder="chat-..."
                      value={memoryFilter.chatId}
                    />
                  ) : null}
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      {t("Memory kind")}
                    </span>
                    <select
                      aria-label={t("Memory kind")}
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        updateMemoryFilter({
                          kind: event.target.value,
                        })
                      }
                      value={memoryFilter.kind}
                    >
                      <option value="">{t("All memory kinds")}</option>
                      {MEMORY_KIND_OPTIONS.map((kind) => (
                        <option key={kind} value={kind}>
                          {memoryKindLabel(kind, t)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <TextField
                    label={t("Search memories")}
                    onChange={(value) =>
                      updateMemoryFilter({
                        query: value,
                      })
                    }
                    placeholder={t("Search memories")}
                    value={memoryFilter.query}
                  />
                  <div className="flex items-end gap-2">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Memory status")}
                      </span>
                      <select
                        aria-label={t("Memory status")}
                        className="h-10 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) =>
                          updateMemoryFilter({
                            status: event.target.value as MemoryFilterState["status"],
                          })
                        }
                        value={memoryFilter.status}
                      >
                        <option value="active">{t("Active")}</option>
                        <option value="pending">{t("Pending review")}</option>
                      </select>
                    </label>
                    <button
                      aria-label={t("Refresh memories")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={() => void loadMemories()}
                      title={t("Refresh memories")}
                      type="button"
                    >
                      {isLoadingMemories ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
                <div className="mt-4 grid gap-3">
                  {memories.length === 0 ? (
                    <div className="rounded-xl border border-dashed border-stone-300 bg-stone-50 px-3 py-6 text-center text-sm font-medium text-stone-500">
                      {t("No memories")}
                    </div>
                  ) : (
                    memories.map((memory) => (
                      <div
                        className={`grid gap-3 rounded-xl border px-3 py-3 sm:grid-cols-[minmax(0,1fr)_auto] ${selectedMemoryId === memory.id
                            ? "border-teal-200 bg-teal-50/80"
                            : "border-stone-200 bg-white hover:border-teal-100 hover:bg-stone-50"
                          }`}
                        key={memory.id}
                      >
                        <button
                          className="min-w-0 text-left"
                          onClick={() => setSelectedMemoryId(memory.id)}
                          type="button"
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <CapabilityPill
                              label={memoryStatusLabel(memory.status, t)}
                              ok={memory.status === "active"}
                            />
                            <CapabilityPill
                              label={memoryKindLabel(memory.kind, t)}
                              ok={memory.pinned}
                            />
                            {memory.scope === "chat" && memory.chatId ? (
                              <span className="text-xs font-semibold text-stone-500">
                                {memory.chatId}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-2 break-words text-sm font-semibold text-stone-900">
                            {memory.fact}
                          </div>
                          <div className="mt-2 text-xs text-stone-500">
                            {memory.updatedAt}
                          </div>
                        </button>
                        <div className="flex items-start justify-end gap-2">
                          {memory.scope !== "global" ? (
                            <button
                              aria-label={t("Promote one level")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              onClick={() => promoteMemoryOneLevel(memory)}
                              title={
                                memory.scope === "chat"
                                  ? t("Promote to workspace")
                                  : t("Promote to global")
                              }
                              type="button"
                            >
                              <ArrowUp aria-hidden="true" className="size-4" />
                            </button>
                          ) : null}
                          <button
                            aria-label={t("Edit memory")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={() => openEditMemoryDialog(memory)}
                            title={t("Edit memory")}
                            type="button"
                          >
                            <Pencil aria-hidden="true" className="size-4" />
                          </button>
                          <button
                            aria-label={t("Delete memory")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                            onClick={() => void forgetMemory(memory.id)}
                            title={t("Delete memory")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </button>
                        </div>
                      </div>
                    ))
                  )}
                </div>
                <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 pt-3 text-sm">
                  <div className="text-stone-500">
                    {t("Showing {start}-{end} of {total}", {
                      end: formatNumber(memoryPageEnd, language),
                      start: formatNumber(memoryPageStart, language),
                      total: formatNumber(memoryListMeta.totalCount, language),
                    })}
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                      <span>{t("Page size")}</span>
                      <input
                        className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        max={200}
                        min={1}
                        onChange={(event) => updateMemoryPageSize(event.target.value)}
                        type="number"
                        value={memoryFilter.pageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Memory pagination")}
                      className="flex items-center gap-1"
                    >
                      <button
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={
                          !isMemoryFilterReady ||
                          isLoadingMemories ||
                          memoryListMeta.page <= 1
                        }
                        onClick={() => goToMemoryPage(memoryListMeta.page - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </button>
                      {memoryPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-stone-400"
                            key={`memory-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <button
                            aria-current={
                              item === memoryListMeta.page ? "page" : undefined
                            }
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === memoryListMeta.page
                                ? "border-teal-700 bg-teal-700 text-white"
                                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            disabled={!isMemoryFilterReady || isLoadingMemories}
                            key={item}
                            onClick={() => goToMemoryPage(item)}
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
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                        disabled={
                          isLoadingMemories ||
                          !isMemoryFilterReady ||
                          memoryListMeta.totalPages === 0 ||
                          memoryListMeta.page >= memoryListMeta.totalPages
                        }
                        onClick={() => goToMemoryPage(memoryListMeta.page + 1)}
                        title={t("Next page")}
                        type="button"
                      >
                        <ChevronRight aria-hidden="true" className="size-4" />
                      </button>
                    </nav>
                  </div>
                </div>
                <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                  <div className="flex items-center gap-2">
                    <CircleAlert aria-hidden="true" className="size-4 text-rose-700" />
                    <h4 className="text-xs font-semibold uppercase tracking-wide text-stone-500">
                      {t("Extraction failures")}
                    </h4>
                  </div>
                  <div className="mt-2 grid gap-2">
                    {memoryExtractionJobs.length === 0 ? (
                      <div className="text-sm text-stone-500">
                        {t("No extraction failures")}
                      </div>
                    ) : (
                      memoryExtractionJobs.map((job) => (
                        <div
                          className="rounded-lg border border-rose-100 bg-white px-3 py-2"
                          key={job.id}
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <CapabilityPill label={job.status} ok={false} />
                            <CapabilityPill
                              label={job.modelId ?? t("Default")}
                              ok={false}
                            />
                            {job.chatId ? (
                              <span className="text-xs font-semibold text-stone-500">
                                {job.chatId}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-2 flex flex-wrap items-start justify-between gap-2">
                            <div className="min-w-0 flex-1 text-sm font-semibold text-rose-700">
                              {job.errorMessage ?? t("Memory extraction failed")}
                            </div>
                            <div className="flex shrink-0 items-center gap-2">
                              <button
                                aria-label={t("Retry extraction")}
                                className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                disabled={isSavingMemory}
                                onClick={() =>
                                  void updateMemoryExtractionJob(job.id, "retry")
                                }
                                title={t("Retry extraction")}
                                type="button"
                              >
                                <Redo2 aria-hidden="true" className="size-3.5" />
                              </button>
                              <button
                                aria-label={t("Skip extraction failure")}
                                className="inline-flex size-8 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                disabled={isSavingMemory}
                                onClick={() =>
                                  void updateMemoryExtractionJob(job.id, "skip")
                                }
                                title={t("Skip extraction failure")}
                                type="button"
                              >
                                <X aria-hidden="true" className="size-3.5" />
                              </button>
                            </div>
                          </div>
                          <div className="mt-1 text-xs text-stone-500">
                            {job.completedAt ?? job.startedAt ?? job.createdAt}
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </div>
                {selectedMemory?.status === "pending" ? (
                  <div className="mt-4 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                    <div className="flex flex-wrap gap-2">
                      <button
                        className="inline-flex h-9 items-center gap-2 rounded-lg bg-teal-800 px-3 text-xs font-semibold text-white hover:bg-teal-900"
                        onClick={() => void setMemoryStatus(selectedMemory.id, "active")}
                        type="button"
                      >
                        <CheckCircle2 aria-hidden="true" className="size-3.5" />
                        {t("Approve memory")}
                      </button>
                      <button
                        className="inline-flex h-9 items-center gap-2 rounded-lg border border-rose-200 bg-white px-3 text-xs font-semibold text-rose-700 hover:bg-rose-50"
                        onClick={() => void setMemoryStatus(selectedMemory.id, "rejected")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-3.5" />
                        {t("Reject memory")}
                      </button>
                    </div>
                  </div>
                ) : null}
              </section>
            </section>
          ) : null}

          {activeSection === "workspaces" ? (
            <section className="grid gap-4">
              {isWorkspaceDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close workspace configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsWorkspaceDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Workspace configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    onSubmit={(event) => void saveWorkspace(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Folder aria-hidden="true" className="size-5 text-teal-700" />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {t("Edit workspace")}
                          </h3>
                        </div>
                        {editingWorkspace ? (
                          <div className="mt-1 truncate text-xs text-stone-500">
                            {editingWorkspace.path}
                          </div>
                        ) : null}
                      </div>
                      <button
                        aria-label={t("Close workspace configuration")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={() => setIsWorkspaceDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    <div className="space-y-3">
                      <TextField
                        label={t("Workspace name")}
                        onChange={(value) =>
                          setWorkspaceForm((current) => ({
                            ...current,
                            name: value,
                          }))
                        }
                        placeholder={t("Workspace name")}
                        value={workspaceForm.name}
                      />
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Path")}
                        </span>
                        <div className="flex gap-2">
                          <input
                            autoComplete="off"
                            className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            name="workspace-path"
                            onChange={(event) =>
                              setWorkspaceForm((current) => ({
                                ...current,
                                path: event.target.value,
                              }))
                            }
                            placeholder="C:/Users/name/workspace"
                            value={workspaceForm.path}
                          />
                          <button
                            aria-label={t("Choose workspace path")}
                            className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                            disabled={isSelectingWorkspaceFormPath || !canUseNativePicker}
                            onClick={() => void selectWorkspaceFormPath()}
                            title={
                              canUseNativePicker
                                ? t("Choose workspace path")
                                : t("Local Foco browser required")
                            }
                            type="button"
                          >
                            {isSelectingWorkspaceFormPath ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="size-4 animate-spin"
                              />
                            ) : (
                              <FolderSearch aria-hidden="true" className="size-4" />
                            )}
                          </button>
                        </div>
                      </label>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
                        <div className="mb-3 flex items-center justify-between gap-3">
                          <div className="flex min-w-0 items-center gap-2">
                            <WorkspaceIcon
                              className="size-10 rounded-lg border border-stone-200 bg-white object-cover p-1"
                              fallbackClassName="size-10 rounded-lg border border-stone-200 bg-white p-2 text-teal-700"
                              logoUrl={editingWorkspace?.logoUrl ?? null}
                            />
                            <div className="min-w-0">
                              <span className="block text-sm font-semibold text-stone-800">
                                {t("Workspace icon")}
                              </span>
                              <span className="block truncate text-xs text-stone-500">
                                {editingWorkspace?.logoUrl
                                  ? t("Custom icon")
                                  : t("Folder icon")}
                              </span>
                            </div>
                          </div>
                          <button
                            aria-label={t("Clear workspace icon")}
                            className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-600 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700 disabled:cursor-not-allowed disabled:text-stone-300"
                            disabled={isSavingWorkspaceLogo || !editingWorkspace?.logoUrl}
                            onClick={() => void clearWorkspaceLogo()}
                            title={t("Clear workspace icon")}
                            type="button"
                          >
                            {isSavingWorkspaceLogo ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="size-4 animate-spin"
                              />
                            ) : (
                              <Trash2 aria-hidden="true" className="size-4" />
                            )}
                          </button>
                        </div>
                        <input
                          aria-label={t("Workspace icon file")}
                          accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                          className="sr-only"
                          onChange={handleWorkspaceLogoFileChange}
                          ref={workspaceLogoInputRef}
                          type="file"
                        />
                        <button
                          aria-label={t("Upload icon")}
                          className="mt-2 inline-flex h-9 items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                          disabled={isSavingWorkspaceLogo}
                          onClick={() => workspaceLogoInputRef.current?.click()}
                          title={t("Upload icon")}
                          type="button"
                        >
                          <Upload aria-hidden="true" className="size-3.5" />
                          {t("Upload icon")}
                        </button>
                      </div>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Terminal shell")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setWorkspaceForm((current) => ({
                              ...current,
                              terminalShell: event.target.value,
                            }))
                          }
                          value={workspaceForm.terminalShell || terminalShells[0]?.shell || ""}
                        >
                          {terminalShells.map((shell) => (
                            <option key={shell.shell} value={shell.shell}>
                              {shell.label}
                            </option>
                          ))}
                        </select>
                      </label>
                      <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
                        <div className="mb-3 flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-stone-700">
                            {t("Common commands")}
                          </span>
                          <button
                            aria-label={t("Add command")}
                            className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={addWorkspaceCommonCommand}
                            title={t("Add command")}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-4" />
                          </button>
                        </div>
                        {workspaceForm.commonCommands.length ? (
                          <div className="space-y-2">
                            <div className="grid gap-2 pr-10 text-xs font-semibold text-stone-500 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)]">
                              <span>{t("Command name")}</span>
                              <span>{t("Command")}</span>
                            </div>
                            {workspaceForm.commonCommands.map((command, index) => (
                              <div
                                className="grid items-center gap-2 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)_2.25rem]"
                                key={index}
                              >
                                <input
                                  aria-label={t("Command name")}
                                  autoComplete="off"
                                  className="h-9 min-w-0 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                  onChange={(event) =>
                                    updateWorkspaceCommonCommand(
                                      index,
                                      "name",
                                      event.target.value,
                                    )
                                  }
                                  placeholder={t("Command name")}
                                  value={command.name}
                                />
                                <input
                                  aria-label={t("Command")}
                                  autoComplete="off"
                                  className="h-9 min-w-0 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                  onChange={(event) =>
                                    updateWorkspaceCommonCommand(
                                      index,
                                      "command",
                                      event.target.value,
                                    )
                                  }
                                  placeholder="npm run dev"
                                  value={command.command}
                                />
                                <button
                                  aria-label={t("Remove command {name}", {
                                    name: command.name || String(index + 1),
                                  })}
                                  className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-500 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                                  onClick={() => removeWorkspaceCommonCommand(index)}
                                  title={t("Remove command")}
                                  type="button"
                                >
                                  <Trash2 aria-hidden="true" className="size-4" />
                                </button>
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </div>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                        <span className="text-sm font-semibold text-stone-700">
                          {t("Pinned workspace")}
                        </span>
                        <input
                          checked={workspaceForm.pinned}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setWorkspaceForm((current) => ({
                              ...current,
                              pinned: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                        <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-700">
                          <ScrollText
                            aria-hidden="true"
                            className="size-4 shrink-0 text-teal-700"
                          />
                          <span className="truncate">{t("Enable Project Spec")}</span>
                        </span>
                        <input
                          checked={workspaceForm.specEnabled}
                          className="size-4 accent-teal-700"
                          disabled={
                            isLoadingWorkspaceSpecSettings ||
                            !isWorkspaceSpecSettingsLoaded
                          }
                          onChange={(event) =>
                            setWorkspaceForm((current) => ({
                              ...current,
                              specEnabled: event.target.checked,
                              specInjectEnabled: event.target.checked
                                ? current.specInjectEnabled
                                : false,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                      <button
                        aria-label={t("Save workspace")}
                        className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                        disabled={
                          isSavingWorkspace ||
                          isLoadingWorkspaceSpecSettings ||
                          !workspaceForm.name.trim() ||
                          !workspaceForm.path.trim()
                        }
                        title={t("Save workspace")}
                        type="submit"
                      >
                        {isSavingWorkspace ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <CheckCircle2 aria-hidden="true" className="size-4" />
                        )}
                        {t("Save")}
                      </button>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Workspace list")}
                  </h3>
                  <button
                    aria-label={t("Add workspace")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    onClick={onAddWorkspace}
                    title={t("Add workspace")}
                    type="button"
                  >
                    <Plus aria-hidden="true" className="size-4" />
                  </button>
                </div>
                <div className="divide-y divide-stone-100">
                  {orderedWorkspaces.length ? (
                    orderedWorkspaces.map((workspace) => (
                      <div
                        className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${draggedWorkspaceId === workspace.id
                            ? "bg-teal-50/70 opacity-80"
                            : "bg-white/0"
                          }`}
                        key={workspace.id}
                        onDragOver={(event) =>
                          handleWorkspaceDragOver(event, workspace.id)
                        }
                        onDrop={(event) => void handleWorkspaceDrop(event)}
                      >
                        <div className="flex items-center">
                          <span
                            aria-label={t("Reorder workspace {name}", {
                              name: workspace.name,
                            })}
                            className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${isSavingWorkspaceOrder
                                ? "cursor-not-allowed opacity-60"
                                : "cursor-grab hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            title={t("Reorder workspace {name}", {
                              name: workspace.name,
                            })}
                            draggable={!isSavingWorkspaceOrder}
                            onDragEnd={handleWorkspaceDragEnd}
                            onDragStart={(event) =>
                              handleWorkspaceDragStart(event, workspace.id)
                            }
                          >
                            {isSavingWorkspaceOrder && draggedWorkspaceId === workspace.id ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="size-4 animate-spin"
                              />
                            ) : (
                              <GripVertical aria-hidden="true" className="size-4" />
                            )}
                          </span>
                        </div>
                        <div className="flex min-w-0 items-center gap-3 select-text">
                          <WorkspaceIcon
                            className="size-9 shrink-0 rounded-lg border border-stone-200 object-cover shadow-sm"
                            fallbackClassName="size-9 shrink-0 rounded-lg border border-stone-200 bg-stone-50 p-2 text-stone-500 shadow-sm"
                            logoUrl={workspace.logoUrl}
                          />
                          <div className="min-w-0">
                            <div className="flex min-w-0 items-center gap-2">
                              <span className="min-w-0 truncate text-sm font-semibold">
                                {workspace.name}
                              </span>
                              {workspace.isDefault ? (
                                <CapabilityPill label={t("Default workspace")} ok />
                              ) : null}
                              {workspace.pinned ? (
                                <CapabilityPill label={t("pinned")} ok />
                              ) : null}
                            </div>
                            <div className="mt-1 truncate text-xs text-stone-500">
                              <span className="font-medium">
                                {terminalShellLabel(terminalShells, workspace.terminalShell)}
                              </span>
                              <span className="text-stone-300"> / </span>
                              <span>{workspace.path}</span>
                            </div>
                          </div>
                        </div>
                        <div className="flex gap-2 justify-end">
                          <button
                            aria-label={t(
                              workspace.pinned
                                ? "Unpin workspace {name}"
                                : "Pin workspace {name}",
                              { name: workspace.name },
                            )}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border shadow-sm ${workspace.pinned
                                ? "border-teal-300 bg-teal-700 text-white shadow-[0_10px_22px_rgba(15,118,110,0.22)] hover:bg-teal-800"
                                : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            disabled={isSavingWorkspaceOrder}
                            onClick={() =>
                              void toggleWorkspacePinned(workspace, !workspace.pinned)
                            }
                            title={t(workspace.pinned ? "Unpin workspace" : "Pin workspace")}
                            type="button"
                          >
                            <Lock aria-hidden="true" className="size-4" />
                          </button>
                          <button
                            aria-label={t("Edit workspace {name}", {
                              name: workspace.name,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={() => void editConfiguredWorkspace(workspace)}
                            title={t("Edit workspace")}
                            type="button"
                          >
                            <Pencil aria-hidden="true" className="size-4" />
                          </button>
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No workspaces")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "hooks" ? (
            <section className="grid gap-4">
              {hookRunDetail ? (
                <>
                  <button
                    aria-label={t("Close hook run detail backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setHookRunDetail(null)}
                    type="button"
                  />
                  <div
                    aria-label={t("Hook run detail")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,46rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    role="dialog"
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Webhook aria-hidden="true" className="size-5 text-teal-700" />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {hookEventLabel(hookRunDetail.event, t)}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          {hookRunDetail.id}
                        </div>
                      </div>
                      <button
                        aria-label={t("Close hook run detail")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={() => setHookRunDetail(null)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    <div className="grid gap-3">
                      <div className="grid gap-2 sm:grid-cols-3">
                        <CapabilityPill
                          label={hookRunStatusLabel(hookRunDetail.status, t)}
                          ok={hookRunDetail.status === "succeeded"}
                        />
                        <CapabilityPill
                          label={hookSourceLabel(hookRunDetail.hookSource, t)}
                          ok={hookRunDetail.hookSource === "global"}
                        />
                        <CapabilityPill
                          label={hookHandlerTypeLabel(hookRunDetail.handlerType, t)}
                          ok
                        />
                      </div>
                      {hookRunDetail.stdoutPreview ? (
                        <pre className="max-h-32 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                          {hookRunDetail.stdoutPreview}
                        </pre>
                      ) : null}
                      {hookRunDetail.stderrPreview ? (
                        <pre className="max-h-32 overflow-auto rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700">
                          {hookRunDetail.stderrPreview}
                        </pre>
                      ) : null}
                      <div className="grid gap-3 lg:grid-cols-2">
                        <pre className="max-h-80 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                          {JSON.stringify(hookRunDetail.input, null, 2)}
                        </pre>
                        <pre className="max-h-80 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                          {JSON.stringify(hookRunDetail.output, null, 2)}
                        </pre>
                      </div>
                    </div>
                  </div>
                </>
              ) : null}

              {isHookDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close hook configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsHookDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Hook configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,40rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    onSubmit={(event) => void submitHookForm(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Webhook aria-hidden="true" className="size-5 text-teal-700" />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {hookForm.handlerIndex === null
                              ? t("Add hook")
                              : t("Edit hook")}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-stone-500">
                          {hookScope === "global" ? t("Global hooks") : selectedHookWorkspace?.name}
                        </div>
                      </div>
                      <button
                        aria-label={t("Close hook configuration")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={() => setIsHookDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>

                    <div className="grid gap-3">
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Event")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                event: event.target.value,
                                groupIndex:
                                  current.handlerIndex === null ? null : current.groupIndex,
                                handlerIndex:
                                  current.handlerIndex === null ? null : current.handlerIndex,
                              }))
                            }
                            value={hookForm.event}
                          >
                            {(hookSettings?.supportedEvents ?? []).map((eventName) => (
                              <option key={eventName} value={eventName}>
                                {hookEventLabel(eventName, t)}
                              </option>
                            ))}
                          </select>
                        </label>
                        <TextField
                          label={t("Matcher")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, matcher: value }))
                          }
                          placeholder="run_command|write_file"
                          value={hookForm.matcher}
                        />
                      </div>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                        <span className="text-sm font-semibold text-stone-700">
                          {t("Enable hook")}
                        </span>
                        <input
                          checked={hookForm.enabled}
                          className="size-4 accent-teal-700"
                          onChange={(event) =>
                            setHookForm((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Handler type")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                type: event.target.value as HookHandlerType,
                              }))
                            }
                            value={hookForm.type}
                          >
                            {["command", "http", "mcp_tool", "prompt"].map((type) => (
                              <option key={type} value={type}>
                                {hookHandlerTypeLabel(type, t)}
                              </option>
                            ))}
                          </select>
                        </label>
                        <TextField
                          label={t("If filter")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, ifFilter: value }))
                          }
                          placeholder="run_command(git *)"
                          value={hookForm.ifFilter}
                        />
                      </div>

                      {hookForm.type === "command" ? (
                        <>
                          <TextField
                            label={t("Command")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, command: value }))
                            }
                            placeholder="node scripts/hook.js"
                            value={hookForm.command}
                          />
                          <div className="grid gap-3 sm:grid-cols-2">
                            <TextField
                              label={t("Shell")}
                              onChange={(value) =>
                                setHookForm((current) => ({ ...current, shell: value }))
                              }
                              placeholder="powershell"
                              value={hookForm.shell}
                            />
                            <TextField
                              inputMode="numeric"
                              label={t("Timeout ms")}
                              onChange={(value) =>
                                setHookForm((current) => ({ ...current, timeout: value }))
                              }
                              placeholder="30000"
                              value={hookForm.timeout}
                            />
                          </div>
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("Args")}
                            </span>
                            <textarea
                              className="min-h-20 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                setHookForm((current) => ({
                                  ...current,
                                  argsText: event.target.value,
                                }))
                              }
                              placeholder={"scripts/hook.js\n--check"}
                              value={hookForm.argsText}
                            />
                          </label>
                        </>
                      ) : null}

                      {hookForm.type === "http" ? (
                        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem]">
                          <TextField
                            label={t("URL")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, url: value }))
                            }
                            placeholder="http://127.0.0.1:8787/hook"
                            value={hookForm.url}
                          />
                          <TextField
                            inputMode="numeric"
                            label={t("Timeout ms")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, timeout: value }))
                            }
                            placeholder="30000"
                            value={hookForm.timeout}
                          />
                        </div>
                      ) : null}

                      {hookForm.type === "mcp_tool" ? (
                        <div className="grid gap-3 sm:grid-cols-2">
                          <TextField
                            label={t("MCP server id")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, serverId: value }))
                            }
                            placeholder="server"
                            value={hookForm.serverId}
                          />
                          <TextField
                            label={t("MCP tool name")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, toolName: value }))
                            }
                            placeholder="validate"
                            value={hookForm.toolName}
                          />
                        </div>
                      ) : null}

                      {hookForm.type === "prompt" ? (
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Prompt")}
                          </span>
                          <textarea
                            className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                prompt: event.target.value,
                              }))
                            }
                            placeholder={t("Return a JSON hook result.")}
                            value={hookForm.prompt}
                          />
                        </label>
                      ) : null}

                      <div className="grid gap-3 sm:grid-cols-2">
                        <TextField
                          label={t("Status message")}
                          onChange={(value) =>
                            setHookForm((current) => ({
                              ...current,
                              statusMessage: value,
                            }))
                          }
                          placeholder={t("Running hook")}
                          value={hookForm.statusMessage}
                        />
                        <TextField
                          inputMode="numeric"
                          label={t("Timeout ms")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, timeout: value }))
                          }
                          placeholder="60000"
                          value={hookForm.timeout}
                        />
                      </div>
                      <div className="grid gap-2 sm:grid-cols-2">
                        <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                          <span className="text-sm font-semibold text-stone-700">
                            {t("Async")}
                          </span>
                          <input
                            checked={hookForm.asyncHook}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                asyncHook: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                          <span className="text-sm font-semibold text-stone-700">
                            {t("Async re-wake")}
                          </span>
                          <input
                            checked={hookForm.asyncRewake}
                            className="size-4 accent-teal-700"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                asyncRewake: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                      </div>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Input override JSON")}
                        </span>
                        <textarea
                          className="min-h-20 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setHookForm((current) => ({
                              ...current,
                              inputText: event.target.value,
                            }))
                          }
                          placeholder="{ }"
                          value={hookForm.inputText}
                        />
                      </label>
                      <button
                        aria-label={t("Save hook")}
                        className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                        disabled={isSavingHooks || !hookForm.event || !hookForm.type}
                        title={t("Save hook")}
                        type="submit"
                      >
                        {isSavingHooks ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <CheckCircle2 aria-hidden="true" className="size-4" />
                        )}
                        {t("Save")}
                      </button>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="grid gap-3 lg:grid-cols-[auto_minmax(0,1fr)_auto]">
                  <div className="inline-flex h-10 rounded-lg border border-stone-200 bg-stone-100 p-1">
                    {(["global", "workspace"] as HookScope[]).map((scope) => (
                      <button
                        className={`rounded-md px-3 text-sm font-semibold ${hookScope === scope
                            ? "bg-white text-teal-900 shadow-sm"
                            : "text-stone-600 hover:text-stone-950"
                          }`}
                        key={scope}
                        onClick={() => setHookScope(scope)}
                        type="button"
                      >
                        {scope === "global" ? t("Global") : t("Workspace")}
                      </button>
                    ))}
                  </div>
                  <label className="min-w-0">
                    <span className="sr-only">{t("Workspace")}</span>
                    <select
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) => {
                        setHookWorkspaceId(event.target.value);
                        setHookRunDetail(null);
                      }}
                      value={hookWorkspaceId}
                    >
                      {workspaces.map((workspace) => (
                        <option key={workspace.id} value={workspace.id}>
                          {workspace.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="flex gap-2">
                    <button
                      aria-label={t("Add hook")}
                      className="inline-flex size-10 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300"
                      disabled={!hookSettings}
                      onClick={startAddingHookHandler}
                      title={t("Add hook")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </button>
                    <button
                      aria-label={t("Reload hooks")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                      disabled={isLoadingHooks || !selectedHookWorkspace}
                      onClick={() =>
                        selectedHookWorkspace
                          ? void loadHooks(selectedHookWorkspace.id)
                          : undefined
                      }
                      title={t("Reload hooks")}
                      type="button"
                    >
                      {isLoadingHooks ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
                <div className="mt-3 break-all rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                  {activeHookPath ?? t("Loading...")}
                </div>
                <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                  <span className="text-sm font-semibold text-stone-700">
                    {t("Disable all hooks")}
                  </span>
                  <input
                    checked={Boolean(activeHookConfig?.disableAllHooks)}
                    className="size-4 accent-teal-700"
                    disabled={isSavingHooks || !activeHookConfig}
                    onChange={(event) =>
                      updateHookConfig({
                        ...(activeHookConfig ?? emptyHookConfig()),
                        disableAllHooks: event.target.checked,
                      })
                    }
                    type="checkbox"
                  />
                </label>
                <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                  <span className="text-sm font-semibold text-stone-700">
                    {t("Record hook run logs")}
                  </span>
                  <input
                    checked={generalForm.hookAuditEnabled}
                    className="size-4 accent-teal-700"
                    disabled={isSavingGeneral || !settings}
                    onChange={(event) => void saveHookAuditEnabled(event.target.checked)}
                    type="checkbox"
                  />
                </label>
              </section>

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Hook rules")}
                  </h3>
                  <CapabilityPill
                    label={t("rules {count}", { count: activeHookGroups.length })}
                    ok={activeHookGroups.length > 0}
                  />
                </div>
                <div className="divide-y divide-stone-100">
                  {activeHookGroups.length ? (
                    activeHookGroups.map((entry) => (
                      <div className="px-4 py-3" key={`${entry.event}-${entry.groupIndex}`}>
                        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-sm font-semibold text-stone-950">
                                {entry.event}
                              </span>
                              <CapabilityPill
                                label={entry.group.enabled === false ? t("disabled") : t("enabled")}
                                ok={entry.group.enabled !== false}
                              />
                              <CapabilityPill
                                label={entry.group.matcher || "*"}
                                ok={Boolean(entry.group.matcher)}
                              />
                            </div>
                            <div className="mt-1 text-xs text-stone-500">
                              {t("handlers {count}", { count: entry.group.hooks.length })}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <button
                              aria-label={t("Move hook up")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                              disabled={entry.groupIndex === 0 || isSavingHooks}
                              onClick={() => moveHookGroup(entry.event, entry.groupIndex, -1)}
                              title={t("Move hook up")}
                              type="button"
                            >
                              <ArrowUp aria-hidden="true" className="size-4" />
                            </button>
                            <button
                              aria-label={t("Move hook down")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                              disabled={
                                entry.groupIndex >=
                                hookGroupsForEvent(activeHookConfig, entry.event).length - 1 ||
                                isSavingHooks
                              }
                              onClick={() => moveHookGroup(entry.event, entry.groupIndex, 1)}
                              title={t("Move hook down")}
                              type="button"
                            >
                              <ArrowDown aria-hidden="true" className="size-4" />
                            </button>
                            <label className="relative inline-flex cursor-pointer items-center">
                              <input
                                aria-label={t("Enable hook group")}
                                checked={entry.group.enabled !== false}
                                className="peer sr-only"
                                disabled={isSavingHooks}
                                onChange={(event) =>
                                  toggleHookGroup(
                                    entry.event,
                                    entry.groupIndex,
                                    event.target.checked,
                                  )
                                }
                                type="checkbox"
                              />
                              <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                              <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                            </label>
                          </div>
                        </div>
                        <div className="mt-3 space-y-2">
                          {entry.group.hooks.map((handler, handlerIndex) => (
                            <div
                              className="grid gap-3 rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
                              key={`${entry.event}-${entry.groupIndex}-${handlerIndex}`}
                            >
                              <div className="min-w-0">
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="font-mono text-xs font-semibold text-stone-800">
                                    {handler.type}
                                  </span>
                                  <CapabilityPill
                                    label={handler.enabled === false ? t("disabled") : t("enabled")}
                                    ok={handler.enabled !== false}
                                  />
                                  {handler.if ? (
                                    <CapabilityPill label={handler.if} ok />
                                  ) : null}
                                  {handler.async ? (
                                    <CapabilityPill label={t("async")} ok />
                                  ) : null}
                                </div>
                                <div className="mt-1 truncate text-xs text-stone-500">
                                  {hookHandlerSummary(handler)}
                                </div>
                              </div>
                              <div className="flex flex-wrap gap-2">
                                <button
                                  aria-label={t("Move handler up")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                  disabled={handlerIndex === 0 || isSavingHooks}
                                  onClick={() =>
                                    moveHookHandler(
                                      entry.event,
                                      entry.groupIndex,
                                      handlerIndex,
                                      -1,
                                    )
                                  }
                                  title={t("Move handler up")}
                                  type="button"
                                >
                                  <ArrowUp aria-hidden="true" className="size-4" />
                                </button>
                                <button
                                  aria-label={t("Move handler down")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                                  disabled={
                                    handlerIndex >= entry.group.hooks.length - 1 ||
                                    isSavingHooks
                                  }
                                  onClick={() =>
                                    moveHookHandler(
                                      entry.event,
                                      entry.groupIndex,
                                      handlerIndex,
                                      1,
                                    )
                                  }
                                  title={t("Move handler down")}
                                  type="button"
                                >
                                  <ArrowDown aria-hidden="true" className="size-4" />
                                </button>
                                <button
                                  aria-label={t("Edit hook")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                  onClick={() =>
                                    editHookHandler(
                                      entry.event,
                                      entry.groupIndex,
                                      handlerIndex,
                                      entry.group,
                                      handler,
                                    )
                                  }
                                  title={t("Edit hook")}
                                  type="button"
                                >
                                  <Pencil aria-hidden="true" className="size-4" />
                                </button>
                                <button
                                  aria-label={t("Delete hook")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                                  disabled={isSavingHooks}
                                  onClick={() =>
                                    deleteHookHandler(
                                      entry.event,
                                      entry.groupIndex,
                                      handlerIndex,
                                    )
                                  }
                                  title={t("Delete hook")}
                                  type="button"
                                >
                                  <Trash2 aria-hidden="true" className="size-4" />
                                </button>
                                <label className="relative inline-flex cursor-pointer items-center">
                                  <input
                                    aria-label={t("Enable hook")}
                                    checked={handler.enabled !== false}
                                    className="peer sr-only"
                                    disabled={isSavingHooks}
                                    onChange={(event) =>
                                      toggleHookHandler(
                                        entry.event,
                                        entry.groupIndex,
                                        handlerIndex,
                                        event.target.checked,
                                      )
                                    }
                                    type="checkbox"
                                  />
                                  <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                                  <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                                </label>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No hook rules")}
                    </div>
                  )}
                </div>
              </section>

              <div className="grid gap-4 xl:grid-cols-2">
                <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                  <div className="flex items-center justify-between gap-3">
                    <h3 className="text-sm font-semibold text-stone-950">
                      {t("Import Claude hooks")}
                    </h3>
                    <div className="flex gap-2">
                      <button
                        aria-label={t("Import to global hooks")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                        disabled={isImportingHooks}
                        onClick={() => void importClaudeHooks("global")}
                        title={t("Import to global hooks")}
                        type="button"
                      >
                        {isImportingHooks ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <Globe aria-hidden="true" className="size-4" />
                        )}
                        {t("Global")}
                      </button>
                      <button
                        aria-label={t("Import to workspace hooks")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                        disabled={isImportingHooks || !selectedHookWorkspace}
                        onClick={() => void importClaudeHooks("workspace")}
                        title={t("Import to workspace hooks")}
                        type="button"
                      >
                        <Folder aria-hidden="true" className="size-4" />
                        {t("Workspace")}
                      </button>
                    </div>
                  </div>
                  <p className="mt-2 text-xs text-stone-500">
                    {t("Global import reads user Claude settings; workspace import reads the selected workspace.")}
                  </p>
                  {hookImportResult ? (
                    <div
                      className={`mt-3 rounded-lg border px-3 py-2 text-sm ${hookImportResult.saved
                          ? "border-teal-200 bg-teal-50 text-teal-800"
                          : "border-amber-200 bg-amber-50 text-amber-800"
                        }`}
                    >
                      <div className="font-semibold">
                        {hookImportResult.saved ? t("Import saved") : t("Import not saved")}
                      </div>
                      <div className="mt-1 break-all text-xs">{hookImportResult.path}</div>
                      {hookImportResult.importedFiles.length ? (
                        <div className="mt-2 space-y-1">
                          {hookImportResult.importedFiles.map((path) => (
                            <div className="break-all text-xs" key={path}>
                              {path}
                            </div>
                          ))}
                        </div>
                      ) : null}
                      {hookImportResult.validationErrors.length ? (
                        <div className="mt-2 space-y-1">
                          {hookImportResult.validationErrors.map((message) => (
                            <div className="break-words text-xs" key={message}>
                              {message}
                            </div>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  ) : null}
                </section>

                <form
                  className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
                  onSubmit={(event) => void testHooks(event)}
                >
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Test hook")}
                  </h3>
                  <div className="mt-3 grid gap-3">
                    <div className="grid gap-3 sm:grid-cols-2">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Event")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) => setHookTestEvent(event.target.value)}
                          value={hookTestEvent}
                        >
                          {(hookSettings?.supportedEvents ?? []).map((eventName) => (
                            <option key={eventName} value={eventName}>
                              {hookEventLabel(eventName, t)}
                            </option>
                          ))}
                        </select>
                      </label>
                      <TextField
                        label={t("Match value")}
                        onChange={setHookTestMatcher}
                        placeholder="run_command"
                        value={hookTestMatcher}
                      />
                    </div>
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Sample payload")}
                      </span>
                      <textarea
                        className="min-h-28 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) => setHookTestPayload(event.target.value)}
                        value={hookTestPayload}
                      />
                    </label>
                    <button
                      aria-label={t("Run hook test")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                      disabled={isTestingHooks || !selectedHookWorkspace}
                      title={t("Run hook test")}
                      type="submit"
                    >
                      {isTestingHooks ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <CheckCircle2 aria-hidden="true" className="size-4" />
                      )}
                      {t("Run")}
                    </button>
                  </div>
                  {hookTestResult ? (
                    <pre className="mt-3 max-h-48 overflow-auto rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                      {JSON.stringify(hookTestResult, null, 2)}
                    </pre>
                  ) : null}
                </form>
              </div>

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Effective hooks")}
                  </h3>
                  <CapabilityPill
                    label={t("hooks {count}", { count: hookSettings?.effective.length ?? 0 })}
                    ok={(hookSettings?.effective.length ?? 0) > 0}
                  />
                </div>
                <div className="divide-y divide-stone-100">
                  {hookSettings?.effective.length ? (
                    hookSettings.effective.map((hook, index) => {
                      const lastRun = latestHookRunForSummary(
                        hook,
                        hookSettings.recentRuns,
                      );

                      return (
                        <div className="grid gap-3 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto]" key={`${hook.source}-${hook.event}-${index}`}>
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-sm font-semibold text-stone-950">
                                {hookEventLabel(hook.event, t)}
                              </span>
                              <CapabilityPill
                                label={hookSourceLabel(hook.source, t)}
                                ok={hook.source === "global"}
                              />
                              <CapabilityPill
                                label={hookHandlerTypeLabel(hook.handlerType, t)}
                                ok
                              />
                              {hook.asyncHook ? <CapabilityPill label={t("async")} ok /> : null}
                              {lastRun ? (
                                <CapabilityPill
                                  label={t("last {status}", {
                                    status: hookRunStatusLabel(lastRun.status, t),
                                  })}
                                  ok={lastRun.status === "succeeded"}
                                />
                              ) : null}
                            </div>
                            <div className="mt-1 truncate text-xs text-stone-500">
                              {[hook.matcher || "*", hook.command, hook.url, hook.serverId, hook.toolName]
                                .filter(Boolean)
                                .join(" / ")}
                            </div>
                          </div>
                          <div className="text-xs text-stone-500">
                            {lastRun?.startedAt ?? hook.statusMessage ?? t("ready")}
                          </div>
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No effective hooks")}
                    </div>
                  )}
                </div>
              </section>

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Recent hook runs")}
                  </h3>
                  <button
                    aria-label={t("Refresh hook runs")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                    disabled={isRefreshingHookRuns || !selectedHookWorkspace}
                    onClick={() => void refreshHookRuns()}
                    title={t("Refresh hook runs")}
                    type="button"
                  >
                    {isRefreshingHookRuns ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <RefreshCw aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>
                <div className="divide-y divide-stone-100">
                  {hookSettings?.recentRuns.length ? (
                    hookSettings.recentRuns.map((run) => (
                      <button
                        className="grid w-full gap-3 px-4 py-3 text-left hover:bg-stone-50 md:grid-cols-[minmax(0,1fr)_auto]"
                        key={run.id}
                        onClick={() => void openHookRunDetail(run.id)}
                        type="button"
                      >
                        <span className="min-w-0">
                          <span className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-semibold text-stone-950">
                              {hookEventLabel(run.event, t)}
                            </span>
                            <CapabilityPill
                              label={hookRunStatusLabel(run.status, t)}
                              ok={run.status === "succeeded"}
                            />
                            <CapabilityPill
                              label={hookHandlerTypeLabel(run.handlerType, t)}
                              ok
                            />
                          </span>
                          <span className="mt-1 block truncate text-xs text-stone-500">
                            {run.id}
                          </span>
                        </span>
                        <span className="text-xs text-stone-500">{run.startedAt}</span>
                      </button>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No hook runs")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "providers" ? (
            <section className="grid gap-4">
              {isProviderDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close provider configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsProviderDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Provider configuration")}
                    className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[min(96vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    onSubmit={(event) => void saveProvider(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <PlugZap aria-hidden="true" className="size-5 text-teal-700" />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {providerForm.id ? t("Edit provider") : t("Add provider")}
                          </h3>
                        </div>
                        {providerForm.id ? (
                          <div className="mt-1 truncate text-xs text-stone-500">
                            {providerForm.id}
                          </div>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <label className="relative inline-flex cursor-pointer items-center">
                          <input
                            aria-label={t("Enable provider")}
                            checked={providerForm.enabled}
                            className="peer sr-only"
                            onChange={(event) =>
                              setProviderForm((current) => ({
                                ...current,
                                enabled: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                          <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                          <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                        </label>
                        {providerForm.id ? (
                          <button
                            aria-label={t("Delete provider")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                            disabled={isSavingProvider}
                            onClick={() => void deleteProvider(providerForm.id)}
                            title={t("Delete provider")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </button>
                        ) : null}
                        <button
                          aria-label={t("Close provider configuration")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                          onClick={() => setIsProviderDialogOpen(false)}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                    </div>
                    <div className="grid gap-4 lg:grid-cols-[13rem_minmax(0,1fr)]">
                      <div className="flex h-full min-h-0 flex-col rounded-xl border border-stone-200 bg-stone-50/70 p-2">
                          <div className="px-2 pb-2 text-xs font-semibold text-stone-600">
                            {t("Service provider")}
                          </div>
                          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
                            {providerServices.map((service) => (
                              <button
                                aria-pressed={selectedProviderServiceId === service.id}
                                className={`flex min-h-9 w-full items-center justify-between gap-2 rounded-lg px-2 py-2 text-left text-sm font-semibold transition ${selectedProviderServiceId === service.id
                                    ? "bg-teal-700 text-white"
                                    : "text-stone-700 hover:bg-white hover:text-teal-800"
                                  }`}
                                key={service.id}
                                onClick={() => applyProviderService(service.id)}
                                type="button"
                              >
                                <span className="min-w-0 truncate">{service.label}</span>
                                <span
                                  className={`rounded-md px-1.5 py-0.5 text-[11px] ${selectedProviderServiceId === service.id
                                      ? "bg-white/15 text-white"
                                      : "bg-stone-200 text-stone-600"
                                    }`}
                                >
                                  {formatNumber(service.kindIds.length, language)}
                                </span>
                              </button>
                            ))}
                          </div>
                        </div>
                      <div className="space-y-3">
                      <TextField
                        label={t("Name")}
                        onChange={(value) =>
                          setProviderForm((current) => ({
                            ...current,
                            name: value,
                          }))
                        }
                        placeholder="OpenAI"
                        value={providerForm.name}
                      />
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Protocol")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            updateProviderProtocol(event.target.value)
                          }
                          value={providerForm.kind || defaultProviderKind(providerKinds)}
                        >
                          {providerProtocolKinds.map((kind) => (
                            <option key={kind.kind} value={kind.kind}>
                              {kind.label}
                            </option>
                          ))}
                        </select>
                      </label>
                      <TextField
                        label={t("Base URL")}
                        onChange={(value) =>
                          setProviderForm((current) => ({
                            ...current,
                            baseUrl: value,
                          }))
                        }
                        placeholder={selectedProviderKind?.defaultBaseUrl ?? ""}
                        value={providerForm.baseUrl}
                      />
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("API key")}
                        </span>
                        <span className="relative block">
                          <input
                            autoComplete="off"
                            className={`h-10 w-full rounded-lg border border-stone-300 bg-white px-3 ${hasProviderKeyClearButton ? "pr-20" : "pr-11"} text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100`}
                            name="api-key"
                            onChange={(event) =>
                              setProviderForm((current) => ({
                                ...current,
                                apiKey: event.target.value,
                                clearApiKey: false,
                              }))
                            }
                            placeholder={
                              hasSavedProviderKey
                                ? t("Saved key is kept unless replaced")
                                : t("API key")
                            }
                            type={isProviderApiKeyVisible ? "text" : "password"}
                            value={providerForm.apiKey}
                          />
                          <button
                            aria-label={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            className={`absolute ${hasProviderKeyClearButton ? "right-10" : "right-1"} top-1 inline-flex size-8 items-center justify-center rounded-md text-stone-500 hover:bg-stone-100 hover:text-stone-900`}
                            onClick={() =>
                              setIsProviderApiKeyVisible((current) => !current)
                            }
                            title={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            type="button"
                          >
                            {isProviderApiKeyVisible ? (
                              <EyeOff aria-hidden="true" className="size-4" />
                            ) : (
                              <Eye aria-hidden="true" className="size-4" />
                            )}
                          </button>
                          {hasProviderKeyClearButton ? (
                            <button
                              aria-label={t("Clear saved API key")}
                              className={`absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md ${providerForm.clearApiKey
                                  ? "bg-rose-50 text-rose-700"
                                  : "text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                                }`}
                              onClick={() =>
                                setProviderForm((current) => ({
                                  ...current,
                                  apiKey: "",
                                  clearApiKey: true,
                                }))
                              }
                              title={t("Clear saved API key")}
                              type="button"
                            >
                              <X aria-hidden="true" className="size-4" />
                            </button>
                          ) : null}
                        </span>
                      </label>
                      <div className="rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <RefreshCw aria-hidden="true" className="size-4 text-teal-700" />
                            <h4 className="text-sm font-semibold text-stone-950">
                              {t("Model sync")}
                            </h4>
                          </div>
                          <CapabilityPill
                            label={
                              providerForm.autoSyncModels
                                ? t("auto sync")
                                : t("manual sync")
                            }
                            ok={providerForm.autoSyncModels}
                          />
                        </div>
                        <div className="mt-3 grid gap-3">
                          <label className="inline-flex items-center gap-2 text-sm font-semibold text-stone-700">
                            <input
                              aria-label={t("Auto sync provider models")}
                              checked={providerForm.autoSyncModels}
                              className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                              onChange={(event) =>
                                setProviderForm((current) => ({
                                  ...current,
                                  autoSyncModels: event.target.checked,
                                }))
                              }
                              type="checkbox"
                            />
                            {t("Auto sync provider models")}
                          </label>
                          <TextField
                            label={t("Model sync filter regex")}
                            onChange={(value) =>
                              setProviderForm((current) => ({
                                ...current,
                                modelSyncFilterRegex: value,
                              }))
                            }
                            placeholder="^gpt-4|^o"
                            value={providerForm.modelSyncFilterRegex}
                          />
                        </div>
                      </div>
                      <div className="rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <PlugZap aria-hidden="true" className="size-4 text-teal-700" />
                            <h4 className="text-sm font-semibold text-stone-950">
                              {t("AI API proxy")}
                            </h4>
                          </div>
                          <CapabilityPill
                            label={
                              providerForm.apiProxyEnabled
                                ? t("Proxy enabled")
                                : t("Proxy disabled")
                            }
                            ok={providerForm.apiProxyEnabled}
                          />
                        </div>
                        <div className="mt-3 grid gap-3">
                          <label className="inline-flex items-center gap-2 text-sm font-semibold text-stone-700">
                            <input
                              aria-label={t("Enable AI API proxy")}
                              checked={providerForm.apiProxyEnabled}
                              className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                              onChange={(event) =>
                                setProviderForm((current) => ({
                                  ...current,
                                  apiProxyEnabled: event.target.checked,
                                }))
                              }
                              type="checkbox"
                            />
                            {t("Enable AI API proxy")}
                          </label>
                          <div className="grid gap-3 sm:grid-cols-[12rem_minmax(0,1fr)]">
                            <label className="block">
                              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                {t("Proxy type")}
                              </span>
                              <select
                                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                onChange={(event) =>
                                  setProviderForm((current) => ({
                                    ...current,
                                    apiProxyType: event.target.value,
                                  }))
                                }
                                value={providerForm.apiProxyType}
                              >
                                {apiProxyTypes.map((proxyType) => (
                                  <option
                                    key={proxyType.proxyType}
                                    value={proxyType.proxyType}
                                  >
                                    {proxyType.label}
                                  </option>
                                ))}
                              </select>
                            </label>
                            <TextField
                              label={t("Proxy server")}
                              onChange={(value) =>
                                setProviderForm((current) => ({
                                  ...current,
                                  apiProxyUrl: value,
                                }))
                              }
                              placeholder="127.0.0.1:7890"
                              value={providerForm.apiProxyUrl}
                            />
                          </div>
                        </div>
                      </div>
                      <div className="rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <SlidersHorizontal aria-hidden="true" className="size-4 text-teal-700" />
                            <h4 className="text-sm font-semibold text-stone-950">
                              {t("Request overrides")}
                            </h4>
                          </div>
                          <button
                            className="inline-flex h-8 items-center gap-1 rounded-lg border border-stone-200 bg-white px-2 text-xs font-semibold text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={addProviderRequestOverride}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-3.5" />
                            {t("Add override")}
                          </button>
                        </div>
                        <p className="mt-2 text-xs leading-5 text-stone-500">
                          {t("Override top-level request headers or body fields for this provider.")}
                        </p>
                        <div className="mt-3 space-y-3">
                          {providerForm.requestOverrides.length ? (
                            providerForm.requestOverrides.map((overrideRule, overrideIndex) => (
                              <div
                                className="rounded-lg border border-stone-200 bg-white p-3"
                                key={overrideIndex}
                              >
                                <div className="grid gap-3 lg:grid-cols-[7rem_minmax(0,1fr)_8rem_minmax(0,1fr)_2.5rem]">
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Target")}
                                    </span>
                                    <select
                                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                      onChange={(event) =>
                                        updateProviderRequestOverride(overrideIndex, {
                                          target: event.target.value as ProviderRequestOverrideTarget,
                                        })
                                      }
                                      value={overrideRule.target}
                                    >
                                      <option value="header">{t("Header")}</option>
                                      <option value="body">{t("Body")}</option>
                                    </select>
                                  </label>
                                  <TextField
                                    label={t("Field")}
                                    onChange={(value) =>
                                      updateProviderRequestOverride(overrideIndex, { name: value })
                                    }
                                    placeholder={overrideRule.target === "header" ? "User-Agent" : "model"}
                                    value={overrideRule.name}
                                  />
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                      {t("Value type")}
                                    </span>
                                    <select
                                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                      onChange={(event) =>
                                        updateProviderRequestOverride(overrideIndex, {
                                          valueType: event.target.value as ProviderRequestOverrideValueType,
                                        })
                                      }
                                      value={overrideRule.valueType}
                                    >
                                      <option value="string">{t("String")}</option>
                                      <option value="number">{t("Number")}</option>
                                      <option value="boolean">{t("Boolean")}</option>
                                    </select>
                                  </label>
                                  {overrideRule.valueType === "boolean" ? (
                                    <label className="block">
                                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                                        {t("Value")}
                                      </span>
                                      <select
                                        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                                        onChange={(event) =>
                                          updateProviderRequestOverride(overrideIndex, {
                                            value: event.target.value === "true",
                                          })
                                        }
                                        value={overrideRule.value ? "true" : "false"}
                                      >
                                        <option value="true">true</option>
                                        <option value="false">false</option>
                                      </select>
                                    </label>
                                  ) : (
                                    <TextField
                                      label={t("Value")}
                                      onChange={(value) =>
                                        updateProviderRequestOverride(overrideIndex, { value })
                                      }
                                      placeholder={
                                        overrideRule.valueType === "number"
                                          ? "1"
                                          : overrideRule.target === "header"
                                            ? "Foco/1.0"
                                            : "gpt-4.1"
                                      }
                                      value={String(overrideRule.value)}
                                    />
                                  )}
                                  <button
                                    aria-label={t("Delete override")}
                                    className="mt-6 inline-flex size-10 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 hover:bg-rose-50"
                                    onClick={() => deleteProviderRequestOverride(overrideIndex)}
                                    title={t("Delete override")}
                                    type="button"
                                  >
                                    <Trash2 aria-hidden="true" className="size-4" />
                                  </button>
                                </div>
                              </div>
                            ))
                          ) : (
                            <p className="rounded-lg border border-dashed border-stone-300 bg-white px-3 py-3 text-xs text-stone-500">
                              {t("No request overrides configured.")}
                            </p>
                          )}
                        </div>
                      </div>
                      <button
                        aria-label={t("Save provider")}
                        className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                        disabled={
                          isSavingProvider ||
                          !providerForm.name.trim() ||
                          !providerForm.kind.trim() ||
                          providerForm.requestOverrides.some(
                            (overrideRule) =>
                              !overrideRule.name.trim() ||
                              (overrideRule.valueType !== "boolean" &&
                                String(overrideRule.value).trim() === "") ||
                              (overrideRule.valueType === "number" &&
                                Number.isNaN(Number(overrideRule.value))),
                          )
                        }
                        title={t("Save provider")}
                        type="submit"
                      >
                        {isSavingProvider ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : (
                          <KeyRound aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </div>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="order-1 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Configured providers")}
                  </h3>
                  <div className="flex gap-2">
                    <button
                      aria-label={t("Add provider")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={startAddingProvider}
                      title={t("Add provider")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </button>
                    <button
                      aria-label={t("Refresh provider models")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      disabled={isLoadingSettings || isRefreshingProviderModels}
                      onClick={() => void refreshProviderModels()}
                      title={t("Refresh provider models")}
                      type="button"
                    >
                      {isRefreshingProviderModels ? (
                        <LoaderCircle
                          aria-hidden="true"
                          className="size-4 animate-spin"
                        />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
                <div className="divide-y divide-stone-100">
                  {providers.length ? (
                    providers.map((provider) => {
                      const test = providerTests[provider.id];
                      const modelList = providerModelLists[provider.id];
                      const isExpanded = expandedProviderIds.has(provider.id);

                      return (
                        <div className="px-4 py-3" key={provider.id}>
                          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                            <button
                              aria-expanded={isExpanded}
                              aria-label={
                                isExpanded
                                  ? t("Hide provider models for {name}", {
                                    name: provider.name,
                                  })
                                  : t("Load provider models for {name}", {
                                    name: provider.name,
                                  })
                              }
                              className="-mx-2 -my-1 flex min-w-0 items-start gap-2 rounded-lg px-2 py-1 text-left hover:bg-stone-50 focus:outline-none focus:ring-2 focus:ring-teal-100"
                              onClick={() => toggleProviderModels(provider.id)}
                              title={
                                isExpanded
                                  ? t("Hide provider models")
                                  : t("Load provider models")
                              }
                              type="button"
                            >
                              <ChevronDown
                                aria-hidden="true"
                                className={`mt-0.5 size-4 shrink-0 text-stone-400 transition ${isExpanded ? "" : "-rotate-90"}`}
                              />
                              <div className="min-w-0">
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="truncate text-sm font-medium">
                                    {provider.name}
                                  </span>
                                  <CapabilityPill
                                    label={
                                      provider.enabled ? t("enabled") : t("disabled")
                                    }
                                    ok={provider.enabled}
                                  />
                                  <CapabilityPill
                                    label={
                                      provider.hasApiKey
                                        ? t("key saved")
                                        : t("key missing")
                                    }
                                    ok={provider.hasApiKey}
                                  />
                                  <CapabilityPill
                                    label={
                                      provider.autoSyncModels
                                        ? t("auto sync")
                                        : t("manual sync")
                                    }
                                    ok={provider.autoSyncModels}
                                  />
                                </div>
                                <div className="mt-1 truncate text-xs font-medium text-stone-500">
                                  {provider.id} / {provider.kindLabel}
                                </div>
                                {provider.modelSyncFilterRegex ? (
                                  <div className="mt-1 truncate font-mono text-xs text-stone-500">
                                    {t("sync regex {pattern}", {
                                      pattern: provider.modelSyncFilterRegex,
                                    })}
                                  </div>
                                ) : null}
                                {provider.baseUrl ? (
                                  <div className="mt-1 truncate text-xs text-stone-500">
                                    {provider.baseUrl}
                                  </div>
                                ) : null}
                              </div>
                            </button>
                            <div className="flex flex-wrap gap-2">
                              <button
                                aria-label={t("Edit provider {name}", {
                                  name: provider.name,
                                })}
                                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                onClick={() => editConfiguredProvider(provider)}
                                title={t("Edit provider")}
                                type="button"
                              >
                                <SlidersHorizontal aria-hidden="true" className="size-4" />
                              </button>
                              <button
                                aria-label={t("Test provider {name}", {
                                  name: provider.name,
                                })}
                                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                                disabled={test?.status === "testing"}
                                onClick={() => void testProvider(provider.id)}
                                title={t("Test provider")}
                                type="button"
                              >
                                {test?.status === "testing" ? (
                                  <LoaderCircle
                                    aria-hidden="true"
                                    className="size-4 animate-spin"
                                  />
                                ) : (
                                  <PlugZap aria-hidden="true" className="size-4" />
                                )}
                              </button>
                            </div>
                          </div>
                          {isExpanded ? (
                            <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50/70 px-3 py-3">
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-900">
                                  <ListChecks
                                    aria-hidden="true"
                                    className="size-4 shrink-0 text-teal-700"
                                  />
                                  <span>{t("Provider models")}</span>
                                </div>
                                {modelList?.status === "ok" ? (
                                  <CapabilityPill
                                    label={t("models {count}", {
                                      count: modelList.models.length,
                                    })}
                                    ok={modelList.models.length > 0}
                                  />
                                ) : null}
                              </div>
                              {modelList?.status === "error" ? (
                                <div className="mt-3 rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                                  {modelList.message}
                                </div>
                              ) : modelList?.status === "ok" ? (
                                modelList.models.length ? (
                                  <div className="mt-3 max-h-56 overflow-y-auto rounded-md border border-stone-200 bg-white">
                                    {modelList.models.map((modelId, modelIndex) => (
                                      <div
                                        className="border-b border-stone-100 px-3 py-2 font-mono text-xs text-stone-700 last:border-b-0"
                                        key={`${modelId}-${modelIndex}`}
                                      >
                                        {modelId}
                                      </div>
                                    ))}
                                  </div>
                                ) : (
                                  <div className="mt-3 rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-500">
                                    {t("No provider models returned")}
                                  </div>
                                )
                              ) : (
                                <div className="mt-3 flex items-center gap-2 rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-500">
                                  <LoaderCircle
                                    aria-hidden="true"
                                    className="size-4 animate-spin"
                                  />
                                  {t("Loading provider models...")}
                                </div>
                              )}
                            </div>
                          ) : null}
                          {test ? (
                            <div
                              className={`mt-3 rounded-lg border px-3 py-2 text-sm ${test.status === "ok"
                                  ? "border-teal-200 bg-teal-50 text-teal-800"
                                  : test.status === "testing"
                                    ? "border-stone-200 bg-stone-50 text-stone-600"
                                    : "border-rose-200 bg-rose-50 text-rose-700"
                                }`}
                            >
                              {test.message}
                            </div>
                          ) : null}
                          <Warnings warnings={provider.warnings} />
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No configured providers")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "mcp" ? (
            <section className="grid gap-4">
              {isMcpDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close MCP server configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsMcpDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("MCP server configuration")}
                    className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    onSubmit={(event) => void saveMcpServer(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Server aria-hidden="true" className="size-5 text-teal-700" />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {mcpForm.id ? t("Edit MCP server") : t("Add MCP server")}
                          </h3>
                        </div>
                        {mcpForm.id ? (
                          <div className="mt-1 truncate text-xs text-stone-500">
                            {mcpForm.id}
                          </div>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <label className="relative inline-flex cursor-pointer items-center">
                          <input
                            aria-label={t("Enable MCP server")}
                            checked={mcpForm.enabled}
                            className="peer sr-only"
                            onChange={(event) =>
                              setMcpForm((current) => ({
                                ...current,
                                enabled: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                          <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                          <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                        </label>
                        {mcpForm.id ? (
                          <button
                            aria-label={t("Delete MCP server")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                            disabled={isSavingMcpServer}
                            onClick={() => void deleteMcpServer(mcpForm.id)}
                            title={t("Delete MCP server")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </button>
                        ) : null}
                        <button
                          aria-label={t("Close MCP server configuration")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                          onClick={() => setIsMcpDialogOpen(false)}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </button>
                      </div>
                    </div>
                    <div className="space-y-3">
                      <TextField
                        label={t("Name")}
                        onChange={(value) =>
                          setMcpForm((current) => ({
                            ...current,
                            name: value,
                          }))
                        }
                        placeholder="CodeGraph"
                        value={mcpForm.name}
                      />
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                          {t("Transport")}
                        </span>
                        <select
                          className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                          onChange={(event) =>
                            setMcpForm((current) => ({
                              ...current,
                              transport: event.target.value,
                            }))
                          }
                          value={mcpForm.transport || mcpTransports[0]?.transport || ""}
                        >
                          {mcpTransports.map((transport) => (
                            <option
                              key={transport.transport}
                              value={transport.transport}
                            >
                              {t(transport.label)}
                            </option>
                          ))}
                        </select>
                      </label>
                      {mcpForm.transport === "streamable-http" ? (
                        <TextField
                          label={t("URL")}
                          onChange={(value) =>
                            setMcpForm((current) => ({
                              ...current,
                              url: value,
                            }))
                          }
                          placeholder="http://127.0.0.1:8000/mcp"
                          value={mcpForm.url}
                        />
                      ) : (
                        <>
                          <TextField
                            label={t("Command")}
                            onChange={(value) =>
                              setMcpForm((current) => ({
                                ...current,
                                command: value,
                              }))
                            }
                            placeholder="codegraph"
                            value={mcpForm.command}
                          />
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("Args")}
                            </span>
                            <textarea
                              className="min-h-24 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) =>
                                setMcpForm((current) => ({
                                  ...current,
                                  argsText: event.target.value,
                                }))
                              }
                              placeholder={"serve\n--stdio"}
                              value={mcpForm.argsText}
                            />
                          </label>
                        </>
                      )}
                      <button
                        aria-label={t("Save MCP server")}
                        className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                        disabled={
                          isSavingMcpServer ||
                          !mcpForm.name.trim() ||
                          !mcpForm.transport.trim() ||
                          (mcpForm.transport === "streamable-http"
                            ? !mcpForm.url.trim()
                            : !mcpForm.command.trim())
                        }
                        title={t("Save MCP server")}
                        type="submit"
                      >
                        {isSavingMcpServer ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : mcpForm.transport === "streamable-http" ? (
                          <Globe aria-hidden="true" className="size-4" />
                        ) : (
                          <Terminal aria-hidden="true" className="size-4" />
                        )}
                      </button>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("MCP servers")}
                  </h3>
                  <div className="flex gap-2">
                    <button
                      aria-label={t("Add MCP server")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={startAddingMcpServer}
                      title={t("Add MCP server")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </button>
                    <button
                      aria-label={t("Reload MCP settings")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      disabled={isLoadingSettings}
                      onClick={() => void loadSettings()}
                      title={t("Reload settings")}
                      type="button"
                    >
                      {isLoadingSettings ? (
                        <LoaderCircle
                          aria-hidden="true"
                          className="size-4 animate-spin"
                        />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
                <div className="divide-y divide-stone-100">
                  {mcpServers.length ? (
                    mcpServers.map((server) => (
                      <div className="px-4 py-3" key={server.id}>
                        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="truncate text-sm font-medium">
                                {server.name}
                              </span>
                              <CapabilityPill
                                label={server.enabled ? t("enabled") : t("disabled")}
                                ok={server.enabled}
                              />
                              <CapabilityPill
                                label={t(server.state)}
                                ok={server.state === "connected"}
                              />
                              <CapabilityPill
                                label={t("tools {count}", {
                                  count: server.toolCount,
                                })}
                                ok={server.toolCount > 0}
                              />
                            </div>
                            <div className="mt-1 truncate text-xs font-medium text-stone-500">
                              {server.id} / {server.transportLabel}
                            </div>
                            <div className="mt-1 truncate text-xs text-stone-500">
                              {server.transport === "streamable-http"
                                ? server.url
                                : [server.command, ...server.args].filter(Boolean).join(" ")}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <button
                              aria-label={t("Edit MCP server {name}", {
                                name: server.name,
                              })}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              onClick={() => editConfiguredMcpServer(server)}
                              title={t("Edit MCP server")}
                              type="button"
                            >
                              <SlidersHorizontal aria-hidden="true" className="size-4" />
                            </button>
                          </div>
                        </div>
                        {server.error ? (
                          <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                            {server.error}
                          </div>
                        ) : null}
                        <Warnings warnings={server.warnings} />
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No configured MCP servers")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "skills" ? (
            <section className="grid gap-4">
              <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Detected skills")}
                  </h3>
                  <div className="flex items-center gap-2">
                    <CapabilityPill
                      label={t("skills {count}", {
                        count: skills?.detected.length ?? 0,
                      })}
                      ok={(skills?.detected.length ?? 0) > 0}
                    />
                    <button
                      aria-label={t("Refresh skill discovery")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                      disabled={isRefreshingSkills}
                      onClick={() => void refreshSkills()}
                      title={t("Refresh skill discovery")}
                      type="button"
                    >
                      {isRefreshingSkills ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
                <div className="divide-y divide-stone-100">
                  {skills?.detected.length ? (
                    skills.detected.map((skill) => {
                      const enabled = enabledSkillIds.has(skill.key);

                      return (
                        <div className="px-4 py-3" key={skill.key}>
                          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                            <div className="min-w-0">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="truncate text-sm font-medium">
                                  {skill.name}
                                </span>
                                <CapabilityPill
                                  label={enabled ? t("enabled") : t("disabled")}
                                  ok={enabled}
                                />
                                <CapabilityPill
                                  label={skillScopeLabel(skill, t)}
                                  ok={skill.scope === "global"}
                                />
                              </div>
                              <div className="mt-1 truncate text-xs font-medium text-stone-500">
                                {skill.key}
                              </div>
                              <div className="mt-1 break-words text-xs text-stone-500">
                                {skill.description}
                              </div>
                              <div className="mt-1 break-all text-xs text-stone-400">
                                {skill.path}
                              </div>
                            </div>
                            <label className="relative inline-flex cursor-pointer items-center justify-self-start md:justify-self-end">
                              <input
                                aria-label={t("Enable skill {name}", {
                                  name: skill.name,
                                })}
                                checked={enabled}
                                className="peer sr-only"
                                disabled={isSavingSkills || !skill.canEnable}
                                onChange={(event) =>
                                  toggleSkill(skill.key, event.target.checked)
                                }
                                type="checkbox"
                              />
                              <span className="h-6 w-11 rounded-full bg-stone-300 transition peer-checked:bg-teal-700" />
                              <span className="absolute left-1 size-4 rounded-full bg-white shadow transition peer-checked:translate-x-5" />
                            </label>
                          </div>
                          <Warnings warnings={skill.warnings} />
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No detected skills")}
                    </div>
                  )}
                </div>
              </section>

              <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center gap-2">
                  <Wrench aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Skill locations")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-2">
                  {skills?.directories.length ? (
                    skills.directories.map((directory) => (
                      <div
                        className="break-all rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs font-medium text-stone-600"
                        key={directory}
                      >
                        {directory}
                      </div>
                    ))
                  ) : (
                    <div className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-sm text-stone-500">
                      {t("Loading...")}
                    </div>
                  )}
                </div>
                {skills?.errors.length ? (
                  <div className="mt-4 space-y-2">
                    {skills.errors.map((skillError) => (
                      <div
                        className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700"
                        key={`${skillError.path}-${skillError.message}`}
                      >
                        <div className="break-all font-medium">{skillError.path}</div>
                        <div className="mt-1 break-words">{skillError.message}</div>
                      </div>
                    ))}
                  </div>
                ) : null}
              </section>
            </section>
          ) : null}

          {activeSection === "models" ? (
            <section className="grid gap-4">
              <div className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
                <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
                  <h3 className="text-sm font-semibold text-stone-950">
                    {t("Models")}
                  </h3>
                  <div className="flex gap-2">
                    <button
                      aria-label={t("Add model")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                      onClick={startAddingModel}
                      title={t("Add model")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </button>
                  </div>
                </div>
                <div className="divide-y divide-stone-100">
                  {orderedConfiguredModels.length ? (
                    orderedConfiguredModels.map((model) => (
                      <div
                        className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${draggedModelId === model.id
                            ? "bg-teal-50/70 opacity-80"
                            : "bg-white/0"
                          }`}
                        draggable={!isSavingModelOrder}
                        key={model.id}
                        onDragEnd={handleModelDragEnd}
                        onDragOver={(event) => handleModelDragOver(event, model.id)}
                        onDragStart={(event) => handleModelDragStart(event, model.id)}
                        onDrop={(event) => void handleModelDrop(event)}
                      >
                        <div className="flex items-center">
                          <span
                            aria-label={t("Reorder model {name}", {
                              name: model.displayName,
                            })}
                            className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${isSavingModelOrder
                                ? "cursor-not-allowed opacity-60"
                                : "cursor-grab hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              }`}
                            title={t("Reorder model {name}", {
                              name: model.displayName,
                            })}
                          >
                            {isSavingModelOrder && draggedModelId === model.id ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="size-4 animate-spin"
                              />
                            ) : (
                              <GripVertical aria-hidden="true" className="size-4" />
                            )}
                          </span>
                        </div>
                        <div className="flex min-w-0 items-center gap-2 overflow-hidden">
                          <span
                            className="min-w-0 truncate text-sm font-semibold"
                            title={model.displayName}
                          >
                            {model.displayName}
                          </span>
                          <span
                            aria-hidden="true"
                            className="shrink-0 text-xs text-stone-300"
                          >
                            /
                          </span>
                          <span
                            className="min-w-0 truncate text-xs font-medium text-stone-500"
                            title={model.id}
                          >
                            {model.id}
                          </span>
                          <CapabilityPill
                            className="min-w-0 shrink"
                            label={t("system prompt {name}", {
                              name: model.systemPromptName,
                            })}
                            ok
                            title={model.systemPromptName}
                          />
                          {!model.canEnable ? (
                            <CapabilityPill
                              className="shrink-0"
                              label={t("limits missing")}
                              ok={false}
                            />
                          ) : null}
                          <CapabilityPill
                            className="shrink-0"
                            label={t("providers {count}", {
                              count: model.providerIds.length,
                            })}
                            ok={model.providerIds.length > 0}
                          />
                          <CapabilityPill
                            className="min-w-0 shrink"
                            label={
                              model.activeProviderId
                                ? t("active {id}", { id: model.activeProviderId })
                                : t("active missing")
                            }
                            ok={model.activeProviderId !== null}
                            title={model.activeProviderId ?? undefined}
                          />
                        </div>
                        <div className="flex shrink-0 items-center gap-2">
                          <label
                            className="relative inline-flex h-6 w-11 cursor-pointer items-center disabled:cursor-not-allowed"
                            title={
                              model.enabled
                                ? t("Disable model {name}", {
                                    name: model.displayName,
                                  })
                                : t("Enable model {name}", {
                                    name: model.displayName,
                                  })
                            }
                          >
                            <input
                              aria-label={
                                model.enabled
                                  ? t("Disable model {name}", {
                                      name: model.displayName,
                                    })
                                  : t("Enable model {name}", {
                                      name: model.displayName,
                                    })
                              }
                              checked={model.enabled}
                              className="peer sr-only"
                              disabled={isSaving || (!model.canEnable && !model.enabled)}
                              onChange={(event) =>
                                void toggleConfiguredModelEnabled(
                                  model,
                                  event.target.checked,
                                )
                              }
                              type="checkbox"
                            />
                            <span className="absolute inset-0 rounded-full bg-stone-200 transition peer-checked:bg-teal-700 peer-disabled:cursor-not-allowed peer-disabled:opacity-50" />
                            <span className="absolute left-0.5 top-0.5 size-5 rounded-full bg-white shadow-sm transition peer-checked:translate-x-5 peer-disabled:opacity-80" />
                          </label>
                          <button
                            aria-label={t("Edit model {name}", {
                              name: model.displayName,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={() => editConfiguredModel(model)}
                            title={t("Edit model")}
                            type="button"
                          >
                            <SlidersHorizontal aria-hidden="true" className="size-4" />
                          </button>
                          <button
                            aria-label={t("Delete model {name}", {
                              name: model.displayName,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                            disabled={isSaving}
                            onClick={() => void deleteModel(model.id)}
                            title={t("Delete model")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </button>
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-stone-500">
                      {t("No configured models")}
                    </div>
                  )}
                </div>
              </div>

              {isModelDialogOpen ? (
                <>
                  <button
                    aria-label={t("Close model configuration backdrop")}
                    className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm"
                    onClick={() => setIsModelDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Model configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(96vw,70rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
                    onSubmit={(event) => void saveModel(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <SlidersHorizontal
                            aria-hidden="true"
                            className="size-5 text-teal-700"
                          />
                          <h3 className="text-sm font-semibold text-stone-950">
                            {editingModel ? t("Edit model") : t("Add model")}
                          </h3>
                        </div>
                        {selectedMetadata ? (
                          <div className="mt-1 truncate text-xs text-stone-500">
                            {selectedMetadata.key}
                          </div>
                        ) : null}
                      </div>
                      <button
                        aria-label={t("Close model configuration")}
                        className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                        onClick={() => setIsModelDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </button>
                    </div>

                    <div className="grid gap-4 lg:grid-cols-[minmax(0,1.05fr)_minmax(20rem,0.95fr)]">
                      <div className="space-y-3">
                        <div className="grid gap-3 sm:grid-cols-2">
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("Model developer")}
                            </span>
                            <select
                              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                              onChange={(event) => selectModelDeveloper(event.target.value)}
                              value={selectedModelDeveloper}
                            >
                              <option value="">{t("Select model developer")}</option>
                              {modelDeveloperOptions.map((developer) => (
                                <option key={developer} value={developer}>
                                  {developer}
                                </option>
                              ))}
                            </select>
                          </label>
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                              {t("Model id")}
                            </span>
                            <select
                              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                              disabled={!selectedModelDeveloper && !editingModel}
                              onChange={(event) => updateModelId(event.target.value)}
                              value={form.modelId}
                            >
                              <option value="">{t("Select model id")}</option>
                              {developerModelOptions.map((model) => (
                                <option key={model.key} value={model.value}>
                                  {model.value}
                                </option>
                              ))}
                              {editingModel &&
                                form.modelId &&
                                !developerModelOptions.some(
                                  (model) => model.value === form.modelId,
                                ) ? (
                                <option value={form.modelId}>{form.modelId}</option>
                              ) : null}
                            </select>
                          </label>
                        </div>

                        {selectedModelDeveloper && !developerModels.length ? (
                          <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                            {t("No cached models for developer")}
                          </div>
                        ) : null}

                        <TextField
                          label={t("Display name")}
                          onChange={(value) =>
                            setForm((current) => ({
                              ...current,
                              displayName: value,
                            }))
                          }
                          placeholder="GPT 5.5"
                          value={form.displayName}
                        />

                        <div className="grid gap-3 sm:grid-cols-2">
                          <TextField
                            inputMode="numeric"
                            label={t("Context window")}
                            onChange={(value) =>
                              setForm((current) => ({
                                ...current,
                                contextWindow: value,
                              }))
                            }
                            placeholder="128000"
                            value={form.contextWindow}
                          />
                          <TextField
                            inputMode="numeric"
                            label={t("Max output tokens")}
                            onChange={(value) =>
                              setForm((current) => ({
                                ...current,
                                maxOutputTokens: value,
                              }))
                            }
                            placeholder="16384"
                            value={form.maxOutputTokens}
                          />
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                            <legend className="px-1 text-xs font-semibold text-stone-600">
                              {t("Input types")}
                            </legend>
                            <div className="grid gap-2">
                              {inputModalityOptions.map((modality) => (
                                <label
                                  className="flex items-center justify-between gap-3 rounded-lg bg-white px-3 py-2 text-sm font-medium text-stone-700"
                                  key={modality}
                                >
                                  <span>{t(modality)}</span>
                                  <input
                                    checked={form.inputModalities.includes(modality)}
                                    className="size-4 accent-teal-700"
                                    onChange={(event) =>
                                      toggleModelModality(
                                        "inputModalities",
                                        modality,
                                        event.target.checked,
                                      )
                                    }
                                    type="checkbox"
                                  />
                                </label>
                              ))}
                            </div>
                          </fieldset>
                          <fieldset className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3">
                            <legend className="px-1 text-xs font-semibold text-stone-600">
                              {t("Output types")}
                            </legend>
                            <div className="grid gap-2">
                              {outputModalityOptions.map((modality) => (
                                <label
                                  className="flex items-center justify-between gap-3 rounded-lg bg-white px-3 py-2 text-sm font-medium text-stone-700"
                                  key={modality}
                                >
                                  <span>{t(modality)}</span>
                                  <input
                                    checked={form.outputModalities.includes(modality)}
                                    className="size-4 accent-teal-700"
                                    onChange={(event) =>
                                      toggleModelModality(
                                        "outputModalities",
                                        modality,
                                        event.target.checked,
                                      )
                                    }
                                    type="checkbox"
                                  />
                                </label>
                              ))}
                            </div>
                          </fieldset>
                        </div>

                        {selectedMetadata ? (
                          <div className="rounded-xl border border-stone-200 bg-stone-50/80 px-3 py-3 text-xs text-stone-600">
                            <div className="truncate font-semibold text-stone-800">
                              {t("Model metadata")}: {selectedMetadata.key}
                            </div>
                            <div className="mt-3 grid gap-2 sm:grid-cols-2">
                              <KeyValue label={t("Input")} value={priceText(selectedMetadata.pricing.input)} />
                              <KeyValue label={t("Output")} value={priceText(selectedMetadata.pricing.output)} />
                              <KeyValue label={t("Cache read")} value={priceText(selectedMetadata.pricing.cacheRead)} />
                              <KeyValue label={t("Cache write")} value={priceText(selectedMetadata.pricing.cacheWrite)} />
                              <KeyValue label={t("Reasoning")} value={priceText(selectedMetadata.pricing.reasoning)} />
                            </div>
                          </div>
                        ) : null}
                      </div>

                      <div className="space-y-3">
                        <div className="rounded-xl border border-stone-200 px-3 py-3">
                          <div className="mb-2 flex items-center justify-between gap-2">
                            <div className="text-xs font-semibold text-stone-600">
                              {t("Providers")}
                            </div>
                            <button
                              aria-label={t("Add provider")}
                              className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                              onClick={startAddingProviderFromModel}
                              title={t("Add provider")}
                              type="button"
                            >
                              <Plus aria-hidden="true" className="size-4" />
                            </button>
                          </div>
                          <div className="panel-scroll max-h-56 space-y-2 overflow-y-auto pr-1">
                            {providers.length ? (
                              providers.map((provider) => {
                                const providerSupportsCurrentModel =
                                  supportedModelProviderIdSet.has(provider.id);

                                return (
                                  <label
                                    className={`flex items-center justify-between gap-3 rounded-lg bg-stone-50/80 px-3 py-2 ${providerSupportsCurrentModel ? "" : "opacity-60"}`}
                                    key={provider.id}
                                  >
                                    <span className="min-w-0">
                                      <span className="block truncate text-sm font-semibold text-stone-700">
                                        {provider.name}
                                      </span>
                                      <span className="block truncate text-xs text-stone-500">
                                        {providerSupportsCurrentModel
                                          ? provider.kindLabel
                                          : t("Model not supported")}
                                      </span>
                                    </span>
                                    <input
                                      aria-label={provider.name}
                                      checked={selectedProviderIds.has(provider.id)}
                                      className="size-4 accent-teal-700 disabled:cursor-not-allowed"
                                      disabled={!providerSupportsCurrentModel}
                                      onChange={(event) =>
                                        toggleModelProvider(
                                          provider.id,
                                          event.target.checked,
                                        )
                                      }
                                      type="checkbox"
                                    />
                                  </label>
                                );
                              })
                            ) : (
                              <button
                                className="flex w-full items-center justify-between rounded-lg border border-dashed border-stone-300 bg-stone-50 px-3 py-3 text-left text-sm text-stone-500 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                                onClick={startAddingProviderFromModel}
                                type="button"
                              >
                                <span>{t("No providers")}</span>
                                <Plus aria-hidden="true" className="size-4" />
                              </button>
                            )}
                          </div>
                        </div>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Active provider")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                            disabled={!modelProviderIds.length}
                            onChange={(event) =>
                              setForm((current) => ({
                                ...current,
                                activeProviderId: event.target.value,
                              }))
                            }
                            value={activeModelProviderId}
                          >
                            <option value="">{t("None")}</option>
                            {modelProviderIds.map((providerId) => {
                              const provider = providers.find(
                                (item) => item.id === providerId,
                              );

                              return (
                                <option key={providerId} value={providerId}>
                                  {provider?.name ?? providerId}
                                </option>
                              );
                            })}
                          </select>
                        </label>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("Thinking level")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                            disabled={!modelThinkingEnabled}
                            onChange={(event) =>
                              setForm((current) => ({
                                ...current,
                                thinkingLevel: event.target.value,
                              }))
                            }
                            value={modelThinkingEnabled ? form.thinkingLevel : ""}
                          >
                            {modelThinkingEnabled ? null : (
                              <option value="">{t("None")}</option>
                            )}
                            {thinkingLevels.map((level) => (
                              <option key={level.value} value={level.value}>
                                {t(level.label)}
                              </option>
                            ))}
                          </select>
                        </label>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                            {t("System prompt")}
                          </span>
                          <select
                            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                            onChange={(event) =>
                              setForm((current) => ({
                                ...current,
                                systemPromptName: event.target.value,
                              }))
                            }
                            value={form.systemPromptName}
                          >
                            {savedSystemPrompts.map((prompt) => (
                              <option key={prompt.name} value={prompt.name}>
                                {prompt.name}
                              </option>
                            ))}
                          </select>
                        </label>

                        {enabledNeedsLimits ? (
                          <div className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                            <CircleAlert
                              aria-hidden="true"
                              className="size-4 shrink-0"
                            />
                            {t("Fill both limits before enabling.")}
                          </div>
                        ) : null}

                        <button
                          aria-label={t("Save model")}
                          className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                          disabled={
                            isSaving ||
                            enabledNeedsLimits ||
                            (!editingModel && !selectedModelDeveloper) ||
                            !form.modelId.trim() ||
                            !form.displayName.trim()
                          }
                          title={t("Save model")}
                          type="submit"
                        >
                          {isSaving ? (
                            <LoaderCircle
                              aria-hidden="true"
                              className="size-4 animate-spin"
                            />
                          ) : (
                            <CheckCircle2 aria-hidden="true" className="size-4" />
                          )}
                        </button>
                      </div>
                    </div>
                  </form>
                </>
              ) : null}

            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function settingsSectionTitle(section: SettingsSection, t: Translate) {
  if (section === "general") {
    return t("General settings");
  }

  if (section === "workspaces") {
    return t("Workspace settings");
  }

  if (section === "prompts") {
    return t("Prompt settings");
  }

  if (section === "spec") {
    return t("Spec settings");
  }

  if (section === "plan") {
    return t("Plan settings");
  }

  if (section === "agents") {
    return t("Agent settings");
  }

  if (section === "web-search") {
    return t("Web search settings");
  }

  if (section === "hooks") {
    return t("Hook settings");
  }

  if (section === "memory") {
    return t("Memory settings");
  }

  if (section === "providers") {
    return t("Provider settings");
  }

  if (section === "models") {
    return t("Model settings");
  }

  if (section === "mcp") {
    return t("MCP settings");
  }

  return t("Skill settings");
}

function settingsSectionSubtitle(section: SettingsSection, t: Translate) {
  if (section === "general") {
    return t("Web service listen address");
  }

  if (section === "workspaces") {
    return t("Workspace order and terminal shell");
  }

  if (section === "prompts") {
    return t("System prompt and user prompt context");
  }

  if (section === "spec") {
    return t("Auto Spec model and prompts");
  }

  if (section === "plan") {
    return t("Plan automation and history");
  }

  if (section === "agents") {
    return t("Agent definitions, models, tools, and permissions");
  }

  if (section === "web-search") {
    return t("Search API credentials and runtime web tools");
  }

  if (section === "hooks") {
    return t("Global and workspace lifecycle hooks");
  }

  if (section === "memory") {
    return t("Local memory graph and review queue");
  }

  if (section === "providers") {
    return t("Provider credentials and connection checks");
  }

  if (section === "mcp") {
    return t("Workspace-scoped MCP server runtimes");
  }

  if (section === "skills") {
    return t("Skill discovery and enablement");
  }

  return t("Model metadata and runtime limits");
}

function SettingsNavButton({
  active,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      aria-current={active ? "page" : undefined}
      className={`inline-flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left text-sm font-semibold ${active
          ? "bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)]"
          : "text-stone-600 hover:bg-stone-100 hover:text-stone-950"
        }`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      <span className="min-w-0 truncate">{label}</span>
    </button>
  );
}

function TextField({
  inputMode,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
}: {
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  type?: "password" | "text";
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <input
        autoComplete="off"
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        inputMode={inputMode}
        name={label.toLowerCase().replace(/\s+/g, "-")}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        type={type}
        value={value}
      />
    </label>
  );
}

function SourceValueEditor({
  id,
  isExpanded,
  minHeightClass,
  onChange,
  onToggle,
  title,
  value,
  t,
}: {
  id: string;
  isExpanded: boolean;
  minHeightClass: string;
  onChange: (value: string) => void;
  onToggle: (id: string) => void;
  title: string;
  value: string;
  t: Translate;
}) {
  const parsed = parseDisplayJson(value);

  if (!parsed) {
    return (
      <textarea
        aria-label={title}
        className={`${minHeightClass} w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 font-mono text-xs text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100`}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        value={value}
      />
    );
  }

  return (
    <div className="rounded-lg border border-stone-200 bg-stone-950 text-stone-100">
      <button
        aria-label={`${isExpanded ? t("Collapse JSON") : t("Expand JSON")} ${title}`}
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-xs font-semibold text-stone-200"
        onClick={() => onToggle(id)}
        type="button"
      >
        <span className="inline-flex min-w-0 items-center gap-2">
          <Code2 aria-hidden="true" className="size-3.5 shrink-0 text-teal-300" />
          <span className="truncate">{title}</span>
        </span>
        <span className="shrink-0 text-stone-400">
          {isExpanded ? t("Collapse JSON") : t("Expand JSON")}
        </span>
      </button>
      {isExpanded ? (
        <textarea
          aria-label={title}
          className={`${minHeightClass} w-full resize-y border-0 border-t border-stone-800 bg-stone-950 px-3 py-3 font-mono text-xs leading-relaxed text-stone-100 outline-none focus:ring-2 focus:ring-inset focus:ring-teal-500/40`}
          onChange={(event) => onChange(event.target.value)}
          spellCheck={false}
          value={parsed.pretty}
        />
      ) : (
        <div className="border-t border-stone-800 px-3 py-2 font-mono text-xs text-stone-400">
          <code>{jsonSyntaxNodes(compactToolText(parsed.pretty))}</code>
        </div>
      )}
    </div>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-lg bg-white px-3 py-2">
      <div className="truncate text-[11px] font-semibold uppercase text-stone-400">
        {label}
      </div>
      <div className="mt-0.5 truncate text-sm font-semibold text-stone-800">
        {value}
      </div>
    </div>
  );
}

function MemorySourceReadonlyDetails({
  source,
  t,
}: {
  source: MemorySourceRecord | undefined;
  t: Translate;
}) {
  if (!source) {
    return null;
  }

  return (
    <div className="grid gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-3 text-xs text-stone-600 sm:grid-cols-2">
      <div>
        <span className="font-semibold text-stone-700">{t("Memory scope")}: </span>
        {memoryScopeLabel(source.scope, t)}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Chat ID")}: </span>
        {source.chatId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Source type")}: </span>
        {source.sourceType}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Source ID")}: </span>
        {source.sourceId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Created")}: </span>
        {source.createdAt}
      </div>
      <div>
        <span className="font-semibold text-stone-700">{t("Updated")}: </span>
        {source.updatedAt}
      </div>
    </div>
  );
}

function memoryKindLabel(kind: string, t: Translate) {
  switch (kind) {
    case "constraint":
      return t("Constraint");
    case "episode":
      return t("Episode");
    case "preference":
      return t("Preference");
    case "procedure":
      return t("Procedure");
    case "project_decision":
      return t("Project decision");
    case "project_fact":
      return t("Project fact");
    case "user_note":
      return t("User note");
    default:
      return kind;
  }
}

function memoryScopeLabel(scope: string, t: Translate) {
  switch (scope) {
    case "chat":
      return t("Chat memory");
    case "global":
      return t("Global memory");
    case "workspace":
      return t("Workspace memory");
    default:
      return scope;
  }
}

function memoryStatusLabel(status: string, t: Translate) {
  switch (status) {
    case "active":
      return t("Active");
    case "expired":
      return t("Expired");
    case "pending":
      return t("Pending review");
    case "rejected":
      return t("Rejected");
    case "superseded":
      return t("Superseded");
    default:
      return status;
  }
}

function emptyProviderRequestOverride(): ProviderRequestOverrideFormState {
  return {
    target: "header",
    name: "",
    valueType: "string",
    value: "",
  };
}

function CapabilityPill({
  className,
  label,
  ok,
  title,
  tone,
}: {
  className?: string;
  label: string;
  ok: boolean;
  title?: string;
  tone?: CapabilityPillTone;
}) {
  const toneClass = capabilityPillToneClass(tone ?? (ok ? "ok" : "muted"));

  return (
    <span
      className={`inline-flex min-h-6 max-w-full items-center rounded-md border px-2 py-0.5 text-xs font-semibold ${toneClass} ${className ?? ""}`}
      title={title}
    >
      <span className="min-w-0 truncate">{label}</span>
    </span>
  );
}

type CapabilityPillTone = "ok" | "success" | "danger" | "active" | "muted";

function capabilityPillToneClass(tone: CapabilityPillTone) {
  switch (tone) {
    case "success":
      return "border-emerald-200 bg-emerald-50 text-emerald-700";
    case "danger":
      return "border-rose-200 bg-rose-50 text-rose-700";
    case "active":
      return "border-amber-200 bg-amber-50 text-amber-800";
    case "muted":
      return "border-stone-200 bg-stone-50 text-stone-500";
    case "ok":
    default:
      return "border-teal-200 bg-teal-50 text-teal-800";
  }
}

function planHistoryAction(status: string) {
  if (status === "implemented" || status === "failed" || status === "cancelled") {
    return "mark_complete";
  }

  return null;
}

function planActionLabel(action: string) {
  switch (action) {
    case "mark_complete":
      return "Mark complete";
    default:
      return action;
  }
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

function planStatusTone(status: string): CapabilityPillTone {
  if (status === "completed" || status === "implemented") {
    return "success";
  }
  if (status === "running") {
    return "active";
  }
  if (status === "failed" || status === "cancelled") {
    return "danger";
  }

  return "muted";
}

function Warnings({ warnings }: { warnings: string[] }) {
  if (!warnings.length) {
    return null;
  }

  return (
    <div className="mt-3 space-y-1">
      {warnings.map((warning) => (
        <div
          className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800"
          key={warning}
        >
          <CircleAlert aria-hidden="true" className="size-4 shrink-0" />
          <span className="min-w-0 break-words">{warning}</span>
        </div>
      ))}
    </div>
  );
}

function emptyModelForm(): ModelFormState {
  return {
    displayName: "",
    enabled: false,
    maxOutputTokens: "",
    modelId: "",
    contextWindow: "",
    providerIds: [],
    activeProviderId: "",
    inputModalities: ["text"],
    outputModalities: ["text"],
    thinkingLevel: "",
    systemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
  };
}

function emptyProviderForm(): ProviderFormState {
  return {
    apiKey: "",
    apiProxyEnabled: false,
    apiProxyType: "http",
    apiProxyUrl: "",
    baseUrl: "",
    clearApiKey: false,
    enabled: true,
    id: "",
    kind: "",
    autoSyncModels: false,
    modelSyncFilterRegex: "",
    name: "",
    requestOverrides: [],
    serviceId: "",
  };
}

function emptyGeneralForm(): GeneralFormState {
  return {
    apiRequestDetailRetentionDays: "3",
    apiSaveRequestResponseDetails: true,
    autoStartEnabled: false,
    hookAuditEnabled: false,
    language: "en",
    listenHost: "127.0.0.1",
    listenPort: "3210",
    llmRequestRetryCount: "3",
    password: "",
    theme: "light",
  };
}

function emptyWebSearchForm(): WebSearchFormState {
  return {
    activeProvider: "tavily",
    apiProxyEnabled: false,
    apiProxyType: "http",
    apiProxyUrl: "",
    braveApiKey: "",
    clearBraveApiKey: false,
    clearTavilyApiKey: false,
    enabled: false,
    tavilyApiKey: "",
  };
}

function emptyPromptSettingsForm(): PromptSettingsFormState {
  return {
    activeSystemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
    extraText: "",
    files: [],
    pendingFile: "",
    pendingSystemPromptName: "",
    pendingSystemPromptRename: "",
    renamingSystemPromptName: null,
    systemPrompts: [],
  };
}

function emptySpecSettingsForm(): SpecSettingsFormState {
  return {
    autoEnabled: true,
    generationModelId: "",
    generationSystemPrompt: "",
    updateSystemPrompt: "",
  };
}

function normalizedSystemPromptSummaries(
  prompts: PromptSettingsSummary,
): SystemPromptSummary[] {
  const systemPrompts = prompts.systemPrompts?.length
    ? prompts.systemPrompts
    : [
      {
        name: DEFAULT_SYSTEM_PROMPT_NAME,
        content: prompts.systemPrompt ?? prompts.defaultSystemPrompt,
      },
    ];

  const filteredPrompts = systemPrompts.filter(
    (prompt) => prompt.name !== IMAGE_AGENT_SYSTEM_PROMPT_NAME,
  );

  const normalizedPrompts = filteredPrompts.some(
    (prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME,
  )
    ? filteredPrompts
    : [
      {
        name: DEFAULT_SYSTEM_PROMPT_NAME,
        content: prompts.defaultSystemPrompt,
      },
      ...filteredPrompts,
    ];

  if (
    prompts.defaultPlanModeSystemPrompt &&
    !normalizedPrompts.some((prompt) => prompt.name === PLAN_MODE_SYSTEM_PROMPT_NAME)
  ) {
    const defaultIndex = normalizedPrompts.findIndex(
      (prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME,
    );
    normalizedPrompts.splice(Math.max(defaultIndex + 1, 0), 0, {
      name: PLAN_MODE_SYSTEM_PROMPT_NAME,
      content: prompts.defaultPlanModeSystemPrompt,
    });
  }

  if (
    prompts.defaultReviewSystemPrompt &&
    !normalizedPrompts.some((prompt) => prompt.name === REVIEW_SYSTEM_PROMPT_NAME)
  ) {
    const planModeIndex = normalizedPrompts.findIndex(
      (prompt) => prompt.name === PLAN_MODE_SYSTEM_PROMPT_NAME,
    );
    const defaultIndex = normalizedPrompts.findIndex(
      (prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME,
    );
    normalizedPrompts.splice(
      Math.max(planModeIndex >= 0 ? planModeIndex + 1 : defaultIndex + 1, 0),
      0,
      {
        name: REVIEW_SYSTEM_PROMPT_NAME,
        content: prompts.defaultReviewSystemPrompt,
      },
    );
  }

  return normalizedPrompts;
}

function isSystemPromptFixed(name: string): boolean {
  return (
    name === DEFAULT_SYSTEM_PROMPT_NAME ||
    name === PLAN_MODE_SYSTEM_PROMPT_NAME ||
    name === REVIEW_SYSTEM_PROMPT_NAME
  );
}

function emptyMemorySettingsForm(): MemorySettingsFormState {
  return {
    enabled: false,
    extractionMode: "manual",
    retrievalMode: "fts",
    extractionModelId: "",
    retrievalModelId: "",
    retentionDays: "",
    dream: {
      enabled: false,
      autoEnabled: false,
      mode: "llm",
      modelId: "",
      workspaceIntervalDays: "7",
      globalIntervalDays: "30",
      createTranscriptChat: true,
      maxFactsPerRun: "200",
      maxChangesPerRun: "50",
      schedulerScanMinutes: "60",
    },
  };
}

function emptyMemoryFilter(): MemoryFilterState {
  return {
    chatId: "",
    kind: "",
    page: 1,
    pageSize: 20,
    query: "",
    scope: "global",
    status: "active",
    workspaceId: "",
  };
}

function emptyManualMemoryForm(): ManualMemoryFormState {
  return {
    chatId: "",
    confidence: "",
    fact: "",
    kind: "user_note",
    metadataText: "{}",
    pinned: false,
    scope: "global",
    workspaceId: "",
  };
}

function emptyWorkspaceForm(): WorkspaceFormState {
  return {
    commonCommands: [],
    id: "",
    name: "",
    path: "",
    pinned: false,
    specEnabled: false,
    specInjectEnabled: false,
    terminalShell: "",
  };
}

function emptyMcpServerForm(): McpServerFormState {
  return {
    argsText: "",
    command: "",
    enabled: true,
    id: "",
    name: "",
    transport: "",
    url: "",
  };
}

function emptyHookConfig(): HookConfig {
  return { disableAllHooks: false };
}

function emptyHookHandlerForm(): HookHandlerFormState {
  return {
    argsText: "",
    asyncHook: false,
    asyncRewake: false,
    command: "",
    enabled: true,
    event: "PreToolUse",
    groupIndex: null,
    handlerIndex: null,
    ifFilter: "",
    inputText: "",
    matcher: "",
    prompt: "",
    serverId: "",
    shell: "",
    statusMessage: "",
    timeout: "",
    toolName: "",
    type: "command",
    url: "",
  };
}

function hookConfigEntries(config: HookConfig | null | undefined) {
  if (!config) {
    return [];
  }

  return Object.entries(config).flatMap(([event, value]) => {
    if (event === "disableAllHooks" || !Array.isArray(value)) {
      return [];
    }

    return value.map((group, groupIndex) => ({
      event,
      group,
      groupIndex,
    }));
  });
}

function hookGroupsForEvent(config: HookConfig | null | undefined, event: string) {
  const value = config?.[event];
  return Array.isArray(value) ? value : [];
}

function hookHandlerFormFromConfig(
  event: string,
  groupIndex: number,
  handlerIndex: number,
  group: HookMatcherGroup,
  handler: HookHandler,
): HookHandlerFormState {
  return {
    argsText: (handler.args ?? []).join("\n"),
    asyncHook: Boolean(handler.async),
    asyncRewake: Boolean(handler.asyncRewake),
    command: handler.command ?? "",
    enabled: handler.enabled !== false,
    event,
    groupIndex,
    handlerIndex,
    ifFilter: handler.if ?? "",
    inputText:
      typeof handler.input === "undefined" || handler.input === null
        ? ""
        : JSON.stringify(handler.input, null, 2),
    matcher: group.matcher ?? "",
    prompt: handler.prompt ?? "",
    serverId: handler.serverId ?? "",
    shell: handler.shell ?? "",
    statusMessage: handler.statusMessage ?? "",
    timeout: numberInputValue(handler.timeout ?? null),
    toolName: handler.toolName ?? "",
    type: hookHandlerType(handler.type),
    url: handler.url ?? "",
  };
}

function hookHandlerType(type: string): HookHandlerType {
  return type === "http" || type === "mcp_tool" || type === "prompt"
    ? type
    : "command";
}

function upsertHookHandlerInConfig(
  config: HookConfig,
  form: HookHandlerFormState,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const event = form.event;
  const nextHandler = hookHandlerFromForm(form);
  const groups = hookGroupsForEvent(nextConfig, event);
  const existingGroupIndex =
    form.groupIndex !== null && form.event === event ? form.groupIndex : null;
  const groupIndex =
    existingGroupIndex !== null && groups[existingGroupIndex]
      ? existingGroupIndex
      : groups.findIndex((group) => (group.matcher ?? "") === form.matcher);

  if (groupIndex >= 0) {
    const group = groups[groupIndex];
    group.enabled = form.enabled;
    group.matcher = optionalText(form.matcher);
    if (form.handlerIndex !== null && group.hooks[form.handlerIndex]) {
      group.hooks[form.handlerIndex] = nextHandler;
    } else {
      group.hooks = [...group.hooks, nextHandler];
    }
  } else {
    groups.push({
      enabled: form.enabled,
      hooks: [nextHandler],
      matcher: optionalText(form.matcher),
    });
  }

  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function hookHandlerFromForm(form: HookHandlerFormState): HookHandler {
  const timeout = optionalPositiveInteger(form.timeout, "Timeout");
  const input = form.inputText.trim()
    ? parseJsonText(form.inputText, "Input override JSON")
    : null;
  const base: HookHandler = {
    enabled: form.enabled,
    type: form.type,
    async: form.asyncHook,
    asyncRewake: form.asyncRewake,
    if: optionalText(form.ifFilter),
    input,
    statusMessage: optionalText(form.statusMessage),
    timeout,
  };

  if (form.type === "command") {
    return {
      ...base,
      args: form.argsText
        .split(/\r?\n/)
        .map((arg) => arg.trim())
        .filter(Boolean),
      command: form.command.trim(),
      shell: optionalText(form.shell),
    };
  }

  if (form.type === "http") {
    return {
      ...base,
      url: form.url.trim(),
    };
  }

  if (form.type === "mcp_tool") {
    return {
      ...base,
      serverId: form.serverId.trim(),
      toolName: form.toolName.trim(),
    };
  }

  return {
    ...base,
    prompt: form.prompt.trim(),
  };
}

function deleteHookHandlerFromConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event);
  const group = groups[groupIndex];
  if (!group) {
    return nextConfig;
  }

  group.hooks = group.hooks.filter((_, index) => index !== handlerIndex);
  if (!group.hooks.length) {
    groups.splice(groupIndex, 1);
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function updateHookGroupInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  patch: Partial<HookMatcherGroup>,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event);
  if (groups[groupIndex]) {
    groups[groupIndex] = { ...groups[groupIndex], ...patch };
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function updateHookHandlerInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
  patch: Partial<HookHandler>,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event);
  const handler = groups[groupIndex]?.hooks[handlerIndex];
  if (handler) {
    groups[groupIndex].hooks[handlerIndex] = { ...handler, ...patch };
  }
  nextConfig[event] = groups;
  return compactHookConfig(nextConfig);
}

function moveHookGroupInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  direction: -1 | 1,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event);
  const targetIndex = groupIndex + direction;
  if (!groups[groupIndex] || targetIndex < 0 || targetIndex >= groups.length) {
    return nextConfig;
  }
  [groups[groupIndex], groups[targetIndex]] = [
    groups[targetIndex],
    groups[groupIndex],
  ];
  nextConfig[event] = groups;
  return nextConfig;
}

function moveHookHandlerInConfig(
  config: HookConfig,
  event: string,
  groupIndex: number,
  handlerIndex: number,
  direction: -1 | 1,
): HookConfig {
  const nextConfig = cloneHookConfig(config);
  const groups = hookGroupsForEvent(nextConfig, event);
  const handlers = groups[groupIndex]?.hooks;
  const targetIndex = handlerIndex + direction;
  if (!handlers || !handlers[handlerIndex] || targetIndex < 0 || targetIndex >= handlers.length) {
    return nextConfig;
  }
  [handlers[handlerIndex], handlers[targetIndex]] = [
    handlers[targetIndex],
    handlers[handlerIndex],
  ];
  nextConfig[event] = groups;
  return nextConfig;
}

function cloneHookConfig(config: HookConfig): HookConfig {
  return structuredClone(config);
}

function compactHookConfig(config: HookConfig): HookConfig {
  const nextConfig: HookConfig = {
    disableAllHooks: Boolean(config.disableAllHooks),
  };

  for (const [event, value] of Object.entries(config)) {
    if (event === "disableAllHooks" || !Array.isArray(value) || !value.length) {
      continue;
    }
    nextConfig[event] = value;
  }

  return nextConfig;
}

function hookHandlerSummary(handler: HookHandler) {
  return (
    handler.command ||
    handler.url ||
    [handler.serverId, handler.toolName].filter(Boolean).join(" / ") ||
    handler.prompt ||
    ""
  );
}

function hookEventLabel(event: string, t: Translate) {
  const labels: Record<string, string> = {
    SessionStart: "Session start",
    SessionEnd: "Session end",
    UserPromptSubmit: "User prompt submit",
    PreToolUse: "Pre tool use",
    PermissionRequest: "Permission request",
    PermissionDenied: "Permission denied",
    PostToolUse: "Post tool use",
    PostToolUseFailure: "Post tool use failure",
    PostToolBatch: "Post tool batch",
    Stop: "Stop",
    StopFailure: "Stop failure",
    PreCompact: "Pre compact",
    PostCompact: "Post compact",
    Elicitation: "Elicitation",
    ElicitationResult: "Elicitation result",
  };

  return t(labels[event] ?? event);
}

function hookHandlerTypeLabel(type: string, t: Translate) {
  switch (type) {
    case "command":
      return t("Command");
    case "http":
      return t("HTTP");
    case "mcp_tool":
      return t("MCP tool");
    case "prompt":
      return t("Prompt");
    default:
      return type;
  }
}

function hookSourceLabel(source: string, t: Translate) {
  switch (source) {
    case "global":
      return t("Global");
    case "workspace":
      return t("Workspace");
    default:
      return source;
  }
}

function hookRunStatusLabel(status: string, t: Translate) {
  switch (status) {
    case "succeeded":
      return t("succeeded");
    case "failed":
      return t("failed");
    case "error":
      return t("error");
    case "blocked":
      return t("blocked");
    case "running":
      return t("running");
    case "cancelled":
      return t("cancelled");
    default:
      return status;
  }
}

function latestHookRunForSummary(
  hook: EffectiveHookSummary,
  runs: HookRunSummaryRow[],
) {
  return runs.find(
    (run) =>
      run.event === hook.event &&
      run.hookSource === hook.source &&
      run.handlerType === hook.handlerType,
  );
}

function parseJsonText(value: string, label: string): JsonValue {
  const parsed = JSON.parse(value) as unknown;
  if (!isJsonValue(parsed)) {
    throw new Error(`${label} must be JSON-compatible`);
  }
  return parsed;
}

function prettyJsonText(value: string) {
  try {
    return JSON.stringify(JSON.parse(value) as JsonValue, null, 2);
  } catch {
    return value || "{}";
  }
}

function parseDisplayJson(value: string) {
  const normalized = normalizedJsonValue(value);
  if (normalized === value) {
    return null;
  }
  return { pretty: formatJsonValue(normalized) };
}

function jsonSyntaxNodes(value: string) {
  const tokenPattern =
    /("(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"(?=\s*:)|"(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"|true|false|null|-?\d+(?:\.\d*)?(?:[eE][+-]?\d+)?)/g;
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenPattern.exec(value)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(value.slice(lastIndex, match.index));
    }
    const token = match[0];
    nodes.push(
      <span className={jsonTokenClass(token)} key={`${match.index}-${token}`}>
        {token}
      </span>,
    );
    lastIndex = match.index + token.length;
  }

  if (lastIndex < value.length) {
    nodes.push(value.slice(lastIndex));
  }

  return nodes;
}

function jsonTokenClass(token: string) {
  if (token.startsWith('"')) {
    return token.endsWith('":') ? "text-sky-300" : "text-emerald-300";
  }
  if (token === "true" || token === "false") {
    return "text-amber-300";
  }
  if (token === "null") {
    return "text-stone-500";
  }
  return "text-violet-300";
}

function memorySourceRecordsToForm(sources: MemorySourceRecord[]): MemorySourceFormState[] {
  return sources.map((source) => ({
    content: source.content,
    id: source.id,
    metadataText: prettyJsonText(source.metadataJson),
    title: source.title,
  }));
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function defaultProviderKind(providerKinds: SettingsResponse["providerKinds"]) {
  return (
    providerKinds.find((kind) => kind.kind === OPENAI_RESPONSES_PROVIDER_KIND)?.kind ??
    providerKinds[0]?.kind ??
    OPENAI_RESPONSES_PROVIDER_KIND
  );
}

function providerServicesForKinds(
  providerKinds: SettingsResponse["providerKinds"],
): ProviderServicePreset[] {
  const supportedKindIds = new Set(providerKinds.map((kind) => kind.kind));

  return PROVIDER_SERVICE_PRESETS.map((service) => ({
    ...service,
    kindIds: service.kindIds.filter((kindId) => supportedKindIds.has(kindId)),
  })).filter((service) => service.kindIds.length > 0);
}

function providerServiceIdForKind(kindId: string) {
  return PROVIDER_SERVICE_PRESETS.find((service) =>
    service.kindIds.includes(kindId),
  )?.id;
}

function providerDefaultKindForService(
  service: ProviderServicePreset,
  providerKinds: SettingsResponse["providerKinds"],
) {
  const supportedKindIds = new Set(providerKinds.map((kind) => kind.kind));

  if (supportedKindIds.has(service.defaultKindId)) {
    return service.defaultKindId;
  }

  return service.kindIds.find((kindId) => supportedKindIds.has(kindId)) ??
    defaultProviderKind(providerKinds);
}

function providerKindDefaultBaseUrl(
  providerKinds: SettingsResponse["providerKinds"],
  kindId: string,
) {
  return providerKinds.find((kind) => kind.kind === kindId)?.defaultBaseUrl ?? "";
}

function nextProviderId(
  name: string,
  kind: string,
  providers: ConfiguredProviderSummary[],
) {
  const base = slugId(name) || slugId(kind);
  const existingIds = new Set(providers.map((provider) => provider.id));

  if (!existingIds.has(base)) {
    return base;
  }

  let index = 2;
  while (existingIds.has(`${base}-${index}`)) {
    index += 1;
  }

  return `${base}-${index}`;
}

function nextMcpServerId(
  name: string,
  transport: string,
  servers: ConfiguredMcpServerSummary[],
) {
  const base = slugId(name) || slugId(transport);
  const existingIds = new Set(servers.map((server) => server.id));

  if (!existingIds.has(base)) {
    return base;
  }

  let index = 2;
  while (existingIds.has(`${base}-${index}`)) {
    index += 1;
  }

  return `${base}-${index}`;
}

function groupedWorkspaceIds(workspaces: ConfiguredWorkspaceSummary[]) {
  return [
    ...workspaces
      .filter((workspace) => workspace.pinned)
      .map((workspace) => workspace.id),
    ...workspaces
      .filter((workspace) => !workspace.pinned)
      .map((workspace) => workspace.id),
  ];
}

function terminalShellLabel(
  terminalShells: TerminalShellSummary[],
  terminalShell: string,
) {
  return (
    terminalShells.find((shell) => shell.shell === terminalShell)?.label ??
    terminalShell
  );
}


function slugId(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

async function fileToBase64(file: File): Promise<string> {
  return arrayBufferToBase64(await file.arrayBuffer());
}

function arrayBufferToBase64(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";

  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }

  return btoa(binary);
}

function skillScopeLabel(skill: ConfiguredSkillSummary, t: Translate) {
  if (skill.scope === "global") {
    return t("Global skill");
  }

  return skill.workspaceName
    ? t("Workspace skill {name}", { name: skill.workspaceName })
    : t("Workspace skill");
}

const MODEL_INPUT_MODALITY_OPTIONS = ["text", "image", "audio", "video", "pdf"];

const MODEL_OUTPUT_MODALITY_OPTIONS = ["text", "image", "audio", "video"];

type ModelModalityField = "inputModalities" | "outputModalities";

function modelModalityOptions(
  models: ModelMetadataRecord[],
  field: ModelModalityField,
  selected: string[],
) {
  const values = normalizeModalities([
    ...(field === "inputModalities"
      ? MODEL_INPUT_MODALITY_OPTIONS
      : MODEL_OUTPUT_MODALITY_OPTIONS),
    ...models.flatMap((model) => model[field]),
    ...selected,
  ]);

  return values;
}

function modelsForDeveloper(models: ModelMetadataRecord[], developer: string) {
  const normalizedDeveloper = normalizeDeveloperToken(developer);

  if (!normalizedDeveloper) {
    return [];
  }

  return models.filter((model) =>
    normalizeDeveloperToken(model.key).startsWith(`${normalizedDeveloper}/`),
  );
}

function modelIdForDeveloper(model: ModelMetadataRecord, developer: string) {
  return stripDeveloperPrefix(
    normalizeDeveloperToken(model.key).startsWith(`${normalizeDeveloperToken(developer)}/`)
      ? model.key.slice(developer.length + 1)
      : model.modelId,
    developer,
  );
}

function stripDeveloperPrefix(modelId: string, developer: string) {
  const prefix = `${normalizeDeveloperToken(developer)}/`;
  let value = modelId.trim();

  while (normalizeDeveloperToken(value).startsWith(prefix)) {
    value = value.slice(prefix.length);
  }

  return value;
}

function normalizeDeveloperToken(value: string) {
  return value.trim().toLowerCase();
}

function normalizeModalities(modalities: string[]) {
  return modalities
    .map((modality) => modality.trim().toLowerCase())
    .filter(Boolean)
    .filter(uniqueString);
}

function defaultModalities(modalities: string[], fallback = ["text"]) {
  const normalized = normalizeModalities(modalities);
  return normalized.length ? normalized : fallback;
}

function uniqueString(value: string, index: number, values: string[]) {
  return values.indexOf(value) === index;
}

function numberInputValue(value: number | null) {
  return value === null || value === 0 ? "" : String(value);
}

function optionalPositiveInteger(value: string, label: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`${label} must be a positive whole number`);
  }

  const numberValue = Number(trimmed);

  if (!Number.isSafeInteger(numberValue) || numberValue <= 0) {
    throw new Error(`${label} must be a positive whole number`);
  }

  return numberValue;
}

function optionalModelLimit(value: string, label: string, required: boolean) {
  const trimmed = value.trim();

  if (!trimmed || (!required && trimmed === "0")) {
    return null;
  }

  return optionalPositiveInteger(value, label);
}

function outputModalitiesRequireLimits(outputModalities: string[]) {
  return outputModalities.length === 0 || outputModalities.includes("text");
}

function requiredPositiveInteger(value: string, label: string) {
  const numberValue = optionalPositiveInteger(value, label);

  if (numberValue === null) {
    throw new Error(`${label} must be a positive whole number`);
  }

  return numberValue;
}

function memoryDreamJobKey(scope: MemoryDreamScope, workspaceId: string | null) {
  return scope === "global" ? "global" : `workspace:${workspaceId ?? ""}`;
}

function memoryDreamScopeLabel(scope: string, t: Translate) {
  return scope === "global" ? t("Global Dream") : t("Workspace Dream");
}

function memoryDreamTriggerLabel(triggerType: string, t: Translate) {
  if (triggerType === "auto_interval") {
    return t("Auto interval");
  }
  if (triggerType === "auto_threshold") {
    return t("Auto threshold");
  }
  return t("Manual");
}

function memoryDreamStatusLabel(status: string, t: Translate) {
  if (status === "completed") {
    return t("Completed");
  }
  if (status === "failed") {
    return t("Failed");
  }
  if (status === "queued") {
    return t("Queued");
  }
  if (status === "running") {
    return t("Running");
  }
  if (status === "cancelled") {
    return t("Cancelled");
  }
  if (status === "skipped") {
    return t("Skipped");
  }
  return status;
}

function memoryDreamStatusTone(status: string): CapabilityPillTone {
  if (status === "completed") {
    return "success";
  }
  if (status === "failed") {
    return "danger";
  }
  if (status === "queued" || status === "running") {
    return "active";
  }
  return "muted";
}

function memoryDreamChangeOperationLabel(operation: string, t: Translate) {
  const labels: Record<string, string> = {
    add_edge: "Dream change add edge",
    expire: "Dream change expire",
    merge: "Dream change merge",
    promote_to_global: "Dream change promote to global",
    reject: "Dream change reject",
    supersede: "Dream change supersede",
    update: "Dream change update",
  };

  return labels[operation] ? t(labels[operation]) : operation;
}

function memoryDreamChangeStatusLabel(status: string, t: Translate) {
  if (status === "applied") {
    return t("Dream change applied");
  }
  if (status === "failed") {
    return t("Failed");
  }
  return status;
}

function memoryDreamRiskLabel(riskLevel: string, t: Translate) {
  if (riskLevel === "low") {
    return t("Dream risk low");
  }
  if (riskLevel === "medium") {
    return t("Dream risk medium");
  }
  if (riskLevel === "high") {
    return t("Dream risk high");
  }
  return riskLevel;
}

function isActiveMemoryDreamStatus(status: string) {
  return status === "queued" || status === "running";
}

function memoryDreamAppliedChangeCount(job: MemoryDreamJobSummary) {
  return (
    job.changeCounts.added +
    job.changeCounts.updated +
    job.changeCounts.superseded +
    job.changeCounts.expired +
    job.changeCounts.rejected
  );
}

function nextMemoryDreamRunEstimate(
  latestSuccessfulJob: MemoryDreamJobSummary | null,
  dream: MemorySettingsFormState["dream"],
  language: AppLanguageId,
  t: Translate,
) {
  if (!dream.autoEnabled) {
    return t("Auto Dream disabled");
  }

  if (!latestSuccessfulJob) {
    return t("After first eligible scan");
  }

  const completedAt = latestSuccessfulJob.completedAt ?? latestSuccessfulJob.createdAt;
  const completedAtMs = Date.parse(completedAt);
  if (Number.isNaN(completedAtMs)) {
    return t("After next scheduler scan");
  }

  const intervalDays =
    latestSuccessfulJob.scope === "global"
      ? Number(dream.globalIntervalDays)
      : Number(dream.workspaceIntervalDays);
  if (!Number.isFinite(intervalDays) || intervalDays <= 0) {
    return t("After next scheduler scan");
  }

  return formatAuditDate(
    new Date(completedAtMs + intervalDays * 24 * 60 * 60 * 1000).toISOString(),
    language,
  );
}

function groupMemoryDreamChanges(changes: MemoryDreamChangeSummary[]) {
  return changes.reduce<Array<{ operation: string; changes: MemoryDreamChangeSummary[] }>>(
    (groups, change) => {
      const group = groups.find((item) => item.operation === change.operation);
      if (group) {
        group.changes.push(change);
      } else {
        groups.push({ operation: change.operation, changes: [change] });
      }
      return groups;
    },
    [],
  );
}

function memoryDreamJsonText(value: JsonValue | null) {
  return value === null ? "null" : JSON.stringify(value, null, 2);
}

function optionalNumber(value: string, label: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  const numberValue = Number(trimmed);

  if (!Number.isFinite(numberValue)) {
    throw new Error(`${label} must be a number`);
  }

  return numberValue;
}

function compactToolText(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 240 ? `${normalized.slice(0, 237)}...` : normalized;
}

function formatJsonValue(value: JsonValue) {
  return JSON.stringify(normalizedJsonValue(value), null, 2);
}

function normalizedJsonValue(value: JsonValue): JsonValue {
  let current = value;

  for (let index = 0; index < 4; index += 1) {
    if (typeof current !== "string") {
      return current;
    }

    const trimmed = current.trim();
    const looksLikeJson =
      trimmed.startsWith("{") ||
      trimmed.startsWith("[") ||
      trimmed.startsWith('"{') ||
      trimmed.startsWith('"[');
    if (!looksLikeJson) {
      return current;
    }

    try {
      const parsed = JSON.parse(trimmed);
      if (!isJsonValue(parsed)) {
        return current;
      }
      current = parsed;
    } catch {
      return current;
    }
  }

  return current;
}

function positiveIntegerText(value: string, fallback: number) {
  const parsed = Number(value);

  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
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

function formatAuditDate(value: string, language: AppLanguageId = "en") {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    second: "2-digit",
    year: "numeric",
  }).format(date);
}

function formatNumber(value: number, language: AppLanguageId = "en") {
  return new Intl.NumberFormat(language).format(value);
}

function priceText(value: number | null) {
  return value === null ? "n/a" : `$${value}`;
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string"
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  if (isObjectRecord(value)) {
    return Object.values(value).every(isJsonValue);
  }

  return false;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nativePickerRequestInit(nativeBrowserToken: string): RequestInit {
  return {
    body: JSON.stringify({ nativeBrowserToken }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  };
}
