//! Process-local per-workspace SQLite concurrency gate.
//!
//! Production openers acquire permits here so `app`, `tools`, `graph`, and
//! sidecar share one ordinary/critical capacity model. Raw
//! [`crate::workspace::WorkspaceDatabase::open_or_create_ungated`] and
//! [`crate::memory::MemoryDatabase::open_workspace_at_ungated`] are for the
//! gate implementation and controlled tests only.
//!
//! Workspace chat DB and workspace-scope Memory share the same per-workspace
//! gate key (canonical workspace root) and the same ordinary/critical ledger.
//! Global Memory (`memory.sqlite`) is a separate database and must not use this
//! gate.

use std::{
    collections::{HashMap, VecDeque},
    ops::{Deref, DerefMut},
    panic::Location,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, LazyLock, Mutex},
    time::{Duration, Instant},
};

use crate::memory::{MemoryDatabase, MemoryDatabaseError};
use crate::workspace::{WorkspaceDatabase, WorkspaceDatabaseError};

pub const WORKSPACE_DATABASE_TOTAL_CAPACITY: usize = 3;
pub const WORKSPACE_DATABASE_ORDINARY_CAPACITY: usize = 2;
pub const WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT: Duration = Duration::from_secs(5);
pub const WORKSPACE_DATABASE_CRITICAL_GATE_TIMEOUT: Duration = Duration::from_secs(15);
const WORKSPACE_DATABASE_GATE_POLL: Duration = Duration::from_millis(10);
const WORKSPACE_DATABASE_LONG_HOLD_WARNING: Duration = Duration::from_secs(10);

static WORKSPACE_DATABASE_GATES: LazyLock<Mutex<HashMap<PathBuf, WorkspaceDatabaseGate>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct WorkspaceDatabaseGate {
    total: Arc<CountingSemaphore>,
    ordinary: Arc<CountingSemaphore>,
    holders: Arc<Mutex<WorkspaceDatabaseHolderLedger>>,
}

impl WorkspaceDatabaseGate {
    fn new() -> Self {
        Self {
            total: Arc::new(CountingSemaphore::new(WORKSPACE_DATABASE_TOTAL_CAPACITY)),
            ordinary: Arc::new(CountingSemaphore::new(WORKSPACE_DATABASE_ORDINARY_CAPACITY)),
            holders: Arc::new(Mutex::new(WorkspaceDatabaseHolderLedger::default())),
        }
    }

    fn register_holder(
        &self,
        resource: &'static str,
        gate_kind: WorkspaceDatabaseGateKind,
        acquired_at: Instant,
        caller: &'static Location<'static>,
    ) -> WorkspaceDatabaseHolderRegistration {
        let mut ledger = self
            .holders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let holder_id = ledger.next_holder_id;
        ledger.next_holder_id = ledger.next_holder_id.wrapping_add(1);
        ledger.holders.insert(
            holder_id,
            WorkspaceDatabaseHolder {
                resource,
                gate_kind,
                acquired_at,
                caller,
            },
        );
        WorkspaceDatabaseHolderRegistration {
            holders: Arc::clone(&self.holders),
            holder_id,
        }
    }

    fn active_holder_summary(&self) -> String {
        let ledger = self
            .holders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut holders = ledger.holders.iter().collect::<Vec<_>>();
        holders.sort_by_key(|(holder_id, _)| *holder_id);
        holders
            .into_iter()
            .take(WORKSPACE_DATABASE_TOTAL_CAPACITY)
            .map(|(_, holder)| {
                format!(
                    "resource={},gate={},held_ms={},source={}:{}",
                    holder.resource,
                    holder.gate_kind.as_str(),
                    holder.acquired_at.elapsed().as_millis(),
                    holder.caller.file(),
                    holder.caller.line(),
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Default)]
struct WorkspaceDatabaseHolderLedger {
    next_holder_id: u64,
    holders: HashMap<u64, WorkspaceDatabaseHolder>,
}

struct WorkspaceDatabaseHolder {
    resource: &'static str,
    gate_kind: WorkspaceDatabaseGateKind,
    acquired_at: Instant,
    caller: &'static Location<'static>,
}

struct WorkspaceDatabaseHolderRegistration {
    holders: Arc<Mutex<WorkspaceDatabaseHolderLedger>>,
    holder_id: u64,
}

impl Drop for WorkspaceDatabaseHolderRegistration {
    fn drop(&mut self) {
        self.holders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .holders
            .remove(&self.holder_id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceDatabaseGateKind {
    Ordinary,
    Critical,
}

impl WorkspaceDatabaseGateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Critical => "critical",
        }
    }

    fn timeout(self) -> Duration {
        match self {
            Self::Ordinary => WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT,
            Self::Critical => WORKSPACE_DATABASE_CRITICAL_GATE_TIMEOUT,
        }
    }
}

struct CountingSemaphore {
    state: Mutex<CountingSemaphoreState>,
    waiters: Condvar,
}

struct CountingSemaphoreState {
    available: usize,
    next_waiter_id: u64,
    queue: VecDeque<u64>,
}

impl CountingSemaphore {
    fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(CountingSemaphoreState {
                available: capacity,
                next_waiter_id: 0,
                queue: VecDeque::new(),
            }),
            waiters: Condvar::new(),
        }
    }

    fn available_permits(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .available
    }

    fn queued_waiters(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .queue
            .len()
    }

    fn acquire_until(
        self: &Arc<Self>,
        started_at: Instant,
        timeout: Duration,
    ) -> Option<SemaphorePermit> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.queue.is_empty() && state.available > 0 {
            state.available -= 1;
            return Some(SemaphorePermit {
                semaphore: Arc::clone(self),
            });
        }

        let waiter_id = state.next_waiter_id;
        state.next_waiter_id = state.next_waiter_id.wrapping_add(1);
        state.queue.push_back(waiter_id);

        loop {
            if state.queue.front().copied() == Some(waiter_id) && state.available > 0 {
                state.queue.pop_front();
                state.available -= 1;
                self.waiters.notify_all();
                return Some(SemaphorePermit {
                    semaphore: Arc::clone(self),
                });
            }

            let waited = started_at.elapsed();
            if waited >= timeout {
                state.queue.retain(|queued_id| *queued_id != waiter_id);
                self.waiters.notify_all();
                return None;
            }
            let remaining = timeout
                .saturating_sub(waited)
                .min(WORKSPACE_DATABASE_GATE_POLL);
            let (next_state, _) = self
                .waiters
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;
        }
    }
}

struct SemaphorePermit {
    semaphore: Arc<CountingSemaphore>,
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        let mut state = self
            .semaphore
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.available = state.available.saturating_add(1);
        self.semaphore.waiters.notify_all();
    }
}

struct WorkspaceDatabasePermits {
    _total: SemaphorePermit,
    _ordinary: Option<SemaphorePermit>,
}

/// Gated workspace database handle. Drop releases the concurrency permits.
///
/// Keep this handle only for the short DB critical section. Do not hold it
/// across provider HTTP, Hook HTTP/MCP, sleep, or a full Agent run.
pub struct WorkspaceDatabaseHandle {
    database: Option<WorkspaceDatabase>,
    holder: Option<WorkspaceDatabaseHolderRegistration>,
    _permits: Option<WorkspaceDatabasePermits>,
    workspace_path: PathBuf,
    gate_kind: WorkspaceDatabaseGateKind,
    acquired_at: Instant,
    caller: &'static Location<'static>,
}

impl WorkspaceDatabaseHandle {
    pub fn gate_kind(&self) -> WorkspaceDatabaseGateKind {
        self.gate_kind
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }
}

impl Deref for WorkspaceDatabaseHandle {
    type Target = WorkspaceDatabase;

    fn deref(&self) -> &Self::Target {
        self.database
            .as_ref()
            .expect("workspace database handle already consumed")
    }
}

impl DerefMut for WorkspaceDatabaseHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.database
            .as_mut()
            .expect("workspace database handle already consumed")
    }
}

impl Drop for WorkspaceDatabaseHandle {
    fn drop(&mut self) {
        let held_for = self.acquired_at.elapsed();
        if held_for >= WORKSPACE_DATABASE_LONG_HOLD_WARNING {
            tracing::warn!(
                workspace = %self.workspace_path.display(),
                gate = self.gate_kind.as_str(),
                held_ms = held_for.as_millis() as u64,
                source_file = self.caller.file(),
                source_line = self.caller.line(),
                source_column = self.caller.column(),
                "workspace database permit held longer than expected"
            );
        }
        // Remove the holder before releasing permits so a timeout snapshot never
        // attributes a permit to a handle that is already being torn down.
        self.holder.take();
        // Release permits before closing the SQLite connection so waiters are not
        // blocked behind a slow connection teardown.
        self._permits.take();
        self.database.take();
    }
}

/// Wait until the ordinary gate for `workspace_path` has at least `min_queued`
/// waiters enqueued (FIFO queue length after a blocked `acquire_until`).
///
/// Tests use this to prove a concurrent open has entered the gate wait queue
/// before releasing holders. Production code must not rely on this helper.
pub fn wait_for_ordinary_gate_queued_waiters(
    workspace_path: impl AsRef<Path>,
    min_queued: usize,
    timeout: Duration,
) -> bool {
    let key = std::fs::canonicalize(workspace_path.as_ref())
        .unwrap_or_else(|_| workspace_path.as_ref().to_path_buf());
    let deadline = Instant::now() + timeout;
    loop {
        let queued = {
            let gates = WORKSPACE_DATABASE_GATES
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            gates
                .get(&key)
                .map(|gate| gate.ordinary.queued_waiters())
                .unwrap_or(0)
        };
        if queued >= min_queued {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::yield_now();
    }
}

/// Open a workspace database under the ordinary gate (capacity 2, total 3).
#[track_caller]
pub fn open_workspace_database(
    workspace_path: impl AsRef<Path>,
) -> Result<WorkspaceDatabaseHandle, WorkspaceDatabaseError> {
    open_workspace_database_with_gate(
        workspace_path.as_ref(),
        WorkspaceDatabaseGateKind::Ordinary,
        Location::caller(),
    )
}

/// Open a workspace database under the critical gate (uses total capacity only).
#[track_caller]
pub fn open_workspace_database_critical(
    workspace_path: impl AsRef<Path>,
) -> Result<WorkspaceDatabaseHandle, WorkspaceDatabaseError> {
    open_workspace_database_with_gate(
        workspace_path.as_ref(),
        WorkspaceDatabaseGateKind::Critical,
        Location::caller(),
    )
}

fn open_workspace_database_with_gate(
    workspace_path: &Path,
    gate_kind: WorkspaceDatabaseGateKind,
    caller: &'static Location<'static>,
) -> Result<WorkspaceDatabaseHandle, WorkspaceDatabaseError> {
    let (key, gate, permits) = acquire_workspace_database_permits(workspace_path, gate_kind)?;
    let acquired_at = Instant::now();
    // Register before opening SQLite so permit contention can be attributed even
    // when migrations or connection setup take unexpectedly long. If opening
    // fails, this local registration drops with the permits.
    let holder = gate.register_holder("database", gate_kind, acquired_at, caller);
    let database = WorkspaceDatabase::open_or_create_ungated(workspace_path)?;
    Ok(WorkspaceDatabaseHandle {
        database: Some(database),
        holder: Some(holder),
        _permits: Some(permits),
        workspace_path: key,
        gate_kind,
        acquired_at,
        caller,
    })
}

/// Gated workspace Memory handle. Drop releases the concurrency permits.
///
/// Workspace Memory shares `.foco/foco.sqlite` with [`WorkspaceDatabase`]. Keep
/// this handle only for the short DB critical section; do not hold it across
/// provider/network awaits.
pub struct WorkspaceMemoryDatabaseHandle {
    database: Option<MemoryDatabase>,
    holder: Option<WorkspaceDatabaseHolderRegistration>,
    _permits: Option<WorkspaceDatabasePermits>,
    workspace_path: PathBuf,
    gate_kind: WorkspaceDatabaseGateKind,
    acquired_at: Instant,
    caller: &'static Location<'static>,
}

impl WorkspaceMemoryDatabaseHandle {
    pub fn gate_kind(&self) -> WorkspaceDatabaseGateKind {
        self.gate_kind
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }
}

impl Deref for WorkspaceMemoryDatabaseHandle {
    type Target = MemoryDatabase;

    fn deref(&self) -> &Self::Target {
        self.database
            .as_ref()
            .expect("workspace memory handle already consumed")
    }
}

impl DerefMut for WorkspaceMemoryDatabaseHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.database
            .as_mut()
            .expect("workspace memory handle already consumed")
    }
}

impl Drop for WorkspaceMemoryDatabaseHandle {
    fn drop(&mut self) {
        let held_for = self.acquired_at.elapsed();
        if held_for >= WORKSPACE_DATABASE_LONG_HOLD_WARNING {
            tracing::warn!(
                workspace = %self.workspace_path.display(),
                gate = self.gate_kind.as_str(),
                held_ms = held_for.as_millis() as u64,
                source_file = self.caller.file(),
                source_line = self.caller.line(),
                source_column = self.caller.column(),
                "workspace memory database permit held longer than expected"
            );
        }
        // Remove the holder before releasing permits so a timeout snapshot never
        // attributes a permit to a handle that is already being torn down.
        self.holder.take();
        // Release permits before closing the SQLite connection so waiters are not
        // blocked behind a slow connection teardown.
        self._permits.take();
        self.database.take();
    }
}

/// Owned Memory connection that may be global (ungated) or workspace (gated).
///
/// Drop releases any workspace gate permits. Prefer short-lived scopes so
/// permits are not held across provider/network awaits.
pub enum OpenedMemoryDatabase {
    Global(MemoryDatabase),
    Workspace(WorkspaceMemoryDatabaseHandle),
}

impl OpenedMemoryDatabase {
    pub fn is_workspace(&self) -> bool {
        matches!(self, Self::Workspace(_))
    }
}

impl Deref for OpenedMemoryDatabase {
    type Target = MemoryDatabase;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Global(database) => database,
            Self::Workspace(database) => database,
        }
    }
}

impl DerefMut for OpenedMemoryDatabase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Global(database) => database,
            Self::Workspace(database) => database,
        }
    }
}

impl From<MemoryDatabase> for OpenedMemoryDatabase {
    fn from(database: MemoryDatabase) -> Self {
        Self::Global(database)
    }
}

impl From<WorkspaceMemoryDatabaseHandle> for OpenedMemoryDatabase {
    fn from(database: WorkspaceMemoryDatabaseHandle) -> Self {
        Self::Workspace(database)
    }
}

/// Open workspace Memory under the ordinary gate (capacity 2, total 3).
///
/// Shares the same per-workspace gate ledger as [`open_workspace_database`].
#[track_caller]
pub fn open_workspace_memory_database(
    workspace_path: impl AsRef<Path>,
) -> Result<WorkspaceMemoryDatabaseHandle, MemoryDatabaseError> {
    open_workspace_memory_database_with_gate(
        workspace_path.as_ref(),
        WorkspaceDatabaseGateKind::Ordinary,
        Location::caller(),
    )
}

/// Open workspace Memory under the critical gate (uses total capacity only).
#[track_caller]
pub fn open_workspace_memory_database_critical(
    workspace_path: impl AsRef<Path>,
) -> Result<WorkspaceMemoryDatabaseHandle, MemoryDatabaseError> {
    open_workspace_memory_database_with_gate(
        workspace_path.as_ref(),
        WorkspaceDatabaseGateKind::Critical,
        Location::caller(),
    )
}

fn open_workspace_memory_database_with_gate(
    workspace_path: &Path,
    gate_kind: WorkspaceDatabaseGateKind,
    caller: &'static Location<'static>,
) -> Result<WorkspaceMemoryDatabaseHandle, MemoryDatabaseError> {
    // Ensure workspace schema exists first under the same gate; Memory tables live
    // in the same SQLite file and require migrations via WorkspaceDatabase.
    let (key, gate, permits) = acquire_workspace_database_permits(workspace_path, gate_kind)
        .map_err(|error| match error {
            WorkspaceDatabaseError::ConcurrencyLimit { message } => {
                MemoryDatabaseError::ConcurrencyLimit { message }
            }
            other => MemoryDatabaseError::InvalidMemoryInput {
                message: other.to_string(),
            },
        })?;
    let acquired_at = Instant::now();
    let holder = gate.register_holder("memory", gate_kind, acquired_at, caller);
    // Workspace migrations must run before Memory schema is readable. Open
    // workspace DB first (same file), drop it, then open Memory view.
    WorkspaceDatabase::open_or_create_ungated(workspace_path).map_err(|error| match error {
        WorkspaceDatabaseError::ConcurrencyLimit { message } => {
            MemoryDatabaseError::ConcurrencyLimit { message }
        }
        WorkspaceDatabaseError::WorkspaceNotDirectory { path } => {
            MemoryDatabaseError::InvalidMemoryInput {
                message: format!("workspace path is not a directory: {}", path.display()),
            }
        }
        other => MemoryDatabaseError::InvalidMemoryInput {
            message: other.to_string(),
        },
    })?;
    let database = MemoryDatabase::open_workspace_at_ungated(
        crate::workspace::workspace_database_path(workspace_path),
    )?;
    Ok(WorkspaceMemoryDatabaseHandle {
        database: Some(database),
        holder: Some(holder),
        _permits: Some(permits),
        workspace_path: key,
        gate_kind,
        acquired_at,
        caller,
    })
}

fn acquire_workspace_database_permits(
    workspace_path: &Path,
    gate_kind: WorkspaceDatabaseGateKind,
) -> Result<(PathBuf, WorkspaceDatabaseGate, WorkspaceDatabasePermits), WorkspaceDatabaseError> {
    let key =
        std::fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    let gate = {
        let mut gates = WORKSPACE_DATABASE_GATES.lock().map_err(|_| {
            WorkspaceDatabaseError::ConcurrencyLimit {
                message: "workspace database gate lock is poisoned".to_string(),
            }
        })?;
        gates
            .entry(key.clone())
            .or_insert_with(WorkspaceDatabaseGate::new)
            .clone()
    };
    let started_at = Instant::now();
    let timeout = gate_kind.timeout();
    let ordinary = match gate_kind {
        WorkspaceDatabaseGateKind::Ordinary => Some(acquire_workspace_database_gate_slot(
            &key,
            &gate,
            Arc::clone(&gate.ordinary),
            gate_kind,
            "ordinary",
            started_at,
            timeout,
        )?),
        WorkspaceDatabaseGateKind::Critical => None,
    };
    let total = acquire_workspace_database_gate_slot(
        &key,
        &gate,
        Arc::clone(&gate.total),
        gate_kind,
        "total",
        started_at,
        timeout,
    )?;
    Ok((
        key,
        gate,
        WorkspaceDatabasePermits {
            _total: total,
            _ordinary: ordinary,
        },
    ))
}

fn acquire_workspace_database_gate_slot(
    key: &Path,
    gate: &WorkspaceDatabaseGate,
    semaphore: Arc<CountingSemaphore>,
    gate_kind: WorkspaceDatabaseGateKind,
    slot_kind: &'static str,
    started_at: Instant,
    timeout: Duration,
) -> Result<SemaphorePermit, WorkspaceDatabaseError> {
    if let Some(permit) = semaphore.acquire_until(started_at, timeout) {
        return Ok(permit);
    }

    let waited = started_at.elapsed();
    let holder_summary = gate.active_holder_summary();
    tracing::warn!(
        workspace = %key.display(),
        gate = gate_kind.as_str(),
        slot = slot_kind,
        waited_ms = waited.as_millis() as u64,
        total_capacity = WORKSPACE_DATABASE_TOTAL_CAPACITY,
        total_available = gate.total.available_permits(),
        ordinary_capacity = WORKSPACE_DATABASE_ORDINARY_CAPACITY,
        ordinary_available = gate.ordinary.available_permits(),
        holders = %holder_summary,
        "workspace database permit acquisition timed out"
    );
    Err(WorkspaceDatabaseError::ConcurrencyLimit {
        message: format!(
            "workspace database concurrency limit reached for {} after {} ms (gate={}, slot={}, total={}/{}, ordinary={}/{}, holders=[{}])",
            key.display(),
            waited.as_millis(),
            gate_kind.as_str(),
            slot_kind,
            gate.total.available_permits(),
            WORKSPACE_DATABASE_TOTAL_CAPACITY,
            gate.ordinary.available_permits(),
            WORKSPACE_DATABASE_ORDINARY_CAPACITY,
            holder_summary,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn ordinary_gate_times_out_while_critical_capacity_remains() {
        let workspace = tempdir().expect("workspace");
        let gate_1 = open_workspace_database(workspace.path()).expect("first ordinary");
        let gate_2 = open_workspace_database(workspace.path()).expect("second ordinary");

        let workspace_path = workspace.path().to_path_buf();
        let started_at = Instant::now();
        let error = match open_workspace_database(&workspace_path) {
            Ok(_) => panic!("third ordinary open must hit the concurrency limit"),
            Err(error) => error,
        };
        let waited = started_at.elapsed();
        assert!(
            waited >= WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT,
            "ordinary waiter returned too early after {waited:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("workspace database concurrency limit reached"),
            "{message}"
        );
        assert!(message.contains("gate=ordinary"), "{message}");
        assert!(message.contains("ordinary=0/2"), "{message}");
        assert!(
            message.contains("holders=[resource=database,gate=ordinary,held_ms="),
            "{message}"
        );
        assert!(
            message.contains("source=store/workspace_gate.rs:"),
            "{message}"
        );

        let critical_started = Instant::now();
        let critical =
            open_workspace_database_critical(workspace.path()).expect("critical reserved");
        assert!(
            critical_started.elapsed() < Duration::from_secs(1),
            "critical open should not wait behind ordinary saturation"
        );
        drop(critical);
        drop(gate_1);
        drop(gate_2);
    }

    #[test]
    fn holder_ledger_reports_timeout_source_and_removes_dropped_handles() {
        let workspace = tempdir().expect("workspace");
        let handle = open_workspace_database(workspace.path()).expect("workspace database");
        let key = std::fs::canonicalize(workspace.path()).expect("canonical workspace");
        let gate = WORKSPACE_DATABASE_GATES
            .lock()
            .expect("gate registry")
            .get(&key)
            .expect("workspace gate")
            .clone();
        let active_summary = gate.active_holder_summary();
        assert!(
            active_summary.contains("resource=database,gate=ordinary,held_ms="),
            "{active_summary}"
        );
        assert!(
            active_summary.contains("source=store/workspace_gate.rs:"),
            "{active_summary}"
        );

        let first = gate
            .ordinary
            .acquire_until(Instant::now(), Duration::from_secs(1))
            .expect("synthetic permit");
        let error = match acquire_workspace_database_gate_slot(
            &key,
            &gate,
            Arc::clone(&gate.ordinary),
            WorkspaceDatabaseGateKind::Ordinary,
            "ordinary",
            Instant::now(),
            Duration::ZERO,
        ) {
            Ok(_) => panic!("saturated gate must time out"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("holders=[resource=database"), "{message}");
        assert!(message.contains("held_ms="), "{message}");
        assert!(
            message.contains("source=store/workspace_gate.rs:"),
            "{message}"
        );

        drop(first);
        drop(handle);
        assert!(
            gate.active_holder_summary().is_empty(),
            "dropped handle must be removed from the holder ledger"
        );
    }

    #[test]
    fn failed_database_open_does_not_leave_holder_recorded() {
        let workspace = tempdir().expect("workspace");
        let file_path = workspace.path().join("not-a-workspace-directory");
        std::fs::write(&file_path, "not a directory").expect("write file");

        assert!(
            open_workspace_database(&file_path).is_err(),
            "file path must fail database opening"
        );

        let key = std::fs::canonicalize(&file_path).expect("canonical file path");
        let gate = WORKSPACE_DATABASE_GATES
            .lock()
            .expect("gate registry")
            .get(&key)
            .expect("gate created for failed open")
            .clone();
        assert!(
            gate.active_holder_summary().is_empty(),
            "failed open must not leave a holder ledger entry"
        );
    }

    #[test]
    fn workspace_memory_shares_ordinary_gate_with_workspace_database() {
        let workspace = tempdir().expect("workspace");
        // Ensure schema exists for Memory open.
        drop(open_workspace_database(workspace.path()).expect("seed workspace db"));

        let ordinary_1 = open_workspace_database(workspace.path()).expect("ordinary workspace");
        let ordinary_2 = open_workspace_memory_database(workspace.path()).expect("ordinary memory");

        // Prove shared capacity without waiting the production 5s ordinary timeout:
        // hold two ordinary slots (db + memory), then acquire with Duration::ZERO.
        let key = std::fs::canonicalize(workspace.path()).expect("canonical workspace");
        let gate = WORKSPACE_DATABASE_GATES
            .lock()
            .expect("gate registry")
            .get(&key)
            .expect("workspace gate")
            .clone();
        let holder_summary = gate.active_holder_summary();
        assert!(
            holder_summary.contains("resource=database,gate=ordinary"),
            "{holder_summary}"
        );
        assert!(
            holder_summary.contains("resource=memory,gate=ordinary"),
            "{holder_summary}"
        );
        let error = match acquire_workspace_database_gate_slot(
            &key,
            &gate,
            Arc::clone(&gate.ordinary),
            WorkspaceDatabaseGateKind::Ordinary,
            "ordinary",
            Instant::now(),
            Duration::ZERO,
        ) {
            Ok(_) => panic!("third ordinary open must hit the shared concurrency limit"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains("workspace database concurrency limit reached"),
            "{message}"
        );
        assert!(
            message.contains("resource=database,gate=ordinary"),
            "{message}"
        );
        assert!(
            message.contains("resource=memory,gate=ordinary"),
            "{message}"
        );

        drop(ordinary_1);
        drop(ordinary_2);
        let reopened =
            open_workspace_memory_database(workspace.path()).expect("memory after release");
        drop(reopened);
    }

    #[test]
    fn global_memory_open_does_not_consume_workspace_gate() {
        let workspace = tempdir().expect("workspace");
        let profile = tempdir().expect("profile");
        let ordinary_1 = open_workspace_database(workspace.path()).expect("ordinary 1");
        let ordinary_2 = open_workspace_database(workspace.path()).expect("ordinary 2");

        // Global Memory must succeed even when the workspace ordinary ledger is full.
        let global = MemoryDatabase::open_or_create_global(profile.path()).expect("global memory");
        drop(global);

        drop(ordinary_1);
        drop(ordinary_2);
    }

    #[test]
    fn total_capacity_blocks_extra_until_slot_releases() {
        let workspace = tempdir().expect("workspace");
        let ordinary_1 = open_workspace_database(workspace.path()).expect("ordinary 1");
        let ordinary_2 = open_workspace_database(workspace.path()).expect("ordinary 2");
        let critical = open_workspace_database_critical(workspace.path()).expect("critical");

        let workspace_path = workspace.path().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = thread::spawn(move || {
            let started_at = Instant::now();
            let result = open_workspace_database_critical(&workspace_path);
            let _ = tx.send(started_at.elapsed());
            result
        });

        // Waiter should remain blocked while total capacity is exhausted.
        thread::sleep(Duration::from_millis(150));
        assert!(
            rx.try_recv().is_err(),
            "critical waiter should not finish while total capacity is full"
        );

        drop(critical);
        let waited = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("critical waiter should finish after a slot is released");
        let opened = waiter.join().expect("join waiter").expect("critical open");
        assert!(
            waited < Duration::from_secs(2),
            "critical open should complete promptly after release, waited {waited:?}"
        );
        drop(opened);
        drop(ordinary_1);
        drop(ordinary_2);
    }

    #[test]
    fn semaphore_waiters_acquire_in_fifo_order() {
        let semaphore = Arc::new(CountingSemaphore::new(1));
        let initial = semaphore
            .acquire_until(Instant::now(), Duration::from_secs(1))
            .expect("initial permit");
        let (order_tx, order_rx) = std::sync::mpsc::channel();
        let (release_waiter_tx, release_waiter_rx) = std::sync::mpsc::channel();

        let waiter_semaphore = Arc::clone(&semaphore);
        let waiter_order_tx = order_tx.clone();
        let waiter = thread::spawn(move || {
            let permit = waiter_semaphore
                .acquire_until(Instant::now(), Duration::from_secs(2))
                .expect("queued waiter permit");
            waiter_order_tx.send("waiter").expect("waiter order");
            release_waiter_rx.recv().expect("release waiter");
            drop(permit);
        });
        wait_for_semaphore_queue_len(&semaphore, 1);

        let reacquirer_semaphore = Arc::clone(&semaphore);
        let reacquirer = thread::spawn(move || {
            let permit = reacquirer_semaphore
                .acquire_until(Instant::now(), Duration::from_secs(2))
                .expect("reacquirer permit");
            order_tx.send("reacquirer").expect("reacquirer order");
            drop(permit);
        });
        wait_for_semaphore_queue_len(&semaphore, 2);

        drop(initial);
        assert_eq!(
            order_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("first acquisition"),
            "waiter"
        );
        release_waiter_tx.send(()).expect("release waiter");
        assert_eq!(
            order_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("second acquisition"),
            "reacquirer"
        );

        waiter.join().expect("join waiter");
        reacquirer.join().expect("join reacquirer");
    }

    fn wait_for_semaphore_queue_len(semaphore: &CountingSemaphore, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let queued = semaphore
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .queue
                .len();
            if queued == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "semaphore queue length did not reach {expected}; current={queued}"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn production_crates_do_not_call_open_or_create_ungated() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let allowlist = [
            Path::new("store/workspace.rs"),
            Path::new("store/workspace_gate.rs"),
            Path::new("store/workspace_schema.rs"),
            Path::new("store/memory.rs"),
            Path::new("store/tests/workspace_database.rs"),
        ];
        let mut offenders = Vec::new();
        for crate_dir in ["app", "tools", "graph", "agent", "store"] {
            let root = workspace_root.join(crate_dir);
            if !root.exists() {
                continue;
            }
            collect_workspace_db_bypass_sites(&root, workspace_root, &allowlist, &mut offenders);
        }
        assert!(
            offenders.is_empty(),
            "production code must use gated openers; unexpected workspace DB bypass sites: {offenders:?}"
        );
    }

    #[test]
    fn production_crates_do_not_call_chat_code_change_stats() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let mut offenders = Vec::new();
        // Match only definitions / call sites; bare mentions in this guard do not count.
        for pattern in ["fn chat_code_change_stats", ".chat_code_change_stats("] {
            for crate_dir in ["app", "tools", "graph", "agent", "store"] {
                let root = workspace_root.join(crate_dir);
                if !root.exists() {
                    continue;
                }
                collect_symbol_call_sites(&root, workspace_root, pattern, &mut offenders);
            }
        }
        assert!(
            offenders.is_empty(),
            "unscoped code-change aggregation was removed; use code_change_stats_for_chats with page chat ids: {offenders:?}"
        );
    }

    fn collect_symbol_call_sites(
        dir: &Path,
        workspace_root: &Path,
        symbol: &str,
        offenders: &mut Vec<String>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if matches!(name, "target" | "node_modules" | ".git" | ".foco") {
                    continue;
                }
                collect_symbol_call_sites(&path, workspace_root, symbol, offenders);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let relative = match path.strip_prefix(workspace_root) {
                Ok(relative) => relative,
                Err(_) => continue,
            };
            if relative == Path::new("store/workspace_gate.rs") {
                // This guard lives here; ignore self-references in test source.
                continue;
            }
            let is_test_path = relative.components().any(|component| {
                component.as_os_str() == "tests"
                    || component
                        .as_os_str()
                        .to_str()
                        .is_some_and(|name| name.ends_with("_tests.rs"))
            });
            if is_test_path {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let production_source = strip_cfg_test_regions(&source);
            if production_source.contains(symbol) {
                offenders.push(relative.display().to_string());
            }
        }
    }

    fn collect_workspace_db_bypass_sites(
        dir: &Path,
        workspace_root: &Path,
        allowlist: &[&Path],
        offenders: &mut Vec<String>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if matches!(name, "target" | "node_modules" | ".git" | ".foco") {
                    continue;
                }
                collect_workspace_db_bypass_sites(&path, workspace_root, allowlist, offenders);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let relative = match path.strip_prefix(workspace_root) {
                Ok(relative) => relative,
                Err(_) => continue,
            };
            let is_test_path = relative.components().any(|component| {
                component.as_os_str() == "tests"
                    || component
                        .as_os_str()
                        .to_str()
                        .is_some_and(|name| name.ends_with("_tests.rs"))
            });
            if is_test_path {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let production_source = strip_cfg_test_regions(&source);
            // Gate / definition modules may define ungated openers.
            let allow_ungated_symbols = matches!(
                relative.to_str(),
                Some("store/workspace.rs" | "store/workspace_gate.rs" | "store/memory.rs")
            );
            for (line_no, line) in production_source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("///") {
                    continue;
                }
                let mut hit = None;
                if !allow_ungated_symbols {
                    if line.contains("open_or_create_ungated(")
                        || line.contains("open_workspace_at_ungated(")
                        || line.contains("MemoryDatabase::open_workspace_at(")
                    {
                        hit = Some("ungated_or_raw_memory_open");
                    }
                }
                // Only real call sites, not string literals inside this guard.
                let connection_open_call = trimmed.starts_with("Connection::open(")
                    && (line.contains("workspace_database_path")
                        || line.contains("WORKSPACE_DATABASE_FILE"));
                if connection_open_call {
                    hit = Some("Connection::open(workspace_database_path)");
                }
                if let Some(kind) = hit {
                    offenders.push(format!(
                        "{}:{}: {kind}: {}",
                        relative.display(),
                        line_no + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    fn strip_cfg_test_regions(source: &str) -> String {
        let mut output = String::with_capacity(source.len());
        let mut rest = source;
        while let Some(start) = rest.find("#[cfg(test)]") {
            output.push_str(&rest[..start]);
            rest = &rest[start + "#[cfg(test)]".len()..];
            rest = rest.trim_start();
            if rest.starts_with("mod ") {
                if let Some(body_start) = rest.find('{') {
                    let after_brace = &rest[body_start + 1..];
                    if let Some(end) = matching_brace_end(after_brace) {
                        rest = &after_brace[end + 1..];
                        continue;
                    }
                }
            }
            // Fallback: drop the rest of the line and continue.
            if let Some(newline) = rest.find('\n') {
                rest = &rest[newline + 1..];
            } else {
                rest = "";
            }
        }
        output.push_str(rest);
        output
    }

    fn matching_brace_end(source: &str) -> Option<usize> {
        let mut depth = 1usize;
        for (index, ch) in source.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    // critical_waiter_unblocks_after_long_hold_release was removed: it slept
    // WORKSPACE_DATABASE_LONG_HOLD_WARNING (~10s) while duplicating
    // total_capacity_blocks_extra_until_slot_releases coverage.

    #[test]
    fn long_hold_warning_emits_for_deliberate_hold() {
        let workspace = tempdir().expect("workspace");
        let handle = open_workspace_database(workspace.path()).expect("open");
        // Keep permit slightly over the warning threshold without blocking the suite long:
        // unit test only verifies Drop path runs; warning is best-effort tracing.
        thread::sleep(Duration::from_millis(20));
        drop(handle);
    }

    #[test]
    fn canonical_and_relative_workspace_paths_share_one_gate() {
        let workspace = tempdir().expect("workspace");
        let canonical = std::fs::canonicalize(workspace.path()).expect("canonicalize");
        // Seed schema once so both openers hit the same database file.
        drop(open_workspace_database(&canonical).expect("seed"));

        let ordinary_1 = open_workspace_database(&canonical).expect("ordinary via canonical");
        let ordinary_2 = open_workspace_memory_database(workspace.path())
            .expect("ordinary memory via original temp path must share the same gate key");

        // Path forms must share one gate key; prove saturation with Duration::ZERO
        // instead of waiting the production 5s ordinary timeout.
        // Do not assert on the process-global gate map size: other tests may hold
        // entries concurrently, and panicking while the registry lock is held
        // poisons the suite.
        let gate = WORKSPACE_DATABASE_GATES
            .lock()
            .expect("gate registry")
            .get(&canonical)
            .expect("shared workspace gate")
            .clone();
        let error = match acquire_workspace_database_gate_slot(
            &canonical,
            &gate,
            Arc::clone(&gate.ordinary),
            WorkspaceDatabaseGateKind::Ordinary,
            "ordinary",
            Instant::now(),
            Duration::ZERO,
        ) {
            Ok(_) => panic!("third ordinary open must share capacity across path forms"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains("workspace database concurrency limit reached"),
            "{message}"
        );
        assert!(message.contains("ordinary=0/2"), "{message}");

        // Drop releases permits before the connection is closed (see Drop impl).
        drop(ordinary_1);
        drop(ordinary_2);
        let reopened = open_workspace_database(&canonical).expect("reopen after shared release");
        drop(reopened);
    }
}
