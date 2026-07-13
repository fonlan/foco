//! Pure-Rust SSH transport for Foco remote servers.
//!
//! This module is the single runtime source of truth for SSH connect, auth,
//! command execution, host-key trust, and stable error classification.
//! HTTP handlers and `RemoteWorkspaceManager` should depend on these types
//! rather than `russh` events or system `ssh`/`scp` processes.

mod config;
mod error;
mod known_hosts;
mod remote_path;
mod session;

// Re-exports form the stable remote SSH API; some are reserved for Phase 4 UI trust flows.
#[allow(unused_imports)]
pub use config::{
    ResolveSshOptions, ResolvedSshProfile, SshAuthConfig, StrictHostKeyChecking,
    resolve_ssh_profile,
};
#[allow(unused_imports)]
pub use error::{HostKeyInfo, SshError, SshErrorKind};
#[allow(unused_imports)]
pub use known_hosts::{
    HostKeyStatus, host_key_info, trust_host_key_if_fingerprint_matches, verify_server_key,
    verify_server_key_path,
};
#[allow(unused_imports)]
pub use remote_path::{
    expand_command, expand_remote_path, shell_quote, validate_remote_path_input,
};
#[allow(unused_imports)]
pub use session::{
    SshCommandResult, SshSession, SshSpawnedExec, connect_with_optional_trust, trust_and_connect,
};
