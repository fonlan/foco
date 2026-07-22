import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { ConfiguredSkillSummary, ConfiguredWorkspaceSummary } from "../../api/types";
import { errorMessage } from "../../shared/api-client";
import { fetchWorkspaceSkillCatalog } from "../skills/workspace-skill-catalog";

export type RemoteWorkspaceSkillCatalogStatus = "error" | "loading" | "ready";

export type RemoteWorkspaceSkillCatalog = {
  workspace: ConfiguredWorkspaceSummary;
  skills: ConfiguredSkillSummary[];
  status: RemoteWorkspaceSkillCatalogStatus;
  error: string | null;
  refreshError: string | null;
};

const EMPTY_REMOTE_WORKSPACES: ConfiguredWorkspaceSummary[] = [];

function remoteWorkspaceTargets(workspaces: ConfiguredWorkspaceSummary[]) {
  const remote = workspaces.filter((workspace) => Boolean(workspace.serverId));
  return remote.length === 0 ? EMPTY_REMOTE_WORKSPACES : remote;
}

function workspaceConnectionLooksReady(status: string | undefined) {
  switch (status?.toLowerCase()) {
    case "connected":
    case "ready":
    case "degraded":
      return true;
    default:
      return false;
  }
}

function unavailableWorkspaceMessage(workspace: ConfiguredWorkspaceSummary) {
  return workspace.lastRemoteError || "Remote workspace is not connected.";
}

function normalizeRemoteWorkspaceSkills(
  skills: ConfiguredSkillSummary[],
  workspace: ConfiguredWorkspaceSummary,
) {
  return skills
    .filter((skill) => skill.scope === "workspace" && skill.workspaceId === workspace.id)
    .map((skill) => ({
      ...skill,
      workspaceId: workspace.id,
      workspaceName: workspace.name,
    }));
}

/**
 * Lazily loads remote workspace-only Skills for the Settings panel. Successful
 * catalogs are cached per workspace so a refresh can preserve prior results;
 * every enabled entry revalidates the catalog and rejects stale responses.
 */
export function useRemoteWorkspaceSkillCatalog(
  enabled: boolean,
  workspaces: ConfiguredWorkspaceSummary[],
) {
  const cacheRef = useRef(new Map<string, ConfiguredSkillSummary[]>());
  const requestGenerationRef = useRef(0);
  const workspaceRequestRef = useRef(new Map<string, number>());
  const [reloadToken, setReloadToken] = useState(0);
  const [catalogsByWorkspaceId, setCatalogsByWorkspaceId] = useState<
    Record<string, RemoteWorkspaceSkillCatalog>
  >({});

  // Prefer a content signature over the workspaces array identity. SettingsPanel
  // may pass a new array reference for the same remote set; depending on identity
  // would re-fire this effect forever (setState → render → new array → effect).
  const workspaceSignature = useMemo(
    () =>
      remoteWorkspaceTargets(workspaces)
        .map(
          (workspace) =>
            `${workspace.id}:${workspace.serverId ?? ""}:${workspace.connectionStatus ?? ""}:${workspace.lastRemoteError ?? ""}`,
        )
        .join("\u0000"),
    [workspaces],
  );
  // Recompute only when the signature changes so we keep a stable array identity.
  const remoteWorkspaces = useMemo(
    () => remoteWorkspaceTargets(workspaces),
    // Intentionally omit `workspaces`: identity churn must not rebuild the list.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- signature is the content key
    [workspaceSignature],
  );

  const loadWorkspaceCatalog = useCallback(
    (workspace: ConfiguredWorkspaceSummary, generation: number, force = false) => {
      const requestId = (workspaceRequestRef.current.get(workspace.id) ?? 0) + 1;
      workspaceRequestRef.current.set(workspace.id, requestId);

      const isCurrentRequest = () =>
        requestGenerationRef.current === generation &&
        workspaceRequestRef.current.get(workspace.id) === requestId;
      const cached = cacheRef.current.get(workspace.id);
      if (!force && !workspaceConnectionLooksReady(workspace.connectionStatus)) {
        const message = unavailableWorkspaceMessage(workspace);
        setCatalogsByWorkspaceId((current) => {
          const previous = current[workspace.id];
          if (
            previous &&
            previous.workspace === workspace &&
            previous.skills === (cached ?? previous.skills) &&
            previous.status === (cached === undefined ? "error" : "ready") &&
            previous.error === (cached === undefined ? message : null) &&
            previous.refreshError === (cached === undefined ? null : message)
          ) {
            return current;
          }
          return {
            ...current,
            [workspace.id]: {
              workspace,
              skills: cached ?? [],
              status: cached === undefined ? "error" : "ready",
              error: cached === undefined ? message : null,
              refreshError: cached === undefined ? null : message,
            },
          };
        });
        return;
      }

      setCatalogsByWorkspaceId((current) => {
        const previous = current[workspace.id];
        if (
          previous &&
          previous.workspace === workspace &&
          previous.skills === (cached ?? previous.skills) &&
          previous.status === "loading" &&
          previous.error === null &&
          previous.refreshError === null
        ) {
          return current;
        }
        return {
          ...current,
          [workspace.id]: {
            workspace,
            skills: cached ?? [],
            status: "loading",
            error: null,
            refreshError: null,
          },
        };
      });

      void (async () => {
        try {
          const response = await fetchWorkspaceSkillCatalog(workspace.id);
          if (!isCurrentRequest()) {
            return;
          }

          const skills = normalizeRemoteWorkspaceSkills(response.skills, workspace);
          cacheRef.current.set(workspace.id, skills);
          setCatalogsByWorkspaceId((current) => ({
            ...current,
            [workspace.id]: {
              workspace,
              skills,
              status: "ready",
              error: null,
              refreshError: null,
            },
          }));
        } catch (requestError) {
          if (!isCurrentRequest()) {
            return;
          }

          const message = errorMessage(requestError);
          const latestCached = cacheRef.current.get(workspace.id);
          setCatalogsByWorkspaceId((current) => ({
            ...current,
            [workspace.id]: {
              workspace,
              skills: latestCached ?? [],
              status: latestCached === undefined ? "error" : "ready",
              error: latestCached === undefined ? message : null,
              refreshError: latestCached === undefined ? null : message,
            },
          }));
        }
      })();
    },
    [],
  );

  useEffect(() => {
    if (!enabled) {
      requestGenerationRef.current += 1;
      setCatalogsByWorkspaceId((current) =>
        Object.keys(current).length === 0 ? current : {},
      );
      return;
    }

    const generation = ++requestGenerationRef.current;
    if (!remoteWorkspaces.length) {
      setCatalogsByWorkspaceId((current) =>
        Object.keys(current).length === 0 ? current : {},
      );
      return;
    }

    // Start all independent workspace discoveries together so one unavailable
    // remote server never delays usable catalogs from the others.
    for (const workspace of remoteWorkspaces) {
      loadWorkspaceCatalog(workspace, generation);
    }

    return () => {
      requestGenerationRef.current += 1;
    };
  }, [enabled, loadWorkspaceCatalog, reloadToken, remoteWorkspaces, workspaceSignature]);

  const reload = useCallback(() => {
    setReloadToken((current) => current + 1);
  }, []);

  const retryWorkspace = useCallback(
    (workspaceId: string) => {
      if (!enabled) {
        return;
      }

      const workspace = remoteWorkspaces.find((item) => item.id === workspaceId);
      if (!workspace) {
        return;
      }

      loadWorkspaceCatalog(workspace, requestGenerationRef.current, true);
    },
    [enabled, loadWorkspaceCatalog, remoteWorkspaces],
  );

  const catalogs = remoteWorkspaces
    .map((workspace) => catalogsByWorkspaceId[workspace.id])
    .filter((catalog): catalog is RemoteWorkspaceSkillCatalog => Boolean(catalog));

  return { catalogs, reload, retryWorkspace };
}
