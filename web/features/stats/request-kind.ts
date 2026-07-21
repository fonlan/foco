import type { Translate } from "../../api/types";

export const CHAT_COMPLETION_REQUEST_KIND = "chat completion";
export const CONTEXT_COMPRESSION_REQUEST_KIND = "contextCompression";

export const STABLE_REQUEST_KINDS = [
  CHAT_COMPLETION_REQUEST_KIND,
  CONTEXT_COMPRESSION_REQUEST_KIND,
  "prompt hook",
  "chat title generation",
  "memory extraction",
  "memory retrieval",
  "memory Dream planner",
  "model availability test",
  "workspace spec generation",
  "workspace spec update",
  "workspace spec compaction",
  "workspace spec update compaction",
  "git_commit_message_generation",
] as const;

type StableRequestKind = (typeof STABLE_REQUEST_KINDS)[number];

const REQUEST_KIND_TRANSLATION_KEYS: Record<StableRequestKind, string> = {
  "chat completion": "Chat completion",
  contextCompression: "Context compression",
  "prompt hook": "Prompt hook",
  "chat title generation": "Chat title generation",
  "memory extraction": "Memory extraction",
  "memory retrieval": "Memory retrieval",
  "memory Dream planner": "Memory Dream planner",
  "model availability test": "Model availability test",
  "workspace spec generation": "Workspace Spec generation",
  "workspace spec update": "Workspace Spec update",
  "workspace spec compaction": "Workspace Spec compaction",
  "workspace spec update compaction": "Workspace Spec update compaction",
  git_commit_message_generation: "Git commit message generation",
};

export function requestKindLabel(requestKind: string, t: Translate) {
  const translationKey =
    REQUEST_KIND_TRANSLATION_KEYS[requestKind as StableRequestKind];
  return translationKey ? t(translationKey) : requestKind;
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
