import { useCallback, type RefObject } from "react";

import type { BrowserRoute, SettingsSection, WorkspaceSummary } from "../api/types";

type AppRoutingOptions = {
  activeChatId: string | null;
  activeChatKeyRef: RefObject<string | null>;
  activeWorkspaceIdOrNull: string | null;
  onMissingWorkspace: (message: string) => void;
  onSelectWorkspaceChat: (
    workspaceId: string,
    chatId: string,
    options: { updateUrl?: boolean },
  ) => void;
  onStartNewWorkspaceChat: (
    workspaceId: string,
    options: { updateUrl?: boolean },
  ) => void;
  setActiveChatId: (chatId: string | null) => void;
  setIsMobileWorkspaceOpen: (isOpen: boolean) => void;
  setMessages: (messages: []) => void;
  setSettingsSection: (section: SettingsSection) => void;
  setViewMode: (viewMode: "chat" | "settings" | "stats") => void;
  updateBrowserRoute: (
    route: BrowserRoute,
    mode?: "push" | "replace",
  ) => void;
  workspaces: WorkspaceSummary[];
};

export function useAppRouting({
  activeChatId,
  activeChatKeyRef,
  activeWorkspaceIdOrNull,
  onMissingWorkspace,
  onSelectWorkspaceChat,
  onStartNewWorkspaceChat,
  setActiveChatId,
  setIsMobileWorkspaceOpen,
  setMessages,
  setSettingsSection,
  setViewMode,
  updateBrowserRoute,
  workspaces,
}: AppRoutingOptions) {
  const currentChatBrowserRoute = useCallback((): BrowserRoute => {
    return {
      chatId: activeChatId,
      viewMode: "chat",
      workspaceId: activeWorkspaceIdOrNull,
    };
  }, [activeChatId, activeWorkspaceIdOrNull]);

  const openSettingsSection = useCallback(
    (section: SettingsSection) => {
      setSettingsSection(section);
      setViewMode("settings");
      setIsMobileWorkspaceOpen(false);
      updateBrowserRoute({ section, viewMode: "settings" });
    },
    [
      setIsMobileWorkspaceOpen,
      setSettingsSection,
      setViewMode,
      updateBrowserRoute,
    ],
  );

  const openStatsView = useCallback(() => {
    setViewMode("stats");
    setIsMobileWorkspaceOpen(false);
    updateBrowserRoute({ viewMode: "stats" });
  }, [setIsMobileWorkspaceOpen, setViewMode, updateBrowserRoute]);

  const openCurrentChatView = useCallback(() => {
    setViewMode("chat");
    updateBrowserRoute(currentChatBrowserRoute());
  }, [currentChatBrowserRoute, setViewMode, updateBrowserRoute]);

  const applyBrowserRoute = useCallback(
    (route: BrowserRoute) => {
      if (route.viewMode === "settings") {
        setSettingsSection(route.section);
        setViewMode("settings");
        setIsMobileWorkspaceOpen(false);
        return;
      }

      if (route.viewMode === "stats") {
        setViewMode("stats");
        setIsMobileWorkspaceOpen(false);
        return;
      }

      setViewMode("chat");
      setIsMobileWorkspaceOpen(false);
      if (!route.workspaceId) {
        setActiveChatId(null);
        activeChatKeyRef.current = null;
        setMessages([]);
        return;
      }

      if (!workspaces.some((workspace) => workspace.id === route.workspaceId)) {
        onMissingWorkspace(`Workspace not found: ${route.workspaceId}`);
        return;
      }

      if (route.chatId) {
        onSelectWorkspaceChat(route.workspaceId, route.chatId, {
          updateUrl: false,
        });
        return;
      }

      onStartNewWorkspaceChat(route.workspaceId, { updateUrl: false });
    },
    [
      activeChatKeyRef,
      onMissingWorkspace,
      onSelectWorkspaceChat,
      onStartNewWorkspaceChat,
      setActiveChatId,
      setIsMobileWorkspaceOpen,
      setMessages,
      setSettingsSection,
      setViewMode,
      workspaces,
    ],
  );

  return {
    applyBrowserRoute,
    currentChatBrowserRoute,
    openCurrentChatView,
    openSettingsSection,
    openStatsView,
  };
}
