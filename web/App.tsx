import {
  Activity,
  BarChart3,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Folder,
  FolderPlus,
  GitBranch,
  GitCompare,
  Globe,
  GripVertical,
  KeyRound,
  LoaderCircle,
  MessageSquare,
  PlugZap,
  Plus,
  RefreshCw,
  Send,
  Server,
  Settings,
  SlidersHorizontal,
  Terminal,
  Trash2,
  User,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  CSSProperties,
  DragEvent as ReactDragEvent,
  FormEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

type ChatSummary = {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
};

type WorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  chats: ChatSummary[];
};

type WorkspacesResponse = {
  activeWorkspaceId: string;
  workspaces: WorkspaceSummary[];
};

type ModelPricing = {
  input: number | null;
  output: number | null;
  reasoning: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
};

type ModelMetadataRecord = {
  key: string;
  providerId: string;
  providerName: string;
  modelId: string;
  name: string;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  pricing: ModelPricing;
  inputModalities: string[];
  outputModalities: string[];
  supportsTools: boolean;
  supportsCache: boolean;
  sourceUrl: string;
  refreshedAt: string;
};

type ConfiguredModelSummary = {
  id: string;
  displayName: string;
  enabled: boolean;
  metadataKey: string | null;
  metadataSourceUrl: string | null;
  metadataRefreshedAt: string | null;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  canEnable: boolean;
  missingLimits: string[];
  providerIds: string[];
  activeProviderId: string | null;
  thinkingLevel: string | null;
  supportsThinking: boolean;
  warnings: string[];
};

type ModelMetadataResponse = {
  sourceUrl: string | null;
  fetchedAt: string | null;
  cachePath: string;
  models: ModelMetadataRecord[];
  configuredModels: ConfiguredModelSummary[];
};

type ModelFormState = {
  displayName: string;
  enabled: boolean;
  maxOutputTokens: string;
  modelId: string;
  contextWindow: string;
  providerIds: string[];
  activeProviderId: string;
  thinkingLevel: string;
};

type ProviderKindSummary = {
  kind: string;
  label: string;
  defaultBaseUrl: string;
};

type ThinkingLevelSummary = {
  value: string;
  label: string;
};

type ConfiguredProviderSummary = {
  id: string;
  name: string;
  kind: string;
  kindLabel: string;
  enabled: boolean;
  baseUrl: string | null;
  hasApiKey: boolean;
  warnings: string[];
};

type WebServerSettingsSummary = {
  listenHost: string;
  listenPort: number;
};

type AppLanguageId = "en" | "zh-CN";

type AppLanguageSummary = {
  id: AppLanguageId;
  name: string;
};

type GeneralSettingsSummary = {
  language: AppLanguageId;
  supportedLanguages: AppLanguageSummary[];
  webServer: WebServerSettingsSummary;
};

type SettingsResponse = {
  general: GeneralSettingsSummary;
  providerKinds: ProviderKindSummary[];
  thinkingLevels: ThinkingLevelSummary[];
  providers: ConfiguredProviderSummary[];
  configuredModels: ConfiguredModelSummary[];
  mcpTransports: McpTransportSummary[];
  mcpServers: ConfiguredMcpServerSummary[];
  skills: SkillsSettingsSummary;
};

type ProviderFormState = {
  apiKey: string;
  baseUrl: string;
  clearApiKey: boolean;
  enabled: boolean;
  id: string;
  kind: string;
  name: string;
};

type GeneralFormState = {
  language: string;
  listenHost: string;
  listenPort: string;
};

type McpTransportSummary = {
  transport: string;
  label: string;
};

type ConfiguredMcpServerSummary = {
  id: string;
  name: string;
  enabled: boolean;
  transport: string;
  transportLabel: string;
  command: string | null;
  args: string[];
  url: string | null;
  state: string;
  error: string | null;
  toolCount: number;
  warnings: string[];
};

type McpServerFormState = {
  argsText: string;
  command: string;
  enabled: boolean;
  id: string;
  name: string;
  transport: string;
  url: string;
};

type SkillsSettingsSummary = {
  directories: string[];
  detected: ConfiguredSkillSummary[];
  errors: SkillDiscoveryErrorSummary[];
};

type ConfiguredSkillSummary = {
  id: string;
  name: string;
  description: string;
  path: string;
  enabled: boolean;
  warnings: string[];
};

type SkillDiscoveryErrorSummary = {
  path: string;
  message: string;
};

type ProviderTestResponse = {
  ok: boolean;
  message: string;
  modelCount: number;
};

type ProviderTestState = {
  message: string;
  status: "error" | "ok" | "testing";
};

type JsonValue =
  | boolean
  | null
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

type ChatToolCallSummary = {
  id: string;
  name: string;
  status: string;
  input: JsonValue;
  output: JsonValue | null;
  isError: boolean;
};

type ChatMessagePart =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "toolCall"; toolCall: ChatToolCallSummary };

type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
  reasoning: string | null;
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
};

type ChatMessagesResponse = {
  messages: ChatMessageSummary[];
};

type ChatUsage = {
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
};

type ChatStreamEvent =
  | {
      type: "start";
      chatId: string;
      userMessageId: string;
      assistantMessageId: string;
      llmRequestId?: string;
    }
  | { type: "textDelta"; assistantMessageId?: string; delta: string }
  | { type: "reasoningDelta"; assistantMessageId?: string; delta: string }
  | { type: "usage"; usage?: ChatUsage }
  | {
      type: "complete";
      chatId: string;
      assistantMessageId: string;
      text: string;
      reasoning?: string | null;
      usage?: ChatUsage | null;
      stopReason?: string | null;
    }
  | {
      type: "toolCall";
      assistantMessageId: string;
      toolCall: ChatToolCallSummary;
    }
  | {
      type: "toolResult";
      assistantMessageId: string;
      toolCallId: string;
      output: JsonValue;
      isError: boolean;
    }
  | {
      type: "gitDiffRefresh";
      workspaceId: string;
    }
  | { type: "error"; message: string };

type WorkspaceFormMode = "add" | "create";
type SettingsSection = "general" | "mcp" | "models" | "providers" | "skills";
type ViewMode = "chat" | "settings" | "stats";

const CREATE_BRANCH_OPTION_VALUE = "__create_branch__";
const CHAT_BOTTOM_LOCK_THRESHOLD_PX = 24;

type Translate = (key: string, values?: Record<string, string | number>) => string;

const TRANSLATIONS: Record<AppLanguageId, Record<string, string>> = {
  en: {},
  "zh-CN": {
    "Local workspace": "本地工作区",
    "Refresh workspaces": "刷新工作区",
    "Create or add workspace": "创建或添加工作区",
    "Collapse chat history": "收起聊天历史",
    "Expand chat history": "展开聊天历史",
    "New chat": "新建聊天",
    "New chat in {name}": "在 {name} 中新建聊天",
    "Delete chat": "删除聊天",
    "Delete chat {title}": "删除聊天 {title}",
    "No chats": "暂无聊天",
    "Loading workspaces...": "正在加载工作区...",
    "No workspaces": "暂无工作区",
    Workspace: "工作区",
    Chat: "聊天",
    Settings: "设置",
    Stats: "统计",
    "Close terminal": "关闭终端",
    "Open terminal": "打开终端",
    "Close git diff": "关闭 Git diff",
    "Open git diff": "打开 Git diff",
    "Cancel the current run before deleting this chat.":
      "删除此聊天前请先取消当前运行。",
    "Select a workspace before creating a branch.":
      "创建分支前请先选择工作区。",
    "Git branch name must not be empty.": "Git 分支名不能为空。",
    "Select a workspace before sending.": "发送前请先选择工作区。",
    "Select an enabled model before sending.": "发送前请先选择已启用的模型。",
    "Run cancelled.": "运行已取消。",
    "Create workspace": "创建工作区",
    "Add existing workspace": "添加现有工作区",
    "Create and register a new local folder.": "创建并注册新的本地文件夹。",
    "Register an existing local folder.": "注册已有本地文件夹。",
    "Close workspace dialog": "关闭工作区弹窗",
    Close: "关闭",
    "Switch to create workspace": "切换到创建工作区",
    "Switch to add workspace": "切换到添加工作区",
    "Add workspace": "添加工作区",
    Name: "名称",
    "Workspace name": "工作区名称",
    Path: "路径",
    Cancel: "取消",
    "Cancel workspace dialog": "取消工作区弹窗",
    "New branch": "新建分支",
    "Close branch dialog": "关闭分支弹窗",
    "Branch name": "分支名",
    "Cancel branch creation": "取消创建分支",
    "Create branch": "创建分支",
    "Workspace shell is ready": "工作区 Shell 已就绪",
    "Pick an enabled model and start the current workspace chat.":
      "选择一个已启用模型，开始当前工作区聊天。",
    "Remove skill": "移除技能",
    "Remove skill {name}": "移除技能 {name}",
    "Message Foco": "给 Foco 发送消息",
    "Select skill {name}": "选择技能 {name}",
    "Skill is disabled": "技能已禁用",
    disabled: "已禁用",
    "No matching skills": "没有匹配的技能",
    Model: "模型",
    "No enabled models": "没有已启用模型",
    Thinking: "思考",
    "Model default": "模型默认",
    "Retry last run": "重试上次运行",
    "Cancel run": "取消运行",
    "Send message": "发送消息",
    Send: "发送",
    "Git branch": "Git 分支",
    "Switch to branch {name}": "切换到分支 {name}",
    "No branches": "暂无分支",
    "Create git branch": "创建 Git 分支",
    Input: "输入",
    Output: "输出",
    error: "错误",
    connected: "已连接",
    connecting: "连接中",
    closed: "已关闭",
    "Terminal container was not mounted.": "终端容器尚未挂载。",
    "Terminal returned an unknown event.": "终端返回了未知事件。",
    "Terminal WebSocket failed.": "终端 WebSocket 失败。",
    "terminal exited: {status}": "终端已退出：{status}",
    "terminal error: {message}": "终端错误：{message}",
    "API statistics": "API 统计",
    "No workspace selected": "未选择工作区",
    "Workspace chats": "工作区聊天",
    "Enabled providers": "已启用供应商",
    "Runnable models": "可运行模型",
    "Configured models": "已配置模型",
    "Request audit": "请求审计",
    "No API request table is exposed yet": "尚未暴露 API 请求表格",
    "The app records LLM request audit rows in the workspace database. This page now has a dedicated surface ready for the request-summary API when it is added.":
      "应用会在工作区数据库中记录 LLM 请求审计行。此页面已预留专用区域，等待请求汇总 API 接入。",
    "audit data pending": "审计数据待接入",
    "Resize git diff panel": "调整 Git diff 面板宽度",
    "Git diff": "Git diff",
    "Workspace changes": "工作区变更",
    "Refresh diff": "刷新 diff",
    "All changed files": "全部变更文件",
    "No changes": "无变更",
    "No diff": "无 diff",
    General: "常规",
    Providers: "供应商",
    Models: "模型",
    Skills: "技能",
    "General settings": "常规设置",
    "Provider settings": "供应商设置",
    "Model settings": "模型设置",
    "MCP settings": "MCP 设置",
    "Skill settings": "技能设置",
    "Web service listen address": "Web 服务监听地址",
    "Provider credentials and connection checks": "供应商凭据与连接检查",
    "Workspace-scoped MCP server runtimes": "工作区级 MCP 服务运行时",
    "Skill discovery and enablement": "技能发现与启用",
    "Model metadata and runtime limits": "模型元数据与运行限制",
    "Fetched {time} from {source}": "已从 {source} 获取：{time}",
    "Model metadata has not been refreshed": "尚未刷新模型元数据",
    "Refresh model metadata": "刷新模型元数据",
    "Web service": "Web 服务",
    "Listen host": "监听 host",
    "Listen port": "监听端口",
    Language: "语言",
    "Save general settings": "保存常规设置",
    Save: "保存",
    "Reload general settings": "重新加载常规设置",
    "Reload settings": "重新加载设置",
    Reload: "重新加载",
    "Saved bind": "已保存绑定",
    "restart required": "需要重启",
    "Loading...": "正在加载...",
    "Saved host and port are used the next time the backend starts.":
      "已保存的 host 和端口会在后端下次启动时生效。",
    "Language changes apply immediately after saving.":
      "语言设置保存后会立即生效。",
    "Provider configuration": "供应商配置",
    "Edit provider": "编辑供应商",
    "Add provider": "添加供应商",
    "Enable provider": "启用供应商",
    "Delete provider": "删除供应商",
    "Close provider configuration": "关闭供应商配置",
    Protocol: "协议",
    "Base URL": "Base URL",
    "API key": "API key",
    "Saved key is kept unless replaced": "已保存的 key 会保留，除非填写新值",
    "Clear saved API key": "清除已保存的 API key",
    "Save provider": "保存供应商",
    "Configured providers": "已配置供应商",
    enabled: "已启用",
    "key saved": "已保存 key",
    "key missing": "缺少 key",
    "Edit provider {name}": "编辑供应商 {name}",
    "Test provider {name}": "测试供应商 {name}",
    "Test provider": "测试供应商",
    "No configured providers": "暂无已配置供应商",
    "Testing connection...": "正在测试连接...",
    "MCP server configuration": "MCP 服务配置",
    "Edit MCP server": "编辑 MCP 服务",
    "Add MCP server": "添加 MCP 服务",
    "Enable MCP server": "启用 MCP 服务",
    "Delete MCP server": "删除 MCP 服务",
    "Close MCP server configuration": "关闭 MCP 服务配置",
    Transport: "传输方式",
    Stdio: "标准输入输出",
    "Streamable HTTP": "Streamable HTTP",
    URL: "URL",
    Command: "命令",
    Args: "参数",
    "Save MCP server": "保存 MCP 服务",
    "MCP servers": "MCP 服务",
    "Reload MCP settings": "重新加载 MCP 设置",
    "tools {count}": "工具 {count}",
    "Edit MCP server {name}": "编辑 MCP 服务 {name}",
    "No configured MCP servers": "暂无已配置 MCP 服务",
    stopped: "已停止",
    "Skill directories": "技能目录",
    Directories: "目录",
    "Save skills": "保存技能",
    "Refresh skill discovery": "刷新技能发现",
    Refresh: "刷新",
    "Detected skills": "已发现技能",
    "skills {count}": "技能 {count}",
    "Enable skill {name}": "启用技能 {name}",
    "No detected skills": "暂无已发现技能",
    "Edit model {name}": "编辑模型 {name}",
    "Reorder model {name}": "调整模型顺序 {name}",
    "Add model": "添加模型",
    "Edit model": "编辑模型",
    "Delete model": "删除模型",
    "Close model configuration": "关闭模型配置",
    "Model configuration": "模型配置",
    "Model id": "模型 ID",
    "Display name": "显示名称",
    "Context window": "上下文窗口",
    "Max output tokens": "最大输出 token",
    "Enable model": "启用模型",
    "limits ok": "limits 已就绪",
    "limits missing": "limits 缺失",
    "providers {count}": "供应商 {count}",
    "active {id}": "当前 {id}",
    "active missing": "缺少当前供应商",
    "No configured models": "暂无已配置模型",
    "No providers": "暂无供应商",
    "Active provider": "当前供应商",
    "Thinking level": "思考级别",
    None: "无",
    "Fill both limits before enabling.": "启用前请填写两个 limits。",
    "Save model": "保存模型",
    "pricing in/out:": "价格 输入/输出：",
    "Search model metadata": "搜索模型元数据",
    "Reload model metadata cache": "重新加载模型元数据缓存",
    "Reload cache": "重新加载缓存",
    "input n/a": "输入不可用",
    "Loading models...": "正在加载模型...",
    "No cached models": "暂无缓存模型",
    Minimal: "最小",
    Low: "低",
    Medium: "中",
    High: "高",
    "Extra High": "极高",
    "Listen port must be a positive whole number": "监听端口必须是正整数",
    Unknown: "未知错误",
    "Unknown error": "未知错误",
  },
};

const I18nContext = createContext<{
  language: AppLanguageId;
  t: Translate;
}>({
  language: "en",
  t: translate,
});

function useI18n() {
  return useContext(I18nContext);
}

function translate(
  key: string,
  values: Record<string, string | number> = {},
  language: AppLanguageId = "en",
) {
  const template = TRANSLATIONS[language][key] ?? key;

  return Object.entries(values).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    template,
  );
}

type GitStatusFileSummary = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
};

type GitDiffResponse = {
  path: string | null;
  status: string;
  diff: string;
  stagedDiff: string;
  files: GitStatusFileSummary[];
};

type GitBranchesResponse = {
  isGitRepository: boolean;
  currentBranch: string | null;
  branches: string[];
};

type TerminalSessionResponse = {
  id: string;
  name: string;
  workingDirectory: string;
};

type TerminalServerEvent =
  | { type: "started"; cwd: string }
  | { type: "output"; data: string }
  | { type: "cwd"; cwd: string }
  | { type: "exit"; status: string }
  | { type: "error"; message: string };

type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  reasoning: string | null;
  status?: "error" | "streaming";
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
};

type RetryRunRequest = {
  workspaceId: string;
  chatId: string | null;
  content: string;
  modelId: string;
  thinkingLevel: string;
  skillIds: string[];
};

export function App() {
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>("");
  const [expandedWorkspaceIds, setExpandedWorkspaceIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [viewMode, setViewMode] = useState<ViewMode>("chat");
  const [formMode, setFormMode] = useState<WorkspaceFormMode>("create");
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [draftMessage, setDraftMessage] = useState("");
  const [messages, setMessages] = useState<ShellMessage[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [selectedThinkingLevel, setSelectedThinkingLevel] = useState("");
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
  const [gitBranches, setGitBranches] = useState<GitBranchesResponse | null>(null);
  const [selectedGitBranch, setSelectedGitBranch] = useState("");
  const [isLoadingBranches, setIsLoadingBranches] = useState(false);
  const [branchError, setBranchError] = useState<string | null>(null);
  const [isBranchDialogOpen, setIsBranchDialogOpen] = useState(false);
  const [newBranchName, setNewBranchName] = useState("");
  const [isSavingBranch, setIsSavingBranch] = useState(false);
  const [isDiffPanelOpen, setIsDiffPanelOpen] = useState(false);
  const [diffPanelWidth, setDiffPanelWidth] = useState(400);
  const [isResizingDiffPanel, setIsResizingDiffPanel] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(288);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [terminalOpenWorkspaceIds, setTerminalOpenWorkspaceIds] = useState<
    Set<string>
  >(() => new Set());
  const [gitDiff, setGitDiff] = useState<GitDiffResponse | null>(null);
  const [selectedDiffPath, setSelectedDiffPath] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [isSendingMessage, setIsSendingMessage] = useState(false);
  const [runningChatKey, setRunningChatKey] = useState<string | null>(null);
  const [retryRunRequest, setRetryRunRequest] =
    useState<RetryRunRequest | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeRunAbortRef = useRef<AbortController | null>(null);
  const hasManuallySelectedModelRef = useRef(false);

  const activeWorkspace = useMemo(
    () =>
      workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
      workspaces[0],
    [activeWorkspaceId, workspaces],
  );
  const availableModels = useMemo(
    () =>
      (settings?.configuredModels ?? []).filter(
        (model) =>
          model.enabled &&
          model.canEnable &&
          model.activeProviderId !== null &&
          model.providerIds.length > 0,
      ),
    [settings],
  );
  const detectedSkills = useMemo(
    () => settings?.skills.detected ?? [],
    [settings],
  );
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const selectedDiffText = formatDiffText(gitDiff);
  const isTerminalOpen = activeWorkspace
    ? terminalOpenWorkspaceIds.has(activeWorkspace.id)
    : false;
  const language = settings?.general.language ?? "en";
  const t = useCallback<Translate>(
    (key, values) => translate(key, values, language),
    [language],
  );

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const refreshWorkspaces = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>("/api/workspaces");
      setWorkspaces(data.workspaces);
      setActiveWorkspaceId((current) =>
        data.workspaces.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );
      setExpandedWorkspaceIds(
        new Set(data.workspaces.map((workspace) => workspace.id)),
      );
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
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, []);

  const loadGitDiff = useCallback(async (workspaceId: string, path: string | null) => {
    setIsLoadingDiff(true);
    setDiffError(null);

    try {
      const query = path ? `?path=${encodeURIComponent(path)}` : "";
      const data = await requestJson<GitDiffResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/diff${query}`,
      );
      setGitDiff(data);
      setSelectedDiffPath((current) => {
        if (path) {
          return path;
        }

        if (current && data.files.some((file) => file.path === current)) {
          return current;
        }

        return null;
      });
    } catch (requestError) {
      setGitDiff(null);
      setDiffError(errorMessage(requestError));
    } finally {
      setIsLoadingDiff(false);
    }
  }, []);

  const loadGitBranches = useCallback(async (workspaceId: string) => {
    setIsLoadingBranches(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/git/branches`,
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");
    } catch (requestError) {
      setGitBranches(null);
      setSelectedGitBranch("");
      setBranchError(errorMessage(requestError));
    } finally {
      setIsLoadingBranches(false);
    }
  }, []);

  useEffect(() => {
    void refreshWorkspaces();
  }, [refreshWorkspaces]);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setGitDiff(null);
      setSelectedDiffPath(null);
      setDiffError(null);
      return;
    }

    if (!isDiffPanelOpen) {
      return;
    }

    void loadGitDiff(activeWorkspace.id, selectedDiffPath);
  }, [activeWorkspace?.id, isDiffPanelOpen, loadGitDiff, selectedDiffPath]);

  useEffect(() => {
    if (!activeWorkspace?.id) {
      setGitBranches(null);
      setSelectedGitBranch("");
      setBranchError(null);
      return;
    }

    void loadGitBranches(activeWorkspace.id);
  }, [activeWorkspace?.id, loadGitBranches]);

  useEffect(() => {
    if (!isResizingDiffPanel) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const nextWidth = window.innerWidth - event.clientX;
      setDiffPanelWidth(Math.min(Math.max(nextWidth, 280), 720));
    }

    function handlePointerUp() {
      setIsResizingDiffPanel(false);
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizingDiffPanel]);

  useEffect(() => {
    if (!isResizingSidebar) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      setSidebarWidth(Math.min(Math.max(event.clientX, 232), 420));
    }

    function handlePointerUp() {
      setIsResizingSidebar(false);
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizingSidebar]);

  useEffect(() => {
    setSelectedModelId((current) => {
      const highestPriorityModelId = availableModels[0]?.id ?? "";

      if (!highestPriorityModelId) {
        hasManuallySelectedModelRef.current = false;
        return "";
      }

      if (!hasManuallySelectedModelRef.current) {
        return highestPriorityModelId;
      }

      if (availableModels.some((model) => model.id === current)) {
        return current;
      }

      hasManuallySelectedModelRef.current = false;
      return highestPriorityModelId;
    });
  }, [availableModels]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );
    setSelectedThinkingLevel(selectedModel?.thinkingLevel ?? "");
  }, [availableModels, selectedModelId]);

  useEffect(() => {
    const enabledSkillIds = new Set(
      detectedSkills.filter((skill) => skill.enabled).map((skill) => skill.id),
    );

    setSelectedSkillIds((current) => {
      const next = current.filter((skillId) => enabledSkillIds.has(skillId));
      return next.length === current.length ? current : next;
    });
  }, [detectedSkills]);

  async function handleWorkspaceSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingWorkspace(true);
    setError(null);

    try {
      const endpoint =
        formMode === "create"
          ? "/api/workspaces/create"
          : "/api/workspaces/add";
      const data = await requestJson<WorkspacesResponse>(endpoint, {
        body: JSON.stringify({
          name: workspaceName,
          path: workspacePath,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      const createdWorkspace = data.workspaces[data.workspaces.length - 1];

      setWorkspaces(data.workspaces);
      setActiveWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      setExpandedWorkspaceIds(
        new Set(data.workspaces.map((workspace) => workspace.id)),
      );
      setWorkspaceName("");
      setWorkspacePath("");
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  async function loadChatMessages(workspaceId: string, chatId: string) {
    setError(null);

    try {
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages`,
      );
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setMessages(data.messages.map(normalizeChatMessageSummary));
      setViewMode("chat");
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function selectWorkspace(workspaceId: string) {
    setActiveWorkspaceId(workspaceId);
    setActiveChatId(null);
    setMessages([]);
    setSelectedDiffPath(null);
  }

  function startNewWorkspaceChat(workspaceId: string) {
    setActiveWorkspaceId(workspaceId);
    setActiveChatId(null);
    setMessages([]);
    setSelectedDiffPath(null);
    setViewMode("chat");
  }

  async function deleteWorkspaceChat(workspaceId: string, chatId: string) {
    if (isSendingMessage && activeChatId === chatId) {
      setError(t("Cancel the current run before deleting this chat."));
      return;
    }

    setError(null);

    try {
      const data = await requestJson<WorkspacesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/delete`,
        { method: "POST" },
      );
      setWorkspaces(data.workspaces);
      setActiveWorkspaceId((current) =>
        data.workspaces.some((workspace) => workspace.id === current)
          ? current
          : data.activeWorkspaceId,
      );

      if (activeChatId === chatId) {
        setActiveWorkspaceId(workspaceId);
        setActiveChatId(null);
        setMessages([]);
      }

      setRetryRunRequest((current) =>
        current?.chatId === chatId ? null : current,
      );
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function toggleWorkspaceTerminal() {
    if (!activeWorkspace) {
      return;
    }

    setTerminalOpenWorkspaceIds((current) => {
      const next = new Set(current);

      if (next.has(activeWorkspace.id)) {
        next.delete(activeWorkspace.id);
      } else {
        next.add(activeWorkspace.id);
      }

      return next;
    });
  }

  function toggleSelectedSkill(skillId: string) {
    setSelectedSkillIds((current) =>
      current.includes(skillId)
        ? current.filter((id) => id !== skillId)
        : [...current, skillId],
    );
  }

  function removeSelectedSkill(skillId: string) {
    setSelectedSkillIds((current) => current.filter((id) => id !== skillId));
  }

  async function handleGitBranchChange(branch: string) {
    if (branch === CREATE_BRANCH_OPTION_VALUE) {
      setNewBranchName("");
      setBranchError(null);
      setIsBranchDialogOpen(true);
      return;
    }

    if (!activeWorkspace || !gitBranches?.isGitRepository || !branch) {
      return;
    }

    if (branch === selectedGitBranch) {
      return;
    }

    setIsLoadingBranches(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/branches/switch`,
        {
          body: JSON.stringify({ name: branch }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");

      if (isDiffPanelOpen) {
        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
      }
    } catch (requestError) {
      setBranchError(errorMessage(requestError));
    } finally {
      setIsLoadingBranches(false);
    }
  }

  async function handleCreateGitBranch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!activeWorkspace) {
      setBranchError(t("Select a workspace before creating a branch."));
      return;
    }

    const branch = newBranchName.trim();
    if (!branch) {
      setBranchError(t("Git branch name must not be empty."));
      return;
    }

    setIsSavingBranch(true);
    setBranchError(null);

    try {
      const data = await requestJson<GitBranchesResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/git/branches/create`,
        {
          body: JSON.stringify({ name: branch }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setGitBranches(data);
      setSelectedGitBranch(data.currentBranch ?? "");
      setNewBranchName("");
      setIsBranchDialogOpen(false);

      if (isDiffPanelOpen) {
        void loadGitDiff(activeWorkspace.id, selectedDiffPath);
      }
    } catch (requestError) {
      setBranchError(errorMessage(requestError));
    } finally {
      setIsSavingBranch(false);
    }
  }

  async function handleSendMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const content = draftMessage.trim();
    if (!content || isSendingMessage) {
      return;
    }

    if (!activeWorkspace) {
      setError(t("Select a workspace before sending."));
      return;
    }

    if (!selectedModelId) {
      setError(t("Select an enabled model before sending."));
      return;
    }

    await runChatMessage({
      chatId: activeChatId,
      content,
      modelId: selectedModelId,
      skillIds: selectedSkillIds,
      thinkingLevel: selectedThinkingLevel,
      workspaceId: activeWorkspace.id,
    });
  }

  async function handleRetryRun() {
    if (!retryRunRequest || isSendingMessage) {
      return;
    }

    const retryRequest = retryRunRequest;
    setActiveWorkspaceId(retryRequest.workspaceId);
    setActiveChatId(retryRequest.chatId);
    hasManuallySelectedModelRef.current = true;
    setSelectedModelId(retryRequest.modelId);
    setSelectedSkillIds(retryRequest.skillIds);
    setSelectedThinkingLevel(retryRequest.thinkingLevel);
    await runChatMessage(retryRequest);
  }

  function handleChatModelChange(modelId: string) {
    hasManuallySelectedModelRef.current = true;
    setSelectedModelId(modelId);
  }

  function handleCancelRun() {
    activeRunAbortRef.current?.abort();
  }

  async function runChatMessage(request: RetryRunRequest) {
    const runKey =
      globalThis.crypto?.randomUUID?.() ??
      `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const localUserId = `local-user-${runKey}`;
    const localAssistantId = `local-assistant-${runKey}`;
    const visibleUserContent = messageWithSelectedSkills(
      detectedSkills,
      request.skillIds,
      request.content,
    );
    let assistantMessageId = localAssistantId;
    let requestChatId = request.chatId;
    let currentRunningChatKey = requestChatId
      ? chatRunKey(request.workspaceId, requestChatId)
      : null;
    const abortController = new AbortController();

    setMessages((current) => [
      ...current,
      {
        id: localUserId,
        role: "user",
        content: visibleUserContent,
        reasoning: null,
        toolCalls: [],
        parts: [{ type: "text", text: visibleUserContent }],
      },
      {
        id: localAssistantId,
        role: "assistant",
        content: "",
        reasoning: null,
        status: "streaming",
        toolCalls: [],
        parts: [],
      },
    ]);
    setDraftMessage("");
    setIsSendingMessage(true);
    setRunningChatKey(currentRunningChatKey);
    setRetryRunRequest(null);
    setError(null);
    activeRunAbortRef.current = abortController;

    const isCurrentAssistantMessage = (
      message: ShellMessage,
      eventAssistantMessageId?: string,
    ) =>
      message.role === "assistant" &&
      (message.id === assistantMessageId ||
        message.id === localAssistantId ||
        message.id === eventAssistantMessageId);

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/stream`,
        {
          body: JSON.stringify({
            chatId: request.chatId,
            message: request.content,
            modelId: request.modelId,
            skillIds: request.skillIds.length ? request.skillIds : null,
            thinkingLevel: request.thinkingLevel || null,
          }),
          cache: "no-store",
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      );

      if (!response.ok) {
        throw new Error(await responseErrorMessage(response));
      }

      await readChatStream(response, (streamEvent) => {
        if (streamEvent.type === "start") {
          assistantMessageId = streamEvent.assistantMessageId;
          requestChatId = streamEvent.chatId;
          currentRunningChatKey = chatRunKey(
            request.workspaceId,
            streamEvent.chatId,
          );
          setActiveChatId(streamEvent.chatId);
          setRunningChatKey(currentRunningChatKey);
          void refreshWorkspaces();
          setMessages((current) =>
            current.map((message) => {
              if (message.id === localUserId) {
                return { ...message, id: streamEvent.userMessageId };
              }

              if (message.role === "assistant" && message.id === localAssistantId) {
                return { ...message, id: streamEvent.assistantMessageId };
              }

              return message;
            }),
          );
          return;
        }

        if (streamEvent.type === "textDelta") {
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    content: message.content + streamEvent.delta,
                    parts: appendTextPart(message.parts, streamEvent.delta),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "reasoningDelta") {
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                  ? {
                      ...message,
                      reasoning: `${message.reasoning ?? ""}${streamEvent.delta}`,
                      parts: appendReasoningPart(
                        message.parts,
                        streamEvent.delta,
                      ),
                    }
                  : message,
              ),
          );
          return;
        }

        if (streamEvent.type === "complete") {
          setActiveChatId(streamEvent.chatId);
          setRetryRunRequest(null);
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? completedAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    toolCalls: upsertToolCall(
                      message.toolCalls,
                      streamEvent.toolCall,
                    ),
                    parts: upsertToolCallPart(message.parts, streamEvent.toolCall),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolResult") {
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? {
                    ...message,
                    toolCalls: applyToolResult(
                      message.toolCalls,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                    parts: applyToolResultToParts(
                      message.parts,
                      streamEvent.toolCallId,
                      streamEvent.output,
                      streamEvent.isError,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          return;
        }

        if (streamEvent.type === "error") {
          setError(streamEvent.message);
          setMessages((current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message)
                ? {
                    ...message,
                    content: streamEvent.message,
                    parts: [{ type: "text", text: streamEvent.message }],
                    status: "error",
                  }
                : message,
            ),
          );
        }
      });

      await refreshWorkspaces();
    } catch (requestError) {
      const wasCancelled =
        requestError instanceof DOMException && requestError.name === "AbortError";
      const message = wasCancelled ? t("Run cancelled.") : errorMessage(requestError);
      setError(message);
      setRetryRunRequest({
        ...request,
        chatId: requestChatId,
      });
      setMessages((current) =>
        current.map((item) =>
          isCurrentAssistantMessage(item)
            ? {
                ...item,
                content: message,
                parts: [{ type: "text", text: message }],
                status: "error",
              }
            : item,
        ),
      );
    } finally {
      if (activeRunAbortRef.current === abortController) {
        activeRunAbortRef.current = null;
      }
      setRunningChatKey((current) =>
        current === currentRunningChatKey ? null : current,
      );
      setIsSendingMessage(false);
    }
  }

  function toggleWorkspace(workspaceId: string) {
    setExpandedWorkspaceIds((current) => {
      const next = new Set(current);

      if (next.has(workspaceId)) {
        next.delete(workspaceId);
      } else {
        next.add(workspaceId);
      }

      return next;
    });
  }

  function openWorkspaceDialog(mode: WorkspaceFormMode) {
    setFormMode(mode);
    setWorkspaceName("");
    setWorkspacePath("");
    setError(null);
    setIsWorkspaceDialogOpen(true);
  }

  return (
    <I18nContext.Provider value={{ language, t }}>
    <main className="app-root text-stone-950">
      <div
        className={`app-shell ${isDiffPanelOpen ? "app-shell-with-diff" : ""}`}
        style={
          {
            "--diff-panel-width": `${diffPanelWidth}px`,
            "--sidebar-width": `${sidebarWidth}px`,
          } as CSSProperties
        }
      >
        <aside className="workspace-sidebar relative border-stone-200/80 lg:border-r">
          <div
            aria-label={t("Resize workspace sidebar")}
            aria-orientation="vertical"
            className="absolute bottom-0 right-0 top-0 z-10 hidden w-1 cursor-col-resize bg-transparent hover:bg-teal-500/40 lg:block"
            onKeyDown={(event) => {
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                setSidebarWidth((current) => Math.max(current - 24, 232));
              }

              if (event.key === "ArrowRight") {
                event.preventDefault();
                setSidebarWidth((current) => Math.min(current + 24, 420));
              }
            }}
            onPointerDown={() => setIsResizingSidebar(true)}
            role="separator"
            tabIndex={0}
          />
          <div className="flex h-full min-h-0 flex-col">
            <div className="flex items-center justify-between border-b border-stone-200/80 px-4 py-3">
              <div className="flex min-w-0 items-center gap-3">
                <span className="inline-flex size-9 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_10px_24px_rgba(15,118,110,0.24)]">
                  <Activity aria-hidden="true" className="size-5" />
                </span>
                <div className="min-w-0">
                  <span className="block truncate text-lg font-semibold">
                    Foco
                  </span>
                  <span className="block truncate text-xs text-stone-500">
                    {t("Local workspace")}
                  </span>
                </div>
              </div>
              <button
                aria-label={t("Refresh workspaces")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={isLoading}
                onClick={() => void refreshWorkspaces()}
                title={t("Refresh workspaces")}
                type="button"
              >
                {isLoading ? (
                  <LoaderCircle
                    aria-hidden="true"
                    className="size-4 animate-spin"
                  />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
              </button>
            </div>

            <div className="border-b border-stone-200/80 px-4 py-2">
              <button
                aria-label={t("Create or add workspace")}
                className={`${workspaceActionClass()} w-full`}
                onClick={() => openWorkspaceDialog("create")}
                title={t("Create or add workspace")}
                type="button"
              >
                <FolderPlus aria-hidden="true" className="size-4" />
              </button>
            </div>

            {error ? (
              <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
                {error}
              </div>
            ) : null}

            <nav className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
              {workspaces.length ? (
                workspaces.map((workspace) => {
                const isExpanded = expandedWorkspaceIds.has(workspace.id);
                const isActive = workspace.id === activeWorkspace?.id;

                return (
                  <div className="mb-1.5" key={workspace.id}>
                    <div className="flex items-center gap-1">
                      <button
                        aria-label={
                          isExpanded
                            ? t("Collapse chat history")
                            : t("Expand chat history")
                        }
                        className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                        onClick={() => toggleWorkspace(workspace.id)}
                        title={
                          isExpanded
                            ? t("Collapse chat history")
                            : t("Expand chat history")
                        }
                        type="button"
                      >
                        {isExpanded ? (
                          <ChevronDown aria-hidden="true" className="size-4" />
                        ) : (
                          <ChevronRight
                            aria-hidden="true"
                            className="size-4"
                          />
                        )}
                      </button>
                      <button
                        className={workspaceItemClass(isActive)}
                        onClick={() => selectWorkspace(workspace.id)}
                        type="button"
                      >
                        <Folder aria-hidden="true" className="size-4 shrink-0" />
                        <span className="min-w-0 flex-1 truncate text-left">
                          {workspace.name}
                        </span>
                      </button>
                      <button
                        aria-label={t("New chat in {name}", {
                          name: workspace.name,
                        })}
                        className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 hover:bg-teal-50 hover:text-teal-800"
                        onClick={() => startNewWorkspaceChat(workspace.id)}
                        title={t("New chat")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    {isExpanded ? (
                      <div className="ml-9 mt-1 space-y-1">
                        {workspace.chats.length > 0 ? (
                          workspace.chats.map((chat) => {
                            const isChatRunning =
                              runningChatKey === chatRunKey(workspace.id, chat.id);

                            return (
                              <div
                                className="group flex min-w-0 items-center gap-1"
                                key={chat.id}
                              >
                                <button
                                  className={`flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs font-medium ${
                                    activeWorkspace?.id === workspace.id &&
                                    activeChatId === chat.id
                                      ? "bg-white text-stone-950 shadow-sm"
                                      : "text-stone-600 hover:bg-white/80 hover:text-stone-950"
                                  }`}
                                  onClick={() =>
                                    void loadChatMessages(workspace.id, chat.id)
                                  }
                                  type="button"
                                >
                                  {isChatRunning ? (
                                    <LoaderCircle
                                      aria-hidden="true"
                                      className="size-3.5 shrink-0 animate-spin text-teal-700"
                                    />
                                  ) : (
                                    <MessageSquare
                                      aria-hidden="true"
                                      className="size-3.5 shrink-0"
                                    />
                                  )}
                                  <span className="min-w-0 flex-1">
                                    <span className="block truncate">
                                      {chat.title}
                                    </span>
                                    <span className="mt-0.5 block truncate text-[0.68rem] font-normal leading-tight text-stone-400">
                                      {formatChatCreatedAt(chat.createdAt)}
                                    </span>
                                  </span>
                                </button>
                                <button
                                  aria-label={t("Delete chat {title}", {
                                    title: chat.title,
                                  })}
                                  className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100"
                                  onClick={() =>
                                    void deleteWorkspaceChat(workspace.id, chat.id)
                                  }
                                  title={t("Delete chat")}
                                  type="button"
                                >
                                  <Trash2
                                    aria-hidden="true"
                                    className="size-3.5"
                                  />
                                </button>
                              </div>
                            );
                          })
                        ) : (
                          <div className="rounded-lg px-2 py-1.5 text-xs text-stone-500">
                            {t("No chats")}
                          </div>
                        )}
                      </div>
                    ) : null}
                  </div>
                );
                })
              ) : (
                <div className="mx-2 rounded-lg border border-dashed border-stone-300 bg-white/60 px-3 py-4 text-sm text-stone-500">
                  {isLoading ? t("Loading workspaces...") : t("No workspaces")}
                </div>
              )}
            </nav>
          </div>
        </aside>

        <section className="app-main-panel flex min-w-0 flex-col">
          <header className="shrink-0 border-b border-stone-200/80 bg-white/80 px-4 py-2 backdrop-blur sm:px-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <h1 className="truncate text-lg font-semibold text-stone-950">
                  {activeWorkspace?.name ?? t("Workspace")}
                </h1>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {activeWorkspace?.path ?? ""}
                </p>
              </div>
              <div className="flex overflow-x-auto rounded-xl border border-stone-200 bg-stone-100/80 p-1 shadow-inner">
                <NavButton
                  active={viewMode === "chat"}
                  icon={MessageSquare}
                  label={t("Chat")}
                  onClick={() => setViewMode("chat")}
                />
                <NavButton
                  active={viewMode === "settings"}
                  icon={Settings}
                  label={t("Settings")}
                  onClick={() => setViewMode("settings")}
                />
                <NavButton
                  active={viewMode === "stats"}
                  icon={BarChart3}
                  label={t("Stats")}
                  onClick={() => setViewMode("stats")}
                />
                <button
                  aria-label={
                    isTerminalOpen ? t("Close terminal") : t("Open terminal")
                  }
                  className={`inline-flex size-9 items-center justify-center rounded-lg ${
                    isTerminalOpen
                      ? "bg-white text-teal-900 shadow-sm"
                      : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
                  } disabled:cursor-not-allowed disabled:text-stone-400`}
                  disabled={!activeWorkspace}
                  onClick={toggleWorkspaceTerminal}
                  title={isTerminalOpen ? t("Close terminal") : t("Open terminal")}
                  type="button"
                >
                  <Terminal aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={
                    isDiffPanelOpen ? t("Close git diff") : t("Open git diff")
                  }
                  className={`inline-flex size-9 items-center justify-center rounded-lg ${
                    isDiffPanelOpen
                      ? "bg-white text-teal-900 shadow-sm"
                      : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
                  }`}
                  onClick={() => setIsDiffPanelOpen((current) => !current)}
                  title={isDiffPanelOpen ? t("Close git diff") : t("Open git diff")}
                  type="button"
                >
                  <GitCompare aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
          </header>

          {viewMode === "chat" ? (
            <ChatPanel
              availableModels={availableModels}
              branchError={branchError}
              chatScrollKey={`${activeWorkspaceId}:${activeChatId ?? ""}`}
              draftMessage={draftMessage}
              gitBranches={gitBranches}
              isLoadingSettings={isLoadingSettings}
              isLoadingBranches={isLoadingBranches}
              isSendingMessage={isSendingMessage}
              messages={messages}
              onBranchChange={(branch) => void handleGitBranchChange(branch)}
              onDraftMessageChange={setDraftMessage}
              onCancelRun={handleCancelRun}
              onModelChange={handleChatModelChange}
              onRemoveSkill={removeSelectedSkill}
              onRetryRun={() => void handleRetryRun()}
              onSubmit={handleSendMessage}
              onThinkingLevelChange={setSelectedThinkingLevel}
              onToggleSkill={toggleSelectedSkill}
              canRetryRun={retryRunRequest !== null && !isSendingMessage}
              selectedGitBranch={selectedGitBranch}
              selectedModelId={selectedModelId}
              selectedSkillIds={selectedSkillIds}
              selectedThinkingLevel={selectedThinkingLevel}
              skills={detectedSkills}
              thinkingLevels={thinkingLevels}
            />
          ) : viewMode === "settings" ? (
            <SettingsPanel onSettingsChange={setSettings} />
          ) : (
            <ApiStatsPanel
              activeWorkspace={activeWorkspace}
              availableModels={availableModels}
              settings={settings}
            />
          )}
          {isTerminalOpen ? (
            <TerminalPanel workspace={activeWorkspace} />
          ) : null}
        </section>

        {isDiffPanelOpen ? (
        <aside className="diff-sidebar min-w-0 border-stone-200/80 lg:border-l">
          <GitDiffPanel
            diffError={diffError}
            diffText={selectedDiffText}
            files={gitDiff?.files ?? []}
            isLoading={isLoadingDiff}
            onClose={() => setIsDiffPanelOpen(false)}
            onRefresh={() => {
              if (activeWorkspace?.id) {
                void loadGitDiff(activeWorkspace.id, selectedDiffPath);
              }
            }}
            onResizeBy={(delta) =>
              setDiffPanelWidth((current) =>
                Math.min(Math.max(current + delta, 280), 720),
              )
            }
            onResizeStart={() => setIsResizingDiffPanel(true)}
            onSelectFile={setSelectedDiffPath}
            selectedPath={selectedDiffPath}
          />
        </aside>
        ) : null}
      </div>
      {isWorkspaceDialogOpen ? (
        <WorkspaceDialog
          formMode={formMode}
          isSaving={isSavingWorkspace}
          name={workspaceName}
          onClose={() => setIsWorkspaceDialogOpen(false)}
          onModeChange={setFormMode}
          onNameChange={setWorkspaceName}
          onPathChange={setWorkspacePath}
          onSubmit={handleWorkspaceSubmit}
          path={workspacePath}
        />
      ) : null}
      {isBranchDialogOpen ? (
        <GitBranchDialog
          branchName={newBranchName}
          error={branchError}
          isSaving={isSavingBranch}
          onBranchNameChange={setNewBranchName}
          onClose={() => setIsBranchDialogOpen(false)}
          onSubmit={handleCreateGitBranch}
        />
      ) : null}
    </main>
    </I18nContext.Provider>
  );
}

function WorkspaceDialog({
  formMode,
  isSaving,
  name,
  onClose,
  onModeChange,
  onNameChange,
  onPathChange,
  onSubmit,
  path,
}: {
  formMode: WorkspaceFormMode;
  isSaving: boolean;
  name: string;
  onClose: () => void;
  onModeChange: (mode: WorkspaceFormMode) => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  path: string;
}) {
  const { t } = useI18n();
  const title =
    formMode === "create"
      ? t("Create workspace")
      : t("Add existing workspace");

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="workspace-dialog-title"
        aria-modal="true"
        className="w-full max-w-lg overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="min-w-0">
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="workspace-dialog-title"
            >
              {title}
            </h2>
            <p className="mt-1 truncate text-xs font-medium text-stone-500">
              {formMode === "create"
                ? t("Create and register a new local folder.")
                : t("Register an existing local folder.")}
            </p>
          </div>
          <button
            aria-label={t("Close workspace dialog")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <div className="grid grid-cols-2 gap-2 border-b border-stone-200 bg-stone-50/80 px-4 py-3">
          <button
            aria-label={t("Switch to create workspace")}
            className={workspaceModeClass(formMode === "create")}
            onClick={() => onModeChange("create")}
            title={t("Create workspace")}
            type="button"
          >
            <Plus aria-hidden="true" className="size-4" />
          </button>
          <button
            aria-label={t("Switch to add workspace")}
            className={workspaceModeClass(formMode === "add")}
            onClick={() => onModeChange("add")}
            title={t("Add workspace")}
            type="button"
          >
            <FolderPlus aria-hidden="true" className="size-4" />
          </button>
        </div>

        <form
          className="space-y-4 px-4 py-4"
          onSubmit={(event) => void onSubmit(event)}
        >
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Name")}
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="workspace-name"
              onChange={(event) => onNameChange(event.target.value)}
              placeholder={t("Workspace name")}
              value={name}
            />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Path")}
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="workspace-path"
              onChange={(event) => onPathChange(event.target.value)}
              placeholder="C:\\Users\\name\\workspace"
              value={path}
            />
          </label>
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel workspace dialog")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={title}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving}
              title={title}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : formMode === "create" ? (
                <Plus aria-hidden="true" className="size-4" />
              ) : (
                <FolderPlus aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function GitBranchDialog({
  branchName,
  error,
  isSaving,
  onBranchNameChange,
  onClose,
  onSubmit,
}: {
  branchName: string;
  error: string | null;
  isSaving: boolean;
  onBranchNameChange: (value: string) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const { t } = useI18n();
  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="git-branch-dialog-title"
        aria-modal="true"
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <GitBranch aria-hidden="true" className="size-5 text-teal-700" />
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="git-branch-dialog-title"
            >
              {t("New branch")}
            </h2>
          </div>
          <button
            aria-label={t("Close branch dialog")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <form className="space-y-4 px-4 py-4" onSubmit={onSubmit}>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Branch name")}
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="git-branch-name"
              onChange={(event) => onBranchNameChange(event.target.value)}
              placeholder="feature/name"
              value={branchName}
            />
          </label>
          {error ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel branch creation")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Create branch")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving || !branchName.trim()}
              title={t("Create branch")}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-4 animate-spin"
                />
              ) : (
                <Plus aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function ChatPanel({
  availableModels,
  branchError,
  chatScrollKey,
  canRetryRun,
  draftMessage,
  gitBranches,
  isLoadingBranches,
  isLoadingSettings,
  isSendingMessage,
  messages,
  onBranchChange,
  onCancelRun,
  onDraftMessageChange,
  onModelChange,
  onRemoveSkill,
  onRetryRun,
  onSubmit,
  onThinkingLevelChange,
  onToggleSkill,
  selectedGitBranch,
  selectedModelId,
  selectedSkillIds,
  selectedThinkingLevel,
  skills,
  thinkingLevels,
}: {
  availableModels: ConfiguredModelSummary[];
  branchError: string | null;
  chatScrollKey: string;
  canRetryRun: boolean;
  draftMessage: string;
  gitBranches: GitBranchesResponse | null;
  isLoadingBranches: boolean;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  messages: ShellMessage[];
  onBranchChange: (value: string) => void;
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onModelChange: (value: string) => void;
  onRemoveSkill: (skillId: string) => void;
  onRetryRun: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onThinkingLevelChange: (value: string) => void;
  onToggleSkill: (skillId: string) => void;
  selectedGitBranch: string;
  selectedModelId: string;
  selectedSkillIds: string[];
  selectedThinkingLevel: string;
  skills: ConfiguredSkillSummary[];
  thinkingLevels: ThinkingLevelSummary[];
}) {
  const { t } = useI18n();
  const messageScrollRef = useRef<HTMLDivElement>(null);
  const messageScrollContentRef = useRef<HTMLDivElement>(null);
  const messageScrollEndRef = useRef<HTMLDivElement>(null);
  const shouldLockMessageScrollRef = useRef(true);
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = new Set(selectedSkillIds);
  const selectedSkills = selectedSkillIds
    .map((skillId) => skills.find((skill) => skill.id === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill));
  const visibleSkills =
    skillQuery === null
      ? []
      : skills.filter((skill) => {
          const query = skillQuery.toLowerCase();
          return (
            !selectedSkillSet.has(skill.id) &&
            (skill.name.toLowerCase().includes(query) ||
              skill.id.toLowerCase().includes(query) ||
              skill.description.toLowerCase().includes(query))
          );
        });

  function scrollMessageListToBottom() {
    messageScrollEndRef.current?.scrollIntoView({
      block: "end",
      inline: "nearest",
    });
  }

  useLayoutEffect(() => {
    shouldLockMessageScrollRef.current = true;
    scrollMessageListToBottom();
  }, [chatScrollKey]);

  useLayoutEffect(() => {
    if (!shouldLockMessageScrollRef.current) {
      return;
    }

    scrollMessageListToBottom();
  }, [messages]);

  useLayoutEffect(() => {
    const container = messageScrollRef.current;
    const content = messageScrollContentRef.current;
    if (!container || !content) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldLockMessageScrollRef.current) {
        scrollMessageListToBottom();
      }
    });
    observer.observe(container);
    observer.observe(content);

    return () => observer.disconnect();
  }, []);

  function handleMessageScroll() {
    const element = messageScrollRef.current;
    if (!element) {
      return;
    }

    shouldLockMessageScrollRef.current =
      element.scrollHeight - element.scrollTop - element.clientHeight <=
      CHAT_BOTTOM_LOCK_THRESHOLD_PX;
  }

  function handleSkillSelect(skill: ConfiguredSkillSummary) {
    if (!skill.enabled) {
      return;
    }

    onDraftMessageChange(removeActiveSkillToken(draftMessage));
    onToggleSkill(skill.id);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div
        className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4"
        onScroll={handleMessageScroll}
        ref={messageScrollRef}
      >
        <div
          className="mx-auto flex w-full max-w-5xl flex-col gap-4"
          ref={messageScrollContentRef}
        >
          {messages.length ? (
            messages.map((message) => {
            const isUser = message.role === "user";
            const parts = message.parts.length
              ? message.parts
              : fallbackMessageParts(message);

            return (
              <div
                className={`flex ${isUser ? "justify-end" : "justify-start"}`}
                key={message.id}
              >
                <div
                  className={`flex max-w-[min(42rem,92%)] items-start gap-3 rounded-2xl border px-4 py-3 shadow-[0_18px_42px_rgba(75,63,42,0.08)] sm:max-w-[78%] ${
                    isUser
                      ? "flex-row-reverse rounded-tr-md border-teal-700 bg-teal-800 text-white"
                      : "flex-row rounded-tl-md border-stone-200 bg-white/90 text-stone-900"
                  }`}
                >
                  <div
                    className={`mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-xl ${
                      isUser
                        ? "bg-teal-950/45 text-white"
                        : "bg-stone-100 text-stone-700"
                    }`}
                  >
                    {isUser ? (
                      <User aria-hidden="true" className="size-4" />
                    ) : (
                      <Bot aria-hidden="true" className="size-4" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1 space-y-3">
                    {parts.length ? (
                      parts.map((part, partIndex) => (
                        <MessagePartBlock
                          isError={message.status === "error"}
                          isUser={isUser}
                          key={`${message.id}-part-${partIndex}`}
                          part={part}
                        />
                      ))
                    ) : message.status === "streaming" ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin"
                      />
                    ) : null}
                  </div>
                </div>
              </div>
            );
            })
          ) : (
            <div className="mx-auto flex min-h-[22rem] w-full max-w-xl flex-col items-center justify-center rounded-2xl border border-dashed border-stone-300 bg-white/60 px-6 py-10 text-center shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
              <div className="inline-flex size-11 items-center justify-center rounded-2xl bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)]">
                <Bot aria-hidden="true" className="size-5" />
              </div>
              <h2 className="mt-4 text-base font-semibold text-stone-950">
                {t("Workspace shell is ready")}
              </h2>
              <p className="mt-2 max-w-sm text-sm leading-6 text-stone-600">
                {t("Pick an enabled model and start the current workspace chat.")}
              </p>
            </div>
          )}
        </div>
        <div aria-hidden="true" className="h-px" ref={messageScrollEndRef} />
      </div>

      <div className="shrink-0 border-t border-stone-200/80 bg-white/80 px-3 py-2 backdrop-blur sm:px-5">
        <form className="mx-auto max-w-5xl" onSubmit={onSubmit}>
          <div className="relative rounded-xl border border-stone-300 bg-white">
            {selectedSkills.length ? (
              <div className="flex flex-wrap gap-1.5 px-3 pt-2">
                {selectedSkills.map((skill) => (
                  <span
                    className="inline-flex max-w-full items-center gap-1 rounded-full border border-teal-200 bg-teal-50 px-2 py-1 text-xs font-semibold text-teal-900"
                    key={skill.id}
                  >
                    <span className="max-w-44 truncate">{skill.name}</span>
                    <button
                      aria-label={t("Remove skill {name}", {
                        name: skill.name,
                      })}
                      className="inline-flex size-4 items-center justify-center rounded-full text-teal-800 hover:bg-teal-100"
                      disabled={isSendingMessage}
                      onClick={() => onRemoveSkill(skill.id)}
                      title={t("Remove skill")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
            ) : null}
            <textarea
              className="message-composer-textarea min-h-24 w-full resize-none border-0 bg-transparent px-3 py-2 text-sm leading-6 text-stone-900 outline-none placeholder:text-stone-400"
              disabled={isSendingMessage}
              name="message"
              onChange={(event) => onDraftMessageChange(event.target.value)}
              onKeyDown={(event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
                if (
                  event.key !== "Enter" ||
                  event.ctrlKey ||
                  event.nativeEvent.isComposing
                ) {
                  return;
                }

                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }}
              placeholder={t("Message Foco")}
              value={draftMessage}
            />
            {skillQuery !== null ? (
              <div className="absolute bottom-full left-0 z-20 mb-2 w-full overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
                <div className="panel-scroll max-h-64 overflow-y-auto py-1">
                  {visibleSkills.length ? (
                    visibleSkills.map((skill) => (
                      <button
                        aria-label={t("Select skill {name}", {
                          name: skill.name,
                        })}
                        className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 px-3 py-2 text-left hover:bg-stone-50 disabled:cursor-not-allowed disabled:bg-stone-50 disabled:text-stone-400"
                        disabled={!skill.enabled || isSendingMessage}
                        key={skill.id}
                        onClick={() => handleSkillSelect(skill)}
                        title={
                          skill.enabled ? skill.description : t("Skill is disabled")
                        }
                        type="button"
                      >
                        <span className="min-w-0">
                          <span className="block truncate text-sm font-semibold text-stone-900">
                            {skill.name}
                          </span>
                          <span className="mt-0.5 block truncate text-xs text-stone-500">
                            {skill.description}
                          </span>
                        </span>
                        <span className="self-center rounded-md border border-stone-200 px-1.5 py-0.5 text-[11px] font-semibold text-stone-500">
                          {skill.enabled ? skill.id : t("disabled")}
                        </span>
                      </button>
                    ))
                  ) : (
                    <div className="px-3 py-3 text-sm text-stone-500">
                      {t("No matching skills")}
                    </div>
                  )}
                </div>
              </div>
            ) : null}
            <div className="flex flex-wrap items-center gap-2 border-t border-stone-100 px-2 py-2">
              <label className="min-w-36 flex-1 sm:max-w-64">
                <span className="sr-only">{t("Model")}</span>
                <select
                  className="h-8 w-full rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  disabled={isLoadingSettings || isSendingMessage}
                  onChange={(event) => onModelChange(event.target.value)}
                  value={selectedModelId}
                >
                  {availableModels.length ? (
                    availableModels.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.displayName}
                      </option>
                    ))
                  ) : (
                    <option value="">{t("No enabled models")}</option>
                  )}
                </select>
              </label>
              <label className="w-36 max-w-full">
                <span className="sr-only">{t("Thinking")}</span>
                <select
                  className="h-8 w-full rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  disabled={isSendingMessage}
                  onChange={(event) => onThinkingLevelChange(event.target.value)}
                  value={selectedThinkingLevel}
                >
                  <option value="">{t("Model default")}</option>
                  {thinkingLevels.map((level) => (
                    <option key={level.value} value={level.value}>
                      {t(level.label)}
                    </option>
                  ))}
                </select>
              </label>
              <BranchSelector
                branches={gitBranches?.branches ?? []}
                currentBranch={selectedGitBranch}
                disabled={isSendingMessage || isLoadingBranches}
                isGitRepository={gitBranches?.isGitRepository ?? false}
                isLoading={isLoadingBranches}
                onChange={onBranchChange}
              />
            {canRetryRun ? (
              <button
                aria-label={t("Retry last run")}
                className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                onClick={onRetryRun}
                title={t("Retry last run")}
                type="button"
              >
                <RefreshCw aria-hidden="true" className="size-4" />
              </button>
            ) : null}
              {isSendingMessage ? (
                <button
                  aria-label={t("Cancel run")}
                  className="ml-auto inline-flex size-8 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                  onClick={onCancelRun}
                  title={t("Cancel run")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              ) : (
                <button
                  aria-label={t("Send message")}
                  className="ml-auto inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                  disabled={!draftMessage.trim() || !selectedModelId}
                  title={t("Send")}
                  type="submit"
                >
                  <Send aria-hidden="true" className="size-4" />
                </button>
              )}
          </div>
          </div>
          {branchError ? (
            <div className="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {branchError}
            </div>
          ) : null}
        </form>
      </div>
    </div>
  );
}

function BranchSelector({
  branches,
  currentBranch,
  disabled,
  isGitRepository,
  isLoading,
  onChange,
}: {
  branches: string[];
  currentBranch: string;
  disabled: boolean;
  isGitRepository: boolean;
  isLoading: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useI18n();
  if (!isGitRepository) {
    return (
      <div
        aria-label={t("Git branch")}
        className="inline-flex h-8 w-44 max-w-full items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-400"
      >
        <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
        <span className="min-w-0 flex-1 truncate" />
      </div>
    );
  }

  function handleSelect(value: string, event: ReactMouseEvent<HTMLButtonElement>) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onChange(value);
  }

  return (
    <details className="group relative">
      <summary
        className={`flex h-8 w-44 max-w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${
          disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
        }`}
        title={t("Git branch")}
      >
        <GitBranch aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="min-w-0 flex-1 truncate">{currentBranch}</span>
        {isLoading ? (
          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        ) : (
          <ChevronDown aria-hidden="true" className="size-3.5" />
        )}
      </summary>
      <div className="absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-52 overflow-y-auto py-1">
          {branches.length ? (
            branches.map((branch) => (
              <button
                aria-label={t("Switch to branch {name}", { name: branch })}
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${
                  branch === currentBranch
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                }`}
                key={branch}
                onClick={(event) => handleSelect(branch, event)}
                type="button"
              >
                <GitBranch aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{branch}</span>
                {branch === currentBranch ? (
                  <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                ) : null}
              </button>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">
              {t("No branches")}
            </div>
          )}
        </div>
        <div className="border-t border-stone-100 bg-white p-1.5">
          <button
            aria-label={t("Create git branch")}
            className="flex h-9 w-full items-center gap-2 rounded-lg px-2 text-sm font-semibold text-teal-800 hover:bg-teal-50"
            onClick={(event) => handleSelect(CREATE_BRANCH_OPTION_VALUE, event)}
            type="button"
          >
            <Plus aria-hidden="true" className="size-4" />
            <span className="min-w-0 flex-1 text-left">{t("New branch")}</span>
          </button>
        </div>
      </div>
    </details>
  );
}

function ReasoningBlock({ reasoning }: { reasoning: string }) {
  const { t } = useI18n();

  return (
    <div className="reasoning-block min-w-0 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
      <div className="mb-1.5 text-xs font-semibold text-stone-500">
        {t("Thinking")}
      </div>
      <MarkdownContent content={reasoning} isUser={false} variant="reasoning" />
    </div>
  );
}

function MessagePartBlock({
  isError,
  isUser,
  part,
}: {
  isError: boolean;
  isUser: boolean;
  part: ChatMessagePart;
}) {
  if (part.type === "reasoning") {
    return <ReasoningBlock reasoning={part.text} />;
  }

  if (part.type === "toolCall") {
    return <ToolCallBlock toolCall={part.toolCall} />;
  }

  return (
    <MarkdownContent content={part.text} isError={isError} isUser={isUser} />
  );
}

function MarkdownContent({
  content,
  isError = false,
  isUser,
  variant = "message",
}: {
  content: string;
  isError?: boolean;
  isUser: boolean;
  variant?: "message" | "reasoning";
}) {
  const skillPrefix = selectedSkillPrefix(content, isUser);
  const markdownContent = skillPrefix?.remaining ?? content;

  return (
    <div
      className={`markdown-content min-w-0 break-words text-sm leading-6 ${
        isUser ? "markdown-content-user" : "markdown-content-assistant"
      } ${variant === "reasoning" ? "markdown-content-reasoning" : ""} ${
        isError ? "text-rose-700" : ""
      }`}
    >
      {skillPrefix ? (
        <div className="message-skill-chip-row">
          {skillPrefix.skills.map((skill) => (
            <span
              aria-label={skill.path}
              className="message-skill-chip"
              key={`${skill.name}-${skill.path}`}
              title={skill.path}
            >
              {skill.name}
            </span>
          ))}
        </div>
      ) : null}
      {markdownContent ? (
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{markdownContent}</ReactMarkdown>
      ) : null}
    </div>
  );
}

function ToolCallBlock({ toolCall }: { toolCall: ChatToolCallSummary }) {
  const { t } = useI18n();
  const input = normalizedToolInput(toolCall.input);
  const detailText = toolCallDetailText(toolCall);

  return (
    <div className="min-w-0 border-t border-stone-200 pt-2">
      <details className="group min-w-0">
        <summary className="flex cursor-pointer list-none items-start gap-2 text-xs font-semibold text-stone-700 marker:hidden">
          <Wrench aria-hidden="true" className="mt-0.5 size-3.5 shrink-0 text-teal-700" />
          <span className="min-w-0 flex-1">
            <span className="block truncate">{toolCall.name}</span>
            <span
              className="mt-0.5 block truncate font-mono text-[11px] font-medium leading-4 text-stone-500"
              title={detailText}
            >
              {detailText}
            </span>
          </span>
          <span
            className={`shrink-0 rounded-md px-1.5 py-0.5 text-[11px] ${
              toolCall.isError
                ? "bg-rose-50 text-rose-700"
                : "bg-stone-100 text-stone-600"
            }`}
          >
            {toolStatusText(toolCall, t)}
          </span>
        </summary>
        <div className="mt-2 grid gap-2 text-xs text-stone-600">
          <div className="min-w-0">
            <div className="mb-1 font-semibold text-stone-500">{t("Input")}</div>
            <pre className="panel-scroll max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-stone-200 pl-3 font-mono text-[11px] leading-5">
              {formatJsonValue(input)}
            </pre>
          </div>
          {toolCall.output !== null ? (
            <div className="min-w-0">
              <div className="mb-1 font-semibold text-stone-500">{t("Output")}</div>
              <pre
                className={`panel-scroll max-h-64 overflow-auto whitespace-pre-wrap break-words border-l pl-3 font-mono text-[11px] leading-5 ${
                  toolCall.isError
                    ? "border-rose-200 text-rose-700"
                    : "border-stone-200"
                }`}
              >
                {formatJsonValue(toolCall.output)}
              </pre>
            </div>
          ) : null}
        </div>
      </details>
    </div>
  );
}

function TerminalPanel({ workspace }: { workspace: WorkspaceSummary | undefined }) {
  const { t } = useI18n();
  const tRef = useRef(t);
  const [cwd, setCwd] = useState("");
  const [status, setStatus] = useState<"closed" | "connected" | "connecting" | "error">(
    "closed",
  );
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const workspaceId = workspace?.id ?? "";
  const workspacePath = workspace?.path ?? "";

  useEffect(() => {
    tRef.current = t;
  }, [t]);

  useEffect(() => {
    if (!workspaceId) {
      return;
    }

    let cancelled = false;
    const terminal = new XTerm({
      allowProposedApi: false,
      convertEol: true,
      cursorBlink: true,
      fontFamily: "Cascadia Mono, Consolas, monospace",
      fontSize: 13,
      rows: 12,
      theme: {
        background: "#16130f",
        foreground: "#f7f3ea",
        cursor: "#14b8a6",
      },
    });
    const fitAddon = new FitAddon();
    let socket: WebSocket | null = null;

    xtermRef.current = terminal;
    fitAddonRef.current = fitAddon;
    terminal.loadAddon(fitAddon);
    setStatus("connecting");
    setError(null);

    if (!containerRef.current) {
      setStatus("error");
      setError(tRef.current("Terminal container was not mounted."));
      terminal.dispose();
      return;
    }

    terminal.open(containerRef.current);
    fitAddon.fit();

    const sendResize = () => {
      if (socket?.readyState !== WebSocket.OPEN) {
        return;
      }

      socket.send(
        JSON.stringify({
          type: "resize",
          cols: terminal.cols,
          rows: terminal.rows,
        }),
      );
    };

    const observer = new ResizeObserver(() => {
      fitAddon.fit();
      sendResize();
    });
    observer.observe(containerRef.current);
    resizeObserverRef.current = observer;

    const inputDisposable = terminal.onData((data) => {
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "input", data }));
      }
    });

    async function connectTerminal() {
      if (!workspaceId) {
        return;
      }

      try {
        const session = await requestJson<TerminalSessionResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/terminal/session`,
          { method: "POST" },
        );
        if (cancelled) {
          return;
        }

        setCwd(session.workingDirectory);
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        socket = new WebSocket(
          `${protocol}//${window.location.host}/api/workspaces/${encodeURIComponent(
            workspaceId,
          )}/terminal/${encodeURIComponent(session.id)}/ws?cols=${terminal.cols}&rows=${terminal.rows}`,
        );
        socketRef.current = socket;

        socket.onopen = () => {
          setStatus("connected");
          sendResize();
          terminal.focus();
        };
        socket.onmessage = (event) => {
          const parsed = JSON.parse(event.data as string) as unknown;
          if (!isTerminalServerEvent(parsed)) {
            setStatus("error");
            setError(tRef.current("Terminal returned an unknown event."));
            return;
          }

          if (parsed.type === "started" || parsed.type === "cwd") {
            setCwd(parsed.cwd);
            return;
          }

          if (parsed.type === "output") {
            terminal.write(parsed.data);
            return;
          }

          if (parsed.type === "exit") {
            setStatus("closed");
            terminal.writeln(
              `\r\n[${tRef.current("terminal exited: {status}", {
                status: parsed.status,
              })}]`,
            );
            return;
          }

          setStatus("error");
          setError(parsed.message);
          terminal.writeln(
            `\r\n[${tRef.current("terminal error: {message}", {
              message: parsed.message,
            })}]`,
          );
        };
        socket.onerror = () => {
          setStatus("error");
          setError(tRef.current("Terminal WebSocket failed."));
        };
        socket.onclose = () => {
          setStatus((current) => (current === "error" ? current : "closed"));
        };
      } catch (requestError) {
        if (!cancelled) {
          const message = errorMessage(requestError);
          setStatus("error");
          setError(message);
          terminal.writeln(
            `[${tRef.current("terminal error: {message}", { message })}]`,
          );
        }
      }
    }

    void connectTerminal();

    return () => {
      cancelled = true;
      inputDisposable.dispose();
      observer.disconnect();
      socket?.close();
      terminal.dispose();
      socketRef.current = null;
      xtermRef.current = null;
      fitAddonRef.current = null;
      resizeObserverRef.current = null;
    };
  }, [workspaceId]);

  return (
    <section className="shrink-0 border-t border-stone-800 bg-[#16130f]">
      <div className="mx-auto w-full max-w-5xl">
        <div className="flex h-8 items-center justify-between gap-3 px-3 text-xs text-stone-400">
          <span className="inline-flex min-w-0 items-center gap-2">
            <Terminal aria-hidden="true" className="size-4 shrink-0" />
            <span className={terminalStatusClass(status)}>
              {terminalStatusText(status, t)}
            </span>
            <span className="min-w-0 truncate">{cwd || workspacePath}</span>
          </span>
          {error ? (
            <span className="shrink-0 text-rose-300">{error}</span>
          ) : null}
        </div>
        <div ref={containerRef} className="h-56 min-w-0 p-2" />
      </div>
    </section>
  );
}

function ApiStatsPanel({
  activeWorkspace,
  availableModels,
  settings,
}: {
  activeWorkspace: WorkspaceSummary | undefined;
  availableModels: ConfiguredModelSummary[];
  settings: SettingsResponse | null;
}) {
  const { language, t } = useI18n();
  const enabledProviders =
    settings?.providers.filter((provider) => provider.enabled).length ?? 0;
  const configuredModels = settings?.configuredModels.length ?? 0;
  const chatCount = activeWorkspace?.chats.length ?? 0;

  return (
    <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="mx-auto flex max-w-6xl flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/80 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex min-w-0 items-center gap-3">
            <span className="inline-flex size-10 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
              <BarChart3 aria-hidden="true" className="size-5" />
            </span>
            <div className="min-w-0">
              <h2 className="truncate text-lg font-semibold text-stone-950">
                {t("API statistics")}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {activeWorkspace?.name ?? t("No workspace selected")}
              </p>
            </div>
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <StatsCard
            icon={Activity}
            label={t("Workspace chats")}
            value={formatNumber(chatCount, language)}
          />
          <StatsCard
            icon={PlugZap}
            label={t("Enabled providers")}
            value={formatNumber(enabledProviders, language)}
          />
          <StatsCard
            icon={Bot}
            label={t("Runnable models")}
            value={formatNumber(availableModels.length, language)}
          />
          <StatsCard
            icon={SlidersHorizontal}
            label={t("Configured models")}
            value={formatNumber(configuredModels, language)}
          />
        </section>

        <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="border-b border-stone-200 px-4 py-3">
            <h3 className="text-sm font-semibold text-stone-950">
              {t("Request audit")}
            </h3>
          </div>
          <div className="grid gap-3 px-4 py-8 text-sm text-stone-600 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
            <div className="min-w-0">
              <div className="font-semibold text-stone-900">
                {t("No API request table is exposed yet")}
              </div>
              <p className="mt-1 max-w-2xl leading-6">
                {t(
                  "The app records LLM request audit rows in the workspace database. This page now has a dedicated surface ready for the request-summary API when it is added.",
                )}
              </p>
            </div>
            <span className="inline-flex h-9 items-center rounded-lg border border-dashed border-stone-300 px-3 text-xs font-semibold text-stone-500">
              {t("audit data pending")}
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}

function StatsCard({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Settings;
  label: string;
  value: string;
}) {
  return (
    <article className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-semibold text-stone-600">{label}</span>
        <Icon aria-hidden="true" className="size-4 text-teal-700" />
      </div>
      <div className="mt-4 font-mono text-3xl font-semibold text-stone-950">
        {value}
      </div>
    </article>
  );
}

function GitDiffPanel({
  diffError,
  diffText,
  files,
  isLoading,
  onClose,
  onRefresh,
  onResizeBy,
  onResizeStart,
  onSelectFile,
  selectedPath,
}: {
  diffError: string | null;
  diffText: string;
  files: GitStatusFileSummary[];
  isLoading: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onResizeBy: (delta: number) => void;
  onResizeStart: () => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col">
      <div
        aria-label={t("Resize git diff panel")}
        aria-orientation="vertical"
        className="absolute bottom-0 left-0 top-0 hidden w-1 cursor-col-resize bg-transparent hover:bg-teal-500/40 lg:block"
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft") {
            event.preventDefault();
            onResizeBy(24);
          }

          if (event.key === "ArrowRight") {
            event.preventDefault();
            onResizeBy(-24);
          }
        }}
        onPointerDown={onResizeStart}
        role="separator"
        tabIndex={0}
      />
      <div className="flex items-center justify-between gap-3 border-b border-stone-200/80 px-4 py-4">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
            <GitCompare aria-hidden="true" className="size-5" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{t("Git diff")}</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {selectedPath ?? t("Workspace changes")}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            aria-label={t("Refresh diff")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
            disabled={isLoading}
            onClick={onRefresh}
            title={t("Refresh diff")}
            type="button"
          >
            {isLoading ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <RefreshCw aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label={t("Close git diff")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close git diff")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
      </div>

      {diffError ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {diffError}
        </div>
      ) : null}

      <div className="border-b border-stone-200/80 px-3 py-3">
        <button
          className={diffFileButtonClass(selectedPath === null)}
          onClick={() => onSelectFile(null)}
          type="button"
        >
          <span className="truncate">{t("All changed files")}</span>
          <span className="text-xs text-stone-500">{files.length}</span>
        </button>
        <div className="panel-scroll mt-2 max-h-56 space-y-1 overflow-y-auto">
          {files.length ? (
            files.map((file) => (
              <button
                className={diffFileButtonClass(selectedPath === file.path)}
                key={file.path}
                onClick={() => onSelectFile(file.path)}
                type="button"
              >
                <span className="min-w-0 flex-1 truncate text-left">
                  {file.path}
                </span>
                <span className="shrink-0 rounded-md bg-stone-100 px-1.5 py-0.5 font-mono text-[11px] text-stone-600">
                  {statusLabel(file)}
                </span>
              </button>
            ))
          ) : (
            <div className="rounded-lg border border-dashed border-stone-300 bg-stone-50/80 px-3 py-3 text-sm text-stone-500">
              {t("No changes")}
            </div>
          )}
        </div>
      </div>

      <div className="panel-scroll min-h-0 flex-1 overflow-auto bg-[#16130f]">
        <pre className="min-h-full whitespace-pre-wrap break-words px-4 py-4 font-mono text-[11px] leading-5 text-stone-100">
          {diffText || t("No diff")}
        </pre>
      </div>
    </div>
  );
}

function SettingsPanel({
  onSettingsChange,
}: {
  onSettingsChange: (settings: SettingsResponse) => void;
}) {
  const { t } = useI18n();
  const [activeSection, setActiveSection] =
    useState<SettingsSection>("general");
  const [isProviderDialogOpen, setIsProviderDialogOpen] = useState(false);
  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [isMcpDialogOpen, setIsMcpDialogOpen] = useState(false);
  const [metadata, setMetadata] = useState<ModelMetadataResponse | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedMetadataKey, setSelectedMetadataKey] = useState("");
  const [modelSearch, setModelSearch] = useState("");
  const [form, setForm] = useState<ModelFormState>(() => emptyModelForm());
  const [providerForm, setProviderForm] = useState<ProviderFormState>(() =>
    emptyProviderForm(),
  );
  const [generalForm, setGeneralForm] = useState<GeneralFormState>(() =>
    emptyGeneralForm(),
  );
  const [mcpForm, setMcpForm] = useState<McpServerFormState>(() =>
    emptyMcpServerForm(),
  );
  const [skillDirectoriesText, setSkillDirectoriesText] = useState("");
  const [enabledSkillIds, setEnabledSkillIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSavingGeneral, setIsSavingGeneral] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [isSavingMcpServer, setIsSavingMcpServer] = useState(false);
  const [isSavingModelOrder, setIsSavingModelOrder] = useState(false);
  const [isSavingSkills, setIsSavingSkills] = useState(false);
  const [isRefreshingSkills, setIsRefreshingSkills] = useState(false);
  const [draggedModelId, setDraggedModelId] = useState<string | null>(null);
  const [modelOrderPreview, setModelOrderPreview] = useState<string[] | null>(
    null,
  );
  const [providerTests, setProviderTests] = useState<
    Record<string, ProviderTestState>
  >({});
  const [error, setError] = useState<string | null>(null);

  const selectedMetadata = useMemo(
    () =>
      metadata?.models.find((model) => model.key === selectedMetadataKey) ??
      null,
    [metadata, selectedMetadataKey],
  );
  const filteredModels = useMemo(() => {
    const query = modelSearch.trim().toLowerCase();
    const models = metadata?.models ?? [];

    if (!query) {
      return models.slice(0, 80);
    }

    return models
      .filter((model) =>
        [
          model.providerName,
          model.providerId,
          model.name,
          model.modelId,
          model.key,
        ]
          .join(" ")
          .toLowerCase()
          .includes(query),
      )
      .slice(0, 80);
  }, [metadata, modelSearch]);
  const enabledNeedsLimits =
    form.enabled &&
    (!form.contextWindow.trim() || !form.maxOutputTokens.trim());
  const providerKinds = settings?.providerKinds ?? [];
  const providers = settings?.providers ?? [];
  const mcpTransports = settings?.mcpTransports ?? [];
  const mcpServers = settings?.mcpServers ?? [];
  const skills = settings?.skills;
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const configuredModels =
    settings?.configuredModels ?? metadata?.configuredModels ?? [];
  const orderedConfiguredModels = useMemo(() => {
    if (!modelOrderPreview) {
      return configuredModels;
    }

    const modelsById = new Map(
      configuredModels.map((model) => [model.id, model]),
    );
    const previewModels = modelOrderPreview
      .map((modelId) => modelsById.get(modelId))
      .filter((model): model is ConfiguredModelSummary => Boolean(model));

    return previewModels.length === configuredModels.length
      ? previewModels
      : configuredModels;
  }, [configuredModels, modelOrderPreview]);
  const editingModel =
    configuredModels.find((model) => model.id === form.modelId) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const selectedProviderIds = new Set(form.providerIds);

  function syncSkillsForm(data: SettingsResponse) {
    setSkillDirectoriesText(data.skills.directories.join("\n"));
    setEnabledSkillIds(
      new Set(
        data.skills.detected
          .filter((skill) => skill.enabled)
          .map((skill) => skill.id),
      ),
    );
  }

  function syncGeneralForm(data: SettingsResponse) {
    setGeneralForm({
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
    });
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
      setDraggedModelId(null);
      setModelOrderPreview(null);
      syncGeneralForm(data);
      setProviderForm((current) => ({
        ...current,
        kind: current.kind || data.providerKinds[0]?.kind || "openai-responses",
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

  useEffect(() => {
    void loadMetadata();
    void loadSettings();
  }, [loadMetadata, loadSettings]);

  function selectMetadataModel(key: string) {
    setSelectedMetadataKey(key);
    const model = metadata?.models.find((item) => item.key === key);

    if (!model) {
      return;
    }

    setForm({
      displayName: model.name,
      enabled: model.contextWindow !== null && model.maxOutputTokens !== null,
      modelId: model.modelId,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: [],
      activeProviderId: "",
      thinkingLevel: "",
    });
    setIsModelDialogOpen(true);
  }

  function modelMetadataForInput(modelId: string) {
    const normalizedModelId = modelId.trim();

    if (!normalizedModelId) {
      return null;
    }

    const models = metadata?.models ?? [];

    return (
      models.find((model) => model.key === normalizedModelId) ??
      models.find((model) => model.modelId === normalizedModelId) ??
      null
    );
  }

  function updateModelId(modelId: string) {
    const model = modelMetadataForInput(modelId);
    setSelectedMetadataKey(model?.key ?? "");

    setForm((current) => {
      if (!model) {
        return {
          ...current,
          modelId,
        };
      }

      return {
        ...current,
        displayName: model.name,
        enabled: model.contextWindow !== null && model.maxOutputTokens !== null,
        modelId: model.modelId,
        contextWindow: numberInputValue(model.contextWindow),
        maxOutputTokens: numberInputValue(model.maxOutputTokens),
      };
    });
  }

  function editConfiguredModel(model: ConfiguredModelSummary) {
    setSelectedMetadataKey(model.metadataKey ?? "");
    setForm({
      displayName: model.displayName,
      enabled: model.enabled,
      modelId: model.id,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: model.providerIds,
      activeProviderId: model.activeProviderId ?? "",
      thinkingLevel: model.thinkingLevel ?? "",
    });
    setIsModelDialogOpen(true);
  }

  function startAddingModel() {
    setSelectedMetadataKey("");
    setForm(emptyModelForm());
    setIsModelDialogOpen(true);
  }

  function startAddingProviderFromModel() {
    setIsModelDialogOpen(false);
    setActiveSection("providers");
    startAddingProvider();
  }

  function editConfiguredProvider(provider: ConfiguredProviderSummary) {
    setProviderForm({
      apiKey: "",
      baseUrl: provider.baseUrl ?? "",
      clearApiKey: false,
      enabled: provider.enabled,
      id: provider.id,
      kind: provider.kind,
      name: provider.name,
    });
    setIsProviderDialogOpen(true);
  }

  function startAddingProvider() {
    setProviderForm({
      ...emptyProviderForm(),
      kind: providerKinds[0]?.kind || "openai-responses",
    });
    setIsProviderDialogOpen(true);
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

  function startAddingMcpServer() {
    setMcpForm({
      ...emptyMcpServerForm(),
      transport: mcpTransports[0]?.transport || "stdio",
    });
    setIsMcpDialogOpen(true);
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
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          listenHost: generalForm.listenHost,
          listenPort: optionalPositiveInteger(
            generalForm.listenPort,
            t("Listen port"),
          ),
          language: generalForm.language,
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
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language,
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setSettings(data);
      onSettingsChange(data);
      setGeneralForm((current) => ({
        ...current,
        language: data.general.language,
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
      setGeneralForm((current) => ({
        ...current,
        language: settings.general.language,
      }));
    } finally {
      setIsSavingLanguage(false);
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
            contextWindow: optionalPositiveInteger(
              form.contextWindow,
              "Context window",
            ),
            maxOutputTokens: optionalPositiveInteger(
              form.maxOutputTokens,
              "Max output tokens",
            ),
            providerIds: form.providerIds,
            activeProviderId: form.activeProviderId,
            thinkingLevel: form.thinkingLevel || null,
            clearThinkingLevel: !form.thinkingLevel,
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

  async function saveProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingProvider(true);
    setError(null);

    try {
      const data = await requestJson<SettingsResponse>(
        "/api/providers/manual",
        {
          body: JSON.stringify({
            apiKey: providerForm.apiKey || null,
            baseUrl: providerForm.baseUrl || null,
            clearApiKey: providerForm.clearApiKey,
            enabled: providerForm.enabled,
            id:
              providerForm.id ||
              nextProviderId(providerForm.name, providerForm.kind, providers),
            kind: providerForm.kind,
            name: providerForm.name,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setSettings(data);
      onSettingsChange(data);
      setProviderForm((current) => ({
        ...current,
        apiKey: "",
        clearApiKey: false,
      }));
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
      setProviderForm({
        ...emptyProviderForm(),
        kind: data.providerKinds[0]?.kind || "openai-responses",
      });
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

  async function saveSkills() {
    setIsSavingSkills(true);
    setError(null);

    try {
      const disabledSkillIds = (skills?.detected ?? [])
        .filter((skill) => !enabledSkillIds.has(skill.id))
        .map((skill) => skill.id);
      const data = await requestJson<SettingsResponse>("/api/skills/manual", {
        body: JSON.stringify({
          directories: skillDirectoriesText
            .split(/\r?\n/)
            .map((directory) => directory.trim())
            .filter(Boolean),
          disabled: disabledSkillIds,
          enabled: Array.from(enabledSkillIds),
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

  function toggleModelProvider(providerId: string, checked: boolean) {
    setForm((current) => {
      const providerIds = checked
        ? [...current.providerIds, providerId].filter(uniqueString)
        : current.providerIds.filter((id) => id !== providerId);
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

  function toggleSkill(skillId: string, checked: boolean) {
    setEnabledSkillIds((current) => {
      const next = new Set(current);

      if (checked) {
        next.add(skillId);
      } else {
        next.delete(skillId);
      }

      return next;
    });
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

    const modelIds = moveModelId(
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
    <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="mx-auto grid max-w-7xl gap-4 lg:grid-cols-[13rem_minmax(0,1fr)]">
        <aside className="rounded-2xl border border-stone-200 bg-white/85 p-2 shadow-[0_18px_42px_rgba(75,63,42,0.07)] lg:self-start">
          <nav
            aria-label={t("Settings")}
            className="flex flex-col gap-1.5"
          >
          <SettingsNavButton
            active={activeSection === "general"}
            icon={Globe}
            label={t("General")}
            onClick={() => setActiveSection("general")}
          />
          <SettingsNavButton
            active={activeSection === "providers"}
            icon={PlugZap}
            label={t("Providers")}
            onClick={() => setActiveSection("providers")}
          />
          <SettingsNavButton
            active={activeSection === "models"}
            icon={SlidersHorizontal}
            label={t("Models")}
            onClick={() => setActiveSection("models")}
          />
          <SettingsNavButton
            active={activeSection === "mcp"}
            icon={Server}
            label={t("MCP")}
            onClick={() => setActiveSection("mcp")}
          />
          <SettingsNavButton
            active={activeSection === "skills"}
            icon={Wrench}
            label={t("Skills")}
            onClick={() => setActiveSection("skills")}
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
                label={t("Listen host")}
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
            <div className="mt-4 flex flex-wrap gap-2">
              <button
                aria-label={t("Save general settings")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingGeneral ||
                  !generalForm.listenHost.trim() ||
                  !generalForm.listenPort.trim()
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

        {activeSection === "providers" ? (
        <section className="grid gap-4">
          {isProviderDialogOpen ? (
          <>
          <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
          <form
            aria-label={t("Provider configuration")}
            className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
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
                    setProviderForm((current) => ({
                      ...current,
                      kind: event.target.value,
                    }))
                  }
                  value={providerForm.kind || providerKinds[0]?.kind || ""}
                >
                  {providerKinds.map((kind) => (
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
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 pr-11 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
                  type="password"
                  value={providerForm.apiKey}
                />
                {hasSavedProviderKey || providerForm.clearApiKey ? (
                  <button
                    aria-label={t("Clear saved API key")}
                    className={`absolute right-1 top-1 inline-flex size-8 items-center justify-center rounded-md ${
                      providerForm.clearApiKey
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
              <button
                aria-label={t("Save provider")}
                className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingProvider ||
                  !providerForm.name.trim() ||
                  !providerForm.kind.trim()
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
                  aria-label={t("Reload settings")}
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
              {providers.length ? (
                providers.map((provider) => {
                  const test = providerTests[provider.id];

                  return (
                    <div className="px-4 py-3" key={provider.id}>
                      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
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
                          </div>
                          <div className="mt-1 truncate text-xs font-medium text-stone-500">
                            {provider.id} / {provider.kindLabel}
                          </div>
                          {provider.baseUrl ? (
                            <div className="mt-1 truncate text-xs text-stone-500">
                              {provider.baseUrl}
                            </div>
                          ) : null}
                        </div>
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
                      {test ? (
                        <div
                          className={`mt-3 rounded-lg border px-3 py-2 text-sm ${
                            test.status === "ok"
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
          <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
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
          <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center gap-2">
              <Wrench aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Skill directories")}
              </h3>
            </div>
            <label className="mt-4 block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Directories")}
              </span>
              <textarea
                className="min-h-36 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) => setSkillDirectoriesText(event.target.value)}
                placeholder={
                  "C:/Users/name/.agents/skills\nC:/Users/name/.claude/skills\n.agents/skills\n.claude/skills"
                }
                value={skillDirectoriesText}
              />
            </label>
            <div className="mt-3 flex flex-wrap gap-2">
              <button
                aria-label={t("Save skills")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={isSavingSkills}
                onClick={() => void saveSkills()}
                title={t("Save skills")}
                type="button"
              >
                {isSavingSkills ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
                {t("Save")}
              </button>
              <button
                aria-label={t("Refresh skill discovery")}
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
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
                {t("Refresh")}
              </button>
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

          <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Detected skills")}
              </h3>
              <CapabilityPill
                label={t("skills {count}", {
                  count: skills?.detected.length ?? 0,
                })}
                ok={(skills?.detected.length ?? 0) > 0}
              />
            </div>
            <div className="divide-y divide-stone-100">
              {skills?.detected.length ? (
                skills.detected.map((skill) => {
                  const enabled = enabledSkillIds.has(skill.id);

                  return (
                    <div className="px-4 py-3" key={skill.id}>
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
                          </div>
                          <div className="mt-1 truncate text-xs font-medium text-stone-500">
                            {skill.id}
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
                            onChange={(event) =>
                              toggleSkill(skill.id, event.target.checked)
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
                  className={`grid gap-3 px-4 py-3 transition md:grid-cols-[auto_minmax(0,1fr)_auto] ${
                    draggedModelId === model.id
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
                  <div className="flex items-start pt-1">
                    <span
                      aria-label={t("Reorder model {name}", {
                        name: model.displayName,
                      })}
                      className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${
                        isSavingModelOrder
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
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-sm font-semibold">
                        {model.displayName}
                      </span>
                      <CapabilityPill
                        label={model.enabled ? t("enabled") : t("disabled")}
                        ok={model.enabled}
                      />
                      <CapabilityPill
                        label={
                          model.canEnable
                            ? t("limits ok")
                            : t("limits missing")
                        }
                        ok={model.canEnable}
                      />
                    </div>
                    <div className="mt-1 truncate text-xs font-medium text-stone-500">
                      {model.id}
                    </div>
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      <CapabilityPill
                        label={t("providers {count}", {
                          count: model.providerIds.length,
                        })}
                        ok={model.providerIds.length > 0}
                      />
                      <CapabilityPill
                        label={
                          model.activeProviderId
                            ? t("active {id}", { id: model.activeProviderId })
                            : t("active missing")
                        }
                        ok={model.activeProviderId !== null}
                      />
                    </div>
                  </div>
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
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={t("Model configuration")}
                className="panel-scroll fixed left-1/2 top-1/2 z-50 max-h-[88dvh] w-[min(92vw,38rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
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
                  <div className="flex shrink-0 gap-2">
                    {editingModel ? (
                      <button
                        aria-label={t("Delete model")}
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={isSaving}
                        onClick={() => void deleteModel(editingModel.id)}
                        title={t("Delete model")}
                        type="button"
                      >
                        <Trash2 aria-hidden="true" className="size-4" />
                      </button>
                    ) : null}
                    <button
                      aria-label={t("Close model configuration")}
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                      onClick={() => setIsModelDialogOpen(false)}
                      title={t("Close")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-4" />
                    </button>
                  </div>
                </div>
                <div className="space-y-3">
                  <TextField
                    label={t("Model id")}
                    onChange={updateModelId}
                    placeholder="gpt-5.5"
                    value={form.modelId}
                  />
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
                  <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
                    <span className="text-sm font-semibold text-stone-700">
                      {t("Enable model")}
                    </span>
                    <input
                      checked={form.enabled}
                      className="size-4 accent-teal-700"
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          enabled: event.target.checked,
                        }))
                      }
                      type="checkbox"
                    />
                  </label>
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
                    <div className="space-y-2">
                      {providers.length ? (
                        providers.map((provider) => (
                          <label
                            className="flex items-center justify-between gap-3 rounded-lg bg-stone-50/80 px-3 py-2"
                            key={provider.id}
                          >
                            <span className="min-w-0">
                              <span className="block truncate text-sm font-semibold text-stone-700">
                                {provider.name}
                              </span>
                              <span className="block truncate text-xs text-stone-500">
                                {provider.kindLabel}
                              </span>
                            </span>
                            <input
                              checked={selectedProviderIds.has(provider.id)}
                              className="size-4 accent-teal-700"
                              onChange={(event) =>
                                toggleModelProvider(
                                  provider.id,
                                  event.target.checked,
                                )
                              }
                              type="checkbox"
                            />
                          </label>
                        ))
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
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      disabled={!form.providerIds.length}
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          activeProviderId: event.target.value,
                        }))
                      }
                      value={form.activeProviderId}
                    >
                      <option value="">{t("None")}</option>
                      {form.providerIds.map((providerId) => {
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
                      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          thinkingLevel: event.target.value,
                        }))
                      }
                      value={form.thinkingLevel}
                    >
                      <option value="">{t("None")}</option>
                      {thinkingLevels.map((level) => (
                        <option key={level.value} value={level.value}>
                          {t(level.label)}
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

                {selectedMetadata ? (
                  <div className="mt-4 border-t border-stone-200 pt-4 text-xs text-stone-500">
                    <div className="truncate">{selectedMetadata.key}</div>
                    <div className="mt-1">
                      {t("pricing in/out:")}{" "}
                      {priceText(selectedMetadata.pricing.input)} /{" "}
                      {priceText(selectedMetadata.pricing.output)}
                    </div>
                  </div>
                ) : null}
              </form>
            </>
          ) : null}

          <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
            <div className="border-b border-stone-200 px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <input
                  className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => setModelSearch(event.target.value)}
                  placeholder={t("Search model metadata")}
                  value={modelSearch}
                />
                <button
                  aria-label={t("Reload model metadata cache")}
                  className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoading}
                  onClick={() => void loadMetadata()}
                  title={t("Reload cache")}
                  type="button"
                >
                  {isLoading ? (
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
            <div className="panel-scroll max-h-80 overflow-y-auto">
              {filteredModels.length > 0 ? (
                filteredModels.map((model) => (
                  <button
                    className={`grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-stone-100 px-4 py-3 text-left hover:bg-stone-50 ${
                      selectedMetadataKey === model.key ? "bg-teal-50" : "bg-white/70"
                    }`}
                    key={model.key}
                    onClick={() => selectMetadataModel(model.key)}
                    type="button"
                  >
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-semibold text-stone-950">
                        {model.name}
                      </span>
                      <span className="mt-1 block truncate text-xs font-medium text-stone-500">
                        {model.providerName} / {model.modelId}
                      </span>
                    </span>
                    <span className="text-right text-xs font-medium text-stone-500">
                      {model.inputModalities.join(", ") || t("input n/a")}
                    </span>
                  </button>
                ))
              ) : (
                <div className="px-4 py-8 text-sm text-stone-500">
                  {isLoading ? t("Loading models...") : t("No cached models")}
                </div>
              )}
            </div>
          </section>
        </section>
        ) : null}
        </div>
      </div>
    </div>
  );
}

function NavButton({
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
      className={`inline-flex size-9 items-center justify-center rounded-lg ${
        active
          ? "bg-white text-teal-900 shadow-sm"
          : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
      }`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function workspaceModeClass(active: boolean) {
  return `inline-flex h-9 items-center justify-center gap-2 rounded-lg border px-2 text-sm font-semibold ${
    active
      ? "border-teal-200 bg-teal-50 text-teal-900 shadow-sm"
      : "border-stone-200 bg-white/80 text-stone-600 hover:border-stone-300 hover:bg-white hover:text-stone-950"
  }`;
}

function workspaceActionClass() {
  return "inline-flex h-10 items-center justify-center rounded-lg border border-stone-200 bg-white/85 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800";
}

function workspaceItemClass(active: boolean) {
  return `flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg px-2 text-sm font-semibold ${
    active
      ? "bg-stone-950 text-white shadow-[0_10px_24px_rgba(33,31,28,0.16)]"
      : "text-stone-700 hover:bg-white/80 hover:text-stone-950"
  }`;
}

function diffFileButtonClass(active: boolean) {
  return `flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-sm ${
    active
      ? "bg-teal-50 text-teal-950 shadow-sm"
      : "text-stone-700 hover:bg-stone-50 hover:text-stone-950"
  }`;
}

function settingsSectionTitle(section: SettingsSection, t: Translate) {
  if (section === "general") {
    return t("General settings");
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
      className={`inline-flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left text-sm font-semibold ${
        active
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
  value,
}: {
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
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
        value={value}
      />
    </label>
  );
}

function CapabilityPill({ label, ok }: { label: string; ok: boolean }) {
  return (
    <span
      className={`inline-flex min-h-6 items-center rounded-md border px-2 py-0.5 text-xs font-semibold ${
        ok
          ? "border-teal-200 bg-teal-50 text-teal-800"
          : "border-stone-200 bg-stone-50 text-stone-500"
      }`}
    >
      {label}
    </span>
  );
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
    thinkingLevel: "",
  };
}

function emptyProviderForm(): ProviderFormState {
  return {
    apiKey: "",
    baseUrl: "",
    clearApiKey: false,
    enabled: true,
    id: "",
    kind: "",
    name: "",
  };
}

function emptyGeneralForm(): GeneralFormState {
  return {
    language: "en",
    listenHost: "127.0.0.1",
    listenPort: "3210",
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

function moveModelId(
  modelIds: string[],
  sourceModelId: string,
  targetModelId: string,
) {
  const sourceIndex = modelIds.indexOf(sourceModelId);
  const targetIndex = modelIds.indexOf(targetModelId);

  if (sourceIndex === -1 || targetIndex === -1 || sourceIndex === targetIndex) {
    return modelIds;
  }

  const next = [...modelIds];
  const [source] = next.splice(sourceIndex, 1);
  next.splice(targetIndex, 0, source);

  return next;
}

function sameStringList(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function slugId(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function activeSkillQuery(value: string) {
  const match = /(^|\s)\/([^\s/]*)$/.exec(value);
  return match ? match[2] : null;
}

function removeActiveSkillToken(value: string) {
  return value.replace(/(^|\s)\/[^\s/]*$/, (_match, prefix: string) => prefix);
}

function selectedSkillPrefix(content: string, isUser: boolean) {
  if (!isUser) {
    return null;
  }

  let remaining = content.trimStart();
  const skills: Array<{ name: string; path: string }> = [];

  while (true) {
    const match = /^\[\$([^\]\n]+)\]\(([^)\n]+)\)(?:\s+|$)/.exec(remaining);
    if (!match) {
      break;
    }

    const path = decodeMarkdownHref(match[2].trim());
    if (!path.replaceAll("\\", "/").endsWith("SKILL.md")) {
      break;
    }

    skills.push({
      name: match[1].trim(),
      path,
    });
    remaining = remaining.slice(match[0].length);
  }

  if (!skills.length) {
    return null;
  }

  return {
    remaining,
    skills,
  };
}

function decodeMarkdownHref(value: string) {
  try {
    return decodeURI(value);
  } catch {
    return value;
  }
}

function messageWithSelectedSkills(
  skills: ConfiguredSkillSummary[],
  skillIds: string[],
  message: string,
) {
  const links = skillIds
    .map((skillId) => skills.find((skill) => skill.id === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill))
    .map((skill) => `[$${skill.name}](${skill.path})`);

  return links.length ? `${links.join(" ")} ${message}` : message;
}

function uniqueString(value: string, index: number, values: string[]) {
  return values.indexOf(value) === index;
}

function numberInputValue(value: number | null) {
  return value === null ? "" : String(value);
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

function upsertToolCall(
  toolCalls: ChatToolCallSummary[],
  nextToolCall: ChatToolCallSummary,
) {
  const normalizedToolCall = normalizedToolCallSummary(nextToolCall);
  const existingIndex = toolCalls.findIndex(
    (toolCall) => toolCall.id === normalizedToolCall.id,
  );

  if (existingIndex === -1) {
    return [...toolCalls, normalizedToolCall];
  }

  return toolCalls.map((toolCall, index) =>
    index === existingIndex ? normalizedToolCall : toolCall,
  );
}

function applyToolResult(
  toolCalls: ChatToolCallSummary[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
) {
  return toolCalls.map((toolCall) =>
    toolCall.id === toolCallId
      ? {
          ...toolCall,
          output,
          isError,
          status: isError ? "error" : "completed",
        }
    : toolCall,
  );
}

function completedAssistantMessage(
  message: ShellMessage,
  streamEvent: Extract<ChatStreamEvent, { type: "complete" }>,
): ShellMessage {
  let parts = message.parts;
  const nextReasoning = streamEvent.reasoning ?? null;
  const reasoningDelta = missingFinalSuffix(message.reasoning ?? "", nextReasoning ?? "");
  if (reasoningDelta) {
    parts = appendReasoningPart(parts, reasoningDelta);
  }
  const textDelta = missingFinalSuffix(message.content, streamEvent.text);
  if (textDelta) {
    parts = appendTextPart(parts, textDelta);
  }

  return {
    ...message,
    content: streamEvent.text,
    reasoning: nextReasoning,
    status: undefined,
    parts: parts.length
      ? parts
      : fallbackMessageParts({
          ...message,
          content: streamEvent.text,
          reasoning: nextReasoning,
          status: undefined,
        }),
  };
}

function missingFinalSuffix(current: string, next: string) {
  if (!next || current === next) {
    return "";
  }

  return next.startsWith(current) ? next.slice(current.length) : "";
}

function appendTextPart(parts: ChatMessagePart[], text: string): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "text") {
    return [...parts, { type: "text", text }];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function appendReasoningPart(
  parts: ChatMessagePart[],
  text: string,
): ChatMessagePart[] {
  if (!text) {
    return parts;
  }

  const lastPart = parts[parts.length - 1];
  if (lastPart?.type !== "reasoning") {
    return [...parts, { type: "reasoning", text }];
  }

  return [
    ...parts.slice(0, -1),
    {
      ...lastPart,
      text: lastPart.text + text,
    },
  ];
}

function upsertToolCallPart(
  parts: ChatMessagePart[],
  nextToolCall: ChatToolCallSummary,
): ChatMessagePart[] {
  const normalizedToolCall = normalizedToolCallSummary(nextToolCall);
  const nextPart: ChatMessagePart = {
    type: "toolCall",
    toolCall: normalizedToolCall,
  };
  const existingIndex = parts.findIndex(
    (part) =>
      part.type === "toolCall" && part.toolCall.id === normalizedToolCall.id,
  );

  if (existingIndex === -1) {
    return [...parts, nextPart];
  }

  return parts.map((part, index) =>
    index === existingIndex ? nextPart : part,
  );
}

function applyToolResultToParts(
  parts: ChatMessagePart[],
  toolCallId: string,
  output: JsonValue,
  isError: boolean,
): ChatMessagePart[] {
  return parts.map((part) =>
    part.type === "toolCall" && part.toolCall.id === toolCallId
      ? ({
          type: "toolCall",
          toolCall: {
            ...part.toolCall,
            output,
            isError,
            status: isError ? "error" : "completed",
          },
        } satisfies ChatMessagePart)
      : part,
  );
}

function fallbackMessageParts(
  message: ShellMessage | ChatMessageSummary,
): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [];
  if (message.reasoning) {
    parts.push({ type: "reasoning", text: message.reasoning });
  }
  if (message.content) {
    parts.push({ type: "text", text: message.content });
  }
  parts.push(
    ...message.toolCalls.map((toolCall) => ({
      type: "toolCall" as const,
      toolCall: normalizedToolCallSummary(toolCall),
    })),
  );
  return parts;
}

function normalizedToolCallSummary(
  toolCall: ChatToolCallSummary,
): ChatToolCallSummary {
  return {
    ...toolCall,
    input: normalizedToolInput(toolCall.input),
    output:
      toolCall.output === null ? null : normalizedJsonValue(toolCall.output),
  };
}

function toolStatusText(toolCall: ChatToolCallSummary, t: Translate) {
  if (toolCall.isError) {
    return t("error");
  }

  return toolCall.status;
}

function toolCallDetailText(toolCall: ChatToolCallSummary) {
  const input = normalizedToolInput(toolCall.input);

  if (!isObjectRecord(input)) {
    return compactToolJson(input);
  }

  if (toolCall.name === "run_command") {
    const command = textField(input, "command");
    const args = stringArrayField(input, "args") ?? [];
    const cwd = textField(input, "cwd");

    if (command) {
      const fullCommand = [command, ...args].map(formatCommandPart).join(" ");
      return compactToolText(cwd && cwd !== "." ? `${fullCommand} | cwd: ${cwd}` : fullCommand);
    }
  }

  const parts = [
    textField(input, "path"),
    textField(input, "query"),
    textField(input, "symbol"),
    numberTextField(input, "symbolId", "symbol_id"),
    numberTextField(input, "durationMs", "duration_ms"),
  ].filter(Boolean);
  const pathIndex = parts.findIndex((part) => part === textField(input, "path"));
  const startLine = numberTextField(input, "startLine", "start_line");
  const endLine = numberTextField(input, "endLine", "end_line");

  if (pathIndex !== -1 && startLine && endLine) {
    parts[pathIndex] = `${parts[pathIndex]}:${startLine}-${endLine}`;
  }

  return parts.length ? compactToolText(parts.join(" | ")) : compactToolJson(input);
}

function normalizedToolInput(value: JsonValue): JsonValue {
  const normalized = normalizedJsonValue(value);
  if (!isObjectRecord(normalized)) {
    return normalized;
  }

  for (const fieldName of ["arguments", "args", "input"]) {
    const nested = normalized[fieldName];
    if (!isJsonValue(nested)) {
      continue;
    }

    const normalizedNested = normalizedJsonValue(nested);
    if (isObjectRecord(normalizedNested)) {
      return normalizedNested;
    }
  }

  return normalized;
}

function textField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "string" ? field : null;
}

function numberTextField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "number" ? String(field) : null;
}

function stringArrayField(value: Record<string, unknown>, camelName: string, snakeName?: string) {
  const field = fieldValue(value, camelName, snakeName);

  if (field === null || typeof field === "undefined") {
    return null;
  }

  return Array.isArray(field) && field.every((item) => typeof item === "string")
    ? field
    : null;
}

function formatCommandPart(value: string) {
  if (value === "") {
    return '""';
  }

  return /^[A-Za-z0-9_./:=@%+,\-\\]+$/.test(value) ? value : JSON.stringify(value);
}

function compactToolJson(value: JsonValue) {
  return compactToolText(JSON.stringify(value));
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

function formatLimit(value: number | null, label: string, language: AppLanguageId = "en") {
  return value === null
    ? `${label} missing`
    : `${label} ${formatNumber(value, language)}`;
}

function formatNumber(value: number, language: AppLanguageId = "en") {
  return new Intl.NumberFormat(language).format(value);
}

function formatChatCreatedAt(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(date);
}

function chatRunKey(workspaceId: string, chatId: string) {
  return `${workspaceId}:${chatId}`;
}

function priceText(value: number | null) {
  return value === null ? "n/a" : `$${value}`;
}

function formatDiffText(diff: GitDiffResponse | null) {
  if (!diff) {
    return "";
  }

  const parts: string[] = [];

  if (diff.stagedDiff.trim()) {
    parts.push(`# staged\n${diff.stagedDiff.trimEnd()}`);
  }

  if (diff.diff.trim()) {
    parts.push(`# unstaged\n${diff.diff.trimEnd()}`);
  }

  if (!parts.length && diff.status.trim()) {
    parts.push(diff.status.trimEnd());
  }

  return parts.join("\n\n");
}

function statusLabel(file: GitStatusFileSummary) {
  return `${file.indexStatus}${file.worktreeStatus}`.replaceAll(" ", ".");
}

function terminalStatusText(
  status: "closed" | "connected" | "connecting" | "error",
  t: Translate,
) {
  if (status === "connected") {
    return t("connected");
  }

  if (status === "connecting") {
    return t("connecting");
  }

  if (status === "error") {
    return t("error");
  }

  return t("closed");
}

function terminalStatusClass(status: "closed" | "connected" | "connecting" | "error") {
  const base = "rounded-md px-1.5 py-0.5 text-[11px] font-semibold";

  if (status === "connected") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "connecting") {
    return `${base} bg-stone-200 text-stone-700`;
  }

  if (status === "error") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-500`;
}

async function readChatStream(
  response: Response,
  onEvent: (event: ChatStreamEvent) => void,
) {
  if (!response.body) {
    throw new Error("chat stream response has no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    buffer += decoder.decode(value, { stream: true });
    buffer = readSseFrames(buffer, onEvent);
  }

  buffer += decoder.decode();
  readSseFrames(`${buffer}\n\n`, onEvent);
}

function readSseFrames(
  buffer: string,
  onEvent: (event: ChatStreamEvent) => void,
) {
  const normalized = buffer.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const frames = normalized.split("\n\n");
  const remaining = frames.pop() ?? "";

  for (const frame of frames) {
    const data = frame
      .split("\n")
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).trimStart())
      .join("\n");

    if (!data) {
      continue;
    }

    const parsed = JSON.parse(data) as unknown;
    const event = parseChatStreamEvent(parsed);
    if (!event) {
      throw new Error(
        `chat stream returned an unknown event: ${describeChatStreamEvent(parsed)}`,
      );
    }

    onEvent(event);
  }

  return remaining;
}

function parseChatStreamEvent(value: unknown): ChatStreamEvent | null {
  if (!isObjectRecord(value) || typeof value.type !== "string") {
    return null;
  }

  if (value.type === "start") {
    const chatId = stringField(value, "chatId", "chat_id");
    const userMessageId = stringField(value, "userMessageId", "user_message_id");
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const llmRequestId = optionalStringField(
      value,
      "llmRequestId",
      "llm_request_id",
    );

    if (!chatId || !userMessageId || !assistantMessageId || llmRequestId === null) {
      return null;
    }

    return {
      type: "start",
      chatId,
      userMessageId,
      assistantMessageId,
      llmRequestId,
    };
  }

  if (value.type === "textDelta" || value.type === "text_delta") {
    const assistantMessageId = optionalStringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const delta = stringField(value, "delta");

    if (assistantMessageId === null || delta === null) {
      return null;
    }

    return { type: "textDelta", assistantMessageId, delta };
  }

  if (value.type === "reasoningDelta" || value.type === "reasoning_delta") {
    const assistantMessageId = optionalStringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const delta = stringField(value, "delta");

    if (assistantMessageId === null || delta === null) {
      return null;
    }

    return { type: "reasoningDelta", assistantMessageId, delta };
  }

  if (value.type === "usage") {
    const usage = parseChatUsage(value.usage);

    if (usage === false) {
      return null;
    }

    return { type: "usage", usage };
  }

  if (value.type === "complete") {
    const chatId = stringField(value, "chatId", "chat_id");
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const text = stringField(value, "text");
    const reasoning = optionalNullableStringField(value, "reasoning");
    const usage = parseNullableChatUsage(fieldValue(value, "usage"));
    const stopReason = optionalNullableStringField(
      value,
      "stopReason",
      "stop_reason",
    );

    if (!chatId || !assistantMessageId || text === null) {
      return null;
    }

    if (reasoning === false || usage === false || stopReason === false) {
      return null;
    }

    return {
      type: "complete",
      chatId,
      assistantMessageId,
      text,
      reasoning,
      usage,
      stopReason,
    };
  }

  if (value.type === "toolCall" || value.type === "tool_call") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const toolCall = parseChatToolCallSummary(
      fieldValue(value, "toolCall", "tool_call"),
    );

    if (!assistantMessageId || !toolCall) {
      return null;
    }

    return { type: "toolCall", assistantMessageId, toolCall };
  }

  if (value.type === "toolResult" || value.type === "tool_result") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const toolCallId = stringField(value, "toolCallId", "tool_call_id");
    const output = fieldValue(value, "output");
    const isError = fieldValue(value, "isError", "is_error");

    if (
      !assistantMessageId ||
      !toolCallId ||
      !isJsonValue(output) ||
      typeof isError !== "boolean"
    ) {
      return null;
    }

    return { type: "toolResult", assistantMessageId, toolCallId, output, isError };
  }

  if (value.type === "gitDiffRefresh" || value.type === "git_diff_refresh") {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");

    if (!workspaceId) {
      return null;
    }

    return { type: "gitDiffRefresh", workspaceId };
  }

  if (value.type === "error") {
    const message = stringField(value, "message");

    if (!message) {
      return null;
    }

    return { type: "error", message };
  }

  return null;
}

function describeChatStreamEvent(value: unknown) {
  const summary = isObjectRecord(value) ? { type: value.type, value } : value;

  try {
    return JSON.stringify(summary).slice(0, 600);
  } catch {
    return String(value);
  }
}

function parseChatToolCallSummary(value: unknown): ChatToolCallSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const name = stringField(value, "name");
  const status = stringField(value, "status");
  const input = fieldValue(value, "input");
  const output = fieldValue(value, "output");
  const isError = fieldValue(value, "isError", "is_error");

  if (
    !id ||
    !name ||
    !status ||
    !isJsonValue(input) ||
    !isJsonValue(output) ||
    typeof isError !== "boolean"
  ) {
    return null;
  }

  return normalizedToolCallSummary({ id, name, status, input, output, isError });
}

function normalizeChatMessageSummary(
  message: ChatMessageSummary,
): ChatMessageSummary {
  const toolCalls = Array.isArray(message.toolCalls)
    ? message.toolCalls.map(normalizedToolCallSummary)
    : [];
  const partsSource = Array.isArray(message.parts) ? message.parts : [];
  const parts = partsSource
    .map((part) => normalizeChatMessagePart(part))
    .filter((part): part is ChatMessagePart => part !== null);
  const normalizedMessage = {
    ...message,
    toolCalls,
    parts,
  };

  return {
    ...normalizedMessage,
    parts: parts.length ? parts : fallbackMessageParts(normalizedMessage),
  };
}

function normalizeChatMessagePart(part: unknown): ChatMessagePart | null {
  if (!isObjectRecord(part)) {
    return null;
  }

  if (part.type === "text") {
    const text = fieldValue(part, "text");
    return typeof text === "string" ? { type: "text", text } : null;
  }

  if (part.type === "reasoning") {
    const text = fieldValue(part, "text");
    return typeof text === "string" ? { type: "reasoning", text } : null;
  }

  if (part.type === "toolCall" || part.type === "tool_call") {
    const toolCall = parseChatToolCallSummary(
      fieldValue(part, "toolCall", "tool_call"),
    );
    return toolCall ? { type: "toolCall", toolCall } : null;
  }

  return null;
}

function parseNullableChatUsage(value: unknown): ChatUsage | null | undefined | false {
  if (value === null) {
    return null;
  }

  return parseChatUsage(value);
}

function parseChatUsage(value: unknown): ChatUsage | undefined | false {
  if (typeof value === "undefined") {
    return undefined;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  const inputTokens = fieldValue(value, "inputTokens", "input_tokens");
  const outputTokens = fieldValue(value, "outputTokens", "output_tokens");
  const cacheReadTokens = fieldValue(value, "cacheReadTokens", "cache_read_tokens");
  const cacheWriteTokens = fieldValue(
    value,
    "cacheWriteTokens",
    "cache_write_tokens",
  );

  if (
    !isNullableNumber(inputTokens) ||
    !isNullableNumber(outputTokens) ||
    !isNullableNumber(cacheReadTokens) ||
    !isNullableNumber(cacheWriteTokens)
  ) {
    return false;
  }

  return { inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens };
}

function isNullableNumber(value: unknown) {
  return typeof value === "number" || value === null;
}

function stringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "string" ? field : null;
}

function optionalStringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);
  return typeof field === "undefined" || typeof field === "string" ? field : null;
}

function optionalNullableStringField(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  const field = fieldValue(value, camelName, snakeName);

  if (
    typeof field === "undefined" ||
    field === null ||
    typeof field === "string"
  ) {
    return field;
  }

  return false;
}

function fieldValue(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  if (typeof value[camelName] !== "undefined") {
    return value[camelName];
  }

  if (snakeName && typeof value[snakeName] !== "undefined") {
    return value[snakeName];
  }

  return undefined;
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

function isTerminalServerEvent(value: unknown): value is TerminalServerEvent {
  if (
    typeof value !== "object" ||
    value === null ||
    !("type" in value) ||
    typeof value.type !== "string"
  ) {
    return false;
  }

  if (value.type === "started" || value.type === "cwd") {
    return "cwd" in value && typeof value.cwd === "string";
  }

  if (value.type === "output") {
    return "data" in value && typeof value.data === "string";
  }

  if (value.type === "exit") {
    return "status" in value && typeof value.status === "string";
  }

  if (value.type === "error") {
    return "message" in value && typeof value.message === "string";
  }

  return false;
}

async function responseErrorMessage(response: Response) {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    const data = (await response.json()) as unknown;

    if (isErrorResponse(data)) {
      return data.error;
    }
  }

  const text = await response.text();
  return text || `request returned ${response.status}`;
}

async function requestJson<T>(
  url: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(url, { cache: "no-store", ...init });
  const contentType = response.headers.get("content-type") ?? "";
  const data = contentType.includes("application/json")
    ? ((await response.json()) as unknown)
    : null;

  if (!response.ok) {
    if (isErrorResponse(data)) {
      throw new Error(data.error);
    }

    throw new Error(`${url} returned ${response.status}`);
  }

  return data as T;
}

function isErrorResponse(value: unknown): value is { error: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "error" in value &&
    typeof value.error === "string"
  );
}

function errorMessage(value: unknown) {
  return value instanceof Error ? value.message : "Unknown error";
}
