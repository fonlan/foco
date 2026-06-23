import type {
  BrowserRoute,
  BrowserRouteChatTab,
  BrowserRouteFileTab,
  SettingsSection,
} from "../api/types";
import { SETTINGS_SECTION_IDS } from "../app/constants";

const CHAT_TAB_QUERY_PARAM = "tab";
const FILE_TAB_QUERY_PARAM = "file";
const ACTIVE_FILE_QUERY_PARAM = "activeFile";
const STATS_PAGE_QUERY_PARAM = "page";

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
    return { page: positivePageFromSearch(search), viewMode: "stats" };
  }

  if (segments[0] === "scheduled") {
    return { viewMode: "scheduled" };
  }

  const tabs = chatTabsFromSearch(search);
  const files = fileTabsFromSearch(search);
  const activeFile = activeFileFromSearch(search);

  if (segments.length >= 2) {
    return chatRouteWithTabs({
      chatId: segments[1],
      viewMode: "chat",
      workspaceId: segments[0],
    }, tabs, files, activeFile);
  }

  if (segments.length === 1) {
    return chatRouteWithTabs(
      { chatId: null, viewMode: "chat", workspaceId: segments[0] },
      tabs,
      files,
      activeFile,
    );
  }

  return chatRouteWithTabs(
    { chatId: null, viewMode: "chat", workspaceId: null },
    tabs,
    files,
    activeFile,
  );
}

export function browserPathForRoute(route: BrowserRoute) {
  if (route.viewMode === "settings") {
    return `/settings/${route.section}`;
  }

  if (route.viewMode === "stats") {
    const params = new URLSearchParams();
    params.set(STATS_PAGE_QUERY_PARAM, String(positivePage(route.page)));
    return `/stats?${params.toString()}`;
  }

  if (route.viewMode === "scheduled") {
    return "/scheduled";
  }

  const path = browserPathnameForChatRoute(route);
  const search = chatRouteSearch(route);
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

function positivePageFromSearch(search: string) {
  const rawPage = new URLSearchParams(search).get(STATS_PAGE_QUERY_PARAM);
  return positivePage(Number(rawPage));
}

function positivePage(value: number) {
  return Number.isSafeInteger(value) && value > 0 ? value : 1;
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

function fileTabsFromSearch(search: string): BrowserRouteFileTab[] {
  const params = new URLSearchParams(search);
  const files: BrowserRouteFileTab[] = [];
  const seen = new Set<string>();

  for (const value of params.getAll(FILE_TAB_QUERY_PARAM)) {
    const file = fileTabFromParamValue(value);
    if (!file) {
      continue;
    }

    const key = `${file.workspaceId}\u0000${file.path}`;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    files.push(file);
  }

  return files;
}

function activeFileFromSearch(search: string) {
  const value = new URLSearchParams(search).get(ACTIVE_FILE_QUERY_PARAM);
  return value ? fileTabFromParamValue(value) : null;
}

function fileTabFromParamValue(value: string): BrowserRouteFileTab | null {
  const separatorIndex = value.indexOf("/");
  if (separatorIndex <= 0 || separatorIndex >= value.length - 1) {
    return null;
  }

  const workspaceId = decodeTabComponent(value.slice(0, separatorIndex));
  const path = decodeTabComponent(value.slice(separatorIndex + 1));
  if (!workspaceId || !path) {
    return null;
  }

  return { path, workspaceId };
}

function chatRouteSearch(route: Extract<BrowserRoute, { viewMode: "chat" }>) {
  const params = new URLSearchParams();
  appendChatTabsSearch(params, route.tabs ?? []);
  appendFileTabsSearch(params, route.files ?? []);

  if (route.activeFile?.workspaceId && route.activeFile.path) {
    params.set(ACTIVE_FILE_QUERY_PARAM, fileTabParamValue(route.activeFile));
  }

  return params.toString();
}

function appendChatTabsSearch(params: URLSearchParams, tabs: BrowserRouteChatTab[]) {
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
}

function appendFileTabsSearch(params: URLSearchParams, files: BrowserRouteFileTab[]) {
  const seen = new Set<string>();
  for (const file of files) {
    if (!file.workspaceId || !file.path) {
      continue;
    }

    const key = `${file.workspaceId}\u0000${file.path}`;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    params.append(FILE_TAB_QUERY_PARAM, fileTabParamValue(file));
  }
}

function fileTabParamValue(file: BrowserRouteFileTab) {
  return `${encodeURIComponent(file.workspaceId)}/${encodeURIComponent(file.path)}`;
}

function chatRouteWithTabs(
  route: Extract<BrowserRoute, { viewMode: "chat" }>,
  tabs: BrowserRouteChatTab[],
  files: BrowserRouteFileTab[],
  activeFile: BrowserRouteFileTab | null,
): BrowserRoute {
  const routeTabs = route.workspaceId && route.chatId
    ? [...tabs, { chatId: route.chatId, workspaceId: route.workspaceId }]
    : tabs;
  const dedupedRouteTabs = dedupeChatTabs(routeTabs);
  const dedupedRouteFiles = dedupeFileTabs(activeFile ? [...files, activeFile] : files);
  const nextRoute: Extract<BrowserRoute, { viewMode: "chat" }> = {
    ...route,
    ...(dedupedRouteTabs.length ? { tabs: dedupedRouteTabs } : {}),
    ...(dedupedRouteFiles.length ? { files: dedupedRouteFiles } : {}),
  };

  return activeFile ? { ...nextRoute, activeFile } : nextRoute;
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

function dedupeFileTabs(files: BrowserRouteFileTab[]) {
  const seen = new Set<string>();
  return files.filter((file) => {
    const key = `${file.workspaceId}\u0000${file.path}`;
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
