use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};

use foco_agent::{
    ToolResource, ToolResourceAccess, ToolResourceLock, tool_resource_locks_conflict,
};
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub(crate) struct ToolResourceLockRegistry {
    inner: Arc<ToolResourceLockRegistryInner>,
}

struct ToolResourceLockRegistryInner {
    active: Mutex<Vec<ActiveToolResourceLock>>,
    next_lease_id: AtomicU64,
    released: Notify,
}

#[derive(Clone)]
struct ActiveToolResourceLock {
    lease_id: u64,
    pub(crate) workspace_id: String,
    lock: ToolResourceLock,
}

pub(crate) struct ToolResourceLease {
    registry: ToolResourceLockRegistry,
    lease_id: u64,
}

impl Default for ToolResourceLockRegistryInner {
    fn default() -> Self {
        Self {
            active: Mutex::new(Vec::new()),
            next_lease_id: AtomicU64::new(1),
            released: Notify::new(),
        }
    }
}

impl ToolResourceLockRegistry {
    pub(crate) async fn acquire(&self, workspace_id: &str, locks: Vec<ToolResourceLock>) -> ToolResourceLease {
        let locks = normalize_tool_resource_locks(locks);
        let workspace_id = workspace_id.to_string();
        let lease_id = self.inner.next_lease_id.fetch_add(1, Ordering::Relaxed);

        loop {
            let notified = {
                let mut active = self
                    .inner
                    .active
                    .lock()
                    .expect("tool resource lock registry mutex poisoned");
                if !tool_locks_conflict_with_active(&workspace_id, &locks, &active) {
                    active.extend(locks.iter().cloned().map(|lock| ActiveToolResourceLock {
                        lease_id,
                        workspace_id: workspace_id.clone(),
                        lock,
                    }));
                    return ToolResourceLease {
                        registry: self.clone(),
                        lease_id,
                    };
                }

                self.inner.released.notified()
            };

            notified.await;
        }
    }
}

impl Drop for ToolResourceLease {
    fn drop(&mut self) {
        let released = {
            let mut active = self
                .registry
                .inner
                .active
                .lock()
                .expect("tool resource lock registry mutex poisoned");
            let before = active.len();
            active.retain(|lock| lock.lease_id != self.lease_id);
            active.len() != before
        };

        if released {
            self.registry.inner.released.notify_waiters();
        }
    }
}

fn normalize_tool_resource_locks(locks: Vec<ToolResourceLock>) -> Vec<ToolResourceLock> {
    let mut normalized: Vec<ToolResourceLock> = Vec::new();
    for lock in locks {
        if let Some(existing) = normalized
            .iter_mut()
            .find(|existing| existing.resource == lock.resource)
        {
            existing.access = strongest_tool_resource_access(existing.access, lock.access);
        } else {
            normalized.push(lock);
        }
    }

    normalized.sort_by(|first, second| {
        tool_resource_sort_key(&first.resource)
            .cmp(&tool_resource_sort_key(&second.resource))
            .then(
                tool_resource_access_rank(first.access)
                    .cmp(&tool_resource_access_rank(second.access)),
            )
    });
    normalized
}

fn strongest_tool_resource_access(
    first: ToolResourceAccess,
    second: ToolResourceAccess,
) -> ToolResourceAccess {
    if matches!(first, ToolResourceAccess::Exclusive)
        || matches!(second, ToolResourceAccess::Exclusive)
    {
        ToolResourceAccess::Exclusive
    } else if matches!(first, ToolResourceAccess::Write)
        || matches!(second, ToolResourceAccess::Write)
    {
        ToolResourceAccess::Write
    } else {
        ToolResourceAccess::Read
    }
}

fn tool_locks_conflict_with_active(
    workspace_id: &str,
    pending: &[ToolResourceLock],
    active: &[ActiveToolResourceLock],
) -> bool {
    pending.iter().any(|pending_lock| {
        active.iter().any(|active_lock| {
            tool_locks_share_scope(workspace_id, pending_lock, active_lock)
                && tool_resource_locks_conflict(pending_lock, &active_lock.lock)
        })
    })
}

fn tool_locks_share_scope(
    workspace_id: &str,
    pending: &ToolResourceLock,
    active: &ActiveToolResourceLock,
) -> bool {
    matches!(
        (&pending.resource, &active.lock.resource),
        (ToolResource::Memory(_), ToolResource::Memory(_))
    ) || active.workspace_id == workspace_id
}

fn tool_resource_sort_key(resource: &ToolResource) -> String {
    match resource {
        ToolResource::WorkspaceFiles => "workspace-files".to_string(),
        ToolResource::File(path) => format!("file:{path}"),
        ToolResource::TodoGraph => "todo-graph".to_string(),
        ToolResource::Memory(scope) => format!("memory:{scope}"),
        ToolResource::ExternalTool(tool_name) => format!("external:{tool_name}"),
    }
}

fn tool_resource_access_rank(access: ToolResourceAccess) -> u8 {
    match access {
        ToolResourceAccess::Read => 0,
        ToolResourceAccess::Write => 1,
        ToolResourceAccess::Exclusive => 2,
    }
}
