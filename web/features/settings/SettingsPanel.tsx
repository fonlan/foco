import focoLogoSvg from "../../../foco.svg?raw";
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
  Download,
  Eye,
  EyeOff,
  FileText,
  Folder,
  FolderSearch,
  Globe,
  GripVertical,
  Info,
  KeyRound,
  ListChecks,
  LoaderCircle,
  Lock,
  Pencil,
  Play,
  PlugZap,
  Plus,
  RadioTower,
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
  FilePickerMode,
  FilePickerTarget,
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
  MemoryDreamPartialUnavailable,
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
  ModelTestResponse,
  ModelTestState,
  Plan,
  PlanResponse,
  PlansResponse,
  PromptSettingsFormState,
  PromptOverrideFieldState,
  PromptSettingsSummary,
  ProviderFormState,
  ProviderApiKeyResponse,
  ProviderModelsRefreshResponse,
  ProviderModelsResponse,
  ProviderModelRedirect,
  ProviderRequestOverrideFormState,
  ProviderRequestOverrideTarget,
  ProviderRequestOverrideValueType,
  ProviderTestResponse,
  ProviderTestState,
  RemoteAuthMethod,
  RemoteServerDiagnosticResponse,
  RemoteServerHostKeyInfo,
  RemoteServerResponse,
  RemoteServerSummary,
  RemoteServerWorkspaceReference,
  TrustHostKeyResponse,
  DeleteFailedWorkspaceSpecJobResponse,
  RetryWorkspaceSpecJobResponse,
  SettingsResponse,
  SettingsSection,
  SettingsWorkspaceSpecJobSummary,
  SpecSettingsFormState,
  SkillLocationSummary,
  SystemPromptSummary,
  TerminalShellSummary,
  Translate,
  UpdateStatusSummary,
  WebSearchFormState,
  WorkspaceCommonCommandSummary,
  WorkspaceFormState,
  WorkspaceSpecJobSummary,
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
import { installUpdateAndWaitForRestart } from "../../shared/update-install";
import { useI18n } from "../../shared/i18n";
import { fetchSettingsWorkspaceSpecJobsList } from "../../shared/settings-spec-jobs-list";
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
  thinkingLevelOptionsForModel,
} from "../../shared/thinking-levels";
import { findVerticalScrollAncestor } from "../../shared/scroll-forwarding";
import { AgentsSettingsPanel } from "../agents/AgentsSettingsPanel";
import { FilePickerDialog, type FilePickerSelection } from "../file-picker/FilePickerDialog";
import {
  Button,
  Modal,
  SettingsButton,
  SettingsInput,
  SettingsSelect,
  SettingsTextArea,
  SettingsTextField,
  Spinner,
} from "../../shared/ui";
import { useRemoteWorkspaceSkillCatalog } from "./use-remote-workspace-skill-catalog";
import { WorkspaceIcon } from "../workspaces/WorkspaceIcon";
import {
  moveItemId,
  sameStringList,
  workspaceNameFromPath,
} from "../workspaces/workspace-helpers";

type ModelTestToast = {
  kind: "error" | "success";
  message: string;
};

type ProviderModelListState = {
  message: string | null;
  models: string[];
  status: "error" | "loading" | "ok";
};

type UpdateConfirmState = {
  status: UpdateStatusSummary;
  source: "check" | "install";
};

type SettingsFilePickerRequest = {
  initialPath?: string | null;
  mode: FilePickerMode;
  multiple?: boolean;
  target: FilePickerTarget;
  title: string;
  onSelect: (selection: FilePickerSelection[]) => void;
};

const OPENAI_RESPONSES_PROVIDER_KIND = "openai-responses";
const OPENAI_RESPONSES_WEBSOCKET_PROVIDER_KIND = "openai-responses-websocket";

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
    kindIds: [
      OPENAI_RESPONSES_PROVIDER_KIND,
      OPENAI_RESPONSES_WEBSOCKET_PROVIDER_KIND,
      "openai-chat",
    ],
    defaultKindId: OPENAI_RESPONSES_PROVIDER_KIND,
  },
  {
    id: "anthropic",
    label: "Anthropic",
    kindIds: ["anthropic"],
    defaultKindId: "anthropic",
  },
  { id: "gemini", label: "Gemini", kindIds: ["gemini"], defaultKindId: "gemini" },
  { id: "xai", label: "xAI", kindIds: ["xai", "xai-responses"], defaultKindId: "xai" },
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

const MEMORY_DREAM_POLL_INTERVAL_MS = 3000;

const SPEC_JOBS_POLL_INTERVAL_MS = 3000;

const MEMORY_DREAM_MAX_PAGE_SIZE = 200;

const EMPTY_WORKSPACES: ConfiguredWorkspaceSummary[] = [];

type SkillStoreUpdateResponse = {
  results: Array<{
    key: string;
    ok: boolean;
    path: string | null;
    error: string | null;
  }>;
  settings: SettingsResponse;
};

type RemoteServerFormState = {
  id: string;
  name: string;
  hostAlias: string;
  user: string;
  port: string;
  identityFile: string;
  authMethod: RemoteAuthMethod;
  /** Write-only; never populated from API. Empty on edit keeps existing password. */
  password: string;
  passwordConfigured: boolean;
  defaultRemoteRoot: string;
  focoCommand: string;
  terminalShell: string;
  connectTimeoutMs: string;
};

type RemoteServerOperation = "test" | "connect" | "disconnect" | "delete" | "save";

type PendingHostKeyTrust = {
  hostKey: RemoteServerHostKeyInfo;
  operation: "test" | "connect";
  server: RemoteServerSummary;
  /** When set, trust success re-runs save-then-connect for the form (save path). */
  retryAfterSave?: boolean;
};

export function SettingsPanel({
  activeSection,
  activeWorkspaceId,
  agentDefinitionOperationKey,
  agentDefinitions,
  agentDefinitionsError,
  defaultAgentRolePrompts,
  canLogout,
  isLoadingAgentDefinitions,
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
  isLoadingAgentDefinitions: boolean;
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
  const [isRemoteServerDialogOpen, setIsRemoteServerDialogOpen] = useState(false);
  const [remoteServerForm, setRemoteServerForm] = useState<RemoteServerFormState>(() =>
    emptyRemoteServerForm(),
  );
  const [remoteServerOperationKey, setRemoteServerOperationKey] = useState<string | null>(null);
  const [remoteServerReferences, setRemoteServerReferences] = useState<
    RemoteServerWorkspaceReference[]
  >([]);
  const [remoteServerDiagnostics, setRemoteServerDiagnostics] = useState<
    Record<string, RemoteServerDiagnosticResponse["result"]>
  >({});
  const [pendingHostKeyTrust, setPendingHostKeyTrust] = useState<PendingHostKeyTrust | null>(
    null,
  );
  const [isTrustingHostKey, setIsTrustingHostKey] = useState(false);
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
  const [planModeModelId, setPlanModeModelId] = useState("");
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
  const [memoryDreamMeta, setMemoryDreamMeta] = useState<MemoryListMeta>({
    page: 1,
    pageSize: MEMORY_DREAM_DEFAULT_PAGE_SIZE,
    totalCount: 0,
    totalPages: 0,
  });
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
  const [memoryDreamDetailJobSnapshot, setMemoryDreamDetailJobSnapshot] =
    useState<MemoryDreamJobSummary | null>(null);
  const [memoryDreamError, setMemoryDreamError] = useState<string | null>(null);
  const [memoryDreamPartialUnavailable, setMemoryDreamPartialUnavailable] = useState<
    MemoryDreamPartialUnavailable[]
  >([]);
  const [selectedMemoryId, setSelectedMemoryId] = useState<string | null>(null);
  const [memorySources, setMemorySources] = useState<MemorySourceRecord[]>([]);
  const [workspaceForm, setWorkspaceForm] = useState<WorkspaceFormState>(() =>
    emptyWorkspaceForm(),
  );
  const [settingsFilePickerRequest, setSettingsFilePickerRequest] =
    useState<SettingsFilePickerRequest | null>(null);
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
  const [enabledSkillIds, setEnabledSkillIds] = useState<Set<string> | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isSavingUpdateSettings, setIsSavingUpdateSettings] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateConfirm, setUpdateConfirm] = useState<UpdateConfirmState | null>(null);
  const [isSavingGeneral, setIsSavingGeneral] = useState(false);
  const [isSavingWebSearch, setIsSavingWebSearch] = useState(false);
  const [isSavingPromptSettings, setIsSavingPromptSettings] = useState(false);
  const [isSavingSpecSettings, setIsSavingSpecSettings] = useState(false);
  const [specSettingsSaveError, setSpecSettingsSaveError] = useState<string | null>(
    null,
  );
  const confirmedSpecSettingsRef = useRef<SpecSettingsFormState>(emptySpecSettingsForm());
  const pendingSpecSettingsSaveRef = useRef<SpecSettingsFormState | null>(null);
  const isSpecSettingsSaveInFlightRef = useRef(false);
  const specSettingsFormRef = useRef<SpecSettingsFormState>(emptySpecSettingsForm());
  const specSettingsMutationGenerationRef = useRef(0);
  const [specJobs, setSpecJobs] = useState<SettingsWorkspaceSpecJobSummary[]>([]);
  const [specJobsPage, setSpecJobsPage] = useState(1);
  const [specJobsPageSize, setSpecJobsPageSize] = useState(20);
  const [specJobsTotalCount, setSpecJobsTotalCount] = useState(0);
  const [specJobsTotalPages, setSpecJobsTotalPages] = useState(0);
  const [showRetryableSpecJobsOnly, setShowRetryableSpecJobsOnly] = useState(true);
  const [isLoadingSpecJobs, setIsLoadingSpecJobs] = useState(false);
  const [specJobsError, setSpecJobsError] = useState<string | null>(null);
  const [specJobOperations, setSpecJobOperations] = useState<
    Partial<Record<string, "retry" | "delete">>
  >({});
  const specJobOperationKeysRef = useRef<Set<string>>(new Set());
  const specJobsRequestGenerationRef = useRef(0);
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
  const [pendingMemoryEnabledIds, setPendingMemoryEnabledIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [memoryDreamRunKey, setMemoryDreamRunKey] = useState<string | null>(null);
  const [isClearingPassword, setIsClearingPassword] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingTheme, setIsSavingTheme] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [pendingDeleteWorkspace, setPendingDeleteWorkspace] =
    useState<ConfiguredWorkspaceSummary | null>(null);
  const [deletingWorkspaceId, setDeletingWorkspaceId] = useState<string | null>(null);
  const [isSavingWorkspaceOrder, setIsSavingWorkspaceOrder] = useState(false);
  const [isSavingWorkspaceLogo, setIsSavingWorkspaceLogo] = useState(false);
  const [isSelectingWorkspaceFormPath, setIsSelectingWorkspaceFormPath] =
    useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [providerOperationIds, setProviderOperationIds] = useState<Set<string>>(
    () => new Set(),
  );
  const providerOperationIdsRef = useRef<Set<string>>(new Set());
  const [isRevealingProviderApiKey, setIsRevealingProviderApiKey] = useState(false);
  const [isRefreshingProviderModels, setIsRefreshingProviderModels] =
    useState(false);
  const [isSavingMcpServer, setIsSavingMcpServer] = useState(false);
  const [isSavingSkills, setIsSavingSkills] = useState(false);
  const [isRefreshingSkills, setIsRefreshingSkills] = useState(false);
  const [updatingSkillKey, setUpdatingSkillKey] = useState<string | null>(null);
  const [isUpdatingAllSkills, setIsUpdatingAllSkills] = useState(false);
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
  const [modelTests, setModelTests] = useState<Record<string, ModelTestState>>({});
  const [modelTestToast, setModelTestToast] = useState<ModelTestToast | null>(null);
  const modelTestsInFlightRef = useRef<Set<string>>(new Set());
  const [expandedProviderIds, setExpandedProviderIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [providerModelLists, setProviderModelLists] = useState<
    Record<string, ProviderModelListState>
  >({});
  const loadingProviderModelIdsRef = useRef<Set<string>>(new Set());
  const settingsLoadRequestIdRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [isGeneralPasswordVisible, setIsGeneralPasswordVisible] = useState(false);
  const [isProviderApiKeyVisible, setIsProviderApiKeyVisible] = useState(false);
  const [isEditingGeneralPassword, setIsEditingGeneralPassword] = useState(false);

  useEffect(() => {
    if (!modelTestToast) {
      return;
    }

    const timeoutId = window.setTimeout(() => setModelTestToast(null), 6000);
    return () => window.clearTimeout(timeoutId);
  }, [modelTestToast]);

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
  const providerModelOptions = useMemo(
    () => loadedProviderModelIds(providerModelLists),
    [providerModelLists],
  );
  const modelIdOptions = useMemo(
    () => [
      ...developerModelOptions.map((model) => ({
        key: `metadata:${model.key}`,
        value: model.value,
      })),
      ...providerModelOptions.map((modelId) => ({
        key: `provider:${modelId}`,
        value: modelId,
      })),
    ].filter(uniqueByValue),
    [developerModelOptions, providerModelOptions],
  );
  const modelOutputsText = form.outputModalities.includes("text");
  const enabledNeedsLimits =
    form.enabled &&
    modelOutputsText &&
    (!form.contextWindow.trim() || !form.maxOutputTokens.trim());
  const providerKinds = settings?.providerKinds ?? [];
  const providers = settings?.providers ?? [];
  const remoteServers = settings?.remoteServers ?? [];
  // Stable empty fallback: a fresh `[]` each render re-triggers remote skill
  // catalog effects while settings are still loading.
  const workspaces = settings?.workspaces ?? EMPTY_WORKSPACES;
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
  const currentMemoryDreamPage = memoryDreamMeta.page;
  const memoryDreamTotalPages = memoryDreamMeta.totalPages;
  const memoryDreamPaginationItems = auditPaginationItems(
    currentMemoryDreamPage,
    memoryDreamTotalPages,
  );
  const memoryDreamPageStart = memoryDreamJobs.length
    ? (currentMemoryDreamPage - 1) * memoryDreamMeta.pageSize + 1
    : 0;
  const memoryDreamPageEnd = memoryDreamJobs.length
    ? Math.min(
      memoryDreamMeta.totalCount,
      memoryDreamPageStart + memoryDreamJobs.length - 1,
    )
    : 0;
  const specJobsPaginationItems = auditPaginationItems(
    specJobsPage,
    specJobsTotalPages,
  );
  const specJobsPageStart = specJobs.length
    ? (specJobsPage - 1) * specJobsPageSize + 1
    : 0;
  const specJobsPageEnd = specJobs.length
    ? Math.min(specJobsTotalCount, specJobsPageStart + specJobs.length - 1)
    : 0;
  const hasActiveSpecJobs = specJobs.some(
    (item) => item.job.status === "queued" || item.job.status === "running",
  );
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
  const memoryDreamDetailJob = memoryDreamDetailJobId
    ? memoryDreamJobs.find((job) => job.id === memoryDreamDetailJobId) ??
      memoryDreamDetailJobSnapshot
    : null;
  const activeMemoryDreamJobKeys = useMemo(
    () =>
      new Set(
        memoryDreamJobs
          .filter((job) => isActiveMemoryDreamStatus(job.status))
          .map((job) => memoryDreamJobKey(job.scope, job.workspaceId)),
      ),
    [memoryDreamJobs],
  );
  const globalDreamRunKey = memoryDreamJobKey("global", null);
  const workspaceDreamRunKey = memoryDreamJobKey(
    "workspace",
    memoryDreamWorkspaceId,
  );
  const latestSuccessfulMemoryDreamJob =
    memoryDreamJobs.find((job) => job.status === "completed") ?? null;
  const latestFailedMemoryDreamJob =
    memoryDreamJobs.find((job) => job.status === "failed") ?? null;
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
  const {
    catalogs: remoteWorkspaceSkillCatalogs,
    reload: reloadRemoteWorkspaceSkillCatalogs,
    retryWorkspace: retryRemoteWorkspaceSkillCatalog,
  } = useRemoteWorkspaceSkillCatalog(activeSection === "skills", workspaces);
  const skillLocations: SkillLocationSummary[] =
    skills?.locations ??
    (skills?.directories ?? []).map((path) => ({
      enabled: true,
      id: path,
      path,
    }));
  const currentEnabledSkillIds =
    enabledSkillIds ??
    new Set((skills?.detected ?? []).filter((skill) => skill.enabled).map((skill) => skill.key));
  const updateableStoreSkills = (skills?.detected ?? []).filter((skill) =>
    Boolean(skill.store?.updateable),
  );
  const detectedSkillRows = useMemo(
    () => [
      ...(skills?.detected ?? []).map((skill) => ({
        key: `local:${skill.key}`,
        skill,
        source: "local" as const,
        workspace: null,
      })),
      ...remoteWorkspaceSkillCatalogs.flatMap((catalog) =>
        catalog.skills.map((skill) => ({
          key: `remote:${catalog.workspace.id}:${skill.key}`,
          skill,
          source: "remote" as const,
          workspace: catalog.workspace,
        })),
      ),
    ],
    [remoteWorkspaceSkillCatalogs, skills?.detected],
  );
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
  const enabledConfiguredModels = useMemo(
    () => configuredModelsByName.filter((model) => model.enabled),
    [configuredModelsByName],
  );
  const specEligibleGenerationModels = useMemo(
    () =>
      configuredModelsByName.filter((model) =>
        isSpecEligibleGenerationModel(model, providers),
      ),
    [configuredModelsByName, providers],
  );
  const selectedSpecGenerationModel =
    configuredModels.find(
      (model) => model.id === specSettingsForm.generationModelId,
    ) ?? null;
  const isSelectedSpecGenerationModelUnavailable = Boolean(
    specSettingsForm.generationModelId &&
      !specEligibleGenerationModels.some(
        (model) => model.id === specSettingsForm.generationModelId,
      ),
  );
  const passwordInputValue =
    generalForm.password ||
    (settings?.general.webServer.passwordEnabled && !isEditingGeneralPassword
      ? SAVED_PASSWORD_MASK
      : "");

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
  const modelThinkingOptions = useMemo(
    () => thinkingLevelOptionsForModel(selectedMetadata ?? editingModel, thinkingLevels),
    [editingModel, selectedMetadata, thinkingLevels],
  );
  const modelThinkingEnabled = modelThinkingOptions.length > 0;
  const editingWorkspace =
    workspaces.find((workspace) => workspace.id === workspaceForm.id) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const providerUsesWebsocket = selectedProviderKind?.usesWebsocket === true;
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
  const listSystemPrompts = ordinarySystemPrompts(systemPrompts);
  const planModeSystemPrompt =
    systemPrompts.find((prompt) => prompt.name === PLAN_MODE_SYSTEM_PROMPT_NAME) ??
    null;
  const reviewSystemPrompt =
    systemPrompts.find((prompt) => prompt.name === REVIEW_SYSTEM_PROMPT_NAME) ??
    null;
  const savedSystemPrompts = settings
    ? normalizedSystemPromptSummaries(settings.prompts)
    : systemPrompts;
  const activeSystemPrompt =
    listSystemPrompts.find(
      (prompt) => prompt.name === promptSettingsForm.activeSystemPromptName,
    ) ??
    listSystemPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME) ??
    listSystemPrompts[0] ??
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
      chatTitleGenerationModelId:
        data.general.chatTitleGenerationModelId ?? "current_chat_model",
      hookAuditEnabled: data.general.hookAuditEnabled,
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
      llmRequestRetryCount: String(data.general.llmRequestRetryCount),
      password: "",
      runtimeToolStateCompressionEnabled:
        data.general.runtimeToolStateCompressionEnabled ?? false,
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
    const listPrompts = ordinarySystemPrompts(systemPrompts);
    setPromptSettingsForm({
      activeSystemPromptName:
        listPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)
          ?.name ??
        listPrompts[0]?.name ??
        DEFAULT_SYSTEM_PROMPT_NAME,
      contextCompression: promptOverrideFromStored(
        data.prompts.contextCompressionSystemPrompt,
        data.prompts.defaultContextCompressionSystemPrompt,
      ),
      generationSystemPrompt: promptOverrideFromStored(
        data.spec.generationSystemPrompt,
        data.spec.defaultGenerationSystemPrompt,
      ),
      updateSystemPrompt: promptOverrideFromStored(
        data.spec.updateSystemPrompt,
        data.spec.defaultUpdateSystemPrompt,
      ),
      memoryRetrieval: promptOverrideFromStored(
        data.prompts.memoryRetrievalSystemPrompt,
        data.prompts.defaultMemoryRetrievalSystemPrompt,
      ),
      memoryExtraction: promptOverrideFromStored(
        data.prompts.memoryExtractionSystemPrompt,
        data.prompts.defaultMemoryExtractionSystemPrompt,
      ),
      memoryDream: promptOverrideFromStored(
        data.prompts.memoryDreamSystemPrompt,
        data.prompts.defaultMemoryDreamSystemPrompt,
      ),
      extraText: data.prompts.extraText,
      files: data.prompts.files,
      pendingFile: "",
      pendingSystemPromptName: "",
      pendingSystemPromptRename: "",
      renamingSystemPromptName: null,
      systemPrompts,
    });
  }

  function applySpecSettingsForm(nextForm: SpecSettingsFormState) {
    specSettingsFormRef.current = nextForm;
    setSpecSettingsForm(nextForm);
  }

  function hasSpecSettingsLocalIntent(): boolean {
    return (
      isSpecSettingsSaveInFlightRef.current ||
      pendingSpecSettingsSaveRef.current !== null ||
      !specSettingsFormsEqual(
        specSettingsFormRef.current,
        confirmedSpecSettingsRef.current,
      )
    );
  }

  function syncSpecSettingsForm(data: SettingsResponse) {
    const nextForm = specSettingsFormFromResponse(data);
    confirmedSpecSettingsRef.current = nextForm;
    applySpecSettingsForm(nextForm);
  }

  function syncPlanSettingsForm(data: SettingsResponse) {
    setPlanMergeAutomationMode(
      data.plan.mergeAutomationMode || "isolated_auto_once",
    );
    setPlanModeModelId(data.plan.modeModelId ?? "");
  }

  function syncMemorySettingsForm(data: SettingsResponse) {
    setMemorySettingsForm({
      enabled: data.memory.enabled,
      extractionMode: data.memory.extractionMode,
      retrievalMode: data.memory.retrievalMode,
      extractionModelId: data.memory.extractionModelId ?? "",
      retrievalModelId: data.memory.retrievalModelId ?? "",
      extractionLlmTimeoutMs: String(data.memory.extractionLlmTimeoutMs),
      retrievalLlmTimeoutMs: String(data.memory.retrievalLlmTimeoutMs),
      contextBudgetPercent: String(data.memory.contextBudgetPercent),
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
        llmTimeoutMs: String(data.memory.dream.llmTimeoutMs),
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
    const requestId = settingsLoadRequestIdRef.current + 1;
    settingsLoadRequestIdRef.current = requestId;
    const specMutationGenerationAtStart = specSettingsMutationGenerationRef.current;
    setIsLoadingSettings(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/settings");
      if (requestId !== settingsLoadRequestIdRef.current) {
        return;
      }
      setSettings(data);
      onSettingsChange(data);
      setHookWorkspaceId((current) => current || data.workspaces[0]?.id || "");
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
      syncGeneralForm(data);
      syncWebSearchForm(data);
      syncPromptSettingsForm(data);
      // Skip Spec form when the user has local edits / in-flight saves, or when a
      // Spec mutation happened after this GET started (stale snapshot).
      if (
        specSettingsMutationGenerationRef.current ===
          specMutationGenerationAtStart &&
        !hasSpecSettingsLocalIntent()
      ) {
        syncSpecSettingsForm(data);
      }
      syncPlanSettingsForm(data);
      syncMemorySettingsForm(data);
      syncSkillsForm(data);
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
    } catch (requestError) {
      if (requestId === settingsLoadRequestIdRef.current) {
        setError(errorMessage(requestError));
      }
    } finally {
      if (requestId === settingsLoadRequestIdRef.current) {
        setIsLoadingSettings(false);
      }
    }
  }, [onSettingsChange]);

  // Seed prompt override editors from loaded settings (system prompt list can fall
  // back to settings while override fields live only in form state).
  useEffect(() => {
    if (!settings) {
      return;
    }
    setPromptSettingsForm((current) => {
      const needsSeed =
        (!current.contextCompression.value &&
          !current.contextCompression.custom) ||
        (!current.generationSystemPrompt.value &&
          !current.generationSystemPrompt.custom) ||
        (!current.updateSystemPrompt.value && !current.updateSystemPrompt.custom) ||
        (!current.memoryRetrieval.value && !current.memoryRetrieval.custom) ||
        (!current.memoryExtraction.value && !current.memoryExtraction.custom) ||
        (!current.memoryDream.value && !current.memoryDream.custom) ||
        current.systemPrompts.length === 0;
      if (!needsSeed) {
        return current;
      }
      const systemPrompts =
        current.systemPrompts.length > 0
          ? current.systemPrompts
          : normalizedSystemPromptSummaries(settings.prompts);
      const listPrompts = ordinarySystemPrompts(systemPrompts);
      return {
        ...current,
        activeSystemPromptName: listPrompts.some(
          (prompt) => prompt.name === current.activeSystemPromptName,
        )
          ? current.activeSystemPromptName
          : (listPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)
              ?.name ??
            listPrompts[0]?.name ??
            DEFAULT_SYSTEM_PROMPT_NAME),
        contextCompression:
          current.contextCompression.value || current.contextCompression.custom
            ? current.contextCompression
            : promptOverrideFromStored(
                settings.prompts.contextCompressionSystemPrompt,
                settings.prompts.defaultContextCompressionSystemPrompt,
              ),
        generationSystemPrompt:
          current.generationSystemPrompt.value || current.generationSystemPrompt.custom
            ? current.generationSystemPrompt
            : promptOverrideFromStored(
                settings.spec.generationSystemPrompt,
                settings.spec.defaultGenerationSystemPrompt,
              ),
        updateSystemPrompt:
          current.updateSystemPrompt.value || current.updateSystemPrompt.custom
            ? current.updateSystemPrompt
            : promptOverrideFromStored(
                settings.spec.updateSystemPrompt,
                settings.spec.defaultUpdateSystemPrompt,
              ),
        memoryRetrieval:
          current.memoryRetrieval.value || current.memoryRetrieval.custom
            ? current.memoryRetrieval
            : promptOverrideFromStored(
                settings.prompts.memoryRetrievalSystemPrompt,
                settings.prompts.defaultMemoryRetrievalSystemPrompt,
              ),
        memoryExtraction:
          current.memoryExtraction.value || current.memoryExtraction.custom
            ? current.memoryExtraction
            : promptOverrideFromStored(
                settings.prompts.memoryExtractionSystemPrompt,
                settings.prompts.defaultMemoryExtractionSystemPrompt,
              ),
        memoryDream:
          current.memoryDream.value || current.memoryDream.custom
            ? current.memoryDream
            : promptOverrideFromStored(
                settings.prompts.memoryDreamSystemPrompt,
                settings.prompts.defaultMemoryDreamSystemPrompt,
              ),
        systemPrompts,
        extraText: current.extraText || settings.prompts.extraText,
        files: current.files.length ? current.files : settings.prompts.files,
      };
    });
  }, [settings]);

  // Keep ordinary system-prompt selection valid after Plan/Review leave the list.
  useEffect(() => {
    if (!settings) {
      return;
    }
    setPromptSettingsForm((current) => {
      const listPrompts = ordinarySystemPrompts(
        current.systemPrompts.length
          ? current.systemPrompts
          : normalizedSystemPromptSummaries(settings.prompts),
      );
      if (
        listPrompts.some((prompt) => prompt.name === current.activeSystemPromptName)
      ) {
        return current;
      }
      const nextActive =
        listPrompts.find((prompt) => prompt.name === DEFAULT_SYSTEM_PROMPT_NAME)
          ?.name ??
        listPrompts[0]?.name ??
        DEFAULT_SYSTEM_PROMPT_NAME;
      if (nextActive === current.activeSystemPromptName) {
        return current;
      }
      return {
        ...current,
        activeSystemPromptName: nextActive,
      };
    });
  }, [promptSettingsForm.activeSystemPromptName, promptSettingsForm.systemPrompts, settings]);

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

  const loadMemoryDreamJobs = useCallback(async (pageOverride?: number) => {
    const requestedPage = pageOverride ?? memoryDreamPage;
    setIsLoadingMemoryDreamJobs(true);
    setMemoryDreamError(null);
    setMemoryDreamPartialUnavailable([]);

    try {
      const params = new URLSearchParams({
        page: String(requestedPage),
        pageSize: String(memoryDreamPageSize),
      });
      const data = await requestJson<MemoryDreamJobsResponse>(
        `/api/memory/dream/jobs?${params.toString()}`,
      );
      setMemoryDreamJobs(data.jobs);
      setMemoryDreamMeta({
        page: data.page,
        pageSize: data.pageSize,
        totalCount: data.totalCount,
        totalPages: data.totalPages,
      });
      setMemoryDreamPage(data.page);
      setMemoryDreamPageSize(data.pageSize);
      setMemoryDreamPartialUnavailable(data.partialUnavailable ?? []);
      setMemoryDreamDetailJobSnapshot((current) => {
        if (!current) {
          return current;
        }
        return data.jobs.find((job) => job.id === current.id) ?? current;
      });
    } catch (requestError) {
      setMemoryDreamJobs([]);
      setMemoryDreamMeta({
        page: requestedPage,
        pageSize: memoryDreamPageSize,
        totalCount: 0,
        totalPages: 0,
      });
      setMemoryDreamDetailJobId(null);
      setMemoryDreamDetailJobSnapshot(null);
      setMemoryDreamPartialUnavailable([]);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamJobs(false);
    }
  }, [memoryDreamPage, memoryDreamPageSize]);


  const loadMemoryDreamChanges = useCallback(async (job: MemoryDreamJobSummary) => {
    setIsLoadingMemoryDreamChanges(true);
    setMemoryDreamError(null);

    try {
      const params = new URLSearchParams();
      if (job.workspaceId) {
        params.set("workspaceId", job.workspaceId);
      }
      const query = params.size > 0 ? `?${params.toString()}` : "";
      const data = await requestJson<MemoryDreamChangesResponse>(
        `/api/memory/dream/jobs/${encodeURIComponent(job.id)}/changes${query}`,
      );
      setMemoryDreamChanges(data.changes);
    } catch (requestError) {
      setMemoryDreamChanges([]);
      setMemoryDreamError(errorMessage(requestError));
    } finally {
      setIsLoadingMemoryDreamChanges(false);
    }
  }, []);

  const showOptimisticMemoryDreamJob = useCallback((job: MemoryDreamJobSummary) => {
    setMemoryDreamPage(1);
    setMemoryDreamJobs((current) =>
      [job, ...current.filter((item) => item.id !== job.id)].slice(
        0,
        memoryDreamPageSize,
      ),
    );
    setMemoryDreamMeta((current) => {
      const totalCount = Math.max(current.totalCount + 1, 1);
      return {
        page: 1,
        pageSize: memoryDreamPageSize,
        totalCount,
        totalPages: Math.max(1, Math.ceil(totalCount / memoryDreamPageSize)),
      };
    });
    setMemoryDreamDetailJobSnapshot((current) =>
      current?.id === job.id ? job : current,
    );
  }, [memoryDreamPageSize]);

  const loadSpecJobs = useCallback(async () => {
    const requestId = ++specJobsRequestGenerationRef.current;
    setIsLoadingSpecJobs(true);
    setSpecJobsError(null);

    try {
      const data = await fetchSettingsWorkspaceSpecJobsList({
        page: specJobsPage,
        pageSize: specJobsPageSize,
        retryableOnly: showRetryableSpecJobsOnly,
      });
      // Ignore stale responses when page/filter changed mid-flight.
      if (specJobsRequestGenerationRef.current !== requestId) {
        return;
      }
      if (data.totalPages > 0 && data.page > data.totalPages) {
        setSpecJobsPage(data.totalPages);
        return;
      }
      if (data.totalPages === 0 && data.page !== 1) {
        setSpecJobsPage(1);
        return;
      }
      setSpecJobs(data.jobs);
      setSpecJobsPage(data.page);
      setSpecJobsPageSize(data.pageSize);
      setSpecJobsTotalCount(data.totalCount);
      setSpecJobsTotalPages(data.totalPages);
      if (data.errors.length > 0) {
        setSpecJobsError(
          data.errors.map((item) => `${item.workspaceName}: ${item.error}`).join("; "),
        );
      }
    } catch (requestError) {
      if (specJobsRequestGenerationRef.current !== requestId) {
        return;
      }
      setSpecJobs([]);
      setSpecJobsTotalCount(0);
      setSpecJobsTotalPages(0);
      setSpecJobsError(errorMessage(requestError));
    } finally {
      if (specJobsRequestGenerationRef.current === requestId) {
        setIsLoadingSpecJobs(false);
      }
    }
  }, [showRetryableSpecJobsOnly, specJobsPage, specJobsPageSize]);

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
    setMemoryDreamDetailJobSnapshot(null);
    setMemoryDreamChanges([]);
  }

  useEffect(() => {
    void loadMetadata();
    void loadSettings();
  }, [loadMetadata, loadSettings]);

  useEffect(() => {
    if (activeSection === "hooks" && hookWorkspaceId) {
      void loadHooks(hookWorkspaceId);
    }
  }, [activeSection, hookWorkspaceId, loadHooks]);
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
    if (activeSection !== "memory" || activeMemoryDreamJobKeys.size === 0) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadMemoryDreamJobs();
    }, MEMORY_DREAM_POLL_INTERVAL_MS);

    return () => window.clearInterval(intervalId);
  }, [activeMemoryDreamJobKeys.size, activeSection, loadMemoryDreamJobs]);

  useEffect(() => {
    if (activeSection === "spec") {
      void loadSpecJobs();
    }
  }, [activeSection, loadSpecJobs]);

  useEffect(() => {
    if (activeSection !== "spec" || !hasActiveSpecJobs) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadSpecJobs();
    }, SPEC_JOBS_POLL_INTERVAL_MS);

    return () => window.clearInterval(intervalId);
  }, [activeSection, hasActiveSpecJobs, loadSpecJobs]);

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

    const detailJob =
      memoryDreamJobs.find((job) => job.id === memoryDreamDetailJobId) ??
      memoryDreamDetailJobSnapshot;
    if (!detailJob) {
      closeMemoryDreamDetailDialog();
      return;
    }

    void loadMemoryDreamChanges(detailJob);
  }, [
    activeSection,
    loadMemoryDreamChanges,
    memoryDreamDetailJobId,
    memoryDreamJobs,
    memoryDreamDetailJobSnapshot,
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

  useEffect(() => {
    if (!isModelDialogOpen || editingModel || !form.modelId || !selectedMetadata) {
      return;
    }

    const matchedProviderIds = providers
      .filter((provider) => providerModelListMatches(provider.id, form.modelId))
      .map((provider) => provider.id);
    if (!matchedProviderIds.length) {
      return;
    }

    setForm((current) => {
      const configuredProviderIds = new Set(providers.map((provider) => provider.id));
      if (current.providerIds.some((providerId) => configuredProviderIds.has(providerId))) {
        return current;
      }
      const activeProviderId = matchedProviderIds.includes(current.activeProviderId)
        ? current.activeProviderId
        : matchedProviderIds[0] ?? "";
      return { ...current, providerIds: matchedProviderIds, activeProviderId };
    });
  }, [
    editingModel,
    form.modelId,
    isModelDialogOpen,
    providerModelLists,
    providers,
    selectedMetadata,
  ]);

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
      thinkingLevel: model.supportedThinkingLevels[0] ?? "",
      webSearchMode: current.webSearchMode || "auto",
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
    const isProviderRawModelId = providerModelOptions.includes(modelId) && modelId.includes("/");
    const model = isProviderRawModelId ? null : modelMetadataForInput(modelId);
    setSelectedMetadataKey(model?.key ?? "");
    if (model) {
      setSelectedModelDeveloper(model.providerId);
    }

    setForm((current) => {
      if (!model) {
        const providerIds = matchedProviderIdsForModel(modelId);
        const nextProviderIds = providerIds.length ? providerIds : current.providerIds;
        const activeProviderId = nextProviderIds.includes(current.activeProviderId)
          ? current.activeProviderId
          : nextProviderIds[0] ?? "";

        return {
          ...current,
          modelId,
          providerIds: nextProviderIds,
          activeProviderId,
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
      thinkingLevel: defaultThinkingLevelForModel(model),
      webSearchMode: model.webSearchMode ?? "auto",
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
      modelRedirects: (provider.modelRedirects ?? []).map((redirect) => ({
        from: redirect.from,
        to: redirect.to,
      })),
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
    setIsRevealingProviderApiKey(false);
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
    setIsRevealingProviderApiKey(false);
    setIsProviderDialogOpen(true);
  }

  function closeProviderDialog() {
    setIsProviderApiKeyVisible(false);
    setIsRevealingProviderApiKey(false);
    setIsProviderDialogOpen(false);
  }

  async function handleToggleProviderApiKeyVisibility() {
    if (isRevealingProviderApiKey) {
      return;
    }

    if (isProviderApiKeyVisible) {
      setIsProviderApiKeyVisible(false);
      return;
    }

    if (providerForm.apiKey || providerForm.clearApiKey || !hasSavedProviderKey || !editingProvider) {
      setIsProviderApiKeyVisible(true);
      return;
    }

    setIsRevealingProviderApiKey(true);
    setError(null);
    const providerId = editingProvider.id;

    try {
      const data = await requestJson<ProviderApiKeyResponse>(
        "/api/providers/reveal-api-key",
        {
          body: JSON.stringify({ id: providerId }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setProviderForm((current) =>
        current.id === providerId
          ? { ...current, apiKey: data.apiKey, clearApiKey: false }
          : current,
      );
      setIsProviderApiKeyVisible(true);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRevealingProviderApiKey(false);
    }
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
    const nextKind = providerKinds.find((item) => item.kind === kind);
    setProviderForm((current) => ({
      ...current,
      kind,
      serviceId: providerServiceIdForKind(kind) || current.serviceId,
      // First-release WebSocket transport does not support API proxy tunneling.
      apiProxyEnabled: nextKind?.usesWebsocket ? false : current.apiProxyEnabled,
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
      executionHost: server.executionHost,
      url: server.url ?? "",
    });
    setIsMcpDialogOpen(true);
  }

  function editConfiguredRemoteServer(server: RemoteServerSummary) {
    setRemoteServerReferences([]);
    setRemoteServerForm(remoteServerFormFromSummary(server));
    setIsRemoteServerDialogOpen(true);
  }

  function startAddingRemoteServer() {
    setRemoteServerReferences([]);
    setRemoteServerForm(emptyRemoteServerForm());
    setIsRemoteServerDialogOpen(true);
  }

  function closeRemoteServerDialog() {
    setIsRemoteServerDialogOpen(false);
    setRemoteServerReferences([]);
  }

  function selectIdentityFile() {
    const current = remoteServerForm.identityFile.trim();
    setSettingsFilePickerRequest({
      initialPath: parentDirectoryPath(current),
      mode: "file",
      multiple: false,
      target: { kind: "local" },
      title: t("Select private key file"),
      onSelect: (selection) => {
        const path = selection[0]?.path;
        if (!path) {
          return;
        }
        setRemoteServerForm((form) => ({ ...form, identityFile: path }));
      },
    });
  }

  function hostKeyPromptFromDiagnostic(
    server: RemoteServerSummary,
    result: RemoteServerDiagnosticResponse["result"],
    operation: "test" | "connect",
    retryAfterSave = false,
  ): boolean {
    if (result.ok) {
      return false;
    }
    if (result.errorKind === "host_key_changed") {
      setError(
        t("Host key changed — manual known_hosts fix required") +
          " " +
          t(
            "This host presented a different key than the one stored in known_hosts. Foco will not overwrite it. Remove or update the entry in your known_hosts file, then try again.",
          ),
      );
      return true;
    }
    const hostKey = result.hostKey;
    const verificationRequired =
      result.hostKeyVerificationRequired === true ||
      (result.errorKind === "host_key_unknown" && Boolean(hostKey));
    if (verificationRequired && hostKey) {
      setPendingHostKeyTrust({
        hostKey,
        operation,
        retryAfterSave,
        server,
      });
      setError(null);
      return true;
    }
    return false;
  }

  async function confirmHostKeyTrust() {
    if (!pendingHostKeyTrust) {
      return;
    }
    const pending = pendingHostKeyTrust;
    setIsTrustingHostKey(true);
    setError(null);
    try {
      await requestJson<TrustHostKeyResponse>(
        `/api/remote-servers/${encodeURIComponent(pending.server.id)}/trust-host-key`,
        {
          body: JSON.stringify({
            fingerprintSha256: pending.hostKey.fingerprintSha256,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setPendingHostKeyTrust(null);
      const retryOk = await runRemoteServerOperation(pending.server, pending.operation);
      // Only close the editor when the post-save connect actually succeeded.
      if (pending.retryAfterSave && retryOk) {
        setIsRemoteServerDialogOpen(false);
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsTrustingHostKey(false);
    }
  }

  function cancelHostKeyTrust() {
    setPendingHostKeyTrust(null);
  }

  async function editConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setError(null);
    setWorkspaceForm({
      commonCommands: workspace.commonCommands.map((command) => ({ ...command })),
      id: workspace.id,
      name: workspace.name,
      path: workspace.serverId ? (workspace.remotePath ?? workspace.path) : workspace.path,
      remotePath: workspace.remotePath ?? null,
      serverId: workspace.serverId ?? null,
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

  function applyUpdateStatus(update: UpdateStatusSummary) {
    if (!settings) {
      return;
    }
    const next = { ...settings, update };
    setSettings(next);
    onSettingsChange(next);
  }

  async function checkForUpdate() {
    setIsCheckingUpdate(true);
    setError(null);
    try {
      const update = await requestJson<UpdateStatusSummary>("/api/update/check", {
        method: "POST",
      });
      applyUpdateStatus(update);
      if (update.updateAvailable) {
        setUpdateConfirm({ status: update, source: "check" });
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsCheckingUpdate(false);
    }
  }

  async function saveAutoUpdateCheck(autoCheckEnabled: boolean) {
    setIsSavingUpdateSettings(true);
    setError(null);
    try {
      const update = await requestJson<UpdateStatusSummary>("/api/update/settings", {
        body: JSON.stringify({ autoCheckEnabled }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      applyUpdateStatus(update);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingUpdateSettings(false);
    }
  }

  async function installUpdate() {
    setIsInstallingUpdate(true);
    setError(null);
    try {
      const update = await installUpdateAndWaitForRestart();
      applyUpdateStatus(update);
      setUpdateConfirm({ status: update, source: "install" });
    } catch (requestError) {
      setError(errorMessage(requestError));
      setIsInstallingUpdate(false);
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
          chatTitleGenerationModelId: generalForm.chatTitleGenerationModelId,
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
          runtimeToolStateCompressionEnabled:
            generalForm.runtimeToolStateCompressionEnabled,
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
          contextCompressionSystemPrompt: promptOverridePayload(
            promptSettingsForm.contextCompression,
          ),
          generationSystemPrompt: promptOverridePayload(
            promptSettingsForm.generationSystemPrompt,
          ),
          updateSystemPrompt: promptOverridePayload(
            promptSettingsForm.updateSystemPrompt,
          ),
          memoryRetrievalSystemPrompt: promptOverridePayload(
            promptSettingsForm.memoryRetrieval,
          ),
          memoryExtractionSystemPrompt: promptOverridePayload(
            promptSettingsForm.memoryExtraction,
          ),
          memoryDreamSystemPrompt: promptOverridePayload(
            promptSettingsForm.memoryDream,
          ),
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

  async function flushSpecSettingsSaveQueue() {
    if (isSpecSettingsSaveInFlightRef.current) {
      return;
    }

    const nextForm = pendingSpecSettingsSaveRef.current;
    if (!nextForm) {
      setIsSavingSpecSettings(false);
      return;
    }

    pendingSpecSettingsSaveRef.current = null;
    isSpecSettingsSaveInFlightRef.current = true;
    setIsSavingSpecSettings(true);
    // Keep any prior save error visible until this request succeeds so a
    // pending/in-flight retry cannot hide a still-unconfirmed failure.

    let saveSucceeded = false;
    try {
      const data = await requestJson<SettingsResponse>("/api/settings/spec", {
        body: JSON.stringify({
          autoEnabled: nextForm.autoEnabled,
          generationModelId: nextForm.generationModelId.trim() || null,
          llmTimeoutMs: requiredPositiveInteger(
            nextForm.llmTimeoutMs,
            t("Spec LLM timeout ms"),
          ),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const confirmed = specSettingsFormFromResponse(data);
      confirmedSpecSettingsRef.current = confirmed;
      setSettings(data);
      onSettingsChange(data);
      // Preserve newer queued saves and mid-save draft edits (e.g. timeout typing).
      if (
        !pendingSpecSettingsSaveRef.current &&
        specSettingsFormsEqual(specSettingsFormRef.current, nextForm)
      ) {
        applySpecSettingsForm(confirmed);
      }
      setSpecSettingsSaveError(null);
      saveSucceeded = true;
    } catch (requestError) {
      if (!pendingSpecSettingsSaveRef.current) {
        if (specSettingsFormsEqual(specSettingsFormRef.current, nextForm)) {
          applySpecSettingsForm(confirmedSpecSettingsRef.current);
        }
        setSpecSettingsSaveError(errorMessage(requestError));
      }
    } finally {
      isSpecSettingsSaveInFlightRef.current = false;
      if (pendingSpecSettingsSaveRef.current) {
        void flushSpecSettingsSaveQueue();
      } else {
        setIsSavingSpecSettings(false);
        if (saveSucceeded) {
          void loadSpecJobs();
        }
      }
    }
  }

  function queueSpecSettingsSave(nextForm: SpecSettingsFormState) {
    applySpecSettingsForm(nextForm);
    pendingSpecSettingsSaveRef.current = nextForm;
    specSettingsMutationGenerationRef.current += 1;
    void flushSpecSettingsSaveQueue();
  }

  async function retrySpecJob(workspaceId: string, jobId: string) {
    const operationKey = `${workspaceId}:${jobId}`;
    if (specJobOperationKeysRef.current.has(operationKey)) {
      return;
    }
    specJobOperationKeysRef.current.add(operationKey);
    setSpecJobOperations((current) => ({
      ...current,
      [operationKey]: "retry",
    }));
    setSpecJobsError(null);

    try {
      await requestJson<RetryWorkspaceSpecJobResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs/${encodeURIComponent(jobId)}/retry`,
        { method: "POST" },
      );
      await loadSpecJobs();
    } catch (requestError) {
      setSpecJobsError(errorMessage(requestError));
    } finally {
      specJobOperationKeysRef.current.delete(operationKey);
      setSpecJobOperations((current) => {
        if (!(operationKey in current)) {
          return current;
        }
        const next = { ...current };
        delete next[operationKey];
        return next;
      });
    }
  }

  async function deleteSpecJob(workspaceId: string, jobId: string) {
    if (!window.confirm(t("Delete Spec job confirmation"))) {
      return;
    }

    const operationKey = `${workspaceId}:${jobId}`;
    if (specJobOperationKeysRef.current.has(operationKey)) {
      return;
    }
    specJobOperationKeysRef.current.add(operationKey);
    setSpecJobOperations((current) => ({
      ...current,
      [operationKey]: "delete",
    }));
    setSpecJobsError(null);

    try {
      await requestJson<DeleteFailedWorkspaceSpecJobResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs/${encodeURIComponent(jobId)}`,
        { method: "DELETE" },
      );
      await loadSpecJobs();
    } catch (requestError) {
      setSpecJobsError(errorMessage(requestError));
    } finally {
      specJobOperationKeysRef.current.delete(operationKey);
      setSpecJobOperations((current) => {
        if (!(operationKey in current)) {
          return current;
        }
        const next = { ...current };
        delete next[operationKey];
        return next;
      });
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
          modeModelId: planModeModelId.trim() || null,
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

  function updateNamedSystemPromptContent(name: string, content: string) {
    setPromptSettingsForm((current) => {
      const currentSystemPrompts = current.systemPrompts.length
        ? current.systemPrompts
        : settings
          ? normalizedSystemPromptSummaries(settings.prompts)
          : [];
      const hasPrompt = currentSystemPrompts.some((prompt) => prompt.name === name);
      return {
        ...current,
        systemPrompts: hasPrompt
          ? currentSystemPrompts.map((prompt) =>
            prompt.name === name
              ? {
                ...prompt,
                content,
              }
              : prompt,
          )
          : [
            ...currentSystemPrompts,
            {
              content,
              name,
            },
          ],
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

      // Plan/Review are edited as cards; only promote Default/user prompts into the list editor.
      const promoteActive =
        name === DEFAULT_SYSTEM_PROMPT_NAME ||
        (name !== PLAN_MODE_SYSTEM_PROMPT_NAME && name !== REVIEW_SYSTEM_PROMPT_NAME);

      return {
        ...current,
        activeSystemPromptName: promoteActive
          ? name
          : current.activeSystemPromptName,
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

  function selectPromptFile() {
    setIsSelectingPromptFile(true);
    setSettingsFilePickerRequest({
      mode: "file",
      multiple: true,
      target: { kind: "local" },
      title: t("Select prompt file"),
      onSelect: (selection) => {
        setPromptSettingsForm((current) => {
          const files = [...current.files];
          for (const item of selection) {
            if (!files.includes(item.path)) {
              files.push(item.path);
            }
          }
          return { ...current, files };
        });
      },
    });
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
          extractionLlmTimeoutMs: requiredPositiveInteger(
            memorySettingsForm.extractionLlmTimeoutMs,
            t("Extraction LLM timeout ms"),
          ),
          retrievalLlmTimeoutMs: requiredPositiveInteger(
            memorySettingsForm.retrievalLlmTimeoutMs,
            t("Retrieval LLM timeout ms"),
          ),
          contextBudgetPercent: requiredIntegerInRange(
            memorySettingsForm.contextBudgetPercent,
            t("Memory context budget %"),
            1,
            100,
          ),
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
            llmTimeoutMs: requiredPositiveInteger(
              memorySettingsForm.dream.llmTimeoutMs,
              t("Dream LLM timeout ms"),
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
    let runError: string | null = null;

    try {
      const data = await requestJson<MemoryDreamRunResponse>("/api/memory/dream/run", {
        body: JSON.stringify({
          scope,
          ...(workspaceId ? { workspaceId } : {}),
          triggerType: "manual",
          mode: memorySettingsForm.dream.mode,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      if (data.job) {
        showOptimisticMemoryDreamJob(data.job);
      }
    } catch (requestError) {
      runError = errorMessage(requestError);
      setMemoryDreamError(runError);
    } finally {
      setMemoryDreamRunKey(null);
      void loadMemoryDreamJobs(1).then(() => {
        if (runError) {
          setMemoryDreamError(runError);
        }
      });
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

  function goToSpecJobsPage(page: number) {
    const maxPage = specJobsTotalPages || 1;
    setSpecJobsPage(Math.min(Math.max(1, page), maxPage));
  }

  function updateSpecJobsPageSize(value: string) {
    setSpecJobsPage(1);
    setSpecJobsPageSize((current) =>
      Math.min(100, positiveIntegerText(value, current)),
    );
  }

  function updateShowRetryableSpecJobsOnly(value: boolean) {
    setSpecJobsPage(1);
    setShowRetryableSpecJobsOnly(value);
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

  function handleSettingsTableWheel(event: ReactWheelEvent<HTMLDivElement>) {
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) {
      return;
    }

    const deltaUnit =
      event.deltaMode === 1
        ? 16
        : event.deltaMode === 2
          ? event.currentTarget.clientHeight
          : 1;
    const node = findVerticalScrollAncestor(event.currentTarget.parentElement);
    if (node) {
      node.scrollTop += event.deltaY * deltaUnit;
      event.preventDefault();
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

  async function updateMemoryEnabled(memory: MemoryFactRecord, enabled: boolean) {
    if (pendingMemoryEnabledIds.has(memory.id)) {
      return;
    }

    setPendingMemoryEnabledIds((current) => new Set(current).add(memory.id));
    setError(null);

    try {
      const response = await requestJson<MemoryMutationResponse>("/api/memory/enabled", {
        body: JSON.stringify({
          chatId: memory.scope === "chat" ? memory.chatId : null,
          enabled,
          factId: memory.id,
          scope: memory.scope,
          workspaceId: memory.scope === "global" ? null : memoryFilter.workspaceId,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      if (!response.memory) {
        throw new Error(t("Memory enabled update returned no memory."));
      }
      setMemories((current) =>
        current.map((item) => (item.id === response.memory?.id ? response.memory : item)),
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setPendingMemoryEnabledIds((current) => {
        const next = new Set(current);
        next.delete(memory.id);
        return next;
      });
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
            thinkingLevel: isModelThinkingLevelSupported(
              selectedMetadata ?? editingModel,
              form.thinkingLevel,
            )
              ? form.thinkingLevel
              : null,
            clearThinkingLevel: !isModelThinkingLevelSupported(
              selectedMetadata ?? editingModel,
              form.thinkingLevel,
            ),
            webSearchMode: form.webSearchMode,
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
          thinkingLevel: isModelThinkingLevelSupported(model, model.thinkingLevel)
            ? model.thinkingLevel
            : null,
          clearThinkingLevel: !isModelThinkingLevelSupported(model, model.thinkingLevel),
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


  async function saveRemoteServer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      remoteServerForm.authMethod === "password" &&
      !remoteServerForm.password.trim() &&
      !remoteServerForm.passwordConfigured
    ) {
      setError(t("Password is required for password authentication"));
      return;
    }
    const operation = operationKey("save", remoteServerForm.id || "new");
    setRemoteServerOperationKey(operation);
    setError(null);

    try {
      const data = await requestJson<RemoteServerResponse>(
        remoteServerForm.id ? "/api/remote-servers/update" : "/api/remote-servers/create",
        {
          body: JSON.stringify(remoteServerFormPayload(remoteServerForm)),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setRemoteServerForm(remoteServerFormFromSummary(data.server));
      try {
        const connectResponse = await requestJson<RemoteServerDiagnosticResponse>(
          `/api/remote-servers/${encodeURIComponent(data.server.id)}/connect`,
          { method: "POST" },
        );
        setRemoteServerDiagnostics((current) => ({
          ...current,
          [data.server.id]: connectResponse.result,
        }));
        const nextSettings = await requestJson<SettingsResponse>("/api/settings");
        setSettings(nextSettings);
        onSettingsChange(nextSettings);
        if (
          hostKeyPromptFromDiagnostic(
            data.server,
            connectResponse.result,
            "connect",
            true,
          )
        ) {
          return;
        }
        if (!connectResponse.result.ok) {
          if (connectResponse.result.message) {
            setError(connectResponse.result.message);
          }
          return;
        }
      } catch (connectError) {
        const nextSettings = await requestJson<SettingsResponse>("/api/settings");
        setSettings(nextSettings);
        onSettingsChange(nextSettings);
        throw connectError;
      }
      setIsRemoteServerDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setRemoteServerOperationKey(null);
    }
  }

  async function runRemoteServerOperation(
    server: RemoteServerSummary,
    operation: Exclude<RemoteServerOperation, "save">,
  ): Promise<boolean> {
    const key = operationKey(operation, server.id);
    setRemoteServerOperationKey(key);
    setRemoteServerReferences([]);
    setError(null);

    try {
      if (operation === "delete") {
        await requestJson<{ deleted: boolean; references: RemoteServerWorkspaceReference[] }>(
          "/api/remote-servers/delete",
          {
            body: JSON.stringify({ id: server.id }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
      } else if (operation === "disconnect") {
        await requestJson<RemoteServerResponse>(
          `/api/remote-servers/${encodeURIComponent(server.id)}/disconnect`,
          { method: "POST" },
        );
      } else {
        const response = await requestJson<RemoteServerDiagnosticResponse>(
          `/api/remote-servers/${encodeURIComponent(server.id)}/${operation}`,
          { method: "POST" },
        );
        setRemoteServerDiagnostics((current) => ({
          ...current,
          [server.id]: response.result,
        }));
        const nextSettings = await requestJson<SettingsResponse>("/api/settings");
        setSettings(nextSettings);
        onSettingsChange(nextSettings);
        if (hostKeyPromptFromDiagnostic(server, response.result, operation)) {
          return false;
        }
        if (!response.result.ok) {
          if (response.result.message) {
            setError(response.result.message);
          }
          return false;
        }
        return true;
      }

      const nextSettings = await requestJson<SettingsResponse>("/api/settings");
      setSettings(nextSettings);
      onSettingsChange(nextSettings);
      if (operation === "delete") {
        setIsRemoteServerDialogOpen(false);
      }
      return true;
    } catch (requestError) {
      const message = errorMessage(requestError);
      setError(message);
      if (operation === "delete") {
        setRemoteServerReferences(remoteServerReferencesForMessage(message, workspaces));
      }
      return false;
    } finally {
      setRemoteServerOperationKey(null);
    }
  }

  async function saveWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    const workspaceId = workspaceForm.id.trim();
    const shouldSaveSpec = Boolean(workspaceId) && isWorkspaceSpecSettingsLoaded;
    const nextSpecEnabled = workspaceForm.specEnabled;
    const nextSpecInjectEnabled = workspaceForm.specEnabled
      ? workspaceForm.specInjectEnabled
      : false;
    const isRemoteWorkspace = Boolean(workspaceForm.serverId?.trim());
    const remotePathValue = isRemoteWorkspace
      ? (workspaceForm.remotePath ?? workspaceForm.path).trim()
      : "";
    const localPathValue = workspaceForm.path.trim();

    try {
      if (!workspaceId) {
        throw new Error(t("Workspace was not found."));
      }

      const data = await requestJson<SettingsResponse>("/api/workspaces/manual", {
        body: JSON.stringify({
          id: workspaceId,
          name: workspaceForm.name,
          path: isRemoteWorkspace ? remotePathValue : localPathValue,
          serverId: isRemoteWorkspace ? workspaceForm.serverId : null,
          remotePath: isRemoteWorkspace ? remotePathValue : null,
          pinned: workspaceForm.pinned,
          terminalShell: workspaceForm.terminalShell,
          commonCommands: workspaceForm.commonCommands,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
      if (shouldSaveSpec) {
        const savedSpec = await saveWorkspaceSpecSettingsRequest(
          workspaceId,
          nextSpecEnabled,
          nextSpecInjectEnabled,
        );
        setWorkspaceForm((current) =>
          current.id === workspaceId
            ? {
                ...current,
                path: isRemoteWorkspace ? remotePathValue : localPathValue,
                remotePath: isRemoteWorkspace ? remotePathValue : null,
                serverId: isRemoteWorkspace ? workspaceForm.serverId : null,
                specEnabled: savedSpec.settings.enabled,
                specInjectEnabled: savedSpec.settings.injectEnabled,
              }
            : current,
        );
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

  async function deleteConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setPendingDeleteWorkspace(null);
    setDeletingWorkspaceId(workspace.id);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        `/api/workspaces/${encodeURIComponent(workspace.id)}`,
        { method: "DELETE" },
      );
      setSettings(data);
      onSettingsChange(data);
      await onWorkspacesChange();
      setWorkspaceOrderPreview(null);
      if (workspaceForm.id === workspace.id) {
        setWorkspaceForm(emptyWorkspaceForm());
        setIsWorkspaceDialogOpen(false);
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setDeletingWorkspaceId(null);
    }
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

    try {
      const data = await requestJson<SettingsResponse>("/api/workspaces/manual", {
        body: JSON.stringify({
          id: workspace.id,
          name: workspace.name,
          path: workspace.serverId ? (workspace.remotePath ?? workspace.path) : workspace.path,
          serverId: workspace.serverId ?? null,
          remotePath: workspace.remotePath ?? null,
          pinned,
          terminalShell: workspace.terminalShell,
          commonCommands: workspace.commonCommands,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
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

  function selectWorkspaceFormPath() {
    setIsSelectingWorkspaceFormPath(true);
    setSettingsFilePickerRequest({
      initialPath: workspaceForm.path,
      mode: "directory",
      target: workspaceForm.serverId
        ? { kind: "remoteServer", serverId: workspaceForm.serverId }
        : { kind: "local" },
      title: t("Choose workspace path"),
      onSelect: (selection) => {
        const selectedPath = selection[0]?.path;
        if (!selectedPath) {
          return;
        }
        setWorkspaceForm((current) => ({
          ...current,
          name: current.name.trim() ? current.name : workspaceNameFromPath(selectedPath),
          path: selectedPath,
          remotePath: current.serverId ? selectedPath : current.remotePath,
        }));
      },
    });
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

  function addProviderModelRedirect() {
    setProviderForm((current) => ({
      ...current,
      modelRedirects: [
        ...current.modelRedirects,
        { from: "", to: "" },
      ],
    }));
  }

  function updateProviderModelRedirect(
    index: number,
    patch: Partial<ProviderModelRedirect>,
  ) {
    setProviderForm((current) => ({
      ...current,
      modelRedirects: current.modelRedirects.map((redirect, redirectIndex) =>
        redirectIndex === index ? { ...redirect, ...patch } : redirect,
      ),
    }));
  }

  function deleteProviderModelRedirect(index: number) {
    setProviderForm((current) => ({
      ...current,
      modelRedirects: current.modelRedirects.filter(
        (_redirect, redirectIndex) => redirectIndex !== index,
      ),
    }));
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
    setIsProviderApiKeyVisible(false);
    setIsRevealingProviderApiKey(false);

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
            modelRedirects: providerForm.modelRedirects.map((redirect) => ({
              from: redirect.from,
              to: redirect.to,
            })),
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
      closeProviderDialog();
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
            executionHost: mcpForm.executionHost,
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

  function startProviderOperation(providerId: string) {
    if (providerOperationIdsRef.current.has(providerId)) {
      return false;
    }

    providerOperationIdsRef.current.add(providerId);
    setProviderOperationIds(new Set(providerOperationIdsRef.current));
    return true;
  }

  function finishProviderOperation(providerId: string) {
    providerOperationIdsRef.current.delete(providerId);
    setProviderOperationIds(new Set(providerOperationIdsRef.current));
  }

  async function toggleConfiguredProviderEnabled(
    provider: ConfiguredProviderSummary,
    enabled: boolean,
  ) {
    if (!startProviderOperation(provider.id)) {
      return;
    }

    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/providers/manual", {
        body: JSON.stringify({
          apiKey: null,
          apiProxy: {
            enabled: provider.apiProxy.enabled,
            proxyType: provider.apiProxy.proxyType,
            url: provider.apiProxy.url,
          },
          baseUrl: provider.baseUrl,
          clearApiKey: false,
          enabled,
          id: provider.id,
          kind: provider.kind,
          autoSyncModels: provider.autoSyncModels,
          modelSyncFilterRegex: provider.modelSyncFilterRegex,
          modelRedirects: provider.modelRedirects.map((redirect) => ({
            from: redirect.from,
            to: redirect.to,
          })),
          name: provider.name,
          requestOverrides: provider.requestOverrides.map((overrideRule) => ({
            ...overrideRule,
          })),
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      finishProviderOperation(provider.id);
    }
  }

  async function deleteProvider(providerId: string) {
    if (!startProviderOperation(providerId)) {
      return;
    }

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
      finishProviderOperation(providerId);
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

  async function saveSkills(
    nextEnabledSkillIds: Set<string>,
    nextTranslationModelId = skills?.translationModelId ?? null,
  ) {
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
          translationModelId: nextTranslationModelId,
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

  async function saveSkillLocations(nextDisabledLocationIds: string[]) {
    setIsSavingSkills(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/skills/manual", {
        body: JSON.stringify({ disabledLocationIds: nextDisabledLocationIds }),
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

  async function deleteSkill(skill: ConfiguredSkillSummary) {
    if (!window.confirm(t("Delete skill confirmation"))) {
      return;
    }

    setIsSavingSkills(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>("/api/skills/delete", {
        body: JSON.stringify({ id: skill.key }),
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

  async function toggleRemoteWorkspaceSkill(
    workspaceId: string,
    skill: ConfiguredSkillSummary,
    enabled: boolean,
  ) {
    setIsSavingSkills(true);
    setError(null);

    try {
      await requestJson(`/api/workspaces/${encodeURIComponent(workspaceId)}/skills/manual`, {
        body: JSON.stringify({ enabled, key: skill.key }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      reloadRemoteWorkspaceSkillCatalogs();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingSkills(false);
    }
  }

  async function deleteRemoteWorkspaceSkill(
    workspaceId: string,
    skill: ConfiguredSkillSummary,
  ) {
    if (!window.confirm(t("Delete skill confirmation"))) {
      return;
    }

    setIsSavingSkills(true);
    setError(null);

    try {
      await requestJson(`/api/workspaces/${encodeURIComponent(workspaceId)}/skills/delete`, {
        body: JSON.stringify({ id: skill.key }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      reloadRemoteWorkspaceSkillCatalogs();
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
      reloadRemoteWorkspaceSkillCatalogs();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsRefreshingSkills(false);
    }
  }

  async function updateSkill(skill: ConfiguredSkillSummary) {
    setUpdatingSkillKey(skill.key);
    setError(null);

    try {
      const data = await requestJson<SkillStoreUpdateResponse>("/api/skill-store/update", {
        body: JSON.stringify({ key: skill.key }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data.settings);
      onSettingsChange(data.settings);
      syncSkillsForm(data.settings);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setUpdatingSkillKey(null);
    }
  }

  async function updateAllStoreSkills() {
    setIsUpdatingAllSkills(true);
    setError(null);

    try {
      const data = await requestJson<SkillStoreUpdateResponse>("/api/skill-store/update-all", {
        method: "POST",
      });
      setSettings(data.settings);
      onSettingsChange(data.settings);
      syncSkillsForm(data.settings);
      const failed = data.results.filter((result) => !result.ok);
      if (failed.length) {
        setError(
          failed
            .map((result) => `${result.key}: ${result.error ?? t("Update failed")}`)
            .join("; "),
        );
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsUpdatingAllSkills(false);
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
      [providerId]: { message: t("Testing connection…"), status: "testing" },
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

  async function testModel(model: ConfiguredModelSummary) {
    if (modelTestsInFlightRef.current.has(model.id)) {
      return;
    }

    modelTestsInFlightRef.current.add(model.id);
    setModelTests((current) => ({
      ...current,
      [model.id]: { testing: true },
    }));
    setModelTestToast(null);

    try {
      const data = await requestJson<ModelTestResponse>("/api/models/test", {
        body: JSON.stringify({ modelId: model.id }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const modelName = model.displayName || data.modelId || model.id;
      const resultMessageKey = data.ok
        ? "Model test succeeded for {name}: {message}"
        : "Model test failed for {name}: {message}";
      setModelTestToast({
        kind: data.ok ? "success" : "error",
        message: t(resultMessageKey, {
          message: data.message,
          name: modelName,
        }),
      });
    } catch (requestError) {
      setModelTestToast({
        kind: "error",
        message: t("Model test failed for {name}: {message}", {
          message: errorMessage(requestError),
          name: model.displayName || model.id,
        }),
      });
    } finally {
      modelTestsInFlightRef.current.delete(model.id);
      setModelTests((current) => ({
        ...current,
        [model.id]: { testing: false },
      }));
    }
  }

  async function loadProviderModels(providerId: string) {
    if (loadingProviderModelIdsRef.current.has(providerId)) {
      return;
    }
    loadingProviderModelIdsRef.current.add(providerId);
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
    } finally {
      loadingProviderModelIdsRef.current.delete(providerId);
    }
  }

  useEffect(() => {
    if (!isModelDialogOpen) {
      return;
    }

    for (const provider of providers) {
      if (
        provider.enabled &&
        !providerModelLists[provider.id] &&
        !loadingProviderModelIdsRef.current.has(provider.id)
      ) {
        void loadProviderModels(provider.id);
      }
    }
  }, [isModelDialogOpen, providerModelLists, providers]);

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
    const next = new Set(currentEnabledSkillIds);

    if (checked) {
      next.add(skillId);
    } else {
      next.delete(skillId);
    }

    setEnabledSkillIds(next);
    void saveSkills(next);
  }

  function toggleSkillLocation(locationId: string, checked: boolean) {
    const nextDisabledLocationIds = skillLocations
      .filter((location) => (location.id === locationId ? !checked : !location.enabled))
      .map((location) => location.id);
    void saveSkillLocations(nextDisabledLocationIds);
  }

  function changeSkillTranslationModel(modelId: string) {
    const currentEnabledSkillIds = new Set(
      (skills?.detected ?? [])
        .filter((skill) => skill.enabled)
        .map((skill) => skill.key),
    );
    void saveSkills(currentEnabledSkillIds, modelId || null);
  }


  return (
    <div className="settings-shell panel-scroll min-h-0 flex-1 overflow-y-auto">
      {modelTestToast ? (
        <div
          aria-live={modelTestToast.kind === "success" ? "polite" : "assertive"}
          className={modelTestToast.kind === "success" ? "app-status-toast" : "app-error-toast"}
          role={modelTestToast.kind === "success" ? "status" : "alert"}
        >
          {modelTestToast.kind === "success" ? (
            <CheckCircle2 aria-hidden="true" className="app-status-toast-icon" />
          ) : (
            <CircleAlert aria-hidden="true" className="app-error-toast-icon" />
          )}
          <div className="app-error-toast-message">{modelTestToast.message}</div>
          <SettingsButton
            aria-label={t("Dismiss model test result")}
            className={
              modelTestToast.kind === "success"
                ? "app-status-toast-close"
                : "app-error-toast-close"
            }
            onClick={() => setModelTestToast(null)}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </SettingsButton>
        </div>
      ) : null}
      <div className="settings-layout grid">
        <aside className="settings-section-nav-card flex min-h-0 flex-col border-[var(--border)] p-2">
          <div className="settings-sidebar-header workspace-sidebar-header flex items-center justify-between gap-2 border-b border-[var(--border)] px-4 py-2">
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
              active={activeSection === "remote-servers"}
              icon={Server}
              label={t("Remote Servers")}
              onClick={() => onActiveSectionChange("remote-servers")}
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
            <SettingsNavButton
              active={activeSection === "about"}
              icon={Info}
              label={t("About")}
              onClick={() => onActiveSectionChange("about")}
            />
          </nav>
        </aside>

        <div className="min-w-0 flex flex-col gap-5">
          <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_75%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="text-lg font-semibold text-[var(--foreground)]">
                  {settingsSectionTitle(activeSection, t)}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-[var(--muted)]">
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
                <SettingsButton
                  aria-label={t("Refresh model metadata")}
                  className="inline-flex size-10 items-center justify-center rounded-lg bg-[var(--accent)] text-white shadow-[var(--overlay-shadow)] hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--default)] disabled:shadow-none"
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
                </SettingsButton>
              ) : null}
            </div>
          </section>

          {error && !isWorkspaceDialogOpen ? (
            <div className="rounded-xl border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
              {error}
            </div>
          ) : null}

          {activeSection === "general" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                onSubmit={(event) => void saveGeneralSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <Globe aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Web service")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <SettingsTextField
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
                  <SettingsTextField
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
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("LLM request retries")}
                    </span>
                    <SettingsInput
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Chat title generation model")}
                    </span>
                    <SettingsSelect
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                      onChange={(event) =>
                        setGeneralForm((current) => ({
                          ...current,
                          chatTitleGenerationModelId: event.target.value,
                        }))
                      }
                      value={generalForm.chatTitleGenerationModelId}
                    >
                      <option value="disabled">{t("Disabled")}</option>
                      <option value="current_chat_model">{t("Current chat model")}</option>
                      {configuredModelsByName
                        .filter((model) => model.enabled)
                        .map((model) => (
                          <option key={model.id} value={model.id}>
                            {model.displayName || model.id}
                          </option>
                        ))}
                    </SettingsSelect>
                  </label>
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Context management")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Runtime tool-state compression")}
                        </p>
                        <p className="mt-1 max-w-3xl text-xs leading-5 text-[var(--muted)]">
                          {t(
                            "At 80% context usage, replace older tool messages with compact snapshots. This breaks the provider's context cache.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Runtime tool-state compression")}
                        className="inline-flex size-10 shrink-0 items-center justify-center self-end rounded-lg border border-[var(--border)] bg-[var(--surface)] sm:self-auto"
                      >
                        <SettingsInput
                          checked={generalForm.runtimeToolStateCompressionEnabled}
                          className="size-4 accent-[var(--accent)]"
                          onChange={(event) =>
                            setGeneralForm((current) => ({
                              ...current,
                              runtimeToolStateCompressionEnabled: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                  </fieldset>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("API request detail retention days")}
                    </span>
                    <SettingsInput
                      autoComplete="off"
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("API request details")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Database
                          aria-hidden="true"
                          className="size-4 shrink-0 text-[var(--accent-soft-foreground)]"
                        />
                        <p className="text-sm font-semibold text-[var(--foreground)]">
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
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                        >
                          <SettingsInput
                            checked={generalForm.apiSaveRequestResponseDetails}
                            className="size-4 accent-[var(--accent)]"
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
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Startup")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="flex min-w-0 items-center gap-2">
                        <Play
                          aria-hidden="true"
                          className="size-4 shrink-0 fill-current text-[var(--accent-soft-foreground)]"
                        />
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Start Foco at startup")}
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
                          aria-label={t("Start Foco at startup")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                        >
                          <SettingsInput
                            checked={generalForm.autoStartEnabled}
                            className="size-4 accent-[var(--accent)]"
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
                <div className="mt-4 border-t border-[var(--border)] pt-4">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2">
                      <Lock aria-hidden="true" className="size-4 text-[var(--accent-soft-foreground)]" />
                      <h4 className="text-sm font-semibold text-[var(--foreground)]">
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
                      <SettingsButton
                        aria-label={t("Log out")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => void onLogout()}
                        title={t("Log out")}
                        type="button"
                      >
                        <Lock aria-hidden="true" className="size-4" />
                        {t("Log out")}
                      </SettingsButton>
                    ) : null}
                    <SettingsButton
                      aria-label={t("Clear browser password")}
                      className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--danger)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:border-[var(--border)] disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                    </SettingsButton>
                  </div>
                  <div className="mt-3 grid gap-3">
                    <div className="grid gap-2">
                      <label className="block min-w-0">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Authentication password")}
                        </span>
                        <span className="relative block">
                          <SettingsInput
                            autoComplete="new-password"
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 pr-10 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          <SettingsButton
                            aria-label={
                              isGeneralPasswordVisible
                                ? t("Hide password")
                                : t("Show password")
                            }
                            className="absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)]"
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
                          </SettingsButton>
                        </span>
                        {settings?.general.webServer.passwordEnabled ? (
                          <span className="mt-1 block text-xs text-[var(--muted)]">
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
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Language")}
                  </span>
                  <SettingsSelect
                    className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                    disabled={isSavingLanguage || isLoadingSettings}
                    onChange={(event) => void saveLanguageSetting(event.target.value)}
                    value={generalForm.language}
                  >
                    {(settings?.general.supportedLanguages ?? []).map((language) => (
                      <option key={language.id} value={language.id}>
                        {language.name}
                      </option>
                    ))}
                  </SettingsSelect>
                  <span className="mt-1 block text-xs text-[var(--muted)]">
                    {t("Language changes apply immediately after saving.")}
                  </span>
                </label>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Theme")}
                  </span>
                  <SettingsSelect
                    className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                  </SettingsSelect>
                  <span className="mt-1 block text-xs text-[var(--muted)]">
                    {t("Theme changes apply immediately after saving.")}
                  </span>
                </label>
                <div className="mt-4 flex flex-wrap gap-2">
                  <SettingsButton
                    aria-label={t("Save general settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                  </SettingsButton>
                  <SettingsButton
                    aria-label={t("Reload general settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                  </SettingsButton>
                </div>
              </form>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Saved bind")}
                  </h3>
                  <CapabilityPill label={t("restart required")} ok={false} />
                </div>
                <div className="mt-4 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                  <div className="break-all text-sm font-semibold text-[var(--foreground)]">
                    {settings
                      ? `${settings.general.webServer.listenHost}:${settings.general.webServer.listenPort}`
                      : t("Loading…")}
                  </div>
                  <div className="mt-2 text-xs text-[var(--muted)]">
                    {t("Saved host and port are used the next time the backend starts.")}
                  </div>
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "web-search" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                onSubmit={(event) => void saveWebSearchSettings(event)}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Search aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Web search")}
                    </h3>
                  </div>
                  <CapabilityPill
                    label={webSearchForm.enabled ? t("enabled") : t("disabled")}
                    ok={webSearchForm.enabled}
                  />
                </div>
                <div className="mt-4 grid gap-4">
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Runtime tool")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Allow web search for chat runs")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-[var(--muted)]">
                          {t(
                            "Master switch for online search. Models with confirmed native search use the provider; others can fall back to Tavily/Brave when a key is configured.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Allow web search for chat runs")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                      >
                        <SettingsInput
                          checked={webSearchForm.enabled}
                          className="size-4 accent-[var(--accent)]"
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
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-xs leading-5 text-[var(--muted)]">
                    {t(
                      "Tavily and Brave are function-path fallbacks for models without confirmed native search. Enabling the master switch does not require a search API key.",
                    )}
                    {settings?.webSearch ? (
                      <span className="mt-1 block font-medium text-[var(--muted)]">
                        {settings.webSearch.fallbackAvailable
                          ? t("Function fallback: available")
                          : t("Function fallback: no API key for active provider")}
                      </span>
                    ) : null}
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Search API")}
                    </span>
                    <SettingsSelect
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Web search proxy")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Proxy search API requests")}
                        </p>
                        <p className="mt-1 text-xs leading-5 text-[var(--muted)]">
                          {t("Applies only to web_search requests sent to the configured search API.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable web search proxy")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                      >
                        <SettingsInput
                          checked={webSearchForm.apiProxyEnabled}
                          className="size-4 accent-[var(--accent)]"
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Proxy type")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <SettingsTextField
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
                          className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3"
                          key={provider.provider}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-sm font-semibold text-[var(--foreground)]">
                              {provider.label}
                            </span>
                            <CapabilityPill
                              label={provider.hasApiKey ? t("saved") : t("missing")}
                              ok={provider.hasApiKey}
                            />
                          </div>
                          <label className="mt-3 block">
                            <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                              {t("API token")}
                            </span>
                            <SettingsInput
                              autoComplete="off"
                              className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                            <label className="mt-3 flex items-center gap-2 text-xs font-semibold text-[var(--muted)]">
                              <SettingsInput
                                checked={Boolean(webSearchForm[clearField])}
                                className="size-4 accent-[var(--accent)]"
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
                  <SettingsButton
                    aria-label={t("Save web search settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                  </SettingsButton>
                  <SettingsButton
                    aria-label={t("Reload web search settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                  </SettingsButton>
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
              thinkingLevels={thinkingLevels}
            />
          ) : null}

          {activeSection === "prompts" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                onSubmit={(event) => void savePromptSettings(event)}
              >
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Extra prompt")}
                  </span>
                  <SettingsTextArea
                    aria-label={t("Extra prompt")}
                    className="min-h-36 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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

                <div className="mt-6 flex items-center justify-between gap-3 border-t border-[var(--border)] pt-4">
                  <div className="flex items-center gap-2">
                    <Bot aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("System prompt")}
                    </h3>
                  </div>
                  <span className="rounded-full border border-[var(--border)] bg-[var(--surface-secondary)] px-2.5 py-1 text-xs font-semibold text-[var(--muted)]">
                    {activeSystemPrompt?.name ?? DEFAULT_SYSTEM_PROMPT_NAME}
                  </span>
                </div>
                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(180px,240px)_minmax(0,1fr)]">
                  <div className="grid content-start gap-2">
                    <div className="overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)]">
                      {listSystemPrompts.map((prompt) => {
                        const isActive =
                          prompt.name === promptSettingsForm.activeSystemPromptName;
                        const isRenaming =
                          prompt.name === promptSettingsForm.renamingSystemPromptName;
                        const isFixed = isSystemPromptFixed(prompt.name);

                        return (
                          <div
                            className={`flex items-center gap-2 px-3 py-2 ${isActive
                                ? "bg-[var(--accent-soft)]"
                                : "hover:bg-[var(--surface)]"
                              }`}
                            key={prompt.name}
                          >
                            {isRenaming ? (
                              <>
                                <SettingsInput
                                  aria-label={t("System prompt name")}
                                  autoComplete="off"
                                  autoFocus
                                  className="h-8 min-w-0 flex-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm font-semibold text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                                <SettingsButton
                                  aria-label={t("Save system prompt name")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                                  disabled={
                                    !promptSettingsForm.pendingSystemPromptRename.trim()
                                  }
                                  onClick={() => submitRenameSystemPrompt(prompt.name)}
                                  title={t("Save system prompt name")}
                                  type="button"
                                >
                                  <CheckCircle2 aria-hidden="true" className="size-4" />
                                </SettingsButton>
                                <SettingsButton
                                  aria-label={t("Cancel system prompt rename")}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                  onClick={cancelRenameSystemPrompt}
                                  title={t("Cancel system prompt rename")}
                                  type="button"
                                >
                                  <X aria-hidden="true" className="size-4" />
                                </SettingsButton>
                              </>
                            ) : (
                              <>
                                <SettingsButton
                                  className={`min-w-0 flex-1 truncate text-left text-sm font-semibold ${isActive
                                      ? "text-[var(--accent-soft-foreground)]"
                                      : "text-[var(--muted)]"
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
                                </SettingsButton>
                                {defaultSystemPromptContent(prompt.name) !== null ? (
                                  <SettingsButton
                                    aria-label={t("Restore default system prompt")}
                                    className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                    disabled={isLoadingSettings || !settings}
                                    onClick={() => restoreSystemPromptDefault(prompt.name)}
                                    title={t("Restore default system prompt")}
                                    type="button"
                                  >
                                    <RefreshCw aria-hidden="true" className="size-4" />
                                  </SettingsButton>
                                ) : isFixed ? null : (
                                  <>
                                    <SettingsButton
                                      aria-label={t("Rename system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                      onClick={() => startRenameSystemPrompt(prompt.name)}
                                      title={t("Rename system prompt")}
                                      type="button"
                                    >
                                      <Pencil aria-hidden="true" className="size-4" />
                                    </SettingsButton>
                                    <SettingsButton
                                      aria-label={t("Remove system prompt {name}", {
                                        name: prompt.name,
                                      })}
                                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)]"
                                      onClick={() => removeSystemPrompt(prompt.name)}
                                      title={t("Remove system prompt")}
                                      type="button"
                                    >
                                      <Trash2 aria-hidden="true" className="size-4" />
                                    </SettingsButton>
                                  </>
                                )}
                              </>
                            )}
                          </div>
                        );
                      })}
                    </div>
                    <div className="flex gap-2">
                      <SettingsInput
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                        onChange={(event) =>
                          setPromptSettingsForm((current) => ({
                            ...current,
                            pendingSystemPromptName: event.target.value,
                          }))
                        }
                        placeholder={t("Prompt name")}
                        value={promptSettingsForm.pendingSystemPromptName}
                      />
                      <SettingsButton
                        aria-label={t("Add system prompt")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                        disabled={!promptSettingsForm.pendingSystemPromptName.trim()}
                        onClick={() =>
                          addSystemPrompt(promptSettingsForm.pendingSystemPromptName)
                        }
                        title={t("Add system prompt")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </div>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("System prompt")}
                    </span>
                    <SettingsTextArea
                      aria-label={t("System prompt")}
                      className="min-h-72 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-sm leading-6 text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                      onChange={(event) =>
                        updateActiveSystemPromptContent(event.target.value)
                      }
                      value={activeSystemPrompt?.content ?? ""}
                    />
                  </label>
                </div>

                <div className="mt-4 grid gap-3">
                  <PromptOverrideEditor
                    description={t(
                      "Used for Plan mode sessions. Stored as the built-in Plan Mode system prompt.",
                    )}
                    onChange={(value) =>
                      updateNamedSystemPromptContent(PLAN_MODE_SYSTEM_PROMPT_NAME, value)
                    }
                    onRestore={() => restoreSystemPromptDefault(PLAN_MODE_SYSTEM_PROMPT_NAME)}
                    restoreAriaLabel={t("Restore default Plan Mode prompt")}
                    testId="plan-mode-system-prompt"
                    title={t("Plan Mode prompt")}
                    t={t}
                    value={planModeSystemPrompt?.content ?? ""}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used for the built-in Review agent. Stored as the built-in Review system prompt.",
                    )}
                    onChange={(value) =>
                      updateNamedSystemPromptContent(REVIEW_SYSTEM_PROMPT_NAME, value)
                    }
                    onRestore={() => restoreSystemPromptDefault(REVIEW_SYSTEM_PROMPT_NAME)}
                    restoreAriaLabel={t("Restore default Review Agent prompt")}
                    testId="review-system-prompt"
                    title={t("Review Agent prompt")}
                    t={t}
                    value={reviewSystemPrompt?.content ?? ""}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used only for internal contextCompression checkpoint requests. It is not injected into normal chat System prompts.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        contextCompression: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        contextCompression: {
                          value:
                            settings?.prompts.defaultContextCompressionSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default context compression prompt")}
                    testId="context-compression-system-prompt"
                    title={t("Context compression prompt")}
                    t={t}
                    value={promptSettingsForm.contextCompression.value}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used when generating a Project Spec from evidence. Stored in Spec settings for compatibility.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        generationSystemPrompt: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        generationSystemPrompt: {
                          value: settings?.spec.defaultGenerationSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default Spec generation prompt")}
                    testId="spec-generation-system-prompt"
                    title={t("Spec generation prompt")}
                    t={t}
                    value={promptSettingsForm.generationSystemPrompt.value}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used when deciding whether and how to update a Project Spec after a chat turn.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        updateSystemPrompt: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        updateSystemPrompt: {
                          value: settings?.spec.defaultUpdateSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default Spec update prompt")}
                    testId="spec-update-system-prompt"
                    title={t("Spec update prompt")}
                    t={t}
                    value={promptSettingsForm.updateSystemPrompt.value}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used for LLM-based memory matching/retrieval. It is not injected into normal chat System prompts.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryRetrieval: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryRetrieval: {
                          value:
                            settings?.prompts.defaultMemoryRetrievalSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default memory matching prompt")}
                    testId="memory-retrieval-system-prompt"
                    title={t("Memory matching prompt")}
                    t={t}
                    value={promptSettingsForm.memoryRetrieval.value}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used for LLM-based memory extraction. A language-specific suffix is still appended at runtime.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryExtraction: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryExtraction: {
                          value:
                            settings?.prompts.defaultMemoryExtractionSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default memory extraction prompt")}
                    testId="memory-extraction-system-prompt"
                    title={t("Memory extraction prompt")}
                    t={t}
                    value={promptSettingsForm.memoryExtraction.value}
                  />
                  <PromptOverrideEditor
                    description={t(
                      "Used for Dream memory consolidation runs. It is not injected into normal chat System prompts.",
                    )}
                    onChange={(value) =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryDream: { value, custom: true },
                      }))
                    }
                    onRestore={() =>
                      setPromptSettingsForm((current) => ({
                        ...current,
                        memoryDream: {
                          value: settings?.prompts.defaultMemoryDreamSystemPrompt ?? "",
                          custom: false,
                        },
                      }))
                    }
                    restoreAriaLabel={t("Restore default Dream prompt")}
                    testId="memory-dream-system-prompt"
                    title={t("Dream prompt")}
                    t={t}
                    value={promptSettingsForm.memoryDream.value}
                  />
                </div>

                <div className="mt-6 flex items-center gap-2 border-t border-[var(--border)] pt-4">
                  <ScrollText aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Prompt files")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Prompt file path")}
                    </span>
                    <div className="flex gap-2">
                      <SettingsInput
                        autoComplete="off"
                        className="h-10 min-w-0 flex-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Add prompt file")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                        disabled={!promptSettingsForm.pendingFile.trim()}
                        onClick={() => addPromptFilePath(promptSettingsForm.pendingFile)}
                        title={t("Add prompt file")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </SettingsButton>
                      <SettingsButton
                        aria-label={t("Choose prompt file")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                        disabled={isSelectingPromptFile}
                        onClick={selectPromptFile}
                        title={t("Choose prompt file")}
                        type="button"
                      >
                        {isSelectingPromptFile ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <FolderSearch aria-hidden="true" className="size-4" />
                        )}
                      </SettingsButton>
                    </div>
                  </label>
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)]">
                    {promptSettingsForm.files.length ? (
                      <div className="divide-y divide-[var(--border)]">
                        {promptSettingsForm.files.map((file) => (
                          <div
                            className="flex min-w-0 items-center justify-between gap-3 px-3 py-2"
                            key={file}
                          >
                            <div className="min-w-0 break-all text-sm font-semibold text-[var(--foreground)]">
                              {file}
                            </div>
                            <SettingsButton
                              aria-label={t("Remove prompt file {path}", { path: file })}
                              className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)]"
                              onClick={() => removePromptFilePath(file)}
                              title={t("Remove prompt file")}
                              type="button"
                            >
                              <Trash2 aria-hidden="true" className="size-4" />
                            </SettingsButton>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="px-3 py-6 text-center text-sm font-medium text-[var(--muted)]">
                        {t("No prompt files")}
                      </div>
                    )}
                  </div>
                </div>

                <div className="mt-4 flex flex-wrap gap-2">
                  <SettingsButton
                    aria-label={t("Save prompt settings")}
                    className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                  </SettingsButton>
                </div>
              </form>
            </section>
          ) : null}

          {activeSection === "spec" ? (
            <section className="grid gap-4">
              <div className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <FileText aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Auto Spec")}
                    </h3>
                  </div>
                  {isSavingSpecSettings ? (
                    <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--muted)]">
                      <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                      {t("Saving…")}
                    </span>
                  ) : null}
                </div>
                {specSettingsSaveError ? (
                  <div className="mt-3 rounded-xl border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
                    {specSettingsSaveError}
                  </div>
                ) : null}
                <div className="mt-4 grid gap-3">
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Automation")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Enable Auto Spec")}
                        </p>
                        <p className="mt-1 text-xs text-[var(--muted)]">
                          {t("Updates enabled workspace specs after successful chat turns.")}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable Auto Spec")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                      >
                        <SettingsInput
                          checked={specSettingsForm.autoEnabled}
                          className="size-4 accent-[var(--accent)]"
                          onChange={(event) =>
                            queueSpecSettingsSave({
                              ...specSettingsFormRef.current,
                              autoEnabled: event.target.checked,
                            })
                          }
                          type="checkbox"
                        />
                      </label>
                    </div>
                    <div className="mt-3">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Spec generation model")}
                        </span>
                        <SettingsSelect
                          aria-label={t("Spec generation model")}
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                          onChange={(event) =>
                            queueSpecSettingsSave({
                              ...specSettingsFormRef.current,
                              generationModelId: event.target.value,
                            })
                          }
                          value={specSettingsForm.generationModelId}
                        >
                          <option value="">{t("Automatic")}</option>
                          {specEligibleGenerationModels.map((model) => (
                            <option key={model.id} value={model.id}>
                              {model.displayName}
                            </option>
                          ))}
                          {isSelectedSpecGenerationModelUnavailable ? (
                            <option value={specSettingsForm.generationModelId}>
                              {t("Model unavailable: {name}", {
                                name:
                                  selectedSpecGenerationModel?.displayName ||
                                  specSettingsForm.generationModelId,
                              })}
                            </option>
                          ) : null}
                        </SettingsSelect>
                      </label>
                    </div>
                    <div className="mt-3">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Spec LLM timeout ms")}
                        </span>
                        <SettingsInput
                          aria-label={t("Spec LLM timeout ms")}
                          autoComplete="off"
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                          inputMode="numeric"
                          name="spec-llm-timeout-ms"
                          onBlur={() => {
                            try {
                              requiredPositiveInteger(
                                specSettingsFormRef.current.llmTimeoutMs,
                                t("Spec LLM timeout ms"),
                              );
                              queueSpecSettingsSave(specSettingsFormRef.current);
                            } catch (validationError) {
                              setSpecSettingsSaveError(errorMessage(validationError));
                            }
                          }}
                          onChange={(event) => {
                            const nextForm = {
                              ...specSettingsFormRef.current,
                              llmTimeoutMs: event.target.value,
                            };
                            applySpecSettingsForm(nextForm);
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") {
                              event.preventDefault();
                              try {
                                requiredPositiveInteger(
                                  specSettingsFormRef.current.llmTimeoutMs,
                                  t("Spec LLM timeout ms"),
                                );
                                queueSpecSettingsSave(specSettingsFormRef.current);
                              } catch (validationError) {
                                setSpecSettingsSaveError(errorMessage(validationError));
                              }
                            }
                          }}
                          placeholder="300000"
                          type="text"
                          value={specSettingsForm.llmTimeoutMs}
                        />
                      </label>
                    </div>
                  </fieldset>
                </div>
              </div>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Spec job history")}
                    </h3>
                    <p className="mt-1 truncate text-xs text-[var(--muted)]">
                      {t("All workspace Spec jobs")}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="inline-flex items-center gap-2 text-sm font-medium text-[var(--muted)]">
                      <SettingsInput
                        checked={showRetryableSpecJobsOnly}
                        className="size-4 accent-[var(--accent)]"
                        onChange={(event) =>
                          updateShowRetryableSpecJobsOnly(event.target.checked)
                        }
                        type="checkbox"
                      />
                      <span>{t("Only retryable Spec jobs")}</span>
                    </label>
                    <SettingsButton
                      aria-label={t("Refresh Spec job history")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
                      disabled={isLoadingSpecJobs}
                      onClick={() => void loadSpecJobs()}
                      title={t("Refresh Spec job history")}
                      type="button"
                    >
                      {isLoadingSpecJobs ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </SettingsButton>
                  </div>
                </div>

                {specJobsError ? (
                  <div className="border-b border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-2 text-sm text-[var(--danger)]">
                    {specJobsError}
                  </div>
                ) : null}

                <div className="panel-scroll overflow-x-auto" onWheel={handleSettingsTableWheel}>
                  <table className="min-w-full divide-y divide-[var(--border)] text-left text-sm">
                    <thead className="bg-[var(--surface-secondary)] text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                      <tr>
                        <th className="px-3 py-2">{t("Time")}</th>
                        <th className="px-3 py-2">{t("Workspace")}</th>
                        <th className="px-3 py-2">{t("Chat title")}</th>
                        <th className="px-3 py-2">{t("Request type")}</th>
                        <th className="px-3 py-2">{t("Status")}</th>
                        <th className="px-3 py-2">{t("Model")}</th>
                        <th className="px-3 py-2">{t("Result")}</th>
                        <th className="px-3 py-2 text-right">{t("Actions")}</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-[var(--border)]">
                      {specJobs.length === 0 ? (
                        <tr>
                          <td
                            className="px-3 py-6 text-center text-sm font-medium text-[var(--muted)]"
                            colSpan={8}
                          >
                            {isLoadingSpecJobs
                              ? t("Loading Spec job history…")
                              : t("No Spec jobs")}
                          </td>
                        </tr>
                      ) : (
                        specJobs.map((item) => {
                          const job = item.job;
                          const operationKey = `${item.workspaceId}:${job.id}`;
                          const operationType = specJobOperations[operationKey];
                          const isBusy = operationType != null;
                          const isRetrying = operationType === "retry";
                          const isDeleting = operationType === "delete";
                          const chatTitle = item.chatTitle?.trim() || null;
                          return (
                            <tr className="bg-[var(--surface)] hover:bg-[var(--surface-secondary)]" key={operationKey}>
                              <td className="px-3 py-2 align-top">
                                <span className="whitespace-nowrap text-xs font-semibold text-[var(--muted)]">
                                  {formatAuditDate(specJobTime(job), language)}
                                </span>
                              </td>
                              <td className="px-3 py-2 align-top">
                                <div className="min-w-40">
                                  <div className="font-semibold text-[var(--foreground)]">
                                    {item.workspaceName}
                                  </div>
                                  <div className="mt-1 max-w-56 truncate text-xs text-[var(--muted)]" title={item.workspacePath}>
                                    {item.workspacePath}
                                  </div>
                                </div>
                              </td>
                              <td className="px-3 py-2 align-top">
                                {chatTitle ? (
                                  <div
                                    className="max-w-48 truncate text-sm font-medium text-[var(--foreground)]"
                                    title={chatTitle}
                                  >
                                    {chatTitle}
                                  </div>
                                ) : (
                                  <span className="text-sm font-medium text-[var(--muted)]">
                                    {t("None")}
                                  </span>
                                )}
                              </td>
                              <td className="px-3 py-2 align-top">
                                {specJobTriggerLabel(job.triggerType, t)}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <CapabilityPill
                                  label={specJobStatusLabel(job.status, t)}
                                  ok={job.status === "completed"}
                                  tone={specJobStatusTone(job.status)}
                                />
                              </td>
                              <td className="px-3 py-2 align-top">
                                {job.modelId ?? t("Default")}
                              </td>
                              <td className="max-w-72 px-3 py-2 align-top text-xs text-[var(--muted)]">
                                {specJobResultLabel(job, t, language)}
                              </td>
                              <td className="px-3 py-2 align-top">
                                <div className="flex items-center justify-end gap-1.5">
                                  {job.status === "failed" && !job.hasRetry ? (
                                    <SettingsButton
                                      aria-label={t("Retry Spec job")}
                                      className="inline-flex h-8 items-center justify-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                      disabled={isBusy}
                                      onClick={() => void retrySpecJob(item.workspaceId, job.id)}
                                      title={t("Retry Spec job")}
                                      type="button"
                                    >
                                      {isRetrying ? (
                                        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                                      ) : (
                                        <Redo2 aria-hidden="true" className="size-3.5" />
                                      )}
                                      {t("Retry")}
                                    </SettingsButton>
                                  ) : null}
                                  {job.status === "failed" ? (
                                    <SettingsButton
                                      aria-label={t("Delete Spec job")}
                                      className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:border-[var(--border)] disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                      disabled={isBusy}
                                      onClick={() => void deleteSpecJob(item.workspaceId, job.id)}
                                      title={t("Delete Spec job")}
                                      type="button"
                                    >
                                      {isDeleting ? (
                                        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
                                      ) : (
                                        <Trash2 aria-hidden="true" className="size-3.5" />
                                      )}
                                    </SettingsButton>
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

                <div className="flex flex-col gap-3 border-t border-[var(--border)] px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
                  <div className="text-xs font-medium text-[var(--muted)]">
                    {specJobsTotalCount
                      ? t("Showing {start}-{end} of {total}", {
                        end: formatNumber(specJobsPageEnd, language),
                        start: formatNumber(specJobsPageStart, language),
                        total: formatNumber(specJobsTotalCount, language),
                      })
                      : t("No Spec jobs")}
                  </div>
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between lg:justify-end">
                    <label className="flex w-full items-center gap-2 text-xs font-semibold text-[var(--muted)] sm:w-auto">
                      <span>{t("Page size")}</span>
                      <SettingsInput
                        className="h-9 w-20 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                        disabled={isLoadingSpecJobs}
                        inputMode="numeric"
                        onChange={(event) => updateSpecJobsPageSize(event.target.value)}
                        value={specJobsPageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Spec job history pagination")}
                      className="flex flex-wrap items-center gap-1.5"
                    >
                      <SettingsButton
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={isLoadingSpecJobs || specJobsPage <= 1}
                        onClick={() => goToSpecJobsPage(specJobsPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </SettingsButton>
                      {specJobsPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-[var(--muted)]"
                            key={`spec-jobs-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <SettingsButton
                            aria-current={item === specJobsPage ? "page" : undefined}
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === specJobsPage
                                ? "border-[var(--accent)] bg-[var(--accent)] text-white"
                                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                              }`}
                            disabled={isLoadingSpecJobs}
                            key={item}
                            onClick={() => goToSpecJobsPage(item)}
                            title={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            type="button"
                          >
                            {formatNumber(item, language)}
                          </SettingsButton>
                        ),
                      )}
                      <SettingsButton
                        aria-label={t("Next page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={
                          isLoadingSpecJobs ||
                          specJobsTotalPages === 0 ||
                          specJobsPage >= specJobsTotalPages
                        }
                        onClick={() => goToSpecJobsPage(specJobsPage + 1)}
                        title={t("Next page")}
                        type="button"
                      >
                        <ChevronRight aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </nav>
                  </div>
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "plan" ? (
            <section className="grid gap-4">
              <form
                className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                onSubmit={(event) => void savePlanSettings(event)}
              >
                <div className="flex items-center gap-2">
                  <ListChecks aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Plan automation")}
                  </h3>
                </div>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Merge automation")}
                  </span>
                  <SettingsSelect
                    aria-label={t("Merge automation")}
                    className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                    onChange={(event) => setPlanMergeAutomationMode(event.target.value)}
                    value={planMergeAutomationMode}
                  >
                    {(settings?.plan.mergeAutomationModes ?? []).map((mode) => (
                      <option key={mode.value} value={mode.value}>
                        {t(mode.label)}
                      </option>
                    ))}
                  </SettingsSelect>
                </label>
                <label className="mt-4 block">
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Plan mode model")}
                  </span>
                  <SettingsSelect
                    aria-label={t("Plan mode model")}
                    className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                    onChange={(event) => setPlanModeModelId(event.target.value)}
                    value={planModeModelId}
                  >
                    <option value="">{t("Default agent model")}</option>
                    {enabledConfiguredModels.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.displayName || model.id}
                      </option>
                    ))}
                  </SettingsSelect>
                </label>
                <SettingsButton
                  aria-label={t("Save plan settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                </SettingsButton>
              </form>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Plan history")}
                    </h3>
                    <p className="mt-1 truncate text-xs text-[var(--muted)]">
                      {planWorkspace?.name ?? t("No workspace selected")}
                    </p>
                  </div>
                  <SettingsButton
                    aria-label={t("Refresh plan history")}
                    className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                  </SettingsButton>
                </div>

                <div className="grid gap-3 border-b border-[var(--border)] px-4 py-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Workspace")}
                    </span>
                    <SettingsSelect
                      aria-label={t("Workspace")}
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Plan status")}
                    </span>
                    <SettingsSelect
                      aria-label={t("Plan status")}
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                </div>

                {planHistoryError ? (
                  <div className="border-b border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--danger)]">
                    {planHistoryError}
                  </div>
                ) : null}

                <div className="divide-y divide-[var(--border)]">
                  {!effectivePlanHistoryWorkspaceId ? (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
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
                                <span className="text-sm font-semibold text-[var(--foreground)]">
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
                              <p className="mt-1 line-clamp-2 text-xs leading-5 text-[var(--muted)]">
                                {plan.overview}
                              </p>
                              <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-[var(--muted)]">
                                <span>{formatAuditDate(plan.updatedAt, language)}</span>
                                {plan.completedByUserAt ? (
                                  <span>{t("Archived")}: {formatAuditDate(plan.completedByUserAt, language)}</span>
                                ) : null}
                              </div>
                            </div>
                            {action ? (
                              <SettingsButton
                                aria-label={t(planActionLabel(action))}
                                className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                              </SettingsButton>
                            ) : null}
                          </div>
                        </article>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {isLoadingPlanHistory ? t("Loading plans…") : t("No plans")}
                    </div>
                  )}
                </div>

                <div className="flex flex-col gap-3 border-t border-[var(--border)] px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
                  <div className="text-xs font-medium text-[var(--muted)]">
                    {planHistoryTotalCount
                      ? t("Showing {start}-{end} of {total}", {
                        end: formatNumber(planHistoryPageEnd, language),
                        start: formatNumber(planHistoryPageStart, language),
                        total: formatNumber(planHistoryTotalCount, language),
                      })
                      : t("No plans")}
                  </div>
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between lg:justify-end">
                    <label className="flex w-full items-center gap-2 text-xs font-semibold text-[var(--muted)] sm:w-auto">
                      <span>{t("Page size")}</span>
                      <SettingsInput
                        className="h-9 w-20 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                        inputMode="numeric"
                        onChange={(event) => updatePlanHistoryPageSize(event.target.value)}
                        value={planHistoryPageSize}
                      />
                    </label>
                    <nav
                      aria-label={t("Plan history pagination")}
                      className="flex flex-wrap items-center gap-1.5"
                    >
                      <SettingsButton
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={isLoadingPlanHistory || planHistoryPage <= 1}
                        onClick={() => goToPlanHistoryPage(planHistoryPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </SettingsButton>
                      {planHistoryPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-[var(--muted)]"
                            key={`plan-history-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <SettingsButton
                            aria-current={item === planHistoryPage ? "page" : undefined}
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === planHistoryPage
                                ? "border-[var(--accent)] bg-[var(--accent)] text-white"
                                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                          </SettingsButton>
                        ),
                      )}
                      <SettingsButton
                        aria-label={t("Next page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                      </SettingsButton>
                    </nav>
                  </div>
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "memory" ? (
            <section className="grid gap-4">
              {isMemoryDialogOpen ? (
                <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && closeMemoryDialog()}>
                  <Modal.Container placement="center" size={memoryDialogMode === "edit" ? "lg" : "sm"}>
                  <Modal.Dialog
                    aria-label={
                      memoryDialogMode === "create"
                        ? t("Create memory")
                        : t("Edit memory")
                    }
                    className={`fixed left-1/2 top-1/2 z-50 max-h-[88vh] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)] ${memoryDialogMode === "edit" ? "w-[min(94vw,72rem)]" : "w-[min(92vw,34rem)]"
                      }`}
                  >
                  <form onSubmit={(event) => void saveMemoryDialog(event)}>
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          {memoryDialogMode === "create" ? (
                            <Plus aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          ) : (
                            <Pencil aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          )}
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {memoryDialogMode === "create"
                              ? t("Create memory")
                              : t("Edit memory")}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-[var(--muted)]">
                          {memoryScopeLabel(manualMemoryForm.scope, t)}
                        </div>
                      </div>
                      <SettingsButton
                        aria-label={t("Close memory dialog")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={closeMemoryDialog}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
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
                              <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                {t("Memory scope")}
                              </span>
                              <SettingsSelect
                                className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                              </SettingsSelect>
                            </label>
                            {manualMemoryForm.scope !== "global" ? (
                              <label className="block">
                                <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                  {t("Workspace")}
                                </span>
                                <SettingsSelect
                                  className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                                </SettingsSelect>
                              </label>
                            ) : null}
                            {manualMemoryForm.scope === "chat" ? (
                              <SettingsTextField
                                label={t("Chat ID")}
                                onChange={(value) =>
                                  setManualMemoryForm((current) => ({
                                    ...current,
                                    chatId: value,
                                  }))
                                }
                                placeholder="chat-…"
                                value={manualMemoryForm.chatId}
                              />
                            ) : null}
                          </>
                        ) : null}
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Memory kind")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <div className="grid gap-3 sm:grid-cols-2">
                          <SettingsTextField
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
                          <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 sm:mt-6">
                            <span className="text-sm font-semibold text-[var(--muted)]">
                              {t("Pinned memory")}
                            </span>
                            <SettingsInput
                              checked={manualMemoryForm.pinned}
                              className="size-4 accent-[var(--accent)]"
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
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Memory fact")}
                          </span>
                          <SettingsTextArea
                            className="min-h-32 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Memory metadata")}
                          </span>
                          <SettingsTextArea
                            className="min-h-28 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-xs text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          <div className="grid gap-2 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-xs text-[var(--muted)]">
                            <div className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                              {t("Memory details")}
                            </div>
                            <div className="grid gap-2 sm:grid-cols-2">
                              <div className="break-all">
                                <span className="font-semibold text-[var(--muted)]">ID: </span>
                                {selectedMemory.id}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Memory status")}:{" "}
                                </span>
                                {memoryStatusLabel(selectedMemory.status, t)}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Enabled")}:{" "}
                                </span>
                                {selectedMemory.enabled ? t("Yes") : t("No")}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Memory scope")}:{" "}
                                </span>
                                {memoryScopeLabel(selectedMemory.scope, t)}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Chat ID")}:{" "}
                                </span>
                                {selectedMemory.chatId ?? "-"}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Latest")}:{" "}
                                </span>
                                {selectedMemory.isLatest ? t("Yes") : t("No")}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Expires at")}:{" "}
                                </span>
                                {selectedMemory.expiresAt ?? "-"}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Created")}:{" "}
                                </span>
                                {selectedMemory.createdAt}
                              </div>
                              <div>
                                <span className="font-semibold text-[var(--muted)]">
                                  {t("Updated")}:{" "}
                                </span>
                                {selectedMemory.updatedAt}
                              </div>
                            </div>
                          </div>
                        ) : null}
                      </div>
                      {memoryDialogMode === "edit" ? (
                        <div className="grid min-w-0 gap-2 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                          <div className="flex items-center justify-between gap-2">
                            <h4 className="text-xs font-semibold text-[var(--muted)]">
                              {t("Memory source details")}
                            </h4>
                            <span className="font-mono text-[11px] font-semibold text-[var(--muted)]">
                              {memorySourceForms.length}
                            </span>
                          </div>
                          {memorySourceForms.length === 0 ? (
                            <div className="rounded-lg border border-dashed border-[var(--border)] bg-[var(--surface)] px-3 py-6 text-center text-sm font-medium text-[var(--muted)]">
                              {t("No memory sources")}
                            </div>
                          ) : (
                            <div className="grid max-h-[58vh] gap-3 overflow-y-auto pr-1">
                              {memorySourceForms.map((source, index) => (
                                <div
                                  className="grid gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface)] px-3 py-3"
                                  key={source.id}
                                >
                                  <div className="flex flex-wrap items-center justify-between gap-2">
                                    <div className="min-w-0">
                                      <div className="text-xs font-semibold text-[var(--muted)]">
                                        {t("Memory sources")} #{index + 1}
                                      </div>
                                      <div className="mt-1 break-all font-mono text-[11px] text-[var(--muted)]">
                                        {source.id}
                                      </div>
                                    </div>
                                  </div>
                                  <SettingsTextField
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
                                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
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
                                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
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
                      <SettingsButton
                        aria-label={
                          memoryDialogMode === "create"
                            ? t("Create memory")
                            : t("Save memory")
                        }
                        className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--accent)] px-3 text-sm font-semibold text-white hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--default)] xl:col-span-2"
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
                      </SettingsButton>
                    </div>
                  </form>
                  </Modal.Dialog>
                  </Modal.Container>
                </Modal.Backdrop>
              ) : null}

              <form
                className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                onSubmit={(event) => void saveMemorySettings(event)}
              >
                <div className="flex items-center gap-2">
                  <Bot aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Memory controls")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-3">
                  <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("General memory control")}
                    </legend>
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <p className="text-sm font-semibold text-[var(--foreground)]">
                          {t("Enable memory")}
                        </p>
                        <p className="mt-1 text-xs text-[var(--muted)]">
                          {t(
                            "Controls whether memory tools, retrieval, and extraction are available.",
                          )}
                        </p>
                      </div>
                      <label
                        aria-label={t("Enable memory")}
                        className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                      >
                        <SettingsInput
                          checked={memorySettingsForm.enabled}
                          className="size-4 accent-[var(--accent)]"
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
                    <fieldset className="rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_75%,transparent)] px-3 py-3">
                      <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                        {t("Memory extraction")}
                      </legend>
                      <div className="mb-3 flex items-start gap-2">
                        <SlidersHorizontal
                          aria-hidden="true"
                          className="mt-0.5 size-4 shrink-0 text-[var(--accent-soft-foreground)]"
                        />
                        <p className="text-xs text-[var(--muted)]">
                          {t(
                            "Controls how new facts are extracted and how long they are retained.",
                          )}
                        </p>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Extraction mode")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Extraction model")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <div className="sm:col-span-2">
                          <SettingsTextField
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
                        <div className="sm:col-span-2">
                          <SettingsTextField
                            inputMode="numeric"
                            label={t("Extraction LLM timeout ms")}
                            onChange={(value) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                extractionLlmTimeoutMs: value,
                              }))
                            }
                            placeholder="300000"
                            value={memorySettingsForm.extractionLlmTimeoutMs}
                          />
                        </div>
                      </div>
                    </fieldset>

                    <fieldset className="rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_75%,transparent)] px-3 py-3">
                      <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                        {t("Memory retrieval")}
                      </legend>
                      <div className="mb-3 flex items-start gap-2">
                        <Brain
                          aria-hidden="true"
                          className="mt-0.5 size-4 shrink-0 text-[var(--accent-soft-foreground)]"
                        />
                        <p className="text-xs text-[var(--muted)]">
                          {t("Controls how existing memory is matched into chat context.")}
                        </p>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Memory matching")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Matching model")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <div className="sm:col-span-2">
                          <SettingsTextField
                            inputMode="numeric"
                            label={t("Retrieval LLM timeout ms")}
                            onChange={(value) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                retrievalLlmTimeoutMs: value,
                              }))
                            }
                            placeholder="300000"
                            value={memorySettingsForm.retrievalLlmTimeoutMs}
                          />
                        </div>
                        <div className="sm:col-span-2">
                          <SettingsTextField
                            inputMode="numeric"
                            label={t("Memory context budget %")}
                            onChange={(value) =>
                              setMemorySettingsForm((current) => ({
                                ...current,
                                contextBudgetPercent: value,
                              }))
                            }
                            placeholder="12"
                            value={memorySettingsForm.contextBudgetPercent}
                          />
                          <p className="mt-1 text-xs text-[var(--muted)]">
                            {t(
                              "Percent of the model's available message tokens that matched memories may occupy. This is a token budget, not a fixed number of memories.",
                            )}
                          </p>
                        </div>
                      </div>
                    </fieldset>
                  </div>

                  <fieldset className="rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_75%,transparent)] px-3 py-3">
                    <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                      {t("Dream")}
                    </legend>
                    <div className="mb-3 flex items-start gap-2">
                      <Sparkles
                        aria-hidden="true"
                        className="mt-0.5 size-4 shrink-0 text-[var(--accent-soft-foreground)]"
                      />
                      <p className="text-xs text-[var(--muted)]">
                        {t(
                          "Consolidates stale, duplicate, and pending memories without creating scheduled task rows.",
                        )}
                      </p>
                    </div>
                    <div className="grid gap-3 md:grid-cols-3">
                      <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-[var(--foreground)]">
                            {t("Enable Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                          >
                            <SettingsInput
                              checked={memorySettingsForm.dream.enabled}
                              className="size-4 accent-[var(--accent)]"
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
                      <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-[var(--foreground)]">
                            {t("Enable Auto Dream")}
                          </span>
                          <label
                            aria-label={t("Enable Auto Dream")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                          >
                            <SettingsInput
                              checked={memorySettingsForm.dream.autoEnabled}
                              className="size-4 accent-[var(--accent)]"
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
                      <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-[var(--foreground)]">
                            {t("Create transcript chat")}
                          </span>
                          <label
                            aria-label={t("Create transcript chat")}
                            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)]"
                          >
                            <SettingsInput
                              checked={memorySettingsForm.dream.createTranscriptChat}
                              className="size-4 accent-[var(--accent)]"
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Dream mode")}
                        </span>
                        <SettingsSelect
                          aria-label={t("Dream mode")}
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Dream model")}
                        </span>
                        <SettingsSelect
                          aria-label={t("Dream model")}
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <SettingsTextField
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
                      <SettingsTextField
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
                      <SettingsTextField
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
                      <SettingsTextField
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
                      <SettingsTextField
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
                      <SettingsTextField
                        inputMode="numeric"
                        label={t("Dream LLM timeout ms")}
                        onChange={(value) =>
                          updateMemoryDreamForm({
                            llmTimeoutMs: value,
                          })
                        }
                        placeholder="300000"
                        value={memorySettingsForm.dream.llmTimeoutMs}
                      />
                    </div>
                  </fieldset>
                </div>
                <SettingsButton
                  aria-label={t("Save memory settings")}
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                </SettingsButton>
              </form>

              <section className="min-w-0 rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Sparkles aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                      <h3 className="text-sm font-semibold text-[var(--foreground)]">
                        {t("Dream history")}
                      </h3>
                    </div>
                    <p className="mt-1 truncate text-xs text-[var(--muted)]">
                      {t("Memory maintenance jobs and applied changes")}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    <SettingsButton
                      aria-label={t("Run workspace Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--accent)] px-3 text-sm font-semibold text-white hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                    </SettingsButton>
                    <SettingsButton
                      aria-label={t("Run global Dream now")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                    </SettingsButton>
                    <SettingsButton
                      aria-label={t("Refresh Dream history")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                      onClick={() => void loadMemoryDreamJobs()}
                      title={t("Refresh Dream history")}
                      type="button"
                    >
                      {isLoadingMemoryDreamJobs ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </SettingsButton>
                  </div>
                </div>

                <div className="mt-4 grid gap-3 md:grid-cols-4">
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="text-xs font-semibold text-[var(--muted)]">
                      {t("Last successful run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-[var(--foreground)]">
                      {latestSuccessfulMemoryDreamJob
                        ? formatAuditDate(
                          latestSuccessfulMemoryDreamJob.completedAt ??
                          latestSuccessfulMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="text-xs font-semibold text-[var(--muted)]">
                      {t("Last failed run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-[var(--foreground)]">
                      {latestFailedMemoryDreamJob
                        ? formatAuditDate(
                          latestFailedMemoryDreamJob.completedAt ??
                          latestFailedMemoryDreamJob.createdAt,
                          language,
                        )
                        : t("None")}
                    </div>
                  </div>
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="text-xs font-semibold text-[var(--muted)]">
                      {t("Next automatic run")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-[var(--foreground)]">
                      {memoryDreamNextRunEstimate}
                    </div>
                  </div>
                  <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="text-xs font-semibold text-[var(--muted)]">
                      {t("Latest applied changes")}
                    </div>
                    <div className="mt-1 text-sm font-semibold text-[var(--foreground)]">
                      {formatNumber(latestMemoryDreamChangeCount, language)}
                    </div>
                  </div>
                </div>

                {memoryDreamError ? (
                  <div className="mt-4 rounded-xl border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-sm text-[var(--warning)]">
                    {memoryDreamError}
                  </div>
                ) : null}

                {memoryDreamPartialUnavailable.length > 0 ? (
                  <div
                    className="mt-4 rounded-xl border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-sm text-[var(--warning)]"
                    role="status"
                  >
                    <div className="font-semibold">
                      {t("Some remote Dream history is unavailable")}
                    </div>
                    <ul className="mt-1 list-disc space-y-0.5 pl-5 text-[var(--warning)]">
                      {memoryDreamPartialUnavailable.map((item) => {
                        const workspaceName =
                          workspaces.find((workspace) => workspace.id === item.workspaceId)
                            ?.name ?? item.workspaceId;
                        return (
                          <li key={`${item.workspaceId}:${item.reason}`}>
                            {workspaceName}: {memoryDreamPartialUnavailableReasonLabel(item.reason, t)}
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                ) : null}

                <div
                  className="settings-table-scroll panel-scroll mt-4 overflow-x-auto rounded-xl border border-[var(--border)] bg-[var(--surface)]"
                  onWheel={handleSettingsTableWheel}
                >
                  <table className="min-w-full divide-y divide-[var(--border)] text-left text-sm">
                    <thead className="bg-[var(--surface-secondary)] text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
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
                    <tbody className="divide-y divide-[var(--border)]">
                      {memoryDreamJobs.length === 0 ? (
                        <tr>
                          <td
                            className="px-3 py-6 text-center text-sm font-medium text-[var(--muted)]"
                            colSpan={7}
                          >
                            {isLoadingMemoryDreamJobs
                              ? t("Loading Dream history…")
                              : t("No Dream jobs")}
                          </td>
                        </tr>
                      ) : (
                        memoryDreamJobs.map((job) => {
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
                              className="bg-[var(--surface)] hover:bg-[var(--surface-secondary)]"
                              key={job.id}
                            >
                              <td className="px-3 py-2 align-top">
                                <span className="text-xs font-semibold text-[var(--muted)]">
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
                              <td className="px-3 py-2 align-top text-xs text-[var(--muted)]">
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
                                  <SettingsButton
                                    aria-label={t("View details")}
                                    className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      setMemoryDreamDetailJobSnapshot(job);
                                      setMemoryDreamDetailJobId(job.id);
                                    }}
                                    title={t("View details")}
                                    type="button"
                                  >
                                    <Eye aria-hidden="true" className="size-4" />
                                  </SettingsButton>
                                  {job.transcriptChatId && transcriptWorkspaceId ? (
                                    <SettingsButton
                                      aria-label={t("Open transcript")}
                                      className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                      onClick={(event) => {
                                        event.stopPropagation();
                                        onOpenChat(transcriptWorkspaceId, job.transcriptChatId!);
                                      }}
                                      title={t("Open transcript")}
                                      type="button"
                                    >
                                      <ScrollText aria-hidden="true" className="size-4" />
                                    </SettingsButton>
                                  ) : job.transcriptChatId ? (
                                    <span className="text-xs text-[var(--muted)]">
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

                <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] pt-3 text-sm">
                  <div className="text-[var(--muted)]">
                    {t("Showing {start}-{end} of {total}", {
                      end: formatNumber(memoryDreamPageEnd, language),
                      start: formatNumber(memoryDreamPageStart, language),
                      total: formatNumber(memoryDreamMeta.totalCount, language),
                    })}
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="flex items-center gap-2 text-xs font-semibold text-[var(--muted)]">
                      <span>{t("Page size")}</span>
                      <SettingsInput
                        className="h-9 w-20 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={isLoadingMemoryDreamJobs || currentMemoryDreamPage <= 1}
                        onClick={() => goToMemoryDreamPage(currentMemoryDreamPage - 1)}
                        title={t("Previous page")}
                        type="button"
                      >
                        <ChevronLeft aria-hidden="true" className="size-4" />
                      </SettingsButton>
                      {memoryDreamPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-[var(--muted)]"
                            key={`memory-dream-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <SettingsButton
                            aria-current={
                              item === currentMemoryDreamPage ? "page" : undefined
                            }
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === currentMemoryDreamPage
                                ? "border-[var(--accent)] bg-[var(--accent)] text-white"
                                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                          </SettingsButton>
                        ),
                      )}
                      <SettingsButton
                        aria-label={t("Next page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                      </SettingsButton>
                    </nav>
                  </div>
                </div>

                {memoryDreamDetailJob ? (
                  <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && closeMemoryDreamDetailDialog()}>
                    <Modal.Container placement="center" size="lg">
                    <Modal.Dialog
                      aria-labelledby="memory-dream-detail-title"
                      className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(94vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    >
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <Sparkles aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                            <h4
                              className="text-sm font-semibold text-[var(--foreground)]"
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
                          <div className="mt-1 text-xs text-[var(--muted)]">
                            {formatAuditDate(memoryDreamDetailJob.createdAt, language)}
                          </div>
                        </div>
                        <SettingsButton
                          aria-label={t("Close Dream job details")}
                          className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                          onClick={closeMemoryDreamDetailDialog}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </SettingsButton>
                      </div>
                      <p className="text-sm text-[var(--muted)]">
                        {memoryDreamDetailJob.summary ||
                          memoryDreamDetailJob.errorMessage ||
                          t("No summary")}
                      </p>
                      <div className="mt-3 grid gap-3">
                        {isLoadingMemoryDreamChanges ? (
                          <div className="text-sm text-[var(--muted)]">
                            {t("Loading Dream changes…")}
                          </div>
                        ) : memoryDreamChangesByOperation.length === 0 ? (
                          <div className="text-sm text-[var(--muted)]">
                            {t("No Dream changes")}
                          </div>
                        ) : (
                          memoryDreamChangesByOperation.map((group) => (
                            <div
                              className="rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-3"
                              key={group.operation}
                            >
                              <div className="mb-3 flex flex-wrap items-center gap-2">
                                <h5 className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
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
                                    className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3"
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
                                        <span className="text-xs font-semibold text-[var(--muted)]">
                                          {formatNumber(
                                            Math.round(change.confidence * 100),
                                            language,
                                          )}
                                          %
                                        </span>
                                      ) : null}
                                    </div>
                                    <div className="mt-2 text-sm font-semibold text-[var(--foreground)]">
                                      {change.reason}
                                    </div>
                                    {change.targetFactIds.length ? (
                                      <div className="mt-1 break-all text-xs text-[var(--muted)]">
                                        {change.targetFactIds.join(", ")}
                                      </div>
                                    ) : null}
                                    {change.errorMessage ? (
                                      <div className="mt-2 text-sm text-[var(--danger)]">
                                        {change.errorMessage}
                                      </div>
                                    ) : null}
                                    <div className="mt-3 grid gap-3 lg:grid-cols-3">
                                      <div>
                                        <div className="text-xs font-semibold text-[var(--muted)]">
                                          {t("Before JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-[var(--muted)]">
                                          {t("Memory state before this Dream change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs text-[var(--muted)]">
                                          {memoryDreamJsonText(change.beforeJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-[var(--muted)]">
                                          {t("After JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-[var(--muted)]">
                                          {t("Memory state Dream wrote or proposed.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs text-[var(--muted)]">
                                          {memoryDreamJsonText(change.afterJson)}
                                        </pre>
                                      </div>
                                      <div>
                                        <div className="text-xs font-semibold text-[var(--muted)]">
                                          {t("Evidence JSON")}
                                        </div>
                                        <p className="mt-1 text-xs text-[var(--muted)]">
                                          {t("Sources Dream used to justify the change.")}
                                        </p>
                                        <pre className="panel-scroll mt-2 max-h-64 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs text-[var(--muted)]">
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
                    </Modal.Dialog>
                    </Modal.Container>
                  </Modal.Backdrop>
                ) : null}
              </section>

              <section className="min-w-0 rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Memory list")}
                    </h3>
                    <p className="mt-1 truncate text-xs text-[var(--muted)]">
                      {memoryScopeLabel(memoryFilter.scope, t)}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    {memoryFilter.scope !== "global" ? (
                      <SettingsButton
                        aria-label={clearFilteredMemoryLabel}
                        className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-[var(--danger)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--danger)] hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={!canClearFilteredMemories || isSavingMemory}
                        onClick={() => void clearFilteredMemories()}
                        title={clearFilteredMemoryLabel}
                        type="button"
                      >
                        <Trash2 aria-hidden="true" className="size-4" />
                        {clearFilteredMemoryLabel}
                      </SettingsButton>
                    ) : null}
                    <SettingsButton
                      aria-label={t("Create memory")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--accent)] px-3 text-sm font-semibold text-white hover:bg-[var(--accent)]"
                      onClick={openCreateMemoryDialog}
                      title={t("Create memory")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                      {t("Create memory")}
                    </SettingsButton>
                  </div>
                </div>
                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(9rem,0.8fr)_minmax(9rem,0.8fr)_minmax(8rem,0.7fr)_minmax(0,1.4fr)_auto]">
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Memory scope")}
                    </span>
                    <SettingsSelect
                      aria-label={t("Memory scope")}
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                  {memoryFilter.scope !== "global" ? (
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                        {t("Workspace")}
                      </span>
                      <SettingsSelect
                        aria-label={t("Workspace")}
                        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      </SettingsSelect>
                    </label>
                  ) : null}
                  {memoryFilter.scope === "chat" ? (
                    <SettingsTextField
                      label={t("Chat ID")}
                      onChange={(value) =>
                        updateMemoryFilter({
                          chatId: value,
                        })
                      }
                      placeholder="chat-…"
                      value={memoryFilter.chatId}
                    />
                  ) : null}
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                      {t("Memory kind")}
                    </span>
                    <SettingsSelect
                      aria-label={t("Memory kind")}
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                  <SettingsTextField
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
                      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                        {t("Memory status")}
                      </span>
                      <SettingsSelect
                        aria-label={t("Memory status")}
                        className="h-10 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                        onChange={(event) =>
                          updateMemoryFilter({
                            status: event.target.value as MemoryFilterState["status"],
                          })
                        }
                        value={memoryFilter.status}
                      >
                        <option value="active">{t("Active")}</option>
                        <option value="pending">{t("Pending review")}</option>
                      </SettingsSelect>
                    </label>
                    <SettingsButton
                      aria-label={t("Refresh memories")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                      onClick={() => void loadMemories()}
                      title={t("Refresh memories")}
                      type="button"
                    >
                      {isLoadingMemories ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </SettingsButton>
                  </div>
                </div>
                <div className="mt-4 grid gap-3">
                  {memories.length === 0 ? (
                    <div className="rounded-xl border border-dashed border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-6 text-center text-sm font-medium text-[var(--muted)]">
                      {t("No memories")}
                    </div>
                  ) : (
                    memories.map((memory) => {
                      const isMemoryEnabledPending = pendingMemoryEnabledIds.has(memory.id);
                      const memoryEnabledToggleLabel = memory.enabled
                        ? t("Disable memory {fact}", { fact: memory.fact })
                        : t("Enable memory {fact}", { fact: memory.fact });

                      return (
                      <div
                        className={`grid gap-3 rounded-xl border px-3 py-3 sm:grid-cols-[minmax(0,1fr)_auto] ${selectedMemoryId === memory.id
                            ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                            : "border-[var(--border)] bg-[var(--surface)] hover:border-[var(--accent)] hover:bg-[var(--surface-secondary)]"
                          }`}
                        key={memory.id}
                      >
                        <SettingsButton
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
                              <span className="text-xs font-semibold text-[var(--muted)]">
                                {memory.chatId}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-2 break-words text-sm font-semibold text-[var(--foreground)]">
                            {memory.fact}
                          </div>
                          <div className="mt-2 text-xs text-[var(--muted)]">
                            {memory.updatedAt}
                          </div>
                        </SettingsButton>
                        <div className="flex items-start justify-end gap-2">
                          <label
                            className="relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center"
                            title={memoryEnabledToggleLabel}
                          >
                            <SettingsInput
                              aria-label={memoryEnabledToggleLabel}
                              checked={memory.enabled}
                              className="peer sr-only"
                              disabled={
                                isMemoryEnabledPending || isLoadingMemories || isSavingMemory
                              }
                              onChange={(event) =>
                                void updateMemoryEnabled(memory, event.target.checked)
                              }
                              role="switch"
                              title={memoryEnabledToggleLabel}
                              type="checkbox"
                            />
                            <span className="absolute inset-0 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)] peer-disabled:cursor-not-allowed peer-disabled:opacity-50" />
                            <span className="absolute left-0.5 top-0.5 size-5 rounded-full bg-[var(--surface)] shadow-sm transition peer-checked:translate-x-5 peer-disabled:opacity-80" />
                          </label>
                          {memory.scope !== "global" ? (
                            <SettingsButton
                              aria-label={t("Promote one level")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                              onClick={() => promoteMemoryOneLevel(memory)}
                              title={
                                memory.scope === "chat"
                                  ? t("Promote to workspace")
                                  : t("Promote to global")
                              }
                              type="button"
                            >
                              <ArrowUp aria-hidden="true" className="size-4" />
                            </SettingsButton>
                          ) : null}
                          <SettingsButton
                            aria-label={t("Edit memory")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            onClick={() => openEditMemoryDialog(memory)}
                            title={t("Edit memory")}
                            type="button"
                          >
                            <Pencil aria-hidden="true" className="size-4" />
                          </SettingsButton>
                          <SettingsButton
                            aria-label={t("Delete memory")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)]"
                            onClick={() => void forgetMemory(memory.id)}
                            title={t("Delete memory")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </SettingsButton>
                        </div>
                      </div>
                      );
                    })
                  )}
                </div>
                <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] pt-3 text-sm">
                  <div className="text-[var(--muted)]">
                    {t("Showing {start}-{end} of {total}", {
                      end: formatNumber(memoryPageEnd, language),
                      start: formatNumber(memoryPageStart, language),
                      total: formatNumber(memoryListMeta.totalCount, language),
                    })}
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-3">
                    <label className="flex items-center gap-2 text-xs font-semibold text-[var(--muted)]">
                      <span>{t("Page size")}</span>
                      <SettingsInput
                        className="h-9 w-20 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Previous page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                      </SettingsButton>
                      {memoryPaginationItems.map((item, index) =>
                        item === "ellipsis" ? (
                          <span
                            aria-hidden="true"
                            className="inline-flex size-9 items-center justify-center text-[var(--muted)]"
                            key={`memory-ellipsis-${index}`}
                          >
                            ...
                          </span>
                        ) : (
                          <SettingsButton
                            aria-current={
                              item === memoryListMeta.page ? "page" : undefined
                            }
                            aria-label={t("Go to page {page}", {
                              page: formatNumber(item, language),
                            })}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === memoryListMeta.page
                                ? "border-[var(--accent)] bg-[var(--accent)] text-white"
                                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                          </SettingsButton>
                        ),
                      )}
                      <SettingsButton
                        aria-label={t("Next page")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                      </SettingsButton>
                    </nav>
                  </div>
                </div>
                <div className="mt-4 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                  <div className="flex items-center gap-2">
                    <CircleAlert aria-hidden="true" className="size-4 text-[var(--danger)]" />
                    <h4 className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                      {t("Extraction failures")}
                    </h4>
                  </div>
                  <div className="mt-2 grid gap-2">
                    {memoryExtractionJobs.length === 0 ? (
                      <div className="text-sm text-[var(--muted)]">
                        {t("No extraction failures")}
                      </div>
                    ) : (
                      memoryExtractionJobs.map((job) => (
                        <div
                          className="rounded-lg border border-[var(--danger)] bg-[var(--surface)] px-3 py-2"
                          key={job.id}
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <CapabilityPill label={job.status} ok={false} />
                            <CapabilityPill
                              label={job.modelId ?? t("Default")}
                              ok={false}
                            />
                            {job.chatId ? (
                              <span className="text-xs font-semibold text-[var(--muted)]">
                                {job.chatId}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-2 flex flex-wrap items-start justify-between gap-2">
                            <div className="min-w-0 flex-1 text-sm font-semibold text-[var(--danger)]">
                              {job.errorMessage ?? t("Memory extraction failed")}
                            </div>
                            <div className="flex shrink-0 items-center gap-2">
                              <SettingsButton
                                aria-label={t("Retry extraction")}
                                className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                disabled={isSavingMemory}
                                onClick={() =>
                                  void updateMemoryExtractionJob(job.id, "retry")
                                }
                                title={t("Retry extraction")}
                                type="button"
                              >
                                <Redo2 aria-hidden="true" className="size-3.5" />
                              </SettingsButton>
                              <SettingsButton
                                aria-label={t("Skip extraction failure")}
                                className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                disabled={isSavingMemory}
                                onClick={() =>
                                  void updateMemoryExtractionJob(job.id, "skip")
                                }
                                title={t("Skip extraction failure")}
                                type="button"
                              >
                                <X aria-hidden="true" className="size-3.5" />
                              </SettingsButton>
                            </div>
                          </div>
                          <div className="mt-1 text-xs text-[var(--muted)]">
                            {job.completedAt ?? job.startedAt ?? job.createdAt}
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </div>
                {selectedMemory?.status === "pending" ? (
                  <div className="mt-4 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="flex flex-wrap gap-2">
                      <SettingsButton
                        className="inline-flex h-9 items-center gap-2 rounded-lg bg-[var(--accent)] px-3 text-xs font-semibold text-white hover:bg-[var(--accent)]"
                        onClick={() => void setMemoryStatus(selectedMemory.id, "active")}
                        type="button"
                      >
                        <CheckCircle2 aria-hidden="true" className="size-3.5" />
                        {t("Approve memory")}
                      </SettingsButton>
                      <SettingsButton
                        className="inline-flex h-9 items-center gap-2 rounded-lg border border-[var(--danger)] bg-[var(--surface)] px-3 text-xs font-semibold text-[var(--danger)] hover:bg-[var(--danger-soft)]"
                        onClick={() => void setMemoryStatus(selectedMemory.id, "rejected")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-3.5" />
                        {t("Reject memory")}
                      </SettingsButton>
                    </div>
                  </div>
                ) : null}
              </section>
            </section>
          ) : null}

          {activeSection === "remote-servers" ? (
            <RemoteServersSettingsSection
              diagnostics={remoteServerDiagnostics}
              form={remoteServerForm}
              isDialogOpen={isRemoteServerDialogOpen}
              isTrustingHostKey={isTrustingHostKey}
              onCancelHostKeyTrust={cancelHostKeyTrust}
              onCloseDialog={closeRemoteServerDialog}
              onConfirmHostKeyTrust={() => void confirmHostKeyTrust()}
              onEdit={editConfiguredRemoteServer}
              onFormChange={setRemoteServerForm}
              onRunOperation={runRemoteServerOperation}
              onSave={saveRemoteServer}
              onSelectIdentityFile={selectIdentityFile}
              onStartAdding={startAddingRemoteServer}
              operationKey={remoteServerOperationKey}
              pendingHostKeyTrust={pendingHostKeyTrust}
              references={remoteServerReferences}
              servers={remoteServers}
              t={t}
            />
          ) : null}

          {activeSection === "workspaces" ? (
            <section className="grid gap-4">
              {isWorkspaceDialogOpen ? (
                <>
                  <SettingsButton
                    aria-label={t("Close workspace configuration backdrop")}
                    className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
                    onClick={() => setIsWorkspaceDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Workspace configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    onSubmit={(event) => void saveWorkspace(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Folder aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {t("Edit workspace")}
                          </h3>
                        </div>
                        {editingWorkspace ? (
                          <div className="mt-1 truncate text-xs text-[var(--muted)]">
                            {editingWorkspace.path}
                          </div>
                        ) : null}
                      </div>
                      <SettingsButton
                        aria-label={t("Close workspace configuration")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => setIsWorkspaceDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </div>
                    {error ? (
                      <div
                        className="mb-3 rounded-xl border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]"
                        role="alert"
                      >
                        {error}
                      </div>
                    ) : null}
                    <div className="space-y-3">
                      <SettingsTextField
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Path")}
                        </span>
                        <div className="flex gap-2">
                          <SettingsInput
                            autoComplete="off"
                            className="h-10 min-w-0 flex-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                            name="workspace-path"
                            onChange={(event) =>
                              setWorkspaceForm((current) => {
                                const nextPath = event.target.value;
                                return {
                                  ...current,
                                  path: nextPath,
                                  remotePath: current.serverId ? nextPath : current.remotePath,
                                };
                              })
                            }
                            placeholder="C:/Users/name/workspace"
                            value={workspaceForm.path}
                          />
                          <SettingsButton
                            aria-label={t("Choose workspace path")}
                            className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                            disabled={isSelectingWorkspaceFormPath}
                            onClick={selectWorkspaceFormPath}
                            title={t("Choose workspace path")}
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
                          </SettingsButton>
                        </div>
                      </label>
                      <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] p-3">
                        <div className="mb-3 flex items-center justify-between gap-3">
                          <div className="flex min-w-0 items-center gap-2">
                            <WorkspaceIcon
                              className="size-10 rounded-lg border border-[var(--border)] bg-[var(--surface)] object-cover p-1"
                              fallbackClassName="size-10 rounded-lg border border-[var(--border)] bg-[var(--surface)] p-2 text-[var(--accent-soft-foreground)]"
                              logoUrl={editingWorkspace?.logoUrl ?? null}
                            />
                            <div className="min-w-0">
                              <span className="block text-sm font-semibold text-[var(--foreground)]">
                                {t("Workspace icon")}
                              </span>
                              <span className="block truncate text-xs text-[var(--muted)]">
                                {editingWorkspace?.logoUrl
                                  ? t("Custom icon")
                                  : t("Folder icon")}
                              </span>
                            </div>
                          </div>
                          <SettingsButton
                            aria-label={t("Clear workspace icon")}
                            className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
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
                          </SettingsButton>
                        </div>
                        <SettingsInput
                          aria-label={t("Workspace icon file")}
                          accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                          className="sr-only"
                          onChange={handleWorkspaceLogoFileChange}
                          ref={workspaceLogoInputRef}
                          type="file"
                        />
                        <SettingsButton
                          aria-label={t("Upload icon")}
                          className="mt-2 inline-flex h-9 items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                          disabled={isSavingWorkspaceLogo}
                          onClick={() => workspaceLogoInputRef.current?.click()}
                          title={t("Upload icon")}
                          type="button"
                        >
                          <Upload aria-hidden="true" className="size-3.5" />
                          {t("Upload icon")}
                        </SettingsButton>
                      </div>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Terminal shell")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] p-3">
                        <div className="mb-3 flex items-center justify-between gap-3">
                          <span className="text-sm font-semibold text-[var(--muted)]">
                            {t("Common commands")}
                          </span>
                          <SettingsButton
                            aria-label={t("Add command")}
                            className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            onClick={addWorkspaceCommonCommand}
                            title={t("Add command")}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-4" />
                          </SettingsButton>
                        </div>
                        {workspaceForm.commonCommands.length ? (
                          <div className="space-y-2">
                            <div className="grid gap-2 pr-10 text-xs font-semibold text-[var(--muted)] sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)]">
                              <span>{t("Command name")}</span>
                              <span>{t("Command")}</span>
                            </div>
                            {workspaceForm.commonCommands.map((command, index) => (
                              <div
                                className="grid items-center gap-2 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)_2.25rem]"
                                key={index}
                              >
                                <SettingsInput
                                  aria-label={t("Command name")}
                                  autoComplete="off"
                                  className="h-9 min-w-0 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                                <SettingsInput
                                  aria-label={t("Command")}
                                  autoComplete="off"
                                  className="h-9 min-w-0 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                                <SettingsButton
                                  aria-label={t("Remove command {name}", {
                                    name: command.name || String(index + 1),
                                  })}
                                  className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                                  onClick={() => removeWorkspaceCommonCommand(index)}
                                  title={t("Remove command")}
                                  type="button"
                                >
                                  <Trash2 aria-hidden="true" className="size-4" />
                                </SettingsButton>
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </div>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                        <span className="text-sm font-semibold text-[var(--muted)]">
                          {t("Pinned workspace")}
                        </span>
                        <SettingsInput
                          checked={workspaceForm.pinned}
                          className="size-4 accent-[var(--accent)]"
                          onChange={(event) =>
                            setWorkspaceForm((current) => ({
                              ...current,
                              pinned: event.target.checked,
                            }))
                          }
                          type="checkbox"
                        />
                      </label>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                        <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-[var(--muted)]">
                          <ScrollText
                            aria-hidden="true"
                            className="size-4 shrink-0 text-[var(--accent-soft-foreground)]"
                          />
                          <span className="truncate">{t("Enable Project Spec")}</span>
                        </span>
                        <SettingsInput
                          checked={workspaceForm.specEnabled}
                          className="size-4 accent-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Save workspace")}
                        className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                        disabled={
                          isSavingWorkspace ||
                          isLoadingWorkspaceSpecSettings ||
                          !isWorkspaceSpecSettingsLoaded ||
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
                      </SettingsButton>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Workspace list")}
                  </h3>
                  <SettingsButton
                    aria-label={t("Add workspace")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                    onClick={onAddWorkspace}
                    title={t("Add workspace")}
                    type="button"
                  >
                    <Plus aria-hidden="true" className="size-4" />
                  </SettingsButton>
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {orderedWorkspaces.length ? (
                    orderedWorkspaces.map((workspace) => (
                      <div
                        className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition ${draggedWorkspaceId === workspace.id
                            ? "bg-[var(--accent-soft)] opacity-80"
                            : "bg-transparent"
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
                            className={`inline-flex size-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm ${isSavingWorkspaceOrder
                                ? "cursor-not-allowed opacity-60"
                                : "cursor-grab hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                            className="size-9 shrink-0 rounded-lg border border-[var(--border)] object-cover shadow-sm"
                            fallbackClassName="size-9 shrink-0 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] p-2 text-[var(--muted)] shadow-sm"
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
                            <div className="mt-1 truncate text-xs text-[var(--muted)]">
                              <span className="font-medium">
                                {terminalShellLabel(terminalShells, workspace.terminalShell)}
                              </span>
                              <span className="text-[var(--muted)]"> / </span>
                              <span>{workspace.path}</span>
                            </div>
                          </div>
                        </div>
                        <div className="flex gap-2 justify-end">
                          <SettingsButton
                            aria-label={t(
                              workspace.pinned
                                ? "Unpin workspace {name}"
                                : "Pin workspace {name}",
                              { name: workspace.name },
                            )}
                            className={`inline-flex size-9 items-center justify-center rounded-lg border shadow-sm ${workspace.pinned
                                ? "border-[var(--accent)] bg-[var(--accent)] text-white shadow-[var(--overlay-shadow)] hover:bg-[var(--accent)]"
                                : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                              }`}
                            disabled={isSavingWorkspaceOrder || deletingWorkspaceId === workspace.id}
                            onClick={() =>
                              void toggleWorkspacePinned(workspace, !workspace.pinned)
                            }
                            title={t(workspace.pinned ? "Unpin workspace" : "Pin workspace")}
                            type="button"
                          >
                            <Lock aria-hidden="true" className="size-4" />
                          </SettingsButton>
                          <SettingsButton
                            aria-label={t("Delete workspace {name}", {
                              name: workspace.name,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)] disabled:cursor-not-allowed disabled:opacity-50"
                            disabled={orderedWorkspaces.length <= 1 || deletingWorkspaceId === workspace.id}
                            onClick={() => setPendingDeleteWorkspace(workspace)}
                            title={t("Delete workspace")}
                            type="button"
                          >
                            {deletingWorkspaceId === workspace.id ? (
                              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                            ) : (
                              <Trash2 aria-hidden="true" className="size-4" />
                            )}
                          </SettingsButton>
                          <SettingsButton
                            aria-label={t("Edit workspace {name}", {
                              name: workspace.name,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            disabled={deletingWorkspaceId === workspace.id}
                            onClick={() => void editConfiguredWorkspace(workspace)}
                            title={t("Edit workspace")}
                            type="button"
                          >
                            <Pencil aria-hidden="true" className="size-4" />
                          </SettingsButton>
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No workspaces")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "workspaces" && pendingDeleteWorkspace ? (
            <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && setPendingDeleteWorkspace(null)}>
              <Modal.Container placement="center" size="sm">
              <Modal.Dialog
                aria-labelledby="delete-workspace-dialog-title"
                className="fixed left-1/2 top-1/2 z-50 grid w-[min(92vw,28rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3
                      className="text-base font-semibold text-[var(--foreground)]"
                      id="delete-workspace-dialog-title"
                    >
                      {t("Delete workspace?")}
                    </h3>
                    <p className="mt-1 text-sm font-medium text-[var(--foreground)]">
                      {pendingDeleteWorkspace.name}
                    </p>
                    <p className="mt-2 text-sm leading-6 text-[var(--muted)]">
                      {t("Delete workspace confirmation", {
                        name: pendingDeleteWorkspace.name,
                      })}
                    </p>
                  </div>
                  <SettingsButton
                    aria-label={t("Cancel workspace deletion")}
                    className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                    onClick={() => setPendingDeleteWorkspace(null)}
                    title={t("Close")}
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </SettingsButton>
                </div>
                <div className="flex justify-end gap-2">
                  <SettingsButton
                    className="inline-flex min-h-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm font-semibold text-[var(--muted)] hover:border-[var(--border)] hover:bg-[var(--surface-secondary)]"
                    onClick={() => setPendingDeleteWorkspace(null)}
                    type="button"
                  >
                    {t("Cancel")}
                  </SettingsButton>
                  <SettingsButton
                    aria-label={t("Confirm delete workspace")}
                    className="inline-flex min-h-10 items-center justify-center gap-2 rounded-lg bg-[var(--danger)] px-3 py-2 text-sm font-semibold text-white shadow-[0_12px_28px_rgba(190,18,60,0.22)] hover:bg-[var(--danger)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                    disabled={deletingWorkspaceId === pendingDeleteWorkspace.id}
                    onClick={() => void deleteConfiguredWorkspace(pendingDeleteWorkspace)}
                    type="button"
                  >
                    {deletingWorkspaceId === pendingDeleteWorkspace.id ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <Trash2 aria-hidden="true" className="size-4" />
                    )}
                    <span>{t("Delete workspace")}</span>
                  </SettingsButton>
                </div>
              </Modal.Dialog>
              </Modal.Container>
            </Modal.Backdrop>
          ) : null}

          {activeSection === "hooks" ? (
            <section className="grid gap-4">
              {hookRunDetail ? (
                <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && setHookRunDetail(null)}>
                  <Modal.Container placement="center" size="lg">
                  <Modal.Dialog
                    aria-label={t("Hook run detail")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,46rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Webhook aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {hookEventLabel(hookRunDetail.event, t)}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-[var(--muted)]">
                          {hookRunDetail.id}
                        </div>
                      </div>
                      <SettingsButton
                        aria-label={t("Close hook run detail")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => setHookRunDetail(null)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
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
                        <pre className="max-h-32 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
                          {hookRunDetail.stdoutPreview}
                        </pre>
                      ) : null}
                      {hookRunDetail.stderrPreview ? (
                        <pre className="max-h-32 overflow-auto rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-xs text-[var(--danger)]">
                          {hookRunDetail.stderrPreview}
                        </pre>
                      ) : null}
                      <div className="grid gap-3 lg:grid-cols-2">
                        <pre className="max-h-80 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
                          {JSON.stringify(hookRunDetail.input, null, 2)}
                        </pre>
                        <pre className="max-h-80 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
                          {JSON.stringify(hookRunDetail.output, null, 2)}
                        </pre>
                      </div>
                    </div>
                  </Modal.Dialog>
                  </Modal.Container>
                </Modal.Backdrop>
              ) : null}

              {isHookDialogOpen ? (
                <>
                  <SettingsButton
                    aria-label={t("Close hook configuration backdrop")}
                    className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
                    onClick={() => setIsHookDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Hook configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,40rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    onSubmit={(event) => void submitHookForm(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Webhook aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {hookForm.handlerIndex === null
                              ? t("Add hook")
                              : t("Edit hook")}
                          </h3>
                        </div>
                        <div className="mt-1 truncate text-xs text-[var(--muted)]">
                          {hookScope === "global" ? t("Global hooks") : selectedHookWorkspace?.name}
                        </div>
                      </div>
                      <SettingsButton
                        aria-label={t("Close hook configuration")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => setIsHookDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </div>

                    <div className="grid gap-3">
                      <div className="grid gap-3 sm:grid-cols-2">
                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Event")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <SettingsTextField
                          label={t("Matcher")}
                          onChange={(value) =>
                            setHookForm((current) => ({ ...current, matcher: value }))
                          }
                          placeholder="run_command|write_file"
                          value={hookForm.matcher}
                        />
                      </div>
                      <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                        <span className="text-sm font-semibold text-[var(--muted)]">
                          {t("Enable hook")}
                        </span>
                        <SettingsInput
                          checked={hookForm.enabled}
                          className="size-4 accent-[var(--accent)]"
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
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Handler type")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>
                        <SettingsTextField
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
                          <SettingsTextField
                            label={t("Command")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, command: value }))
                            }
                            placeholder="node scripts/hook.js"
                            value={hookForm.command}
                          />
                          <div className="grid gap-3 sm:grid-cols-2">
                            <SettingsTextField
                              label={t("Shell")}
                              onChange={(value) =>
                                setHookForm((current) => ({ ...current, shell: value }))
                              }
                              placeholder="powershell"
                              value={hookForm.shell}
                            />
                            <SettingsTextField
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
                            <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                              {t("Args")}
                            </span>
                            <SettingsTextArea
                              className="min-h-20 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          <SettingsTextField
                            label={t("URL")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, url: value }))
                            }
                            placeholder="http://127.0.0.1:8787/hook"
                            value={hookForm.url}
                          />
                          <SettingsTextField
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
                          <SettingsTextField
                            label={t("MCP server id")}
                            onChange={(value) =>
                              setHookForm((current) => ({ ...current, serverId: value }))
                            }
                            placeholder="server"
                            value={hookForm.serverId}
                          />
                          <SettingsTextField
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
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Prompt")}
                          </span>
                          <SettingsTextArea
                            className="min-h-28 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        <SettingsTextField
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
                        <SettingsTextField
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
                        <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                          <span className="text-sm font-semibold text-[var(--muted)]">
                            {t("Async")}
                          </span>
                          <SettingsInput
                            checked={hookForm.asyncHook}
                            className="size-4 accent-[var(--accent)]"
                            onChange={(event) =>
                              setHookForm((current) => ({
                                ...current,
                                asyncHook: event.target.checked,
                              }))
                            }
                            type="checkbox"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                          <span className="text-sm font-semibold text-[var(--muted)]">
                            {t("Async re-wake")}
                          </span>
                          <SettingsInput
                            checked={hookForm.asyncRewake}
                            className="size-4 accent-[var(--accent)]"
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Input override JSON")}
                        </span>
                        <SettingsTextArea
                          className="min-h-20 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-xs text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Save hook")}
                        className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                      </SettingsButton>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="grid gap-3 lg:grid-cols-[auto_minmax(0,1fr)_auto]">
                  <div className="inline-flex h-10 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] p-1">
                    {(["global", "workspace"] as HookScope[]).map((scope) => (
                      <SettingsButton
                        className={`rounded-md px-3 text-sm font-semibold ${hookScope === scope
                            ? "bg-[var(--surface)] text-[var(--accent-soft-foreground)] shadow-sm"
                            : "text-[var(--muted)] hover:text-[var(--foreground)]"
                          }`}
                        key={scope}
                        onClick={() => setHookScope(scope)}
                        type="button"
                      >
                        {scope === "global" ? t("Global") : t("Workspace")}
                      </SettingsButton>
                    ))}
                  </div>
                  <label className="min-w-0">
                    <span className="sr-only">{t("Workspace")}</span>
                    <SettingsSelect
                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                    </SettingsSelect>
                  </label>
                  <div className="flex gap-2">
                    <SettingsButton
                      aria-label={t("Add hook")}
                      className="inline-flex size-10 items-center justify-center rounded-lg bg-[var(--accent)] text-white shadow-[var(--overlay-shadow)] hover:bg-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                      disabled={!hookSettings}
                      onClick={startAddingHookHandler}
                      title={t("Add hook")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </SettingsButton>
                    <SettingsButton
                      aria-label={t("Reload hooks")}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                    </SettingsButton>
                  </div>
                </div>
                <div className="mt-3 break-all rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
                  {activeHookPath ?? t("Loading…")}
                </div>
                <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                  <span className="text-sm font-semibold text-[var(--muted)]">
                    {t("Disable all hooks")}
                  </span>
                  <SettingsInput
                    checked={Boolean(activeHookConfig?.disableAllHooks)}
                    className="size-4 accent-[var(--accent)]"
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
                <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2">
                  <span className="text-sm font-semibold text-[var(--muted)]">
                    {t("Record hook run logs")}
                  </span>
                  <SettingsInput
                    checked={generalForm.hookAuditEnabled}
                    className="size-4 accent-[var(--accent)]"
                    disabled={isSavingGeneral || !settings}
                    onChange={(event) => void saveHookAuditEnabled(event.target.checked)}
                    type="checkbox"
                  />
                </label>
              </section>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Hook rules")}
                  </h3>
                  <CapabilityPill
                    label={t("rules {count}", { count: activeHookGroups.length })}
                    ok={activeHookGroups.length > 0}
                  />
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {activeHookGroups.length ? (
                    activeHookGroups.map((entry) => (
                      <div className="px-4 py-3" key={`${entry.event}-${entry.groupIndex}`}>
                        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-sm font-semibold text-[var(--foreground)]">
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
                            <div className="mt-1 text-xs text-[var(--muted)]">
                              {t("handlers {count}", { count: entry.group.hooks.length })}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <SettingsButton
                              aria-label={t("Move hook up")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                              disabled={entry.groupIndex === 0 || isSavingHooks}
                              onClick={() => moveHookGroup(entry.event, entry.groupIndex, -1)}
                              title={t("Move hook up")}
                              type="button"
                            >
                              <ArrowUp aria-hidden="true" className="size-4" />
                            </SettingsButton>
                            <SettingsButton
                              aria-label={t("Move hook down")}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                            </SettingsButton>
                            <label className="relative inline-flex cursor-pointer items-center">
                              <SettingsInput
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
                              <span className="h-6 w-11 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)]" />
                              <span className="absolute left-1 size-4 rounded-full bg-[var(--surface)] shadow transition peer-checked:translate-x-5" />
                            </label>
                          </div>
                        </div>
                        <div className="mt-3 space-y-2">
                          {entry.group.hooks.map((handler, handlerIndex) => (
                            <div
                              className="grid gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
                              key={`${entry.event}-${entry.groupIndex}-${handlerIndex}`}
                            >
                              <div className="min-w-0">
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="font-mono text-xs font-semibold text-[var(--foreground)]">
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
                                <div className="mt-1 truncate text-xs text-[var(--muted)]">
                                  {hookHandlerSummary(handler)}
                                </div>
                              </div>
                              <div className="flex flex-wrap gap-2">
                                <SettingsButton
                                  aria-label={t("Move handler up")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                                </SettingsButton>
                                <SettingsButton
                                  aria-label={t("Move handler down")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                                </SettingsButton>
                                <SettingsButton
                                  aria-label={t("Edit hook")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                                </SettingsButton>
                                <SettingsButton
                                  aria-label={t("Delete hook")}
                                  className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
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
                                </SettingsButton>
                                <label className="relative inline-flex cursor-pointer items-center">
                                  <SettingsInput
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
                                  <span className="h-6 w-11 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)]" />
                                  <span className="absolute left-1 size-4 rounded-full bg-[var(--surface)] shadow transition peer-checked:translate-x-5" />
                                </label>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No hook rules")}
                    </div>
                  )}
                </div>
              </section>

              <div className="grid gap-4 xl:grid-cols-2">
                <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                  <div className="flex items-center justify-between gap-3">
                    <h3 className="text-sm font-semibold text-[var(--foreground)]">
                      {t("Import Claude hooks")}
                    </h3>
                    <div className="flex gap-2">
                      <SettingsButton
                        aria-label={t("Import to global hooks")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                      </SettingsButton>
                      <SettingsButton
                        aria-label={t("Import to workspace hooks")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
                        disabled={isImportingHooks || !selectedHookWorkspace}
                        onClick={() => void importClaudeHooks("workspace")}
                        title={t("Import to workspace hooks")}
                        type="button"
                      >
                        <Folder aria-hidden="true" className="size-4" />
                        {t("Workspace")}
                      </SettingsButton>
                    </div>
                  </div>
                  <p className="mt-2 text-xs text-[var(--muted)]">
                    {t("Global import reads user Claude settings; workspace import reads the selected workspace.")}
                  </p>
                  {hookImportResult ? (
                    <div
                      className={`mt-3 rounded-lg border px-3 py-2 text-sm ${hookImportResult.saved
                          ? "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]"
                          : "border-[var(--warning)] bg-[var(--warning-soft)] text-[var(--warning)]"
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
                  className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                  onSubmit={(event) => void testHooks(event)}
                >
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Test hook")}
                  </h3>
                  <div className="mt-3 grid gap-3">
                    <div className="grid gap-3 sm:grid-cols-2">
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Event")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                          onChange={(event) => setHookTestEvent(event.target.value)}
                          value={hookTestEvent}
                        >
                          {(hookSettings?.supportedEvents ?? []).map((eventName) => (
                            <option key={eventName} value={eventName}>
                              {hookEventLabel(eventName, t)}
                            </option>
                          ))}
                        </SettingsSelect>
                      </label>
                      <SettingsTextField
                        label={t("Match value")}
                        onChange={setHookTestMatcher}
                        placeholder="run_command"
                        value={hookTestMatcher}
                      />
                    </div>
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                        {t("Sample payload")}
                      </span>
                      <SettingsTextArea
                        className="min-h-28 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-xs text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                        onChange={(event) => setHookTestPayload(event.target.value)}
                        value={hookTestPayload}
                      />
                    </label>
                    <SettingsButton
                      aria-label={t("Run hook test")}
                      className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 text-sm font-semibold text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                    </SettingsButton>
                  </div>
                  {hookTestResult ? (
                    <pre className="mt-3 max-h-48 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs text-[var(--muted)]">
                      {JSON.stringify(hookTestResult, null, 2)}
                    </pre>
                  ) : null}
                </form>
              </div>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Effective hooks")}
                  </h3>
                  <CapabilityPill
                    label={t("hooks {count}", { count: hookSettings?.effective.length ?? 0 })}
                    ok={(hookSettings?.effective.length ?? 0) > 0}
                  />
                </div>
                <div className="divide-y divide-[var(--border)]">
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
                              <span className="text-sm font-semibold text-[var(--foreground)]">
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
                            <div className="mt-1 truncate text-xs text-[var(--muted)]">
                              {[hook.matcher || "*", hook.command, hook.url, hook.serverId, hook.toolName]
                                .filter(Boolean)
                                .join(" / ")}
                            </div>
                          </div>
                          <div className="text-xs text-[var(--muted)]">
                            {lastRun?.startedAt ?? hook.statusMessage ?? t("ready")}
                          </div>
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No effective hooks")}
                    </div>
                  )}
                </div>
              </section>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Recent hook runs")}
                  </h3>
                  <SettingsButton
                    aria-label={t("Refresh hook runs")}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
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
                  </SettingsButton>
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {hookSettings?.recentRuns.length ? (
                    hookSettings.recentRuns.map((run) => (
                      <SettingsButton
                        className="grid w-full gap-3 px-4 py-3 text-left hover:bg-[var(--surface-secondary)] md:grid-cols-[minmax(0,1fr)_auto]"
                        key={run.id}
                        onClick={() => void openHookRunDetail(run.id)}
                        type="button"
                      >
                        <span className="min-w-0">
                          <span className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-semibold text-[var(--foreground)]">
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
                          <span className="mt-1 block truncate text-xs text-[var(--muted)]">
                            {run.id}
                          </span>
                        </span>
                        <span className="text-xs text-[var(--muted)]">{run.startedAt}</span>
                      </SettingsButton>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
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
                  <SettingsButton
                    aria-label={t("Close provider configuration backdrop")}
                    className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
                    onClick={closeProviderDialog}
                    type="button"
                  />
                  <form
                    aria-label={t("Provider configuration")}
                    className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[min(96vw,72rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    onSubmit={(event) => void saveProvider(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <PlugZap aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {providerForm.id ? t("Edit provider") : t("Add provider")}
                          </h3>
                        </div>
                        {providerForm.id ? (
                          <div className="mt-1 truncate text-xs text-[var(--muted)]">
                            {providerForm.id}
                          </div>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <SettingsButton
                          aria-label={t("Close provider configuration")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                          onClick={closeProviderDialog}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </SettingsButton>
                      </div>
                    </div>
                    <div className="grid gap-4 lg:grid-cols-[13rem_minmax(0,1fr)]">
                      <div className="flex h-full min-h-0 flex-col rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] p-2">
                          <div className="px-2 pb-2 text-xs font-semibold text-[var(--muted)]">
                            {t("Service provider")}
                          </div>
                          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
                            {providerServices.map((service) => (
                              <SettingsButton
                                aria-pressed={selectedProviderServiceId === service.id}
                                className={`flex min-h-9 w-full items-center justify-between gap-2 rounded-lg px-2 py-2 text-left text-sm font-semibold transition ${selectedProviderServiceId === service.id
                                    ? "bg-[var(--accent)] text-white"
                                    : "text-[var(--muted)] hover:bg-[var(--surface)] hover:text-[var(--accent-soft-foreground)]"
                                  }`}
                                key={service.id}
                                onClick={() => applyProviderService(service.id)}
                                type="button"
                              >
                                <span className="min-w-0 truncate">{service.label}</span>
                                <span
                                  className={`rounded-md px-1.5 py-0.5 text-[11px] ${selectedProviderServiceId === service.id
                                      ? "bg-[color-mix(in_oklab,var(--surface)_15%,transparent)] text-white"
                                      : "bg-[var(--default)] text-[var(--muted)]"
                                    }`}
                                >
                                  {formatNumber(service.kindIds.length, language)}
                                </span>
                              </SettingsButton>
                            ))}
                          </div>
                        </div>
                      <div className="space-y-3">
                      <SettingsTextField
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Protocol")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <SettingsTextField
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
                      {providerUsesWebsocket ? (
                        <p className="text-xs text-[var(--muted)]">
                          {t(
                            "OpenAI Responses WebSocket reuses this HTTP Base URL only. Foco derives the WebSocket endpoint by converting the full Responses URL scheme (http→ws, https→wss). Prefer this protocol for long tool chains; API proxy is disabled in this release and there is no silent HTTP fallback.",
                          )}
                        </p>
                      ) : null}
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("API key")}
                        </span>
                        <span className="relative block">
                          <SettingsInput
                            autoComplete="off"
                            className={`h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 ${hasProviderKeyClearButton ? "pr-20" : "pr-11"} text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]`}
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
                          <SettingsButton
                            aria-label={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            className={`absolute ${hasProviderKeyClearButton ? "right-10" : "right-1"} top-1 inline-flex size-8 items-center justify-center rounded-md text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)] disabled:cursor-not-allowed disabled:text-[var(--muted)]`}
                            disabled={isRevealingProviderApiKey}
                            onClick={() => void handleToggleProviderApiKeyVisibility()}
                            title={
                              isProviderApiKeyVisible
                                ? t("Hide API key")
                                : t("Show API key")
                            }
                            type="button"
                          >
                            {isRevealingProviderApiKey ? (
                              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                            ) : isProviderApiKeyVisible ? (
                              <EyeOff aria-hidden="true" className="size-4" />
                            ) : (
                              <Eye aria-hidden="true" className="size-4" />
                            )}
                          </SettingsButton>
                          {hasProviderKeyClearButton ? (
                            <SettingsButton
                              aria-label={t("Clear saved API key")}
                              className={`absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md ${providerForm.clearApiKey
                                  ? "bg-[var(--danger-soft)] text-[var(--danger)]"
                                  : "text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)]"
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
                            </SettingsButton>
                          ) : null}
                        </span>
                      </label>
                      <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <RefreshCw aria-hidden="true" className="size-4 text-[var(--accent-soft-foreground)]" />
                            <h4 className="text-sm font-semibold text-[var(--foreground)]">
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
                          <label className="inline-flex items-center gap-2 text-sm font-semibold text-[var(--muted)]">
                            <SettingsInput
                              aria-label={t("Auto sync provider models")}
                              checked={providerForm.autoSyncModels}
                              className="size-4 rounded border-[var(--border)] text-[var(--accent-soft-foreground)] focus:ring-[var(--accent)]"
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
                          <SettingsTextField
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
                      <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <PlugZap aria-hidden="true" className="size-4 text-[var(--accent-soft-foreground)]" />
                            <h4 className="text-sm font-semibold text-[var(--foreground)]">
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
                        {providerUsesWebsocket ? (
                          <p className="mt-3 text-xs text-[var(--muted)]">
                            {t(
                              "AI API proxy is not supported for the OpenAI Responses WebSocket protocol in this release.",
                            )}
                          </p>
                        ) : null}
                        <div className="mt-3 grid gap-3">
                          <label className="inline-flex items-center gap-2 text-sm font-semibold text-[var(--muted)]">
                            <SettingsInput
                              aria-label={t("Enable AI API proxy")}
                              checked={providerForm.apiProxyEnabled}
                              className="size-4 rounded border-[var(--border)] text-[var(--accent-soft-foreground)] focus:ring-[var(--accent)]"
                              disabled={providerUsesWebsocket}
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
                              <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                {t("Proxy type")}
                              </span>
                              <SettingsSelect
                                className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                disabled={providerUsesWebsocket}
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
                              </SettingsSelect>
                            </label>
                            <SettingsTextField
                              disabled={providerUsesWebsocket}
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
                      <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <RefreshCw aria-hidden="true" className="size-4 text-[var(--accent-soft-foreground)]" />
                            <h4 className="text-sm font-semibold text-[var(--foreground)]">
                              {t("Model redirects")}
                            </h4>
                          </div>
                          <SettingsButton
                            className="inline-flex h-8 items-center gap-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            onClick={addProviderModelRedirect}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-3.5" />
                            {t("Add redirect")}
                          </SettingsButton>
                        </div>
                        <p className="mt-2 text-xs leading-5 text-[var(--muted)]">
                          {t("Expose provider model IDs under local model IDs.")}
                        </p>
                        <div className="mt-3 space-y-3">
                          {providerForm.modelRedirects.length ? (
                            providerForm.modelRedirects.map((redirect, redirectIndex) => (
                              <div
                                className="rounded-lg border border-[var(--border)] bg-[var(--surface)] p-3"
                                key={redirectIndex}
                              >
                                <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.5rem]">
                                  <SettingsTextField
                                    label={t("Upstream model")}
                                    onChange={(value) =>
                                      updateProviderModelRedirect(redirectIndex, { from: value })
                                    }
                                    placeholder="qwen/qwen3.6-35b-a3b"
                                    value={redirect.from}
                                  />
                                  <SettingsTextField
                                    label={t("Local model")}
                                    onChange={(value) =>
                                      updateProviderModelRedirect(redirectIndex, { to: value })
                                    }
                                    placeholder="qwen3.6-35b-a3b"
                                    value={redirect.to}
                                  />
                                  <SettingsButton
                                    aria-label={t("Delete redirect")}
                                    className="mt-6 inline-flex size-10 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] hover:bg-[var(--danger-soft)]"
                                    onClick={() => deleteProviderModelRedirect(redirectIndex)}
                                    title={t("Delete redirect")}
                                    type="button"
                                  >
                                    <Trash2 aria-hidden="true" className="size-4" />
                                  </SettingsButton>
                                </div>
                              </div>
                            ))
                          ) : (
                            <p className="rounded-lg border border-dashed border-[var(--border)] bg-[var(--surface)] px-3 py-3 text-xs text-[var(--muted)]">
                              {t("No model redirects configured.")}
                            </p>
                          )}
                        </div>
                      </div>
                      <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-2">
                            <SlidersHorizontal aria-hidden="true" className="size-4 text-[var(--accent-soft-foreground)]" />
                            <h4 className="text-sm font-semibold text-[var(--foreground)]">
                              {t("Request overrides")}
                            </h4>
                          </div>
                          <SettingsButton
                            className="inline-flex h-8 items-center gap-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2 text-xs font-semibold text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            onClick={addProviderRequestOverride}
                            type="button"
                          >
                            <Plus aria-hidden="true" className="size-3.5" />
                            {t("Add override")}
                          </SettingsButton>
                        </div>
                        <p className="mt-2 text-xs leading-5 text-[var(--muted)]">
                          {t("Override request headers or body field paths for this provider.")}
                        </p>
                        <div className="mt-3 space-y-3">
                          {providerForm.requestOverrides.length ? (
                            providerForm.requestOverrides.map((overrideRule, overrideIndex) => (
                              <div
                                className="rounded-lg border border-[var(--border)] bg-[var(--surface)] p-3"
                                key={overrideIndex}
                              >
                                <div className="grid gap-3 lg:grid-cols-[7rem_minmax(0,1fr)_8rem_minmax(0,1fr)_2.5rem]">
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                      {t("Target")}
                                    </span>
                                    <SettingsSelect
                                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                                      onChange={(event) =>
                                        updateProviderRequestOverride(overrideIndex, {
                                          target: event.target.value as ProviderRequestOverrideTarget,
                                        })
                                      }
                                      value={overrideRule.target}
                                    >
                                      <option value="header">{t("Header")}</option>
                                      <option value="body">{t("Body")}</option>
                                    </SettingsSelect>
                                  </label>
                                  <SettingsTextField
                                    label={t("Field")}
                                    onChange={(value) =>
                                      updateProviderRequestOverride(overrideIndex, { name: value })
                                    }
                                    placeholder={overrideRule.target === "header" ? "User-Agent" : "text.verbosity"}
                                    value={overrideRule.name}
                                  />
                                  <label className="block">
                                    <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                      {t("Value type")}
                                    </span>
                                    <SettingsSelect
                                      className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                                    </SettingsSelect>
                                  </label>
                                  {overrideRule.valueType === "boolean" ? (
                                    <label className="block">
                                      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                                        {t("Value")}
                                      </span>
                                      <SettingsSelect
                                        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                                        onChange={(event) =>
                                          updateProviderRequestOverride(overrideIndex, {
                                            value: event.target.value === "true",
                                          })
                                        }
                                        value={overrideRule.value ? "true" : "false"}
                                      >
                                        <option value="true">true</option>
                                        <option value="false">false</option>
                                      </SettingsSelect>
                                    </label>
                                  ) : (
                                    <SettingsTextField
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
                                  <SettingsButton
                                    aria-label={t("Delete override")}
                                    className="mt-6 inline-flex size-10 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] hover:bg-[var(--danger-soft)]"
                                    onClick={() => deleteProviderRequestOverride(overrideIndex)}
                                    title={t("Delete override")}
                                    type="button"
                                  >
                                    <Trash2 aria-hidden="true" className="size-4" />
                                  </SettingsButton>
                                </div>
                              </div>
                            ))
                          ) : (
                            <p className="rounded-lg border border-dashed border-[var(--border)] bg-[var(--surface)] px-3 py-3 text-xs text-[var(--muted)]">
                              {t("No request overrides configured.")}
                            </p>
                          )}
                        </div>
                      </div>
                      <SettingsButton
                        aria-label={t("Save provider")}
                        className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                          ) ||
                          providerForm.modelRedirects.some(
                            (redirect) => !redirect.from.trim() || !redirect.to.trim(),
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
                        <span>{t("Save provider")}</span>
                      </SettingsButton>
                    </div>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="order-1 rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Configured providers")}
                  </h3>
                  <div className="flex gap-2">
                    <SettingsButton
                      aria-label={t("Add provider")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                      onClick={startAddingProvider}
                      title={t("Add provider")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </SettingsButton>
                    <SettingsButton
                      aria-label={t("Refresh provider models")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                    </SettingsButton>
                  </div>
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {providers.length ? (
                    providers.map((provider) => {
                      const test = providerTests[provider.id];
                      const modelList = providerModelLists[provider.id];
                      const isExpanded = expandedProviderIds.has(provider.id);
                      const isProviderOperationPending = providerOperationIds.has(provider.id);
                      const providerToggleLabel = provider.enabled
                        ? t("Disable provider {name}", { name: provider.name })
                        : t("Enable provider {name}", { name: provider.name });
                      const providerDeleteLabel = t("Delete provider {name}", {
                        name: provider.name,
                      });

                      return (
                        <div className="px-4 py-3" key={provider.id}>
                          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                            <SettingsButton
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
                              className="-mx-2 -my-1 flex min-w-0 items-start gap-2 rounded-lg px-2 py-1 text-left hover:bg-[var(--surface-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent)]"
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
                                className={`mt-0.5 size-4 shrink-0 text-[var(--muted)] transition ${isExpanded ? "" : "-rotate-90"}`}
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
                                <div className="mt-1 truncate text-xs font-medium text-[var(--muted)]">
                                  {provider.id} / {provider.kindLabel}
                                </div>
                                {provider.modelSyncFilterRegex ? (
                                  <div className="mt-1 truncate font-mono text-xs text-[var(--muted)]">
                                    {t("sync regex {pattern}", {
                                      pattern: provider.modelSyncFilterRegex,
                                    })}
                                  </div>
                                ) : null}
                                {provider.modelRedirects?.length ? (
                                  <div className="mt-1 truncate font-mono text-xs text-[var(--muted)]">
                                    {provider.modelRedirects
                                      .map((redirect) => `${redirect.from} -> ${redirect.to}`)
                                      .join(", ")}
                                  </div>
                                ) : null}
                                {provider.baseUrl ? (
                                  <div className="mt-1 truncate text-xs text-[var(--muted)]">
                                    {provider.baseUrl}
                                  </div>
                                ) : null}
                              </div>
                            </SettingsButton>
                            <div className="flex flex-wrap items-center justify-end gap-2 md:self-start">
                              <label
                                className="relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center"
                                title={providerToggleLabel}
                              >
                                <SettingsInput
                                  aria-label={providerToggleLabel}
                                  checked={provider.enabled}
                                  className="peer sr-only"
                                  disabled={isProviderOperationPending}
                                  onChange={(event) =>
                                    void toggleConfiguredProviderEnabled(
                                      provider,
                                      event.target.checked,
                                    )
                                  }
                                  title={providerToggleLabel}
                                  type="checkbox"
                                />
                                <span className="absolute inset-0 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)] peer-disabled:cursor-not-allowed peer-disabled:opacity-50" />
                                <span className="absolute left-0.5 top-0.5 size-5 rounded-full bg-[var(--surface)] shadow-sm transition peer-checked:translate-x-5 peer-disabled:opacity-80" />
                              </label>
                              <SettingsButton
                                aria-label={t("Edit provider {name}", {
                                  name: provider.name,
                                })}
                                className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:opacity-60"
                                disabled={isProviderOperationPending}
                                onClick={() => editConfiguredProvider(provider)}
                                title={t("Edit provider")}
                                type="button"
                              >
                                <SlidersHorizontal aria-hidden="true" className="size-4" />
                              </SettingsButton>
                              <SettingsButton
                                aria-label={t("Test provider {name}", {
                                  name: provider.name,
                                })}
                                className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)]"
                                disabled={isProviderOperationPending || test?.status === "testing"}
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
                              </SettingsButton>
                              <SettingsButton
                                aria-label={providerDeleteLabel}
                                className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                                disabled={isProviderOperationPending}
                                onClick={() => void deleteProvider(provider.id)}
                                title={providerDeleteLabel}
                                type="button"
                              >
                                {isProviderOperationPending ? (
                                  <LoaderCircle
                                    aria-hidden="true"
                                    className="size-4 animate-spin"
                                  />
                                ) : (
                                  <Trash2 aria-hidden="true" className="size-4" />
                                )}
                              </SettingsButton>
                            </div>
                          </div>
                          {isExpanded ? (
                            <div className="mt-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-[var(--foreground)]">
                                  <ListChecks
                                    aria-hidden="true"
                                    className="size-4 shrink-0 text-[var(--accent-soft-foreground)]"
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
                                <div className="mt-3 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
                                  {modelList.message}
                                </div>
                              ) : modelList?.status === "ok" ? (
                                modelList.models.length ? (
                                  <div className="mt-3 max-h-56 overflow-y-auto rounded-md border border-[var(--border)] bg-[var(--surface)]">
                                    {modelList.models.map((modelId, modelIndex) => (
                                      <div
                                        className="border-b border-[var(--border)] px-3 py-2 font-mono text-xs text-[var(--muted)] last:border-b-0"
                                        key={`${modelId}-${modelIndex}`}
                                      >
                                        {modelId}
                                      </div>
                                    ))}
                                  </div>
                                ) : (
                                  <div className="mt-3 rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--muted)]">
                                    {t("No provider models returned")}
                                  </div>
                                )
                              ) : (
                                <div className="mt-3 flex items-center gap-2 rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--muted)]">
                                  <LoaderCircle
                                    aria-hidden="true"
                                    className="size-4 animate-spin"
                                  />
                                  {t("Loading provider models…")}
                                </div>
                              )}
                            </div>
                          ) : null}
                          {test ? (
                            <div
                              className={`mt-3 rounded-lg border px-3 py-2 text-sm ${test.status === "ok"
                                  ? "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]"
                                  : test.status === "testing"
                                    ? "border-[var(--border)] bg-[var(--surface-secondary)] text-[var(--muted)]"
                                    : "border-[var(--danger)] bg-[var(--danger-soft)] text-[var(--danger)]"
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
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
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
                  <SettingsButton
                    aria-label={t("Close MCP server configuration backdrop")}
                    className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
                    onClick={() => setIsMcpDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("MCP server configuration")}
                    className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    onSubmit={(event) => void saveMcpServer(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Server aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {mcpForm.id ? t("Edit MCP server") : t("Add MCP server")}
                          </h3>
                        </div>
                        {mcpForm.id ? (
                          <div className="mt-1 truncate text-xs text-[var(--muted)]">
                            {mcpForm.id}
                          </div>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <label className="relative inline-flex cursor-pointer items-center">
                          <SettingsInput
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
                          <span className="h-6 w-11 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)]" />
                          <span className="absolute left-1 size-4 rounded-full bg-[var(--surface)] shadow transition peer-checked:translate-x-5" />
                        </label>
                        {mcpForm.id ? (
                          <SettingsButton
                            aria-label={t("Delete MCP server")}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                            disabled={isSavingMcpServer}
                            onClick={() => void deleteMcpServer(mcpForm.id)}
                            title={t("Delete MCP server")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </SettingsButton>
                        ) : null}
                        <SettingsButton
                          aria-label={t("Close MCP server configuration")}
                          className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                          onClick={() => setIsMcpDialogOpen(false)}
                          title={t("Close")}
                          type="button"
                        >
                          <X aria-hidden="true" className="size-4" />
                        </SettingsButton>
                      </div>
                    </div>
                    <div className="space-y-3">
                      <SettingsTextField
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
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Transport")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                        </SettingsSelect>
                      </label>
                      <label className="block">
                        <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                          {t("Execution host")}
                        </span>
                        <SettingsSelect
                          className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                          onChange={(event) =>
                            setMcpForm((current) => ({
                              ...current,
                              executionHost: event.target.value as McpServerFormState["executionHost"],
                            }))
                          }
                          value={mcpForm.executionHost}
                        >
                          <option value="auto">{t("Auto")}</option>
                          <option value="local">{t("Local")}</option>
                          <option value="workspace">{t("Workspace")}</option>
                        </SettingsSelect>
                      </label>
                      {mcpForm.transport === "streamable-http" ? (
                        <SettingsTextField
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
                          <SettingsTextField
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
                            <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                              {t("Args")}
                            </span>
                            <SettingsTextArea
                              className="min-h-24 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                      <SettingsButton
                        aria-label={t("Save MCP server")}
                        className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-[var(--foreground)] text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
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
                      </SettingsButton>
                    </div>
                  </form>
                </>
              ) : null}

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("MCP servers")}
                  </h3>
                  <div className="flex gap-2">
                    <SettingsButton
                      aria-label={t("Add MCP server")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                      onClick={startAddingMcpServer}
                      title={t("Add MCP server")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </SettingsButton>
                    <SettingsButton
                      aria-label={t("Reload MCP settings")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
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
                    </SettingsButton>
                  </div>
                </div>
                <div className="divide-y divide-[var(--border)]">
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
                            <div className="mt-1 truncate text-xs font-medium text-[var(--muted)]">
                              {server.id} / {server.transportLabel}
                            </div>
                            <div className="mt-1 truncate text-xs text-[var(--muted)]">
                              {server.transport === "streamable-http"
                                ? server.url
                                : [server.command, ...server.args].filter(Boolean).join(" ")}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <SettingsButton
                              aria-label={t("Edit MCP server {name}", {
                                name: server.name,
                              })}
                              className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                              onClick={() => editConfiguredMcpServer(server)}
                              title={t("Edit MCP server")}
                              type="button"
                            >
                              <SlidersHorizontal aria-hidden="true" className="size-4" />
                            </SettingsButton>
                          </div>
                        </div>
                        {server.error ? (
                          <div className="mt-3 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
                            {server.error}
                          </div>
                        ) : null}
                        <Warnings warnings={server.warnings} />
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No configured MCP servers")}
                    </div>
                  )}
                </div>
              </section>
            </section>
          ) : null}

          {activeSection === "skills" ? (
            <section className="grid gap-4">
              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                    {t("Skill translation model")}
                  </span>
                  <SettingsSelect
                    aria-label={t("Skill translation model")}
                    className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                    disabled={isSavingSkills}
                    onChange={(event) => changeSkillTranslationModel(event.target.value)}
                    value={skills?.translationModelId ?? ""}
                  >
                    <option value="">{t("No translation model")}</option>
                    {configuredModelsByName.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.displayName}
                      </option>
                    ))}
                  </SettingsSelect>
                </label>
              </section>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Detected skills")}
                  </h3>
                  <div className="flex items-center gap-2">
                    {updateableStoreSkills.length ? (
                      <SettingsButton
                        aria-label={t("Update all store skills")}
                        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                        disabled={
                          isUpdatingAllSkills ||
                          updatingSkillKey !== null ||
                          isRefreshingSkills
                        }
                        onClick={() => void updateAllStoreSkills()}
                        title={t("Updates overwrite local changes")}
                        type="button"
                      >
                        {isUpdatingAllSkills ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <RefreshCw aria-hidden="true" className="size-4" />
                        )}
                        <span>
                          {isUpdatingAllSkills ? t("Updating…") : t("Update all store skills")}
                        </span>
                      </SettingsButton>
                    ) : null}
                    <CapabilityPill
                      label={t("skills {count}", {
                        count: detectedSkillRows.length,
                      })}
                      ok={detectedSkillRows.length > 0}
                    />
                    <SettingsButton
                      aria-label={t("Refresh skill discovery")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                      disabled={
                        isRefreshingSkills || isUpdatingAllSkills || updatingSkillKey !== null
                      }
                      onClick={() => void refreshSkills()}
                      title={t("Refresh skill discovery")}
                      type="button"
                    >
                      {isRefreshingSkills ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      )}
                    </SettingsButton>
                  </div>
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {detectedSkillRows.length ? (
                    detectedSkillRows.map((row) => {
                      const { skill } = row;
                      const isRemoteSkill = row.source === "remote";
                      const enabled = isRemoteSkill
                        ? skill.enabled
                        : currentEnabledSkillIds.has(skill.key);
                      const isStoreUpdateable =
                        !isRemoteSkill && Boolean(skill.store?.updateable);
                      const isUpdatingSkill = updatingSkillKey === skill.key;
                      return (
                        <div className="px-4 py-3" key={row.key}>
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
                                {isStoreUpdateable ? (
                                  <CapabilityPill
                                    label={t("Store-installed skill")}
                                    ok={true}
                                    tone="ok"
                                  />
                                ) : null}
                              </div>
                              <div className="mt-1 truncate text-xs font-medium text-[var(--muted)]">
                                {skill.key}
                              </div>
                              <div className="mt-1 break-words text-xs text-[var(--muted)]">
                                {skill.description}
                              </div>
                              <div className="mt-1 break-all text-xs text-[var(--muted)]">
                                {skill.path}
                              </div>
                            </div>
                            <div className="flex items-center gap-2 justify-self-start md:justify-self-end">
                              {!isRemoteSkill && isStoreUpdateable ? (
                                <SettingsButton
                                    aria-label={t("Update skill {name}", { name: skill.name })}
                                    className="inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2.5 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                    disabled={
                                      isSavingSkills ||
                                      isRefreshingSkills ||
                                      isUpdatingAllSkills ||
                                      (updatingSkillKey !== null && !isUpdatingSkill)
                                    }
                                    onClick={() => void updateSkill(skill)}
                                    title={t("Updates overwrite local changes")}
                                    type="button"
                                  >
                                    {isUpdatingSkill ? (
                                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                                    ) : (
                                      <RefreshCw aria-hidden="true" className="size-4" />
                                    )}
                                    <span>{isUpdatingSkill ? t("Updating…") : t("Update skill")}</span>
                                </SettingsButton>
                              ) : null}
                                <SettingsButton
                                  aria-label={t("Delete skill {name}", { name: skill.name })}
                                  className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                                  disabled={
                                    isSavingSkills ||
                                    isRefreshingSkills ||
                                    isUpdatingAllSkills ||
                                    updatingSkillKey !== null
                                  }
                                  onClick={() => {
                                    if (isRemoteSkill && row.workspace) {
                                      void deleteRemoteWorkspaceSkill(row.workspace.id, skill);
                                      return;
                                    }
                                    void deleteSkill(skill);
                                  }}
                                  title={t("Delete skill")}
                                  type="button"
                                >
                                  <Trash2 aria-hidden="true" className="size-4" />
                                </SettingsButton>
                                <label className="relative inline-flex cursor-pointer items-center">
                                  <SettingsInput
                                    aria-label={t("Enable skill {name}", {
                                      name: skill.name,
                                    })}
                                    checked={enabled}
                                    className="peer sr-only"
                                    disabled={
                                      isSavingSkills ||
                                      isRefreshingSkills ||
                                      isUpdatingAllSkills ||
                                      updatingSkillKey !== null ||
                                      !skill.canEnable
                                    }
                                    onChange={(event) => {
                                      if (isRemoteSkill && row.workspace) {
                                        void toggleRemoteWorkspaceSkill(
                                          row.workspace.id,
                                          skill,
                                          event.target.checked,
                                        );
                                        return;
                                      }
                                      toggleSkill(skill.key, event.target.checked);
                                    }}
                                    type="checkbox"
                                  />
                                  <span className="h-6 w-11 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)]" />
                                  <span className="absolute left-1 size-4 rounded-full bg-[var(--surface)] shadow transition peer-checked:translate-x-5" />
                                </label>
                              </div>
                          </div>
                          <Warnings warnings={skill.warnings} />
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No detected skills")}
                    </div>
                  )}
                </div>
                {remoteWorkspaceSkillCatalogs.some((catalog) => catalog.status === "loading") ? (
                  <div className="border-t border-[var(--border)] px-4 py-3 text-sm text-[var(--muted)]">
                    {t("Loading remote workspace skills…")}
                  </div>
                ) : null}
                {remoteWorkspaceSkillCatalogs.some(
                  (catalog) => catalog.error || catalog.refreshError,
                ) ? (
                  <div className="space-y-2 border-t border-[var(--border)] px-4 py-3">
                    {remoteWorkspaceSkillCatalogs
                      .filter((catalog) => catalog.error || catalog.refreshError)
                      .map((catalog) => {
                        const message = catalog.error ?? catalog.refreshError;
                        const serverLabel =
                          catalog.workspace.serverName ?? catalog.workspace.serverId;

                        return (
                          <div
                            className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]"
                            key={`remote-error:${catalog.workspace.id}`}
                            role="alert"
                          >
                            <div className="min-w-0 break-words">
                              <span className="font-medium">{catalog.workspace.name}</span>
                              {serverLabel ? ` · ${serverLabel}` : null}: {message}
                            </div>
                            <SettingsButton
                              className="inline-flex h-8 shrink-0 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] px-2.5 text-xs font-semibold text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)]"
                              onClick={() => retryRemoteWorkspaceSkillCatalog(catalog.workspace.id)}
                              type="button"
                            >
                              {t("Retry")}
                            </SettingsButton>
                          </div>
                        );
                      })}
                  </div>
                ) : null}
              </section>

              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="flex items-center gap-2">
                  <Wrench aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Skill locations")}
                  </h3>
                </div>
                <div className="mt-4 grid gap-2">
                  {skillLocations.length ? (
                    skillLocations.map((location) => (
                      <div
                        className="flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-xs font-medium text-[var(--muted)]"
                        key={location.id}
                      >
                        <span className="min-w-0 break-all">{location.path}</span>
                        <label className="relative inline-flex shrink-0 cursor-pointer items-center">
                          <SettingsInput
                            aria-label={t("Enable skill location {path}", {
                              path: location.path,
                            })}
                            checked={location.enabled}
                            className="peer sr-only"
                            disabled={
                              isSavingSkills ||
                              isRefreshingSkills ||
                              isUpdatingAllSkills ||
                              updatingSkillKey !== null
                            }
                            onChange={(event) =>
                              toggleSkillLocation(location.id, event.target.checked)
                            }
                            title={t("Enable skill location {path}", {
                              path: location.path,
                            })}
                            type="checkbox"
                          />
                          <span className="h-6 w-11 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)]" />
                          <span className="absolute left-1 size-4 rounded-full bg-[var(--surface)] shadow transition peer-checked:translate-x-5" />
                        </label>
                      </div>
                    ))
                  ) : (
                    <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-sm text-[var(--muted)]">
                      {t("Loading…")}
                    </div>
                  )}
                </div>
                {skills?.errors.length ? (
                  <div className="mt-4 space-y-2">
                    {skills.errors.map((skillError) => (
                      <div
                        className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]"
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
              <div className="min-w-0 rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
                <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {t("Models")}
                  </h3>
                  <div className="flex gap-2">
                    <SettingsButton
                      aria-label={t("Add model")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                      onClick={startAddingModel}
                      title={t("Add model")}
                      type="button"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                    </SettingsButton>
                  </div>
                </div>
                <div className="divide-y divide-[var(--border)]">
                  {configuredModelsByName.length ? (
                    configuredModelsByName.map((model) => (
                      <div
                        className="grid grid-cols-1 items-center gap-3 px-4 py-2.5 transition sm:grid-cols-[minmax(0,1fr)_auto]"
                        key={model.id}
                      >
                        <div className="min-w-0 space-y-1.5 overflow-hidden sm:flex sm:items-center sm:gap-2 sm:space-y-0">
                          <div className="flex min-w-0 items-baseline gap-2 overflow-hidden">
                            <span
                              className="min-w-0 truncate text-sm font-semibold"
                              title={model.displayName}
                            >
                              {model.displayName}
                            </span>
                            <span
                              aria-hidden="true"
                              className="shrink-0 text-xs text-[var(--muted)]"
                            >
                              /
                            </span>
                            <span
                              className="min-w-0 truncate text-xs font-medium text-[var(--muted)]"
                              title={model.id}
                            >
                              {model.id}
                            </span>
                          </div>
                          <div className="flex min-w-0 flex-wrap items-center gap-1.5 sm:flex-nowrap sm:gap-2">
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
                        </div>
                        <div className="flex shrink-0 items-center justify-end gap-2 sm:self-center">
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
                            <SettingsInput
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
                            <span className="absolute inset-0 rounded-full bg-[var(--default)] transition peer-checked:bg-[var(--accent)] peer-disabled:cursor-not-allowed peer-disabled:opacity-50" />
                            <span className="absolute left-0.5 top-0.5 size-5 rounded-full bg-[var(--surface)] shadow-sm transition peer-checked:translate-x-5 peer-disabled:opacity-80" />
                          </label>
                          <SettingsButton
                            aria-label={t("Test model {name}", {
                              name: model.displayName,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)] disabled:cursor-not-allowed disabled:opacity-60"
                            disabled={modelTests[model.id]?.testing === true}
                            onClick={() => void testModel(model)}
                            title={t("Test model")}
                            type="button"
                          >
                            {modelTests[model.id]?.testing ? (
                              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                            ) : (
                              <RadioTower aria-hidden="true" className="size-4" />
                            )}
                          </SettingsButton>
                          <SettingsButton
                            aria-label={t("Edit model {name}", {
                              name: model.displayName,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                            onClick={() => editConfiguredModel(model)}
                            title={t("Edit model")}
                            type="button"
                          >
                            <SlidersHorizontal aria-hidden="true" className="size-4" />
                          </SettingsButton>
                          <SettingsButton
                            aria-label={t("Delete model {name}", {
                              name: model.displayName,
                            })}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--danger)] bg-[var(--surface)] text-[var(--danger)] shadow-sm hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                            disabled={isSaving}
                            onClick={() => void deleteModel(model.id)}
                            title={t("Delete model")}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="size-4" />
                          </SettingsButton>
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="px-4 py-6 text-sm text-[var(--muted)]">
                      {t("No configured models")}
                    </div>
                  )}
                </div>
              </div>

              {isModelDialogOpen ? (
                <>
                  <SettingsButton
                    aria-label={t("Close model configuration backdrop")}
                    className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
                    onClick={() => setIsModelDialogOpen(false)}
                    type="button"
                  />
                  <form
                    aria-label={t("Model configuration")}
                    className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(96vw,70rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    onSubmit={(event) => void saveModel(event)}
                  >
                    <div className="mb-4 flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <SlidersHorizontal
                            aria-hidden="true"
                            className="size-5 text-[var(--accent-soft-foreground)]"
                          />
                          <h3 className="text-sm font-semibold text-[var(--foreground)]">
                            {editingModel ? t("Edit model") : t("Add model")}
                          </h3>
                        </div>
                        {selectedMetadata ? (
                          <div className="mt-1 truncate text-xs text-[var(--muted)]">
                            {selectedMetadata.key}
                          </div>
                        ) : null}
                      </div>
                      <SettingsButton
                        aria-label={t("Close model configuration")}
                        className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => setIsModelDialogOpen(false)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </div>

                    <div className="grid gap-4 lg:grid-cols-[minmax(0,1.05fr)_minmax(20rem,0.95fr)]">
                      <div className="space-y-3">
                        <div className="grid gap-3 sm:grid-cols-2">
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                              {t("Model developer")}
                            </span>
                            <SettingsSelect
                              className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                              onChange={(event) => selectModelDeveloper(event.target.value)}
                              value={selectedModelDeveloper}
                            >
                              <option value="">{t("Select model developer")}</option>
                              {modelDeveloperOptions.map((developer) => (
                                <option key={developer} value={developer}>
                                  {developer}
                                </option>
                              ))}
                            </SettingsSelect>
                          </label>
                          <label className="block">
                            <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                              {t("Model id")}
                            </span>
                            <SettingsSelect
                              className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                              disabled={!modelIdOptions.length && !editingModel}
                              onChange={(event) => updateModelId(event.target.value)}
                              value={form.modelId}
                            >
                              <option value="">{t("Select model id")}</option>
                              {modelIdOptions.map((model) => (
                                <option key={model.key} value={model.value}>
                                  {model.value}
                                </option>
                              ))}
                              {editingModel &&
                                form.modelId &&
                                !modelIdOptions.some(
                                  (model) => model.value === form.modelId,
                                ) ? (
                                <option value={form.modelId}>{form.modelId}</option>
                              ) : null}
                            </SettingsSelect>
                          </label>
                        </div>

                        {selectedModelDeveloper && !developerModels.length ? (
                          <div className="rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-sm text-[var(--warning)]">
                            {t("No cached models for developer")}
                          </div>
                        ) : null}

                        <SettingsTextField
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
                          <SettingsTextField
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
                          <SettingsTextField
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
                          <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                            <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                              {t("Input types")}
                            </legend>
                            <div className="grid gap-2">
                              {inputModalityOptions.map((modality) => (
                                <label
                                  className="flex items-center justify-between gap-3 rounded-lg bg-[var(--surface)] px-3 py-2 text-sm font-medium text-[var(--muted)]"
                                  key={modality}
                                >
                                  <span>{t(modality)}</span>
                                  <SettingsInput
                                    checked={form.inputModalities.includes(modality)}
                                    className="size-4 accent-[var(--accent)]"
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
                          <fieldset className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                            <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                              {t("Output types")}
                            </legend>
                            <div className="grid gap-2">
                              {outputModalityOptions.map((modality) => (
                                <label
                                  className="flex items-center justify-between gap-3 rounded-lg bg-[var(--surface)] px-3 py-2 text-sm font-medium text-[var(--muted)]"
                                  key={modality}
                                >
                                  <span>{t(modality)}</span>
                                  <SettingsInput
                                    checked={form.outputModalities.includes(modality)}
                                    className="size-4 accent-[var(--accent)]"
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
                          <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-xs text-[var(--muted)]">
                            <div className="truncate font-semibold text-[var(--foreground)]">
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
                        <div className="rounded-xl border border-[var(--border)] px-3 py-3">
                          <div className="mb-2 flex items-center justify-between gap-2">
                            <div className="text-xs font-semibold text-[var(--muted)]">
                              {t("Providers")}
                            </div>
                            <SettingsButton
                              aria-label={t("Add provider")}
                              className="inline-flex size-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                              onClick={startAddingProviderFromModel}
                              title={t("Add provider")}
                              type="button"
                            >
                              <Plus aria-hidden="true" className="size-4" />
                            </SettingsButton>
                          </div>
                          <div className="panel-scroll max-h-56 space-y-2 overflow-y-auto pr-1">
                            {providers.length ? (
                              providers.map((provider) => {
                                const providerSupportsCurrentModel =
                                  supportedModelProviderIdSet.has(provider.id);

                                return (
                                  <label
                                    className={`flex items-center justify-between gap-3 rounded-lg bg-[var(--surface-secondary)] px-3 py-2 ${providerSupportsCurrentModel ? "" : "opacity-60"}`}
                                    key={provider.id}
                                  >
                                    <span className="min-w-0">
                                      <span className="block truncate text-sm font-semibold text-[var(--muted)]">
                                        {provider.name}
                                      </span>
                                      <span className="block truncate text-xs text-[var(--muted)]">
                                        {providerSupportsCurrentModel
                                          ? provider.kindLabel
                                          : t("Model not supported")}
                                      </span>
                                    </span>
                                    <SettingsInput
                                      aria-label={provider.name}
                                      checked={selectedProviderIds.has(provider.id)}
                                      className="size-4 accent-[var(--accent)] disabled:cursor-not-allowed"
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
                              <SettingsButton
                                className="flex w-full items-center justify-between rounded-lg border border-dashed border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-left text-sm text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                                onClick={startAddingProviderFromModel}
                                type="button"
                              >
                                <span>{t("No providers")}</span>
                                <Plus aria-hidden="true" className="size-4" />
                              </SettingsButton>
                            )}
                          </div>
                        </div>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Active provider")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
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
                          </SettingsSelect>
                        </label>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Thinking level")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] disabled:cursor-not-allowed disabled:bg-[var(--surface-secondary)] disabled:text-[var(--muted)]"
                            disabled={!modelThinkingEnabled}
                            onChange={(event) =>
                              setForm((current) => ({
                                ...current,
                                thinkingLevel: event.target.value,
                              }))
                            }
                            value={
                              modelThinkingEnabled &&
                                isModelThinkingLevelSupported(
                                  selectedMetadata ?? editingModel,
                                  form.thinkingLevel,
                                )
                                ? form.thinkingLevel
                                : ""
                            }
                          >
                            {modelThinkingEnabled ? (
                              <option value="">{t("Model default")}</option>
                            ) : (
                              <option value="">{t("None")}</option>
                            )}
                            {modelThinkingOptions.map((level) => (
                              <option key={level.value} value={level.value}>
                                {t(level.label)}
                              </option>
                            ))}
                          </SettingsSelect>
                        </label>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("Web search mode")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                            onChange={(event) =>
                              setForm((current) => ({
                                ...current,
                                webSearchMode: event.target
                                  .value as import("../../api/types").WebSearchMode,
                              }))
                            }
                            value={form.webSearchMode || "auto"}
                          >
                            <option value="auto">{t("Auto (native when confirmed)")}</option>
                            <option value="native">{t("Native only")}</option>
                            <option value="function">{t("Function fallback only")}</option>
                            <option value="disabled">{t("Disabled for this model")}</option>
                          </SettingsSelect>
                          <p className="mt-1.5 text-xs leading-5 text-[var(--muted)]">
                            {t(
                              "Auto prefers confirmed provider-native search; unknown capability falls back to Tavily/Brave when available.",
                            )}
                          </p>
                          {settings?.webSearch && form.webSearchMode !== "disabled" ? (
                            <p className="mt-1 text-xs leading-5 text-[var(--muted)]">
                              {settings.webSearch.enabled
                                ? settings.webSearch.fallbackAvailable
                                  ? t(
                                      "Global web search is on. Function fallback key is configured.",
                                    )
                                  : t(
                                      "Global web search is on. Function fallback key is not configured.",
                                    )
                                : t("Global web search master switch is off.")}
                            </p>
                          ) : null}
                        </label>

                        <label className="block">
                          <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                            {t("System prompt")}
                          </span>
                          <SettingsSelect
                            className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
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
                          </SettingsSelect>
                        </label>

                        {enabledNeedsLimits ? (
                          <div className="flex items-center gap-2 rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-sm text-[var(--warning)]">
                            <CircleAlert
                              aria-hidden="true"
                              className="size-4 shrink-0"
                            />
                            {t("Fill both limits before enabling.")}
                          </div>
                        ) : null}

                        <SettingsButton
                          aria-label={t("Save model")}
                          className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                          disabled={
                            isSaving ||
                            enabledNeedsLimits ||
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
                          <span>{t("Save model")}</span>
                        </SettingsButton>
                      </div>
                    </div>
                  </form>
                </>
              ) : null}

            </section>
          ) : null}
          {activeSection === "about" ? (
            <>
              <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] px-4 py-4 shadow-[var(--overlay-shadow)]">
                <div className="grid gap-5 md:grid-cols-[minmax(0,1fr)_minmax(16rem,0.8fr)] md:items-start">
                  <div className="min-w-0 space-y-4">
                    <div className="flex items-center gap-3">
                      <div
                        aria-hidden="true"
                        className="size-12 shrink-0 overflow-hidden rounded-xl shadow-[var(--overlay-shadow)] [&>svg]:block [&>svg]:size-full"
                        dangerouslySetInnerHTML={{ __html: focoLogoSvg }}
                      />
                      <div className="min-w-0">
                        <h3 className="text-xl font-semibold text-[var(--foreground)]">Foco</h3>
                        <p className="mt-1 text-sm font-medium text-[var(--muted)]">
                          {t("Local-first AI coding workspace")}
                        </p>
                      </div>
                    </div>
                    <div className="flex flex-wrap items-center gap-2">
                      <SettingsButton
                        className="inline-flex min-h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                        disabled={isCheckingUpdate}
                        onClick={() => void checkForUpdate()}
                        type="button"
                      >
                        {isCheckingUpdate ? (
                          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                        ) : (
                          <RefreshCw aria-hidden="true" className="size-4" />
                        )}
                        <span>{t("Check for updates")}</span>
                      </SettingsButton>
                      {settings?.update.updateAvailable ? (
                        <SettingsButton
                          className="inline-flex min-h-10 items-center justify-center gap-2 rounded-lg border border-[var(--accent)] bg-[var(--accent-soft)] px-3 py-2 text-sm font-semibold text-[var(--accent-soft-foreground)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] disabled:cursor-not-allowed disabled:opacity-60"
                          disabled={isInstallingUpdate}
                          onClick={() => void installUpdate()}
                          type="button"
                        >
                          {isInstallingUpdate ? (
                            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                          ) : (
                            <Download aria-hidden="true" className="size-4" />
                          )}
                          <span>{isInstallingUpdate ? t("Installing update…") : t("Install update")}</span>
                        </SettingsButton>
                      ) : null}
                    </div>
                    <label className="inline-flex items-center gap-2 text-sm font-medium text-[var(--muted)]">
                      <SettingsInput
                        checked={Boolean(settings?.update.autoCheckEnabled)}
                        className="size-4 rounded border-[var(--border)] text-[var(--accent-soft-foreground)] focus:ring-[var(--accent)]"
                        disabled={isSavingUpdateSettings}
                        onChange={(event) => void saveAutoUpdateCheck(event.target.checked)}
                        type="checkbox"
                      />
                      <span>{t("Automatically check for updates")}</span>
                    </label>
                    {settings?.update.error ? (
                      <div className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
                        {settings.update.error}
                      </div>
                    ) : null}
                  </div>
                  <dl className="grid gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
                    <div className="flex items-center justify-between gap-3">
                      <dt className="text-xs font-semibold uppercase text-[var(--muted)]">
                        {t("Current version")}
                      </dt>
                      <dd className="text-sm font-semibold text-[var(--foreground)]">
                        {settings?.update.currentVersion ?? settings?.appVersion ?? ""}
                      </dd>
                    </div>
                    <div className="flex items-center justify-between gap-3">
                      <dt className="text-xs font-semibold uppercase text-[var(--muted)]">
                        {t("Latest version")}
                      </dt>
                      <dd className="text-sm font-semibold text-[var(--foreground)]">
                        {settings?.update.targetVersion ?? t("Up to date")}
                      </dd>
                    </div>
                    <div className="flex items-center justify-between gap-3">
                      <dt className="text-xs font-semibold uppercase text-[var(--muted)]">
                        {t("Last checked")}
                      </dt>
                      <dd className="text-right text-sm font-semibold text-[var(--foreground)]">
                        {settings?.update.lastCheckedAt
                          ? formatAuditDate(settings.update.lastCheckedAt, language)
                          : t("Never")}
                      </dd>
                    </div>
                    {settings?.update.releaseUrl ? (
                      <div className="flex items-center justify-between gap-3">
                        <dt className="text-xs font-semibold uppercase text-[var(--muted)]">
                          {t("Release")}
                        </dt>
                        <dd className="min-w-0 text-right text-sm font-semibold">
                          <a
                            className="break-all text-[var(--accent-soft-foreground)] underline-offset-2 hover:text-[var(--accent-soft-foreground)] hover:underline"
                            href={settings.update.releaseUrl}
                            rel="noreferrer"
                            target="_blank"
                          >
                            {settings.update.releaseName ?? settings.update.releaseUrl}
                          </a>
                        </dd>
                      </div>
                    ) : null}
                    <div className="flex items-center justify-between gap-3">
                      <dt className="text-xs font-semibold uppercase text-[var(--muted)]">
                        {t("GitHub repository")}
                      </dt>
                      <dd className="min-w-0 text-right text-sm font-semibold">
                        <a
                          aria-label={t("Open GitHub repository")}
                          className="break-all text-[var(--accent-soft-foreground)] underline-offset-2 hover:text-[var(--accent-soft-foreground)] hover:underline"
                          href="https://github.com/fonlan/foco"
                          rel="noreferrer"
                          target="_blank"
                        >
                          https://github.com/fonlan/foco
                        </a>
                      </dd>
                    </div>
                  </dl>
                </div>
              </section>
              {updateConfirm ? (
                <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && setUpdateConfirm(null)}>
                  <Modal.Container placement="center" size="sm">
                  <Modal.Dialog
                    className="fixed left-1/2 top-1/2 z-50 grid w-[min(92vw,28rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
                    aria-label={updateConfirm.source === "install" ? t("Update is installing") : t("Update available")}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <h3 className="text-base font-semibold text-[var(--foreground)]">
                          {updateConfirm.source === "install"
                            ? t("Update is installing")
                            : t("Update available")}
                        </h3>
                        <p className="mt-1 text-sm text-[var(--muted)]">
                          {updateConfirm.source === "install"
                            ? t("Foco will restart shortly.")
                            : t("Version {version} is available", {
                                version: updateConfirm.status.targetVersion ?? "",
                              })}
                        </p>
                      </div>
                      <SettingsButton
                        aria-label={t("Close")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onClick={() => setUpdateConfirm(null)}
                        title={t("Close")}
                        type="button"
                      >
                        <X aria-hidden="true" className="size-4" />
                      </SettingsButton>
                    </div>
                    {updateConfirm.source === "check" ? (
                      <div className="flex justify-end gap-2">
                        <SettingsButton
                          className="inline-flex min-h-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm font-semibold text-[var(--muted)] hover:border-[var(--border)] hover:bg-[var(--surface-secondary)]"
                          onClick={() => setUpdateConfirm(null)}
                          type="button"
                        >
                          {t("Not now")}
                        </SettingsButton>
                        <SettingsButton
                          className="inline-flex min-h-10 items-center justify-center gap-2 rounded-lg bg-[var(--foreground)] px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                          disabled={isInstallingUpdate}
                          onClick={() => void installUpdate()}
                          type="button"
                        >
                          {isInstallingUpdate ? (
                            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                          ) : (
                            <Download aria-hidden="true" className="size-4" />
                          )}
                          <span>{t("Install update")}</span>
                        </SettingsButton>
                      </div>
                    ) : null}
                  </Modal.Dialog>
                  </Modal.Container>
                </Modal.Backdrop>
              ) : null}
            </>
          ) : null}
          {settingsFilePickerRequest ? (
            <FilePickerDialog
              initialPath={settingsFilePickerRequest.initialPath}
              mode={settingsFilePickerRequest.mode}
              multiple={settingsFilePickerRequest.multiple}
              open={true}
              target={settingsFilePickerRequest.target}
              title={settingsFilePickerRequest.title}
              t={t}
              onClose={() => {
                setSettingsFilePickerRequest(null);
                setIsSelectingPromptFile(false);
                setIsSelectingWorkspaceFormPath(false);
              }}
              onSelect={(selection) => {
                const request = settingsFilePickerRequest;
                setSettingsFilePickerRequest(null);
                setIsSelectingPromptFile(false);
                setIsSelectingWorkspaceFormPath(false);
                request.onSelect(selection);
              }}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

function RemoteServersSettingsSection({
  diagnostics,
  form,
  isDialogOpen,
  isTrustingHostKey,
  onCancelHostKeyTrust,
  onCloseDialog,
  onConfirmHostKeyTrust,
  onEdit,
  onFormChange,
  onRunOperation,
  onSave,
  onSelectIdentityFile,
  onStartAdding,
  operationKey: activeOperationKey,
  pendingHostKeyTrust,
  references,
  servers,
  t,
}: {
  diagnostics: Record<string, RemoteServerDiagnosticResponse["result"]>;
  form: RemoteServerFormState;
  isDialogOpen: boolean;
  isTrustingHostKey: boolean;
  onCancelHostKeyTrust: () => void;
  onCloseDialog: () => void;
  onConfirmHostKeyTrust: () => void;
  onEdit: (server: RemoteServerSummary) => void;
  onFormChange: (updater: (current: RemoteServerFormState) => RemoteServerFormState) => void;
  onRunOperation: (
    server: RemoteServerSummary,
    operation: Exclude<RemoteServerOperation, "save">,
  ) => Promise<boolean>;
  onSave: (event: FormEvent<HTMLFormElement>) => void;
  onSelectIdentityFile: () => void;
  onStartAdding: () => void;
  operationKey: string | null;
  pendingHostKeyTrust: PendingHostKeyTrust | null;
  references: RemoteServerWorkspaceReference[];
  servers: RemoteServerSummary[];
  t: Translate;
}) {
  const isSaving = activeOperationKey === operationKeyForFormSave(form);
  const passwordTabId = "remote-server-auth-password";
  const keyTabId = "remote-server-auth-key";
  const passwordPanelId = "remote-server-auth-password-panel";
  const keyPanelId = "remote-server-auth-key-panel";

  return (
    <section className="grid gap-4">
      {pendingHostKeyTrust ? (
        <Modal.Backdrop
          isDismissable={!isTrustingHostKey}
          isOpen
          onOpenChange={(open) => {
            if (!open && !isTrustingHostKey) {
              onCancelHostKeyTrust();
            }
          }}
        >
          <Modal.Container placement="center" size="sm">
            <Modal.Dialog aria-label={t("Unknown SSH host key")}>
              <Modal.Header>
                <Modal.Icon className="bg-warning-soft text-warning-soft-foreground">
                  <KeyRound aria-hidden="true" className="size-5" />
                </Modal.Icon>
                <Modal.Heading>{t("Unknown SSH host key")}</Modal.Heading>
                <p className="text-sm text-muted">
                  {t(
                    "Trust this host and retry the connection? Only confirm if you trust this server.",
                  )}
                </p>
              </Modal.Header>
              <Modal.Body>
                <dl className="grid gap-2 rounded-lg border border-border bg-surface px-3 py-2 text-xs text-foreground">
                  <div className="flex justify-between gap-3">
                    <dt className="shrink-0 font-semibold text-muted">{t("SSH hostname / IP")}</dt>
                    <dd className="truncate font-mono">{pendingHostKeyTrust.hostKey.host}</dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="shrink-0 font-semibold text-muted">{t("SSH port")}</dt>
                    <dd className="font-mono">{pendingHostKeyTrust.hostKey.port}</dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="shrink-0 font-semibold text-muted">{t("Host key algorithm")}</dt>
                    <dd className="truncate font-mono">{pendingHostKeyTrust.hostKey.algorithm}</dd>
                  </div>
                  <div className="grid gap-1">
                    <dt className="font-semibold text-muted">{t("SHA-256 fingerprint")}</dt>
                    <dd className="break-all font-mono text-[11px] leading-relaxed">
                      {pendingHostKeyTrust.hostKey.fingerprintSha256}
                    </dd>
                  </div>
                </dl>
              </Modal.Body>
              <Modal.Footer>
                <Button
                  aria-label={t("Cancel host key trust")}
                  isDisabled={isTrustingHostKey}
                  variant="tertiary"
                  onPress={onCancelHostKeyTrust}
                >
                  {t("Cancel")}
                </Button>
                <Button
                  aria-label={t("Confirm and continue")}
                  isPending={isTrustingHostKey}
                  onPress={onConfirmHostKeyTrust}
                >
                  {({ isPending }) => (
                    <>
                      {isPending ? (
                        <Spinner color="current" size="sm" />
                      ) : (
                        <CheckCircle2 aria-hidden="true" className="size-4" />
                      )}
                      {t("Confirm and continue")}
                    </>
                  )}
                </Button>
              </Modal.Footer>
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      ) : null}

      {isDialogOpen ? (
        <>
          <SettingsButton
            aria-label={t("Close remote server configuration backdrop")}
            className="fixed inset-0 z-40 bg-[color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur-sm"
            onClick={onCloseDialog}
            type="button"
          />
          <form
            aria-label={t("Remote server configuration")}
            className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(92vw,38rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-[var(--border)] bg-[var(--surface)] px-4 py-4 shadow-[var(--overlay-shadow)]"
            onSubmit={onSave}
          >
            <div className="mb-4 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <Server aria-hidden="true" className="size-5 text-[var(--accent-soft-foreground)]" />
                  <h3 className="text-sm font-semibold text-[var(--foreground)]">
                    {form.id ? t("Edit remote server") : t("Add remote server")}
                  </h3>
                </div>
                <div className="mt-1 truncate text-xs text-[var(--muted)]">
                  {form.hostAlias || t("SSH hostname / IP")}
                </div>
              </div>
              <SettingsButton
                aria-label={t("Close remote server configuration")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                onClick={onCloseDialog}
                title={t("Close")}
                type="button"
              >
                <X aria-hidden="true" className="size-4" />
              </SettingsButton>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <SettingsTextField
                label={t("Server name")}
                onChange={(value) => onFormChange((current) => ({ ...current, name: value }))}
                placeholder={t("Server name")}
                value={form.name}
              />
              <SettingsTextField
                label={t("SSH hostname / IP")}
                onChange={(value) => onFormChange((current) => ({ ...current, hostAlias: value }))}
                placeholder="192.168.1.10"
                value={form.hostAlias}
              />
              <SettingsTextField
                label={t("SSH user")}
                onChange={(value) => onFormChange((current) => ({ ...current, user: value }))}
                placeholder="root"
                value={form.user}
              />
              <SettingsTextField
                inputMode="numeric"
                label={t("SSH port")}
                onChange={(value) => onFormChange((current) => ({ ...current, port: value }))}
                placeholder="22"
                value={form.port}
              />
              <div className="sm:col-span-2">
                <div className="mb-1.5 text-xs font-semibold text-[var(--muted)]">
                  {t("Authentication")}
                </div>
                <div
                  aria-label={t("Authentication")}
                  className="mb-3 inline-flex rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] p-0.5"
                  role="tablist"
                >
                  <SettingsButton
                    aria-controls={keyPanelId}
                    aria-selected={form.authMethod === "key"}
                    className={`rounded-md px-3 py-1.5 text-xs font-semibold transition ${
                      form.authMethod === "key"
                        ? "bg-[var(--surface)] text-[var(--foreground)] shadow-sm"
                        : "text-[var(--muted)] hover:text-[var(--foreground)]"
                    }`}
                    id={keyTabId}
                    onClick={() =>
                      onFormChange((current) => ({
                        ...current,
                        authMethod: "key",
                      }))
                    }
                    role="tab"
                    tabIndex={form.authMethod === "key" ? 0 : -1}
                    type="button"
                  >
                    {t("Key")}
                  </SettingsButton>
                  <SettingsButton
                    aria-controls={passwordPanelId}
                    aria-selected={form.authMethod === "password"}
                    className={`rounded-md px-3 py-1.5 text-xs font-semibold transition ${
                      form.authMethod === "password"
                        ? "bg-[var(--surface)] text-[var(--foreground)] shadow-sm"
                        : "text-[var(--muted)] hover:text-[var(--foreground)]"
                    }`}
                    id={passwordTabId}
                    onClick={() =>
                      onFormChange((current) => ({
                        ...current,
                        authMethod: "password",
                      }))
                    }
                    role="tab"
                    tabIndex={form.authMethod === "password" ? 0 : -1}
                    type="button"
                  >
                    {t("Password")}
                  </SettingsButton>
                </div>
                {form.authMethod === "password" ? (
                  <div
                    aria-labelledby={passwordTabId}
                    id={passwordPanelId}
                    role="tabpanel"
                  >
                    <SettingsTextField
                      autoComplete="current-password"
                      label={t("SSH password")}
                      onChange={(value) =>
                        onFormChange((current) => ({ ...current, password: value }))
                      }
                      placeholder={
                        form.passwordConfigured
                          ? t("Leave empty to keep saved password")
                          : t("Required for password auth")
                      }
                      type="password"
                      value={form.password}
                    />
                  </div>
                ) : (
                  <div aria-labelledby={keyTabId} id={keyPanelId} role="tabpanel">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                        {t("Identity file")}
                      </span>
                      <div className="flex gap-2">
                        <SettingsInput
                          autoComplete="off"
                          className="h-10 min-w-0 flex-1 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
                          name="identity-file"
                          onChange={(event) =>
                            onFormChange((current) => ({
                              ...current,
                              identityFile: event.target.value,
                            }))
                          }
                          placeholder="~/.ssh/id_ed25519"
                          type="text"
                          value={form.identityFile}
                        />
                        <SettingsButton
                          aria-label={t("Browse for private key")}
                          className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
                          onClick={onSelectIdentityFile}
                          title={t("Browse for private key")}
                          type="button"
                        >
                          <FolderSearch aria-hidden="true" className="size-4" />
                        </SettingsButton>
                      </div>
                    </label>
                  </div>
                )}
              </div>
              <SettingsTextField
                label={t("Default remote root")}
                onChange={(value) => onFormChange((current) => ({ ...current, defaultRemoteRoot: value }))}
                placeholder="~/workspaces"
                value={form.defaultRemoteRoot}
              />
              <SettingsTextField
                label={t("Terminal shell override")}
                onChange={(value) => onFormChange((current) => ({ ...current, terminalShell: value }))}
                placeholder="/bin/bash"
                value={form.terminalShell}
              />
              <SettingsTextField
                label={t("Foco command")}
                onChange={(value) => onFormChange((current) => ({ ...current, focoCommand: value }))}
                placeholder="foco"
                value={form.focoCommand}
              />
              <SettingsTextField
                inputMode="numeric"
                label={t("Connect timeout ms")}
                onChange={(value) => onFormChange((current) => ({ ...current, connectTimeoutMs: value }))}
                placeholder="10000"
                value={form.connectTimeoutMs}
              />
            </div>
            {references.length ? (
              <div className="mt-4 rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-xs text-[var(--warning)]">
                <div className="font-semibold">{t("Server is used by workspaces")}</div>
                <div className="mt-1 flex flex-wrap gap-1.5">
                  {references.map((reference) => (
                    <CapabilityPill
                      key={reference.id}
                      label={`${reference.name}: ${reference.remotePath}`}
                      ok={false}
                      tone="active"
                    />
                  ))}
                </div>
              </div>
            ) : null}
            <div className="mt-4 flex justify-end gap-2">
              <SettingsButton
                aria-label={t("Cancel remote server configuration")}
                className="inline-flex size-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                onClick={onCloseDialog}
                title={t("Cancel")}
                type="button"
              >
                <X aria-hidden="true" className="size-4" />
              </SettingsButton>
              <SettingsButton
                aria-label={t("Save remote server")}
                className="inline-flex size-10 items-center justify-center rounded-lg bg-[var(--foreground)] text-white hover:bg-[var(--foreground)] disabled:cursor-not-allowed disabled:bg-[var(--default)]"
                disabled={
                  isSaving ||
                  !form.name.trim() ||
                  !form.hostAlias.trim() ||
                  (form.authMethod === "password" &&
                    !form.password.trim() &&
                    !form.passwordConfigured)
                }
                title={t("Save remote server")}
                type="submit"
              >
                {isSaving ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
              </SettingsButton>
            </div>
          </form>
        </>
      ) : null}

      <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] shadow-[var(--overlay-shadow)]">
        <div className="flex items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-3">
          <h3 className="text-sm font-semibold text-[var(--foreground)]">
            {t("Remote server list")}
          </h3>
          <SettingsButton
            aria-label={t("Add remote server")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
            onClick={onStartAdding}
            title={t("Add remote server")}
            type="button"
          >
            <Plus aria-hidden="true" className="size-4" />
          </SettingsButton>
        </div>
        {servers.length ? (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-[var(--border)] text-left text-sm">
              <thead className="bg-[var(--surface-secondary)] text-xs font-semibold uppercase text-[var(--muted)]">
                <tr>
                  <th className="px-4 py-2">{t("Server")}</th>
                  <th className="px-4 py-2">{t("Status")}</th>
                  <th className="px-4 py-2">{t("Target")}</th>
                  <th className="px-4 py-2">{t("Sidecar")}</th>
                  <th className="px-4 py-2">{t("Workspaces")}</th>
                  <th className="px-4 py-2">{t("Last checked")}</th>
                  <th className="px-4 py-2 text-right">{t("Actions")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border)]">
                {servers.map((server) => {
                  const diagnostic = diagnostics[server.id];
                  const isBusy = Boolean(activeOperationKey?.endsWith(`:${server.id}`));
                  const isConnected = server.status === "connected" || server.status === "ready";
                  const toggleOperation = isConnected ? "disconnect" : "connect";
                  return (
                    <tr key={server.id} className="align-top">
                      <td className="px-4 py-3">
                        <div className="flex min-w-0 items-center gap-2">
                          <span className="relative inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] text-[var(--accent-soft-foreground)]">
                            <Server aria-hidden="true" className="size-4" />
                            <span className={`absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-white ${remoteStatusDotClass(server.status)}`} />
                          </span>
                          <div className="min-w-0">
                            <div className="truncate font-semibold text-[var(--foreground)]">{server.name}</div>
                            <div className="truncate text-xs text-[var(--muted)]">{remoteServerDisplayTarget(server)}</div>
                            {server.lastError ? (
                              <div className="mt-1 line-clamp-2 text-xs text-[var(--danger)]">{server.lastError}</div>
                            ) : null}
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <CapabilityPill
                          label={remoteStatusLabel(server.status, t)}
                          ok={server.status === "connected" || server.status === "ready"}
                          tone={remoteStatusTone(server.status)}
                        />
                      </td>
                      <td className="px-4 py-3 text-xs text-[var(--muted)]">
                        {server.lastKnownTarget ?? "-"}
                      </td>
                      <td className="px-4 py-3 text-xs text-[var(--muted)]">
                        <div>{server.sidecarVersion ?? "-"}</div>
                        <div className="mt-1 text-[var(--muted)]">
                          {t("Cached diagnostic; refreshed after a verified workspace connection")}
                        </div>
                        <div className="mt-1 text-[var(--muted)]">
                          {remoteSidecarInstallStateLabel(server.sidecarInstallState, t)}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-xs font-semibold text-[var(--muted)]">
                        {server.workspaceCount}
                      </td>
                      <td className="px-4 py-3 text-xs text-[var(--muted)]">
                        {server.lastCheckedAt ?? "-"}
                        {diagnostic ? (
                          <div className="mt-2 grid gap-1">
                            {diagnostic.stages.map((stage) => (
                              <div key={stage.stage} className="flex items-center gap-1.5">
                                <span className={`size-2 rounded-full ${remoteStageDotClass(stage.status)}`} />
                                <span className="truncate" title={stage.message}>
                                  {remoteDiagnosticStageLabel(stage.stage, t)}
                                </span>
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex justify-end gap-1.5">
                          <IconActionButton
                            disabled={isBusy}
                            icon={RefreshCw}
                            label={t("Test remote server")}
                            loading={activeOperationKey === operationKey("test", server.id)}
                            onClick={() => void onRunOperation(server, "test")}
                          />
                          <IconActionButton
                            disabled={isBusy}
                            icon={isConnected ? CircleAlert : Play}
                            label={t(isConnected ? "Disconnect remote server" : "Connect remote server")}
                            loading={activeOperationKey === operationKey(toggleOperation, server.id)}
                            onClick={() => void onRunOperation(server, toggleOperation)}
                          />
                          <IconActionButton
                            disabled={isBusy}
                            icon={Pencil}
                            label={t("Edit remote server")}
                            onClick={() => onEdit(server)}
                          />
                          <IconActionButton
                            disabled={isBusy}
                            icon={Trash2}
                            label={t("Delete remote server")}
                            loading={activeOperationKey === operationKey("delete", server.id)}
                            onClick={() => void onRunOperation(server, "delete")}
                            tone="danger"
                          />
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="px-4 py-6 text-sm text-[var(--muted)]">
            {t("No remote servers")}
          </div>
        )}
      </section>
    </section>
  );
}

function operationKeyForFormSave(form: RemoteServerFormState) {
  return operationKey("save", form.id || "new");
}

function IconActionButton({
  disabled,
  icon: Icon,
  label,
  loading = false,
  onClick,
  tone = "default",
}: {
  disabled?: boolean;
  icon: LucideIcon;
  label: string;
  loading?: boolean;
  onClick: () => void;
  tone?: "danger" | "default";
}) {
  return (
    <SettingsButton
      aria-label={label}
      className={`inline-flex size-8 items-center justify-center rounded-lg border shadow-sm disabled:cursor-not-allowed disabled:text-[var(--muted)] ${tone === "danger"
          ? "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--danger)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
          : "border-[var(--border)] bg-[var(--surface)] text-[var(--muted)] hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
        }`}
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      {loading ? (
        <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
      ) : (
        <Icon aria-hidden="true" className="size-3.5" />
      )}
    </SettingsButton>
  );
}

function remoteServerReferencesForMessage(
  message: string,
  workspaces: ConfiguredWorkspaceSummary[],
): RemoteServerWorkspaceReference[] {
  const names = message.includes(":")
    ? message
      .slice(message.indexOf(":") + 1)
      .split(",")
      .map((name) => name.trim())
      .filter(Boolean)
    : [];
  return workspaces
    .filter((workspace) => workspace.serverId && names.includes(workspace.name))
    .map((workspace) => ({
      id: workspace.id,
      name: workspace.name,
      remotePath: workspace.remotePath ?? workspace.path,
    }));
}

function remoteStatusLabel(status: string, t: Translate) {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return t("Connected");
  }
  if (normalized === "checking" || normalized === "connecting") {
    return t("Checking");
  }
  if (normalized === "failed" || normalized === "failedauth") {
    return t("Failed");
  }
  if (normalized === "offline") {
    return t("Offline");
  }
  if (normalized === "error") {
    return t("Error");
  }
  if (normalized === "unknown") {
    return t("Unknown");
  }
  return status;
}

function remoteStatusTone(status: string): CapabilityPillTone {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return "success";
  }
  if (normalized === "checking" || normalized === "connecting" || normalized === "reconnecting") {
    return "active";
  }
  if (normalized === "failed" || normalized === "failedauth" || normalized === "error") {
    return "danger";
  }
  return "muted";
}

function remoteStatusDotClass(status: string) {
  switch (remoteStatusTone(status)) {
    case "success":
      return "bg-[var(--success)]";
    case "active":
      return "bg-[var(--warning-soft)]";
    case "danger":
      return "bg-[var(--danger-soft)]";
    default:
      return "bg-[var(--default)]";
  }
}

function remoteStageDotClass(status: string) {
  if (status === "success") {
    return "bg-[var(--success)]";
  }
  if (status === "failed") {
    return "bg-[var(--danger-soft)]";
  }
  if (status === "skipped") {
    return "bg-[var(--default)]";
  }
  return "bg-[var(--warning-soft)]";
}

function remoteSidecarInstallStateLabel(state: string, t: Translate) {
  switch (state) {
    case "available":
      return t("Sidecar available");
    case "customCommand":
      return t("Custom command");
    case "missingAsset":
      return t("Sidecar asset missing");
    case "notInstalled":
      return t("Sidecar not installed");
    case "unknown":
      return t("Unknown");
    default:
      return state;
  }
}

function remoteDiagnosticStageLabel(stage: string, t: Translate) {
  switch (stage) {
    case "ssh":
      return t("Checking SSH");
    case "target":
      return t("Detecting target");
    case "sidecarAsset":
      return t("Installing sidecar");
    case "remoteInstallDirWritable":
      return t("Starting sidecar");
    case "focoCommandVersion":
      return t("Checking Sidecar version");
    default:
      return stage;
  }
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

  if (section === "remote-servers") {
    return t("Remote server settings");
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

  if (section === "skills") {
    return t("Skill settings");
  }

  return t("About Foco");
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

  if (section === "remote-servers") {
    return t("Reusable SSH connection profiles");
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

  if (section === "about") {
    return t("Local-first AI coding workspace");
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
    <SettingsButton
      aria-label={label}
      aria-current={active ? "page" : undefined}
      className={`inline-flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left text-sm font-semibold ${active
          ? "bg-[var(--accent)] text-white shadow-[var(--overlay-shadow)]"
          : "text-[var(--muted)] hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)]"
        }`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      <span className="min-w-0 truncate">{label}</span>
    </SettingsButton>
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
      <SettingsTextArea
        aria-label={title}
        className={`${minHeightClass} w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-xs text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]`}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        value={value}
      />
    );
  }

  return (
    <div className="rounded-lg border border-[var(--border)] bg-[var(--foreground)] text-[var(--muted)]">
      <SettingsButton
        aria-label={`${isExpanded ? t("Collapse JSON") : t("Expand JSON")} ${title}`}
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-xs font-semibold text-[var(--muted)]"
        onClick={() => onToggle(id)}
        type="button"
      >
        <span className="inline-flex min-w-0 items-center gap-2">
          <Code2 aria-hidden="true" className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]" />
          <span className="truncate">{title}</span>
        </span>
        <span className="shrink-0 text-[var(--muted)]">
          {isExpanded ? t("Collapse JSON") : t("Expand JSON")}
        </span>
      </SettingsButton>
      {isExpanded ? (
        <SettingsTextArea
          aria-label={title}
          className={`${minHeightClass} w-full resize-y border-0 border-t border-[var(--border)] bg-[var(--foreground)] px-3 py-3 font-mono text-xs leading-relaxed text-[var(--muted)] outline-none focus:ring-2 focus:ring-inset focus:ring-[var(--accent)]`}
          onChange={(event) => onChange(event.target.value)}
          spellCheck={false}
          value={parsed.pretty}
        />
      ) : (
        <div className="border-t border-[var(--border)] px-3 py-2 font-mono text-xs text-[var(--muted)]">
          <code>{jsonSyntaxNodes(compactToolText(parsed.pretty))}</code>
        </div>
      )}
    </div>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-lg bg-[var(--surface)] px-3 py-2">
      <div className="truncate text-[11px] font-semibold uppercase text-[var(--muted)]">
        {label}
      </div>
      <div className="mt-0.5 truncate text-sm font-semibold text-[var(--foreground)]">
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
    <div className="grid gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3 text-xs text-[var(--muted)] sm:grid-cols-2">
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Memory scope")}: </span>
        {memoryScopeLabel(source.scope, t)}
      </div>
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Chat ID")}: </span>
        {source.chatId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Source type")}: </span>
        {source.sourceType}
      </div>
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Source ID")}: </span>
        {source.sourceId ?? "-"}
      </div>
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Created")}: </span>
        {source.createdAt}
      </div>
      <div>
        <span className="font-semibold text-[var(--muted)]">{t("Updated")}: </span>
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
      return "border-[var(--success)] bg-[var(--success-soft)] text-[var(--success-soft-foreground)]";
    case "danger":
      return "border-[var(--danger)] bg-[var(--danger-soft)] text-[var(--danger)]";
    case "active":
      return "border-[var(--warning)] bg-[var(--warning-soft)] text-[var(--warning)]";
    case "muted":
      return "border-[var(--border)] bg-[var(--surface-secondary)] text-[var(--muted)]";
    case "ok":
    default:
      return "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)]";
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
          className="flex items-center gap-2 rounded-lg border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-sm text-[var(--warning)]"
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
    webSearchMode: "auto",
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
    modelRedirects: [],
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
    chatTitleGenerationModelId: "current_chat_model",
    hookAuditEnabled: false,
    language: "en",
    listenHost: "127.0.0.1",
    listenPort: "3210",
    llmRequestRetryCount: "3",
    password: "",
    runtimeToolStateCompressionEnabled: false,
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

function emptyPromptOverrideField(): PromptOverrideFieldState {
  return { value: "", custom: false };
}

function emptyPromptSettingsForm(): PromptSettingsFormState {
  return {
    activeSystemPromptName: DEFAULT_SYSTEM_PROMPT_NAME,
    contextCompression: emptyPromptOverrideField(),
    generationSystemPrompt: emptyPromptOverrideField(),
    updateSystemPrompt: emptyPromptOverrideField(),
    memoryRetrieval: emptyPromptOverrideField(),
    memoryExtraction: emptyPromptOverrideField(),
    memoryDream: emptyPromptOverrideField(),
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
    llmTimeoutMs: "300000",
  };
}

function specSettingsFormFromResponse(data: SettingsResponse): SpecSettingsFormState {
  return {
    autoEnabled: data.spec.autoEnabled,
    generationModelId: data.spec.generationModelId ?? "",
    llmTimeoutMs: String(data.spec.llmTimeoutMs),
  };
}

function specSettingsFormsEqual(
  left: SpecSettingsFormState,
  right: SpecSettingsFormState,
): boolean {
  return (
    left.autoEnabled === right.autoEnabled &&
    left.generationModelId === right.generationModelId &&
    left.llmTimeoutMs === right.llmTimeoutMs
  );
}

/** Matches backend validate_spec_settings: enabled + active provider enabled. */
function isSpecEligibleGenerationModel(
  model: ConfiguredModelSummary,
  providers: readonly ConfiguredProviderSummary[],
): boolean {
  if (!model.enabled) {
    return false;
  }
  const activeProviderId = model.activeProviderId;
  if (!activeProviderId) {
    return false;
  }
  const provider = providers.find((item) => item.id === activeProviderId);
  return provider?.enabled === true;
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

/** System prompts shown in the list editor (not Plan Mode / Review cards). */
function ordinarySystemPrompts(prompts: SystemPromptSummary[]): SystemPromptSummary[] {
  return prompts.filter(
    (prompt) =>
      prompt.name !== PLAN_MODE_SYSTEM_PROMPT_NAME &&
      prompt.name !== REVIEW_SYSTEM_PROMPT_NAME &&
      prompt.name !== IMAGE_AGENT_SYSTEM_PROMPT_NAME,
  );
}

function promptOverrideFromStored(
  override: string | null | undefined,
  defaultValue: string | undefined,
): PromptOverrideFieldState {
  const custom = typeof override === "string" && override.trim().length > 0;
  return {
    value: custom ? override : (defaultValue ?? ""),
    custom,
  };
}

function promptOverridePayload(field: PromptOverrideFieldState): string | null {
  return field.custom && field.value.trim() ? field.value : null;
}

function PromptOverrideEditor({
  description,
  onChange,
  onRestore,
  restoreAriaLabel,
  t,
  testId,
  title,
  value,
}: {
  description?: string;
  onChange: (value: string) => void;
  onRestore: () => void;
  restoreAriaLabel: string;
  t: Translate;
  testId?: string;
  title: string;
  value: string;
}) {
  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-semibold text-[var(--muted)]">{title}</p>
          {description ? (
            <p className="mt-1 text-xs text-[var(--muted)]">{description}</p>
          ) : null}
        </div>
        <SettingsButton
          aria-label={restoreAriaLabel}
          className="inline-flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2.5 text-xs font-semibold text-[var(--muted)] shadow-sm hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent-soft-foreground)]"
          onClick={onRestore}
          title={t("Restore default")}
          type="button"
        >
          <RefreshCw aria-hidden="true" className="size-3.5" />
          {t("Restore default")}
        </SettingsButton>
      </div>
      <label className="mt-3 block">
        <span className="sr-only">{title}</span>
        <SettingsTextArea
          aria-label={title}
          className="min-h-44 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-sm leading-6 text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]"
          data-testid={testId}
          onChange={(event) => onChange(event.target.value)}
          value={value}
        />
      </label>
    </div>
  );
}

function emptyMemorySettingsForm(): MemorySettingsFormState {
  return {
    enabled: false,
    extractionMode: "manual",
    retrievalMode: "fts",
    extractionModelId: "",
    retrievalModelId: "",
    extractionLlmTimeoutMs: "300000",
    retrievalLlmTimeoutMs: "300000",
    contextBudgetPercent: "12",
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
      llmTimeoutMs: "300000",
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
    remotePath: null,
    serverId: null,
    pinned: false,
    specEnabled: false,
    specInjectEnabled: false,
    terminalShell: "",
  };
}

function emptyRemoteServerForm(): RemoteServerFormState {
  return {
    authMethod: "key",
    connectTimeoutMs: "10000",
    defaultRemoteRoot: "~",
    focoCommand: "",
    hostAlias: "",
    id: "",
    identityFile: "",
    name: "",
    password: "",
    passwordConfigured: false,
    port: "",
    terminalShell: "",
    user: "root",
  };
}

function remoteServerFormFromSummary(server: RemoteServerSummary): RemoteServerFormState {
  return {
    authMethod: server.authMethod ?? "key",
    connectTimeoutMs: String(server.connectTimeoutMs),
    defaultRemoteRoot: server.defaultRemoteRoot ?? "",
    focoCommand: server.focoCommand ?? "",
    hostAlias: server.hostAlias,
    id: server.id,
    identityFile: server.identityFile ?? "",
    name: server.name,
    password: "",
    passwordConfigured: server.passwordConfigured,
    port: server.port ? String(server.port) : "",
    terminalShell: server.terminalShell ?? "",
    user: server.user ?? "",
  };
}

function remoteServerFormPayload(form: RemoteServerFormState) {
  const trimmedPort = form.port.trim();
  const trimmedTimeout = form.connectTimeoutMs.trim();
  const authMethod = form.authMethod === "password" ? "password" : "key";
  return {
    authMethod,
    connectTimeoutMs: trimmedTimeout ? Number(trimmedTimeout) : undefined,
    defaultRemoteRoot: nullableTrimmed(form.defaultRemoteRoot),
    focoCommand: nullableTrimmed(form.focoCommand),
    hostAlias: form.hostAlias.trim(),
    id: form.id.trim() || undefined,
    identityFile: authMethod === "key" ? nullableTrimmed(form.identityFile) : null,
    name: form.name.trim(),
    // Empty password on update keeps existing; create with password mode requires value.
    password: authMethod === "password" ? form.password : null,
    port: trimmedPort ? Number(trimmedPort) : null,
    terminalShell: nullableTrimmed(form.terminalShell),
    user: nullableTrimmed(form.user),
  };
}

function nullableTrimmed(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function operationKey(operation: RemoteServerOperation, id: string) {
  return `${operation}:${id}`;
}

function remoteServerDisplayTarget(server: RemoteServerSummary) {
  const user = server.user ? `${server.user}@` : "";
  const port = server.port ? `:${server.port}` : "";
  return `${user}${server.hostAlias}${port}`;
}

/** Parent directory for FilePicker initialPath; null when empty/unknown. */
function parentDirectoryPath(path: string): string | null {
  const trimmed = path.trim();
  if (!trimmed) {
    return null;
  }
  const normalized = trimmed.replace(/\\/g, "/");
  const lastSlash = normalized.lastIndexOf("/");
  if (lastSlash <= 0) {
    return null;
  }
  return normalized.slice(0, lastSlash);
}

function emptyMcpServerForm(): McpServerFormState {
  return {
    argsText: "",
    command: "",
    enabled: true,
    executionHost: "auto",
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
    return token.endsWith('":') ? "text-[var(--accent-soft-foreground)]" : "text-[var(--success-soft-foreground)]";
  }
  if (token === "true" || token === "false") {
    return "text-[var(--warning)]";
  }
  if (token === "null") {
    return "text-[var(--muted)]";
  }
  return "text-[var(--accent-soft-foreground)]";
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

function loadedProviderModelIds(modelLists: Record<string, ProviderModelListState>) {
  return Object.values(modelLists)
    .filter((modelList) => modelList.status === "ok")
    .flatMap((modelList) => modelList.models)
    .filter(uniqueString);
}

function uniqueByValue<T extends { value: string }>(item: T, index: number, values: T[]) {
  return values.findIndex((value) => value.value === item.value) === index;
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

/** Parse a required whole number in inclusive [min, max]. Empty/non-integer/out-of-range throw. */
function requiredIntegerInRange(
  value: string,
  label: string,
  min: number,
  max: number,
) {
  const trimmed = value.trim();

  if (!trimmed || !/^\d+$/.test(trimmed)) {
    throw new Error(`${label} must be a whole number from ${min} to ${max}`);
  }

  const numberValue = Number(trimmed);

  if (
    !Number.isSafeInteger(numberValue) ||
    numberValue < min ||
    numberValue > max
  ) {
    throw new Error(`${label} must be a whole number from ${min} to ${max}`);
  }

  return numberValue;
}

function memoryDreamJobKey(scope: MemoryDreamScope, workspaceId: string | null) {
  return scope === "global" ? "global" : `workspace:${workspaceId ?? ""}`;
}

function memoryDreamScopeLabel(scope: string, t: Translate) {
  return scope === "global" ? t("Global Dream") : t("Workspace Dream");
}

function memoryDreamPartialUnavailableReasonLabel(reason: string, t: Translate) {
  switch (reason) {
    case "notConnected":
      return t("Remote workspace is not connected");
    case "invalidResponse":
      return t("Remote Dream history returned an invalid response");
    default:
      return t("Remote Dream history is temporarily unavailable");
  }
}

function specJobTime(job: WorkspaceSpecJobSummary) {
  return job.completedAt ?? job.startedAt ?? job.createdAt;
}

function specJobTriggerLabel(triggerType: string, t: Translate) {
  const labels: Record<string, string> = {
    chat_completed: "Chat completed",
    manual_refresh: "Manual refresh",
  };
  return t(labels[triggerType] ?? triggerType);
}

function specJobStatusLabel(status: string, t: Translate) {
  return memoryDreamStatusLabel(status, t);
}

function specJobStatusTone(status: string): CapabilityPillTone {
  return memoryDreamStatusTone(status);
}

function specJobResultLabel(
  job: WorkspaceSpecJobSummary,
  t: Translate,
  language: AppLanguageId,
) {
  if (job.status === "failed") {
    return job.errorMessage ?? t("Spec job failed");
  }
  if (job.status === "skipped") {
    return specJobSkippedReasonLabel(job.errorMessage, t);
  }
  if (job.status !== "completed") {
    return "";
  }

  const output = jsonObject(job.output);
  const revision = output ? jsonNumber(output.revision) : null;
  const contentBytes = output ? jsonNumber(output.contentBytes) : null;
  const parts: string[] = [];
  if (revision !== null) {
    parts.push(t("revision {revision}", { revision: formatNumber(revision, language) }));
  }
  if (contentBytes !== null) {
    parts.push(t("{count} bytes", { count: formatNumber(contentBytes, language) }));
  }
  return parts.join(" / ");
}

function specJobSkippedReasonLabel(reason: string | null, t: Translate) {
  if (!reason) {
    return t("Skipped");
  }
  const labels: Record<string, string> = {
    stale_revision: "Spec changed before this job could write",
    workspace_spec_disabled: "Workspace Spec is disabled",
    no_update_needed: "No update needed",
  };
  return labels[reason] ? t(labels[reason]) : reason;
}

function jsonObject(value: JsonValue | null): Record<string, JsonValue> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value : null;
}

function jsonNumber(value: JsonValue | undefined) {
  return typeof value === "number" ? value : null;
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
  return normalized.length > 240 ? `${normalized.slice(0, 237)}…` : normalized;
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
