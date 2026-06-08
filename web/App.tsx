import {
  Activity,
  BarChart3,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Copy,
  Eye,
  EyeOff,
  Folder,
  FolderPlus,
  FolderSearch,
  GitBranch,
  GitCompare,
  Globe,
  GripVertical,
  KeyRound,
  ListChecks,
  Lock,
  LoaderCircle,
  MessageSquare,
  Pencil,
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
  Children,
  CSSProperties,
  DragEvent as ReactDragEvent,
  FormEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  createContext,
  isValidElement,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
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
  pinned: boolean;
  terminalShell: string;
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

type ContextUsageResponse = {
  usedMessageTokens: number;
  availableMessageTokens: number;
  usagePercent: number;
  compressionTriggerTokens: number;
  compressionTriggerPercent: number;
  willCompressOnNextSend: boolean;
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
  passwordEnabled: boolean;
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

type ConfiguredWorkspaceSummary = {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  terminalShell: string;
  isDefault: boolean;
};

type TerminalShellSummary = {
  shell: string;
  label: string;
};

type SettingsResponse = {
  general: GeneralSettingsSummary;
  workspaces: ConfiguredWorkspaceSummary[];
  terminalShells: TerminalShellSummary[];
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
  password: string;
};

type AuthStatusResponse = {
  authenticated: boolean;
  enabled: boolean;
};

type WorkspaceFormState = {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  terminalShell: string;
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
  key: string;
  id: string;
  name: string;
  description: string;
  path: string;
  scope: string;
  workspaceId: string | null;
  workspaceName: string | null;
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

type AiRequestAuditSummary = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  chatId: string | null;
  chatTitle: string | null;
  providerId: string;
  modelId: string;
  requestStartedAt: string;
  firstTokenAt: string | null;
  completedAt: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  cacheRatio: number | null;
  firstTokenLatencyMs: number | null;
  totalLatencyMs: number | null;
  statusCode: number | null;
  finalState: string;
};

type AiRequestAuditDetail = AiRequestAuditSummary & {
  requestBody: JsonValue | null;
  responseBody: JsonValue | null;
};

type AiStatisticsResponse = {
  page: number;
  pageSize: number;
  requests: AiRequestAuditSummary[];
  totalCount: number;
  totalPages: number;
};

type AiRequestDetailResponse = {
  request: AiRequestAuditDetail;
};

type AiStatsFilterState = {
  workspaceId: string;
  chatId: string;
  providerId: string;
  modelId: string;
  status: string;
  startedAfter: string;
  startedBefore: string;
  page: string;
  pageSize: string;
};

type AiStatsColumn = {
  cellClassName: string;
  headerClassName?: string;
  id: AiStatsColumnId;
  label: string;
  render: (request: AiRequestAuditSummary) => ReactNode;
};

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
  metrics: ChatReplyMetrics | null;
};

type ChatMessagesResponse = {
  messages: ChatMessageSummary[];
};

type TaskStatus =
  | "pending"
  | "ready"
  | "running"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled";

type TaskGraphTask = {
  id: string;
  title: string;
  status: TaskStatus;
  dependsOn: string[];
  acceptance: string[];
  summary: string | null;
  createdAt: string;
  updatedAt: string;
  subtasks: TaskGraphTask[];
};

type TaskGraphResponse = {
  chatId: string;
  exists: boolean;
  tasks: TaskGraphTask[];
  createdAt: string | null;
  updatedAt: string | null;
};

type ChatUsage = {
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
};

type ChatReplyMetrics = {
  modelId: string;
  providerId: string;
  totalLatencyMs: number | null;
  firstTokenLatencyMs: number | null;
  outputTokens: number | null;
};

type QuestionOptionSummary = {
  label: string;
  value: string;
  description: string | null;
};

type QuestionItemSummary = {
  id: string;
  question: string;
  options: QuestionOptionSummary[];
  allowFreeText: boolean;
};

type QuestionRequestSummary = {
  id: string;
  toolCallId: string;
  workspaceId: string;
  chatId: string;
  questions: QuestionItemSummary[];
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
      metrics: ChatReplyMetrics;
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
      type: "questionRequest";
      assistantMessageId: string;
      request: QuestionRequestSummary;
    }
  | {
      type: "gitDiffRefresh";
      workspaceId: string;
    }
  | {
      type: "taskGraphRefresh";
      workspaceId: string;
      chatId: string;
    }
  | { type: "error"; message: string };

type SettingsSection =
  | "general"
  | "mcp"
  | "models"
  | "providers"
  | "skills"
  | "workspaces";
type ViewMode = "chat" | "settings" | "stats";

const CREATE_BRANCH_OPTION_VALUE = "__create_branch__";
const CHAT_BOTTOM_LOCK_THRESHOLD_PX = 24;
const SAVED_PASSWORD_MASK = "********";
const AI_STATS_COLUMN_IDS = [
  "requestTime",
  "workspace",
  "chat",
  "provider",
  "model",
  "inputTokens",
  "outputTokens",
  "cacheRead",
  "cacheWrite",
  "cacheRatio",
  "latency",
  "firstToken",
  "statusCode",
  "status",
  "details",
] as const;
const DEFAULT_AI_STATS_COLUMN_IDS: AiStatsColumnId[] = [...AI_STATS_COLUMN_IDS];

type Translate = (key: string, values?: Record<string, string | number>) => string;
type AiStatsColumnId = (typeof AI_STATS_COLUMN_IDS)[number];

type MermaidRuntime = {
  initialize: (config: Record<string, unknown>) => void;
  render: (
    id: string,
    definition: string,
  ) => Promise<{
    bindFunctions?: (element: Element) => void;
    svg: string;
  }>;
};

const MERMAID_CONFIG: Record<string, unknown> = {
  flowchart: {
    curve: "basis",
  },
  htmlLabels: false,
  securityLevel: "strict",
  startOnLoad: false,
  theme: "base",
  themeVariables: {
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
    lineColor: "#78716c",
    primaryBorderColor: "#0f766e",
    primaryColor: "#f5f5f4",
    primaryTextColor: "#1c1917",
    secondaryBorderColor: "#a8a29e",
    secondaryColor: "#fafaf9",
    tertiaryColor: "#ccfbf1",
  },
};
let mermaidRuntimePromise: Promise<MermaidRuntime> | null = null;

const MARKDOWN_COMPONENTS: Components = {
  pre({ children, node: _node, ...props }) {
    const mermaidDefinition = mermaidDefinitionFromPreChildren(children);
    if (mermaidDefinition !== null) {
      return <MermaidDiagram definition={mermaidDefinition} />;
    }

    return <pre {...props}>{children}</pre>;
  },
};

const TRANSLATIONS: Record<AppLanguageId, Record<string, string>> = {
  en: {},
  "zh-CN": {
    "Local workspace": "本地工作区",
    "Refresh workspaces": "刷新工作区",
    "Add workspace": "添加工作区",
    "Collapse chat history": "收起聊天历史",
    "Expand chat history": "展开聊天历史",
    "New chat": "新建聊天",
    "New chat in {name}": "在 {name} 中新建聊天",
    "Delete chat": "删除聊天",
    "Delete chat {title}": "删除聊天 {title}",
    "Delete this chat?": "删除此聊天？",
    "This will delete the saved chat history.": "这会删除已保存的聊天历史。",
    "Cancel chat deletion": "取消删除聊天",
    "Confirm delete chat": "确认删除聊天",
    "Close chat tab {title}": "关闭会话标签 {title}",
    "Chat is running": "会话运行中",
    "No chats": "暂无聊天",
    "No open chats": "暂无打开的会话",
    "Loading workspaces...": "正在加载工作区...",
    "No workspaces": "暂无工作区",
    Workspaces: "工作区",
    "Workspace settings": "工作区设置",
    "Workspace order and terminal shell": "工作区顺序与终端 Shell",
    "Workspace configuration": "工作区配置",
    "Close workspace configuration": "关闭工作区配置",
    "Edit workspace": "编辑工作区",
    "Workspace list": "工作区列表",
    "Terminal shell": "终端 Shell",
    "Pinned workspace": "置顶工作区",
    "Save workspace": "保存工作区",
    "Default workspace": "Default 工作区",
    pinned: "已置顶",
    "Pin workspace": "置顶工作区",
    "Unpin workspace": "取消置顶工作区",
    "Pin workspace {name}": "置顶工作区 {name}",
    "Unpin workspace {name}": "取消置顶工作区 {name}",
    "Edit workspace {name}": "编辑工作区 {name}",
    "Reorder workspace {name}": "调整工作区顺序 {name}",
    "All workspaces": "全部工作区",
    "All chats": "全部聊天",
    Workspace: "工作区",
    Chat: "聊天",
    Settings: "设置",
    Stats: "统计",
    "Close terminal": "关闭终端",
    "Close terminal {number}": "关闭终端 {number}",
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
    "Foco needs your answer": "Foco 需要你的回答",
    "Waiting for your answer": "正在等待你的回答",
    "Custom answer": "手动输入",
    "Continue run": "继续运行",
    "Answer must not be empty.": "回答不能为空。",
    "Create or register a local folder.": "创建或注册本地文件夹。",
    "Close workspace dialog": "关闭工作区弹窗",
    Close: "关闭",
    Name: "名称",
    "Workspace name": "工作区名称",
    Path: "路径",
    "Choose workspace path": "选择工作区路径",
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
    "Skill locations": "技能位置",
    "Global skill": "全局技能",
    "Workspace skill": "工作区技能",
    "Workspace skill {name}": "工作区技能：{name}",
    disabled: "已禁用",
    "No matching skills": "没有匹配的技能",
    Model: "模型",
    "No enabled models": "没有已启用模型",
    Thinking: "思考",
    "Collapse thinking": "收起思考",
    "Expand thinking": "展开思考",
    "Model default": "模型默认",
    "Retry last run": "重试上次运行",
    "Cancel run": "取消运行",
    "Send message": "发送消息",
    Send: "发送",
    "Context usage": "上下文使用量",
    "Context usage {percent}%": "上下文使用量 {percent}%",
    "Context compression may run on the next send":
      "下次发送可能会触发上下文压缩",
    "Git branch": "Git 分支",
    "Switch to branch {name}": "切换到分支 {name}",
    "No branches": "暂无分支",
    "Create git branch": "创建 Git 分支",
    Input: "输入",
    Output: "输出",
    "Mermaid diagram failed to render.": "Mermaid 图渲染失败。",
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
    "Refresh request audit": "刷新请求审计",
    Columns: "列",
    "Recorded requests": "已记录请求",
    "Input tokens": "输入 token",
    "Output tokens": "输出 token",
    "Average latency": "平均延迟",
    "requests {count}": "请求 {count}",
    "All providers": "全部供应商",
    "All models": "全部模型",
    "All statuses": "全部状态",
    succeeded: "成功",
    failed: "失败",
    "Page size": "每页数量",
    "Showing {start}-{end} of {total}": "显示 {start}-{end}，共 {total}",
    "Page {page} of {totalPages}": "第 {page} 页，共 {totalPages} 页",
    "Previous page": "上一页",
    "Next page": "下一页",
    "Go to page {page}": "前往第 {page} 页",
    "Request audit pagination": "请求审计分页",
    "Started after": "开始时间不早于",
    "Started before": "开始时间不晚于",
    "Request time": "请求时间",
    Provider: "供应商",
    Channel: "渠道",
    "Total time": "总耗时",
    "tokens/s": "token/秒",
    Status: "状态",
    "Cache read": "缓存读取",
    "Cache write": "缓存写入",
    "Cache ratio": "缓存命中率",
    Latency: "延迟",
    "First token": "首 token",
    "First token latency": "首字延迟",
    "Status code": "状态码",
    Details: "详情",
    "View request details": "查看请求详情",
    "No recorded requests": "暂无已记录请求",
    "Request details": "请求详情",
    "Close request details": "关闭请求详情",
    "Request body": "请求正文",
    "Response body": "响应正文",
    Copy: "复制",
    Copied: "已复制",
    "Copy {label}": "复制 {label}",
    "Resize git diff panel": "调整 Git diff 面板宽度",
    "Resize task graph and git diff panels": "调整任务图和 Git diff 面板高度",
    "Resize terminal panel": "调整终端面板高度",
    "New terminal": "新建终端",
    "Terminal sessions": "终端列表",
    "Terminal {number}": "终端 {number}",
    "Task graph": "任务图",
    "Updated {time}": "更新于 {time}",
    pending: "待处理",
    ready: "就绪",
    running: "运行中",
    blocked: "阻塞",
    completed: "已完成",
    cancelled: "已取消",
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
    "Listen address": "监听地址",
    "Listen port": "监听端口",
    "Browser authentication": "浏览器认证",
    "Authentication password": "认证密码",
    "Password required": "需要密码",
    Password: "密码",
    "Show password": "显示密码",
    "Hide password": "隐藏密码",
    "Log in": "登录",
    "Log out": "退出登录",
    "Password is enabled": "已启用密码",
    "Password is disabled": "未启用密码",
    "New password is kept empty unless changed.":
      "不填写则保留当前密码。",
    "Saved password cannot be revealed; type a new password to preview it.":
      "已保存的密码无法显示；输入新密码后可预览。",
    "Set a password to require browser login.":
      "设置密码后，浏览器访问需要先登录。",
    "Clear browser password": "清除浏览器密码",
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

type TerminalPaneStatus = "closed" | "connected" | "connecting" | "error";

type TerminalPanelSession = {
  clientId: string;
  cwd: string;
  error: string | null;
  number: number;
  serverSessionId: string | null;
  status: TerminalPaneStatus;
};

type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  reasoning: string | null;
  status?: "error" | "streaming";
  toolCalls: ChatToolCallSummary[];
  parts: ChatMessagePart[];
  metrics: ChatReplyMetrics | null;
};

type OpenChatTab = {
  workspaceId: string;
  chatId: string;
  fallbackTitle: string;
  fallbackWorkspaceName: string;
};

type ChatTabSummary = OpenChatTab & {
  title: string;
  workspaceName: string;
};

type PendingDeleteChat = {
  workspaceId: string;
  chatId: string;
  title: string;
  workspaceName: string;
};

type RetryRunRequest = {
  workspaceId: string;
  chatId: string | null;
  content: string;
  modelId: string;
  thinkingLevel: string;
  skillIds: string[];
};

type QuestionAnswerSubmission = {
  answers: {
    id: string;
    answer: string;
    selectedOptionValue: string | null;
  }[];
};

export function App() {
  const [authStatus, setAuthStatus] = useState<AuthStatusResponse | null>(null);
  const [authPassword, setAuthPassword] = useState("");
  const [isCheckingAuth, setIsCheckingAuth] = useState(true);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>("");
  const [expandedWorkspaceId, setExpandedWorkspaceId] = useState<string | null>(
    null,
  );
  const [viewMode, setViewMode] = useState<ViewMode>("chat");
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
  const [workspaceDialogRevision, setWorkspaceDialogRevision] = useState(0);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [draftMessage, setDraftMessage] = useState("");
  const [messages, setMessages] = useState<ShellMessage[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [openChatTabs, setOpenChatTabs] = useState<OpenChatTab[]>([]);
  const [pendingDeleteChat, setPendingDeleteChat] =
    useState<PendingDeleteChat | null>(null);
  const [chatMessagesByKey, setChatMessagesByKey] = useState<
    Record<string, ShellMessage[]>
  >({});
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
  const [taskGraphPanelHeightPercent, setTaskGraphPanelHeightPercent] =
    useState(48);
  const [isResizingTaskGraphPanel, setIsResizingTaskGraphPanel] =
    useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(288);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [isMobileWorkspaceOpen, setIsMobileWorkspaceOpen] = useState(false);
  const [terminalOpenWorkspaceIds, setTerminalOpenWorkspaceIds] = useState<
    Set<string>
  >(() => new Set());
  const [gitDiff, setGitDiff] = useState<GitDiffResponse | null>(null);
  const [selectedDiffPath, setSelectedDiffPath] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [taskGraph, setTaskGraph] = useState<TaskGraphResponse | null>(null);
  const [isLoadingTaskGraph, setIsLoadingTaskGraph] = useState(false);
  const [taskGraphError, setTaskGraphError] = useState<string | null>(null);
  const [isSendingMessage, setIsSendingMessage] = useState(false);
  const [runningChatKey, setRunningChatKey] = useState<string | null>(null);
  const [retryRunRequest, setRetryRunRequest] =
    useState<RetryRunRequest | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSelectingWorkspacePath, setIsSelectingWorkspacePath] = useState(false);
  const [pendingQuestion, setPendingQuestion] =
    useState<QuestionRequestSummary | null>(null);
  const [isAnsweringQuestion, setIsAnsweringQuestion] = useState(false);
  const [questionError, setQuestionError] = useState<string | null>(null);
  const [contextUsage, setContextUsage] = useState<ContextUsageResponse | null>(null);
  const [isLoadingContextUsage, setIsLoadingContextUsage] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeRunAbortRef = useRef<AbortController | null>(null);
  const activeChatKeyRef = useRef<string | null>(null);
  const hasManuallySelectedModelRef = useRef(false);
  const diffPanelSplitRef = useRef<HTMLDivElement | null>(null);

  const activeWorkspace = useMemo(
    () =>
      workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
      workspaces[0],
    [activeWorkspaceId, workspaces],
  );
  const chatTabs = useMemo(
    () => openChatTabs.map((tab) => hydrateChatTab(tab, workspaces)),
    [openChatTabs, workspaces],
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
  const isGlobalView = viewMode === "settings" || viewMode === "stats";
  const showDiffPanel = !isGlobalView && isDiffPanelOpen;
  const canLogout = Boolean(settings?.general.webServer.passwordEnabled);
  const language = settings?.general.language ?? "en";
  const t = useCallback<Translate>(
    (key, values) => translate(key, values, language),
    [language],
  );

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  useEffect(() => {
    activeChatKeyRef.current =
      activeChatId === null ? null : chatRunKey(activeWorkspaceId, activeChatId);
  }, [activeChatId, activeWorkspaceId]);

  const loadAuthStatus = useCallback(async () => {
    setIsCheckingAuth(true);
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/status");
      setAuthStatus(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsCheckingAuth(false);
    }
  }, []);

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
      setExpandedWorkspaceId(data.activeWorkspaceId);
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

  useEffect(() => {
    if (isGlobalView || !activeWorkspace || !selectedModelId) {
      setContextUsage(null);
      setIsLoadingContextUsage(false);
      return;
    }

    const abortController = new AbortController();
    const timeoutId = window.setTimeout(() => {
      setIsLoadingContextUsage(true);
      void requestJson<ContextUsageResponse>(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/context-usage`,
        {
          body: JSON.stringify({
            chatId: activeChatId,
            draftMessage: draftMessage.trim() || null,
            modelId: selectedModelId,
            skillIds: selectedSkillIds.length ? selectedSkillIds : null,
            thinkingLevel: selectedThinkingLevel || null,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
          signal: abortController.signal,
        },
      )
        .then((data) => {
          setContextUsage(data);
        })
        .catch((requestError) => {
          if (requestError instanceof DOMException && requestError.name === "AbortError") {
            return;
          }

          setContextUsage(null);
        })
        .finally(() => {
          if (!abortController.signal.aborted) {
            setIsLoadingContextUsage(false);
          }
        });
    }, 300);

    return () => {
      window.clearTimeout(timeoutId);
      abortController.abort();
      setIsLoadingContextUsage(false);
    };
  }, [
    activeChatId,
    activeWorkspace,
    draftMessage,
    isGlobalView,
    selectedModelId,
    selectedSkillIds,
    selectedThinkingLevel,
  ]);

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

  const loadTaskGraph = useCallback(async (workspaceId: string, chatId: string) => {
    const requestedChatKey = chatRunKey(workspaceId, chatId);
    setIsLoadingTaskGraph(true);
    setTaskGraphError(null);

    try {
      const data = await requestJson<TaskGraphResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/task-graph`,
      );
      if (activeChatKeyRef.current === requestedChatKey) {
        setTaskGraph(data);
      }
    } catch (requestError) {
      if (activeChatKeyRef.current === requestedChatKey) {
        setTaskGraph(null);
        setTaskGraphError(errorMessage(requestError));
      }
    } finally {
      if (activeChatKeyRef.current === requestedChatKey) {
        setIsLoadingTaskGraph(false);
      }
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
    void loadAuthStatus();
  }, [loadAuthStatus]);

  useEffect(() => {
    if (!authStatus?.authenticated) {
      return;
    }

    void refreshWorkspaces();
    void loadSettings();
  }, [authStatus?.authenticated, loadSettings, refreshWorkspaces]);

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
    if (!activeWorkspace?.id || !activeChatId) {
      setTaskGraph(null);
      setTaskGraphError(null);
      setIsLoadingTaskGraph(false);
      return;
    }

    setTaskGraph(null);
    setTaskGraphError(null);
    void loadTaskGraph(activeWorkspace.id, activeChatId);
  }, [activeChatId, activeWorkspace?.id, loadTaskGraph]);

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
    setOpenChatTabs((current) => {
      const next = current.filter((tab) => workspaceHasChat(workspaces, tab));
      return next.length === current.length ? current : next;
    });

    setPendingDeleteChat((current) =>
      current && workspaceHasChat(workspaces, current) ? current : null,
    );
  }, [workspaces]);

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
    if (!isResizingTaskGraphPanel) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const container = diffPanelSplitRef.current;
      if (!container) {
        return;
      }

      const rect = container.getBoundingClientRect();
      if (rect.height <= 0) {
        return;
      }

      const nextHeight = ((event.clientY - rect.top) / rect.height) * 100;
      setTaskGraphPanelHeightPercent(
        Math.min(Math.max(nextHeight, 24), 76),
      );
    }

    function handlePointerUp() {
      setIsResizingTaskGraphPanel(false);
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizingTaskGraphPanel]);

  useEffect(() => {
    if (!workspaces.length) {
      setExpandedWorkspaceId(null);
      return;
    }

    setExpandedWorkspaceId((current) => {
      if (
        current === null ||
        workspaces.some((workspace) => workspace.id === current)
      ) {
        return current;
      }

      return activeWorkspace?.id ?? workspaces[0]?.id ?? null;
    });
  }, [activeChatId, activeWorkspace?.id, activeWorkspaceId, workspaces]);

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
      detectedSkills.filter((skill) => skill.enabled).map((skill) => skill.key),
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
      const data = await requestJson<WorkspacesResponse>("/api/workspaces/add", {
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
      setExpandedWorkspaceId(createdWorkspace?.id ?? data.activeWorkspaceId);
      setWorkspaceName("");
      setWorkspacePath("");
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
    }
  }

  async function handleSelectWorkspacePath() {
    setIsSelectingWorkspacePath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        { method: "POST" },
      );

      if (data.path) {
        setWorkspacePath(data.path);
        setWorkspaceName((current) =>
          current.trim() ? current : workspaceNameFromPath(data.path ?? ""),
        );
      }
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSelectingWorkspacePath(false);
    }
  }

  function setMessagesForChatKey(
    chatKey: string | null,
    updater: ShellMessage[] | ((current: ShellMessage[]) => ShellMessage[]),
  ) {
    const resolveNext = (current: ShellMessage[]) =>
      typeof updater === "function" ? updater(current) : updater;

    if (!chatKey) {
      setMessages((current) => resolveNext(current));
      return;
    }

    setChatMessagesByKey((current) => {
      const next = resolveNext(current[chatKey] ?? []);
      return { ...current, [chatKey]: next };
    });

    if (activeChatKeyRef.current === chatKey) {
      setMessages((current) => resolveNext(current));
    }
  }

  function moveMessagesForChatKey(
    fromChatKey: string,
    toChatKey: string,
    updater: (current: ShellMessage[]) => ShellMessage[],
  ) {
    setChatMessagesByKey((current) => {
      const nextMessages = updater(current[fromChatKey] ?? []);
      const { [fromChatKey]: _removed, ...next } = current;
      return { ...next, [toChatKey]: nextMessages };
    });

    if (activeChatKeyRef.current === fromChatKey) {
      activeChatKeyRef.current = toChatKey;
      setMessages((current) => updater(current));
    }
  }

  function removeMessagesForChatKey(chatKey: string) {
    setChatMessagesByKey((current) => {
      if (!(chatKey in current)) {
        return current;
      }

      const { [chatKey]: _removed, ...next } = current;
      return next;
    });
  }

  async function loadChatMessages(workspaceId: string, chatId: string) {
    setError(null);

    try {
      const data = await requestJson<ChatMessagesResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/chats/${encodeURIComponent(chatId)}/messages`,
      );
      const chatKey = chatRunKey(workspaceId, chatId);
      const nextMessages = data.messages.map(normalizeChatMessageSummary);
      setActiveWorkspaceId(workspaceId);
      setActiveChatId(chatId);
      setExpandedWorkspaceId(workspaceId);
      openChatTab(workspaceId, chatId);
      activeChatKeyRef.current = chatKey;
      setMessages(nextMessages);
      setChatMessagesByKey((current) => ({ ...current, [chatKey]: nextMessages }));
      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  function selectWorkspaceChat(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);
    const cachedMessages = chatMessagesByKey[chatKey];

    if (!cachedMessages) {
      void loadChatMessages(workspaceId, chatId);
      return;
    }

    setActiveWorkspaceId(workspaceId);
    setActiveChatId(chatId);
    setExpandedWorkspaceId(workspaceId);
    openChatTab(workspaceId, chatId);
    activeChatKeyRef.current = chatKey;
    setMessages(cachedMessages);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
  }

  function startNewWorkspaceChat(workspaceId: string) {
    setExpandedWorkspaceId(workspaceId);
    setActiveWorkspaceId(workspaceId);
    setActiveChatId(null);
    activeChatKeyRef.current = null;
    setMessages([]);
    setSelectedDiffPath(null);
    setViewMode("chat");
    setIsMobileWorkspaceOpen(false);
  }

  function openChatTab(workspaceId: string, chatId: string) {
    const workspace = workspaces.find((workspace) => workspace.id === workspaceId);
    const chat = workspace?.chats.find((chat) => chat.id === chatId);

    setOpenChatTabs((current) =>
      upsertOpenChatTab(current, {
        workspaceId,
        chatId,
        fallbackTitle: chat?.title ?? t("Chat"),
        fallbackWorkspaceName: workspace?.name ?? t("Workspace"),
      }),
    );
  }

  function closeChatTab(workspaceId: string, chatId: string) {
    const chatKey = chatRunKey(workspaceId, chatId);
    if (runningChatKey === chatKey) {
      return;
    }

    const tabIndex = openChatTabs.findIndex(
      (tab) => tab.workspaceId === workspaceId && tab.chatId === chatId,
    );
    setOpenChatTabs((current) =>
      current.filter(
        (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
      ),
    );
    removeMessagesForChatKey(chatKey);

    if (activeWorkspaceId !== workspaceId || activeChatId !== chatId) {
      return;
    }

    const nextTabs = openChatTabs.filter(
      (tab) => tab.workspaceId !== workspaceId || tab.chatId !== chatId,
    );
    const nextTab =
      nextTabs[Math.min(tabIndex, nextTabs.length - 1)] ?? nextTabs.at(-1);

    if (nextTab) {
      selectWorkspaceChat(nextTab.workspaceId, nextTab.chatId);
      return;
    }

    setActiveChatId(null);
    activeChatKeyRef.current = null;
    setMessages([]);
  }

  function requestDeleteWorkspaceChat(workspace: WorkspaceSummary, chat: ChatSummary) {
    if (runningChatKey === chatRunKey(workspace.id, chat.id)) {
      setError(t("Cancel the current run before deleting this chat."));
      return;
    }

    setError(null);
    setPendingDeleteChat({
      workspaceId: workspace.id,
      chatId: chat.id,
      title: chat.title,
      workspaceName: workspace.name,
    });
  }

  async function confirmDeleteWorkspaceChat() {
    const target = pendingDeleteChat;
    if (!target) {
      return;
    }

    await deleteWorkspaceChat(target.workspaceId, target.chatId);
  }

  async function deleteWorkspaceChat(workspaceId: string, chatId: string) {
    if (runningChatKey === chatRunKey(workspaceId, chatId)) {
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
      setPendingDeleteChat(null);
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

  async function handleQuestionSubmit(answer: QuestionAnswerSubmission) {
    if (!pendingQuestion || isAnsweringQuestion) {
      return;
    }

    setIsAnsweringQuestion(true);
    setQuestionError(null);

    try {
      await requestJson<{ ok: boolean; questionId: string }>(
        `/api/chat/questions/${encodeURIComponent(pendingQuestion.id)}/answer`,
        {
          body: JSON.stringify(answer),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      setPendingQuestion((current) =>
        current?.id === pendingQuestion.id ? null : current,
      );
    } catch (requestError) {
      setQuestionError(errorMessage(requestError));
    } finally {
      setIsAnsweringQuestion(false);
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

    const skillIds = [...selectedSkillIds];
    setSelectedSkillIds([]);

    await runChatMessage({
      chatId: activeChatId,
      content,
      modelId: selectedModelId,
      skillIds,
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
    setPendingQuestion(null);
    setQuestionError(null);
    setIsAnsweringQuestion(false);
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
    let runMessagesKey = requestChatId
      ? chatRunKey(request.workspaceId, requestChatId)
      : pendingChatRunKey(request.workspaceId, runKey);
    let currentRunningChatKey = runMessagesKey;
    const abortController = new AbortController();

    activeChatKeyRef.current = runMessagesKey;
    setMessagesForChatKey(runMessagesKey, (current) => [
      ...current,
      {
        id: localUserId,
        role: "user",
        content: visibleUserContent,
        reasoning: null,
        toolCalls: [],
        parts: [{ type: "text", text: visibleUserContent }],
        metrics: null,
      },
      {
        id: localAssistantId,
        role: "assistant",
        content: "",
        reasoning: null,
        status: "streaming",
        toolCalls: [],
        parts: [],
        metrics: null,
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
          credentials: "same-origin",
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
          openChatTab(request.workspaceId, streamEvent.chatId);
          if (runMessagesKey !== currentRunningChatKey) {
            moveMessagesForChatKey(runMessagesKey, currentRunningChatKey, (current) =>
              current.map((message) => {
                if (message.id === localUserId) {
                  return { ...message, id: streamEvent.userMessageId };
                }

                if (
                  message.role === "assistant" &&
                  message.id === localAssistantId
                ) {
                  return { ...message, id: streamEvent.assistantMessageId };
                }

                return message;
              }),
            );
            runMessagesKey = currentRunningChatKey;
          } else {
            setMessagesForChatKey(currentRunningChatKey, (current) =>
              current.map((message) => {
                if (message.id === localUserId) {
                  return { ...message, id: streamEvent.userMessageId };
                }

                if (
                  message.role === "assistant" &&
                  message.id === localAssistantId
                ) {
                  return { ...message, id: streamEvent.assistantMessageId };
                }

                return message;
              }),
            );
          }
          if (
            activeChatKeyRef.current === currentRunningChatKey ||
            activeChatKeyRef.current === null ||
            request.chatId
          ) {
            setActiveChatId(streamEvent.chatId);
            activeChatKeyRef.current = currentRunningChatKey;
          }
          setRunningChatKey(currentRunningChatKey);
          void refreshWorkspaces();
          return;
        }

        if (streamEvent.type === "textDelta") {
          setMessagesForChatKey(runMessagesKey, (current) =>
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
          setMessagesForChatKey(runMessagesKey, (current) =>
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
          setRetryRunRequest(null);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message, streamEvent.assistantMessageId)
                ? completedAssistantMessage(message, streamEvent)
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          setMessagesForChatKey(runMessagesKey, (current) =>
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
          setMessagesForChatKey(runMessagesKey, (current) =>
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

        if (streamEvent.type === "questionRequest") {
          setQuestionError(null);
          setPendingQuestion(streamEvent.request);
          return;
        }

        if (streamEvent.type === "gitDiffRefresh") {
          void loadGitDiff(streamEvent.workspaceId, selectedDiffPath);
          return;
        }

        if (streamEvent.type === "taskGraphRefresh") {
          const activeKey = activeChatKeyRef.current;
          if (
            activeKey ===
            chatRunKey(streamEvent.workspaceId, streamEvent.chatId)
          ) {
            setIsDiffPanelOpen(true);
            void loadTaskGraph(streamEvent.workspaceId, streamEvent.chatId);
          }
          return;
        }

        if (streamEvent.type === "error") {
          setError(streamEvent.message);
          setPendingQuestion(null);
          setQuestionError(null);
          setIsAnsweringQuestion(false);
          setMessagesForChatKey(runMessagesKey, (current) =>
            current.map((message) =>
              isCurrentAssistantMessage(message)
                ? {
                    ...message,
                    content: streamEvent.message,
                    parts: [{ type: "text", text: streamEvent.message }],
                    metrics: null,
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
      setPendingQuestion(null);
      setQuestionError(null);
      setIsAnsweringQuestion(false);
      setRetryRunRequest({
        ...request,
        chatId: requestChatId,
      });
      setMessagesForChatKey(runMessagesKey, (current) =>
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
    setExpandedWorkspaceId((current) =>
      current === workspaceId ? null : workspaceId,
    );
  }

  function openWorkspaceDialog() {
    setWorkspaceName("");
    setWorkspacePath("");
    setError(null);
    setWorkspaceDialogRevision((current) => current + 1);
    setIsWorkspaceDialogOpen(true);
  }

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsLoggingIn(true);
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/login", {
        body: JSON.stringify({ password: authPassword }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      setAuthStatus(data);
      setAuthPassword("");
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoggingIn(false);
    }
  }

  async function handleLogout() {
    setError(null);

    try {
      const data = await requestJson<AuthStatusResponse>("/api/auth/logout", {
        method: "POST",
      });
      setAuthStatus(data);
      setWorkspaces([]);
      setSettings(null);
      setOpenChatTabs([]);
      setActiveChatId(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
    }
  }

  if (isCheckingAuth) {
    return (
      <I18nContext.Provider value={{ language, t }}>
        <main className="app-root grid place-items-center bg-stone-100 text-stone-950">
          <LoaderCircle aria-hidden="true" className="size-6 animate-spin text-teal-700" />
        </main>
      </I18nContext.Provider>
    );
  }

  if (authStatus?.enabled && !authStatus.authenticated) {
    return (
      <I18nContext.Provider value={{ language, t }}>
        <LoginView
          error={error}
          isLoggingIn={isLoggingIn}
          onLogin={(event) => void handleLogin(event)}
          onPasswordChange={setAuthPassword}
          password={authPassword}
        />
      </I18nContext.Provider>
    );
  }

  return (
    <I18nContext.Provider value={{ language, t }}>
    <main className="app-root text-stone-950">
      {isGlobalView ? (
        <section className="flex h-full min-h-0 flex-col">
          <header className="shrink-0 border-b border-stone-200/80 bg-white/85 px-4 py-3 backdrop-blur">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <FocoLogoMark />
                <div className="min-w-0">
                  <span className="block truncate text-lg font-semibold">
                    Foco
                  </span>
                  <span className="block truncate text-xs text-stone-500">
                    {t("Local workspace")}
                  </span>
                </div>
              </div>
              <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                <button
                  aria-label={t("Workspaces")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={() => setViewMode("chat")}
                  title={t("Workspaces")}
                  type="button"
                >
                  <Folder aria-hidden="true" className="size-4" />
                </button>
                <div className="flex rounded-xl border border-stone-200 bg-stone-100/80 p-1 shadow-inner">
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
            </div>
          </header>
          {viewMode === "settings" ? (
            <SettingsPanel
              canLogout={canLogout}
              onAddWorkspace={openWorkspaceDialog}
              onLogout={handleLogout}
              onSettingsChange={setSettings}
              onWorkspacesChange={refreshWorkspaces}
              workspaceDialogRevision={workspaceDialogRevision}
            />
          ) : (
            <ApiStatsPanel
              settings={settings}
              workspaces={workspaces}
            />
          )}
        </section>
      ) : (
        <div
          className={`app-shell ${showDiffPanel ? "app-shell-with-diff" : ""}`}
          style={
            {
              "--diff-panel-width": `${diffPanelWidth}px`,
              "--sidebar-width": `${sidebarWidth}px`,
              "--task-graph-panel-height": `${taskGraphPanelHeightPercent}%`,
            } as CSSProperties
          }
        >
        {isMobileWorkspaceOpen ? (
          <button
            aria-label={t("Close")}
            className="mobile-sidebar-backdrop"
            onClick={() => setIsMobileWorkspaceOpen(false)}
            type="button"
          />
        ) : null}
        <aside
          className={`workspace-sidebar relative border-stone-200/80 lg:border-r ${
            isMobileWorkspaceOpen ? "workspace-sidebar-mobile-open" : ""
          }`}
        >
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
            <div className="flex items-center justify-between gap-2 border-b border-stone-200/80 px-4 py-2">
              <div className="flex min-w-0 items-center gap-3">
                <FocoLogoMark />
                <div className="min-w-0">
                  <span className="block truncate text-lg font-semibold">
                    Foco
                  </span>
                  <span className="mt-1 block truncate text-xs text-stone-500">
                    {t("Local workspace")}
                  </span>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <button
                  aria-label={t("Close")}
                  className="mobile-sidebar-close inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                  onClick={() => setIsMobileWorkspaceOpen(false)}
                  title={t("Close")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Settings")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={() => setViewMode("settings")}
                  title={t("Settings")}
                  type="button"
                >
                  <Settings aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Stats")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={() => setViewMode("stats")}
                  title={t("Stats")}
                  type="button"
                >
                  <BarChart3 aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={t("Add workspace")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={() => openWorkspaceDialog()}
                  title={t("Add workspace")}
                  type="button"
                >
                  <FolderPlus aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>

            {error ? (
              <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
                {error}
              </div>
            ) : null}

            <nav className="panel-scroll min-h-0 flex-1 overflow-y-auto px-2 py-3">
              {workspaces.length ? (
                workspaces.map((workspace) => {
                const isExpanded = expandedWorkspaceId === workspace.id;
                const isActive = workspace.id === activeWorkspace?.id;
                const isNewChatActive = isActive && activeChatId === null;

                return (
                  <div className="mb-1.5" key={workspace.id}>
                    <div className={workspaceMenuClass(isActive)}>
                      <button
                        aria-expanded={isExpanded}
                        className={workspaceItemClass(isActive)}
                        onClick={() => toggleWorkspace(workspace.id)}
                        title={
                          isExpanded
                            ? t("Collapse chat history")
                            : t("Expand chat history")
                        }
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
                        className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg text-stone-500 hover:text-teal-800"
                        onClick={() => startNewWorkspaceChat(workspace.id)}
                        title={t("New chat")}
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    {isExpanded ? (
                      <div className="ml-4 mt-1 space-y-1 border-l border-stone-200/80 pl-2">
                        {isNewChatActive ? (
                          <div className="group flex min-w-0 items-center gap-1">
                            <button
                              aria-current="page"
                              className={chatItemClass(true)}
                              onClick={() => startNewWorkspaceChat(workspace.id)}
                              type="button"
                            >
                              <MessageSquare
                                aria-hidden="true"
                                className="size-3.5 shrink-0"
                              />
                              <span className="min-w-0 flex-1">
                                <span className="block truncate">
                                  {t("New chat")}
                                </span>
                                <span
                                  aria-hidden="true"
                                  className="mt-0.5 block truncate text-[0.68rem] font-normal leading-tight text-transparent"
                                >
                                  0
                                </span>
                              </span>
                            </button>
                            <span
                              aria-hidden="true"
                              className="inline-flex size-7 shrink-0"
                            />
                          </div>
                        ) : null}
                        {workspace.chats.length > 0 ? (
                          workspace.chats.map((chat) => {
                            const isChatRunning =
                              runningChatKey === chatRunKey(workspace.id, chat.id);
                            const isChatActive =
                              activeWorkspace?.id === workspace.id &&
                              activeChatId === chat.id;

                            return (
                              <div
                                className="group flex min-w-0 items-center gap-1"
                                key={chat.id}
                              >
                                <button
                                  aria-current={
                                    isChatActive ? "page" : undefined
                                  }
                                  className={chatItemClass(isChatActive)}
                                  onClick={() =>
                                    selectWorkspaceChat(workspace.id, chat.id)
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
                                    requestDeleteWorkspaceChat(workspace, chat)
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
                        ) : !isNewChatActive ? (
                          <div className="rounded-lg px-2 py-1.5 text-xs text-stone-500">
                            {t("No chats")}
                          </div>
                        ) : null}
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
              <header className="shrink-0 border-b border-stone-200/80 bg-white/80 px-3 py-2 backdrop-blur sm:px-4">
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <button
                    aria-label={t("Workspaces")}
                    className="mobile-workspace-button inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 md:hidden"
                    onClick={() => setIsMobileWorkspaceOpen(true)}
                    title={t("Workspaces")}
                    type="button"
                  >
                    <Folder aria-hidden="true" className="size-4" />
                  </button>
                  <ChatTabBar
                    activeChatId={activeChatId}
                    activeWorkspaceId={activeWorkspaceId}
                    onCloseTab={closeChatTab}
                    onSelectTab={selectWorkspaceChat}
                    runningChatKey={runningChatKey}
                    tabs={chatTabs}
                  />
                  <div className="chat-header-actions flex flex-wrap items-center gap-2">
                    <div className="flex overflow-x-auto rounded-xl border border-stone-200 bg-stone-100/80 p-1 shadow-inner">
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
                        title={
                          isDiffPanelOpen ? t("Close git diff") : t("Open git diff")
                        }
                        type="button"
                      >
                        <GitCompare aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    <div className="flex rounded-xl border border-stone-200 bg-stone-100/80 p-1 shadow-inner">
                      <button
                        aria-label={
                          isTerminalOpen
                            ? t("Close terminal")
                            : t("Open terminal")
                        }
                        className={`inline-flex size-9 items-center justify-center rounded-lg ${
                          isTerminalOpen
                            ? "bg-white text-teal-900 shadow-sm"
                            : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
                        } disabled:cursor-not-allowed disabled:text-stone-400`}
                        disabled={!activeWorkspace}
                        onClick={toggleWorkspaceTerminal}
                        title={
                          isTerminalOpen ? t("Close terminal") : t("Open terminal")
                        }
                        type="button"
                      >
                        <Terminal aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                  </div>
                </div>
              </header>
          <ChatPanel
              availableModels={availableModels}
              branchError={branchError}
              chatScrollKey={`${activeWorkspaceId}:${activeChatId ?? ""}`}
              contextUsage={contextUsage}
              draftMessage={draftMessage}
              gitBranches={gitBranches}
              isLoadingContextUsage={isLoadingContextUsage}
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
          {isTerminalOpen ? (
            <TerminalPanel
              onClose={() => {
                if (activeWorkspace) {
                  setTerminalOpenWorkspaceIds((current) => {
                    const next = new Set(current);
                    next.delete(activeWorkspace.id);
                    return next;
                  });
                }
              }}
              workspace={activeWorkspace}
            />
          ) : null}
        </section>

        {showDiffPanel ? (
        <aside className="diff-sidebar min-w-0 border-stone-200/80 lg:border-l">
          <div className="relative flex h-full min-h-0 min-w-0 flex-col">
            <div
              aria-label={t("Resize git diff panel")}
              aria-orientation="vertical"
              className="absolute bottom-0 left-0 top-0 z-10 hidden w-1 cursor-col-resize bg-transparent hover:bg-teal-500/40 lg:block"
              onKeyDown={(event) => {
                if (event.key === "ArrowLeft") {
                  event.preventDefault();
                  setDiffPanelWidth((current) =>
                    Math.min(current + 24, 720),
                  );
                }

                if (event.key === "ArrowRight") {
                  event.preventDefault();
                  setDiffPanelWidth((current) =>
                    Math.max(current - 24, 280),
                  );
                }
              }}
              onPointerDown={() => setIsResizingDiffPanel(true)}
              role="separator"
              tabIndex={0}
            />
            {taskGraph?.exists && taskGraph.tasks.length ? (
              <div
                className="diff-panel-split flex min-h-0 flex-1 flex-col"
                ref={diffPanelSplitRef}
              >
                <div className="task-graph-panel-slot min-h-0">
                  <TaskGraphPanel
                    error={taskGraphError}
                    isLoading={isLoadingTaskGraph}
                    taskGraph={taskGraph}
                  />
                </div>
                <div
                  aria-label={t("Resize task graph and git diff panels")}
                  aria-orientation="horizontal"
                  className="diff-panel-row-resizer group relative h-3 shrink-0 cursor-row-resize border-y border-stone-200/80 bg-stone-100/70"
                  onKeyDown={(event) => {
                    if (event.key === "ArrowUp") {
                      event.preventDefault();
                      setTaskGraphPanelHeightPercent((current) =>
                        Math.max(current - 5, 24),
                      );
                    }

                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      setTaskGraphPanelHeightPercent((current) =>
                        Math.min(current + 5, 76),
                      );
                    }
                  }}
                  onPointerDown={() => setIsResizingTaskGraphPanel(true)}
                  role="separator"
                  tabIndex={0}
                >
                  <span className="absolute left-1/2 top-1/2 h-1 w-10 -translate-x-1/2 -translate-y-1/2 rounded-full bg-stone-300 transition-colors group-hover:bg-teal-500/70" />
                </div>
                <div className="git-diff-panel-slot min-h-0">
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
                    onSelectFile={setSelectedDiffPath}
                    selectedPath={selectedDiffPath}
                  />
                </div>
              </div>
            ) : (
              <div className="min-h-0 flex-1">
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
                  onSelectFile={setSelectedDiffPath}
                  selectedPath={selectedDiffPath}
                />
              </div>
            )}
          </div>
        </aside>
        ) : null}
      </div>
      )}
      {isWorkspaceDialogOpen ? (
        <WorkspaceDialog
          isSelectingPath={isSelectingWorkspacePath}
          isSaving={isSavingWorkspace}
          name={workspaceName}
          onClose={() => setIsWorkspaceDialogOpen(false)}
          onNameChange={setWorkspaceName}
          onPathChange={setWorkspacePath}
          onSelectPath={handleSelectWorkspacePath}
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
      {pendingDeleteChat ? (
        <DeleteChatDialog
          chat={pendingDeleteChat}
          onClose={() => setPendingDeleteChat(null)}
          onConfirm={() => void confirmDeleteWorkspaceChat()}
        />
      ) : null}
      {pendingQuestion ? (
        <QuestionDialog
          error={questionError}
          isSaving={isAnsweringQuestion}
          onCancelRun={handleCancelRun}
          onSubmit={handleQuestionSubmit}
          question={pendingQuestion}
        />
      ) : null}
    </main>
    </I18nContext.Provider>
  );
}

function WorkspaceDialog({
  isSelectingPath,
  isSaving,
  name,
  onClose,
  onNameChange,
  onPathChange,
  onSelectPath,
  onSubmit,
  path,
}: {
  isSelectingPath: boolean;
  isSaving: boolean;
  name: string;
  onClose: () => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSelectPath: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  path: string;
}) {
  const { t } = useI18n();
  const title = t("Add workspace");

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
              {t("Create or register a local folder.")}
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
            <div className="flex gap-2">
              <input
                autoComplete="off"
                className="h-11 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                name="workspace-path"
                onChange={(event) => onPathChange(event.target.value)}
                placeholder="C:/Users/name/workspace"
                value={path}
              />
              <button
                aria-label={t("Choose workspace path")}
                className="inline-flex size-11 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                disabled={isSelectingPath}
                onClick={onSelectPath}
                title={t("Choose workspace path")}
                type="button"
              >
                {isSelectingPath ? (
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

function DeleteChatDialog({
  chat,
  onClose,
  onConfirm,
}: {
  chat: PendingDeleteChat;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="delete-chat-dialog-title"
        aria-modal="true"
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <Trash2 aria-hidden="true" className="size-5 text-rose-700" />
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="delete-chat-dialog-title"
            >
              {t("Delete this chat?")}
            </h2>
          </div>
          <button
            aria-label={t("Cancel chat deletion")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Cancel")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="space-y-4 px-4 py-4">
          <div>
            <p className="text-sm font-medium text-stone-950">{chat.title}</p>
            <p className="mt-1 text-xs font-medium text-stone-500">
              {chat.workspaceName}
            </p>
          </div>
          <p className="text-sm leading-6 text-stone-600">
            {t("This will delete the saved chat history.")}
          </p>
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel chat deletion")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-stone-300 hover:bg-stone-50"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Confirm delete chat")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-rose-700 text-white shadow-[0_12px_28px_rgba(190,18,60,0.22)] hover:bg-rose-800"
              onClick={onConfirm}
              title={t("Delete chat")}
              type="button"
            >
              <Trash2 aria-hidden="true" className="size-4" />
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function QuestionDialog({
  error,
  isSaving,
  onCancelRun,
  onSubmit,
  question,
}: {
  error: string | null;
  isSaving: boolean;
  onCancelRun: () => void;
  onSubmit: (answer: QuestionAnswerSubmission) => void;
  question: QuestionRequestSummary;
}) {
  const { t } = useI18n();
  const [draftAnswers, setDraftAnswers] = useState<
    Record<string, { manualAnswer: string; selectedOptionValue: string | null }>
  >({});
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    setDraftAnswers(
      Object.fromEntries(
        question.questions.map((item) => [
          item.id,
          {
            manualAnswer: "",
            selectedOptionValue: null,
          },
        ]),
      ),
    );
    setLocalError(null);
  }, [question.id, question.questions]);

  function submitAnswer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const answers = question.questions.map((item) => {
      const draft = draftAnswers[item.id] ?? {
        manualAnswer: "",
        selectedOptionValue: null,
      };

      if (draft.selectedOptionValue !== null) {
        return {
          id: item.id,
          answer: draft.selectedOptionValue,
          selectedOptionValue: draft.selectedOptionValue,
        };
      }

      return {
        id: item.id,
        answer: draft.manualAnswer.trim(),
        selectedOptionValue: null,
      };
    });

    if (answers.some((answer) => !answer.answer)) {
      setLocalError(t("Answer must not be empty."));
      return;
    }

    onSubmit({ answers });
  }

  const displayedError = localError ?? error;
  const canSubmit =
    question.questions.length > 0 &&
    question.questions.every((item) => {
      const draft = draftAnswers[item.id];
      if (!draft) {
        return false;
      }

      return (
        draft.selectedOptionValue !== null ||
        draft.manualAnswer.trim().length > 0
      );
    });

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="question-dialog-title"
        aria-modal="true"
        className="w-full max-w-xl overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <MessageSquare
              aria-hidden="true"
              className="size-5 shrink-0 text-teal-700"
            />
            <div className="min-w-0">
              <h2
                className="truncate text-base font-semibold text-stone-950"
                id="question-dialog-title"
              >
                {t("Foco needs your answer")}
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {t("Waiting for your answer")}
              </p>
            </div>
          </div>
          <button
            aria-label={t("Cancel run")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onCancelRun}
            title={t("Cancel run")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <form
          className="max-h-[min(72vh,720px)] space-y-4 overflow-y-auto px-4 py-4"
          onSubmit={submitAnswer}
        >
          <div className="space-y-4">
            {question.questions.map((item, index) => {
              const draft = draftAnswers[item.id] ?? {
                manualAnswer: "",
                selectedOptionValue: null,
              };

              return (
                <section
                  className="space-y-3 rounded-lg border border-stone-200 bg-stone-50/60 p-3"
                  key={item.id}
                >
                  <p className="whitespace-pre-wrap text-sm font-semibold leading-6 text-stone-900">
                    {question.questions.length > 1
                      ? `${index + 1}. ${item.question}`
                      : item.question}
                  </p>

                  {item.options.length ? (
                    <div className="space-y-2">
                      {item.options.map((option) => {
                        const isSelected =
                          draft.selectedOptionValue === option.value;
                        return (
                          <label
                            className={`flex cursor-pointer gap-3 rounded-lg border px-3 py-2 text-sm transition ${
                              isSelected
                                ? "border-teal-700 bg-teal-50 text-teal-950"
                                : "border-stone-200 bg-white text-stone-800 hover:border-teal-200 hover:bg-teal-50/60"
                            }`}
                            key={option.value}
                          >
                            <input
                              checked={isSelected}
                              className="mt-1 size-4 accent-teal-800"
                              name={`question-option-${item.id}`}
                              onChange={() => {
                                setDraftAnswers((current) => ({
                                  ...current,
                                  [item.id]: {
                                    manualAnswer:
                                      current[item.id]?.manualAnswer ?? "",
                                    selectedOptionValue: option.value,
                                  },
                                }));
                                setLocalError(null);
                              }}
                              type="radio"
                            />
                            <span className="min-w-0">
                              <span className="block font-semibold">
                                {option.label}
                              </span>
                              {option.description ? (
                                <span className="mt-0.5 block text-xs leading-5 text-stone-500">
                                  {option.description}
                                </span>
                              ) : null}
                            </span>
                          </label>
                        );
                      })}
                    </div>
                  ) : null}

                  {item.allowFreeText ? (
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                        {t("Custom answer")}
                      </span>
                      <textarea
                        className="min-h-24 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                        onChange={(event) => {
                          setDraftAnswers((current) => ({
                            ...current,
                            [item.id]: {
                              manualAnswer: event.target.value,
                              selectedOptionValue: null,
                            },
                          }));
                          setLocalError(null);
                        }}
                        value={draft.manualAnswer}
                      />
                    </label>
                  ) : null}
                </section>
              );
            })}
          </div>

          {displayedError ? (
            <div className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {displayedError}
            </div>
          ) : null}

          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel run")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onCancelRun}
              title={t("Cancel run")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Continue run")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isSaving || !canSubmit}
              title={t("Continue run")}
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
        </form>
      </section>
    </div>
  );
}

function ChatTabBar({
  activeChatId,
  activeWorkspaceId,
  onCloseTab,
  onSelectTab,
  runningChatKey,
  tabs,
}: {
  activeChatId: string | null;
  activeWorkspaceId: string;
  onCloseTab: (workspaceId: string, chatId: string) => void;
  onSelectTab: (workspaceId: string, chatId: string) => void;
  runningChatKey: string | null;
  tabs: ChatTabSummary[];
}) {
  const { t } = useI18n();

  return (
    <div className="min-w-0 flex-1">
      <div
        aria-label={t("Chat")}
        className="chat-tab-list panel-scroll flex min-w-0 gap-1 overflow-x-auto"
        role="tablist"
      >
        {tabs.length ? (
          tabs.map((tab) => {
            const isActive =
              activeWorkspaceId === tab.workspaceId && activeChatId === tab.chatId;
            const isRunning = runningChatKey === chatRunKey(tab.workspaceId, tab.chatId);
            const title = tab.title || t("Chat");

            return (
              <div
                className={`chat-tab-item group flex h-12 min-w-44 max-w-64 shrink-0 items-center rounded-lg border px-2 py-1.5 transition-colors ${
                  isActive
                    ? "border-teal-200 bg-white text-stone-950 shadow-sm"
                    : "border-stone-200 bg-stone-50/80 text-stone-600 hover:border-stone-300 hover:bg-white"
                }`}
                key={chatRunKey(tab.workspaceId, tab.chatId)}
              >
                <button
                  aria-selected={isActive}
                  className="min-w-0 flex-1 text-left"
                  onClick={() => onSelectTab(tab.workspaceId, tab.chatId)}
                  role="tab"
                  title={`${title} · ${tab.workspaceName}`}
                  type="button"
                >
                  <span className="block truncate text-sm font-semibold leading-5">
                    {title}
                  </span>
                  <span className="block truncate text-[11px] font-medium leading-4 text-stone-400">
                    {tab.workspaceName}
                  </span>
                </button>
                <span className="ml-1 inline-flex size-7 shrink-0 items-center justify-center">
                  {isRunning ? (
                    <span aria-label={t("Chat is running")} role="status">
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin text-teal-700"
                      />
                    </span>
                  ) : (
                    <button
                      aria-label={t("Close chat tab {title}", { title })}
                      className="inline-flex size-7 items-center justify-center rounded-md text-stone-400 opacity-0 hover:bg-rose-50 hover:text-rose-700 focus:opacity-100 group-hover:opacity-100"
                      onClick={() => onCloseTab(tab.workspaceId, tab.chatId)}
                      title={t("Close")}
                      type="button"
                    >
                      <X aria-hidden="true" className="size-3.5" />
                    </button>
                  )}
                </span>
              </div>
            );
          })
        ) : (
          <div className="flex h-12 min-w-0 items-center rounded-lg border border-dashed border-stone-300 bg-white/55 px-3 text-sm font-medium text-stone-500">
            {t("No open chats")}
          </div>
        )}
      </div>
    </div>
  );
}

function ChatPanel({
  availableModels,
  branchError,
  chatScrollKey,
  canRetryRun,
  contextUsage,
  draftMessage,
  gitBranches,
  isLoadingContextUsage,
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
  contextUsage: ContextUsageResponse | null;
  draftMessage: string;
  gitBranches: GitBranchesResponse | null;
  isLoadingContextUsage: boolean;
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
  const messageTextareaRef = useRef<HTMLTextAreaElement>(null);
  const shouldLockMessageScrollRef = useRef(true);
  const skillQuery = activeSkillQuery(draftMessage);
  const selectedSkillSet = new Set(selectedSkillIds);
  const selectedSkills = selectedSkillIds
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill));
  const modelOptions = availableModels.map((model) => ({
    label: model.displayName,
    value: model.id,
  }));
  const thinkingOptions = [
    { label: t("Model default"), value: "" },
    ...thinkingLevels.map((level) => ({
      label: t(level.label),
      value: level.value,
    })),
  ];
  const visibleSkills =
    skillQuery === null
      ? []
      : skills.filter((skill) => {
          const query = skillQuery.toLowerCase();
          return (
            !selectedSkillSet.has(skill.key) &&
            (skill.name.toLowerCase().includes(query) ||
              skill.id.toLowerCase().includes(query) ||
              skill.key.toLowerCase().includes(query) ||
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
    onToggleSkill(skill.key);
  }

  function handleComposerSubmit(event: FormEvent<HTMLFormElement>) {
    onSubmit(event);
    window.requestAnimationFrame(() => messageTextareaRef.current?.focus());
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
                          isStreaming={message.status === "streaming"}
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
                    {!isUser && message.metrics ? (
                      <ChatReplyMetricsLine metrics={message.metrics} />
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

      <div className="shrink-0 border-t border-stone-200/80 bg-transparent px-3 py-1.5 sm:px-5">
        <form className="mx-auto max-w-5xl" onSubmit={handleComposerSubmit}>
          <div className="relative rounded-xl border border-stone-300 bg-white">
            {selectedSkills.length ? (
              <div className="flex flex-wrap gap-1.5 px-3 pt-2">
                {selectedSkills.map((skill) => (
                  <span
                    className="inline-flex max-w-full items-center gap-1 rounded-full border border-teal-200 bg-teal-50 px-2 py-1 text-xs font-semibold text-teal-900"
                    key={skill.key}
                  >
                    <span className="max-w-44 truncate">{skill.name}</span>
                    <button
                      aria-label={t("Remove skill {name}", {
                        name: skill.name,
                      })}
                      className="inline-flex size-4 items-center justify-center rounded-full text-teal-800 hover:bg-teal-100"
                      onClick={() => onRemoveSkill(skill.key)}
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
              className="message-composer-textarea min-h-16 w-full resize-none border-0 bg-transparent px-3 py-1.5 text-sm leading-6 text-stone-900 outline-none placeholder:text-stone-400"
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
                if (isSendingMessage) {
                  return;
                }

                event.currentTarget.form?.requestSubmit();
              }}
              placeholder={t("Message Foco")}
              ref={messageTextareaRef}
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
                        disabled={!skill.enabled}
                        key={skill.key}
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
                          {skill.enabled ? skillScopeLabel(skill, t) : t("disabled")}
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
            <div
              className={`message-composer-actions flex flex-wrap items-center gap-2 border-t border-stone-100 px-2 py-2 ${
                canRetryRun ? "message-composer-actions-with-retry" : ""
              }`}
            >
              <ComposerSelectMenu
                ariaLabel={t("Model")}
                className="composer-model-select max-w-full"
                disabled={isLoadingSettings || isSendingMessage || !modelOptions.length}
                emptyLabel={t("No enabled models")}
                icon={Bot}
                onChange={onModelChange}
                options={modelOptions}
                selectedValue={selectedModelId}
              />
              <ComposerSelectMenu
                ariaLabel={t("Thinking")}
                className="composer-thinking-select max-w-full"
                disabled={isSendingMessage}
                emptyLabel={t("Model default")}
                icon={SlidersHorizontal}
                onChange={onThinkingLevelChange}
                options={thinkingOptions}
                selectedValue={selectedThinkingLevel}
              />
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
                className="composer-retry-button composer-run-button inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                onClick={onRetryRun}
                title={t("Retry last run")}
                type="button"
              >
                <RefreshCw aria-hidden="true" className="size-4" />
              </button>
            ) : null}
              <ContextUsageCircle
                isLoading={isLoadingContextUsage}
                className="ml-auto"
                usage={contextUsage}
              />
              {isSendingMessage ? (
                <button
                  aria-label={t("Cancel run")}
                  className="composer-run-button inline-flex size-8 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                  onClick={onCancelRun}
                  title={t("Cancel run")}
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              ) : (
                <button
                  aria-label={t("Send message")}
                  className="composer-run-button inline-flex size-8 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
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

function ContextUsageCircle({
  className = "",
  isLoading,
  usage,
}: {
  className?: string;
  isLoading: boolean;
  usage: ContextUsageResponse | null;
}) {
  const { t } = useI18n();
  const percent = usage?.usagePercent ?? null;
  const percentLabel = percent ?? 0;
  const boundedPercent =
    percent === null ? 0 : Math.max(0, Math.min(100, percent));
  const title =
    usage === null
      ? t("Context usage")
      : usage.willCompressOnNextSend
        ? `${t("Context usage {percent}%", { percent: percentLabel })}. ${t(
            "Context compression may run on the next send",
          )}`
        : t("Context usage {percent}%", { percent: percentLabel });
  const tone =
    usage?.willCompressOnNextSend ||
    (usage !== null && percent !== null && percent >= usage.compressionTriggerPercent)
      ? "context-usage-circle-critical"
      : percent !== null && percent >= 70
        ? "context-usage-circle-warn"
        : "context-usage-circle-normal";

  return (
    <div
      aria-label={title}
      className={`context-usage-circle ${tone} ${
        isLoading ? "context-usage-circle-loading" : ""
      } ${className}`}
      role="status"
      style={{
        "--context-usage-percent": `${boundedPercent}%`,
      } as CSSProperties}
      title={title}
    >
      {percent === null ? "--" : `${percent}%`}
    </div>
  );
}

type ComposerSelectOption = {
  label: string;
  value: string;
};

function ComposerSelectMenu({
  ariaLabel,
  className,
  disabled,
  emptyLabel,
  icon: Icon,
  onChange,
  options,
  selectedValue,
}: {
  ariaLabel: string;
  className: string;
  disabled: boolean;
  emptyLabel: string;
  icon: LucideIcon;
  onChange: (value: string) => void;
  options: ComposerSelectOption[];
  selectedValue: string;
}) {
  const selectedOption =
    options.find((option) => option.value === selectedValue) ?? null;
  const selectedLabel = selectedOption?.label ?? emptyLabel;
  const detailsRef = useCloseDetailsOnOutsidePointerDown();

  function handleSelect(value: string, event: ReactMouseEvent<HTMLButtonElement>) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onChange(value);
  }

  return (
    <details
      className={`composer-select-menu group relative ${className}`}
      ref={detailsRef}
    >
      <summary
        aria-disabled={disabled}
        aria-label={ariaLabel}
        className={`composer-select-summary flex h-8 w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${
          disabled ? "pointer-events-none text-stone-400" : "hover:border-stone-300"
        }`}
        title={selectedLabel}
      >
        <Icon aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="min-w-0 flex-1 truncate">{selectedLabel}</span>
        <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
      </summary>
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
        <div className="panel-scroll max-h-52 overflow-y-auto py-1">
          {options.length ? (
            options.map((option) => (
              <button
                aria-label={`${ariaLabel}: ${option.label}`}
                className={`flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-stone-50 ${
                  option.value === selectedValue
                    ? "font-semibold text-teal-900"
                    : "text-stone-700"
                }`}
                key={option.value}
                onClick={(event) => handleSelect(option.value, event)}
                type="button"
              >
                <Icon aria-hidden="true" className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{option.label}</span>
                {option.value === selectedValue ? (
                  <CheckCircle2 aria-hidden="true" className="size-3.5 shrink-0" />
                ) : null}
              </button>
            ))
          ) : (
            <div className="px-3 py-3 text-sm text-stone-500">{emptyLabel}</div>
          )}
        </div>
      </div>
    </details>
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
  const detailsRef = useCloseDetailsOnOutsidePointerDown();
  if (!isGitRepository) {
    return (
      <div
        aria-label={t("Git branch")}
        className="composer-branch-select inline-flex h-8 max-w-full items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-400"
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
    <details
      className="composer-branch-select group relative max-w-full"
      ref={detailsRef}
    >
      <summary
        className={`composer-select-summary flex h-8 w-full cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-stone-50/80 px-2 text-xs font-medium text-stone-900 outline-none transition marker:hidden focus-visible:ring-2 focus-visible:ring-teal-100 ${
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
      <div className="composer-select-popover absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-xl border border-stone-200 bg-white shadow-[0_20px_46px_rgba(33,31,28,0.16)]">
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

function useCloseDetailsOnOutsidePointerDown() {
  const detailsRef = useRef<HTMLDetailsElement | null>(null);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      const details = detailsRef.current;
      if (!details?.open) {
        return;
      }
      const target = event.target;
      if (!(target instanceof Node) || details.contains(target)) {
        return;
      }
      details.removeAttribute("open");
    }

    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  return detailsRef;
}

function ReasoningBlock({
  isStreaming,
  reasoning,
}: {
  isStreaming: boolean;
  reasoning: string;
}) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(isStreaming);
  const preview = compactInlineText(reasoning);

  useEffect(() => {
    setIsExpanded(isStreaming);
  }, [isStreaming]);

  const toggleLabel = isExpanded ? t("Collapse thinking") : t("Expand thinking");

  return (
    <div className="reasoning-block min-w-0 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
      <button
        aria-expanded={isExpanded}
        aria-label={toggleLabel}
        className="flex min-h-6 w-full min-w-0 items-center gap-2 text-left text-xs font-semibold text-stone-500 hover:text-stone-700"
        onClick={() => setIsExpanded((current) => !current)}
        title={toggleLabel}
        type="button"
      >
        {isExpanded ? (
          <ChevronDown aria-hidden="true" className="size-3.5 shrink-0" />
        ) : (
          <ChevronRight aria-hidden="true" className="size-3.5 shrink-0" />
        )}
        <span className="shrink-0">{t("Thinking")}</span>
        {isExpanded ? null : (
          <span
            className="min-w-0 flex-1 truncate font-normal text-stone-600"
            title={preview}
          >
            {preview}
          </span>
        )}
      </button>
      {isExpanded ? (
        <div className="mt-1.5">
          <MarkdownContent content={reasoning} isUser={false} variant="reasoning" />
        </div>
      ) : null}
    </div>
  );
}

function MessagePartBlock({
  isError,
  isStreaming,
  isUser,
  part,
}: {
  isError: boolean;
  isStreaming: boolean;
  isUser: boolean;
  part: ChatMessagePart;
}) {
  if (part.type === "reasoning") {
    return <ReasoningBlock isStreaming={isStreaming} reasoning={part.text} />;
  }

  if (part.type === "toolCall") {
    return <ToolCallBlock toolCall={part.toolCall} />;
  }

  return (
    <MarkdownContent content={part.text} isError={isError} isUser={isUser} />
  );
}

function ChatReplyMetricsLine({ metrics }: { metrics: ChatReplyMetrics }) {
  const { language, t } = useI18n();
  const values = [
    `${t("Model")}: ${metrics.modelId}`,
    `${t("Channel")}: ${metrics.providerId}`,
    `${t("Total time")}: ${formatNullableLatency(
      metrics.totalLatencyMs,
      language,
    )}`,
    `${t("tokens/s")}: ${formatTokensPerSecond(metrics, language)}`,
    `${t("First token latency")}: ${formatNullableLatency(
      metrics.firstTokenLatencyMs,
      language,
    )}`,
  ];

  return (
    <div className="flex flex-wrap gap-x-2 gap-y-1 border-t border-stone-100 pt-2 text-[11px] leading-4 text-stone-400">
      {values.map((value) => (
        <span className="min-w-0 break-words" key={value}>
          {value}
        </span>
      ))}
    </div>
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
  const markdownContent = deferIncompleteMermaidBlocks(
    skillPrefix?.remaining ?? content,
  );

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
        <ReactMarkdown components={MARKDOWN_COMPONENTS} remarkPlugins={[remarkGfm]}>
          {markdownContent}
        </ReactMarkdown>
      ) : null}
    </div>
  );
}

function MermaidDiagram({ definition }: { definition: string }) {
  const { t } = useI18n();
  const reactId = useId();
  const baseRenderId = `foco-mermaid-${reactId.replaceAll(":", "")}`;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const renderCounterRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState("");

  useEffect(() => {
    let cancelled = false;
    renderCounterRef.current += 1;
    const renderId = `${baseRenderId}-${renderCounterRef.current}`;

    async function renderDiagram() {
      setError(null);
      setSvg("");

      try {
        const mermaid = await loadMermaidRuntime();
        if (cancelled) {
          return;
        }
        const result = await mermaid.render(renderId, definition);
        if (cancelled) {
          return;
        }
        setSvg(result.svg);
        window.setTimeout(() => {
          if (!cancelled && containerRef.current) {
            result.bindFunctions?.(containerRef.current);
          }
        }, 0);
      } catch (renderError) {
        if (!cancelled) {
          setError(errorMessage(renderError));
        }
      }
    }

    void renderDiagram();

    return () => {
      cancelled = true;
    };
  }, [definition, baseRenderId]);

  if (error !== null) {
    return (
      <div className="mermaid-diagram mermaid-diagram-error">
        <div className="mermaid-diagram-error-title">
          {t("Mermaid diagram failed to render.")}
        </div>
        <div className="mermaid-diagram-error-message">{error}</div>
        <pre>
          <code>{definition}</code>
        </pre>
      </div>
    );
  }

  return (
    <div
      aria-label="Mermaid diagram"
      className={`mermaid-diagram ${svg ? "" : "mermaid-diagram-loading"}`}
      dangerouslySetInnerHTML={svg ? { __html: svg } : undefined}
      ref={containerRef}
      role="img"
    />
  );
}

async function loadMermaidRuntime() {
  mermaidRuntimePromise ??= import("mermaid").then((module) => {
    module.default.initialize(MERMAID_CONFIG);
    return module.default;
  });

  return mermaidRuntimePromise;
}

function mermaidDefinitionFromPreChildren(children: ReactNode) {
  const childNodes = Children.toArray(children);
  if (childNodes.length !== 1) {
    return null;
  }

  const child = childNodes[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(child)) {
    return null;
  }

  const className = child.props.className ?? "";
  if (!/\blanguage-mermaid\b/i.test(className)) {
    return null;
  }

  const definition = Children.toArray(child.props.children).join("").trim();
  return definition ? definition : null;
}

function deferIncompleteMermaidBlocks(content: string) {
  const lines = content.match(/[^\r\n]*(?:\r\n|\n|\r|$)/g) ?? [];
  const nonEmptyLines = lines.filter((line) => line.length > 0);
  if (nonEmptyLines.length === 0) {
    return content;
  }

  let activeFence: MarkdownFence | null = null;
  for (let index = 0; index < nonEmptyLines.length; index += 1) {
    const line = nonEmptyLines[index];

    if (activeFence !== null) {
      if (isFenceClosingLine(line, activeFence)) {
        activeFence = null;
      }
      continue;
    }

    const fence = parseMarkdownFence(line);
    if (fence !== null) {
      activeFence = {
        ...fence,
        lineIndex: index,
      };
    }
  }

  if (activeFence?.language !== "mermaid") {
    return content;
  }

  const nextLines = [...nonEmptyLines];
  nextLines[activeFence.lineIndex] = neutralizeMermaidFenceLine(
    nextLines[activeFence.lineIndex],
  );
  return nextLines.join("");
}

type MarkdownFence = {
  char: "`" | "~";
  length: number;
  language: string | null;
  lineIndex: number;
};

function parseMarkdownFence(line: string) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return null;
  }

  const marker = match[2];
  const language = match[3].trim().split(/\s+/, 1)[0]?.toLowerCase() || null;
  return {
    char: marker[0] as "`" | "~",
    length: marker.length,
    language,
    lineIndex: -1,
  };
}

function isFenceClosingLine(line: string, fence: MarkdownFence) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const escapedChar = fence.char === "`" ? "`" : "~";
  return new RegExp(`^[ \\t]{0,3}${escapedChar}{${fence.length},}[ \\t]*$`).test(
    body,
  );
}

function neutralizeMermaidFenceLine(line: string) {
  const lineEnding = line.match(/(?:\r\n|\n|\r)$/)?.[0] ?? "";
  const body = line.slice(0, line.length - lineEnding.length);
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return line;
  }

  return `${match[1]}${match[2]}text${lineEnding}`;
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

function TerminalPanel({
  onClose,
  workspace,
}: {
  onClose: () => void;
  workspace: WorkspaceSummary | undefined;
}) {
  const { t } = useI18n();
  const [panelHeight, setPanelHeight] = useState(256);
  const [isResizing, setIsResizing] = useState(false);
  const [activeClientId, setActiveClientId] = useState("");
  const [sessions, setSessions] = useState<TerminalPanelSession[]>(() => [
    createTerminalPanelSession(1),
  ]);
  const nextSessionNumberRef = useRef(2);
  const previousWorkspaceIdRef = useRef(workspace?.id ?? "");
  const workspaceId = workspace?.id ?? "";
  const workspacePath = workspace?.path ?? "";
  const activeSession =
    sessions.find((session) => session.clientId === activeClientId) ??
    sessions[0] ??
    null;

  useEffect(() => {
    if (previousWorkspaceIdRef.current === workspaceId) {
      return;
    }

    previousWorkspaceIdRef.current = workspaceId;
    const initialSession = createTerminalPanelSession(1);
    nextSessionNumberRef.current = 2;
    setSessions([initialSession]);
    setActiveClientId(initialSession.clientId);
  }, [workspaceId]);

  useEffect(() => {
    if (activeClientId || sessions.length === 0) {
      return;
    }

    setActiveClientId(sessions[0].clientId);
  }, [activeClientId, sessions]);

  useEffect(() => {
    if (!isResizing) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const nextHeight = window.innerHeight - event.clientY;
      setPanelHeight(Math.min(Math.max(nextHeight, 180), 520));
    }

    function handlePointerUp() {
      setIsResizing(false);
    }

    document.body.style.cursor = "row-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizing]);

  const updateSession = useCallback(
    (clientId: string, patch: Partial<Omit<TerminalPanelSession, "clientId">>) => {
      setSessions((current) =>
        current.map((session) =>
          session.clientId === clientId ? { ...session, ...patch } : session,
        ),
      );
    },
    [],
  );

  const markSessionClosed = useCallback((clientId: string) => {
    setSessions((current) =>
      current.map((session) =>
        session.clientId === clientId
          ? {
              ...session,
              status: session.status === "error" ? "error" : "closed",
            }
          : session,
      ),
    );
  }, []);

  function createSession() {
    const session = createTerminalPanelSession(nextSessionNumberRef.current);
    nextSessionNumberRef.current += 1;
    setSessions((current) => [...current, session]);
    setActiveClientId(session.clientId);
  }

  function closeSession(clientId: string) {
    if (sessions.length <= 1) {
      return;
    }

    const next = sessions.filter((session) => session.clientId !== clientId);
    setSessions(next);
    if (clientId === activeClientId) {
      setActiveClientId(next[0]?.clientId ?? "");
    }
  }

  return (
    <section
      className="terminal-panel relative shrink-0 border-t border-stone-800 bg-[#16130f]"
      style={{ "--terminal-panel-height": `${panelHeight}px` } as CSSProperties}
    >
      <div
        aria-label={t("Resize terminal panel")}
        aria-orientation="horizontal"
        className="absolute left-0 right-0 top-0 z-10 h-1 cursor-row-resize bg-transparent hover:bg-teal-500/50"
        onKeyDown={(event) => {
          if (event.key === "ArrowUp") {
            event.preventDefault();
            setPanelHeight((current) => Math.min(current + 24, 520));
          }

          if (event.key === "ArrowDown") {
            event.preventDefault();
            setPanelHeight((current) => Math.max(current - 24, 180));
          }
        }}
        onPointerDown={(event) => {
          event.preventDefault();
          setIsResizing(true);
        }}
        role="separator"
        tabIndex={0}
      />
      <div className="terminal-panel-body mx-auto flex h-[var(--terminal-panel-height)] w-full max-w-5xl min-w-0">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-8 items-center justify-between gap-3 px-3 text-xs text-stone-400">
            <span className="inline-flex min-w-0 items-center gap-2">
              <Terminal aria-hidden="true" className="size-4 shrink-0" />
              <span className={terminalStatusClass(activeSession?.status ?? "closed")}>
                {terminalStatusText(activeSession?.status ?? "closed", t)}
              </span>
              <span className="min-w-0 truncate">
                {activeSession?.cwd || workspacePath}
              </span>
            </span>
            <span className="flex min-w-0 shrink-0 items-center gap-2">
              {activeSession?.error ? (
                <span className="min-w-0 truncate text-rose-300">
                  {activeSession.error}
                </span>
              ) : null}
              <button
                aria-label={t("New terminal")}
                className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-stone-800 hover:text-stone-100"
                onClick={createSession}
                title={t("New terminal")}
                type="button"
              >
                <Plus aria-hidden="true" className="size-3.5" />
              </button>
              <button
                aria-label={t("Close terminal")}
                className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-rose-950/60 hover:text-rose-200"
                onClick={onClose}
                title={t("Close terminal")}
                type="button"
              >
                <X aria-hidden="true" className="size-3.5" />
              </button>
            </span>
          </div>
          <div className="relative min-h-0 flex-1">
            {sessions.map((session) => (
              <TerminalSessionPane
                isActive={session.clientId === activeSession?.clientId}
                key={session.clientId}
                markClosed={markSessionClosed}
                onUpdate={updateSession}
                session={session}
                workspaceId={workspaceId}
              />
            ))}
          </div>
        </div>
        {sessions.length > 1 ? (
          <aside
            aria-label={t("Terminal sessions")}
            className="terminal-session-list panel-scroll w-44 shrink-0 overflow-y-auto border-l border-stone-800 bg-stone-950/35 px-2 py-2"
          >
            {sessions.map((session) => (
              <div
                className={`flex w-full min-w-0 items-center gap-1 rounded-md text-xs ${
                  session.clientId === activeSession?.clientId
                    ? "bg-stone-800 text-stone-100"
                    : "text-stone-400 hover:bg-stone-900 hover:text-stone-100"
                }`}
                key={session.clientId}
              >
                <button
                  className="flex min-w-0 flex-1 items-center gap-2 px-2 py-2 text-left"
                  onClick={() => setActiveClientId(session.clientId)}
                  type="button"
                >
                  <span
                    aria-label={terminalStatusText(session.status, t)}
                    className={`size-2 shrink-0 rounded-full ${
                      isTerminalConnected(session.status)
                        ? "bg-emerald-400"
                        : "bg-rose-500"
                    }`}
                    title={terminalStatusText(session.status, t)}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-semibold">
                      {t("Terminal {number}", { number: session.number })}
                    </span>
                    <span
                      className="block truncate text-[11px] opacity-60"
                      title={session.cwd || workspacePath}
                    >
                      {session.cwd || workspacePath}
                    </span>
                  </span>
                </button>
                <button
                  aria-label={t("Close terminal {number}", {
                    number: session.number,
                  })}
                  className="mr-1 inline-flex size-6 shrink-0 items-center justify-center rounded-md text-stone-500 hover:bg-rose-950/60 hover:text-rose-200"
                  onClick={() => closeSession(session.clientId)}
                  title={t("Close terminal {number}", { number: session.number })}
                  type="button"
                >
                  <X aria-hidden="true" className="size-3.5" />
                </button>
              </div>
            ))}
          </aside>
        ) : null}
      </div>
    </section>
  );
}

function TerminalSessionPane({
  isActive,
  markClosed,
  onUpdate,
  session,
  workspaceId,
}: {
  isActive: boolean;
  markClosed: (clientId: string) => void;
  onUpdate: (
    clientId: string,
    patch: Partial<Omit<TerminalPanelSession, "clientId">>,
  ) => void;
  session: TerminalPanelSession;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const tRef = useRef(t);
  const isActiveRef = useRef(isActive);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const { clientId } = session;

  useEffect(() => {
    tRef.current = t;
  }, [t]);

  useEffect(() => {
    isActiveRef.current = isActive;
    if (isActive) {
      xtermRef.current?.focus();
    }
  }, [isActive]);

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
    onUpdate(clientId, { error: null, status: "connecting" });

    if (!containerRef.current) {
      onUpdate(clientId, {
        error: tRef.current("Terminal container was not mounted."),
        status: "error",
      });
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
        const serverSession = await requestJson<TerminalSessionResponse>(
          `/api/workspaces/${encodeURIComponent(workspaceId)}/terminal/session`,
          { method: "POST" },
        );
        if (cancelled) {
          return;
        }

        onUpdate(clientId, {
          cwd: serverSession.workingDirectory,
          serverSessionId: serverSession.id,
        });
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        socket = new WebSocket(
          `${protocol}//${window.location.host}/api/workspaces/${encodeURIComponent(
            workspaceId,
          )}/terminal/${encodeURIComponent(serverSession.id)}/ws?cols=${terminal.cols}&rows=${terminal.rows}`,
        );
        socketRef.current = socket;

        socket.onopen = () => {
          onUpdate(clientId, { status: "connected" });
          sendResize();
          if (isActiveRef.current) {
            terminal.focus();
          }
        };
        socket.onmessage = (event) => {
          const parsed = JSON.parse(event.data as string) as unknown;
          if (!isTerminalServerEvent(parsed)) {
            onUpdate(clientId, {
              error: tRef.current("Terminal returned an unknown event."),
              status: "error",
            });
            return;
          }

          if (parsed.type === "started" || parsed.type === "cwd") {
            onUpdate(clientId, { cwd: parsed.cwd });
            return;
          }

          if (parsed.type === "output") {
            terminal.write(parsed.data);
            return;
          }

          if (parsed.type === "exit") {
            onUpdate(clientId, { status: "closed" });
            terminal.writeln(
              `\r\n[${tRef.current("terminal exited: {status}", {
                status: parsed.status,
              })}]`,
            );
            return;
          }

          onUpdate(clientId, { error: parsed.message, status: "error" });
          terminal.writeln(
            `\r\n[${tRef.current("terminal error: {message}", {
              message: parsed.message,
            })}]`,
          );
        };
        socket.onerror = () => {
          onUpdate(clientId, {
            error: tRef.current("Terminal WebSocket failed."),
            status: "error",
          });
        };
        socket.onclose = () => {
          markClosed(clientId);
        };
      } catch (requestError) {
        if (!cancelled) {
          const message = errorMessage(requestError);
          onUpdate(clientId, { error: message, status: "error" });
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
  }, [clientId, markClosed, onUpdate, workspaceId]);

  return (
    <div
      aria-hidden={!isActive}
      className={`terminal-session-pane absolute inset-0 min-h-0 min-w-0 p-2 ${
        isActive ? "" : "pointer-events-none opacity-0"
      }`}
    >
      <div ref={containerRef} className="terminal-xterm h-full min-w-0" />
    </div>
  );
}

function createTerminalPanelSession(number: number): TerminalPanelSession {
  return {
    clientId: `${Date.now().toString(36)}-${number}-${Math.random()
      .toString(36)
      .slice(2)}`,
    cwd: "",
    error: null,
    number,
    serverSessionId: null,
    status: "closed",
  };
}

function ApiStatsPanel({
  settings,
  workspaces,
}: {
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const [filters, setFilters] = useState<AiStatsFilterState>(
    emptyAiStatsFilters,
  );
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [detail, setDetail] = useState<AiRequestDetailResponse | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [visibleColumnIds, setVisibleColumnIds] = useState<
    Set<AiStatsColumnId>
  >(() => new Set(DEFAULT_AI_STATS_COLUMN_IDS));
  const requests = stats?.requests ?? [];
  const totalCount = stats?.totalCount ?? requests.length;
  const currentPage = stats?.page ?? positiveIntegerText(filters.page, 1);
  const pageSize = stats?.pageSize ?? positiveIntegerText(filters.pageSize, 50);
  const totalPages =
    stats?.totalPages ?? (totalCount ? Math.ceil(totalCount / pageSize) : 0);
  const paginationItems = auditPaginationItems(currentPage, totalPages);
  const pageStart = requests.length ? (currentPage - 1) * pageSize + 1 : 0;
  const pageEnd = requests.length
    ? Math.min(totalCount, pageStart + requests.length - 1)
    : 0;
  const chatCount = workspaces.reduce(
    (total, workspace) => total + workspace.chats.length,
    0,
  );
  const selectedWorkspace =
    workspaces.find((workspace) => workspace.id === filters.workspaceId) ?? null;
  const chatOptions = (selectedWorkspace ? [selectedWorkspace] : workspaces)
    .flatMap((workspace) =>
      workspace.chats.map((chat) => ({
        label: selectedWorkspace
          ? chat.title
          : `${workspace.name} / ${chat.title}`,
        value: chat.id,
      })),
    );
  const providerOptions = auditOptions(
    settings?.providers.map((provider) => ({
      label: provider.name,
      value: provider.id,
    })) ?? [],
    requests.map((request) => request.providerId),
  );
  const modelOptions = auditOptions(
    settings?.configuredModels.map((model) => ({
      label: model.displayName,
      value: model.id,
    })) ?? [],
    requests.map((request) => request.modelId),
  );
  const statusOptions = auditOptions(
    ["succeeded", "failed", "completed"].map((status) => ({
      label: auditStatusText(status, t),
      value: status,
    })),
    requests.map((request) => request.finalState),
    (status) => auditStatusText(status, t),
  );
  const totalInputTokens = requests.reduce(
    (total, request) => total + (request.inputTokens ?? 0),
    0,
  );
  const totalOutputTokens = requests.reduce(
    (total, request) => total + (request.outputTokens ?? 0),
    0,
  );
  const latencyValues = requests
    .map((request) => request.totalLatencyMs)
    .filter((value): value is number => value !== null);
  const averageLatency = latencyValues.length
    ? Math.round(
        latencyValues.reduce((total, value) => total + value, 0) /
          latencyValues.length,
      )
    : null;
  const aiStatsColumns: AiStatsColumn[] = [
    {
      cellClassName: "px-4 py-3 font-medium text-stone-900",
      id: "requestTime",
      label: t("Request time"),
      render: (request) =>
        formatAuditDate(request.requestStartedAt, language),
    },
    {
      cellClassName: "max-w-[10rem] truncate px-4 py-3 text-stone-700",
      id: "workspace",
      label: t("Workspace"),
      render: (request) => request.workspaceName,
    },
    {
      cellClassName: "max-w-[12rem] truncate px-4 py-3 text-stone-600",
      id: "chat",
      label: t("Chat"),
      render: (request) => request.chatTitle ?? request.chatId ?? "n/a",
    },
    {
      cellClassName:
        "max-w-[12rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "provider",
      label: t("Provider"),
      render: (request) => request.providerId,
    },
    {
      cellClassName:
        "max-w-[14rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "model",
      label: t("Model"),
      render: (request) => request.modelId,
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "inputTokens",
      label: t("Input tokens"),
      render: (request) => formatNullableNumber(request.inputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "outputTokens",
      label: t("Output tokens"),
      render: (request) => formatNullableNumber(request.outputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRead",
      label: t("Cache read"),
      render: (request) =>
        formatNullableNumber(request.cacheReadTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheWrite",
      label: t("Cache write"),
      render: (request) =>
        formatNullableNumber(request.cacheWriteTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRatio",
      label: t("Cache ratio"),
      render: (request) => formatPercent(request.cacheRatio, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "latency",
      label: t("Latency"),
      render: (request) =>
        formatNullableLatency(request.totalLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "firstToken",
      label: t("First token"),
      render: (request) =>
        formatNullableLatency(request.firstTokenLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "statusCode",
      label: t("Status code"),
      render: (request) => formatNullableNumber(request.statusCode, language),
    },
    {
      cellClassName: "px-4 py-3",
      id: "status",
      label: t("Status"),
      render: (request) => (
        <span className={auditStatusClass(request.finalState)}>
          {auditStatusText(request.finalState, t)}
        </span>
      ),
    },
    {
      cellClassName: "px-4 py-3 text-right",
      headerClassName: "text-right",
      id: "details",
      label: t("Details"),
      render: (request) => (
        <button
          aria-label={t("View request details")}
          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
          onClick={() => void openRequestDetail(request)}
          title={t("View request details")}
          type="button"
        >
          <Eye aria-hidden="true" className="size-4" />
        </button>
      ),
    },
  ];
  const visibleColumns = aiStatsColumns.filter((column) =>
    visibleColumnIds.has(column.id),
  );

  function updateAuditFilters(update: Partial<AiStatsFilterState>) {
    setFilters((current) => ({
      ...current,
      ...update,
      page: "1",
    }));
  }

  function goToAuditPage(page: number) {
    const maxPage = Math.max(1, totalPages);
    setFilters((current) => ({
      ...current,
      page: String(Math.min(maxPage, Math.max(1, page))),
    }));
  }

  function toggleAiStatsColumn(columnId: AiStatsColumnId) {
    setVisibleColumnIds((current) => {
      if (current.has(columnId) && current.size === 1) {
        return current;
      }

      const next = new Set(current);
      if (next.has(columnId)) {
        next.delete(columnId);
      } else {
        next.add(columnId);
      }

      return next;
    });
  }

  const loadStats = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const query = aiStatsQuery(filters);
      const data = await requestJson<AiStatisticsResponse>(
        `/api/ai-statistics${query ? `?${query}` : ""}`,
      );
      setStats(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void loadStats();
  }, [loadStats]);

  async function openRequestDetail(request: AiRequestAuditSummary) {
    setSelectedRequestId(request.id);
    setDetail(null);
    setDetailError(null);
    setCopiedKey(null);
    setIsLoadingDetail(true);

    try {
      const data = await requestJson<AiRequestDetailResponse>(
        `/api/workspaces/${encodeURIComponent(
          request.workspaceId,
        )}/ai-statistics/${encodeURIComponent(request.id)}`,
      );
      setDetail(data);
    } catch (requestError) {
      setDetailError(errorMessage(requestError));
    } finally {
      setIsLoadingDetail(false);
    }
  }

  async function copyAuditText(key: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      window.setTimeout(() => {
        setCopiedKey((current) => (current === key ? null : current));
      }, 1600);
    } catch (copyError) {
      setDetailError(errorMessage(copyError));
    }
  }

  return (
    <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="flex w-full min-w-0 flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/80 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className="inline-flex size-10 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
                <BarChart3 aria-hidden="true" className="size-5" />
              </span>
              <div className="min-w-0">
                <h2 className="truncate text-lg font-semibold text-stone-950">
                  {t("API statistics")}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {filters.workspaceId
                    ? selectedWorkspace?.name ?? filters.workspaceId
                    : t("All workspaces")}
                </p>
              </div>
            </div>
            <button
              aria-label={t("Refresh request audit")}
              className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
              disabled={isLoading}
              onClick={() => void loadStats()}
              title={t("Refresh request audit")}
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

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <StatsCard
            icon={Activity}
            label={t("Recorded requests")}
            value={formatNumber(totalCount, language)}
          />
          <StatsCard
            icon={MessageSquare}
            label={t("All chats")}
            value={formatNumber(chatCount, language)}
          />
          <StatsCard
            icon={Bot}
            label={t("Input tokens")}
            value={formatNumber(totalInputTokens, language)}
          />
          <StatsCard
            icon={SlidersHorizontal}
            label={t("Average latency")}
            value={formatNullableLatency(averageLatency, language)}
          />
        </section>

        <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
            <div>
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Request audit")}
              </h3>
              <p className="mt-1 text-xs text-stone-500">
                {t("requests {count}", {
                  count: formatNumber(totalCount, language),
                })}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <div className="text-xs text-stone-500">
                {t("Output tokens")}: {formatNumber(totalOutputTokens, language)}
              </div>
              <details className="relative">
                <summary className="inline-flex h-9 cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 [&::-webkit-details-marker]:hidden">
                  <SlidersHorizontal aria-hidden="true" className="size-4" />
                  {t("Columns")}
                </summary>
                <div className="absolute right-0 z-20 mt-2 w-56 rounded-xl border border-stone-200 bg-white p-2 shadow-[0_18px_42px_rgba(75,63,42,0.16)]">
                  {aiStatsColumns.map((column) => (
                    <label
                      className="flex min-h-9 cursor-pointer items-center gap-2 rounded-lg px-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                      key={column.id}
                    >
                      <input
                        checked={visibleColumnIds.has(column.id)}
                        className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                        disabled={
                          visibleColumnIds.has(column.id) &&
                          visibleColumnIds.size === 1
                        }
                        onChange={() => toggleAiStatsColumn(column.id)}
                        type="checkbox"
                      />
                      <span className="min-w-0 truncate">{column.label}</span>
                    </label>
                  ))}
                </div>
              </details>
            </div>
          </div>
          <div className="grid gap-3 border-b border-stone-200 bg-stone-50/70 px-4 py-4 md:grid-cols-2 xl:grid-cols-7">
            <FilterSelect
              label={t("Workspace")}
              onChange={(value) =>
                updateAuditFilters({
                  chatId: "",
                  workspaceId: value,
                })
              }
              options={workspaces.map((workspace) => ({
                label: workspace.name,
                value: workspace.id,
              }))}
              placeholder={t("All workspaces")}
              value={filters.workspaceId}
            />
            <FilterSelect
              label={t("Chat")}
              onChange={(value) => updateAuditFilters({ chatId: value })}
              options={chatOptions}
              placeholder={t("All chats")}
              value={filters.chatId}
            />
            <FilterSelect
              label={t("Provider")}
              onChange={(value) => updateAuditFilters({ providerId: value })}
              options={providerOptions}
              placeholder={t("All providers")}
              value={filters.providerId}
            />
            <FilterSelect
              label={t("Model")}
              onChange={(value) => updateAuditFilters({ modelId: value })}
              options={modelOptions}
              placeholder={t("All models")}
              value={filters.modelId}
            />
            <FilterSelect
              label={t("Status")}
              onChange={(value) => updateAuditFilters({ status: value })}
              options={statusOptions}
              placeholder={t("All statuses")}
              value={filters.status}
            />
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started after")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateAuditFilters({
                    startedAfter: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedAfter}
              />
            </label>
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started before")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateAuditFilters({
                    startedBefore: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedBefore}
              />
            </label>
          </div>
          {error ? (
            <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div className="overflow-x-auto">
            <table className="w-full min-w-max text-left text-sm">
              <thead className="border-b border-stone-200 bg-white text-xs font-semibold text-stone-500">
                <tr>
                  {visibleColumns.map((column) => (
                    <th
                      className={`whitespace-nowrap px-4 py-3 ${
                        column.headerClassName ?? ""
                      }`}
                      key={column.id}
                    >
                      {column.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {requests.length ? (
                  requests.map((request) => (
                    <tr key={request.id} className="align-top hover:bg-teal-50/40">
                      {visibleColumns.map((column) => (
                        <td
                          className={`whitespace-nowrap ${column.cellClassName}`}
                          key={column.id}
                        >
                          {column.render(request)}
                        </td>
                      ))}
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td
                      className="px-4 py-10 text-center text-sm text-stone-500"
                      colSpan={visibleColumns.length}
                    >
                      {isLoading ? t("Loading...") : t("No recorded requests")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 px-4 py-3 text-sm">
            <div className="text-stone-500">
              {t("Showing {start}-{end} of {total}", {
                end: formatNumber(pageEnd, language),
                start: formatNumber(pageStart, language),
                total: formatNumber(totalCount, language),
              })}
            </div>
            <div className="flex flex-wrap items-center justify-end gap-3">
              <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                <span>{t("Page size")}</span>
                <input
                  className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  max={500}
                  min={1}
                  onChange={(event) =>
                    updateAuditFilters({ pageSize: event.target.value })
                  }
                  type="number"
                  value={filters.pageSize}
                />
              </label>
              <nav
                aria-label={t("Request audit pagination")}
                className="flex items-center gap-1"
              >
              <button
                aria-label={t("Previous page")}
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                disabled={isLoading || currentPage <= 1}
                onClick={() => goToAuditPage(currentPage - 1)}
                title={t("Previous page")}
                type="button"
              >
                <ChevronLeft aria-hidden="true" className="size-4" />
              </button>
              {paginationItems.map((item, index) =>
                item === "ellipsis" ? (
                  <span
                    aria-hidden="true"
                    className="inline-flex size-9 items-center justify-center text-stone-400"
                    key={`ellipsis-${index}`}
                  >
                    ...
                  </span>
                ) : (
                  <button
                    aria-current={item === currentPage ? "page" : undefined}
                    aria-label={t("Go to page {page}", {
                      page: formatNumber(item, language),
                    })}
                    className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${
                      item === currentPage
                        ? "border-teal-700 bg-teal-700 text-white"
                        : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    }`}
                    disabled={isLoading}
                    key={item}
                    onClick={() => goToAuditPage(item)}
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
                  isLoading || totalPages === 0 || currentPage >= totalPages
                }
                onClick={() => goToAuditPage(currentPage + 1)}
                title={t("Next page")}
                type="button"
              >
                <ChevronRight aria-hidden="true" className="size-4" />
              </button>
              </nav>
            </div>
          </div>
        </section>
      </div>
      {selectedRequestId ? (
        <AiRequestDetailDialog
          copiedKey={copiedKey}
          detail={detail}
          error={detailError}
          isLoading={isLoadingDetail}
          onClose={() => {
            setSelectedRequestId(null);
            setDetail(null);
            setDetailError(null);
            setCopiedKey(null);
          }}
          onCopy={(key, text) => void copyAuditText(key, text)}
        />
      ) : null}
    </div>
  );
}

function FilterSelect({
  label,
  onChange,
  options,
  placeholder,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: { label: string; value: string }[];
  placeholder: string;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <select
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        <option value="">{placeholder}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function AiRequestDetailDialog({
  copiedKey,
  detail,
  error,
  isLoading,
  onClose,
  onCopy,
}: {
  copiedKey: string | null;
  detail: AiRequestDetailResponse | null;
  error: string | null;
  isLoading: boolean;
  onClose: () => void;
  onCopy: (key: string, text: string) => void;
}) {
  const { language, t } = useI18n();
  const request = detail?.request ?? null;

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="ai-request-detail-title"
        aria-modal="true"
        className="flex max-h-[90vh] w-full max-w-6xl flex-col overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="min-w-0">
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="ai-request-detail-title"
            >
              {t("Request details")}
            </h2>
            <p className="mt-1 truncate text-xs font-medium text-stone-500">
              {request ? request.id : t("Loading...")}
            </p>
          </div>
          <button
            aria-label={t("Close request details")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          {error ? (
            <div className="mb-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          {isLoading ? (
            <div className="flex items-center gap-2 py-8 text-sm text-stone-500">
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              {t("Loading...")}
            </div>
          ) : null}
          {request ? (
            <div className="grid gap-4">
              <div className="grid gap-3 rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3 md:grid-cols-2 xl:grid-cols-4">
                <AuditMeta label={t("Workspace")} value={request.workspaceName} />
                <AuditMeta
                  label={t("Chat")}
                  value={request.chatTitle ?? request.chatId ?? "n/a"}
                />
                <AuditMeta label={t("Provider")} value={request.providerId} />
                <AuditMeta label={t("Model")} value={request.modelId} />
                <AuditMeta
                  label={t("Request time")}
                  value={formatAuditDate(request.requestStartedAt, language)}
                />
                <AuditMeta
                  label={t("Latency")}
                  value={formatNullableLatency(request.totalLatencyMs, language)}
                />
                <AuditMeta
                  label={t("First token")}
                  value={formatNullableLatency(
                    request.firstTokenLatencyMs,
                    language,
                  )}
                />
                <AuditMeta
                  label={t("Status")}
                  value={auditStatusText(request.finalState, t)}
                />
              </div>
              <div className="grid gap-4 xl:grid-cols-2">
                <AuditJsonBlock
                  copied={copiedKey === "request"}
                  label={t("Request body")}
                  onCopy={() =>
                    onCopy("request", auditJsonText(request.requestBody))
                  }
                  value={request.requestBody}
                />
                <AuditJsonBlock
                  copied={copiedKey === "response"}
                  label={t("Response body")}
                  onCopy={() =>
                    onCopy("response", auditJsonText(request.responseBody))
                  }
                  value={request.responseBody}
                />
              </div>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function AuditMeta({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs font-semibold text-stone-500">{label}</div>
      <div className="mt-1 truncate text-sm font-medium text-stone-950">
        {value}
      </div>
    </div>
  );
}

function AuditJsonBlock({
  copied,
  label,
  onCopy,
  value,
}: {
  copied: boolean;
  label: string;
  onCopy: () => void;
  value: JsonValue | null;
}) {
  const { t } = useI18n();

  return (
    <section className="min-w-0 rounded-xl border border-stone-200 bg-white">
      <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-3 py-2">
        <h3 className="text-sm font-semibold text-stone-950">{label}</h3>
        <button
          aria-label={t("Copy {label}", { label })}
          className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-stone-200 bg-white px-2 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
          onClick={onCopy}
          title={t("Copy {label}", { label })}
          type="button"
        >
          <Copy aria-hidden="true" className="size-3.5" />
          {copied ? t("Copied") : t("Copy")}
        </button>
      </div>
      <pre className="max-h-[32rem] overflow-auto whitespace-pre-wrap break-words bg-white px-3 py-3 text-xs leading-5 text-stone-950">
        {auditJsonText(value)}
      </pre>
    </section>
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

function TaskGraphPanel({
  error,
  isLoading,
  taskGraph,
}: {
  error: string | null;
  isLoading: boolean;
  taskGraph: TaskGraphResponse;
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
            <h2 className="truncate text-sm font-semibold">
              {t("Task graph")}
            </h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {taskGraph.updatedAt
                ? t("Updated {time}", {
                    time: formatTaskGraphDate(taskGraph.updatedAt, language),
                  })
                : taskGraph.chatId}
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
          {taskGraph.tasks.map((task) => (
            <TaskGraphTaskItem key={task.id} level={0} task={task} />
          ))}
        </div>
      </div>
    </div>
  );
}

function TaskGraphTaskItem({
  level,
  task,
}: {
  level: number;
  task: TaskGraphTask;
}) {
  const { t } = useI18n();

  return (
    <div>
      <div
        className="rounded-lg border border-stone-200 bg-white px-3 py-2 shadow-sm"
        style={{ marginLeft: level ? Math.min(level * 14, 42) : 0 }}
      >
        <div className="flex min-w-0 items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] font-semibold text-stone-500">
                {task.id}
              </span>
              <span className={taskStatusClass(task.status)}>
                {t(task.status)}
              </span>
            </div>
            <h3 className="mt-1 break-words text-sm font-semibold leading-snug text-stone-950">
              {task.title}
            </h3>
          </div>
        </div>
        {task.summary ? (
          <p className="mt-2 break-words text-xs leading-5 text-stone-600">
            {task.summary}
          </p>
        ) : null}
        {task.dependsOn.length ? (
          <div className="mt-2 flex flex-wrap gap-1.5">
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
      {task.subtasks.length ? (
        <div className="mt-2 space-y-2">
          {task.subtasks.map((subtask) => (
            <TaskGraphTaskItem
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

function GitDiffPanel({
  diffError,
  diffText,
  files,
  isLoading,
  onClose,
  onRefresh,
  onSelectFile,
  selectedPath,
}: {
  diffError: string | null;
  diffText: string;
  files: GitStatusFileSummary[];
  isLoading: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onSelectFile: (path: string | null) => void;
  selectedPath: string | null;
}) {
  const { t } = useI18n();

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col">
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
  canLogout,
  onAddWorkspace,
  onLogout,
  onSettingsChange,
  onWorkspacesChange,
  workspaceDialogRevision,
}: {
  canLogout: boolean;
  onAddWorkspace: () => void;
  onLogout: () => Promise<void>;
  onSettingsChange: (settings: SettingsResponse) => void;
  onWorkspacesChange: () => Promise<void>;
  workspaceDialogRevision: number;
}) {
  const { t } = useI18n();
  const [activeSection, setActiveSection] =
    useState<SettingsSection>("general");
  const [isWorkspaceDialogOpen, setIsWorkspaceDialogOpen] = useState(false);
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
  const [workspaceForm, setWorkspaceForm] = useState<WorkspaceFormState>(() =>
    emptyWorkspaceForm(),
  );
  const [mcpForm, setMcpForm] = useState<McpServerFormState>(() =>
    emptyMcpServerForm(),
  );
  const [enabledSkillIds, setEnabledSkillIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSavingGeneral, setIsSavingGeneral] = useState(false);
  const [isClearingPassword, setIsClearingPassword] = useState(false);
  const [isSavingLanguage, setIsSavingLanguage] = useState(false);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [isSavingWorkspaceOrder, setIsSavingWorkspaceOrder] = useState(false);
  const [isSelectingWorkspaceFormPath, setIsSelectingWorkspaceFormPath] =
    useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
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
  const [providerTests, setProviderTests] = useState<
    Record<string, ProviderTestState>
  >({});
  const [error, setError] = useState<string | null>(null);
  const [isGeneralPasswordVisible, setIsGeneralPasswordVisible] = useState(false);
  const [isEditingGeneralPassword, setIsEditingGeneralPassword] = useState(false);

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
  const workspaces = settings?.workspaces ?? [];
  const terminalShells = settings?.terminalShells ?? [];
  const mcpTransports = settings?.mcpTransports ?? [];
  const mcpServers = settings?.mcpServers ?? [];
  const skills = settings?.skills;
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const configuredModels =
    settings?.configuredModels ?? metadata?.configuredModels ?? [];
  const passwordInputValue =
    generalForm.password ||
    (settings?.general.webServer.passwordEnabled && !isEditingGeneralPassword
      ? SAVED_PASSWORD_MASK
      : "");
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
  const editingWorkspace =
    workspaces.find((workspace) => workspace.id === workspaceForm.id) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const selectedProviderIds = new Set(form.providerIds);

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
      language: data.general.language,
      listenHost: data.general.webServer.listenHost,
      listenPort: String(data.general.webServer.listenPort),
      password: "",
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
      setDraggedWorkspaceId(null);
      setWorkspaceOrderPreview(null);
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

  useEffect(() => {
    if (workspaceDialogRevision > 0) {
      void loadSettings();
    }
  }, [loadSettings, workspaceDialogRevision]);

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

  function editConfiguredWorkspace(workspace: ConfiguredWorkspaceSummary) {
    setWorkspaceForm({
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
      pinned: workspace.pinned,
      terminalShell: workspace.terminalShell,
    });
    setIsWorkspaceDialogOpen(true);
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
      const password = generalForm.password;
      const data = await requestJson<SettingsResponse>("/api/settings/general", {
        body: JSON.stringify({
          clearPassword: false,
          listenHost: generalForm.listenHost,
          listenPort: optionalPositiveInteger(
            generalForm.listenPort,
            t("Listen port"),
          ),
          language: generalForm.language,
          password: password.trim() ? password : null,
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
          clearPassword: false,
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language,
          password: null,
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
          listenHost: settings.general.webServer.listenHost,
          listenPort: settings.general.webServer.listenPort,
          language: settings.general.language,
          password: null,
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
      setIsWorkspaceDialogOpen(false);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingWorkspace(false);
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
    setIsSelectingWorkspaceFormPath(true);
    setError(null);

    try {
      const data = await requestJson<{ path: string | null }>(
        "/api/native/select-directory",
        { method: "POST" },
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
    <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="settings-layout mx-auto grid max-w-7xl gap-4 lg:grid-cols-[13rem_minmax(0,1fr)]">
        <aside className="settings-section-nav-card rounded-2xl border border-stone-200 bg-white/85 p-2 shadow-[0_18px_42px_rgba(75,63,42,0.07)] lg:self-start">
          <nav
            aria-label={t("Settings")}
            className="settings-section-nav flex flex-col gap-1.5"
          >
          <SettingsNavButton
            active={activeSection === "general"}
            icon={Globe}
            label={t("General")}
            onClick={() => setActiveSection("general")}
          />
          <SettingsNavButton
            active={activeSection === "workspaces"}
            icon={Folder}
            label={t("Workspaces")}
            onClick={() => setActiveSection("workspaces")}
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

        {activeSection === "workspaces" ? (
        <section className="grid gap-4">
          {isWorkspaceDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label={t("Workspace configuration")}
                className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
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
                        disabled={isSelectingWorkspaceFormPath}
                        onClick={() => void selectWorkspaceFormPath()}
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
                      </button>
                    </div>
                  </label>
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
                  <button
                    aria-label={t("Save workspace")}
                    className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={
                      isSavingWorkspace ||
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
                    className={`grid gap-3 px-4 py-3 transition md:grid-cols-[auto_minmax(0,1fr)_auto] ${
                      draggedWorkspaceId === workspace.id
                        ? "bg-teal-50/70 opacity-80"
                        : "bg-white/0"
                    }`}
                    key={workspace.id}
                    onDragOver={(event) =>
                      handleWorkspaceDragOver(event, workspace.id)
                    }
                    onDrop={(event) => void handleWorkspaceDrop(event)}
                  >
                    <div className="flex items-start pt-1">
                      <span
                        aria-label={t("Reorder workspace {name}", {
                          name: workspace.name,
                        })}
                        className={`inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-400 shadow-sm ${
                          isSavingWorkspaceOrder
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
                    <div className="min-w-0 select-text">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm font-semibold">
                          {workspace.name}
                        </span>
                        {workspace.isDefault ? (
                          <CapabilityPill label={t("Default workspace")} ok />
                        ) : null}
                        {workspace.pinned ? (
                          <CapabilityPill label={t("pinned")} ok />
                        ) : null}
                      </div>
                      <div className="mt-1 truncate text-xs font-medium text-stone-500">
                        {workspace.id} / {terminalShellLabel(terminalShells, workspace.terminalShell)}
                      </div>
                      <div className="mt-1 break-all text-xs text-stone-500">
                        {workspace.path}
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-2 md:justify-end">
                      <button
                        aria-label={t(
                          workspace.pinned
                            ? "Unpin workspace {name}"
                            : "Pin workspace {name}",
                          { name: workspace.name },
                        )}
                        className={`inline-flex size-9 items-center justify-center rounded-lg border shadow-sm ${
                          workspace.pinned
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
                        onClick={() => editConfiguredWorkspace(workspace)}
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
                            disabled={isSavingSkills}
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

function FocoLogoMark() {
  return (
    <span className="inline-flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-white shadow-[0_10px_24px_rgba(15,118,110,0.2)] ring-1 ring-stone-200/80">
      <img
        alt=""
        aria-hidden="true"
        className="size-full object-cover"
        src="/foco.svg"
      />
    </span>
  );
}

function workspaceItemClass(active: boolean) {
  return `flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg px-2 text-sm font-semibold ${
    active ? "text-teal-950" : "text-stone-700"
  }`;
}

function workspaceNameFromPath(path: string) {
  const trimmedPath = path.trim().replace(/[\\/]+$/g, "");
  const parts = trimmedPath.split(/[\\/]+/);

  return parts.at(-1) ?? "";
}

function workspaceMenuClass(active: boolean) {
  return `flex min-w-0 items-center gap-1 rounded-xl border px-1.5 py-1 transition-colors ${
    active
      ? "border-teal-200 bg-teal-50 text-teal-950 shadow-sm"
      : "border-transparent bg-stone-100/60 text-stone-700 hover:border-stone-200 hover:bg-white/90 hover:text-stone-950"
  }`;
}

function chatItemClass(active: boolean) {
  return `flex min-h-11 min-w-0 flex-1 items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-xs font-medium ${
    active
      ? "border-teal-100 bg-white text-stone-950 shadow-sm"
      : "border-transparent text-stone-600 hover:border-stone-200 hover:bg-white/80 hover:text-stone-950"
  }`;
}

function hydrateChatTab(
  tab: OpenChatTab,
  workspaces: WorkspaceSummary[],
): ChatTabSummary {
  const workspace = workspaces.find((workspace) => workspace.id === tab.workspaceId);
  const chat = workspace?.chats.find((chat) => chat.id === tab.chatId);

  return {
    ...tab,
    title: chat?.title ?? tab.fallbackTitle,
    workspaceName: workspace?.name ?? tab.fallbackWorkspaceName,
  };
}

function upsertOpenChatTab(tabs: OpenChatTab[], nextTab: OpenChatTab) {
  if (
    tabs.some(
      (tab) =>
        tab.workspaceId === nextTab.workspaceId && tab.chatId === nextTab.chatId,
    )
  ) {
    return tabs;
  }

  return [...tabs, nextTab];
}

function workspaceHasChat(
  workspaces: WorkspaceSummary[],
  tab: { workspaceId: string; chatId: string },
) {
  return workspaces.some(
    (workspace) =>
      workspace.id === tab.workspaceId &&
      workspace.chats.some((chat) => chat.id === tab.chatId),
  );
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

  if (section === "workspaces") {
    return t("Workspace settings");
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

function LoginView({
  error,
  isLoggingIn,
  onLogin,
  onPasswordChange,
  password,
}: {
  error: string | null;
  isLoggingIn: boolean;
  onLogin: (event: FormEvent<HTMLFormElement>) => void;
  onPasswordChange: (value: string) => void;
  password: string;
}) {
  const { t } = useI18n();

  return (
    <main className="app-root grid place-items-center bg-stone-100 px-4 text-stone-950">
      <form
        aria-label={t("Foco authentication")}
        className="w-full max-w-sm rounded-2xl border border-stone-200 bg-white/90 px-4 py-5 shadow-[0_24px_70px_rgba(33,31,28,0.16)]"
        onSubmit={onLogin}
      >
        <div className="flex items-center gap-3">
          <FocoLogoMark />
          <div className="min-w-0">
            <h1 className="text-lg font-semibold text-stone-950">Foco</h1>
            <p className="mt-1 text-xs font-medium text-stone-500">
              {t("Password required")}
            </p>
          </div>
        </div>
        <label className="mt-5 block">
          <span className="mb-1.5 block text-xs font-semibold text-stone-600">
            {t("Password")}
          </span>
          <input
            autoComplete="current-password"
            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
            onChange={(event) => onPasswordChange(event.target.value)}
            type="password"
            value={password}
          />
        </label>
        {error ? (
          <div className="mt-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}
        <button
          aria-label={t("Log in")}
          className="mt-4 inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-stone-950 px-3 text-sm font-semibold text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
          disabled={isLoggingIn || !password.trim()}
          type="submit"
        >
          {isLoggingIn ? (
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          ) : (
            <Lock aria-hidden="true" className="size-4" />
          )}
          {t("Log in")}
        </button>
      </form>
    </main>
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
    password: "",
  };
}

function emptyWorkspaceForm(): WorkspaceFormState {
  return {
    id: "",
    name: "",
    path: "",
    pinned: false,
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

function moveItemId(
  itemIds: string[],
  sourceItemId: string,
  targetItemId: string,
) {
  const sourceIndex = itemIds.indexOf(sourceItemId);
  const targetIndex = itemIds.indexOf(targetItemId);

  if (sourceIndex === -1 || targetIndex === -1 || sourceIndex === targetIndex) {
    return itemIds;
  }

  const next = [...itemIds];
  const [source] = next.splice(sourceIndex, 1);
  next.splice(targetIndex, 0, source);

  return next;
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
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill))
    .map((skill) => `[$${skill.name}](${skill.path})`);

  return links.length ? `${links.join(" ")} ${message}` : message;
}

function skillScopeLabel(skill: ConfiguredSkillSummary, t: Translate) {
  if (skill.scope === "global") {
    return t("Global skill");
  }

  return skill.workspaceName
    ? t("Workspace skill {name}", { name: skill.workspaceName })
    : t("Workspace skill");
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
    metrics: streamEvent.metrics,
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

function compactInlineText(value: string) {
  return value.replace(/\s+/g, " ").trim();
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

function emptyAiStatsFilters(): AiStatsFilterState {
  return {
    chatId: "",
    modelId: "",
    page: "1",
    pageSize: "50",
    providerId: "",
    startedAfter: "",
    startedBefore: "",
    status: "",
    workspaceId: "",
  };
}

function aiStatsQuery(filters: AiStatsFilterState) {
  const params = new URLSearchParams();
  const entries: [keyof AiStatsFilterState, string][] = [
    ["workspaceId", filters.workspaceId],
    ["chatId", filters.chatId],
    ["providerId", filters.providerId],
    ["modelId", filters.modelId],
    ["status", filters.status],
    ["startedAfter", datetimeLocalToRfc3339(filters.startedAfter)],
    ["startedBefore", datetimeLocalToRfc3339(filters.startedBefore)],
    ["page", filters.page.trim()],
    ["pageSize", filters.pageSize.trim()],
  ];

  for (const [key, value] of entries) {
    if (value) {
      params.set(key, value);
    }
  }

  return params.toString();
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

function datetimeLocalToRfc3339(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "";
  }

  const date = new Date(trimmed);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`invalid date time: ${value}`);
  }

  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function auditOptions(
  configuredOptions: { label: string; value: string }[],
  observedValues: string[],
  labelForObserved: (value: string) => string = (value) => value,
) {
  const optionsByValue = new Map<string, { label: string; value: string }>();

  for (const option of configuredOptions) {
    if (option.value) {
      optionsByValue.set(option.value, option);
    }
  }

  for (const value of observedValues) {
    if (value && !optionsByValue.has(value)) {
      optionsByValue.set(value, { label: labelForObserved(value), value });
    }
  }

  return Array.from(optionsByValue.values()).sort((left, right) =>
    left.label.localeCompare(right.label),
  );
}

function auditStatusText(status: string, t: Translate) {
  if (status === "succeeded" || status === "completed") {
    return t("succeeded");
  }

  if (status === "failed") {
    return t("failed");
  }

  return status;
}

function auditStatusClass(status: string) {
  const base = "inline-flex rounded-md px-2 py-1 text-xs font-semibold";

  if (status === "succeeded" || status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "failed") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function taskStatusClass(status: TaskStatus) {
  const base = "inline-flex rounded-md px-2 py-0.5 text-[11px] font-semibold";

  if (status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "running" || status === "ready") {
    return `${base} bg-sky-100 text-sky-800`;
  }

  if (status === "blocked") {
    return `${base} bg-amber-100 text-amber-800`;
  }

  if (status === "failed" || status === "cancelled") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function auditJsonText(value: JsonValue | null) {
  return value === null ? "null" : formatJsonValue(value);
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

function formatTaskGraphDate(value: string, language: AppLanguageId = "en") {
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

function formatNullableNumber(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : formatNumber(value, language);
}

function formatNullableLatency(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : `${formatNumber(value, language)} ms`;
}

function formatTokensPerSecond(
  metrics: ChatReplyMetrics,
  language: AppLanguageId = "en",
) {
  if (
    metrics.outputTokens === null ||
    metrics.totalLatencyMs === null ||
    metrics.totalLatencyMs <= 0
  ) {
    return "n/a";
  }

  return new Intl.NumberFormat(language, {
    maximumFractionDigits: 2,
  }).format(metrics.outputTokens / (metrics.totalLatencyMs / 1000));
}

function formatPercent(value: number | null, language: AppLanguageId = "en") {
  if (value === null) {
    return "n/a";
  }

  return new Intl.NumberFormat(language, {
    maximumFractionDigits: 1,
    style: "percent",
  }).format(value);
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

function pendingChatRunKey(workspaceId: string, runKey: string) {
  return `${workspaceId}:pending:${runKey}`;
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

function isTerminalConnected(status: TerminalPaneStatus) {
  return status === "connected";
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
    const metrics = parseRequiredChatReplyMetrics(fieldValue(value, "metrics"));

    if (!chatId || !assistantMessageId || text === null) {
      return null;
    }

    if (
      reasoning === false ||
      usage === false ||
      stopReason === false ||
      metrics === false
    ) {
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
      metrics,
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

  if (value.type === "questionRequest" || value.type === "question_request") {
    const assistantMessageId = stringField(
      value,
      "assistantMessageId",
      "assistant_message_id",
    );
    const request = parseQuestionRequestSummary(fieldValue(value, "request"));

    if (!assistantMessageId || !request) {
      return null;
    }

    return { type: "questionRequest", assistantMessageId, request };
  }

  if (value.type === "gitDiffRefresh" || value.type === "git_diff_refresh") {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");

    if (!workspaceId) {
      return null;
    }

    return { type: "gitDiffRefresh", workspaceId };
  }

  if (
    value.type === "taskGraphRefresh" ||
    value.type === "task_graph_refresh"
  ) {
    const workspaceId = stringField(value, "workspaceId", "workspace_id");
    const chatId = stringField(value, "chatId", "chat_id");

    if (!workspaceId || !chatId) {
      return null;
    }

    return { type: "taskGraphRefresh", workspaceId, chatId };
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

function parseQuestionRequestSummary(value: unknown): QuestionRequestSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const toolCallId = stringField(value, "toolCallId", "tool_call_id");
  const workspaceId = stringField(value, "workspaceId", "workspace_id");
  const chatId = stringField(value, "chatId", "chat_id");
  const questions = fieldValue(value, "questions");

  if (
    !id ||
    !toolCallId ||
    !workspaceId ||
    !chatId ||
    !Array.isArray(questions) ||
    questions.length === 0
  ) {
    return null;
  }

  const parsedQuestions = questions.map(parseQuestionItemSummary);
  if (parsedQuestions.some((question) => question === null)) {
    return null;
  }

  return {
    chatId,
    id,
    questions: parsedQuestions as QuestionItemSummary[],
    toolCallId,
    workspaceId,
  };
}

function parseQuestionItemSummary(value: unknown): QuestionItemSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const id = stringField(value, "id");
  const question = stringField(value, "question");
  const options = fieldValue(value, "options");
  const allowFreeText = fieldValue(value, "allowFreeText", "allow_free_text");

  if (
    !id ||
    !question ||
    !Array.isArray(options) ||
    typeof allowFreeText !== "boolean"
  ) {
    return null;
  }

  const parsedOptions = options.map(parseQuestionOptionSummary);
  if (parsedOptions.some((option) => option === null)) {
    return null;
  }

  return {
    allowFreeText,
    id,
    options: parsedOptions as QuestionOptionSummary[],
    question,
  };
}

function parseQuestionOptionSummary(value: unknown): QuestionOptionSummary | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const label = stringField(value, "label");
  const optionValue = stringField(value, "value");
  const description = optionalNullableStringField(value, "description");

  if (!label || !optionValue || description === false) {
    return null;
  }

  return {
    description: description ?? null,
    label,
    value: optionValue,
  };
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
  const metrics = parseOptionalChatReplyMetrics(message.metrics);
  if (metrics === false) {
    throw new Error("chat message metrics are invalid");
  }

  const toolCalls = Array.isArray(message.toolCalls)
    ? message.toolCalls.map(normalizedToolCallSummary)
    : [];
  const partsSource = Array.isArray(message.parts) ? message.parts : [];
  const parts = partsSource
    .map((part) => normalizeChatMessagePart(part))
    .filter((part): part is ChatMessagePart => part !== null);
  const normalizedMessage = {
    ...message,
    metrics,
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

function parseRequiredChatReplyMetrics(value: unknown): ChatReplyMetrics | false {
  const metrics = parseChatReplyMetrics(value);

  if (metrics === undefined || metrics === null) {
    return false;
  }

  return metrics;
}

function parseOptionalChatReplyMetrics(
  value: unknown,
): ChatReplyMetrics | null | false {
  if (typeof value === "undefined" || value === null) {
    return null;
  }

  const metrics = parseChatReplyMetrics(value);

  return metrics === undefined ? false : metrics;
}

function parseChatReplyMetrics(
  value: unknown,
): ChatReplyMetrics | undefined | false {
  if (typeof value === "undefined") {
    return undefined;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  const modelId = stringField(value, "modelId", "model_id");
  const providerId = stringField(value, "providerId", "provider_id");
  const totalLatencyMs = fieldValue(
    value,
    "totalLatencyMs",
    "total_latency_ms",
  );
  const firstTokenLatencyMs = fieldValue(
    value,
    "firstTokenLatencyMs",
    "first_token_latency_ms",
  );
  const outputTokens = fieldValue(value, "outputTokens", "output_tokens");

  if (
    !modelId ||
    !providerId ||
    !isNullableNumber(totalLatencyMs) ||
    !isNullableNumber(firstTokenLatencyMs) ||
    !isNullableNumber(outputTokens)
  ) {
    return false;
  }

  return {
    firstTokenLatencyMs,
    modelId,
    outputTokens,
    providerId,
    totalLatencyMs,
  };
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
  const response = await fetch(url, {
    cache: "no-store",
    credentials: "same-origin",
    ...init,
  });
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
