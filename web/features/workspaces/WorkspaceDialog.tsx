import { FolderPlus, FolderSearch, LoaderCircle, ScrollText, Trash2, Upload, X } from "lucide-react";
import { ChangeEvent as ReactChangeEvent, FormEvent } from "react";

import type { WorkspaceIconDraft } from "../../api/types";
import { useI18n } from "../../shared/i18n";
import { WorkspaceIcon } from "./WorkspaceIcon";

export function WorkspaceDialog({
  canUseNativePicker,
  iconDraft,
  iconInputRef,
  isSelectingPath,
  isSaving,
  name,
  onClearIcon,
  onClose,
  onIconFileChange,
  onNameChange,
  onPathChange,
  onSelectPath,
  onSpecEnabledChange,
  onSubmit,
  path,
  specEnabled,
}: {
  canUseNativePicker: boolean;
  iconDraft: WorkspaceIconDraft | null;
  iconInputRef: { current: HTMLInputElement | null };
  isSelectingPath: boolean;
  isSaving: boolean;
  name: string;
  onClearIcon: () => void;
  onClose: () => void;
  onIconFileChange: (event: ReactChangeEvent<HTMLInputElement>) => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onSelectPath: () => void;
  onSpecEnabledChange: (enabled: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  path: string;
  specEnabled: boolean;
}) {
  const { language, t } = useI18n();
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
          <label className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
            <span className="flex min-w-0 items-center gap-2 text-sm font-semibold text-stone-700">
              <ScrollText
                aria-hidden="true"
                className="size-4 shrink-0 text-teal-700"
              />
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
          <div className="rounded-lg border border-stone-200 bg-stone-50/80 p-3">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <WorkspaceIcon
                  className="size-10 rounded-lg border border-stone-200 bg-white object-cover p-1"
                  fallbackClassName="size-10 rounded-lg border border-stone-200 bg-white p-2 text-teal-700"
                  logoUrl={iconDraft?.previewUrl || null}
                />
                <div className="min-w-0">
                  <span className="block text-sm font-semibold text-stone-800">
                    {t("Workspace icon")}
                  </span>
                  <span className="block truncate text-xs text-stone-500">
                    {iconDraft?.name ?? t("Folder icon")}
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
