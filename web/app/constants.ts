import type { CSSProperties } from "react";

import type { SettingsSection } from "../api/types";

export const DEFAULT_SYSTEM_PROMPT_NAME = "Default";
export const IMAGE_AGENT_SYSTEM_PROMPT_NAME = "Image Generation";
export const PLAN_MODE_SYSTEM_PROMPT_NAME = "Plan Mode";
export const REVIEW_SYSTEM_PROMPT_NAME = "Review";
export const CHAT_BOTTOM_LOCK_THRESHOLD_PX = 24;
export const WORKSPACE_CHAT_HISTORY_PAGE_SIZE = 5;
export const WORKSPACE_CHAT_CONTEXT_MENU_LONG_PRESS_MS = 520;
export const WORKSPACE_SIDEBAR_MIN_WIDTH = 232;
export const WORKSPACE_SIDEBAR_MAX_WIDTH = 420;
export const CONTEXT_PANEL_MIN_WIDTH = 280;
export const CONTEXT_PANEL_DEFAULT_WIDTH = 360;
export const CONTEXT_PANEL_MAX_WIDTH = 720;
export const CONTEXT_PANEL_MIN_HEIGHT = 224;
export const CONTEXT_PANEL_DEFAULT_MOBILE_HEIGHT = 280;
export const CONTEXT_PANEL_MAX_HEIGHT_RATIO = 0.72;
/** Phone navigation/UI breakpoint (CSS max-width: 767px). */
export const MOBILE_BREAKPOINT_PX = 768;
/**
 * Context panel stacked (main above panel) layout breakpoint.
 * Matches CSS `@media (max-width: 1199px)` → JS `innerWidth < 1200`.
 */
export const CONTEXT_PANEL_STACKED_BREAKPOINT_PX = 1200;
export const MAX_CHAT_ATTACHMENTS = 6;
export const MAX_CHAT_ATTACHMENT_BYTES = 10 * 1024 * 1024;
export const MAX_CHAT_ATTACHMENT_TOTAL_BYTES = 24 * 1024 * 1024;
export const SAVED_PASSWORD_MASK = "********";
export const SETTINGS_SECTION_IDS: SettingsSection[] = [
  "general",
  "agents",
  "prompts",
  "spec",
  "plan",
  "web-search",
  "workspaces",
  "remote-servers",
  "hooks",
  "memory",
  "providers",
  "models",
  "mcp",
  "skills",
  "about",
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
  "session",
  "providerModel",
  "requestKind",
  "thinkingLevel",
  "input",
  "output",
  "duration",
  "status",
  "details",
] as const;
export type AiStatsColumnId = (typeof AI_STATS_COLUMN_IDS)[number];
export const AI_STATS_VISIBLE_COLUMNS_STORAGE_KEY = "foco.aiStats.visibleColumns";
export const PLAN_AUTO_RUN_ENABLED_STORAGE_KEY = "foco.planAutoRun.enabled";
export const MODEL_ROUTING_EXPANDED_STORAGE_KEY = "foco.modelRouting.expanded";
export const MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY =
  "foco.modelRouting.heightRatio";
/** Default share of the flexible sidebar stack (workspace-nav + model routing). */
export const DEFAULT_MODEL_ROUTING_HEIGHT_RATIO = 0.42;
export const MODEL_ROUTING_PANEL_MIN_HEIGHT_PX = 120;
export const WORKSPACE_NAV_MIN_HEIGHT_PX = 120;
export const MODEL_ROUTING_RESIZE_STEP_PX = 24;
export function planAutoRunEnabledStorageKey(workspaceId: string) {
  return `${PLAN_AUTO_RUN_ENABLED_STORAGE_KEY}.${encodeURIComponent(workspaceId)}`;
}
export const DEFAULT_AI_STATS_COLUMN_IDS: AiStatsColumnId[] = [...AI_STATS_COLUMN_IDS];
export const ANALYTICS_CHART_COLORS = [
  "var(--chart-primary)",
  "var(--link)",
  "var(--success)",
  "var(--danger)",
  "var(--warning)",
  "var(--default)",
  "var(--muted)",
  "var(--foreground)",
];
export function chartColor(index: number) {
  return ANALYTICS_CHART_COLORS[index % ANALYTICS_CHART_COLORS.length];
}
export const chartTooltipStyle: CSSProperties = {
  backgroundColor: "var(--overlay)",
  border: "1px solid var(--border)",
  borderRadius: "10px",
  boxShadow: "var(--overlay-shadow)",
  color: "var(--overlay-foreground)",
  fontSize: "12px",
};
export const chartTooltipLabelStyle: CSSProperties = {
  color: "var(--muted)",
  fontWeight: 700,
  marginBottom: "4px",
};
