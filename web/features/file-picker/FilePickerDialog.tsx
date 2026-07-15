import { ChevronRight, File, Folder, FolderOpen, LoaderCircle, RefreshCw, X } from "lucide-react";
import { KeyboardEvent as ReactKeyboardEvent, useEffect, useMemo, useState } from "react";

import type {
  FilePickerEntry,
  FilePickerListResponse,
  FilePickerMode,
  FilePickerTarget,
  NativeSelectedFile,
  Translate,
} from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";

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
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(() => new Set());
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
        setSelectedPaths((current) =>
          new Set([...current].filter((selected) => data.entries.some((entry) => entry.path === selected))),
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
  }, [allowOutsideWorkspace, mode, open, path, refreshRevision, showHidden, target, targetIdentity]);

  const selectableEntries = useMemo(
    () => response?.entries.filter((entry) => isSelectable(entry, mode)) ?? [],
    [mode, response],
  );
  const canConfirm = selectedPaths.size > 0 || (mode === "directory" && Boolean(response?.path));

  if (!open) {
    return null;
  }

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
    const finalPaths = paths.length ? paths : mode === "directory" && response?.path ? [response.path] : [];
    if (!finalPaths.length) {
      return;
    }

    setIsConfirming(true);
    setError(null);
    try {
      if (readFiles) {
        const data = await requestJson<{ files: NativeSelectedFile[] }>("/api/file-picker/read-files", {
          body: JSON.stringify({
            allowOutsideWorkspace,
            paths: finalPaths,
            target,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        });
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

  function handleEntryKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>, entry: FilePickerEntry) {
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/40 px-4 py-6" role="presentation">
      <div
        aria-label={title}
        aria-modal="true"
        className="flex max-h-[86vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-2xl"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            onClose();
          }
        }}
        role="dialog"
      >
        <div className="flex items-center justify-between border-b border-stone-200 px-4 py-3">
          <div>
            <h2 className="text-sm font-semibold text-stone-900">{title}</h2>
            <p className="text-xs text-stone-500">{mode === "directory" ? t("Select a folder") : t("Select file")}</p>
          </div>
          <button className="rounded-lg p-2 text-stone-500 hover:bg-stone-100" onClick={onClose} type="button">
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>

        <form
          className="flex gap-2 border-b border-stone-200 px-4 py-3"
          onSubmit={(event) => {
            event.preventDefault();
            openPath(draftPath.trim());
          }}
        >
          <input
            aria-label={t("Path")}
            className="min-w-0 flex-1 rounded-lg border border-stone-200 px-3 py-2 text-sm outline-none focus:border-stone-400 focus:ring-2 focus:ring-stone-200"
            onChange={(event) => setDraftPath(event.target.value)}
            value={draftPath}
          />
          <button className="rounded-lg border border-stone-200 px-3 py-2 text-sm hover:bg-stone-50" type="submit">
            {t("Open")}
          </button>
          <button
            aria-label={t("Refresh")}
            className="rounded-lg border border-stone-200 px-3 py-2 text-sm hover:bg-stone-50"
            onClick={() => setRefreshRevision((value) => value + 1)}
            type="button"
          >
            <RefreshCw aria-hidden="true" className="size-4" />
          </button>
        </form>

        <label className="flex items-center gap-2 border-b border-stone-100 px-4 py-2 text-xs font-medium text-stone-600">
          <input
            checked={showHidden}
            className="size-4 rounded border-stone-300 text-stone-900 focus:ring-stone-300"
            onChange={(event) => setShowHidden(event.target.checked)}
            type="checkbox"
          />
          <span>{t("Show hidden files")}</span>
        </label>

        <div className="flex items-center gap-2 border-b border-stone-100 px-4 py-2 text-xs text-stone-500">
          <button
            className="rounded px-2 py-1 hover:bg-stone-100 disabled:opacity-50"
            disabled={!response?.parentPath}
            onClick={() => response?.parentPath && openPath(response.parentPath)}
            type="button"
          >
            {t("Up")}
          </button>
          <ChevronRight aria-hidden="true" className="size-3" />
          <span className="truncate">{response?.path || path || "/"}</span>
        </div>

        <div className="min-h-64 flex-1 overflow-auto px-2 py-2">
          {isLoading ? (
            <div className="flex h-40 items-center justify-center text-sm text-stone-500">
              <LoaderCircle aria-hidden="true" className="mr-2 size-4 animate-spin" />
              {t("Loading")}
            </div>
          ) : error ? (
            <div className="rounded-xl border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{error}</div>
          ) : response?.entries.length ? (
            <div className="space-y-1">
              {response.entries.map((entry) => {
                const selected = selectedPaths.has(entry.path);
                const selectable = isSelectable(entry, mode) && !entry.disabled;
                return (
                  <button
                    aria-disabled={!selectable && !(entry.isDirectory && mode === "file")}
                    aria-pressed={selected}
                    className={`flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-sm outline-none focus:ring-2 focus:ring-stone-300 ${selected ? "bg-stone-900 text-white" : "hover:bg-stone-100"} ${entry.disabled ? "opacity-50" : ""}`}
                    key={entry.path}
                    onClick={() => {
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
                    type="button"
                  >
                    {entry.isDirectory ? (
                      <Folder aria-hidden="true" className="size-4 shrink-0" />
                    ) : (
                      <File aria-hidden="true" className="size-4 shrink-0" />
                    )}
                    <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                    {entry.isDirectory ? <FolderOpen aria-hidden="true" className="size-3.5 shrink-0 opacity-60" /> : null}
                  </button>
                );
              })}
            </div>
          ) : (
            <div className="flex h-40 items-center justify-center text-sm text-stone-500">{t("No files")}</div>
          )}
        </div>

        {response?.truncated ? (
          <div className="border-t border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-700">
            {t("Showing first {count} entries", { count: String(FILE_PICKER_LIMIT) })}
          </div>
        ) : null}
        {response?.warnings?.length ? (
          <div className="border-t border-stone-200 px-4 py-2 text-xs text-stone-500">{response.warnings[0]}</div>
        ) : null}
        {selectableEntries.length === 0 && response?.entries.length ? (
          <div className="border-t border-stone-200 px-4 py-2 text-xs text-stone-500">{t("No selectable files in this folder")}</div>
        ) : null}

        <div className="flex justify-end gap-2 border-t border-stone-200 px-4 py-3">
          <button className="rounded-lg border border-stone-200 px-3 py-2 text-sm hover:bg-stone-50" onClick={onClose} type="button">
            {t("Cancel")}
          </button>
          <button
            className="inline-flex items-center rounded-lg bg-stone-900 px-3 py-2 text-sm font-medium text-white hover:bg-stone-800 disabled:opacity-50"
            disabled={!canConfirm || isConfirming}
            onClick={() => void confirmSelection()}
            type="button"
          >
            {isConfirming ? <LoaderCircle aria-hidden="true" className="mr-2 size-4 animate-spin" /> : null}
            {t("Select")}
          </button>
        </div>
      </div>
    </div>
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
