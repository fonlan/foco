import type { Translate } from "../../api/types";

export const CHAT_COMPLETION_REQUEST_KIND = "chat completion";
export const CONTEXT_COMPRESSION_REQUEST_KIND = "contextCompression";

export function requestKindLabel(requestKind: string, t: Translate) {
  switch (requestKind) {
    case CHAT_COMPLETION_REQUEST_KIND:
      return t("Chat completion");
    case CONTEXT_COMPRESSION_REQUEST_KIND:
      return t("Context compression");
    case "prompt hook":
      return t("Prompt hook");
    default:
      return requestKind;
  }
}

export function requestKindBadgeClass(requestKind: string) {
  const base =
    "inline-flex max-w-full items-center rounded-full border px-2 py-0.5 text-xs font-semibold";

  switch (requestKind) {
    case CHAT_COMPLETION_REQUEST_KIND:
      return `${base} border-teal-200 bg-teal-50 text-teal-800`;
    case CONTEXT_COMPRESSION_REQUEST_KIND:
      return `${base} border-violet-200 bg-violet-50 text-violet-800`;
    default:
      return `${base} border-stone-200 bg-stone-100 text-stone-700`;
  }
}
