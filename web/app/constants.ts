import type { CSSProperties } from "react";

import type { SettingsSection } from "../api/types";

export const DEFAULT_SYSTEM_PROMPT_NAME = "Default";
export const CREATE_BRANCH_OPTION_VALUE = "__create_branch__";
export const CHAT_BOTTOM_LOCK_THRESHOLD_PX = 24;
export const STREAM_CONTEXT_USAGE_REFRESH_DELAY_MS = 1200;
export const WORKSPACE_CHAT_HISTORY_PAGE_SIZE = 5;
export const WORKSPACE_SIDEBAR_MIN_WIDTH = 232;
export const WORKSPACE_SIDEBAR_MAX_WIDTH = 420;
export const CONTEXT_PANEL_MIN_WIDTH = 280;
export const CONTEXT_PANEL_MAX_WIDTH = 720;
export const CONTEXT_PANEL_MIN_HEIGHT = 224;
export const CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT = 280;
export const CONTEXT_PANEL_MAX_HEIGHT_RATIO = 0.72;
export const MOBILE_BREAKPOINT_PX = 768;
export const MAX_CHAT_ATTACHMENTS = 6;
export const MAX_CHAT_ATTACHMENT_BYTES = 10 * 1024 * 1024;
export const MAX_CHAT_ATTACHMENT_TOTAL_BYTES = 24 * 1024 * 1024;
export const SAVED_PASSWORD_MASK = "********";
export const SETTINGS_SECTION_IDS: SettingsSection[] = [
  "general",
  "prompts",
  "web-search",
  "workspaces",
  "hooks",
  "memory",
  "providers",
  "models",
  "mcp",
  "skills",
];
export const MEMORY_KIND_OPTIONS = [
  "user_note",
  "preference",
  "project_fact",
  "project_decision",
  "procedure",
  "constraint",
  "episode",
];
export const AI_STATS_COLUMN_IDS = [
  "requestTime",
  "workspace",
  "chat",
  "provider",
  "model",
  "inputTokens",
  "outputTokens",
  "cacheRead",
  "cacheWrite",
  "cacheRatio",
  "latency",
  "firstToken",
  "statusCode",
  "status",
  "details",
] as const;
export type AiStatsColumnId = (typeof AI_STATS_COLUMN_IDS)[number];
export const AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY = "foco.aiStats.visibleColumns";
export const DEFAULT_AI_STATS_COLUMN_IDS: AiStatsColumnId[] = [...AI_STATS_COLUMN_IDS];
export const ANALYTICS_CHART_COLORS = [
  "#0f766e",
  "#2563eb",
  "#7c3aed",
  "#dc2626",
  "#ca8a04",
  "#16a34a",
  "#475569",
  "#db2777",
];
export const chartTooltipStyle: CSSProperties = {
  backgroundColor: "#ffffff",
  border: "1px solid #e7e5e4",
  borderRadius: "10px",
  boxShadow: "0 12px 28px rgba(33, 31, 28, 0.14)",
  color: "#1c1917",
  fontSize: "12px",
};
export const chartTooltipLabelStyle: CSSProperties = {
  color: "#57534e",
  fontWeight: 700,
  marginBottom: "4px",
};