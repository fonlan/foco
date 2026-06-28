use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

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
    owner: ToolResourceLockOwner,
    acquired_at: Instant,
    wait_ms: u128,
}

pub(crate) struct ToolResourceLease {
    registry: ToolResourceLockRegistry,
    lease_id: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ToolResourceLockOwner {
    pub(crate) instance_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) tool_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolResourceLockOwnerSnapshot {
    pub(crate) owner: ToolResourceLockOwner,
    pub(crate) active_ms: u128,
    pub(crate) wait_ms: u128,
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
    #[cfg(test)]
    pub(crate) async fn acquire(
        &self,
        workspace_id: &str,
        locks: Vec<ToolResourceLock>,
    ) -> ToolResourceLease {
        self.acquire_with_owner(workspace_id, locks, ToolResourceLockOwner::default())
            .await
    }

    pub(crate) async fn acquire_with_owner(
        &self,
        workspace_id: &str,
        locks: Vec<ToolResourceLock>,
        owner: ToolResourceLockOwner,
    ) -> ToolResourceLease {
        let locks = normalize_tool_resource_locks(locks);
        let workspace_id = workspace_id.to_string();
        let lease_id = self.inner.next_lease_id.fetch_add(1, Ordering::Relaxed);
        let wait_started = Instant::now();

        loop {
            let notified = {
                let mut active = self
                    .inner
                    .active
                    .lock()
                    .expect("tool resource lock registry mutex poisoned");
                if !tool_locks_conflict_with_active(&workspace_id, &locks, &active) {
                    let acquired_at = Instant::now();
                    let wait_ms = acquired_at.duration_since(wait_started).as_millis();
                    active.extend(locks.iter().cloned().map(|lock| ActiveToolResourceLock {
                        lease_id,
                        workspace_id: workspace_id.clone(),
                        lock,
                        owner: owner.clone(),
                        acquired_at,
                        wait_ms,
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

    pub(crate) fn blocking_owners(
        &self,
        workspace_id: &str,
        locks: &[ToolResourceLock],
    ) -> Vec<ToolResourceLockOwnerSnapshot> {
        let locks = normalize_tool_resource_locks(locks.to_vec());
        let active = self
            .inner
            .active
            .lock()
            .expect("tool resource lock registry mutex poisoned");
        let now = Instant::now();
        let mut seen = Vec::new();
        let mut owners = Vec::new();

        for active_lock in active.iter() {
            if seen.contains(&active_lock.lease_id) {
                continue;
            }

            if locks.iter().any(|pending_lock| {
                tool_locks_share_scope(workspace_id, pending_lock, active_lock)
                    && tool_resource_locks_conflict(pending_lock, &active_lock.lock)
            }) {
                seen.push(active_lock.lease_id);
                owners.push(ToolResourceLockOwnerSnapshot {
                    owner: active_lock.owner.clone(),
                    active_ms: now.duration_since(active_lock.acquired_at).as_millis(),
                    wait_ms: active_lock.wait_ms,
                });
            }
        }

        owners
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
        ToolResource::WorkspaceMutationLease => "workspace-mutation-lease".to_string(),
        ToolResource::WorkspaceFiles => "workspace-files".to_string(),
        ToolResource::File(path) => format!("file:{path}"),
        ToolResource::TodoGraph => "todo-graph".to_string(),
        ToolResource::Plan => "plan".to_string(),
        ToolResource::ProjectSpec => "project-spec".to_string(),
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
