import {
  CheckCircle2,
  Code2,
  FolderPlus,
  FolderSearch,
  ScrollText,
  Server,
  Terminal,
  Trash2,
  Upload,
} from "lucide-react";
import { FormEvent } from "react";

import type {
  RemoteServerDiagnosticStage,
  RemoteServerSummary,
  WorkspaceIconDraft,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";
import {
  Button,
  Input,
  Label,
  ListBox,
  Modal,
  Select,
  Spinner,
  Switch,
  TextField,
} from "../../shared/ui";
import { WorkspaceIcon } from "./WorkspaceIcon";

export function WorkspaceDialog({
  codeGraphEnabled,
  iconDraft,
  inlineServerHost,
  inlineServerName,
  isCreatingInlineServer,
  isSaving,
  isTestingConnection,
  mode,
  name,
  onClearIcon,
  onClose,
  onCodeGraphEnabledChange,
  onCreateInlineServer,
  onInlineServerHostChange,
  onInlineServerNameChange,
  onModeChange,
  onNameChange,
  onPathChange,
  onSelectPath,
  onSelectIcon,
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
  codeGraphEnabled: boolean;
  iconDraft: WorkspaceIconDraft | null;
  inlineServerHost: string;
  inlineServerName: string;
  isCreatingInlineServer: boolean;
  isSaving: boolean;
  isTestingConnection: boolean;
  mode: "local" | "ssh";
  name: string;
  onClearIcon: () => void;
  onClose: () => void;
  onCodeGraphEnabledChange: (enabled: boolean) => void;
  onCreateInlineServer: () => void;
  onInlineServerHostChange: (value: string) => void;
  onInlineServerNameChange: (value: string) => void;
  onModeChange: (mode: "local" | "ssh") => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSelectPath: () => void;
  onSelectIcon: () => void;
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
  const selectedServer =
    remoteServers.find((server) => server.id === selectedServerId) ?? null;
  const selectedServerAvailable =
    selectedServer?.sidecarInstallState === "available" ||
    selectedServer?.sidecarInstallState === "customCommand";
  const isRemote = mode === "ssh";

  return (
    <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && onClose()}>
      <Modal.Container placement="center" scroll="inside" size="lg">
        <Modal.Dialog aria-label={title}>
          <Modal.CloseTrigger aria-label={t("Close workspace dialog")} />
          <Modal.Header>
            <Modal.Heading>{title}</Modal.Heading>
            <p className="text-sm text-muted">
              {isRemote
                ? t("Register an SSH workspace.")
                : t("Create or register a local folder.")}
            </p>
          </Modal.Header>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void onSubmit(event);
            }}
          >
            <Modal.Body className="space-y-4">
              <div className="grid grid-cols-2 gap-2 rounded-lg bg-default p-1">
                <Button
                  aria-pressed={!isRemote}
                  className={!isRemote ? undefined : "bg-transparent shadow-none"}
                  type="button"
                  variant={!isRemote ? "secondary" : "ghost"}
                  onPress={() => onModeChange("local")}
                >
                  <FolderPlus aria-hidden="true" className="size-4" />
                  {t("Local")}
                </Button>
                <Button
                  aria-pressed={isRemote}
                  className={isRemote ? undefined : "bg-transparent shadow-none"}
                  type="button"
                  variant={isRemote ? "secondary" : "ghost"}
                  onPress={() => onModeChange("ssh")}
                >
                  <Server aria-hidden="true" className="size-4" />
                  {t("SSH")}
                </Button>
              </div>

              {isRemote ? (
                <section className="grid gap-3 rounded-lg border border-border bg-surface p-3">
                  <Select
                    className="w-full"
                    placeholder={t("Select remote server")}
                    selectedKey={selectedServerId || null}
                    onSelectionChange={(key) =>
                      onServerChange(key == null ? "" : String(key))
                    }
                  >
                    <Label>{t("Remote Server")}</Label>
                    <Select.Trigger>
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover>
                      <ListBox>
                        {remoteServers.map((server) => {
                          const isServerAvailable =
                            server.sidecarInstallState === "available" ||
                            server.sidecarInstallState === "customCommand";
                          return (
                            <ListBox.Item
                              id={server.id}
                              isDisabled={!isServerAvailable}
                              key={server.id}
                              textValue={`${server.name} (${server.hostAlias})`}
                            >
                              {server.name} ({server.hostAlias})
                              <ListBox.ItemIndicator />
                            </ListBox.Item>
                          );
                        })}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                  {selectedServer ? (
                    <div className="flex items-center justify-between gap-3 rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted">
                      <span className="min-w-0 truncate">
                        {selectedServer.name}:{" "}
                        {selectedServer.defaultRemoteRoot ??
                          selectedServer.hostAlias}
                      </span>
                      <span
                        className={`size-2.5 shrink-0 rounded-full ${remoteDialogStatusDotClass(selectedServer.status)}`}
                      />
                    </div>
                  ) : null}
                  <div className="grid gap-2 sm:grid-cols-2">
                    <TextField
                      fullWidth
                      name="inline-server-name"
                      value={inlineServerName}
                      onChange={onInlineServerNameChange}
                    >
                      <Label className="sr-only">{t("Server name")}</Label>
                      <Input
                        autoComplete="off"
                        placeholder={t("Server name")}
                      />
                    </TextField>
                    <div className="flex gap-2">
                      <TextField
                        className="min-w-0 flex-1"
                        fullWidth
                        name="inline-server-host"
                        value={inlineServerHost}
                        onChange={onInlineServerHostChange}
                      >
                        <Label className="sr-only">
                          {t("SSH hostname / IP")}
                        </Label>
                        <Input
                          autoComplete="off"
                          placeholder={t("SSH hostname / IP")}
                        />
                      </TextField>
                      <Button
                        aria-label={t("Add remote server")}
                        isDisabled={isCreatingInlineServer}
                        isIconOnly
                        isPending={isCreatingInlineServer}
                        type="button"
                        variant="secondary"
                        onPress={onCreateInlineServer}
                      >
                        {({ isPending }) =>
                          isPending ? (
                            <Spinner color="current" size="sm" />
                          ) : (
                            <Server aria-hidden="true" className="size-4" />
                          )
                        }
                      </Button>
                    </div>
                  </div>
                </section>
              ) : null}

              <TextField
                fullWidth
                name="workspace-name"
                value={name}
                onChange={onNameChange}
              >
                <Label>{t("Name")}</Label>
                <Input
                  autoComplete="off"
                  placeholder={t("Workspace name")}
                />
              </TextField>

              <div className="flex items-end gap-2">
                <TextField
                  className="min-w-0 flex-1"
                  fullWidth
                  name="workspace-path"
                  value={path}
                  onChange={onPathChange}
                >
                  <Label>{isRemote ? t("Remote path") : t("Path")}</Label>
                  <Input
                    autoComplete="off"
                    placeholder={
                      isRemote
                        ? "/home/name/workspace"
                        : "C:/Users/name/workspace"
                    }
                  />
                </TextField>
                <Button
                  aria-label={t("Choose workspace path")}
                  isDisabled={
                    isRemote && (!selectedServerId || !selectedServerAvailable)
                  }
                  isIconOnly
                  type="button"
                  variant="secondary"
                  onPress={onSelectPath}
                >
                  <FolderSearch aria-hidden="true" className="size-4" />
                </Button>
              </div>

              {isRemote ? (
                <div className="rounded-lg border border-border bg-background p-3">
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-foreground">
                      <Terminal
                        aria-hidden="true"
                        className="size-4 shrink-0 text-accent"
                      />
                      <span className="truncate">{t("Test connection")}</span>
                    </div>
                    <Button
                      aria-label={t("Test connection")}
                      isDisabled={isTestingConnection || !selectedServerId}
                      isIconOnly
                      isPending={isTestingConnection}
                      type="button"
                      variant="secondary"
                      onPress={onTestConnection}
                    >
                      {({ isPending }) =>
                        isPending ? (
                          <Spinner color="current" size="sm" />
                        ) : (
                          <CheckCircle2
                            aria-hidden="true"
                            className="size-4"
                          />
                        )
                      }
                    </Button>
                  </div>
                  {testStages.length ? (
                    <div className="mt-3 grid gap-1.5 text-xs text-muted">
                      {testStages.map((stage) => (
                        <div
                          className="flex items-center gap-2"
                          key={stage.stage}
                        >
                          <span
                            className={`size-2 rounded-full ${remoteDialogStageDotClass(stage.status)}`}
                          />
                          <span className="font-medium text-foreground">
                            {remoteDialogStageLabel(stage.stage, t)}
                          </span>
                          <span className="min-w-0 truncate text-muted">
                            {stage.message}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}

              <div className="flex items-center justify-between gap-3 rounded-lg border border-border bg-surface px-3 py-2">
                <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-foreground">
                  <ScrollText
                    aria-hidden="true"
                    className="size-4 shrink-0 text-accent"
                  />
                  <span className="truncate" id="workspace-dialog-spec-label">
                    {t("Enable Project Spec")}
                  </span>
                </span>
                <Switch
                  aria-labelledby="workspace-dialog-spec-label"
                  isDisabled={isSaving}
                  isSelected={specEnabled}
                  onChange={onSpecEnabledChange}
                >
                  <Switch.Content>
                    <Switch.Control>
                      <Switch.Thumb />
                    </Switch.Control>
                  </Switch.Content>
                </Switch>
              </div>

              <div className="rounded-lg border border-border bg-surface px-3 py-2">
                <div className="flex items-center justify-between gap-3">
                  <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-foreground">
                    <Code2
                      aria-hidden="true"
                      className="size-4 shrink-0 text-accent"
                    />
                    <span
                      className="truncate"
                      id="workspace-dialog-code-graph-label"
                    >
                      {t("Enable Codegraph")}
                    </span>
                  </span>
                  <Switch
                    aria-labelledby="workspace-dialog-code-graph-label"
                    aria-describedby="workspace-dialog-code-graph-description"
                    isDisabled={isSaving}
                    isSelected={codeGraphEnabled}
                    onChange={onCodeGraphEnabledChange}
                  >
                    <Switch.Content>
                      <Switch.Control>
                        <Switch.Thumb />
                      </Switch.Control>
                    </Switch.Content>
                  </Switch>
                </div>
                <p
                  className="mt-1 text-xs text-muted"
                  id="workspace-dialog-code-graph-description"
                >
                  {t(
                    "Indexes code symbols and powers code graph tools in chats. When disabled, no index is built or maintained and chat does not receive code graph tools.",
                  )}
                </p>
              </div>

              <details className="rounded-lg border border-border bg-surface p-3">
                <summary className="cursor-pointer text-sm font-semibold text-foreground">
                  {t("Advanced")}
                </summary>
                <TextField
                  className="mt-3"
                  fullWidth
                  name="terminal-shell"
                  value={terminalShell}
                  onChange={onTerminalShellChange}
                >
                  <Label>{t("Terminal shell override")}</Label>
                  <Input
                    autoComplete="off"
                    placeholder={isRemote ? "/bin/bash" : ""}
                  />
                </TextField>
              </details>

              <div className="rounded-lg border border-border bg-surface p-3">
                <div className="mb-3 flex items-center justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <WorkspaceIcon
                      className="size-10 rounded-lg border border-border bg-background object-cover p-1"
                      fallbackClassName="size-10 rounded-lg border border-border bg-background p-2 text-accent"
                      isRemote={isRemote}
                      logoUrl={iconDraft?.previewUrl || null}
                    />
                    <div className="min-w-0">
                      <span className="block text-sm font-semibold text-foreground">
                        {t("Workspace icon")}
                      </span>
                      <span className="block truncate text-xs text-muted">
                        {iconDraft?.name ??
                          (isRemote
                            ? t("Remote workspace icon")
                            : t("Folder icon"))}
                      </span>
                    </div>
                  </div>
                  <Button
                    aria-label={t("Clear workspace icon")}
                    isDisabled={isSaving || !iconDraft}
                    isIconOnly
                    type="button"
                    variant="danger-soft"
                    onPress={onClearIcon}
                  >
                    <Trash2 aria-hidden="true" className="size-4" />
                  </Button>
                </div>
                <Button
                  aria-label={t("Upload icon")}
                  isDisabled={
                    isSaving ||
                    (isRemote &&
                      (!selectedServerId || !selectedServerAvailable))
                  }
                  size="sm"
                  type="button"
                  variant="secondary"
                  onPress={onSelectIcon}
                >
                  <Upload aria-hidden="true" className="size-3.5" />
                  {t("Upload icon")}
                </Button>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button
                aria-label={t("Cancel workspace dialog")}
                type="button"
                variant="tertiary"
                onPress={onClose}
              >
                {t("Cancel")}
              </Button>
              <Button
                aria-label={title}
                isDisabled={
                  isRemote && (!selectedServerId || !selectedServerAvailable)
                }
                isPending={isSaving}
                type="submit"
              >
                {({ isPending }) => (
                  <>
                    {isPending ? (
                      <Spinner color="current" size="sm" />
                    ) : (
                      <FolderPlus aria-hidden="true" className="size-4" />
                    )}
                    {title}
                  </>
                )}
              </Button>
            </Modal.Footer>
          </form>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function remoteDialogStatusDotClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "connected" || normalized === "ready") {
    return "bg-success";
  }
  if (
    normalized === "checking" ||
    normalized === "connecting" ||
    normalized === "reconnecting"
  ) {
    return "bg-warning";
  }
  if (normalized === "failed" || normalized === "failedauth") {
    return "bg-danger";
  }
  return "bg-default";
}

function remoteDialogStageDotClass(status: string) {
  if (status === "success") {
    return "bg-success";
  }
  if (status === "failed") {
    return "bg-danger";
  }
  if (status === "skipped") {
    return "bg-default";
  }
  return "bg-warning";
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
      return t("Checking Sidecar version");
    case "ready":
      return t("Ready");
    default:
      return stage;
  }
}
