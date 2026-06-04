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
  GitCompare,
  KeyRound,
  LoaderCircle,
  MessageSquare,
  PlugZap,
  Plus,
  RefreshCw,
  Send,
  Settings,
  SlidersHorizontal,
  Terminal,
  Trash2,
  User,
  Wrench,
  X,
} from "lucide-react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  FormEvent,
  CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

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

type SettingsResponse = {
  providerKinds: ProviderKindSummary[];
  thinkingLevels: ThinkingLevelSummary[];
  providers: ConfiguredProviderSummary[];
  configuredModels: ConfiguredModelSummary[];
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

type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
  toolCalls: ChatToolCallSummary[];
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
      llmRequestId: string;
    }
  | { type: "textDelta"; delta: string }
  | { type: "reasoningDelta"; delta: string }
  | { type: "usage"; usage: ChatUsage }
  | {
      type: "complete";
      chatId: string;
      assistantMessageId: string;
      text: string;
      usage: ChatUsage | null;
      stopReason: string | null;
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
type SettingsSection = "models" | "providers";
type ViewMode = "chat" | "settings" | "stats";

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
  status?: "error" | "streaming";
  toolCalls: ChatToolCallSummary[];
};

type RetryRunRequest = {
  workspaceId: string;
  chatId: string | null;
  content: string;
  modelId: string;
  thinkingLevel: string;
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
  const [retryRunRequest, setRetryRunRequest] =
    useState<RetryRunRequest | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeRunAbortRef = useRef<AbortController | null>(null);

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
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const selectedDiffText = formatDiffText(gitDiff);
  const isTerminalOpen = activeWorkspace
    ? terminalOpenWorkspaceIds.has(activeWorkspace.id)
    : false;

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
    setSelectedModelId((current) =>
      availableModels.some((model) => model.id === current)
        ? current
        : (availableModels[0]?.id ?? ""),
    );
  }, [availableModels]);

  useEffect(() => {
    const selectedModel = availableModels.find(
      (model) => model.id === selectedModelId,
    );
    setSelectedThinkingLevel(selectedModel?.thinkingLevel ?? "");
  }, [availableModels, selectedModelId]);

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
      setMessages(data.messages);
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

  async function handleSendMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const content = draftMessage.trim();
    if (!content || isSendingMessage) {
      return;
    }

    if (!activeWorkspace) {
      setError("Select a workspace before sending.");
      return;
    }

    if (!selectedModelId) {
      setError("Select an enabled model before sending.");
      return;
    }

    await runChatMessage({
      chatId: activeChatId,
      content,
      modelId: selectedModelId,
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
    setSelectedModelId(retryRequest.modelId);
    setSelectedThinkingLevel(retryRequest.thinkingLevel);
    await runChatMessage(retryRequest);
  }

  function handleCancelRun() {
    activeRunAbortRef.current?.abort();
  }

  async function runChatMessage(request: RetryRunRequest) {
    const localUserId = `local-user-${Date.now()}`;
    let assistantMessageId = `local-assistant-${Date.now()}`;
    let requestChatId = request.chatId;
    const abortController = new AbortController();

    setMessages((current) => [
      ...current,
      {
        id: localUserId,
        role: "user",
        content: request.content,
        toolCalls: [],
      },
      {
        id: assistantMessageId,
        role: "assistant",
        content: "",
        status: "streaming",
        toolCalls: [],
      },
    ]);
    setDraftMessage("");
    setIsSendingMessage(true);
    setRetryRunRequest(null);
    setError(null);
    activeRunAbortRef.current = abortController;

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(request.workspaceId)}/chat/stream`,
        {
          body: JSON.stringify({
            chatId: request.chatId,
            message: request.content,
            modelId: request.modelId,
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
          setActiveChatId(streamEvent.chatId);
          setMessages((current) =>
            current.map((message) => {
              if (message.id === localUserId) {
                return { ...message, id: streamEvent.userMessageId };
              }

              if (message.id === assistantMessageId || message.id.startsWith("local-assistant-")) {
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
              message.id === assistantMessageId
                ? { ...message, content: message.content + streamEvent.delta }
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
              message.id === assistantMessageId
                ? {
                    ...message,
                    content: streamEvent.text,
                    status: undefined,
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolCall") {
          setMessages((current) =>
            current.map((message) =>
              message.id === streamEvent.assistantMessageId
                ? {
                    ...message,
                    toolCalls: upsertToolCall(
                      message.toolCalls,
                      streamEvent.toolCall,
                    ),
                  }
                : message,
            ),
          );
          return;
        }

        if (streamEvent.type === "toolResult") {
          setMessages((current) =>
            current.map((message) =>
              message.id === streamEvent.assistantMessageId
                ? {
                    ...message,
                    toolCalls: applyToolResult(
                      message.toolCalls,
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
              message.id === assistantMessageId
                ? {
                    ...message,
                    content: streamEvent.message,
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
      const message = wasCancelled ? "Run cancelled." : errorMessage(requestError);
      setError(message);
      setRetryRunRequest({
        ...request,
        chatId: requestChatId,
      });
      setMessages((current) =>
        current.map((item) =>
          item.id === assistantMessageId
            ? { ...item, content: message, status: "error" }
            : item,
        ),
      );
    } finally {
      if (activeRunAbortRef.current === abortController) {
        activeRunAbortRef.current = null;
      }
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
            aria-label="Resize workspace sidebar"
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
                    Local workspace
                  </span>
                </div>
              </div>
              <button
                aria-label="Refresh workspaces"
                className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white/90 text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={isLoading}
                onClick={() => void refreshWorkspaces()}
                title="Refresh workspaces"
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
                aria-label="Create or add workspace"
                className={`${workspaceActionClass()} w-full`}
                onClick={() => openWorkspaceDialog("create")}
                title="Create or add workspace"
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
                            ? "Collapse chat history"
                            : "Expand chat history"
                        }
                        className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 hover:bg-stone-100 hover:text-stone-900"
                        onClick={() => toggleWorkspace(workspace.id)}
                        title={
                          isExpanded
                            ? "Collapse chat history"
                            : "Expand chat history"
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
                        aria-label={`New chat in ${workspace.name}`}
                        className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 hover:bg-teal-50 hover:text-teal-800"
                        onClick={() => startNewWorkspaceChat(workspace.id)}
                        title="New chat"
                        type="button"
                      >
                        <Plus aria-hidden="true" className="size-4" />
                      </button>
                    </div>
                    {isExpanded ? (
                      <div className="ml-9 mt-1 space-y-1">
                        {workspace.chats.length > 0 ? (
                          workspace.chats.map((chat) => (
                            <button
                              className="flex w-full min-w-0 items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs font-medium text-stone-600 hover:bg-white/80 hover:text-stone-950"
                              key={chat.id}
                              onClick={() =>
                                void loadChatMessages(workspace.id, chat.id)
                              }
                              type="button"
                            >
                              <MessageSquare
                                aria-hidden="true"
                                className="size-3.5 shrink-0"
                              />
                              <span className="truncate">{chat.title}</span>
                            </button>
                          ))
                        ) : (
                          <div className="rounded-lg px-2 py-1.5 text-xs text-stone-500">
                            No chats
                          </div>
                        )}
                      </div>
                    ) : null}
                  </div>
                );
                })
              ) : (
                <div className="mx-2 rounded-lg border border-dashed border-stone-300 bg-white/60 px-3 py-4 text-sm text-stone-500">
                  {isLoading ? "Loading workspaces..." : "No workspaces"}
                </div>
              )}
            </nav>
          </div>
        </aside>

        <section className="app-main-panel flex min-w-0 flex-col">
          <header className="border-b border-stone-200/80 bg-white/80 px-4 py-2 backdrop-blur sm:px-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <h1 className="truncate text-lg font-semibold text-stone-950">
                  {activeWorkspace?.name ?? "Workspace"}
                </h1>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {activeWorkspace?.path ?? ""}
                </p>
              </div>
              <div className="flex overflow-x-auto rounded-xl border border-stone-200 bg-stone-100/80 p-1 shadow-inner">
                <NavButton
                  active={viewMode === "chat"}
                  icon={MessageSquare}
                  label="Chat"
                  onClick={() => setViewMode("chat")}
                />
                <NavButton
                  active={viewMode === "settings"}
                  icon={Settings}
                  label="Settings"
                  onClick={() => setViewMode("settings")}
                />
                <NavButton
                  active={viewMode === "stats"}
                  icon={BarChart3}
                  label="Stats"
                  onClick={() => setViewMode("stats")}
                />
                <button
                  aria-label={isTerminalOpen ? "Close terminal" : "Open terminal"}
                  className={`inline-flex size-9 items-center justify-center rounded-lg ${
                    isTerminalOpen
                      ? "bg-white text-teal-900 shadow-sm"
                      : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
                  } disabled:cursor-not-allowed disabled:text-stone-400`}
                  disabled={!activeWorkspace}
                  onClick={toggleWorkspaceTerminal}
                  title={isTerminalOpen ? "Close terminal" : "Open terminal"}
                  type="button"
                >
                  <Terminal aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label={isDiffPanelOpen ? "Close git diff" : "Open git diff"}
                  className={`inline-flex size-9 items-center justify-center rounded-lg ${
                    isDiffPanelOpen
                      ? "bg-white text-teal-900 shadow-sm"
                      : "text-stone-600 hover:bg-white/60 hover:text-stone-950"
                  }`}
                  onClick={() => setIsDiffPanelOpen((current) => !current)}
                  title={isDiffPanelOpen ? "Close git diff" : "Open git diff"}
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
              draftMessage={draftMessage}
              isLoadingSettings={isLoadingSettings}
              isSendingMessage={isSendingMessage}
              messages={messages}
              onDraftMessageChange={setDraftMessage}
              onCancelRun={handleCancelRun}
              onModelChange={setSelectedModelId}
              onRetryRun={() => void handleRetryRun()}
              onSubmit={handleSendMessage}
              onThinkingLevelChange={setSelectedThinkingLevel}
              canRetryRun={retryRunRequest !== null && !isSendingMessage}
              selectedModelId={selectedModelId}
              selectedThinkingLevel={selectedThinkingLevel}
              thinkingLevels={thinkingLevels}
            />
          ) : viewMode === "settings" ? (
            <SettingsPanel />
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
    </main>
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
  const title =
    formMode === "create" ? "Create workspace" : "Add existing workspace";

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
                ? "Create and register a new local folder."
                : "Register an existing local folder."}
            </p>
          </div>
          <button
            aria-label="Close workspace dialog"
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title="Close"
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <div className="grid grid-cols-2 gap-2 border-b border-stone-200 bg-stone-50/80 px-4 py-3">
          <button
            aria-label="Switch to create workspace"
            className={workspaceModeClass(formMode === "create")}
            onClick={() => onModeChange("create")}
            title="Create workspace"
            type="button"
          >
            <Plus aria-hidden="true" className="size-4" />
          </button>
          <button
            aria-label="Switch to add workspace"
            className={workspaceModeClass(formMode === "add")}
            onClick={() => onModeChange("add")}
            title="Add workspace"
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
              Name
            </span>
            <input
              autoComplete="off"
              className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              name="workspace-name"
              onChange={(event) => onNameChange(event.target.value)}
              placeholder="Workspace name"
              value={name}
            />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              Path
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
              aria-label="Cancel workspace dialog"
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
              onClick={onClose}
              title="Cancel"
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

function ChatPanel({
  availableModels,
  canRetryRun,
  draftMessage,
  isLoadingSettings,
  isSendingMessage,
  messages,
  onCancelRun,
  onDraftMessageChange,
  onModelChange,
  onRetryRun,
  onSubmit,
  onThinkingLevelChange,
  selectedModelId,
  selectedThinkingLevel,
  thinkingLevels,
}: {
  availableModels: ConfiguredModelSummary[];
  canRetryRun: boolean;
  draftMessage: string;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  messages: ShellMessage[];
  onCancelRun: () => void;
  onDraftMessageChange: (value: string) => void;
  onModelChange: (value: string) => void;
  onRetryRun: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onThinkingLevelChange: (value: string) => void;
  selectedModelId: string;
  selectedThinkingLevel: string;
  thinkingLevels: ThinkingLevelSummary[];
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3 sm:px-5 sm:py-4">
        <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
          {messages.length ? (
            messages.map((message) => {
            const isUser = message.role === "user";

            return (
              <div
                className={`flex ${isUser ? "justify-end" : "justify-start"}`}
                key={message.id}
              >
                <div
                  className={`flex max-w-[min(42rem,92%)] gap-3 rounded-2xl border px-4 py-3 shadow-[0_18px_42px_rgba(75,63,42,0.08)] sm:max-w-[78%] ${
                    isUser
                      ? "rounded-tr-md border-teal-700 bg-teal-800 text-white"
                      : "rounded-tl-md border-stone-200 bg-white/90 text-stone-900"
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
                    <div
                      className={`min-w-0 whitespace-pre-wrap break-words text-sm leading-6 ${
                        message.status === "error" ? "text-rose-700" : ""
                      }`}
                    >
                      {message.content ||
                        (message.status === "streaming" ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-4 animate-spin"
                          />
                        ) : null)}
                    </div>
                    {!isUser && message.toolCalls.length > 0 ? (
                      <ToolCallList toolCalls={message.toolCalls} />
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
                Workspace shell is ready
              </h2>
              <p className="mt-2 max-w-sm text-sm leading-6 text-stone-600">
                Pick an enabled model and start the current workspace chat.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="border-t border-stone-200/80 bg-white/80 px-3 py-2 backdrop-blur sm:px-5">
        <form className="mx-auto max-w-5xl" onSubmit={onSubmit}>
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <label className="min-w-0 flex-1 sm:max-w-64">
              <span className="sr-only">
                Model
              </span>
              <select
                className="h-8 w-full rounded-lg border border-stone-300 bg-white px-2 text-xs font-medium text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
                  <option value="">No enabled models</option>
                )}
              </select>
            </label>
            <label className="w-36 max-w-full">
              <span className="sr-only">
                Thinking
              </span>
              <select
                className="h-8 w-full rounded-lg border border-stone-300 bg-white px-2 text-xs font-medium text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                disabled={isSendingMessage}
                onChange={(event) => onThinkingLevelChange(event.target.value)}
                value={selectedThinkingLevel}
              >
                <option value="">Model default</option>
                {thinkingLevels.map((level) => (
                  <option key={level.value} value={level.value}>
                    {level.label}
                  </option>
                ))}
              </select>
            </label>
            {canRetryRun ? (
              <button
                aria-label="Retry last run"
                className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                onClick={onRetryRun}
                title="Retry last run"
                type="button"
              >
                <RefreshCw aria-hidden="true" className="size-4" />
              </button>
            ) : null}
          </div>
          <div className="relative">
            <textarea
              className="min-h-20 w-full resize-none rounded-xl border border-stone-300 bg-white px-3 py-2 pb-10 pr-14 text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              disabled={isSendingMessage}
              name="message"
              onChange={(event) => onDraftMessageChange(event.target.value)}
              placeholder="Message Foco"
              value={draftMessage}
            />
            {isSendingMessage ? (
              <button
                aria-label="Cancel run"
                className="absolute bottom-2 right-2 inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50"
                onClick={onCancelRun}
                title="Cancel run"
                type="button"
              >
                <X aria-hidden="true" className="size-5" />
              </button>
            ) : (
              <button
                aria-label="Send message"
                className="absolute bottom-2 right-2 inline-flex size-9 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                disabled={!draftMessage.trim() || !selectedModelId}
                title="Send"
                type="submit"
              >
                <Send aria-hidden="true" className="size-5" />
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
}

function ToolCallList({ toolCalls }: { toolCalls: ChatToolCallSummary[] }) {
  return (
    <div className="space-y-2 border-t border-stone-200 pt-2">
      {toolCalls.map((toolCall) => (
        <details className="group min-w-0" key={toolCall.id}>
          <summary className="flex cursor-pointer list-none items-center gap-2 text-xs font-semibold text-stone-700 marker:hidden">
            <Wrench aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
            <span className="min-w-0 flex-1 truncate">{toolCall.name}</span>
            <span
              className={`shrink-0 rounded-md px-1.5 py-0.5 text-[11px] ${
                toolCall.isError
                  ? "bg-rose-50 text-rose-700"
                  : "bg-stone-100 text-stone-600"
              }`}
            >
              {toolStatusText(toolCall)}
            </span>
          </summary>
          <div className="mt-2 grid gap-2 text-xs text-stone-600">
            <div className="min-w-0">
              <div className="mb-1 font-semibold text-stone-500">Input</div>
              <pre className="panel-scroll max-h-48 overflow-auto whitespace-pre-wrap break-words border-l border-stone-200 pl-3 font-mono text-[11px] leading-5">
                {formatJsonValue(toolCall.input)}
              </pre>
            </div>
            {toolCall.output !== null ? (
              <div className="min-w-0">
                <div className="mb-1 font-semibold text-stone-500">Output</div>
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
      ))}
    </div>
  );
}

function TerminalPanel({ workspace }: { workspace: WorkspaceSummary | undefined }) {
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
      setError("Terminal container was not mounted.");
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
            setError("Terminal returned an unknown event.");
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
            terminal.writeln(`\r\n[terminal exited: ${parsed.status}]`);
            return;
          }

          setStatus("error");
          setError(parsed.message);
          terminal.writeln(`\r\n[terminal error: ${parsed.message}]`);
        };
        socket.onerror = () => {
          setStatus("error");
          setError("Terminal WebSocket failed.");
        };
        socket.onclose = () => {
          setStatus((current) => (current === "error" ? current : "closed"));
        };
      } catch (requestError) {
        if (!cancelled) {
          const message = errorMessage(requestError);
          setStatus("error");
          setError(message);
          terminal.writeln(`[terminal error: ${message}]`);
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
    <section className="border-t border-stone-800 bg-[#16130f]">
      <div className="mx-auto w-full max-w-5xl">
        <div className="flex h-8 items-center justify-between gap-3 px-3 text-xs text-stone-400">
          <span className="inline-flex min-w-0 items-center gap-2">
            <Terminal aria-hidden="true" className="size-4 shrink-0" />
            <span className={terminalStatusClass(status)}>
              {terminalStatusText(status)}
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
                API statistics
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {activeWorkspace?.name ?? "No workspace selected"}
              </p>
            </div>
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <StatsCard
            icon={Activity}
            label="Workspace chats"
            value={formatNumber(chatCount)}
          />
          <StatsCard
            icon={PlugZap}
            label="Enabled providers"
            value={formatNumber(enabledProviders)}
          />
          <StatsCard
            icon={Bot}
            label="Runnable models"
            value={formatNumber(availableModels.length)}
          />
          <StatsCard
            icon={SlidersHorizontal}
            label="Configured models"
            value={formatNumber(configuredModels)}
          />
        </section>

        <section className="rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="border-b border-stone-200 px-4 py-3">
            <h3 className="text-sm font-semibold text-stone-950">
              Request audit
            </h3>
          </div>
          <div className="grid gap-3 px-4 py-8 text-sm text-stone-600 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
            <div className="min-w-0">
              <div className="font-semibold text-stone-900">
                No API request table is exposed yet
              </div>
              <p className="mt-1 max-w-2xl leading-6">
                The app records LLM request audit rows in the workspace
                database. This page now has a dedicated surface ready for the
                request-summary API when it is added.
              </p>
            </div>
            <span className="inline-flex h-9 items-center rounded-lg border border-dashed border-stone-300 px-3 text-xs font-semibold text-stone-500">
              audit data pending
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
  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-col">
      <div
        aria-label="Resize git diff panel"
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
            <h2 className="truncate text-sm font-semibold">Git diff</h2>
            <p className="truncate text-xs font-medium text-stone-500">
              {selectedPath ?? "Workspace changes"}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            aria-label="Refresh diff"
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
            disabled={isLoading}
            onClick={onRefresh}
            title="Refresh diff"
            type="button"
          >
            {isLoading ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <RefreshCw aria-hidden="true" className="size-4" />
            )}
          </button>
          <button
            aria-label="Close git diff"
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title="Close git diff"
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
          <span className="truncate">All changed files</span>
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
              No changes
            </div>
          )}
        </div>
      </div>

      <div className="panel-scroll min-h-0 flex-1 overflow-auto bg-[#16130f]">
        <pre className="min-h-full whitespace-pre-wrap break-words px-4 py-4 font-mono text-[11px] leading-5 text-stone-100">
          {diffText || "No diff"}
        </pre>
      </div>
    </div>
  );
}

function SettingsPanel() {
  const [activeSection, setActiveSection] =
    useState<SettingsSection>("providers");
  const [isProviderDialogOpen, setIsProviderDialogOpen] = useState(false);
  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [metadata, setMetadata] = useState<ModelMetadataResponse | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedMetadataKey, setSelectedMetadataKey] = useState("");
  const [modelSearch, setModelSearch] = useState("");
  const [form, setForm] = useState<ModelFormState>(() => emptyModelForm());
  const [providerForm, setProviderForm] = useState<ProviderFormState>(() =>
    emptyProviderForm(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
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
  const thinkingLevels = settings?.thinkingLevels ?? [];
  const configuredModels =
    settings?.configuredModels ?? metadata?.configuredModels ?? [];
  const editingModel =
    configuredModels.find((model) => model.id === form.modelId) ?? null;
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
  const editingProvider =
    providers.find((provider) => provider.id === providerForm.id) ?? null;
  const hasSavedProviderKey = editingProvider?.hasApiKey ?? false;
  const selectedProviderIds = new Set(form.providerIds);

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
      setProviderForm((current) => ({
        ...current,
        kind: current.kind || data.providerKinds[0]?.kind || "openai-responses",
      }));
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoadingSettings(false);
    }
  }, []);

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

  async function testProvider(providerId: string) {
    setProviderTests((current) => ({
      ...current,
      [providerId]: { message: "Testing connection...", status: "testing" },
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

  return (
    <div className="panel-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="mx-auto grid max-w-7xl gap-4 lg:grid-cols-[4.5rem_minmax(0,1fr)]">
        <aside className="flex gap-2 rounded-2xl border border-stone-200 bg-white/85 p-2 shadow-[0_18px_42px_rgba(75,63,42,0.07)] lg:flex-col lg:self-start">
          <SettingsNavButton
            active={activeSection === "providers"}
            icon={PlugZap}
            label="Providers"
            onClick={() => setActiveSection("providers")}
          />
          <SettingsNavButton
            active={activeSection === "models"}
            icon={SlidersHorizontal}
            label="Models"
            onClick={() => setActiveSection("models")}
          />
        </aside>

        <div className="min-w-0 flex flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/75 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-stone-950">
                Provider and model settings
              </h2>
              <p className="mt-1 truncate text-xs font-medium text-stone-500">
                {metadata?.fetchedAt
                  ? `Fetched ${metadata.fetchedAt} from ${metadata.sourceUrl}`
                  : "Model metadata has not been refreshed"}
              </p>
            </div>
            <button
              aria-label="Refresh model metadata"
              className="inline-flex size-10 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
              disabled={isRefreshing}
              onClick={() => void refreshMetadata()}
              title="Refresh model metadata"
              type="button"
            >
              {isRefreshing ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <RefreshCw aria-hidden="true" className="size-4" />
              )}
            </button>
          </div>
        </section>

        {error ? (
          <div className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        {activeSection === "providers" ? (
        <section className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
          {isProviderDialogOpen ? (
          <>
          <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
          <form
            aria-label="Provider configuration"
            className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-stone-200 bg-white px-4 py-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
            onSubmit={(event) => void saveProvider(event)}
          >
            <div className="mb-4 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <PlugZap aria-hidden="true" className="size-5 text-teal-700" />
                  <h3 className="text-sm font-semibold text-stone-950">
                    {providerForm.id ? "Edit provider" : "Add provider"}
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
                    aria-label="Enable provider"
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
                  aria-label="Delete provider"
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                  disabled={isSavingProvider}
                  onClick={() => void deleteProvider(providerForm.id)}
                  title="Delete provider"
                  type="button"
                >
                  <Trash2 aria-hidden="true" className="size-4" />
                </button>
                ) : null}
                <button
                  aria-label="Close provider configuration"
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                  onClick={() => setIsProviderDialogOpen(false)}
                  title="Close"
                  type="button"
                >
                  <X aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
            <div className="space-y-3">
              <TextField
                label="Name"
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
                  Protocol
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
                label="Base URL"
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
                  API key
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
                      ? "Saved key is kept unless replaced"
                      : "API key"
                  }
                  type="password"
                  value={providerForm.apiKey}
                />
                {hasSavedProviderKey || providerForm.clearApiKey ? (
                  <button
                    aria-label="Clear saved API key"
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
                    title="Clear saved API key"
                    type="button"
                  >
                    <X aria-hidden="true" className="size-4" />
                  </button>
                ) : null}
                </span>
              </label>
              <button
                aria-label="Save provider"
                className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                disabled={
                  isSavingProvider ||
                  !providerForm.name.trim() ||
                  !providerForm.kind.trim()
                }
                title="Save provider"
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
                Configured providers
              </h3>
              <div className="flex gap-2">
                <button
                  aria-label="Add provider"
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={startAddingProvider}
                  title="Add provider"
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
                <button
                  aria-label="Reload settings"
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoadingSettings}
                  onClick={() => void loadSettings()}
                  title="Reload settings"
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
                              label={provider.enabled ? "enabled" : "disabled"}
                              ok={provider.enabled}
                            />
                            <CapabilityPill
                              label={provider.hasApiKey ? "key saved" : "key missing"}
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
                            aria-label={`Edit provider ${provider.name}`}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                            onClick={() => editConfiguredProvider(provider)}
                            title="Edit provider"
                            type="button"
                          >
                            <SlidersHorizontal aria-hidden="true" className="size-4" />
                          </button>
                          <button
                            aria-label={`Test provider ${provider.name}`}
                            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
                            disabled={test?.status === "testing"}
                            onClick={() => void testProvider(provider.id)}
                            title="Test provider"
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
                  No configured providers
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
              <h3 className="text-sm font-semibold text-stone-950">Models</h3>
              <div className="flex gap-2">
                <button
                  aria-label="Add model"
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  onClick={startAddingModel}
                  title="Add model"
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                </button>
              </div>
            </div>
            <div className="divide-y divide-stone-100">
              {configuredModels.length ? (
                configuredModels.map((model) => (
                <div
                  className="grid gap-3 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
                  key={model.id}
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-sm font-semibold">
                        {model.displayName}
                      </span>
                      <CapabilityPill
                        label={model.enabled ? "enabled" : "disabled"}
                        ok={model.enabled}
                      />
                      <CapabilityPill
                        label={model.canEnable ? "limits ok" : "limits missing"}
                        ok={model.canEnable}
                      />
                    </div>
                    <div className="mt-1 truncate text-xs font-medium text-stone-500">
                      {model.id}
                    </div>
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      <CapabilityPill
                        label={`providers ${model.providerIds.length}`}
                        ok={model.providerIds.length > 0}
                      />
                      <CapabilityPill
                        label={
                          model.activeProviderId
                            ? `active ${model.activeProviderId}`
                            : "active missing"
                        }
                        ok={model.activeProviderId !== null}
                      />
                    </div>
                  </div>
                  <button
                    aria-label={`Edit model ${model.displayName}`}
                    className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                    onClick={() => editConfiguredModel(model)}
                    title="Edit model"
                    type="button"
                  >
                    <SlidersHorizontal aria-hidden="true" className="size-4" />
                  </button>
                </div>
                ))
              ) : (
                <div className="px-4 py-6 text-sm text-stone-500">
                  No configured models
                </div>
              )}
            </div>
          </div>

          {isModelDialogOpen ? (
            <>
              <div className="fixed inset-0 z-40 bg-stone-950/35 backdrop-blur-sm" />
              <form
                aria-label="Model configuration"
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
                        {editingModel ? "Edit model" : "Add model"}
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
                        aria-label="Delete model"
                        className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 shadow-sm hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-400"
                        disabled={isSaving}
                        onClick={() => void deleteModel(editingModel.id)}
                        title="Delete model"
                        type="button"
                      >
                        <Trash2 aria-hidden="true" className="size-4" />
                      </button>
                    ) : null}
                    <button
                      aria-label="Close model configuration"
                      className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
                      onClick={() => setIsModelDialogOpen(false)}
                      title="Close"
                      type="button"
                    >
                      <X aria-hidden="true" className="size-4" />
                    </button>
                  </div>
                </div>
                <div className="space-y-3">
                  <TextField
                    label="Model id"
                    onChange={(value) =>
                      setForm((current) => ({ ...current, modelId: value }))
                    }
                    placeholder="gpt-5.5"
                    value={form.modelId}
                  />
                  <TextField
                    label="Display name"
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
                      label="Context window"
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
                      label="Max output tokens"
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
                      Enable model
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
                        Providers
                      </div>
                      <button
                        aria-label="Add provider"
                        className="inline-flex size-8 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        onClick={startAddingProviderFromModel}
                        title="Add provider"
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
                          <span>No providers</span>
                          <Plus aria-hidden="true" className="size-4" />
                        </button>
                      )}
                    </div>
                  </div>
                  <label className="block">
                    <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                      Active provider
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
                      <option value="">None</option>
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
                      Thinking level
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
                      <option value="">None</option>
                      {thinkingLevels.map((level) => (
                        <option key={level.value} value={level.value}>
                          {level.label}
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
                      Fill both limits before enabling.
                    </div>
                  ) : null}
                  <button
                    aria-label="Save model"
                    className="inline-flex h-11 w-full items-center justify-center rounded-lg bg-stone-950 text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:bg-stone-300"
                    disabled={
                      isSaving ||
                      enabledNeedsLimits ||
                      !form.modelId.trim() ||
                      !form.displayName.trim()
                    }
                    title="Save model"
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
                      pricing in/out:{" "}
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
                  placeholder="Search model metadata"
                  value={modelSearch}
                />
                <button
                  aria-label="Reload model metadata cache"
                  className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                  disabled={isLoading}
                  onClick={() => void loadMetadata()}
                  title="Reload cache"
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
                      {model.inputModalities.join(", ") || "input n/a"}
                    </span>
                  </button>
                ))
              ) : (
                <div className="px-4 py-8 text-sm text-stone-500">
                  {isLoading ? "Loading models..." : "No cached models"}
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
  icon: typeof MessageSquare;
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

function SettingsNavButton({
  active,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: typeof Settings;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className={`inline-flex size-10 items-center justify-center rounded-xl ${
        active
          ? "bg-teal-800 text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)]"
          : "text-stone-600 hover:bg-stone-100 hover:text-stone-950"
      }`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
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

function slugId(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
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
  const existingIndex = toolCalls.findIndex(
    (toolCall) => toolCall.id === nextToolCall.id,
  );

  if (existingIndex === -1) {
    return [...toolCalls, nextToolCall];
  }

  return toolCalls.map((toolCall, index) =>
    index === existingIndex ? nextToolCall : toolCall,
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

function toolStatusText(toolCall: ChatToolCallSummary) {
  if (toolCall.isError) {
    return "error";
  }

  return toolCall.status;
}

function formatJsonValue(value: JsonValue) {
  return JSON.stringify(value, null, 2);
}

function formatLimit(value: number | null, label: string) {
  return value === null ? `${label} missing` : `${label} ${formatNumber(value)}`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("en-US").format(value);
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

function terminalStatusText(status: "closed" | "connected" | "connecting" | "error") {
  if (status === "connected") {
    return "connected";
  }

  if (status === "connecting") {
    return "connecting";
  }

  if (status === "error") {
    return "error";
  }

  return "closed";
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
    if (!isChatStreamEvent(parsed)) {
      throw new Error("chat stream returned an unknown event");
    }

    onEvent(parsed);
  }

  return remaining;
}

function isChatStreamEvent(value: unknown): value is ChatStreamEvent {
  return (
    typeof value === "object" &&
    value !== null &&
    "type" in value &&
    typeof value.type === "string"
  );
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
