import type { GitDiffLineStats, GitDiffResponse } from "../../api/types";

export type GitDiffSection = {
  kind: "staged" | "unstaged";
  files: GitDiffFile[];
};

export type GitDiffFile = {
  isBinary: boolean;
  lines: GitDiffLine[];
  path: string;
};

export type GitDiffLine = {
  kind: "add" | "context" | "hunk" | "meta" | "remove";
  prefix: string;
  text: string;
};

export function parseGitDiffSections(
  diff: GitDiffResponse | null,
): GitDiffSection[] {
  if (!diff) {
    return [];
  }

  return [
    { kind: "staged" as const, text: diff.stagedDiff },
    { kind: "unstaged" as const, text: diff.diff },
  ]
    .map(({ kind, text }) => ({
      kind,
      files: parseGitDiffFiles(text),
    }))
    .filter((section) => section.files.length > 0);
}

export function hasGitDiffStats(stats: GitDiffLineStats) {
  return stats.additions > 0 || stats.deletions > 0;
}

export function parseGitDiffLineStats(value: unknown): GitDiffLineStats | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const additions = fieldValue(value, "additions");
  const deletions = fieldValue(value, "deletions");
  if (
    typeof additions !== "number" ||
    typeof deletions !== "number" ||
    !Number.isSafeInteger(additions) ||
    !Number.isSafeInteger(deletions) ||
    additions < 0 ||
    deletions < 0
  ) {
    return null;
  }

  return { additions, deletions };
}

export function diffLineClass(kind: GitDiffLine["kind"]) {
  const base = "flex min-w-max px-3";

  if (kind === "add") {
    return `${base} bg-emerald-50 text-emerald-950`;
  }

  if (kind === "remove") {
    return `${base} bg-rose-50 text-rose-950`;
  }

  if (kind === "hunk") {
    return `${base} bg-sky-50 text-sky-900`;
  }

  if (kind === "meta") {
    return `${base} text-stone-500`;
  }

  return `${base} text-stone-700`;
}

// Private helpers

function parseGitDiffFiles(diffText: string): GitDiffFile[] {
  const files: GitDiffFile[] = [];
  let current: GitDiffFile | null = null;

  for (const line of diffText.split("\n")) {
    if (line.startsWith("diff --git ")) {
      if (current) {
        files.push(current);
      }
      current = {
        isBinary: false,
        lines: [],
        path: pathFromDiffHeader(line) ?? "",
      };
      continue;
    }

    if (!current) {
      continue;
    }

    if (line.startsWith("Binary files ")) {
      current.isBinary = true;
      continue;
    }

    if (line.startsWith("--- ") || line.startsWith("+++ ")) {
      const path = pathFromDiffMarker(line);
      if (path) {
        current.path = path;
      }
      continue;
    }

    if (line.startsWith("@@")) {
      current.lines.push({ kind: "hunk", prefix: "", text: line });
      continue;
    }

    if (line.startsWith("+")) {
      current.lines.push({ kind: "add", prefix: "+", text: line.slice(1) });
      continue;
    }

    if (line.startsWith("-")) {
      current.lines.push({ kind: "remove", prefix: "-", text: line.slice(1) });
      continue;
    }

    if (line.startsWith(" ")) {
      current.lines.push({ kind: "context", prefix: " ", text: line.slice(1) });
      continue;
    }

    if (line.startsWith("\\")) {
      current.lines.push({ kind: "meta", prefix: "", text: line });
    }
  }

  if (current) {
    files.push(current);
  }

  return files.filter((file) => file.path);
}

function pathFromDiffHeader(line: string) {
  const prefix = "diff --git a/";
  if (!line.startsWith(prefix)) {
    return null;
  }

  const rest = line.slice(prefix.length);
  const nextMarker = rest.indexOf(" b/");
  return nextMarker >= 0 ? rest.slice(0, nextMarker) : rest;
}

function pathFromDiffMarker(line: string) {
  const marker = line.slice(4);
  if (marker === "/dev/null") {
    return null;
  }

  if (marker.startsWith("a/") || marker.startsWith("b/")) {
    return marker.slice(2);
  }

  return marker;
}

function fieldValue(
  value: Record<string, unknown>,
  camelName: string,
  snakeName?: string,
) {
  if (typeof value[camelName] !== "undefined") {
    return value[camelName];
  }

  if (snakeName && typeof value[snakeName] !== "undefined") {
    return value[snakeName];
  }

  return undefined;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
