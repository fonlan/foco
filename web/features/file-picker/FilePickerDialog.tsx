import {
  ChevronRight,
  File,
  Folder,
  FolderOpen,
  RefreshCw,
  X,
} from "lucide-react";
import {
  KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useMemo,
  useState,
} from "react";

import type {
  FilePickerEntry,
  FilePickerListResponse,
  FilePickerMode,
  FilePickerTarget,
  NativeSelectedFile,
  Translate,
} from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import {
  Button,
  Input,
  Label,
  Modal,
  Spinner,
  Switch,
  TextField,
} from "../../shared/ui";

export type FilePickerSelection = {
  path: string;
  file?: NativeSelectedFile;
};

type FilePickerDialogProps = {
  /** Attachment-only: browse any absolute path on the workspace host. Default false. */
  allowOutsideWorkspace?: boolean;
  initialPath?: string | null;
  mode: FilePickerMode;
  multiple?: boolean;
  open: boolean;
  readFiles?: boolean;
  target: FilePickerTarget;
  title: string;
  t: Translate;
  onClose: () => void;
  onSelect: (selection: FilePickerSelection[]) => void;
};

const FILE_PICKER_LIMIT = 500;

export function FilePickerDialog({
  allowOutsideWorkspace = false,
  initialPath,
  mode,
  multiple = false,
  open,
  readFiles = false,
  target,
  title,
  t,
  onClose,
  onSelect,
}: FilePickerDialogProps) {
  const [path, setPath] = useState(initialPath?.trim() ?? "");
  const [draftPath, setDraftPath] = useState(initialPath?.trim() ?? "");
  const [response, setResponse] = useState<FilePickerListResponse | null>(null);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(
    () => new Set(),
  );
  const [isLoading, setIsLoading] = useState(false);
  const [isConfirming, setIsConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshRevision, setRefreshRevision] = useState(0);
  const [showHidden, setShowHidden] = useState(false);
  const targetIdentity = targetKey(target);

  useEffect(() => {
    if (!open) {
      return;
    }
    const nextPath = initialPath?.trim() ?? "";
    setPath(nextPath);
    setDraftPath(nextPath);
    setSelectedPaths(new Set());
    setShowHidden(false);
    setError(null);
  }, [initialPath, open, targetIdentity]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const controller = new AbortController();
    setIsLoading(true);
    setError(null);

    void requestJson<FilePickerListResponse>("/api/file-picker/list", {
      body: JSON.stringify({
        allowOutsideWorkspace,
        includeFiles: mode === "file",
        limit: FILE_PICKER_LIMIT,
        mode,
        path,
        showHidden,
        target,
      }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
      signal: controller.signal,
    })
      .then((data) => {
        setResponse(data);
        setPath(data.path);
        setDraftPath(data.path);
        setSelectedPaths(
          (current) =>
            new Set(
              [...current].filter((selected) =>
                data.entries.some((entry) => entry.path === selected),
              ),
            ),
        );
      })
      .catch((requestError) => {
        if (controller.signal.aborted) {
          return;
        }
        setResponse(null);
        setError(errorMessage(requestError));
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setIsLoading(false);
        }
      });

    return () => controller.abort();
  }, [
    allowOutsideWorkspace,
    mode,
    open,
    path,
    refreshRevision,
    showHidden,
    target,
    targetIdentity,
  ]);

  const selectableEntries = useMemo(
    () => response?.entries.filter((entry) => isSelectable(entry, mode)) ?? [],
    [mode, response],
  );
  const canConfirm =
    selectedPaths.size > 0 || (mode === "directory" && Boolean(response?.path));

  function openPath(nextPath: string) {
    setPath(nextPath);
    setDraftPath(nextPath);
    setSelectedPaths(new Set());
  }

  function toggleEntry(entry: FilePickerEntry) {
    if (!isSelectable(entry, mode) || entry.disabled) {
      return;
    }
    setSelectedPaths((current) => {
      const next = new Set(multiple ? current : []);
      if (next.has(entry.path)) {
        next.delete(entry.path);
      } else {
        next.add(entry.path);
      }
      return next;
    });
  }

  async function confirmSelection(pathsOverride?: string[]) {
    const paths = pathsOverride ?? [...selectedPaths];
    const finalPaths =
      paths.length
        ? paths
        : mode === "directory" && response?.path
          ? [response.path]
          : [];
    if (!finalPaths.length) {
      return;
    }

    setIsConfirming(true);
    setError(null);
    try {
      if (readFiles) {
        const data = await requestJson<{ files: NativeSelectedFile[] }>(
          "/api/file-picker/read-files",
          {
            body: JSON.stringify({
              allowOutsideWorkspace,
              paths: finalPaths,
              target,
            }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );
        onSelect(data.files.map((file) => ({ file, path: file.path })));
      } else {
        onSelect(finalPaths.map((selectedPath) => ({ path: selectedPath })));
      }
      onClose();
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsConfirming(false);
    }
  }

  function handleEntryKeyDown(
    event: ReactKeyboardEvent<HTMLButtonElement>,
    entry: FilePickerEntry,
  ) {
    if (event.key !== "Enter") {
      return;
    }
    event.preventDefault();
    if (entry.isDirectory && mode === "file") {
      openPath(entry.path);
      return;
    }
    if (isSelectable(entry, mode)) {
      if (!multiple) {
        void confirmSelection([entry.path]);
        return;
      }
      toggleEntry(entry);
    }
  }

  return (
    <Modal.Backdrop
      isDismissable
      isOpen={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          onClose();
        }
      }}
    >
      <Modal.Container placement="center" scroll="inside" size="lg">
        <Modal.Dialog aria-label={title}>
          <Modal.Header className="flex-row items-start justify-between gap-3">
            <div className="min-w-0">
              <Modal.Heading>{title}</Modal.Heading>
              <p className="text-xs text-muted">
                {mode === "directory" ? t("Select a folder") : t("Select file")}
              </p>
            </div>
            <Button
              aria-label={t("Close")}
              isIconOnly
              size="sm"
              variant="ghost"
              onPress={onClose}
            >
              <X aria-hidden="true" className="size-4" />
            </Button>
          </Modal.Header>

          <Modal.Body className="flex min-h-0 flex-col gap-0 p-0">
            <form
              className="flex gap-2 border-b border-border px-4 py-3"
              onSubmit={(event) => {
                event.preventDefault();
                openPath(draftPath.trim());
              }}
            >
              <TextField
                className="min-w-0 flex-1"
                fullWidth
                name="file-picker-path"
                value={draftPath}
                onChange={setDraftPath}
              >
                <Label className="sr-only">{t("Path")}</Label>
                <Input aria-label={t("Path")} />
              </TextField>
              <Button size="sm" type="submit" variant="secondary">
                {t("Open")}
              </Button>
              <Button
                aria-label={t("Refresh")}
                isIconOnly
                size="sm"
                type="button"
                variant="secondary"
                onPress={() => setRefreshRevision((value) => value + 1)}
              >
                <RefreshCw aria-hidden="true" className="size-4" />
              </Button>
            </form>

            <div className="flex items-center gap-2 border-b border-border px-4 py-2 text-xs font-medium text-muted">
              <Switch isSelected={showHidden} onChange={setShowHidden}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  <span>{t("Show hidden files")}</span>
                </Switch.Content>
              </Switch>
            </div>

            <div className="flex items-center gap-2 border-b border-border px-4 py-2 text-xs text-muted">
              <Button
                isDisabled={!response?.parentPath}
                size="sm"
                variant="ghost"
                onPress={() =>
                  response?.parentPath && openPath(response.parentPath)
                }
              >
                {t("Up")}
              </Button>
              <ChevronRight aria-hidden="true" className="size-3" />
              <span className="truncate">{response?.path || path || "/"}</span>
            </div>

            <div className="min-h-64 flex-1 overflow-auto px-2 py-2">
              {isLoading ? (
                <div className="flex h-40 items-center justify-center gap-2 text-sm text-muted">
                  <Spinner color="current" size="sm" />
                  {t("Loading")}
                </div>
              ) : error ? (
                <div className="rounded-xl border border-danger bg-danger-soft px-3 py-2 text-sm text-danger-soft-foreground">
                  {error}
                </div>
              ) : response?.entries.length ? (
                <div className="space-y-1">
                  {response.entries.map((entry) => {
                    const selected = selectedPaths.has(entry.path);
                    const selectable =
                      isSelectable(entry, mode) && !entry.disabled;
                    return (
                      <Button
                        aria-disabled={
                          !selectable && !(entry.isDirectory && mode === "file")
                        }
                        aria-pressed={selected}
                        className={`flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-focus ${selected ? "bg-accent text-accent-foreground" : "hover:bg-default"} ${entry.disabled ? "opacity-50" : ""}`}
                        key={entry.path}
                        onPress={() => {
                          if (entry.isDirectory && mode === "file") {
                            openPath(entry.path);
                            return;
                          }
                          toggleEntry(entry);
                        }}
                        onDoubleClick={() => {
                          if (entry.isDirectory) {
                            openPath(entry.path);
                          }
                        }}
                        onKeyDown={(event) => handleEntryKeyDown(event, entry)}
                      >
                        {entry.isDirectory ? (
                          <Folder
                            aria-hidden="true"
                            className="size-4 shrink-0"
                          />
                        ) : (
                          <File
                            aria-hidden="true"
                            className="size-4 shrink-0"
                          />
                        )}
                        <span className="min-w-0 flex-1 truncate">
                          {entry.name}
                        </span>
                        {entry.isDirectory ? (
                          <FolderOpen
                            aria-hidden="true"
                            className="size-3.5 shrink-0 opacity-60"
                          />
                        ) : null}
                      </Button>
                    );
                  })}
                </div>
              ) : (
                <div className="flex h-40 items-center justify-center text-sm text-muted">
                  {t("No files")}
                </div>
              )}
            </div>

            {response?.truncated ? (
              <div className="border-t border-warning bg-warning-soft px-4 py-2 text-xs text-warning-soft-foreground">
                {t("Showing first {count} entries", {
                  count: String(FILE_PICKER_LIMIT),
                })}
              </div>
            ) : null}
            {response?.warnings?.length ? (
              <div className="border-t border-border px-4 py-2 text-xs text-muted">
                {response.warnings[0]}
              </div>
            ) : null}
            {selectableEntries.length === 0 && response?.entries.length ? (
              <div className="border-t border-border px-4 py-2 text-xs text-muted">
                {t("No selectable files in this folder")}
              </div>
            ) : null}
          </Modal.Body>

          <Modal.Footer>
            <Button variant="tertiary" onPress={onClose}>
              {t("Cancel")}
            </Button>
            <Button
              isDisabled={!canConfirm}
              isPending={isConfirming}
              onPress={() => void confirmSelection()}
            >
              {({ isPending }) => (
                <>
                  {isPending ? <Spinner color="current" size="sm" /> : null}
                  {t("Select")}
                </>
              )}
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function isSelectable(entry: FilePickerEntry, mode: FilePickerMode) {
  return mode === "directory" ? entry.isDirectory : !entry.isDirectory;
}

function targetKey(target: FilePickerTarget) {
  if (target.kind === "remoteServer") {
    return `${target.kind}:${target.serverId}`;
  }
  if (target.kind === "workspace") {
    return `${target.kind}:${target.workspaceId}`;
  }
  return target.kind;
}
