import {
  DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
  MODEL_ROUTING_EXPANDED_STORAGE_KEY,
  MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY,
  MODEL_ROUTING_PANEL_MIN_HEIGHT_PX,
  WORKSPACE_NAV_MIN_HEIGHT_PX,
} from "../../app/constants";

export function readModelRoutingExpanded(defaultExpanded = false): boolean {
  try {
    const saved = window.localStorage.getItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY);
    if (saved === "1" || saved === "true") {
      return true;
    }
    if (saved === "0" || saved === "false") {
      return false;
    }
  } catch {
    // ponytail: localStorage may be unavailable in private mode; fall back to default.
  }
  return defaultExpanded;
}

export function writeModelRoutingExpanded(expanded: boolean) {
  try {
    window.localStorage.setItem(
      MODEL_ROUTING_EXPANDED_STORAGE_KEY,
      expanded ? "1" : "0",
    );
  } catch {
    // ignore persistence failures
  }
}

/**
 * Accept finite panel-share ratios in (0, 1]. Invalid values fall back to ~42%.
 * Ratio 1 is valid (panel uses the whole flexible stack on a short viewport).
 */
export function normalizeModelRoutingHeightRatio(value: unknown): number {
  const ratio =
    typeof value === "number"
      ? value
      : typeof value === "string"
        ? Number(value)
        : Number.NaN;
  if (!Number.isFinite(ratio) || ratio <= 0 || ratio > 1) {
    return DEFAULT_MODEL_ROUTING_HEIGHT_RATIO;
  }
  return ratio;
}

export function readModelRoutingHeightRatio(
  defaultRatio = DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
): number {
  try {
    const saved = window.localStorage.getItem(
      MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY,
    );
    if (saved == null) {
      return defaultRatio;
    }
    return normalizeModelRoutingHeightRatio(saved);
  } catch {
    // ponytail: localStorage may be unavailable in private mode; fall back to default.
  }
  return defaultRatio;
}

export function writeModelRoutingHeightRatio(ratio: number) {
  const normalized = normalizeModelRoutingHeightRatio(ratio);
  try {
    window.localStorage.setItem(
      MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY,
      String(normalized),
    );
  } catch {
    // ignore persistence failures
  }
}

/**
 * Legal panel height range inside the flexible stack shared with `.workspace-nav`.
 * When both CSS mins fit, enforce them. When the stack is shorter than
 * panelMin + navMin, shrink the panel floor so JS does not force overflow past
 * the nav CSS min-height (prefer keeping nav usable).
 */
export function modelRoutingPanelHeightBounds(availableHeight: number): {
  min: number;
  max: number;
} {
  if (!Number.isFinite(availableHeight) || availableHeight <= 0) {
    return { min: 0, max: 0 };
  }

  const desiredMinPanel = MODEL_ROUTING_PANEL_MIN_HEIGHT_PX;
  const desiredMinNav = WORKSPACE_NAV_MIN_HEIGHT_PX;

  if (availableHeight >= desiredMinPanel + desiredMinNav) {
    return {
      min: desiredMinPanel,
      max: availableHeight - desiredMinNav,
    };
  }

  // Short stack: reserve as much of the nav min as still fits; panel gets the rest.
  // This keeps min <= max and avoids inventing height beyond availableHeight.
  const reservedNav = Math.min(desiredMinNav, availableHeight);
  const max = Math.max(0, availableHeight - reservedNav);
  const min = Math.min(desiredMinPanel, max);
  return { min, max };
}

/**
 * Clamp model-routing panel height within the flexible stack shared with
 * `.workspace-nav`. Does not invent extra space beyond availableHeight.
 */
export function clampModelRoutingPanelHeight(
  height: number,
  availableHeight: number,
): number {
  const { min, max } = modelRoutingPanelHeightBounds(availableHeight);
  if (max <= 0 && min <= 0) {
    return 0;
  }
  const target = Number.isFinite(height) ? height : min;
  return Math.min(Math.max(target, min), max);
}

export function panelHeightFromRatio(
  ratio: number,
  availableHeight: number,
): number {
  const safeRatio = normalizeModelRoutingHeightRatio(ratio);
  return clampModelRoutingPanelHeight(
    Math.round(availableHeight * safeRatio),
    availableHeight,
  );
}

/**
 * Convert a clamped panel height to a durable ratio. Legitimate extremes
 * (panel uses all / nearly none of the stack) stay near 0/1 instead of snapping
 * back to the default 0.42 used for invalid storage input.
 */
export function ratioFromPanelHeight(
  panelHeight: number,
  availableHeight: number,
): number {
  if (!Number.isFinite(availableHeight) || availableHeight <= 0) {
    return DEFAULT_MODEL_ROUTING_HEIGHT_RATIO;
  }
  if (!Number.isFinite(panelHeight)) {
    return DEFAULT_MODEL_ROUTING_HEIGHT_RATIO;
  }
  const raw = panelHeight / availableHeight;
  if (!Number.isFinite(raw)) {
    return DEFAULT_MODEL_ROUTING_HEIGHT_RATIO;
  }
  // Keep open-unit (0, 1]; avoid 0 which is rejected by normalize as invalid.
  if (raw <= 0) {
    const { min, max } = modelRoutingPanelHeightBounds(availableHeight);
    const floor = max > 0 ? min / availableHeight : DEFAULT_MODEL_ROUTING_HEIGHT_RATIO;
    return normalizeModelRoutingHeightRatio(Math.max(floor, Number.EPSILON));
  }
  if (raw > 1) {
    return 1;
  }
  return raw;
}
