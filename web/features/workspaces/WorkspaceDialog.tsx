import {
  CheckCircle2,
  FolderPlus,
  FolderSearch,
  LoaderCircle,
  ScrollText,
  Server,
  Terminal,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { ChangeEvent as ReactChangeEvent, FormEvent } from "react";

import type {
  RemoteServerDiagnosticStage,
  RemoteServerSummary,
  WorkspaceIconDraft,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";
import { WorkspaceIcon } from "./WorkspaceIcon";

export function WorkspaceDialog({
  canUseNativePicker,
  iconDraft,
  iconInputRef,
  inlineServerHost,
  inlineServerName,
  isCreatingInlineServer,
  isSelectingPath,
  isSaving,
  isTestingConnection,
  mode,
  name,
  onClearIcon,
  onClose,
  onCreateInlineServer,
  onIconFileChange,
  onInlineServerHostChange,
  onInlineServerNameChange,
  onModeChange,
  onNameChange,
  onPathChange,
  onSelectPath,
  onServerChange,
  onSpecEnabledChange,
  onSubmit,
  onTerminalShellChange,
  onTestConnection,
  path,
  remoteServers,
  selectedServerId,
  specEnabled,
  terminalShell,
  testStages,
}: {
  canUseNativePicker: boolean;
  iconDraft: WorkspaceIconDraft | null;
  iconInputRef: { current: HTMLInputElement | null };
  inlineServerHost: string;
  inlineServerName: string;
  isCreatingInlineServer: boolean;
  isSelectingPath: boolean;
  isSaving: boolean;
  isTestingConnection: boolean;
  mode: "local" | "ssh";
  name: string;
  onClearIcon: () => void;
  onClose: () => void;
  onCreateInlineServer: () => void;
  onIconFileChange: (event: ReactChangeEvent<HTMLInputElement>) => void;
  onInlineServerHostChange: (value: string) => void;
  onInlineServerNameChange: (value: string) => void;
  onModeChange: (mode: "local" | "ssh") => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSelectPath: () => void;
  onServerChange: (serverId: string) => void;
  onSpecEnabledChange: (enabled: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onTerminalShellChange: (value: string) => void;
  onTestConnection: () => void;
  path: string;
  remoteServers: RemoteServerSummary[];
  selectedServerId: string;
  specEnabled: boolean;
  terminalShell: string;
  testStages: RemoteServerDiagnosticStage[];
}) {
  const { t } = useI18n();
  const title = t("Add workspace");
  const selectedServer = remoteServers.find((server) => server.id === selectedServerId) ?? null;
  const isRemote = mode === "ssh";

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="workspace-dialog-title"
        aria-modal="true"
        className="panel-scroll max-h-[88vh] w-full max-w-xl overflow-y-auto rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
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
              {isRemote ? t("Register an SSH workspace.") : t("Create or register a local folder.")}
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

        <form className="space-y-4 px-4 py-4" onSubmit={(event) => void onSubmit(event)}>
          <div className="grid grid-cols-2 gap-2 rounded-lg bg-stone-100 p-1">
            <button
              aria-pressed={!isRemote}
              className={`inline-flex h-10 items-center justify-center gap-2 rounded-md text-sm font-semibold ${!isRemote
                  ? "bg-white text-stone-950 shadow-sm"
                  : "text-stone-500 hover:text-stone-800"
                }`}
              onClick={() => onModeChange("local")}
              type="button"
            >
              <FolderPlus aria-hidden="true" className="size-4" />
              {t("Local")}
            </button>
            <button
              aria-pressed={isRemote}
              className={`inline-flex h-10 items-center justify-center gap-2 rounded-md text-sm font-semibold ${isRemote
                  ? "bg-white text-stone-950 shadow-sm"
                  : "text-stone-500 hover:text-stone-800"
                }`}
              onClick={() => onModeChange("ssh")}
              type="button"
            >
              <Server aria-hidden="true" className="size-4" />
              {t("SSH")}
            </button>
          </div>

          {isRemote ? (
            <section className="grid gap-3 rounded-lg border border-stone-200 bg-stone-50/70 p-3">
              <label className="block">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("Remote Server")}
                </span>
                <select
                  className="h-11 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => onServerChange(event.target.value)}
                  value={selectedServerId}
                >
                  <option value="">{t("Select remote server")}</option>
                  {remoteServers.map((server) => (
                    <option key={server.id} value={server.id}>
                      {server.name} ({server.hostAlias})
                    </option>
                  ))}
                </select>
              </label>
              {selectedServer ? (
                <div className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-600">
                  <span className="min-w-0 truncate">
                    {selectedServer.name}: {selectedServer.defaultRemoteRoot ?? selectedServer.hostAlias}
                  </span>
                  <span className={`size-2.5 shrink-0 rounded-full ${remoteDialogStatusDotClass(selectedServer.status)}`} />
                </div>
              ) : null}
              <div className="grid gap-2 sm:grid-cols-2">
                <input
                  autoComplete="off"
                  className="h-10 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => onInlineServerNameChange(event.target.value)}
                  placeholder={t("Server name")}
                  value={inlineServerName}
                />
                <div className="flex gap-2">
                  <input
                    autoComplete="off"
                    className="h-10 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                    onChange={(event) => onInlineServerHostChange(event.target.value)}
                    placeholder={t("SSH host alias")}
                    value={inlineServerHost}
                  />
                  <button
                    aria-label={t("Add remote server")}
                    className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                    disabled={isCreatingInlineServer}
                    onClick={onCreateInlineServer}
                    title={t("Add remote server")}
                    type="button"
                  >
                    {isCreatingInlineServer ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <Server aria-hidden="true" className="size-4" />
                    )}
                  </button>
                </div>
              </div>
            </section>
          ) : null}

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
              {isRemote ? t("Remote path") : t("Path")}
            </span>
            <div className="flex gap-2">
              <input
                autoComplete="off"
                className="h-11 min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                name="workspace-path"
                onChange={(event) => onPathChange(event.target.value)}
                placeholder={isRemote ? "/home/name/workspace" : "C:/Users/name/workspace"}
                value={path}
              />
              {!isRemote ? (
                <button
                  aria-label={t("Choose workspace path")}
                  className="inline-flex size-11 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                  disabled={isSelectingPath || !canUseNativePicker}
                  onClick={onSelectPath}
                  title={
                    canUseNativePicker
                      ? t("Choose workspace path")
                      : t("Local Foco browser required")
                  }
                  type="button"
                >
                  {isSelectingPath ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <FolderSearch aria-hidden="true" className="size-4" />
                  )}
                </button>
              ) : null}
            </div>
          </label>

          {isRemote ? (
            <div className="rounded-lg border border-stone-200 bg-white p-3">
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-700">
                  <Terminal aria-hidden="true" className="size-4 shrink-0 text-teal-700" />
                  <span className="truncate">{t("Test connection")}</span>
                </div>
                <button
                  aria-label={t("Test connection")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
                  disabled={isTestingConnection || !selectedServerId}
                  onClick={onTestConnection}
                  title={t("Test connection")}
                  type="button"
                >
                  {isTestingConnection ? (
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                  ) : (
                    <CheckCircle2 aria-hidden="true" className="size-4" />
                  )}
                </button>
              </div>
              {testStages.length ? (
                <div className="mt-3 grid gap-1.5 text-xs text-stone-600">
                  {testStages.map((stage) => (
                    <div key={stage.stage} className="flex items-center gap-2">
                      <span className={`size-2 rounded-full ${remoteDialogStageDotClass(stage.status)}`} />
                      <span className="font-medium text-stone-700">
                        {remoteDialogStageLabel(stage.stage, t)}
                      </span>
                      <span className="min-w-0 truncate text-stone-500">{stage.message}</span>
                    </div>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}

          <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
            <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-700">
              <ScrollText aria-hidden="true" className="size-4 shrink-0 text-teal-700" />
              <span className="truncate">{t("Enable Project Spec")}</span>
            </span>
            <input
              checked={specEnabled}
              className="size-4 accent-teal-700"
              disabled={isSaving}
              onChange={(event) => onSpecEnabledChange(event.target.checked)}
              type="checkbox"
            />
          </label>

          <details className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
            <summary className="cursor-pointer text-sm font-semibold text-stone-700">
              {t("Advanced")}
            </summary>
            <label className="mt-3 block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Terminal shell override")}
              </span>
              <input
                autoComplete="off"
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) => onTerminalShellChange(event.target.value)}
                placeholder={isRemote ? "/bin/bash" : ""}
                value={terminalShell}
              />
            </label>
          </details>

          <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <WorkspaceIcon
                  className="size-10 rounded-lg border border-stone-200 bg-white object-cover p-1"
                  fallbackClassName="size-10 rounded-lg border border-stone-200 bg-white p-2 text-teal-700"
                  isRemote={isRemote}
                  logoUrl={iconDraft?.previewUrl || null}
                />
                <div className="min-w-0">
                  <span className="block text-sm font-semibold text-stone-800">
                    {t("Workspace icon")}
                  </span>
                  <span className="block truncate text-xs text-stone-500">
                    {iconDraft?.name ?? (isRemote ? t("Remote workspace icon") : t("Folder icon"))}
                  </span>
                </div>
              </div>
              <button
                aria-label={t("Clear workspace icon")}
                className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-600 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={isSaving || !iconDraft}
                onClick={onClearIcon}
                title={t("Clear workspace icon")}
                type="button"
              >
                <Trash2 aria-hidden="true" className="size-4" />
              </button>
            </div>
            <input
              aria-label={t("Workspace icon file")}
              accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
              className="sr-only"
              disabled={isSaving}
              onChange={onIconFileChange}
              ref={iconInputRef}
              type="file"
            />
            <button
              aria-label={t("Upload icon")}
              className="mt-2 inline-flex h-9 items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:text-stone-400"
              disabled={isSaving}
              onClick={() => iconInputRef.current?.click()}
              title={t("Upload icon")}
              type="button"
            >
              <Upload aria-hidden="true" className="size-3.5" />
              {t("Upload icon")}
            </button>
          </div>
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
              disabled={isSaving || (isRemote && !selectedServerId)}
              title={title}
              type="submit"
            >
              {isSaving ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
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

function remoteDialogStatusDotClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return "bg-emerald-500";
  }
  if (normalized === "checking" || normalized === "connecting" || normalized === "reconnecting") {
    return "bg-amber-500";
  }
  if (normalized === "failed" || normalized === "failedauth") {
    return "bg-rose-500";
  }
  return "bg-stone-300";
}

function remoteDialogStageDotClass(status: string) {
  if (status === "success") {
    return "bg-emerald-500";
  }
  if (status === "failed") {
    return "bg-rose-500";
  }
  if (status === "skipped") {
    return "bg-stone-300";
  }
  return "bg-amber-500";
}

function remoteDialogStageLabel(stage: string, t: (key: string) => string) {
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
      return t("Syncing config");
    case "ready":
      return t("Ready");
    default:
      return stage;
  }
}
