import { useCallback, useEffect, useRef, useState } from "react";

import type { ConfiguredSkillSummary } from "../../api/types";
import { errorMessage } from "../../shared/api-client";
import { fetchWorkspaceSkillCatalog } from "../skills/workspace-skill-catalog";

export type WorkspaceSkillCatalogStatus = "idle" | "loading" | "ready" | "error";

export type WorkspaceSkillCatalogState = {
  /** Enabled skills for the active workspace menu (authoritative when status is ready). */
  skills: ConfiguredSkillSummary[];
  status: WorkspaceSkillCatalogStatus;
  /** Hard error when no successful catalog is available for the active workspace. */
  error: string | null;
  /** Soft error when a refresh failed but a prior successful cache is still shown. */
  refreshError: string | null;
  workspaceId: string | null;
};

function enabledMenuSkills(skills: ConfiguredSkillSummary[]) {
  return skills.filter((skill) => skill.enabled);
}

/**
 * Loads the workspace-scoped skill catalog used by the composer slash menu.
 * Caches successful responses per workspaceId, ignores late responses after
 * workspace switches, and never falls back to host-local settings detection.
 */
export function useWorkspaceSkillCatalog(workspaceId: string | null) {
  const cacheRef = useRef(new Map<string, ConfiguredSkillSummary[]>());
  const requestGenerationRef = useRef(0);
  const [reloadToken, setReloadToken] = useState(0);
  const [state, setState] = useState<WorkspaceSkillCatalogState>({
    skills: [],
    status: "idle",
    error: null,
    refreshError: null,
    workspaceId: null,
  });

  const invalidate = useCallback((targetWorkspaceId?: string | null) => {
    if (targetWorkspaceId) {
      cacheRef.current.delete(targetWorkspaceId);
    } else {
      cacheRef.current.clear();
    }
    setReloadToken((current) => current + 1);
  }, []);

  const reload = useCallback(() => {
    setReloadToken((current) => current + 1);
  }, []);

  useEffect(() => {
    if (!workspaceId) {
      requestGenerationRef.current += 1;
      setState({
        skills: [],
        status: "idle",
        error: null,
        refreshError: null,
        workspaceId: null,
      });
      return;
    }

    const generation = ++requestGenerationRef.current;
    const cached = cacheRef.current.get(workspaceId);
    const hasCache = cached !== undefined;

    setState({
      skills: hasCache ? enabledMenuSkills(cached) : [],
      status: hasCache ? "ready" : "loading",
      error: null,
      refreshError: null,
      workspaceId,
    });

    let cancelled = false;

    void (async () => {
      try {
        const data = await fetchWorkspaceSkillCatalog(workspaceId);
        if (cancelled || requestGenerationRef.current !== generation) {
          return;
        }

        cacheRef.current.set(workspaceId, data.skills);
        setState({
          skills: enabledMenuSkills(data.skills),
          status: "ready",
          error: null,
          refreshError: null,
          workspaceId,
        });
      } catch (requestError) {
        if (cancelled || requestGenerationRef.current !== generation) {
          return;
        }

        const message = errorMessage(requestError);
        const latestCache = cacheRef.current.get(workspaceId);
        if (latestCache !== undefined) {
          setState({
            skills: enabledMenuSkills(latestCache),
            status: "ready",
            error: null,
            refreshError: message,
            workspaceId,
          });
          return;
        }

        setState({
          skills: [],
          status: "error",
          error: message,
          refreshError: null,
          workspaceId,
        });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [reloadToken, workspaceId]);

  // Derive a synchronous view so a workspace switch never paints the previous
  // workspace catalog for one render before the effect resets state.
  const isCurrentWorkspace = state.workspaceId === workspaceId;
  const view: WorkspaceSkillCatalogState = !workspaceId
    ? {
        skills: [],
        status: "idle",
        error: null,
        refreshError: null,
        workspaceId: null,
      }
    : isCurrentWorkspace
      ? state
      : {
          skills: [],
          status: "loading",
          error: null,
          refreshError: null,
          workspaceId,
        };

  return {
    ...view,
    invalidate,
    reload,
  };
}
