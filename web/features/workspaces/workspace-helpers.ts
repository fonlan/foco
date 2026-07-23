export function workspaceItemClass(active: boolean) {
  return `workspace-item flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg px-2 text-sm font-semibold ${
    active ? "workspace-item-active text-[var(--accent-soft-foreground)]" : "text-[var(--muted)]"
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
      ? "foco-reticle-on workspace-menu-active border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent-soft-foreground)] shadow-sm"
      : "border-transparent bg-[color-mix(in_oklab,var(--surface-secondary)_60%,transparent)] text-[var(--muted)] hover:border-[var(--border)] hover:bg-[color-mix(in_oklab,var(--surface)_90%,transparent)] hover:text-[var(--foreground)]"
  }`;
}

export function chatItemClass(active: boolean) {
  return `chat-item flex min-h-11 min-w-0 w-full items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-xs font-medium ${
    active
      ? "chat-item-active border-[var(--accent)] bg-[var(--surface)] text-[var(--foreground)] shadow-sm"
      : "border-transparent text-[var(--muted)] hover:border-[var(--border)] hover:bg-[color-mix(in_oklab,var(--surface)_80%,transparent)] hover:text-[var(--foreground)]"
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
