import type { BrowserRoute, BrowserRouteChatTab, SettingsSection } from "../api/types";
import { SETTINGS_SECTION_IDS } from "../app/constants";

const CHAT_TAB_QUERY_PARAM = "tab";

export function currentBrowserRoute(): BrowserRoute {
  if (typeof window === "undefined") {
    return { chatId: null, viewMode: "chat", workspaceId: null };
  }

  return browserRouteFromPathname(window.location.pathname, window.location.search);
}

export function browserRouteFromPathname(
  pathname: string,
  search = "",
): BrowserRoute {
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

  if (segments[0] === "scheduled") {
    return { viewMode: "scheduled" };
  }

  const tabs = chatTabsFromSearch(search);

  if (segments.length >= 2) {
    return chatRouteWithTabs({
      chatId: segments[1],
      viewMode: "chat",
      workspaceId: segments[0],
    }, tabs);
  }

  if (segments.length === 1) {
    return chatRouteWithTabs(
      { chatId: null, viewMode: "chat", workspaceId: segments[0] },
      tabs,
    );
  }

  return chatRouteWithTabs(
    { chatId: null, viewMode: "chat", workspaceId: null },
    tabs,
  );
}

export function browserPathForRoute(route: BrowserRoute) {
  if (route.viewMode === "settings") {
    return `/settings/${route.section}`;
  }

  if (route.viewMode === "stats") {
    return "/stats";
  }

  if (route.viewMode === "scheduled") {
    return "/scheduled";
  }

  const path = browserPathnameForChatRoute(route);
  const search = chatTabsSearch(route.tabs ?? []);
  return search ? `${path}?${search}` : path;
}

function browserPathnameForChatRoute(
  route: Extract<BrowserRoute, { viewMode: "chat" }>,
) {
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

function chatTabsFromSearch(search: string): BrowserRouteChatTab[] {
  const params = new URLSearchParams(search);
  const tabs: BrowserRouteChatTab[] = [];
  const seen = new Set<string>();

  for (const value of params.getAll(CHAT_TAB_QUERY_PARAM)) {
    const tab = chatTabFromParamValue(value);
    if (!tab) {
      continue;
    }

    const key = `${tab.workspaceId}\u0000${tab.chatId}`;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    tabs.push(tab);
  }

  return tabs;
}

function chatTabFromParamValue(value: string): BrowserRouteChatTab | null {
  const separatorIndex = value.indexOf("/");
  if (separatorIndex <= 0 || separatorIndex >= value.length - 1) {
    return null;
  }

  const workspaceId = decodeTabComponent(value.slice(0, separatorIndex));
  const chatId = decodeTabComponent(value.slice(separatorIndex + 1));
  if (!workspaceId || !chatId) {
    return null;
  }

  return { chatId, workspaceId };
}

function chatTabsSearch(tabs: BrowserRouteChatTab[]) {
  if (!tabs.length) {
    return "";
  }

  const params = new URLSearchParams();
  const seen = new Set<string>();
  for (const tab of tabs) {
    if (!tab.workspaceId || !tab.chatId) {
      continue;
    }

    const key = `${tab.workspaceId}\u0000${tab.chatId}`;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    params.append(
      CHAT_TAB_QUERY_PARAM,
      `${encodeURIComponent(tab.workspaceId)}/${encodeURIComponent(tab.chatId)}`,
    );
  }

  return params.toString();
}

function chatRouteWithTabs(
  route: Extract<BrowserRoute, { viewMode: "chat" }>,
  tabs: BrowserRouteChatTab[],
): BrowserRoute {
  const routeTabs = route.workspaceId && route.chatId
    ? [...tabs, { chatId: route.chatId, workspaceId: route.workspaceId }]
    : tabs;
  const dedupedRouteTabs = dedupeChatTabs(routeTabs);

  return dedupedRouteTabs.length ? { ...route, tabs: dedupedRouteTabs } : route;
}

function dedupeChatTabs(tabs: BrowserRouteChatTab[]) {
  const seen = new Set<string>();
  return tabs.filter((tab) => {
    const key = `${tab.workspaceId}\u0000${tab.chatId}`;
    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function decodePathSegment(segment: string) {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

function decodeTabComponent(value: string) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function settingsSectionFromPathSegment(
  segment: string | undefined,
): SettingsSection {
  return SETTINGS_SECTION_IDS.includes(segment as SettingsSection)
    ? (segment as SettingsSection)
    : "general";
}
