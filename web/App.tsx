import {
  Activity,
  BarChart3,
  Bot,
  ChevronDown,
  ChevronRight,
  Folder,
  FolderPlus,
  GitCompare,
  LoaderCircle,
  MessageSquare,
  Plus,
  RefreshCw,
  Send,
  Settings,
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

type WorkspaceFormMode = "add" | "create";
type ViewMode = "chat" | "settings" | "stats";

type ShellMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
};

const starterMessages: ShellMessage[] = [
  {
    id: "assistant-ready",
    role: "assistant",
    content: "Workspace shell is ready.",
  },
  {
    id: "user-example",
    role: "user",
    content: "Start from TODO.md step 4.",
  },
];

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
  const [messages, setMessages] = useState<ShellMessage[]>(starterMessages);
  const [isLoading, setIsLoading] = useState(true);
  const [isSavingWorkspace, setIsSavingWorkspace] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeWorkspace = useMemo(
    () =>
      workspaces.find((workspace) => workspace.id === activeWorkspaceId) ??
      workspaces[0],
    [activeWorkspaceId, workspaces],
  );

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

  useEffect(() => {
    void refreshWorkspaces();
  }, [refreshWorkspaces]);

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

  function handleSendMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const content = draftMessage.trim();
    if (!content) {
      return;
    }

    setMessages((current) => [
      ...current,
      {
        id: `local-user-${Date.now()}`,
        role: "user",
        content,
      },
    ]);
    setDraftMessage("");
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
                        onClick={() => setActiveWorkspaceId(workspace.id)}
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
              draftMessage={draftMessage}
              messages={messages}
              onDraftMessageChange={setDraftMessage}
              onSubmit={handleSendMessage}
            />
          ) : viewMode === "settings" ? (
            <PlaceholderPanel icon={Settings} title="Settings" />
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
  draftMessage,
  messages,
  onDraftMessageChange,
  onSubmit,
}: {
  draftMessage: string;
  messages: ShellMessage[];
  onDraftMessageChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
        <div className="mx-auto flex max-w-4xl flex-col gap-3">
          {messages.map((message) => {
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
                  <p className="min-w-0 whitespace-pre-wrap break-words text-sm leading-6">
                    {message.content}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className="border-t border-zinc-200 bg-white px-5 py-3">
        <form className="mx-auto max-w-4xl" onSubmit={onSubmit}>
          <div className="flex gap-2">
            <textarea
              className="min-h-20 flex-1 resize-none rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm leading-6 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => onDraftMessageChange(event.target.value)}
              placeholder="Message Foco"
              value={draftMessage}
            />
            <button
              className="inline-flex h-20 w-12 items-center justify-center rounded-md bg-teal-700 text-white transition hover:bg-teal-800 disabled:cursor-not-allowed disabled:bg-zinc-400"
              disabled={!draftMessage.trim()}
              title="Send"
              type="submit"
            >
              <Send aria-hidden="true" className="size-5" />
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
