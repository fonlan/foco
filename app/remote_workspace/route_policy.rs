//! Execution-host policy for browser-facing workspace routes.
//!
//! This module is deliberately transport-neutral. The HTTP contract owns route
//! inventory, while proxy middleware asks these policy methods whether an SSH
//! workspace may be forwarded to a sidecar. Keeping that decision here prevents
//! a separate proxy-prefix allowlist from drifting away from the declared route.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteRouteAuthority {
    /// The remote sidecar owns the workspace path and its SQLite data.
    Sidecar,
    /// The main process owns the data or capability even for an SSH workspace.
    MainProcess,
    /// The API intentionally does not support SSH workspaces.
    LocalOnly,
}

impl RemoteRouteAuthority {
    /// Whether a browser request for an SSH workspace must be sent to its sidecar.
    pub(crate) const fn proxies_to_sidecar(self) -> bool {
        matches!(self, Self::Sidecar)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteRouteAlignment {
    /// Local, proxy, and sidecar implementations must all exist.
    Required,
    /// The route has a durable remote control plane, but its worker runtime is
    /// intentionally not yet a full local-equivalent implementation.
    ControlPlaneOnly,
    /// The route is intentionally main-process owned rather than proxied.
    MainProcessAuthority,
    /// The API is intentionally unavailable for SSH workspaces.
    LocalOnly,
    /// The browser route is safely proxied but the sidecar returns an explicit
    /// unsupported response until the domain implementation is added.
    KnownGap,
}

impl RemoteRouteAlignment {
    /// Whether the declared route must have an exact sidecar registration.
    #[cfg(test)]
    pub(crate) const fn requires_sidecar_route(self) -> bool {
        matches!(self, Self::Required | Self::ControlPlaneOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrokerRequirement {
    None,
    Required,
}
