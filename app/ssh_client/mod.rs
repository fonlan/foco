//! Pure-Rust SSH transport for Foco remote servers.
//!
//! This module is the single runtime source of truth for SSH connect, auth,
//! command execution, host-key trust, and stable error classification.
//! HTTP handlers and `RemoteWorkspaceManager` should depend on these types
//! rather than `russh` events or system `ssh`/`scp` processes.

// Public surface is consumed by later remote-server phases; keep re-exports stable.
#![allow(dead_code, unused_imports)]

mod config;
mod error;
mod known_hosts;
mod session;

pub use config::{
    ResolveSshOptions, ResolvedSshProfile, SshAuthConfig, StrictHostKeyChecking,
    resolve_ssh_profile,
};
pub use error::{HostKeyInfo, SshError, SshErrorKind};
pub use known_hosts::{
    HostKeyStatus, host_key_info, trust_host_key_if_fingerprint_matches, verify_server_key,
    verify_server_key_path,
};
pub use session::{
    SshCommandResult, SshSession, SshSpawnedExec, connect_with_optional_trust, trust_and_connect,
};
