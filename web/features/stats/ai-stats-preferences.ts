import {
  AI_STATS_COLUMN_IDS,
  AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY,
  DEFAULT_AI_STATS_COLUMN_IDS,
  type AiStatsColumnId,
} from "../../app/constants";

export function readAiStatsVisibleColumnIds(): Set<AiStatsColumnId> {
  const savedValue = window.localStorage.getItem(AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY);
  if (!savedValue) {
    return new Set(DEFAULT_AI_STATS_COLUMN_IDS);
  }

  const savedIds = JSON.parse(savedValue);
  if (!Array.isArray(savedIds)) {
    return new Set(DEFAULT_AI_STATS_COLUMN_IDS);
  }

  const visibleIds = savedIds.filter(isAiStatsColumnId);
  const hasCombinedColumn = visibleIds.some((columnId) =>
    AI_STATS_COMBINED_COLUMN_IDS.has(columnId),
  );
  return new Set(
    visibleIds.length && hasCombinedColumn
      ? visibleIds
      : DEFAULT_AI_STATS_COLUMN_IDS,
  );
}

export function writeAiStatsVisibleColumnIds(
  visibleColumnIds: Set<AiStatsColumnId>,
) {
  const savedIds = AI_STATS_COLUMN_IDS.filter((columnId) =>
    visibleColumnIds.has(columnId),
  );
  window.localStorage.setItem(
    AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY,
    JSON.stringify(savedIds),
  );
}

const AI_STATS_COMBINED_COLUMN_IDS = new Set<AiStatsColumnId>([
  "session",
  "providerModel",
  "input",
  "output",
  "duration",
]);

function isAiStatsColumnId(value: unknown): value is AiStatsColumnId {
  return (
    typeof value === "string" &&
    (AI_STATS_COLUMN_IDS as readonly string[]).includes(value)
  );
}
