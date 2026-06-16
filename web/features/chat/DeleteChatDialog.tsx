import { Trash2, X } from "lucide-react";

import type { PendingDeleteChat } from "../../api/types";
import { useI18n } from "../../shared/i18n";

export function DeleteChatDialog({
  chat,
  onClose,
  onConfirm,
}: {
  chat: PendingDeleteChat;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-stone-950/35 p-4 backdrop-blur-sm"
      role="presentation"
    >
      <section
        aria-labelledby="delete-chat-dialog-title"
        aria-modal="true"
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <Trash2 aria-hidden="true" className="size-5 text-rose-700" />
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="delete-chat-dialog-title"
            >
              {t("Delete this chat?")}
            </h2>
          </div>
          <button
            aria-label={t("Cancel chat deletion")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Cancel")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="space-y-4 px-4 py-4">
          <div>
            <p className="text-sm font-medium text-stone-950">{chat.title}</p>
            <p className="mt-1 text-xs font-medium text-stone-500">
              {chat.workspaceName}
            </p>
          </div>
          <p className="text-sm leading-6 text-stone-600">
            {t("This will delete the saved chat history.")}
          </p>
          <div className="flex justify-end gap-2">
            <button
              aria-label={t("Cancel chat deletion")}
              className="inline-flex size-11 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-stone-300 hover:bg-stone-50"
              onClick={onClose}
              title={t("Cancel")}
              type="button"
            >
              <X aria-hidden="true" className="size-4" />
            </button>
            <button
              aria-label={t("Confirm delete chat")}
              className="inline-flex size-11 items-center justify-center rounded-lg bg-rose-700 text-white shadow-[0_12px_28px_rgba(190,18,60,0.22)] hover:bg-rose-800"
              onClick={onConfirm}
              title={t("Delete chat")}
              type="button"
            >
              <Trash2 aria-hidden="true" className="size-4" />
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

