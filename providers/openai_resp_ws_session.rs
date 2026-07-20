//! Run-scoped OpenAI Responses WebSocket session registry.
//!
//! Affinity key: workspace + run identity + provider + model.
//! Connection identity (base URL, API key hash, overrides, …) is checked before socket reuse.
//! One connection per key; serial `response.create`; optional previous_response_id continuation
//! only when committed message prefix content still matches.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::stream::{SplitSink, SplitStream};
use genai::chat::{ChatMessage, ChatOptions, ChatRequest};
use tokio::sync::{Mutex, Notify, OwnedMutexGuard};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

/// Official OpenAI Responses WebSocket sessions last up to 60 minutes; rotate early.
pub const OPENAI_RESP_WS_MAX_CONNECTION_AGE: Duration = Duration::from_secs(55 * 60);
/// Drop idle connections to free sockets and avoid cross-run leakage after cancel gaps.
pub const OPENAI_RESP_WS_IDLE_RECLAIM: Duration = Duration::from_secs(10 * 60);
const DEFAULT_MAX_SESSIONS: usize = 64;

pub type OpenAiRespWsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
pub type OpenAiRespWsWrite = SplitSink<OpenAiRespWsStream, Message>;
pub type OpenAiRespWsRead = SplitStream<OpenAiRespWsStream>;

/// Session affinity for Provider WebSocket reuse (never broker RPC request id).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OpenAiRespWsSessionKey {
    pub workspace_id: String,
    /// Local: assistant_message_id (stable across the agent tool loop).
    /// SSH broker: remote chat `runId` from sidecar payload.
    pub run_affinity_id: String,
    pub provider_id: String,
    pub model_id: String,
}

impl OpenAiRespWsSessionKey {
    pub fn new(
        workspace_id: impl Into<String>,
        run_affinity_id: impl Into<String>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            run_affinity_id: run_affinity_id.into(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }
}

/// Optional context passed into WebSocket streaming when run affinity is available.
#[derive(Clone)]
pub struct ProviderWsSessionContext {
    pub registry: Arc<OpenAiRespWsSessionRegistry>,
    pub key: OpenAiRespWsSessionKey,
    /// When true, store responses and continue with previous_response_id when safe.
    /// Chat completion = true; one-shot internal kinds should omit the whole context.
    pub enable_continuation: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ContinuationState {
    /// Hash of connection identity + routing + tools + system + options (not message bodies).
    pub fingerprint: u64,
    /// Content hash of messages[0..committed_messages_len] at commit time.
    pub committed_prefix_hash: u64,
    /// `ChatRequest.messages` length committed after the last successful response.
    pub committed_messages_len: usize,
    pub previous_response_id: String,
}

#[derive(Debug)]
pub(crate) struct LiveWsConnection {
    pub write: OpenAiRespWsWrite,
    pub read: OpenAiRespWsRead,
    pub connected_at: Instant,
    /// Identity of the Provider config used when this socket was opened.
    pub connection_identity: u64,
    /// Real HTTP upgrade status observed at connect time (connection-level).
    pub handshake_status: u16,
}

impl LiveWsConnection {
    pub fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.connected_at) >= OPENAI_RESP_WS_MAX_CONNECTION_AGE
    }
}

struct SessionInner {
    connection: Option<LiveWsConnection>,
    continuation: Option<ContinuationState>,
    last_used_at: Instant,
    /// True while a turn holds the connection out of the pool.
    turn_in_flight: bool,
}

/// Shared session state; turns take the serial lock for exclusive response.create.
struct SessionShared {
    key: OpenAiRespWsSessionKey,
    turn_lock: Arc<Mutex<()>>,
    state: Mutex<SessionInner>,
    /// Count of `begin_turn` holders that received this Arc but have not yet
    /// `finish()`/`Drop`. Includes callers still waiting on `turn_lock`.
    /// Capacity eviction must not remove reserved sessions, or same-affinity
    /// waiters could be orphaned onto a detached Arc while a new map entry is created.
    reserved_turns: AtomicUsize,
}

/// Process-wide (AppState-held) hard-bounded registry of run-scoped WS sessions.
pub struct OpenAiRespWsSessionRegistry {
    sessions: Mutex<HashMap<OpenAiRespWsSessionKey, Arc<SessionShared>>>,
    max_sessions: usize,
    /// Wakes waiters when capacity may free (finish / invalidate / reclaim).
    capacity_notify: Notify,
}

impl Default for OpenAiRespWsSessionRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SESSIONS)
    }
}

impl OpenAiRespWsSessionRegistry {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            max_sessions: max_sessions.max(1),
            capacity_notify: Notify::new(),
        }
    }

    /// Acquire exclusive turn access for this affinity key (serial response.create).
    pub async fn begin_turn(
        self: &Arc<Self>,
        key: OpenAiRespWsSessionKey,
    ) -> OpenAiRespWsSessionTurn {
        // get_or_create reserves the session under the map lock so a concurrent capacity
        // eviction cannot detach it while we wait for turn_lock.
        let session = self.get_or_create_session(key).await;
        // RAII: if this future is cancelled while awaiting turn_lock, Drop releases the
        // reservation so capacity/eviction accounting stays accurate.
        let reservation = TurnReservation {
            registry: Arc::clone(self),
            session: Arc::clone(&session),
            released: false,
        };
        let turn_guard = session.turn_lock.clone().lock_owned().await;
        {
            let mut state = session.state.lock().await;
            state.turn_in_flight = true;
            state.last_used_at = Instant::now();
        }
        OpenAiRespWsSessionTurn {
            session,
            turn_guard: Some(turn_guard),
            reservation: Some(reservation),
        }
    }

    async fn get_or_create_session(&self, key: OpenAiRespWsSessionKey) -> Arc<SessionShared> {
        loop {
            // Register the capacity waiter *before* re-checking the map so a concurrent
            // finish()/invalidate cannot notify_waiters() between drop(sessions) and await
            // (lost wakeup would hang begin_turn indefinitely).
            let notified = self.capacity_notify.notified();
            let mut notified = std::pin::pin!(notified);
            notified.as_mut().enable();

            let mut sessions = self.sessions.lock().await;
            self.reclaim_locked(&mut sessions, Instant::now());
            if let Some(existing) = sessions.get(&key) {
                // Reserve before releasing the map lock so waiters on turn_lock stay pinned.
                existing.reserved_turns.fetch_add(1, Ordering::AcqRel);
                return Arc::clone(existing);
            }

            // Hard capacity: never grow past max_sessions. Wait until an idle, unreserved
            // session can be evicted (or reclaimed) rather than inserting over the limit.
            if sessions.len() >= self.max_sessions {
                if self.evict_one_idle_locked(&mut sessions, Instant::now()) {
                    // Evicted one; loop to insert or re-check.
                    continue;
                }
                drop(sessions);
                notified.as_mut().await;
                continue;
            }

            let shared = Arc::new(SessionShared {
                key: key.clone(),
                turn_lock: Arc::new(Mutex::new(())),
                state: Mutex::new(SessionInner {
                    connection: None,
                    continuation: None,
                    last_used_at: Instant::now(),
                    turn_in_flight: false,
                }),
                // Creator is the first reserved holder until finish/Drop.
                reserved_turns: AtomicUsize::new(1),
            });
            sessions.insert(key, Arc::clone(&shared));
            return shared;
        }
    }

    /// Invalidate all sessions for a chat run (cancel / terminal / delete).
    pub async fn invalidate_run(&self, workspace_id: &str, run_affinity_id: &str) {
        let mut sessions = self.sessions.lock().await;
        let keys: Vec<_> = sessions
            .keys()
            .filter(|key| {
                key.workspace_id == workspace_id && key.run_affinity_id == run_affinity_id
            })
            .cloned()
            .collect();
        for key in keys {
            if let Some(session) = sessions.remove(&key) {
                drop_session_connection(session).await;
            }
        }
        self.capacity_notify.notify_waiters();
    }

    /// Invalidate every session for a workspace (SSH disconnect / broker reconnect / offline).
    pub async fn invalidate_workspace(&self, workspace_id: &str) {
        let mut sessions = self.sessions.lock().await;
        let keys: Vec<_> = sessions
            .keys()
            .filter(|key| key.workspace_id == workspace_id)
            .cloned()
            .collect();
        for key in keys {
            if let Some(session) = sessions.remove(&key) {
                drop_session_connection(session).await;
            }
        }
        self.capacity_notify.notify_waiters();
    }

    /// Drop all sessions (app shutdown).
    pub async fn shutdown_all(&self) {
        let mut sessions = self.sessions.lock().await;
        let values: Vec<_> = sessions.drain().map(|(_, session)| session).collect();
        drop(sessions);
        for session in values {
            drop_session_connection(session).await;
        }
        self.capacity_notify.notify_waiters();
    }

    fn reclaim_locked(
        &self,
        sessions: &mut HashMap<OpenAiRespWsSessionKey, Arc<SessionShared>>,
        now: Instant,
    ) {
        let before = sessions.len();
        sessions.retain(|_, session| {
            // Never reclaim sessions that still have begin_turn holders (including turn_lock waiters).
            if session.reserved_turns.load(Ordering::Acquire) > 0 {
                return true;
            }
            // Best-effort idle reclaim without awaiting session mutex: keep Arc if recently used.
            let Ok(state) = session.state.try_lock() else {
                return true;
            };
            if state.turn_in_flight {
                return true;
            }
            if now.duration_since(state.last_used_at) >= OPENAI_RESP_WS_IDLE_RECLAIM {
                return false;
            }
            if let Some(conn) = state.connection.as_ref()
                && conn.is_expired(now)
            {
                return false;
            }
            true
        });
        if sessions.len() < before {
            self.capacity_notify.notify_waiters();
        }
    }

    fn evict_one_idle_locked(
        &self,
        sessions: &mut HashMap<OpenAiRespWsSessionKey, Arc<SessionShared>>,
        now: Instant,
    ) -> bool {
        let mut oldest: Option<(OpenAiRespWsSessionKey, Instant)> = None;
        for (key, session) in sessions.iter() {
            // Reserved sessions (active or queued on turn_lock) are not capacity-evictable.
            if session.reserved_turns.load(Ordering::Acquire) > 0 {
                continue;
            }
            let Ok(state) = session.state.try_lock() else {
                continue;
            };
            if state.turn_in_flight {
                continue;
            }
            match oldest {
                Some((_, ts)) if state.last_used_at >= ts => {}
                _ => oldest = Some((key.clone(), state.last_used_at)),
            }
            let _ = now;
        }
        if let Some((key, _)) = oldest {
            sessions.remove(&key);
            self.capacity_notify.notify_waiters();
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub async fn session_count(&self) -> usize {
        self.sessions.lock().await.len()
    }

    #[cfg(test)]
    pub async fn has_session(&self, key: &OpenAiRespWsSessionKey) -> bool {
        self.sessions.lock().await.contains_key(key)
    }

    #[cfg(test)]
    pub fn max_sessions(&self) -> usize {
        self.max_sessions
    }

    #[cfg(test)]
    pub async fn reserved_turns(&self, key: &OpenAiRespWsSessionKey) -> Option<usize> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(key)
            .map(|session| session.reserved_turns.load(Ordering::Acquire))
    }

    #[cfg(test)]
    pub async fn session_arc_count(&self, key: &OpenAiRespWsSessionKey) -> Option<usize> {
        let sessions = self.sessions.lock().await;
        sessions.get(key).map(Arc::strong_count)
    }

    /// Stable identity of the in-map `SessionShared` for pointer-equality checks in tests.
    #[cfg(test)]
    pub async fn session_ptr(&self, key: &OpenAiRespWsSessionKey) -> Option<usize> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(key)
            .map(|session| Arc::as_ptr(session) as usize)
    }
}

async fn drop_session_connection(session: Arc<SessionShared>) {
    let mut state = session.state.lock().await;
    if let Some(mut conn) = state.connection.take() {
        let _ = futures_util::SinkExt::close(&mut conn.write).await;
    }
    state.continuation = None;
    state.turn_in_flight = false;
}

/// Pins a session in the registry for one begin_turn attempt (including turn_lock wait).
struct TurnReservation {
    registry: Arc<OpenAiRespWsSessionRegistry>,
    session: Arc<SessionShared>,
    released: bool,
}

impl TurnReservation {
    fn release(&mut self) {
        if self.released {
            return;
        }
        self.session.reserved_turns.fetch_sub(1, Ordering::AcqRel);
        self.released = true;
        self.registry.capacity_notify.notify_waiters();
    }
}

impl Drop for TurnReservation {
    fn drop(&mut self) {
        self.release();
    }
}

/// Exclusive turn handle for one `response.create` cycle.
pub struct OpenAiRespWsSessionTurn {
    session: Arc<SessionShared>,
    /// Unlocked in `Drop` / `finish` *before* releasing `reservation`.
    turn_guard: Option<OwnedMutexGuard<()>>,
    /// Released only after `turn_guard` so capacity waiters never observe
    /// `reserved_turns == 0` while this turn still holds `turn_lock` (fork race).
    reservation: Option<TurnReservation>,
}

impl OpenAiRespWsSessionTurn {
    pub fn key(&self) -> &OpenAiRespWsSessionKey {
        &self.session.key
    }

    /// Take a reusable live connection only when connection config identity still matches.
    /// On identity mismatch, age-out, or missing socket: close any stale socket, clear
    /// continuation, and return None so the caller opens a fresh connection.
    pub(crate) async fn take_connection_for_identity(
        &self,
        connection_identity: u64,
    ) -> Option<LiveWsConnection> {
        let mut state = self.session.state.lock().await;
        let now = Instant::now();

        match state.connection.take() {
            Some(conn)
                if conn.connection_identity == connection_identity && !conn.is_expired(now) =>
            {
                state.last_used_at = now;
                Some(conn)
            }
            Some(mut conn) => {
                let reason = if conn.connection_identity != connection_identity {
                    "provider connection config changed"
                } else {
                    "rotating connection before 60m limit"
                };
                tracing::info!(
                    workspace_id = %self.session.key.workspace_id,
                    run_affinity_id = %self.session.key.run_affinity_id,
                    provider_id = %self.session.key.provider_id,
                    model_id = %self.session.key.model_id,
                    reason,
                    "openai responses websocket: discarding live socket"
                );
                let _ = futures_util::SinkExt::close(&mut conn.write).await;
                // Must not keep previous_response_id across connection rebuild.
                state.continuation = None;
                None
            }
            None => {
                // Missing socket: force full-context create (continuation already unsafe).
                state.continuation = None;
                None
            }
        }
    }

    pub async fn clear_continuation(&self, reason: &str) {
        let mut state = self.session.state.lock().await;
        if state.continuation.is_some() {
            tracing::info!(
                workspace_id = %self.session.key.workspace_id,
                run_affinity_id = %self.session.key.run_affinity_id,
                reason,
                "openai responses websocket: clearing previous_response_id continuation"
            );
        }
        state.continuation = None;
    }

    /// Apply continuation when safe: set store/previous_response_id and keep only new messages.
    ///
    /// Safety requires matching routing fingerprint **and** that messages[0..committed_len]
    /// still have the same content hash as at commit time (compression / prompt edits / reorders
    /// force a full resend).
    pub async fn apply_continuation(
        &self,
        mut chat_request: ChatRequest,
        fingerprint: u64,
        enable_continuation: bool,
    ) -> AppliedContinuation {
        let full_messages_len = chat_request.messages.len();
        let full_messages_hash = messages_prefix_hash(&chat_request.messages, full_messages_len);

        if !enable_continuation {
            chat_request.store = Some(false);
            chat_request.previous_response_id = None;
            return AppliedContinuation {
                chat_request,
                used_previous_response_id: None,
                full_messages_len,
                full_messages_hash,
                fingerprint,
            };
        }

        let mut used_previous = None;

        {
            let state = self.session.state.lock().await;
            if let Some(cont) = state.continuation.as_ref()
                && cont.fingerprint == fingerprint
                && cont.committed_messages_len <= full_messages_len
                && !cont.previous_response_id.is_empty()
            {
                let current_prefix_hash =
                    messages_prefix_hash(&chat_request.messages, cont.committed_messages_len);
                if current_prefix_hash == cont.committed_prefix_hash {
                    // Only append new items; server already has the prefix.
                    let delta = chat_request.messages.split_off(cont.committed_messages_len);
                    chat_request.messages = delta;
                    chat_request.previous_response_id = Some(cont.previous_response_id.clone());
                    used_previous = Some(cont.previous_response_id.clone());
                } else {
                    tracing::info!(
                        workspace_id = %self.session.key.workspace_id,
                        run_affinity_id = %self.session.key.run_affinity_id,
                        committed_len = cont.committed_messages_len,
                        "openai responses websocket: committed message prefix changed; full context"
                    );
                }
            }
        }

        // Always opt into store for chat-completion affinity sessions so the next turn can continue.
        chat_request.store = Some(true);
        if used_previous.is_none() {
            chat_request.previous_response_id = None;
        }

        AppliedContinuation {
            chat_request,
            used_previous_response_id: used_previous,
            full_messages_len,
            full_messages_hash,
            fingerprint,
        }
    }

    /// Record successful Complete response_id for the next turn.
    pub async fn commit_success(
        &self,
        response_id: Option<String>,
        full_messages_len: usize,
        full_messages_hash: u64,
        fingerprint: u64,
        enable_continuation: bool,
    ) {
        let mut state = self.session.state.lock().await;
        state.last_used_at = Instant::now();
        if !enable_continuation {
            state.continuation = None;
            return;
        }
        match response_id.filter(|id| !id.trim().is_empty()) {
            Some(id) => {
                state.continuation = Some(ContinuationState {
                    fingerprint,
                    committed_prefix_hash: full_messages_hash,
                    committed_messages_len: full_messages_len,
                    previous_response_id: id,
                });
            }
            None => {
                state.continuation = None;
            }
        }
    }

    /// Failure / incomplete: never keep a failed or unknown response id.
    pub async fn commit_failure(&self) {
        let mut state = self.session.state.lock().await;
        state.continuation = None;
        state.last_used_at = Instant::now();
    }

    /// Return a healthy connection for reuse; close if `keep` is false.
    pub(crate) async fn return_connection(&self, connection: Option<LiveWsConnection>, keep: bool) {
        let mut state = self.session.state.lock().await;
        state.last_used_at = Instant::now();
        if let Some(mut conn) = connection {
            if keep && !conn.is_expired(Instant::now()) {
                state.connection = Some(conn);
            } else {
                let _ = futures_util::SinkExt::close(&mut conn.write).await;
                // Dropped/rebuilt socket must not keep server-side continuation assumptions.
                if !keep {
                    state.continuation = None;
                }
            }
        }
    }

    pub async fn finish(self) {
        {
            let mut state = self.session.state.lock().await;
            state.turn_in_flight = false;
            state.last_used_at = Instant::now();
        }
        // Drop order is enforced in `Drop` (unlock, then release reservation).
    }

    /// Unlock the serial turn lock, then release the capacity reservation.
    /// Order is load-bearing: reverse it and capacity eviction can fork same-affinity sessions.
    fn release_guards(&mut self) {
        drop(self.turn_guard.take());
        drop(self.reservation.take());
    }

    /// Test-only: mark not in-flight, unlock, run `between` while reservation still pins the
    /// map entry, then release reservation. Proves capacity waiters cannot fork the session
    /// in the unlock-before-reservation window.
    #[cfg(test)]
    pub async fn finish_between_unlock_and_reservation_for_test<F, Fut>(mut self, between: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        {
            let mut state = self.session.state.lock().await;
            state.turn_in_flight = false;
            state.last_used_at = Instant::now();
        }
        drop(self.turn_guard.take());
        between().await;
        drop(self.reservation.take());
        // Prevent Drop from double-releasing (guards already taken).
        self.turn_guard = None;
        self.reservation = None;
    }

    /// Test-only inverted order: release reservation while still holding `turn_lock`.
    /// Used to document why unlock-before-reservation is required.
    #[cfg(test)]
    pub async fn finish_reservation_before_unlock_for_test<F, Fut>(mut self, between: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        {
            let mut state = self.session.state.lock().await;
            state.turn_in_flight = false;
            state.last_used_at = Instant::now();
        }
        drop(self.reservation.take());
        between().await;
        drop(self.turn_guard.take());
        self.turn_guard = None;
        self.reservation = None;
    }
}

impl Drop for OpenAiRespWsSessionTurn {
    fn drop(&mut self) {
        // Best-effort: mark not in-flight if the turn was abandoned without finish().
        if let Ok(mut state) = self.session.state.try_lock() {
            state.turn_in_flight = false;
            state.last_used_at = Instant::now();
        }
        // CRITICAL: unlock before releasing reservation. See release_guards docs.
        self.release_guards();
    }
}

#[derive(Debug)]
pub struct AppliedContinuation {
    pub chat_request: ChatRequest,
    pub used_previous_response_id: Option<String>,
    pub full_messages_len: usize,
    /// Content hash of the full message list before any delta split (for commit).
    pub full_messages_hash: u64,
    pub fingerprint: u64,
}

/// Fingerprint of fields that must match for previous_response_id reuse.
/// Includes connection identity so endpoint/credential changes force full context.
pub fn continuation_fingerprint(
    model_id: &str,
    base_url: Option<&str>,
    provider_kind: &str,
    system: Option<&str>,
    tools_json: &str,
    connection_identity: u64,
    options_json: &str,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model_id.hash(&mut hasher);
    base_url.unwrap_or("").hash(&mut hasher);
    provider_kind.hash(&mut hasher);
    system.unwrap_or("").hash(&mut hasher);
    tools_json.hash(&mut hasher);
    connection_identity.hash(&mut hasher);
    options_json.hash(&mut hasher);
    hasher.finish()
}

/// Stable hash of Provider connection fields that bind a live WebSocket.
/// API key is hashed (never stored); empty key still participates as a sentinel.
pub fn connection_identity_from_parts(
    kind: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
    proxy_url: Option<&str>,
    request_overrides_json: &str,
    model_redirects_json: &str,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    base_url.unwrap_or("").hash(&mut hasher);
    // Hash the key material so credential rotation invalidates the socket without storing secrets.
    api_key.unwrap_or("").hash(&mut hasher);
    proxy_url.unwrap_or("").hash(&mut hasher);
    request_overrides_json.hash(&mut hasher);
    model_redirects_json.hash(&mut hasher);
    hasher.finish()
}

pub fn tools_fingerprint_json(tools: &Option<Vec<genai::chat::Tool>>) -> String {
    match tools {
        Some(tools) => serde_json::to_string(tools).unwrap_or_else(|_| "[]".to_string()),
        None => "null".to_string(),
    }
}

pub fn chat_options_fingerprint_json(options: &ChatOptions) -> String {
    // Per-turn correlation headers (x-client-request-id / x-foco-run-id) must not
    // invalidate previous_response_id continuation. Identity, session/thread, and
    // OpenAI-Beta remain part of the fingerprint.
    let mut value = match serde_json::to_value(options) {
        Ok(value) => value,
        Err(_) => return "{}".to_string(),
    };
    if let Some(extra_headers) = value
        .get_mut("extra_headers")
        .and_then(|v| v.as_object_mut())
    {
        for name in crate::AGENT_VOLATILE_HEADER_NAMES {
            let key = name.to_ascii_lowercase();
            let victims: Vec<String> = extra_headers
                .keys()
                .filter(|existing| existing.eq_ignore_ascii_case(name) || existing.as_str() == key)
                .cloned()
                .collect();
            for victim in victims {
                extra_headers.remove(&victim);
            }
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Content fingerprint for messages[0..len] (or all messages when len exceeds length).
pub fn messages_prefix_hash(messages: &[ChatMessage], len: usize) -> u64 {
    let end = len.min(messages.len());
    let prefix = &messages[..end];
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match serde_json::to_string(prefix) {
        Ok(json) => json.hash(&mut hasher),
        Err(_) => {
            // Fallback: length + debug to avoid panicking; forces mismatch if serialization fails inconsistently.
            end.hash(&mut hasher);
            for message in prefix {
                format!("{message:?}").hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use genai::chat::ChatMessage;

    #[tokio::test]
    async fn session_keys_isolate_workspace_run_provider_model() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(16));
        let a = OpenAiRespWsSessionKey::new("ws1", "run1", "p1", "m1");
        let b = OpenAiRespWsSessionKey::new("ws1", "run2", "p1", "m1");
        let turn_a = registry.begin_turn(a.clone()).await;
        turn_a.finish().await;
        let turn_b = registry.begin_turn(b.clone()).await;
        turn_b.finish().await;
        assert!(registry.has_session(&a).await);
        assert!(registry.has_session(&b).await);
        assert_eq!(registry.session_count().await, 2);
    }

    #[tokio::test]
    async fn invalidate_run_removes_matching_sessions_only() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(16));
        let keep = OpenAiRespWsSessionKey::new("ws1", "run-keep", "p1", "m1");
        let drop_key = OpenAiRespWsSessionKey::new("ws1", "run-drop", "p1", "m1");
        registry.begin_turn(keep.clone()).await.finish().await;
        registry.begin_turn(drop_key.clone()).await.finish().await;
        registry.invalidate_run("ws1", "run-drop").await;
        assert!(registry.has_session(&keep).await);
        assert!(!registry.has_session(&drop_key).await);
    }

    #[tokio::test]
    async fn continuation_sends_delta_and_previous_response_id() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let key = OpenAiRespWsSessionKey::new("ws", "run", "prov", "model");
        let turn = registry.begin_turn(key).await;
        let fingerprint = continuation_fingerprint(
            "model",
            Some("https://x/v1/"),
            "ws",
            Some("sys"),
            "null",
            1,
            "{}",
        );

        let first = ChatRequest::from_messages(vec![ChatMessage::user("u1")]).with_system("sys");
        let applied = turn.apply_continuation(first, fingerprint, true).await;
        assert!(applied.used_previous_response_id.is_none());
        assert_eq!(applied.chat_request.store, Some(true));
        assert_eq!(applied.chat_request.messages.len(), 1);

        turn.commit_success(
            Some("resp_1".into()),
            applied.full_messages_len,
            applied.full_messages_hash,
            fingerprint,
            true,
        )
        .await;

        let second = ChatRequest::from_messages(vec![
            ChatMessage::user("u1"),
            ChatMessage::assistant("a1"),
            ChatMessage::user("u2"),
        ])
        .with_system("sys");
        let applied2 = turn.apply_continuation(second, fingerprint, true).await;
        assert_eq!(
            applied2.used_previous_response_id.as_deref(),
            Some("resp_1")
        );
        assert_eq!(applied2.chat_request.messages.len(), 2);
        assert_eq!(
            applied2.chat_request.previous_response_id.as_deref(),
            Some("resp_1")
        );
        turn.finish().await;
    }

    #[tokio::test]
    async fn fingerprint_mismatch_clears_continuation_path() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let key = OpenAiRespWsSessionKey::new("ws", "run", "prov", "model");
        let turn = registry.begin_turn(key).await;
        let fp1 = continuation_fingerprint(
            "model",
            Some("https://x/v1/"),
            "ws",
            Some("sys"),
            "null",
            1,
            "{}",
        );
        let first = ChatRequest::from_messages(vec![ChatMessage::user("u1")]).with_system("sys");
        let applied = turn.apply_continuation(first, fp1, true).await;
        turn.commit_success(
            Some("resp_1".into()),
            applied.full_messages_len,
            applied.full_messages_hash,
            fp1,
            true,
        )
        .await;

        let fp2 = continuation_fingerprint(
            "model",
            Some("https://x/v1/"),
            "ws",
            Some("sys-changed"),
            "null",
            1,
            "{}",
        );
        let second =
            ChatRequest::from_messages(vec![ChatMessage::user("u1"), ChatMessage::user("u2")])
                .with_system("sys-changed");
        let applied2 = turn.apply_continuation(second, fp2, true).await;
        assert!(applied2.used_previous_response_id.is_none());
        assert_eq!(applied2.chat_request.messages.len(), 2);
        assert!(applied2.chat_request.previous_response_id.is_none());
        turn.finish().await;
    }

    #[tokio::test]
    async fn prefix_content_change_blocks_continuation_even_if_length_grows() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let key = OpenAiRespWsSessionKey::new("ws", "run", "prov", "model");
        let turn = registry.begin_turn(key).await;
        let fingerprint = continuation_fingerprint(
            "model",
            Some("https://x/v1/"),
            "ws",
            Some("sys"),
            "null",
            1,
            "{}",
        );

        let first = ChatRequest::from_messages(vec![
            ChatMessage::user("original-user"),
            ChatMessage::assistant("original-assistant"),
        ])
        .with_system("sys");
        let applied = turn.apply_continuation(first, fingerprint, true).await;
        turn.commit_success(
            Some("resp_1".into()),
            applied.full_messages_len,
            applied.full_messages_hash,
            fingerprint,
            true,
        )
        .await;

        // Same length or longer, but prefix content replaced (e.g. context compression summary).
        let compressed = ChatRequest::from_messages(vec![
            ChatMessage::user("SUMMARY_OF_HISTORY"),
            ChatMessage::user("new-turn"),
        ])
        .with_system("sys");
        let applied2 = turn.apply_continuation(compressed, fingerprint, true).await;
        assert!(
            applied2.used_previous_response_id.is_none(),
            "must not continue when committed prefix content changed"
        );
        assert_eq!(applied2.chat_request.messages.len(), 2);
        turn.finish().await;
    }

    #[tokio::test]
    async fn failure_clears_previous_response_id() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let key = OpenAiRespWsSessionKey::new("ws", "run", "prov", "model");
        let turn = registry.begin_turn(key).await;
        let fp = continuation_fingerprint("m", None, "k", None, "null", 0, "{}");
        turn.commit_success(Some("resp_x".into()), 1, 99, fp, true)
            .await;
        turn.commit_failure().await;
        let req = ChatRequest::from_messages(vec![ChatMessage::user("a"), ChatMessage::user("b")]);
        let applied = turn.apply_continuation(req, fp, true).await;
        assert!(applied.used_previous_response_id.is_none());
        turn.finish().await;
    }

    #[test]
    fn continuation_fingerprint_changes_with_tools_base_url_identity_and_options() {
        let a = continuation_fingerprint("m", Some("https://a/"), "k", Some("s"), "[1]", 1, "{}");
        let b = continuation_fingerprint("m", Some("https://b/"), "k", Some("s"), "[1]", 1, "{}");
        let c = continuation_fingerprint("m", Some("https://a/"), "k", Some("s"), "[2]", 1, "{}");
        let d = continuation_fingerprint("m", Some("https://a/"), "k", Some("s"), "[1]", 2, "{}");
        let e = continuation_fingerprint(
            "m",
            Some("https://a/"),
            "k",
            Some("s"),
            "[1]",
            1,
            "{\"temperature\":0.2}",
        );
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, e);
    }

    #[test]
    fn connection_identity_changes_with_api_key_and_overrides() {
        let a = connection_identity_from_parts(
            "openai-responses-websocket",
            Some("https://api.openai.com/v1/"),
            Some("sk-a"),
            None,
            "[]",
            "[]",
        );
        let b = connection_identity_from_parts(
            "openai-responses-websocket",
            Some("https://api.openai.com/v1/"),
            Some("sk-b"),
            None,
            "[]",
            "[]",
        );
        let c = connection_identity_from_parts(
            "openai-responses-websocket",
            Some("https://gateway.example/v1/"),
            Some("sk-a"),
            None,
            "[]",
            "[]",
        );
        let d = connection_identity_from_parts(
            "openai-responses-websocket",
            Some("https://api.openai.com/v1/"),
            Some("sk-a"),
            None,
            "[{\"name\":\"x\"}]",
            "[]",
        );
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn messages_prefix_hash_detects_content_change() {
        let a = vec![ChatMessage::user("one"), ChatMessage::assistant("two")];
        let b = vec![ChatMessage::user("ONE"), ChatMessage::assistant("two")];
        assert_eq!(messages_prefix_hash(&a, 2), messages_prefix_hash(&a, 2));
        assert_ne!(messages_prefix_hash(&a, 2), messages_prefix_hash(&b, 2));
        assert_ne!(messages_prefix_hash(&a, 1), messages_prefix_hash(&a, 2));
    }

    #[test]
    fn chat_options_fingerprint_ignores_volatile_agent_headers() {
        use genai::Headers;
        use genai::chat::ChatOptions;

        let stable = ChatOptions::default().with_extra_headers(Headers::from(vec![
            ("session-id".to_string(), "chat-1".to_string()),
            ("thread-id".to_string(), "chat-1".to_string()),
            ("originator".to_string(), "foco".to_string()),
            ("x-client-request-id".to_string(), "req-turn-1".to_string()),
            ("x-foco-run-id".to_string(), "run-1".to_string()),
        ]));
        let next_turn = ChatOptions::default().with_extra_headers(Headers::from(vec![
            ("session-id".to_string(), "chat-1".to_string()),
            ("thread-id".to_string(), "chat-1".to_string()),
            ("originator".to_string(), "foco".to_string()),
            ("x-client-request-id".to_string(), "req-turn-2".to_string()),
            ("x-foco-run-id".to_string(), "run-2".to_string()),
        ]));
        let session_changed = ChatOptions::default().with_extra_headers(Headers::from(vec![
            ("session-id".to_string(), "other-session".to_string()),
            ("thread-id".to_string(), "chat-1".to_string()),
            ("originator".to_string(), "foco".to_string()),
            ("x-client-request-id".to_string(), "req-turn-1".to_string()),
        ]));

        assert_eq!(
            chat_options_fingerprint_json(&stable),
            chat_options_fingerprint_json(&next_turn),
            "per-turn x-client-request-id / x-foco-run-id must not break continuation fingerprint"
        );
        assert_ne!(
            chat_options_fingerprint_json(&stable),
            chat_options_fingerprint_json(&session_changed),
            "session-id changes must still invalidate continuation fingerprint"
        );
    }

    #[tokio::test]
    async fn hard_max_sessions_does_not_grow_past_limit() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(2));
        let k1 = OpenAiRespWsSessionKey::new("ws", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws", "r2", "p", "m");
        let k3 = OpenAiRespWsSessionKey::new("ws", "r3", "p", "m");

        let t1 = registry.begin_turn(k1.clone()).await;
        let t2 = registry.begin_turn(k2.clone()).await;
        assert_eq!(registry.session_count().await, 2);

        // Finish one so eviction can free a slot for the third key.
        t1.finish().await;
        let t3 = registry.begin_turn(k3.clone()).await;
        t2.finish().await;
        t3.finish().await;

        assert!(registry.session_count().await <= registry.max_sessions());
        assert!(registry.has_session(&k3).await);
    }

    /// Regression: waiters must register with Notify before releasing the registry lock so a
    /// concurrent finish() cannot notify_waiters() into the void between drop(lock) and await.
    #[tokio::test]
    async fn capacity_wait_wakes_when_busy_session_finishes() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(1));
        let k1 = OpenAiRespWsSessionKey::new("ws", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws", "r2", "p", "m");

        let t1 = registry.begin_turn(k1).await;
        assert_eq!(registry.session_count().await, 1);

        let reg = Arc::clone(&registry);
        let waiter = tokio::spawn(async move {
            let t2 = reg.begin_turn(k2).await;
            t2.finish().await;
        });

        // Allow the waiter to observe full capacity and park on capacity_notify.
        for _ in 0..20 {
            tokio::task::yield_now().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
            if waiter.is_finished() {
                break;
            }
        }

        t1.finish().await;

        tokio::time::timeout(Duration::from_secs(3), waiter)
            .await
            .expect("begin_turn must not hang after capacity frees")
            .expect("waiter task should complete");
    }

    #[tokio::test]
    async fn invalidate_workspace_frees_capacity_for_waiters() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(1));
        let k1 = OpenAiRespWsSessionKey::new("ws-a", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws-b", "r2", "p", "m");

        let t1 = registry.begin_turn(k1).await;
        let reg = Arc::clone(&registry);
        let waiter = tokio::spawn(async move {
            let t2 = reg.begin_turn(k2).await;
            t2.finish().await;
        });

        for _ in 0..20 {
            tokio::task::yield_now().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
            if waiter.is_finished() {
                break;
            }
        }

        // Drop without finish is ok; Drop notifies capacity. Prefer explicit invalidate path.
        drop(t1);
        registry.invalidate_workspace("ws-a").await;

        tokio::time::timeout(Duration::from_secs(3), waiter)
            .await
            .expect("invalidate_workspace must wake capacity waiters")
            .expect("waiter task should complete");
    }

    /// Regression: same-affinity begin_turn that is still waiting on turn_lock must pin the
    /// map entry via reserved_turns. Otherwise capacity eviction can remove the entry, leave
    /// the waiter on a detached Arc, and allow a second SessionShared for the same key
    /// (forked response.create serial chain + live sockets beyond max_sessions).
    #[tokio::test]
    async fn queued_same_affinity_turn_prevents_capacity_eviction_fork() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(1));
        let k1 = OpenAiRespWsSessionKey::new("ws", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws", "r2", "p", "m");

        let t1 = registry.begin_turn(k1.clone()).await;
        assert_eq!(registry.reserved_turns(&k1).await, Some(1));
        assert_eq!(registry.session_count().await, 1);

        let reg_q = Arc::clone(&registry);
        let k1_q = k1.clone();
        let queued = tokio::spawn(async move {
            let turn = reg_q.begin_turn(k1_q).await;
            turn.finish().await;
        });

        // Wait until the queued same-affinity call has reserved the session (still on turn_lock).
        for _ in 0..100 {
            if registry.reserved_turns(&k1).await == Some(2) {
                break;
            }
            tokio::task::yield_now().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            registry.reserved_turns(&k1).await,
            Some(2),
            "queued same-affinity turn must reserve before acquiring turn_lock"
        );
        assert!(registry.has_session(&k1).await);
        // map entry + t1 + queued Arc while waiting on lock
        assert!(
            registry.session_arc_count(&k1).await.unwrap_or(0) >= 3,
            "queued waiter must hold the same map Arc"
        );

        let reg_k2 = Arc::clone(&registry);
        let k2_w = k2.clone();
        let k2_waiter = tokio::spawn(async move {
            let turn = reg_k2.begin_turn(k2_w).await;
            turn.finish().await;
        });

        // Give k2 time to attempt capacity eviction — it must stay blocked.
        for _ in 0..30 {
            tokio::task::yield_now().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
            assert!(
                !k2_waiter.is_finished(),
                "k2 must not steal capacity while k1 has a queued turn"
            );
            assert!(
                registry.has_session(&k1).await,
                "k1 map entry must stay while reserved_turns > 0"
            );
            assert_eq!(
                registry.session_count().await,
                1,
                "registry map must stay at hard max while k1 is reserved"
            );
            assert_eq!(registry.reserved_turns(&k1).await, Some(2));
        }

        // First k1 turn completes; queued k1 acquires the same session.
        t1.finish().await;
        tokio::time::timeout(Duration::from_secs(3), queued)
            .await
            .expect("queued same-affinity turn must acquire after first finishes")
            .expect("queued task ok");

        // Only after all k1 holders release can k2 enter (single slot).
        tokio::time::timeout(Duration::from_secs(3), k2_waiter)
            .await
            .expect("k2 must proceed after k1 fully releases reservation")
            .expect("k2 task ok");

        assert!(registry.session_count().await <= registry.max_sessions());
    }

    /// Regression for reservation/turn_lock drop order under capacity pressure.
    ///
    /// Critical window: `t1` is the *only* reservation on `k1`, and `k2` is waiting for a free
    /// slot. After unlock but while reservation still pins the map entry, a concurrent same-
    /// affinity `begin_turn` must attach to the same `SessionShared` (not a forked Arc).
    /// Releasing reservation before unlock would let capacity eviction remove the entry while
    /// `turn_lock` is still held on a detached Arc.
    #[tokio::test]
    async fn finish_does_not_fork_same_affinity_under_capacity_pressure() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(1));
        let k1 = OpenAiRespWsSessionKey::new("ws", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws", "r2", "p", "m");

        let t1 = registry.begin_turn(k1.clone()).await;
        assert_eq!(registry.reserved_turns(&k1).await, Some(1));
        let ptr_before = registry
            .session_ptr(&k1)
            .await
            .expect("k1 session must exist");

        // Fill capacity wait queue with a different affinity.
        let reg_k2 = Arc::clone(&registry);
        let k2w = k2.clone();
        let k2_waiter = tokio::spawn(async move {
            let turn = reg_k2.begin_turn(k2w).await;
            turn.finish().await;
        });
        // Park long enough for k2 to observe full capacity and wait.
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(
            registry.has_session(&k1).await,
            "k1 must remain the sole map entry while reserved"
        );
        assert_eq!(registry.session_count().await, 1);

        let reg = Arc::clone(&registry);
        let k1_for_mid = k1.clone();
        let same_affinity_handle: Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<usize>>>> =
            Arc::new(std::sync::Mutex::new(None));
        let release_same_affinity = Arc::new(tokio::sync::Notify::new());

        // Correct order window: unlocked, reservation still held → pin same Arc.
        t1.finish_between_unlock_and_reservation_for_test(|| {
            let reg = Arc::clone(&reg);
            let k1_for_mid = k1_for_mid.clone();
            let same_affinity_handle = Arc::clone(&same_affinity_handle);
            let release_same_affinity = Arc::clone(&release_same_affinity);
            async move {
                assert_eq!(
                    reg.reserved_turns(&k1_for_mid).await,
                    Some(1),
                    "sole reservation must still pin the map entry after unlock"
                );
                assert!(
                    reg.has_session(&k1_for_mid).await,
                    "capacity waiters must not evict while reservation remains"
                );
                assert_eq!(
                    reg.session_ptr(&k1_for_mid).await,
                    Some(ptr_before),
                    "map entry must be the original SessionShared"
                );

                // Start same-affinity begin_turn while reservation still pins the entry.
                // Hold the turn until the parent asserts pinning, so reserved_turns stays >= 2.
                let reg2 = Arc::clone(&reg);
                let k1b = k1_for_mid.clone();
                let release_same_affinity = Arc::clone(&release_same_affinity);
                let handle = tokio::spawn(async move {
                    let turn = reg2.begin_turn(k1b.clone()).await;
                    let ptr = reg2
                        .session_ptr(&k1b)
                        .await
                        .expect("session must exist while turn is held");
                    release_same_affinity.notified().await;
                    turn.finish().await;
                    ptr
                });
                *same_affinity_handle.lock().unwrap() = Some(handle);

                // Wait until the same-affinity turn reserved (queued or acquired).
                for _ in 0..200 {
                    if reg.reserved_turns(&k1_for_mid).await.unwrap_or(0) >= 2 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                assert!(
                    reg.reserved_turns(&k1_for_mid).await.unwrap_or(0) >= 2,
                    "same-affinity begin_turn must pin original session before reservation drop"
                );
                assert_eq!(
                    reg.session_ptr(&k1_for_mid).await,
                    Some(ptr_before),
                    "must not create a second SessionShared for the same affinity"
                );
            }
        })
        .await;

        // t1 reservation is now released; same-affinity still holds the pin.
        assert!(
            registry.reserved_turns(&k1).await.unwrap_or(0) >= 1,
            "same-affinity turn must keep the session reserved after t1 finishes"
        );
        assert_eq!(
            registry.session_ptr(&k1).await,
            Some(ptr_before),
            "map entry must remain the original SessionShared after t1 reservation drop"
        );

        release_same_affinity.notify_one();
        let same_affinity = same_affinity_handle
            .lock()
            .unwrap()
            .take()
            .expect("same-affinity task must have been spawned in the unlock window");
        let ptr_seen = tokio::time::timeout(Duration::from_secs(3), same_affinity)
            .await
            .expect("same-affinity turn must complete")
            .expect("same-affinity task ok");
        assert_eq!(
            ptr_seen, ptr_before,
            "same-affinity turn must reuse the original SessionShared"
        );

        tokio::time::timeout(Duration::from_secs(3), k2_waiter)
            .await
            .expect("k2 must proceed after k1 fully releases")
            .expect("k2 task ok");

        assert!(registry.session_count().await <= registry.max_sessions());
    }

    /// Documents the inverted release order hazard: once reservation hits zero while
    /// `turn_lock` is still held, capacity eviction may remove the map entry.
    #[tokio::test]
    async fn reservation_before_unlock_allows_capacity_eviction_of_map_entry() {
        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(1));
        let k1 = OpenAiRespWsSessionKey::new("ws", "r1", "p", "m");
        let k2 = OpenAiRespWsSessionKey::new("ws", "r2", "p", "m");

        let t1 = registry.begin_turn(k1.clone()).await;
        assert_eq!(registry.reserved_turns(&k1).await, Some(1));

        let reg_k2 = Arc::clone(&registry);
        let k2w = k2.clone();
        let k2_waiter = tokio::spawn(async move {
            let turn = reg_k2.begin_turn(k2w).await;
            turn.finish().await;
        });
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }

        // Wrong order: drop reservation while still holding turn_lock → k2 can take the slot.
        t1.finish_reservation_before_unlock_for_test(|| {
            let reg = Arc::clone(&registry);
            let k1 = k1.clone();
            async move {
                // Give capacity waiter a chance to evict after reservation hit zero.
                for _ in 0..100 {
                    if !reg.has_session(&k1).await {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            }
        })
        .await;

        tokio::time::timeout(Duration::from_secs(3), k2_waiter)
            .await
            .expect("k2 must complete after inverted release frees capacity")
            .expect("k2 task ok");

        // After inverted order, k1 may already be gone (evicted for k2) — that is the hazard.
        // Production code uses unlock-before-reservation so same-affinity waiters stay pinned.
        assert!(registry.session_count().await <= registry.max_sessions());
    }
}
