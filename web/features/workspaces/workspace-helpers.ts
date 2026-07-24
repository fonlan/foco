export function workspaceItemClass(active: boolean) {
  return `workspace-item min-w-0 ${active ? "workspace-item-active" : ""}`;
}

export function workspaceNewChatButtonClass(active: boolean) {
  return `accordion__trigger workspace-new-chat-button ${
    active ? "workspace-item-active" : ""
  }`;
}

export function workspaceNameFromPath(path: string) {
  const trimmedPath = path.trim().replace(/[\\/]+$/g, "");
  const parts = trimmedPath.split(/[\\/]+/);

  return parts.at(-1) ?? "";
}

export function chatItemClass() {
  return "chat-item flex min-h-11 min-w-0 w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs font-medium";
}

export function moveItemId(
  itemIds: string[],
  sourceItemId: string,
  targetItemId: string,
) {
  const sourceIndex = itemIds.indexOf(sourceItemId);
  const targetIndex = itemIds.indexOf(targetItemId);

  if (sourceIndex === -1 || targetIndex === -1 || sourceIndex === targetIndex) {
    return itemIds;
  }

  const next = [...itemIds];
  const [source] = next.splice(sourceIndex, 1);
  next.splice(targetIndex, 0, source);

  return next;
}

export function sameStringList(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function reorderWorkspacesByIds<T extends { id: string }>(
  workspaces: T[],
  workspaceIds: string[],
) {
  if (sameStringList(workspaces.map((workspace) => workspace.id), workspaceIds)) {
    return workspaces;
  }

  const workspacesById = new Map(workspaces.map((workspace) => [workspace.id, workspace]));
  const next = workspaceIds
    .map((workspaceId) => workspacesById.get(workspaceId))
    .filter((workspace): workspace is T => Boolean(workspace));

  return next.length === workspaces.length ? next : workspaces;
}
