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
  User,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

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

type ChatMessageSummary = {
  id: string;
  role: "assistant" | "user";
  content: string;
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
  | { type: "error"; message: string };

type WorkspaceFormMode = "add" | "create";
type ViewMode = "chat" | "settings" | "stats";

type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  status?: "error" | "streaming";
};

export function App() {
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>("");
  const [expandedWorkspaceIds, setExpandedWorkspaceIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [viewMode, setViewMode] = useState<ViewMode>("chat");
  const [formMode, setFormMode] = useState<WorkspaceFormMode>("create");
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspacePath, setWorkspacePath] = useState("");
  const [draftMessage, setDraftMessage] = useState("");
  const [messages, setMessages] = useState<ShellMessage[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [selectedThinkingLevel, setSelectedThinkingLevel] = useState("");
  const [isSendingMessage, setIsSendingMessage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  useEffect(() => {
    void refreshWorkspaces();
  }, [refreshWorkspaces]);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

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

    const localUserId = `local-user-${Date.now()}`;
    let assistantMessageId = `local-assistant-${Date.now()}`;

    setMessages((current) => [
      ...current,
      {
        id: localUserId,
        role: "user",
        content,
      },
      {
        id: assistantMessageId,
        role: "assistant",
        content: "",
        status: "streaming",
      },
    ]);
    setDraftMessage("");
    setIsSendingMessage(true);
    setError(null);

    try {
      const response = await fetch(
        `/api/workspaces/${encodeURIComponent(activeWorkspace.id)}/chat/stream`,
        {
          body: JSON.stringify({
            chatId: activeChatId,
            message: content,
            modelId: selectedModelId,
            thinkingLevel: selectedThinkingLevel || null,
          }),
          cache: "no-store",
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );

      if (!response.ok) {
        throw new Error(await responseErrorMessage(response));
      }

      await readChatStream(response, (streamEvent) => {
        if (streamEvent.type === "start") {
          assistantMessageId = streamEvent.assistantMessageId;
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
      const message = errorMessage(requestError);
      setError(message);
      setMessages((current) =>
        current.map((item) =>
          item.id === assistantMessageId
            ? { ...item, content: message, status: "error" }
            : item,
        ),
      );
    } finally {
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

  return (
    <main className="min-h-screen bg-zinc-100 text-zinc-950">
      <div className="grid min-h-screen grid-cols-1 lg:grid-cols-[18rem_minmax(0,1fr)_3.5rem]">
        <aside className="border-b border-zinc-200 bg-white lg:border-b-0 lg:border-r">
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between border-b border-zinc-200 px-4 py-3">
              <div className="flex min-w-0 items-center gap-2">
                <Activity aria-hidden="true" className="size-5 text-teal-700" />
                <span className="truncate text-base font-semibold">Foco</span>
              </div>
              <button
                className="inline-flex size-8 items-center justify-center rounded-md border border-zinc-200 bg-white text-zinc-700 transition hover:bg-zinc-50"
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

            <div className="flex gap-2 border-b border-zinc-200 px-4 py-3">
              <button
                className={workspaceModeClass(formMode === "create")}
                onClick={() => setFormMode("create")}
                type="button"
              >
                <Plus aria-hidden="true" className="size-4" />
                New
              </button>
              <button
                className={workspaceModeClass(formMode === "add")}
                onClick={() => setFormMode("add")}
                type="button"
              >
                <FolderPlus aria-hidden="true" className="size-4" />
                Add
              </button>
            </div>

            <form
              className="space-y-2 border-b border-zinc-200 px-4 py-3"
              onSubmit={(event) => void handleWorkspaceSubmit(event)}
            >
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-zinc-600">
                  Name
                </span>
                <input
                  className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => setWorkspaceName(event.target.value)}
                  placeholder="Workspace name"
                  value={workspaceName}
                />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-zinc-600">
                  Path
                </span>
                <input
                  className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => setWorkspacePath(event.target.value)}
                  placeholder="C:\\Users\\name\\workspace"
                  value={workspacePath}
                />
              </label>
              <button
                className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-teal-700 px-3 text-sm font-medium text-white transition hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
                disabled={isSavingWorkspace}
                type="submit"
              >
                {isSavingWorkspace ? (
                  <LoaderCircle
                    aria-hidden="true"
                    className="size-4 animate-spin"
                  />
                ) : formMode === "create" ? (
                  <Plus aria-hidden="true" className="size-4" />
                ) : (
                  <FolderPlus aria-hidden="true" className="size-4" />
                )}
                {formMode === "create" ? "Create Workspace" : "Add Workspace"}
              </button>
            </form>

            {error ? (
              <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
                {error}
              </div>
            ) : null}

            <nav className="min-h-0 flex-1 overflow-y-auto px-2 py-3">
              {workspaces.map((workspace) => {
                const isExpanded = expandedWorkspaceIds.has(workspace.id);
                const isActive = workspace.id === activeWorkspace?.id;

                return (
                  <div className="mb-1" key={workspace.id}>
                    <div className="flex items-center gap-1">
                      <button
                        className="inline-flex size-8 items-center justify-center rounded-md text-zinc-500 transition hover:bg-zinc-100"
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
                    </div>
                    {isExpanded ? (
                      <div className="ml-9 mt-1 space-y-1">
                        {workspace.chats.length > 0 ? (
                          workspace.chats.map((chat) => (
                            <button
                              className="flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-zinc-600 transition hover:bg-zinc-100 hover:text-zinc-950"
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
                          <div className="rounded-md px-2 py-1.5 text-xs text-zinc-500">
                            No chats
                          </div>
                        )}
                      </div>
                    ) : null}
                  </div>
                );
              })}
            </nav>
          </div>
        </aside>

        <section className="flex min-h-screen min-w-0 flex-col bg-zinc-50">
          <header className="border-b border-zinc-200 bg-white px-5 py-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <h1 className="truncate text-base font-semibold">
                  {activeWorkspace?.name ?? "Workspace"}
                </h1>
                <p className="mt-1 truncate text-xs text-zinc-500">
                  {activeWorkspace?.path ?? ""}
                </p>
              </div>
              <div className="flex rounded-md border border-zinc-200 bg-zinc-50 p-1">
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
                  label="AI Stats"
                  onClick={() => setViewMode("stats")}
                />
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
              onModelChange={setSelectedModelId}
              onSubmit={handleSendMessage}
              onThinkingLevelChange={setSelectedThinkingLevel}
              selectedModelId={selectedModelId}
              selectedThinkingLevel={selectedThinkingLevel}
              thinkingLevels={thinkingLevels}
            />
          ) : viewMode === "settings" ? (
            <SettingsPanel />
          ) : (
            <PlaceholderPanel icon={BarChart3} title="AI Statistics" />
          )}
        </section>

        <aside className="flex min-h-14 items-center justify-center border-t border-zinc-200 bg-white lg:min-h-screen lg:border-l lg:border-t-0">
          <button
            className="inline-flex h-10 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50 lg:h-auto lg:w-10 lg:flex-col lg:px-0 lg:py-3"
            title="Git diff"
            type="button"
          >
            <GitCompare aria-hidden="true" className="size-4" />
            <span className="lg:[writing-mode:vertical-rl]">Diff</span>
          </button>
        </aside>
      </div>
    </main>
  );
}

function ChatPanel({
  availableModels,
  draftMessage,
  isLoadingSettings,
  isSendingMessage,
  messages,
  onDraftMessageChange,
  onModelChange,
  onSubmit,
  onThinkingLevelChange,
  selectedModelId,
  selectedThinkingLevel,
  thinkingLevels,
}: {
  availableModels: ConfiguredModelSummary[];
  draftMessage: string;
  isLoadingSettings: boolean;
  isSendingMessage: boolean;
  messages: ShellMessage[];
  onDraftMessageChange: (value: string) => void;
  onModelChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onThinkingLevelChange: (value: string) => void;
  selectedModelId: string;
  selectedThinkingLevel: string;
  thinkingLevels: ThinkingLevelSummary[];
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
        <div className="mx-auto flex max-w-4xl flex-col gap-3">
          {messages.length ? (
            messages.map((message) => {
            const isUser = message.role === "user";

            return (
              <div
                className={`flex ${isUser ? "justify-end" : "justify-start"}`}
                key={message.id}
              >
                <div
                  className={`flex max-w-[78%] gap-3 rounded-md border px-4 py-3 shadow-sm ${
                    isUser
                      ? "border-teal-200 bg-teal-700 text-white"
                      : "border-zinc-200 bg-white text-zinc-900"
                  }`}
                >
                  <div
                    className={`mt-0.5 inline-flex size-7 shrink-0 items-center justify-center rounded-md ${
                      isUser
                        ? "bg-teal-800 text-white"
                        : "bg-zinc-100 text-zinc-700"
                    }`}
                  >
                    {isUser ? (
                      <User aria-hidden="true" className="size-4" />
                    ) : (
                      <Bot aria-hidden="true" className="size-4" />
                    )}
                  </div>
                  <p
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
                  </p>
                </div>
              </div>
            );
            })
          ) : (
            <div className="flex justify-start">
              <div className="flex max-w-[78%] gap-3 rounded-md border border-zinc-200 bg-white px-4 py-3 text-zinc-600 shadow-sm">
                <div className="mt-0.5 inline-flex size-7 shrink-0 items-center justify-center rounded-md bg-zinc-100 text-zinc-700">
                  <Bot aria-hidden="true" className="size-4" />
                </div>
                <p className="min-w-0 whitespace-pre-wrap break-words text-sm leading-6">
                  Workspace shell is ready.
                </p>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="border-t border-zinc-200 bg-white px-5 py-3">
        <form className="mx-auto max-w-4xl" onSubmit={onSubmit}>
          <div className="mb-2 grid gap-2 sm:grid-cols-[minmax(0,1fr)_12rem]">
            <label className="block">
              <span className="mb-1 block text-xs font-medium text-zinc-600">
                Model
              </span>
              <select
                className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
            <label className="block">
              <span className="mb-1 block text-xs font-medium text-zinc-600">
                Thinking
              </span>
              <select
                className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
          </div>
          <div className="flex gap-2">
            <textarea
              className="min-h-20 flex-1 resize-none rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm leading-6 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              disabled={isSendingMessage}
              onChange={(event) => onDraftMessageChange(event.target.value)}
              placeholder="Message Foco"
              value={draftMessage}
            />
            <button
              className="inline-flex h-20 w-12 items-center justify-center rounded-md bg-teal-700 text-white transition hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
              disabled={
                isSendingMessage || !draftMessage.trim() || !selectedModelId
              }
              title="Send"
              type="submit"
            >
              {isSendingMessage ? (
                <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
              ) : (
                <Send aria-hidden="true" className="size-5" />
              )}
            </button>
          </div>
        </form>
        <button
          className="mx-auto mt-3 flex h-9 w-full max-w-4xl items-center justify-between rounded-md border border-zinc-200 bg-zinc-50 px-3 text-sm font-medium text-zinc-700 transition hover:bg-zinc-100"
          type="button"
        >
          <span className="inline-flex items-center gap-2">
            <Terminal aria-hidden="true" className="size-4" />
            Terminal
          </span>
          <ChevronRight aria-hidden="true" className="size-4" />
        </button>
      </div>
    </div>
  );
}

function PlaceholderPanel({
  icon: Icon,
  title,
}: {
  icon: typeof Settings;
  title: string;
}) {
  return (
    <div className="grid min-h-0 flex-1 place-items-center p-6">
      <div className="flex items-center gap-3 rounded-md border border-zinc-200 bg-white px-4 py-3 text-zinc-700 shadow-sm">
        <Icon aria-hidden="true" className="size-5 text-teal-700" />
        <span className="text-sm font-medium">{title}</span>
      </div>
    </div>
  );
}

function SettingsPanel() {
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
  const selectedProviderKind = providerKinds.find(
    (kind) => kind.kind === providerForm.kind,
  );
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
      modelId: model.key,
      contextWindow: numberInputValue(model.contextWindow),
      maxOutputTokens: numberInputValue(model.maxOutputTokens),
      providerIds: [],
      activeProviderId: "",
      thinkingLevel: "",
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
            id: providerForm.id,
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
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsSavingProvider(false);
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
    <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
      <div className="mx-auto flex max-w-6xl flex-col gap-4">
        <section className="border-b border-zinc-200 pb-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <h2 className="text-base font-semibold">
                Provider And Model Settings
              </h2>
              <p className="mt-1 truncate text-xs text-zinc-500">
                {metadata?.fetchedAt
                  ? `Fetched ${metadata.fetchedAt} from ${metadata.sourceUrl}`
                  : `Cache path: ${metadata?.cachePath ?? ""}`}
              </p>
            </div>
            <button
              className="inline-flex h-9 items-center gap-2 rounded-md bg-teal-700 px-3 text-sm font-medium text-white transition hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
              disabled={isRefreshing}
              onClick={() => void refreshMetadata()}
              type="button"
            >
              {isRefreshing ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <RefreshCw aria-hidden="true" className="size-4" />
              )}
              Refresh
            </button>
          </div>
        </section>

        {error ? (
          <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        <section className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
          <form
            className="rounded-md border border-zinc-200 bg-white px-4 py-4"
            onSubmit={(event) => void saveProvider(event)}
          >
            <div className="mb-4 flex items-center gap-2">
              <PlugZap aria-hidden="true" className="size-5 text-teal-700" />
              <h3 className="text-sm font-semibold">Provider</h3>
            </div>
            <div className="space-y-3">
              <div className="grid gap-3 sm:grid-cols-2">
                <TextField
                  label="Provider ID"
                  onChange={(value) =>
                    setProviderForm((current) => ({ ...current, id: value }))
                  }
                  placeholder="openai"
                  value={providerForm.id}
                />
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
              </div>
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-zinc-600">
                  Kind
                </span>
                <select
                  className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
              <TextField
                label="API Key"
                onChange={(value) =>
                  setProviderForm((current) => ({ ...current, apiKey: value }))
                }
                placeholder="Leave blank to keep saved key"
                value={providerForm.apiKey}
              />
              <label className="flex items-center justify-between gap-3 rounded-md border border-zinc-200 bg-zinc-50 px-3 py-2">
                <span className="text-sm font-medium text-zinc-700">
                  Enable provider
                </span>
                <input
                  checked={providerForm.enabled}
                  className="size-4 accent-teal-700"
                  onChange={(event) =>
                    setProviderForm((current) => ({
                      ...current,
                      enabled: event.target.checked,
                    }))
                  }
                  type="checkbox"
                />
              </label>
              <label className="flex items-center justify-between gap-3 rounded-md border border-zinc-200 bg-white px-3 py-2">
                <span className="text-sm font-medium text-zinc-700">
                  Clear saved API key
                </span>
                <input
                  checked={providerForm.clearApiKey}
                  className="size-4 accent-teal-700"
                  onChange={(event) =>
                    setProviderForm((current) => ({
                      ...current,
                      clearApiKey: event.target.checked,
                    }))
                  }
                  type="checkbox"
                />
              </label>
              <button
                className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-zinc-900 px-3 text-sm font-medium text-white transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
                disabled={
                  isSavingProvider ||
                  !providerForm.id.trim() ||
                  !providerForm.name.trim() ||
                  !providerForm.kind.trim()
                }
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
                Save Provider
              </button>
            </div>
          </form>

          <section className="rounded-md border border-zinc-200 bg-white">
            <div className="flex items-center justify-between gap-3 border-b border-zinc-200 px-4 py-3">
              <h3 className="text-sm font-semibold">Configured Providers</h3>
              <button
                className="inline-flex size-8 items-center justify-center rounded-md border border-zinc-200 bg-white text-zinc-700 transition hover:bg-zinc-50"
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
            <div className="divide-y divide-zinc-100">
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
                          <div className="mt-1 truncate text-xs text-zinc-500">
                            {provider.id} / {provider.kindLabel}
                          </div>
                          {provider.baseUrl ? (
                            <div className="mt-1 truncate text-xs text-zinc-500">
                              {provider.baseUrl}
                            </div>
                          ) : null}
                        </div>
                        <div className="flex flex-wrap gap-2">
                          <button
                            className="inline-flex h-8 items-center justify-center rounded-md border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50"
                            onClick={() => editConfiguredProvider(provider)}
                            type="button"
                          >
                            Edit
                          </button>
                          <button
                            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50 disabled:cursor-not-allowed disabled:bg-zinc-100"
                            disabled={test?.status === "testing"}
                            onClick={() => void testProvider(provider.id)}
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
                            Test
                          </button>
                        </div>
                      </div>
                      {test ? (
                        <div
                          className={`mt-3 rounded-md border px-3 py-2 text-sm ${
                            test.status === "ok"
                              ? "border-teal-200 bg-teal-50 text-teal-800"
                              : test.status === "testing"
                                ? "border-zinc-200 bg-zinc-50 text-zinc-600"
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
                <div className="px-4 py-6 text-sm text-zinc-500">
                  No configured providers
                </div>
              )}
            </div>
          </section>
        </section>

        <section className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(22rem,0.9fr)]">
          <div className="min-w-0 rounded-md border border-zinc-200 bg-white">
            <div className="border-b border-zinc-200 px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <input
                  className="h-9 min-w-0 flex-1 rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => setModelSearch(event.target.value)}
                  placeholder="Search provider or model"
                  value={modelSearch}
                />
                <button
                  className="inline-flex size-9 items-center justify-center rounded-md border border-zinc-200 bg-white text-zinc-700 transition hover:bg-zinc-50"
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
            <div className="max-h-[34rem] overflow-y-auto">
              {filteredModels.length > 0 ? (
                filteredModels.map((model) => (
                  <button
                    className={`grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-zinc-100 px-4 py-3 text-left transition hover:bg-zinc-50 ${
                      selectedMetadataKey === model.key ? "bg-teal-50" : "bg-white"
                    }`}
                    key={model.key}
                    onClick={() => selectMetadataModel(model.key)}
                    type="button"
                  >
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-medium text-zinc-950">
                        {model.name}
                      </span>
                      <span className="mt-1 block truncate text-xs text-zinc-500">
                        {model.providerName} / {model.modelId}
                      </span>
                      <span className="mt-2 flex flex-wrap gap-1.5">
                        <CapabilityPill
                          label={formatLimit(model.contextWindow, "ctx")}
                          ok={model.contextWindow !== null}
                        />
                        <CapabilityPill
                          label={formatLimit(model.maxOutputTokens, "out")}
                          ok={model.maxOutputTokens !== null}
                        />
                        <CapabilityPill label="tools" ok={model.supportsTools} />
                        <CapabilityPill label="cache" ok={model.supportsCache} />
                      </span>
                    </span>
                    <span className="text-right text-xs text-zinc-500">
                      {model.inputModalities.join(", ") || "input n/a"}
                    </span>
                  </button>
                ))
              ) : (
                <div className="px-4 py-8 text-sm text-zinc-500">
                  {isLoading ? "Loading models..." : "No cached models"}
                </div>
              )}
            </div>
          </div>

          <form
            className="rounded-md border border-zinc-200 bg-white px-4 py-4"
            onSubmit={(event) => void saveModel(event)}
          >
            <div className="mb-4 flex items-center gap-2">
              <SlidersHorizontal
                aria-hidden="true"
                className="size-5 text-teal-700"
              />
              <h3 className="text-sm font-semibold">Model Limits</h3>
            </div>
            <div className="space-y-3">
              <TextField
                label="Model ID"
                onChange={(value) =>
                  setForm((current) => ({ ...current, modelId: value }))
                }
                placeholder="openai/gpt-4.1"
                value={form.modelId}
              />
              <TextField
                label="Display Name"
                onChange={(value) =>
                  setForm((current) => ({ ...current, displayName: value }))
                }
                placeholder="GPT 4.1"
                value={form.displayName}
              />
              <div className="grid gap-3 sm:grid-cols-2">
                <TextField
                  inputMode="numeric"
                  label="Context Window"
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
                  label="Max Output Tokens"
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
              <label className="flex items-center justify-between gap-3 rounded-md border border-zinc-200 bg-zinc-50 px-3 py-2">
                <span className="text-sm font-medium text-zinc-700">
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
              <div className="rounded-md border border-zinc-200 px-3 py-3">
                <div className="mb-2 text-xs font-medium text-zinc-600">
                  Providers
                </div>
                <div className="space-y-2">
                  {providers.length ? (
                    providers.map((provider) => (
                      <label
                        className="flex items-center justify-between gap-3 rounded-md bg-zinc-50 px-3 py-2"
                        key={provider.id}
                      >
                        <span className="min-w-0">
                          <span className="block truncate text-sm font-medium text-zinc-700">
                            {provider.name}
                          </span>
                          <span className="block truncate text-xs text-zinc-500">
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
                    <div className="rounded-md bg-zinc-50 px-3 py-2 text-sm text-zinc-500">
                      No providers
                    </div>
                  )}
                </div>
              </div>
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-zinc-600">
                  Active Provider
                </span>
                <select
                  className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
                <span className="mb-1 block text-xs font-medium text-zinc-600">
                  Thinking Level
                </span>
                <select
                  className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
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
                <div className="flex items-center gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                  <CircleAlert aria-hidden="true" className="size-4 shrink-0" />
                  Fill both limits before enabling.
                </div>
              ) : null}
              <button
                className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-zinc-900 px-3 text-sm font-medium text-white transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
                disabled={
                  isSaving ||
                  enabledNeedsLimits ||
                  !form.modelId.trim() ||
                  !form.displayName.trim()
                }
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
                Save Model
              </button>
            </div>

            {selectedMetadata ? (
              <div className="mt-4 border-t border-zinc-200 pt-4 text-xs text-zinc-500">
                <div className="truncate">{selectedMetadata.key}</div>
                <div className="mt-1">
                  pricing in/out: {priceText(selectedMetadata.pricing.input)} /{" "}
                  {priceText(selectedMetadata.pricing.output)}
                </div>
              </div>
            ) : null}
          </form>
        </section>

        <section className="rounded-md border border-zinc-200 bg-white">
          <div className="border-b border-zinc-200 px-4 py-3">
            <h3 className="text-sm font-semibold">Configured Models</h3>
          </div>
          <div className="divide-y divide-zinc-100">
            {configuredModels.length ? (
              configuredModels.map((model) => (
                <div
                  className="grid gap-3 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
                  key={model.id}
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-sm font-medium">
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
                      <CapabilityPill
                        label={model.supportsThinking ? "thinking ready" : "thinking unknown"}
                        ok={model.supportsThinking}
                      />
                    </div>
                    <div className="mt-1 truncate text-xs text-zinc-500">
                      {model.id}
                    </div>
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      <CapabilityPill
                        label={formatLimit(model.contextWindow, "ctx")}
                        ok={model.contextWindow !== null}
                      />
                      <CapabilityPill
                        label={formatLimit(model.maxOutputTokens, "out")}
                        ok={model.maxOutputTokens !== null}
                      />
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
                      <CapabilityPill
                        label={
                          model.thinkingLevel
                            ? `thinking ${model.thinkingLevel}`
                            : "thinking none"
                        }
                        ok={model.thinkingLevel !== null}
                      />
                    </div>
                    <Warnings warnings={model.warnings} />
                  </div>
                  <button
                    className="inline-flex h-8 items-center justify-center rounded-md border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50"
                    onClick={() => editConfiguredModel(model)}
                    type="button"
                  >
                    Edit
                  </button>
                </div>
              ))
            ) : (
              <div className="px-4 py-6 text-sm text-zinc-500">
                No configured models
              </div>
            )}
          </div>
        </section>
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
      className={`inline-flex h-8 items-center gap-2 rounded-md px-3 text-sm font-medium transition ${
        active
          ? "bg-white text-teal-800 shadow-sm"
          : "text-zinc-600 hover:text-zinc-950"
      }`}
      onClick={onClick}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
      <span className="hidden sm:inline">{label}</span>
    </button>
  );
}

function workspaceModeClass(active: boolean) {
  return `inline-flex h-8 flex-1 items-center justify-center gap-2 rounded-md border px-2 text-sm font-medium transition ${
    active
      ? "border-teal-200 bg-teal-50 text-teal-800"
      : "border-zinc-200 bg-white text-zinc-600 hover:bg-zinc-50 hover:text-zinc-950"
  }`;
}

function workspaceItemClass(active: boolean) {
  return `flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-sm font-medium transition ${
    active
      ? "bg-zinc-900 text-white"
      : "text-zinc-700 hover:bg-zinc-100 hover:text-zinc-950"
  }`;
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
      <span className="mb-1 block text-xs font-medium text-zinc-600">
        {label}
      </span>
      <input
        className="h-9 w-full rounded-md border border-zinc-300 bg-white px-3 text-sm outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        inputMode={inputMode}
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
      className={`inline-flex h-6 items-center rounded-md border px-2 text-xs font-medium ${
        ok
          ? "border-teal-200 bg-teal-50 text-teal-800"
          : "border-zinc-200 bg-zinc-50 text-zinc-500"
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
          className="flex items-center gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800"
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

function formatLimit(value: number | null, label: string) {
  return value === null ? `${label} missing` : `${label} ${formatNumber(value)}`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("en-US").format(value);
}

function priceText(value: number | null) {
  return value === null ? "n/a" : `$${value}`;
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
