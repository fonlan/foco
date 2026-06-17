import { beforeEach, describe, expect, it } from "vitest";

import { AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY } from "../../app/constants";
import { readAiStatsVisibleColumnIds, writeAiStatsVisibleColumnIds } from "./ai-stats-preferences";

describe("AI statistics column preferences", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("uses defaults when storage is missing or invalid", () => {
    expect(readAiStatsVisibleColumnIds().size).toBeGreaterThan(0);
    window.localStorage.setItem(AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY, JSON.stringify(["unknown"]));
    expect(readAiStatsVisibleColumnIds().size).toBeGreaterThan(0);
  });

  it("persists only known visible columns", () => {
    writeAiStatsVisibleColumnIds(new Set(["provider", "model"]));
    expect(JSON.parse(window.localStorage.getItem(AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY) ?? "[]")).toEqual([
      "provider",
      "model",
    ]);
    expect([...readAiStatsVisibleColumnIds()]).toEqual(["provider", "model"]);
  });
});
