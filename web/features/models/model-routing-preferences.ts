import { MODEL_ROUTING_EXPANDED_STORAGE_KEY } from "../../app/constants";

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
