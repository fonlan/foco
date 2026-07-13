//! Stable SSH error classification for the pure-Rust remote transport.
//!
//! Higher layers map these kinds without scraping English stderr from OpenSSH.

use std::fmt;
use std::path::PathBuf;

/// Stable error kinds shared with diagnostics (`authentication_failed`, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    AuthenticationFailed,
    HostUnreachable,
    StartupFailed,
    Timeout,
    HostKeyUnknown,
    HostKeyChanged,
    ProxyUnsupported,
    Config,
}

impl SshErrorKind {
    /// Wire / diagnostic string used by remote server diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticationFailed => "authentication_failed",
            Self::HostUnreachable => "host_unreachable",
            Self::StartupFailed => "startup_failed",
            Self::Timeout => "timeout",
            Self::HostKeyUnknown => "host_key_unknown",
            Self::HostKeyChanged => "host_key_changed",
            Self::ProxyUnsupported => "proxy_unsupported",
            Self::Config => "config_error",
        }
    }
}

impl fmt::Display for SshErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured host key payload when the server key is unknown or changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    /// OpenSSH-style `SHA256:...` fingerprint (no full key material).
    pub fingerprint_sha256: String,
    pub known_hosts_path: PathBuf,
}

/// SSH runtime error with stable kind and safe display message (no secrets).
#[derive(Debug, Clone)]
pub struct SshError {
    pub kind: SshErrorKind,
    message: String,
    pub host_key: Option<HostKeyInfo>,
}

impl SshError {
    pub fn new(kind: SshErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            host_key: None,
        }
    }

    pub fn with_host_key(
        kind: SshErrorKind,
        message: impl Into<String>,
        host_key: HostKeyInfo,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            host_key: Some(host_key),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn kind_str(&self) -> &'static str {
        self.kind.as_str()
    }
}

impl fmt::Display for SshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for SshError {}

impl From<std::io::Error> for SshError {
    fn from(value: std::io::Error) -> Self {
        let kind = match value.kind() {
            std::io::ErrorKind::TimedOut => SshErrorKind::Timeout,
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::HostUnreachable => SshErrorKind::HostUnreachable,
            _ => {
                let lower = value.to_string().to_ascii_lowercase();
                if lower.contains("timed out") || lower.contains("timeout") {
                    SshErrorKind::Timeout
                } else if lower.contains("name or service not known")
                    || lower.contains("nodename nor servname")
                    || lower.contains("failed to lookup")
                    || lower.contains("no such host")
                    || lower.contains("connection refused")
                    || lower.contains("network is unreachable")
                    || lower.contains("no route to host")
                {
                    SshErrorKind::HostUnreachable
                } else {
                    SshErrorKind::StartupFailed
                }
            }
        };
        SshError::new(kind, value.to_string())
    }
}

/// Map a russh library error into stable transport kinds (no secret content).
pub(crate) fn map_russh_error(err: russh::Error) -> SshError {
    use russh::Error as E;
    match &err {
        E::UnknownKey => SshError::new(
            SshErrorKind::HostKeyUnknown,
            "server host key was rejected by the host-key policy",
        ),
        E::Disconnect => {
            SshError::new(SshErrorKind::HostUnreachable, "ssh connection disconnected")
        }
        other => {
            let text = other.to_string();
            let lower = text.to_ascii_lowercase();
            if lower.contains("auth") || lower.contains("permission denied") {
                SshError::new(SshErrorKind::AuthenticationFailed, text)
            } else if lower.contains("timeout") || lower.contains("timed out") {
                SshError::new(SshErrorKind::Timeout, text)
            } else if lower.contains("connect")
                || lower.contains("resolve")
                || lower.contains("unreachable")
                || lower.contains("refused")
            {
                SshError::new(SshErrorKind::HostUnreachable, text)
            } else {
                SshError::new(SshErrorKind::StartupFailed, text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_strings_match_diagnostics_contract() {
        assert_eq!(
            SshErrorKind::AuthenticationFailed.as_str(),
            "authentication_failed"
        );
        assert_eq!(SshErrorKind::HostUnreachable.as_str(), "host_unreachable");
        assert_eq!(SshErrorKind::StartupFailed.as_str(), "startup_failed");
        assert_eq!(SshErrorKind::Timeout.as_str(), "timeout");
        assert_eq!(SshErrorKind::HostKeyChanged.as_str(), "host_key_changed");
    }

    #[test]
    fn display_does_not_require_host_key_payload() {
        let err = SshError::new(SshErrorKind::Timeout, "connect timed out");
        assert!(err.to_string().contains("timeout"));
        assert!(err.host_key.is_none());
    }
}
