import { GitBranch, LoaderCircle, Plus, X } from "lucide-react";
import { FormEvent } from "react";

import { useI18n } from "../../shared/i18n";

export function GitBranchDialog({
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

