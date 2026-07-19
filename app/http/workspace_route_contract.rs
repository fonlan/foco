//! Machine-checkable remote-workspace HTTP contract.
//!
//! The browser keeps using the local workspace URL shape for SSH workspaces.
//! This inventory records where that request must execute and which differences
//! are intentional. It is deliberately separate from provider configuration:
//! provider secrets and real LLM wire remain on the main process, while
//! workspace/chat data remains in the remote workspace database.

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
pub(crate) enum RemoteRouteAuthority {
    /// The remote sidecar owns the workspace path and its SQLite data.
    Sidecar,
    /// The main process owns the data or capability even for an SSH workspace.
    MainProcess,
    /// The API intentionally does not support SSH workspaces.
    LocalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteRouteAlignment {
    /// Local, proxy, and sidecar implementations must all exist.
    Required,
    /// The route is intentionally main-process owned rather than proxied.
    MainProcessAuthority,
    /// The API is intentionally unavailable for SSH workspaces.
    LocalOnly,
    /// A known missing remote path, retained as an explicit Phase 1 baseline.
    KnownGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrokerRequirement {
    None,
    Required,
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
        Some("scheduled-tasks"),
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
        Some("scheduled-task-runs"),
        None,
        LocalOnly,
        LocalOnly,
        None,
        "400: scheduled tasks are not available for remote workspaces",
        "Scheduled tasks intentionally remain local-workspace only."
    ),
    route!(
        "workspace-hooks-settings",
        "/api/hooks?workspaceId={workspace_id}",
        Get,
        None,
        Some("/api/remote/workspace/hooks/settings"),
        Sidecar,
        KnownGap,
        None,
        "currently opens the main-process workspace database",
        "The browser path is global/query-scoped, so proxy middleware cannot select the remote sidecar."
    ),
    route!(
        "workspace-hooks-save",
        "/api/hooks/workspace",
        Post,
        None,
        Some("/api/remote/workspace/hooks/settings"),
        Sidecar,
        KnownGap,
        None,
        "currently writes through the main-process workspace database",
        "The request carries workspaceId in its body rather than its path."
    ),
    route!(
        "workspace-memory-list",
        "/api/memory?scope=workspace|chat&workspaceId={workspace_id}",
        Get,
        None,
        Some("/api/remote/workspace/memory"),
        Sidecar,
        KnownGap,
        None,
        "currently reads the main-process workspace database",
        "Workspace/chat Memory must be remote SQLite; Global Memory remains main-process owned."
    ),
    route!(
        "workspace-memory-mutations",
        "/api/memory/{manual|status|enabled|edit|forget|clear|promote}",
        Post,
        None,
        None,
        Sidecar,
        KnownGap,
        None,
        "currently mutates the main-process workspace database",
        "The global endpoint shape prevents proxy routing and sidecar only exposes list/manual today."
    ),
    route!(
        "agent-instance-transcript",
        "/api/workspaces/{workspace_id}/agent-team/instances/{instance_id}/transcript",
        Get,
        Some("agent-team"),
        Some("/api/remote/workspace/agent-team/instances/{instance_id}/transcript"),
        Sidecar,
        KnownGap,
        None,
        "501 while remote multi-agent runtime is not implemented",
        "The sidecar wildcard intentionally returns explicit unavailable responses; it must not fall back locally."
    ),
    route!(
        "agent-task-action",
        "/api/workspaces/{workspace_id}/agent-tasks/{task_id}/action",
        Post,
        Some("agent-tasks"),
        Some("/api/remote/workspace/agent-tasks/{task_id}/action"),
        Sidecar,
        KnownGap,
        None,
        "501 while remote multi-agent runtime is not implemented",
        "The sidecar wildcard intentionally returns explicit unavailable responses; it must not fall back locally."
    ),
    route!(
        "workspace-hook-runs",
        "/api/workspaces/{workspace_id}/hooks/runs",
        Get,
        Some("hooks"),
        Some("/api/remote/workspace/hooks/runs"),
        Sidecar,
        KnownGap,
        None,
        "currently reads the main-process workspace database",
        "Hook run history is workspace-scoped but its browser route is not in the remote proxy allowlist."
    ),
    route!(
        "workspace-hook-run-detail",
        "/api/workspaces/{workspace_id}/hooks/runs/{hook_run_id}",
        Get,
        Some("hooks"),
        Some("/api/remote/workspace/hooks/runs/{hook_run_id}"),
        Sidecar,
        KnownGap,
        None,
        "currently reads the main-process workspace database",
        "Hook run detail has the same missing proxy boundary as Hook run history."
    ),
    route!(
        "agent-team-runtime",
        "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/action",
        Post,
        Some("chats"),
        Some("/api/remote/workspace/chats/{chat_id}/agent-team/action"),
        Sidecar,
        KnownGap,
        None,
        "501 while remote multi-agent runtime is not implemented",
        "The sidecar intentionally returns explicit unavailable responses; it must not fall back locally."
    ),
];

/// Proxy prefixes actually consumed by [`crate::http::router::proxy_workspace_route_path`].
///
/// This is kept separate from the route matrix because the proxy rewrites a path
/// family, while the sidecar registers individual method/path pairs.
pub(crate) const REMOTE_WORKSPACE_PROXY_PREFIXES: &[&str] = &[
    "files",
    "git",
    "terminal",
    "spec",
    "plans",
    "code-graph",
    "graph",
    "chats",
    "chat",
    "context-usage",
    "hooks",
    "agent-team",
    "agent-tasks",
    "scheduled-tasks",
    "scheduled-task-runs",
];

/// Prefixes retained by the proxy for non-browser compatibility paths.
/// Every proxy prefix must be represented by a browser route or one of these
/// explicit exceptions, so the proxy allowlist cannot drift silently.
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

pub(crate) fn is_remote_workspace_proxy_prefix(prefix: &str) -> bool {
    REMOTE_WORKSPACE_PROXY_PREFIXES.contains(&prefix)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS, REMOTE_WORKSPACE_PROXY_PREFIXES,
        RemoteRouteAlignment, WORKSPACE_ROUTE_CONTRACTS, WorkspaceRouteMethod,
        is_remote_workspace_proxy_prefix,
    };

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
        let route_end = route_body.find(".route(").unwrap_or(route_body.len());

        route_body[..route_end].contains(method.sidecar_router_prefix())
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
    fn proxy_workspace_route_path_matches_the_declared_browser_inventory() {
        for contract in WORKSPACE_ROUTE_CONTRACTS {
            let browser_path = concrete_browser_path(contract.browser_path);
            let request_path = browser_path.split('?').next().unwrap_or(&browser_path);
            let proxied = crate::http::router::proxy_workspace_route_path(request_path);

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
    fn required_remote_routes_have_exact_proxy_and_sidecar_method_coverage() {
        let sidecar_source: String = include_str!("../remote_workspace.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        for contract in WORKSPACE_ROUTE_CONTRACTS
            .iter()
            .filter(|contract| contract.alignment == RemoteRouteAlignment::Required)
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
            .filter_map(|contract| contract.proxy_prefix)
            .collect();
        let exception_prefixes: HashSet<_> = REMOTE_WORKSPACE_PROXY_PREFIX_EXCEPTIONS
            .iter()
            .map(|(prefix, _)| *prefix)
            .collect();

        for prefix in REMOTE_WORKSPACE_PROXY_PREFIXES {
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
            "workspace-hooks-settings",
            "workspace-hooks-save",
            "workspace-memory-list",
            "workspace-memory-mutations",
            "scheduled-tasks",
            "ai-statistics-detail",
        ] {
            assert!(
                exception_ids.contains(required),
                "Phase 1 exception {required} must remain declared until its route is aligned"
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
