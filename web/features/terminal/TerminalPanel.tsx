import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Play, Plus, Terminal, X } from "lucide-react";
import {
  MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";

import type {
  TerminalPanelSession,
  TerminalPaneStatus,
  TerminalServerEvent,
  TerminalSessionResponse,
  Translate,
  WorkspaceCommonCommandSummary,
  WorkspaceSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";

function TerminalCommandButton({
  commands,
  disabled,
  onRun,
}: {
  commands: WorkspaceCommonCommandSummary[];
  disabled: boolean;
  onRun: (command: WorkspaceCommonCommandSummary) => void;
}) {
  const { t } = useI18n();
  const detailsRef = useCloseDetailsOnOutsidePointerDown();

  if (!commands.length) {
    return null;
  }

  if (commands.length === 1) {
    const command = commands[0];
    return (
      <button
        aria-label={t("Run common command {name}", { name: command.name })}
        className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 hover:bg-stone-800 hover:text-stone-100 disabled:cursor-not-allowed disabled:text-stone-600 disabled:hover:bg-transparent"
        disabled={disabled}
        onClick={() => onRun(command)}
        title={t("Run common command {name}", { name: command.name })}
        type="button"
      >
        <Play aria-hidden="true" className="size-3.5 fill-current" />
      </button>
    );
  }

  function handleSelect(
    command: WorkspaceCommonCommandSummary,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    event.currentTarget.closest("details")?.removeAttribute("open");
    onRun(command);
  }

  return (
    <details className="relative" ref={detailsRef}>
      <summary
        aria-disabled={disabled}
        aria-label={t("Run common command")}
        className={`inline-flex size-6 cursor-pointer list-none items-center justify-center rounded-md text-stone-400 outline-none marker:hidden focus-visible:ring-2 focus-visible:ring-teal-500/60 [&::-webkit-details-marker]:hidden ${disabled
            ? "pointer-events-none text-stone-600"
            : "hover:bg-stone-800 hover:text-stone-100"
          }`}
        title={t("Run common command")}
      >
        <Play aria-hidden="true" className="size-3.5 fill-current" />
      </summary>
      <div className="absolute right-0 top-full z-30 mt-2 w-56 overflow-hidden rounded-lg border border-stone-800 bg-stone-950 shadow-[0_18px_40px_rgba(0,0,0,0.28)]">
        <div className="panel-scroll max-h-56 overflow-y-auto py-1">
          {commands.map((command, index) => (
            <button
              aria-label={t("Run common command {name}", { name: command.name })}
              className="flex min-h-9 w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-xs font-semibold text-stone-200 hover:bg-stone-800"
              key={`${command.name}-${index}`}
              onClick={(event) => handleSelect(command, event)}
              type="button"
            >
              <Play
                aria-hidden="true"
                className="size-3.5 shrink-0 fill-current text-teal-400"
              />
              <span className="min-w-0 flex-1 truncate">{command.name}</span>
            </button>
          ))}
        </div>
      </div>
    </details>
  );
}

export function TerminalPanel({
  errorMessage,
  isVisible,
  onClose,
  requestJson,
  workspace,
}: {
  errorMessage: (value: unknown) => string;
  isVisible: boolean;
  onClose: () => void;
  requestJson: <T>(path: string, init?: RequestInit) => Promise<T>;
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
  const commonCommands = workspace?.commonCommands ?? [];
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

  function runWorkspaceCommonCommand(command: WorkspaceCommonCommandSummary) {
    if (!activeSession || !workspace) {
      return;
    }

    updateSession(activeSession.clientId, {
      pendingCommand: {
        input: terminalCommandInput(
          workspace.terminalShell,
          workspace.path,
          command.command,
        ),
      },
    });
  }

  return (
    <section
      aria-hidden={!isVisible}
      className="terminal-panel relative shrink-0 border-t border-stone-800 bg-[#16130f]"
      hidden={!isVisible}
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
              <TerminalCommandButton
                commands={commonCommands}
                disabled={!activeSession || !isTerminalConnected(activeSession.status)}
                onRun={runWorkspaceCommonCommand}
              />
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
                errorMessage={errorMessage}
                markClosed={markSessionClosed}
                onUpdate={updateSession}
                requestJson={requestJson}
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
                className={`flex w-full min-w-0 items-center gap-1 rounded-md text-xs ${session.clientId === activeSession?.clientId
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
                    className={`size-2 shrink-0 rounded-full ${isTerminalConnected(session.status)
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
  errorMessage,
  isActive,
  markClosed,
  onUpdate,
  requestJson,
  session,
  workspaceId,
}: {
  errorMessage: (value: unknown) => string;
  isActive: boolean;
  markClosed: (clientId: string) => void;
  onUpdate: (
    clientId: string,
    patch: Partial<Omit<TerminalPanelSession, "clientId">>,
  ) => void;
  requestJson: <T>(path: string, init?: RequestInit) => Promise<T>;
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

  useEffect(() => {
    const pendingCommand = session.pendingCommand;
    if (!pendingCommand) {
      return;
    }

    const socket = socketRef.current;
    if (socket?.readyState !== WebSocket.OPEN) {
      onUpdate(clientId, {
        error: tRef.current("Terminal is not connected."),
        pendingCommand: null,
      });
      return;
    }

    socket.send(JSON.stringify({ type: "input", data: pendingCommand.input }));
    onUpdate(clientId, { pendingCommand: null });
  }, [clientId, onUpdate, session.pendingCommand]);

  return (
    <div
      aria-hidden={!isActive}
      className={`terminal-session-pane absolute inset-0 min-h-0 min-w-0 p-2 ${isActive ? "" : "pointer-events-none opacity-0"
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
    pendingCommand: null,
    serverSessionId: null,
    status: "closed",
  };
}

function terminalCommandInput(
  terminalShell: string,
  workspacePath: string,
  command: string,
) {
  const commandInput =
    command.endsWith("\n") || command.endsWith("\r") ? command : `${command}\r`;
  return `${terminalCdCommand(terminalShell, workspacePath)}\r${commandInput}`;
}

function terminalCdCommand(terminalShell: string, workspacePath: string) {
  if (terminalShell === "powershell") {
    return `Set-Location -LiteralPath '${quotePowerShellSingle(workspacePath)}'`;
  }

  if (terminalShell === "cmd") {
    return `cd /d "${workspacePath.replaceAll('"', '""')}"`;
  }

  return `cd -- '${quotePosixSingle(workspacePath)}'`;
}

function quotePowerShellSingle(value: string) {
  return value.replaceAll("'", "''");
}

function quotePosixSingle(value: string) {
  return value.replaceAll("'", "'\\''");
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
    return `${base} bg-emerald-100 text-emerald-800`;
  }

  if (status === "connecting") {
    return `${base} bg-amber-100 text-amber-800`;
  }

  if (status === "error") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  return `${base} bg-stone-100 text-stone-500`;
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



