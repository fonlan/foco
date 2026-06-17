import { describe, expect, it } from "vitest";

import {
  diffLineClass,
  hasGitDiffStats,
  parseGitDiffLineStats,
  parseGitDiffSections,
} from "./diff-parser";

describe("git diff parser", () => {
  it("parses staged and unstaged file sections including binary notices", () => {
    const sections = parseGitDiffSections({
      diff: "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-old\n+new\n context\n\\ No newline at end of file",
      files: [],
      isGitRepository: true,
      stagedDiff: "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ",
      status: "",
    });

    expect(sections).toHaveLength(2);
    expect(sections[0]).toMatchObject({
      kind: "staged",
      files: [{ isBinary: true, path: "logo.png" }],
    });
    expect(sections[1].files[0].lines.map((line) => line.kind)).toEqual([
      "hunk",
      "remove",
      "add",
      "context",
      "meta",
    ]);
  });

  it("validates line stats and maps classes", () => {
    expect(parseGitDiffLineStats({ additions: 2, deletions: 1 })).toEqual({ additions: 2, deletions: 1 });
    expect(parseGitDiffLineStats({ additions: -1, deletions: 1 })).toBeNull();
    expect(hasGitDiffStats({ additions: 0, deletions: 0 })).toBe(false);
    expect(hasGitDiffStats({ additions: 1, deletions: 0 })).toBe(true);
    expect(diffLineClass("add")).toContain("bg-emerald-50");
    expect(diffLineClass("remove")).toContain("bg-rose-50");
  });
});
