import type { WorkspaceSkillsDiscoveryResponse } from "../../api/types";
import { requestJson } from "../../shared/api-client";

export function workspaceSkillCatalogUrl(workspaceId: string) {
  return `/api/workspaces/${encodeURIComponent(workspaceId)}/skills`;
}

/** Loads the authoritative Skill catalog for one workspace. */
export function fetchWorkspaceSkillCatalog(workspaceId: string) {
  return requestJson<WorkspaceSkillsDiscoveryResponse>(workspaceSkillCatalogUrl(workspaceId));
}
