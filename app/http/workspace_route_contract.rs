//! Machine-checkable remote-workspace HTTP contract.
//!
//! The browser keeps using the local workspace URL shape for SSH workspaces.
//! This inventory records where that request must execute and which differences
//! are intentional. It is deliberately separate from provider configuration:
//! provider secrets and real LLM wire remain on the main process, while
//! workspace/chat data remains in the remote workspace database.

use crate::remote_workspace::route_policy::{
    BrokerRequirement, RemoteRouteAlignment, RemoteRouteAuthority,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkspaceRouteMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    WebSocket,
}

impl WorkspaceRouteMethod {
    pub(crate) fn from_http_method(method: &str, is_websocket: bool) -> Option<Self> {
        if is_websocket {
            return Some(Self::WebSocket);
        }
        match method {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }

    #[cfg(test)]
    const fn sidecar_router_prefix(self) -> &'static str {
        match self {
            Self::Get | Self::WebSocket => "get(",
            Self::Post => "post(",
            Self::Put => "put(",
            Self::Patch => "patch(",
            Self::Delete => "delete(",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkspaceRouteContract {
    /// Stable name used by tests and future route implementations.
    pub(crate) id: &'static str,
    /// Browser-visible local route template, including the HTTP method.
    pub(crate) browser_path: &'static str,
    pub(crate) method: WorkspaceRouteMethod,
    /// First path segment after `{workspace_id}` accepted by the main proxy.
    pub(crate) proxy_prefix: Option<&'static str>,
    /// Exact sidecar route used after main-process proxy rewriting.
    pub(crate) sidecar_path: Option<&'static str>,
    pub(crate) authority: RemoteRouteAuthority,
    pub(crate) alignment: RemoteRouteAlignment,
    pub(crate) broker: BrokerRequirement,
    /// User-visible failure behavior when the remote execution dependency is down.
    pub(crate) offline_behavior: &'static str,
    /// Why a non-required route differs from the normal parity path.
    pub(crate) exception: Option<&'static str>,
}

macro_rules! route {
    ($id:literal, $path:literal, $method:ident, $proxy:expr, $sidecar:expr, $authority:ident, $alignment:ident, $broker:ident, $offline:literal $(, $exception:literal)?) => {
        WorkspaceRouteContract {
            id: $id,
            browser_path: $path,
            method: WorkspaceRouteMethod::$method,
            proxy_prefix: $proxy,
            sidecar_path: $sidecar,
            authority: RemoteRouteAuthority::$authority,
            alignment: RemoteRouteAlignment::$alignment,
            broker: BrokerRequirement::$broker,
            offline_behavior: $offline,
            exception: route!(@exception $($exception)?),
        }
    };
    (@exception $exception:literal) => { Some($exception) };
    (@exception) => { None };
}

/// The authoritative Phase 1 inventory of browser-facing workspace capabilities.
///
/// A route may have a different physical execution host without being a product
/// gap. In particular, provider secrets/wire and Global Memory stay on the main
/// process, whereas remote workspace/chat state is sidecar-owned.
pub(crate) const WORKSPACE_ROUTE_CONTRACTS: &[WorkspaceRouteContract] = &[
    route!(
        "workspace-chats",
        "/api/workspaces/{workspace_id}/chats",
        Get,
        Some("chats"),
        Some("/api/remote/workspace/chats"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-tree",
        "/api/workspaces/{workspace_id}/files",
        Get,
        Some("files"),
        Some("/api/remote/workspace/files"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-children",
        "/api/workspaces/{workspace_id}/files/children",
        Get,
        Some("files"),
        Some("/api/remote/workspace/files/children"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-content",
        "/api/workspaces/{workspace_id}/files/content",
        Post,
        Some("files"),
        Some("/api/remote/workspace/files/content"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-blob",
        "/api/workspaces/{workspace_id}/files/blob",
        Get,
        Some("files"),
        Some("/api/remote/workspace/files/blob"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-download",
        "/api/workspaces/{workspace_id}/files/download",
        Get,
        Some("files"),
        Some("/api/remote/workspace/files/download"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-save",
        "/api/workspaces/{workspace_id}/files/save",
        Post,
        Some("files"),
        Some("/api/remote/workspace/files/save"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-delete",
        "/api/workspaces/{workspace_id}/files/delete",
        Post,
        Some("files"),
        Some("/api/remote/workspace/files/delete"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "files-rename",
        "/api/workspaces/{workspace_id}/files/rename",
        Post,
        Some("files"),
        Some("/api/remote/workspace/files/rename"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "spec-read",
        "/api/workspaces/{workspace_id}/spec",
        Get,
        Some("spec"),
        Some("/api/remote/workspace/spec"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "spec-write",
        "/api/workspaces/{workspace_id}/spec",
        Put,
        Some("spec"),
        Some("/api/remote/workspace/spec"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "spec-settings",
        "/api/workspaces/{workspace_id}/spec/settings",
        Put,
        Some("spec"),
        Some("/api/remote/workspace/spec/settings"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "spec-generate",
        "/api/workspaces/{workspace_id}/spec/generate",
        Post,
        Some("spec"),
        Some("/api/remote/workspace/spec/generate"),
        Sidecar,
        Required,
        Required,
        "502 when the broker cannot provide the LLM",
        "The sidecar persists the job; the broker supplies only the LLM turn."
    ),
    route!(
        "spec-jobs-list",
        "/api/workspaces/{workspace_id}/spec/jobs",
        Get,
        Some("spec"),
        Some("/api/remote/workspace/spec/jobs"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "spec-job-retry",
        "/api/workspaces/{workspace_id}/spec/jobs/{job_id}/retry",
        Post,
        Some("spec"),
        Some("/api/remote/workspace/spec/jobs/{job_id}/retry"),
        Sidecar,
        Required,
        Required,
        "502 when the broker cannot provide the LLM"
    ),
    route!(
        "spec-job-delete",
        "/api/workspaces/{workspace_id}/spec/jobs/{job_id}",
        Delete,
        Some("spec"),
        Some("/api/remote/workspace/spec/jobs/{job_id}"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-list",
        "/api/workspaces/{workspace_id}/plans",
        Get,
        Some("plans"),
        Some("/api/remote/workspace/plans"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-create",
        "/api/workspaces/{workspace_id}/plans",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-auto-run-read",
        "/api/workspaces/{workspace_id}/plans/auto-run",
        Get,
        Some("plans"),
        Some("/api/remote/workspace/plans/auto-run"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-auto-run-write",
        "/api/workspaces/{workspace_id}/plans/auto-run",
        Put,
        Some("plans"),
        Some("/api/remote/workspace/plans/auto-run"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-update",
        "/api/workspaces/{workspace_id}/plans/{plan_id}",
        Patch,
        Some("plans"),
        Some("/api/remote/workspace/plans/{plan_id}"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-delete",
        "/api/workspaces/{workspace_id}/plans/{plan_id}",
        Delete,
        Some("plans"),
        Some("/api/remote/workspace/plans/{plan_id}"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plan-action",
        "/api/workspaces/{workspace_id}/plans/{plan_id}/action",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans/{plan_id}/action"),
        Sidecar,
        Required,
        Required,
        "502 when the broker cannot provide the implementation LLM"
    ),
    route!(
        "plan-phase-retry",
        "/api/workspaces/{workspace_id}/plans/{plan_id}/phases/{phase_id}/retry",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans/{plan_id}/phases/{phase_id}/retry"),
        Sidecar,
        Required,
        Required,
        "502 when the broker cannot provide the implementation LLM"
    ),
    route!(
        "plan-step-action",
        "/api/workspaces/{workspace_id}/plans/{plan_id}/steps/{step_id}/action",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans/{plan_id}/steps/{step_id}/action"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-messages",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/messages",
        Get,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/messages"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-message-edit",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/messages/{message_id}/edit",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/messages/{message_id}/edit"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-statistics",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/statistics",
        Get,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/statistics"),
        Sidecar,
        Required,
        Required,
        "502 when brokered Global Memory aggregation is unavailable"
    ),
    route!(
        "chat-todo-graph",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/todo-graph",
        Get,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/todo-graph"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-delete",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/delete",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/delete"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-queue",
        "/api/workspaces/{workspace_id}/chat/queue",
        Post,
        Some("chat"),
        Some("/api/remote/workspace/chat/queue"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-stream",
        "/api/workspaces/{workspace_id}/chat/stream",
        Post,
        Some("chat"),
        Some("/api/remote/workspace/chat/stream"),
        Sidecar,
        Required,
        Required,
        "502 when the broker cannot provide the chat LLM"
    ),
    route!(
        "chat-run-stream",
        "/api/workspaces/{workspace_id}/chat/runs/{run_id}/stream",
        Get,
        Some("chat"),
        Some("/api/remote/workspace/chat/runs/{run_id}/stream"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-run-cancel",
        "/api/workspaces/{workspace_id}/chat/runs/{run_id}/cancel",
        Post,
        Some("chat"),
        Some("/api/remote/workspace/chat/runs/{run_id}/cancel"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "chat-guidance",
        "/api/workspaces/{workspace_id}/chat/guidance",
        Post,
        Some("chat"),
        Some("/api/remote/workspace/chat/guidance"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "context-usage",
        "/api/workspaces/{workspace_id}/context-usage",
        Post,
        Some("context-usage"),
        Some("/api/remote/workspace/context-usage"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-status",
        "/api/workspaces/{workspace_id}/git/status",
        Get,
        Some("git"),
        Some("/api/remote/workspace/git/status"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-diff",
        "/api/workspaces/{workspace_id}/git/diff",
        Get,
        Some("git"),
        Some("/api/remote/workspace/git/diff"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-stage",
        "/api/workspaces/{workspace_id}/git/stage",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/stage"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-unstage",
        "/api/workspaces/{workspace_id}/git/unstage",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/unstage"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-discard",
        "/api/workspaces/{workspace_id}/git/discard",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/discard"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-commit",
        "/api/workspaces/{workspace_id}/git/commit",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/commit"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-branches",
        "/api/workspaces/{workspace_id}/git/branches",
        Get,
        Some("git"),
        Some("/api/remote/workspace/git/branches"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-branch-switch",
        "/api/workspaces/{workspace_id}/git/branches/switch",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/branches/switch"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-branch-create",
        "/api/workspaces/{workspace_id}/git/branches/create",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/branches/create"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "git-commit-message",
        "/api/workspaces/{workspace_id}/git/commit-message",
        Post,
        Some("git"),
        Some("/api/remote/workspace/git/commit-message"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "terminal-session",
        "/api/workspaces/{workspace_id}/terminal/session",
        Post,
        Some("terminal"),
        Some("/api/remote/workspace/terminal/session"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "terminal-websocket",
        "/api/workspaces/{workspace_id}/terminal/{session_id}/ws",
        WebSocket,
        Some("terminal"),
        Some("/api/remote/workspace/terminal/{session_id}/ws"),
        Sidecar,
        Required,
        None,
        "websocket upgrade fails when the remote sidecar is offline"
    ),
    route!(
        "workspace-delete",
        "/api/workspaces/{workspace_id}",
        Delete,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "removes the local registration and closes any remote session",
        "Workspace registration is global configuration owned by the main process."
    ),
    route!(
        "workspace-logo-read",
        "/api/workspaces/{workspace_id}/logo",
        Get,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "uses the local workspace registration asset store",
        "Workspace logos are configuration assets, not remote workspace files."
    ),
    route!(
        "workspace-logo-write",
        "/api/workspaces/{workspace_id}/logo",
        Post,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "uses the local workspace registration asset store",
        "Workspace logos are configuration assets, not remote workspace files."
    ),
    route!(
        "workspace-logo-delete",
        "/api/workspaces/{workspace_id}/logo",
        Delete,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "uses the local workspace registration asset store",
        "Workspace logos are configuration assets, not remote workspace files."
    ),
    route!(
        "workspace-logo-thumbnail",
        "/api/workspaces/{workspace_id}/logo/thumbnail",
        Get,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "uses the local workspace registration asset store",
        "Workspace logos are configuration assets, not remote workspace files."
    ),
    route!(
        "scheduled-tasks-create",
        "/api/workspaces/{workspace_id}/scheduled-tasks",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-read",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}",
        Get,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-delete",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}",
        Delete,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-pause",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/pause",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-resume",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/resume",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-archive",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/archive",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-duplicate",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/duplicate",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-run-now",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/run-now",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-runs",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/runs",
        Get,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-run-read",
        "/api/workspaces/{workspace_id}/scheduled-task-runs/{scheduled_run_id}",
        Get,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "plans-order",
        "/api/workspaces/{workspace_id}/plans/order",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans/order"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-worktrees-audit",
        "/api/workspaces/{workspace_id}/plans/worktrees/audit",
        Get,
        Some("plans"),
        Some("/api/remote/workspace/plans/worktrees/audit"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "plans-worktrees-cleanup",
        "/api/workspaces/{workspace_id}/plans/worktrees/cleanup",
        Post,
        Some("plans"),
        Some("/api/remote/workspace/plans/worktrees/cleanup"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "agent-team-enable",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/enable",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/agent-team/enable"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
    route!(
        "agent-team-snapshot",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team",
        Get,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/agent-team"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
    route!(
        "agent-team-instances-create",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/instances/create",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/agent-team/instances/create"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
    route!(
        "preview-session-create",
        "/api/workspaces/{workspace_id}/preview/sessions",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: HTML preview sessions are available only for local workspaces",
        "Preview sessions depend on the main-process local preview registry."
    ),
    route!(
        "preview-session-delete",
        "/api/workspaces/{workspace_id}/preview/sessions/{token}",
        Delete,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: HTML preview sessions are available only for local workspaces",
        "Preview sessions depend on the main-process local preview registry."
    ),
    route!(
        "ai-statistics-detail",
        "/api/workspaces/{workspace_id}/ai-statistics/{request_id}",
        Get,
        None,
        None,
        MainProcess,
        MainProcessAuthority,
        None,
        "detail is unavailable when the main-process audit mirror has no v1 wire",
        "Real provider wire is retained only in the main-process remote audit mirror."
    ),
    route!(
        "scheduled-tasks",
        "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}",
        Patch,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "scheduled-task-run-cancel",
        "/api/workspaces/{workspace_id}/scheduled-task-runs/{scheduled_run_id}/cancel",
        Post,
        None,
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "agent-instance-transcript",
        "/api/workspaces/{workspace_id}/agent-team/instances/{instance_id}/transcript",
        Get,
        Some("agent-team"),
        Some("/api/remote/workspace/agent-team/instances/{instance_id}/transcript"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
    route!(
        "agent-task-action",
        "/api/workspaces/{workspace_id}/agent-tasks/{task_id}/action",
        Post,
        Some("agent-tasks"),
        Some("/api/remote/workspace/agent-tasks/{task_id}/action"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
    route!(
        "workspace-hook-runs",
        "/api/workspaces/{workspace_id}/hooks/runs",
        Get,
        Some("hooks"),
        Some("/api/remote/workspace/hooks/runs"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "workspace-hook-run-detail",
        "/api/workspaces/{workspace_id}/hooks/runs/{hook_run_id}",
        Get,
        Some("hooks"),
        Some("/api/remote/workspace/hooks/runs/{hook_run_id}"),
        Sidecar,
        Required,
        None,
        "503 when the remote sidecar is offline"
    ),
    route!(
        "agent-team-runtime",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/action",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/agent-team/action"),
        Sidecar,
        ControlPlaneOnly,
        None,
        "502 when the remote sidecar is disconnected",
        "Remote Agent Team control-plane state is durable, but worker scheduling, wait-resume, collaboration tools, and cross-attempt SSE are not yet local-equivalent."
    ),
];

/// Where a legacy global browser API carries the workspace id used for sidecar routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalWorkspaceIdSource {
    Query,
    JsonBody,
}

/// Browser-facing workspace APIs that predate `/api/workspaces/{workspace_id}/…`.
///
/// Keeping these in the same policy module as path-scoped routes makes their
/// remote ownership auditable instead of leaving a second handwritten router map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlobalWorkspaceRouteContract {
    pub(crate) id: &'static str,
    pub(crate) browser_path: &'static str,
    pub(crate) method: WorkspaceRouteMethod,
    pub(crate) sidecar_suffix: &'static str,
    pub(crate) workspace_id_source: GlobalWorkspaceIdSource,
    pub(crate) alignment: RemoteRouteAlignment,
    /// Why the request remains safely proxied but is not yet an implemented sidecar capability.
    pub(crate) exception: Option<&'static str>,
    /// Global Memory is main-process owned even though workspace/chat Memory is remote.
    pub(crate) global_memory_scope_stays_local: bool,
}

macro_rules! global_workspace_route {
    ($id:literal, $path:literal, $method:ident, $suffix:literal, $source:ident, $global_memory:expr $(, $gap:literal)?) => {
        GlobalWorkspaceRouteContract {
            id: $id,
            browser_path: $path,
            method: WorkspaceRouteMethod::$method,
            sidecar_suffix: $suffix,
            workspace_id_source: GlobalWorkspaceIdSource::$source,
            alignment: global_workspace_route!(@alignment $($gap)?),
            exception: global_workspace_route!(@exception $($gap)?),
            global_memory_scope_stays_local: $global_memory,
        }
    };
    (@alignment $gap:literal) => { RemoteRouteAlignment::KnownGap };
    (@alignment) => { RemoteRouteAlignment::Required };
    (@exception $gap:literal) => { Some($gap) };
    (@exception) => { None };
}

pub(crate) const GLOBAL_WORKSPACE_ROUTE_CONTRACTS: &[GlobalWorkspaceRouteContract] = &[
    global_workspace_route!(
        "hooks-settings-read",
        "/api/hooks",
        Get,
        "hooks/settings",
        Query,
        false
    ),
    global_workspace_route!("memory-list", "/api/memory", Get, "memory", Query, true),
    global_workspace_route!(
        "memory-sources",
        "/api/memory/sources",
        Get,
        "memory/sources",
        Query,
        true
    ),
    global_workspace_route!(
        "memory-dream-jobs",
        "/api/memory/dream/jobs",
        Get,
        "memory/dream/jobs",
        Query,
        true
    ),
    global_workspace_route!(
        "memory-dream-job",
        "/api/memory/dream/jobs/{job_id}",
        Get,
        "memory/dream/jobs/{job_id}",
        Query,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Dream job detail is implemented"
    ),
    global_workspace_route!(
        "memory-dream-job-changes",
        "/api/memory/dream/jobs/{job_id}/changes",
        Get,
        "memory/dream/jobs/{job_id}/changes",
        Query,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Dream job changes are implemented"
    ),
    global_workspace_route!(
        "hooks-settings-write",
        "/api/hooks/workspace",
        Post,
        "hooks/settings",
        JsonBody,
        false
    ),
    global_workspace_route!(
        "hooks-import-claude",
        "/api/hooks/import-claude",
        Post,
        "hooks/import-claude",
        JsonBody,
        false
    ),
    global_workspace_route!(
        "hooks-test",
        "/api/hooks/test",
        Post,
        "hooks/test",
        JsonBody,
        false
    ),
    global_workspace_route!(
        "memory-manual",
        "/api/memory/manual",
        Post,
        "memory/manual",
        JsonBody,
        true
    ),
    global_workspace_route!(
        "memory-status",
        "/api/memory/status",
        Post,
        "memory/status",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory status is implemented"
    ),
    global_workspace_route!(
        "memory-enabled",
        "/api/memory/enabled",
        Post,
        "memory/enabled",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory enablement is implemented"
    ),
    global_workspace_route!(
        "memory-edit",
        "/api/memory/edit",
        Post,
        "memory/edit",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory editing is implemented"
    ),
    global_workspace_route!(
        "memory-forget",
        "/api/memory/forget",
        Post,
        "memory/forget",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory deletion is implemented"
    ),
    global_workspace_route!(
        "memory-clear",
        "/api/memory/clear",
        Post,
        "memory/clear",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory clearing is implemented"
    ),
    global_workspace_route!(
        "memory-promote",
        "/api/memory/promote",
        Post,
        "memory/promote",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory promotion is implemented"
    ),
    global_workspace_route!(
        "memory-extraction-retry",
        "/api/memory/extraction/retry",
        Post,
        "memory/extraction/retry",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory extraction retry is implemented"
    ),
    global_workspace_route!(
        "memory-extraction-skip",
        "/api/memory/extraction/skip",
        Post,
        "memory/extraction/skip",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Memory extraction skip is implemented"
    ),
    global_workspace_route!(
        "memory-dream-run",
        "/api/memory/dream/run",
        Post,
        "memory/dream/run",
        JsonBody,
        true,
        "remote sidecar returns an explicit unsupported response until workspace/chat Dream execution is implemented"
    ),
];

#[cfg(test)]
pub(crate) const LEGACY_GLOBAL_WORKSPACE_ROUTE_EXCEPTIONS: &[(&str, WorkspaceRouteMethod, &str)] =
    &[(
        "/api/hooks/global",
        WorkspaceRouteMethod::Post,
        "global hook configuration is main-process owned and carries no workspace identity",
    )];

pub(crate) fn global_workspace_route_contract(
    path: &str,
    method: WorkspaceRouteMethod,
) -> Option<&'static GlobalWorkspaceRouteContract> {
    GLOBAL_WORKSPACE_ROUTE_CONTRACTS.iter().find(|contract| {
        contract.method == method && route_template_matches(contract.browser_path, path)
    })
}

pub(crate) fn global_workspace_sidecar_suffix(
    contract: &GlobalWorkspaceRouteContract,
    path: &str,
) -> Option<String> {
    if !route_template_matches(contract.browser_path, path) {
        return None;
    }

    let mut suffix = contract.sidecar_suffix.to_string();
    for (template_segment, path_segment) in contract.browser_path.split('/').zip(path.split('/')) {
        if template_segment.starts_with('{') && template_segment.ends_with('}') {
            suffix = suffix.replace(template_segment, path_segment);
        }
    }
    Some(suffix)
}

pub(crate) fn is_sidecar_workspace_route(path: &str, method: WorkspaceRouteMethod) -> bool {
    WORKSPACE_ROUTE_CONTRACTS.iter().any(|contract| {
        contract.authority.proxies_to_sidecar()
            && contract.method == method
            && route_template_matches(contract.browser_path, path)
    })
}

fn route_template_matches(template: &str, path: &str) -> bool {
    let template_segments = template.split('/');
    let path_segments = path.split('/');
    template_segments
        .zip(path_segments)
        .all(|(template, path)| {
            (!path.is_empty() && template.starts_with('{') && template.ends_with('}'))
                || template == path
        })
        && template.split('/').count() == path.split('/').count()
}

/// Prefixes retained by the proxy for non-browser compatibility paths.
/// Every proxy prefix must be represented by a browser route or one of these
/// explicit exceptions. Browser-facing prefixes are derived from the route
/// policy, so adding a sidecar-owned workspace route cannot silently diverge
/// from proxy authorization.
#[cfg(test)]
pub(crate) const REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS: &[(&str, &str)] = &[
    (
        "code-graph",
        "legacy/integration graph compatibility prefix; no browser-facing Axum route is registered",
    ),
    (
        "graph",
        "legacy/integration graph compatibility prefix; no browser-facing Axum route is registered",
    ),
];

#[cfg(test)]
pub(crate) fn remote_workspace_proxy_prefixes() -> impl Iterator<Item = &'static str> {
    WORKSPACE_ROUTE_CONTRACTS
        .iter()
        .filter(|contract| contract.authority.proxies_to_sidecar())
        .filter_map(|contract| contract.proxy_prefix)
        .chain(
            REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS
                .iter()
                .map(|(prefix, _)| *prefix),
        )
}

#[cfg(test)]
pub(crate) fn is_remote_workspace_proxy_prefix(prefix: &str) -> bool {
    remote_workspace_proxy_prefixes().any(|declared| declared == prefix)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        GLOBAL_WORKSPACE_ROUTE_CONTRACTS, LEGACY_GLOBAL_WORKSPACE_ROUTE_EXCEPTIONS,
        REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS, RemoteRouteAlignment, WORKSPACE_ROUTE_CONTRACTS,
        WorkspaceRouteMethod, is_remote_workspace_proxy_prefix, remote_workspace_proxy_prefixes,
    };

    fn route_body_registers_method(route_body: &str, method: WorkspaceRouteMethod) -> bool {
        let marker = method.sidecar_router_prefix();
        route_body.starts_with(marker) || route_body.contains(&format!(".{marker}"))
    }

    fn route_body_end(route_body: &str) -> usize {
        [route_body.find(".route("), route_body.find(".fallback(")]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(route_body.len())
    }

    fn router_registers_method(
        compact_source: &str,
        route_path: &str,
        method: WorkspaceRouteMethod,
    ) -> bool {
        let marker = format!(".route(\"{route_path}\",");
        let Some(route_start) = compact_source.find(&marker) else {
            return false;
        };
        let route_body = &compact_source[route_start + marker.len()..];
        let route_end = route_body_end(route_body);

        route_body_registers_method(&route_body[..route_end], method)
    }

    fn router_registers_method_or_catch_all(
        compact_source: &str,
        route_path: &str,
        method: WorkspaceRouteMethod,
    ) -> bool {
        router_registers_method(compact_source, route_path, method)
            || route_path.rsplit_once('/').is_some_and(|(parent, _)| {
                router_registers_method(compact_source, &format!("{parent}/{{*path}}"), method)
            })
    }

    fn local_workspace_router_methods(compact_source: &str) -> Vec<(&str, WorkspaceRouteMethod)> {
        const ROUTE_MARKER: &str = ".route(\"";
        let mut routes = Vec::new();
        let mut remaining = compact_source;

        while let Some(route_start) = remaining.find(ROUTE_MARKER) {
            let after_marker = &remaining[route_start + ROUTE_MARKER.len()..];
            let Some((path, remaining_after_path)) = after_marker.split_once("\",") else {
                break;
            };
            let route_end = route_body_end(remaining_after_path);
            let route_body = &remaining_after_path[..route_end];

            if path == "/api/workspaces/{workspace_id}"
                || path.starts_with("/api/workspaces/{workspace_id}/")
            {
                for method in [
                    WorkspaceRouteMethod::Get,
                    WorkspaceRouteMethod::Post,
                    WorkspaceRouteMethod::Put,
                    WorkspaceRouteMethod::Patch,
                    WorkspaceRouteMethod::Delete,
                ] {
                    if route_body_registers_method(route_body, method) {
                        routes.push((path, method));
                    }
                }
            }

            remaining = &remaining_after_path[route_end..];
        }

        routes
    }

    fn legacy_global_workspace_router_methods(
        compact_source: &str,
    ) -> Vec<(&str, WorkspaceRouteMethod)> {
        const ROUTE_MARKER: &str = ".route(\"";
        let mut routes = Vec::new();
        let mut remaining = compact_source;

        while let Some(route_start) = remaining.find(ROUTE_MARKER) {
            let after_marker = &remaining[route_start + ROUTE_MARKER.len()..];
            let Some((path, remaining_after_path)) = after_marker.split_once("\",") else {
                break;
            };
            let route_end = route_body_end(remaining_after_path);
            let route_body = &remaining_after_path[..route_end];

            if path == "/api/hooks"
                || path.starts_with("/api/hooks/")
                || path == "/api/memory"
                || path.starts_with("/api/memory/")
            {
                for method in [
                    WorkspaceRouteMethod::Get,
                    WorkspaceRouteMethod::Post,
                    WorkspaceRouteMethod::Put,
                    WorkspaceRouteMethod::Patch,
                    WorkspaceRouteMethod::Delete,
                ] {
                    if route_body_registers_method(route_body, method) {
                        routes.push((path, method));
                    }
                }
            }

            remaining = &remaining_after_path[route_end..];
        }

        routes
    }

    fn concrete_browser_path(template: &str) -> String {
        template
            .replace("{workspace_id}", "workspace-contract")
            .replace("{chat_id}", "chat-contract")
            .replace("{message_id}", "message-contract")
            .replace("{run_id}", "run-contract")
            .replace("{request_id}", "request-contract")
            .replace("{job_id}", "job-contract")
            .replace("{plan_id}", "plan-contract")
            .replace("{phase_id}", "phase-contract")
            .replace("{step_id}", "step-contract")
            .replace("{task_id}", "task-contract")
            .replace("{scheduled_run_id}", "scheduled-run-contract")
    }

    #[test]
    fn browser_workspace_routes_are_registered_by_the_local_router_with_the_same_method() {
        let local_router_source: String = include_str!("router.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for contract in WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.browser_path.starts_with("/api/workspaces/"))
        {
            assert!(
                router_registers_method(
                    &local_router_source,
                    contract.browser_path,
                    contract.method
                ),
                "local Router is missing {} for {} ({})",
                match contract.method {
                    WorkspaceRouteMethod::Get => "GET",
                    WorkspaceRouteMethod::Post => "POST",
                    WorkspaceRouteMethod::Put => "PUT",
                    WorkspaceRouteMethod::Patch => "PATCH",
                    WorkspaceRouteMethod::Delete => "DELETE",
                    WorkspaceRouteMethod::WebSocket => "WebSocket GET upgrade",
                },
                contract.browser_path,
                contract.id
            );
        }
    }

    #[test]
    fn legacy_workspace_routes_have_declared_local_and_sidecar_coverage() {
        let local_router_source: String = include_str!("router.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let sidecar_source: String = include_str!("../remote_workspace.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for contract in GLOBAL_WORKSPACE_ROUTE_CONTRACTS {
            assert!(
                router_registers_method(
                    &local_router_source,
                    contract.browser_path,
                    contract.method
                ),
                "local Router is missing legacy workspace route {} ({})",
                contract.browser_path,
                contract.id,
            );
            let sidecar_path = format!("/api/remote/workspace/{}", contract.sidecar_suffix);
            if contract.alignment.requires_sidecar_route() {
                assert!(
                    router_registers_method_or_catch_all(
                        &sidecar_source,
                        &sidecar_path,
                        contract.method,
                    ),
                    "sidecar Router is missing legacy workspace route {sidecar_path} ({})",
                    contract.id,
                );
            } else {
                assert!(
                    contract.exception.is_some(),
                    "legacy workspace route {} needs an explicit remote capability gap",
                    contract.id,
                );
            }
        }
    }

    #[test]
    fn legacy_hooks_and_memory_routes_declare_remote_policy_or_an_explicit_exception() {
        let local_router_source: String = include_str!("router.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for (path, method) in legacy_global_workspace_router_methods(&local_router_source) {
            let has_contract = GLOBAL_WORKSPACE_ROUTE_CONTRACTS
                .iter()
                .any(|contract| contract.browser_path == path && contract.method == method);
            let has_exception = LEGACY_GLOBAL_WORKSPACE_ROUTE_EXCEPTIONS.iter().any(
                |(exception_path, exception_method, _)| {
                    *exception_path == path && *exception_method == method
                },
            );
            assert!(
                has_contract || has_exception,
                "legacy workspace-aware route {} {} needs a remote policy declaration or explicit exception",
                match method {
                    WorkspaceRouteMethod::Get => "GET",
                    WorkspaceRouteMethod::Post => "POST",
                    WorkspaceRouteMethod::Put => "PUT",
                    WorkspaceRouteMethod::Patch => "PATCH",
                    WorkspaceRouteMethod::Delete => "DELETE",
                    WorkspaceRouteMethod::WebSocket => "WebSocket GET upgrade",
                },
                path,
            );
        }

        for (path, method, reason) in LEGACY_GLOBAL_WORKSPACE_ROUTE_EXCEPTIONS {
            assert!(
                !reason.is_empty(),
                "legacy workspace route exception {path} needs a reason"
            );
            assert!(
                router_registers_method(&local_router_source, path, *method),
                "legacy workspace route exception {path} is not registered by the local router"
            );
        }
    }

    #[test]
    fn every_local_workspace_router_method_declares_a_remote_execution_policy() {
        let local_router_source: String = include_str!("router.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for (path, method) in local_workspace_router_methods(&local_router_source) {
            let method = if path.ends_with("/ws") && method == WorkspaceRouteMethod::Get {
                WorkspaceRouteMethod::WebSocket
            } else {
                method
            };
            assert!(
                WORKSPACE_ROUTE_CONTRACTS
                    .iter()
                    .any(|contract| { contract.browser_path == path && contract.method == method }),
                "local workspace route {} {} is missing a remote execution policy declaration",
                match method {
                    WorkspaceRouteMethod::Get => "GET",
                    WorkspaceRouteMethod::Post => "POST",
                    WorkspaceRouteMethod::Put => "PUT",
                    WorkspaceRouteMethod::Patch => "PATCH",
                    WorkspaceRouteMethod::Delete => "DELETE",
                    WorkspaceRouteMethod::WebSocket => "WebSocket GET upgrade",
                },
                path
            );
        }
    }

    #[test]
    fn proxy_workspace_route_path_matches_the_declared_browser_inventory() {
        for contract in WORKSPACE_ROUTE_CONTRACTS {
            let browser_path = concrete_browser_path(contract.browser_path);
            let request_path = browser_path.split('?').next().unwrap_or(&browser_path);
            let proxied = crate::http::router::proxy_workspace_route_path_for_method(
                request_path,
                contract.method,
            );

            match contract.proxy_prefix {
                Some(prefix) => assert_eq!(
                    proxied.and_then(|suffix| suffix.split('/').next()),
                    Some(prefix),
                    "browser route {} must proxy through {prefix}",
                    contract.id
                ),
                None => assert!(
                    proxied.is_none(),
                    "browser route {} must remain on the main process",
                    contract.id
                ),
            }
        }
    }

    #[test]
    fn proxy_policy_requires_an_exact_declared_method_and_path() {
        let chats_path = "/api/workspaces/workspace-contract/chats";
        assert!(
            crate::http::router::proxy_workspace_route_path_for_method(
                chats_path,
                WorkspaceRouteMethod::Get,
            )
            .is_some()
        );
        assert!(
            crate::http::router::proxy_workspace_route_path_for_method(
                chats_path,
                WorkspaceRouteMethod::Post,
            )
            .is_none()
        );

        let agent_action =
            "/api/workspaces/workspace-contract/chats/chat-contract/agent-team/action";
        assert!(
            crate::http::router::proxy_workspace_route_path_for_method(
                agent_action,
                WorkspaceRouteMethod::Post,
            )
            .is_some()
        );
        assert!(
            crate::http::router::proxy_workspace_route_path_for_method(
                agent_action,
                WorkspaceRouteMethod::Get,
            )
            .is_none()
        );
    }

    #[test]
    fn sidecar_owned_workspace_routes_declare_proxy_prefixes() {
        for contract in WORKSPACE_ROUTE_CONTRACTS.iter().filter(|contract| {
            contract.browser_path.starts_with("/api/workspaces/")
                && contract.authority.proxies_to_sidecar()
        }) {
            let prefix = contract.proxy_prefix.unwrap_or_else(|| {
                panic!(
                    "sidecar-owned route {} is missing a proxy prefix",
                    contract.id
                )
            });
            assert!(
                is_remote_workspace_proxy_prefix(prefix),
                "sidecar-owned route {} uses undeclared proxy prefix {prefix}",
                contract.id
            );
        }
    }

    #[test]
    fn required_remote_routes_have_exact_proxy_and_sidecar_method_coverage() {
        let sidecar_source: String = include_str!("../remote_workspace.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for contract in WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.alignment.requires_sidecar_route())
        {
            let proxy_prefix = contract.proxy_prefix.unwrap_or_else(|| {
                panic!("required route {} is missing a proxy prefix", contract.id)
            });
            let sidecar_path = contract.sidecar_path.unwrap_or_else(|| {
                panic!("required route {} is missing a sidecar path", contract.id)
            });
            assert!(
                is_remote_workspace_proxy_prefix(proxy_prefix),
                "required route {} has an unproxied prefix {proxy_prefix}",
                contract.id
            );
            assert!(
                router_registers_method(&sidecar_source, sidecar_path, contract.method),
                "required route {} is missing exact {} registration for {sidecar_path}",
                contract.id,
                match contract.method {
                    WorkspaceRouteMethod::Get => "GET",
                    WorkspaceRouteMethod::Post => "POST",
                    WorkspaceRouteMethod::Put => "PUT",
                    WorkspaceRouteMethod::Patch => "PATCH",
                    WorkspaceRouteMethod::Delete => "DELETE",
                    WorkspaceRouteMethod::WebSocket => "WebSocket GET upgrade",
                }
            );
        }
    }

    #[test]
    fn every_proxy_prefix_has_a_browser_contract_or_an_explicit_compatibility_exception() {
        let contract_prefixes: HashSet<_> = WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.authority.proxies_to_sidecar())
            .filter_map(|contract| contract.proxy_prefix)
            .collect();
        let exception_prefixes: HashSet<_> = REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS
            .iter()
            .map(|(prefix, _)| *prefix)
            .collect();

        for prefix in remote_workspace_proxy_prefixes() {
            assert!(
                contract_prefixes.contains(prefix) || exception_prefixes.contains(prefix),
                "proxy prefix {prefix} is neither a browser route contract nor a declared compatibility exception"
            );
        }

        for (prefix, reason) in REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS {
            assert!(
                !reason.is_empty(),
                "proxy compatibility exception {prefix} needs a reason"
            );
        }
    }

    #[test]
    fn exceptions_are_explicit_and_do_not_look_like_remote_parity() {
        let exception_ids: HashSet<_> = WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.alignment != RemoteRouteAlignment::Required)
            .map(|contract| contract.id)
            .collect();

        for required in [
            "scheduled-tasks",
            "ai-statistics-detail",
            "agent-team-enable",
            "agent-team-runtime",
        ] {
            assert!(
                exception_ids.contains(required),
                "non-parity route {required} must remain explicitly documented"
            );
        }

        for contract in WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.alignment != RemoteRouteAlignment::Required)
        {
            assert!(
                contract.exception.is_some(),
                "exceptional route {} needs a durable explanation",
                contract.id
            );
        }
    }

    #[test]
    fn route_matrix_keeps_method_variants_distinct() {
        let mut keys = HashSet::new();
        let mut methods = HashSet::new();

        for contract in WORKSPACE_ROUTE_CONTRACTS {
            assert!(
                keys.insert((contract.browser_path, contract.method)),
                "duplicate browser route/method contract: {}",
                contract.id
            );
            methods.insert(contract.method);
        }

        assert!(methods.contains(&WorkspaceRouteMethod::Get));
        assert!(methods.contains(&WorkspaceRouteMethod::Post));
        assert!(methods.contains(&WorkspaceRouteMethod::Put));
        assert!(methods.contains(&WorkspaceRouteMethod::Patch));
        assert!(methods.contains(&WorkspaceRouteMethod::Delete));
        assert!(methods.contains(&WorkspaceRouteMethod::WebSocket));
    }
}
