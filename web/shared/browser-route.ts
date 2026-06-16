import type { BrowserRoute, SettingsSection } from "../api/types";
import { SETTINGS_SECTION_IDS } from "../app/constants";

export function currentBrowserRoute(): BrowserRoute {
  if (typeof window === "undefined") {
    return { chatId: null, viewMode: "chat", workspaceId: null };
  }

  return browserRouteFromPathname(window.location.pathname);
}

export function browserRouteFromPathname(pathname: string): BrowserRoute {
  const segments = pathname
    .split("/")
    .filter(Boolean)
    .map(decodePathSegment);

  if (segments[0] === "settings") {
    const section = settingsSectionFromPathSegment(segments[1]);
    return { section, viewMode: "settings" };
  }

  if (segments[0] === "stats") {
    return { viewMode: "stats" };
  }

  if (segments.length >= 2) {
    return {
      chatId: segments[1],
      viewMode: "chat",
      workspaceId: segments[0],
    };
  }

  if (segments.length === 1) {
    return { chatId: null, viewMode: "chat", workspaceId: segments[0] };
  }

  return { chatId: null, viewMode: "chat", workspaceId: null };
}

export function browserPathForRoute(route: BrowserRoute) {
  if (route.viewMode === "settings") {
    return `/settings/${route.section}`;
  }

  if (route.viewMode === "stats") {
    return "/stats";
  }

  if (route.workspaceId && route.chatId) {
    return `/${encodeURIComponent(route.workspaceId)}/${encodeURIComponent(
      route.chatId,
    )}`;
  }

  if (route.workspaceId) {
    return `/${encodeURIComponent(route.workspaceId)}`;
  }

  return "/";
}

function decodePathSegment(segment: string) {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

function settingsSectionFromPathSegment(
  segment: string | undefined,
): SettingsSection {
  return SETTINGS_SECTION_IDS.includes(segment as SettingsSection)
    ? (segment as SettingsSection)
    : "general";
}