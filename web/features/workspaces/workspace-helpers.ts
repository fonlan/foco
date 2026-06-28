export function workspaceItemClass(active: boolean) {
  return `workspace-item flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg px-2 text-sm font-semibold ${
    active ? "workspace-item-active text-teal-950" : "text-stone-700"
  }`;
}

export function workspaceNameFromPath(path: string) {
  const trimmedPath = path.trim().replace(/[\\/]+$/g, "");
  const parts = trimmedPath.split(/[\\/]+/);

  return parts.at(-1) ?? "";
}

export function workspaceMenuClass(active: boolean) {
  return `workspace-menu foco-reticle flex min-w-0 items-center gap-1 rounded-xl border px-1.5 py-1 transition-colors ${
    active
      ? "foco-reticle-on workspace-menu-active border-teal-200 bg-teal-50 text-teal-950 shadow-sm"
      : "border-transparent bg-stone-100/60 text-stone-700 hover:border-stone-200 hover:bg-white/90 hover:text-stone-950"
  }`;
}

export function chatItemClass(active: boolean) {
  return `chat-item flex min-h-11 min-w-0 w-full items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-xs font-medium ${
    active
      ? "chat-item-active border-teal-100 bg-white text-stone-950 shadow-sm"
      : "border-transparent text-stone-600 hover:border-stone-200 hover:bg-white/80 hover:text-stone-950"
  }`;
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
