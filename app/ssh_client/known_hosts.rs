//! OpenSSH-compatible known_hosts verification and explicit trust writes.

use std::path::Path;

use russh::keys::{self, PublicKeyBase64};
use tracing::warn;

use super::config::{ResolvedSshProfile, StrictHostKeyChecking};
use super::error::{HostKeyInfo, SshError, SshErrorKind};

/// Outcome of comparing the live server key with known_hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    /// Known host entry matches the presented key.
    Trusted,
    /// No matching host entry; caller must confirm before learning.
    Unknown { info: HostKeyInfo },
    /// Same host/port has a different key recorded (TOFU conflict).
    Changed { info: HostKeyInfo },
}

/// Build SHA-256 fingerprint display and algorithm label for a server key.
pub fn host_key_info(
    host: &str,
    port: u16,
    server_key: &russh::keys::PublicKey,
    known_hosts_path: &Path,
) -> HostKeyInfo {
    let fingerprint = server_key.fingerprint(russh::keys::HashAlg::Sha256);
    HostKeyInfo {
        host: host.to_string(),
        port,
        algorithm: server_key.algorithm().to_string(),
        fingerprint_sha256: fingerprint.to_string(),
        known_hosts_path: known_hosts_path.to_path_buf(),
    }
}

/// Check `server_key` against the profile known_hosts file.
pub fn verify_server_key(
    profile: &ResolvedSshProfile,
    server_key: &russh::keys::PublicKey,
) -> Result<HostKeyStatus, SshError> {
    verify_server_key_path(
        &profile.hostname,
        profile.port,
        server_key,
        &profile.known_hosts_path,
        profile.strict_host_key_checking,
    )
}

pub fn verify_server_key_path(
    host: &str,
    port: u16,
    server_key: &russh::keys::PublicKey,
    known_hosts_path: &Path,
    policy: StrictHostKeyChecking,
) -> Result<HostKeyStatus, SshError> {
    let info = host_key_info(host, port, server_key, known_hosts_path);
    match keys::check_known_hosts_path(host, port, server_key, known_hosts_path) {
        Ok(true) => Ok(HostKeyStatus::Trusted),
        Ok(false) => match policy {
            StrictHostKeyChecking::No => {
                warn!(
                  host = %host,
                  port = port,
                  algorithm = %info.algorithm,
                  fingerprint = %info.fingerprint_sha256,
                  "StrictHostKeyChecking=no: auto-accepting unknown SSH host key"
                );
                // Learn immediately so subsequent connections stay consistent.
                learn_known_hosts_path(host, port, server_key, known_hosts_path)?;
                Ok(HostKeyStatus::Trusted)
            }
            StrictHostKeyChecking::Yes | StrictHostKeyChecking::Ask => {
                Ok(HostKeyStatus::Unknown { info })
            }
        },
        Err(keys::Error::KeyChanged { line }) => {
            let message = format!(
                "REMOTE HOST IDENTIFICATION HAS CHANGED for {host}:{port} (known_hosts line {line}). Refusing to connect; remove the stale entry only after you verify the new key out-of-band"
            );
            Err(SshError::with_host_key(
                SshErrorKind::HostKeyChanged,
                message,
                info,
            ))
        }
        Err(source) => Err(SshError::new(
            SshErrorKind::StartupFailed,
            format!("failed to check known_hosts: {source}"),
        )),
    }
}

/// Append a host key to known_hosts only when the live key still matches
/// `expected_fingerprint_sha256` (prevents TOCTOU swap between confirm and write).
pub fn trust_host_key_if_fingerprint_matches(
    host: &str,
    port: u16,
    server_key: &russh::keys::PublicKey,
    known_hosts_path: &Path,
    expected_fingerprint_sha256: &str,
) -> Result<(), SshError> {
    let info = host_key_info(host, port, server_key, known_hosts_path);
    if !fingerprints_equal(&info.fingerprint_sha256, expected_fingerprint_sha256) {
        return Err(SshError::with_host_key(
            SshErrorKind::HostKeyChanged,
            format!(
                "host key fingerprint changed before trust write: expected {}, got {}",
                expected_fingerprint_sha256, info.fingerprint_sha256
            ),
            info,
        ));
    }

    // Never overwrite a changed key via the normal trust path.
    match keys::check_known_hosts_path(host, port, server_key, known_hosts_path) {
        Ok(true) => Ok(()),
        Ok(false) => learn_known_hosts_path(host, port, server_key, known_hosts_path),
        Err(keys::Error::KeyChanged { line }) => Err(SshError::with_host_key(
            SshErrorKind::HostKeyChanged,
            format!(
                "host key for {host}:{port} already differs in known_hosts (line {line}); refusing automatic overwrite"
            ),
            info,
        )),
        Err(source) => Err(SshError::new(
            SshErrorKind::StartupFailed,
            format!("failed to check known_hosts before trust write: {source}"),
        )),
    }
}

fn learn_known_hosts_path(
    host: &str,
    port: u16,
    server_key: &russh::keys::PublicKey,
    known_hosts_path: &Path,
) -> Result<(), SshError> {
    keys::known_hosts::learn_known_hosts_path(host, port, server_key, known_hosts_path).map_err(
        |source| {
            SshError::new(
                SshErrorKind::StartupFailed,
                format!(
                    "failed to write known_hosts {}: {source}",
                    known_hosts_path.display()
                ),
            )
        },
    )
}

fn fingerprints_equal(left: &str, right: &str) -> bool {
    normalize_fingerprint(left) == normalize_fingerprint(right)
}

fn normalize_fingerprint(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Encode a public key to OpenSSH single-line form for tests/debug (not logged by default).
#[cfg(test)]
pub(crate) fn public_key_openssh_line(key: &russh::keys::PublicKey) -> String {
    format!("{} {}", key.algorithm(), key.public_key_base64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    use russh::keys::{PublicKey, parse_public_key_base64};

    fn sample_key() -> PublicKey {
        parse_public_key_base64(
            "AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ",
        )
        .expect("key")
    }

    fn other_key() -> PublicKey {
        parse_public_key_base64(
            "AAAAC3NzaC1lZDI1NTE5AAAAIA6rWI3G1sz07DnfFlrouTcysQlj2P+jpNSOEWD9OJ3X",
        )
        .expect("key")
    }

    #[test]
    fn trusted_when_known_hosts_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known_hosts");
        let key = sample_key();
        let mut file = fs::File::create(&path).expect("create");
        writeln!(file, "[localhost]:13265 {}", public_key_openssh_line(&key)).expect("write");

        let status =
            verify_server_key_path("localhost", 13265, &key, &path, StrictHostKeyChecking::Ask)
                .expect("verify");
        assert_eq!(status, HostKeyStatus::Trusted);
    }

    #[test]
    fn unknown_key_returns_structured_fingerprint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known_hosts");
        let key = sample_key();
        let status =
            verify_server_key_path("example.com", 22, &key, &path, StrictHostKeyChecking::Ask)
                .expect("verify");
        match status {
            HostKeyStatus::Unknown { info } => {
                assert_eq!(info.host, "example.com");
                assert_eq!(info.port, 22);
                assert!(
                    info.fingerprint_sha256
                        .to_ascii_lowercase()
                        .contains("sha256")
                );
                assert!(!info.algorithm.is_empty());
            }
            other => panic!("expected unknown, got {other:?}"),
        }
    }

    #[test]
    fn changed_key_hard_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known_hosts");
        let known = sample_key();
        let mut file = fs::File::create(&path).expect("create");
        writeln!(file, "evil.example {}", public_key_openssh_line(&known)).expect("write");

        let err = verify_server_key_path(
            "evil.example",
            22,
            &other_key(),
            &path,
            StrictHostKeyChecking::Ask,
        )
        .expect_err("changed");
        assert_eq!(err.kind, SshErrorKind::HostKeyChanged);
        assert!(err.host_key.is_some());
        assert!(err.message().contains("IDENTIFICATION HAS CHANGED"));
    }

    #[test]
    fn trust_write_requires_matching_fingerprint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known_hosts");
        let key = sample_key();
        let info = host_key_info("new.example", 22, &key, &path);

        trust_host_key_if_fingerprint_matches(
            "new.example",
            22,
            &key,
            &path,
            &info.fingerprint_sha256,
        )
        .expect("trust");

        assert!(keys::check_known_hosts_path("new.example", 22, &key, &path).expect("check"));

        let err = trust_host_key_if_fingerprint_matches(
            "new.example",
            22,
            &other_key(),
            &path,
            &info.fingerprint_sha256,
        )
        .expect_err("mismatch");
        assert_eq!(err.kind, SshErrorKind::HostKeyChanged);
    }

    #[test]
    fn non_default_port_uses_bracket_form() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known_hosts");
        let key = sample_key();
        let info = host_key_info("localhost", 2222, &key, &path);
        trust_host_key_if_fingerprint_matches(
            "localhost",
            2222,
            &key,
            &path,
            &info.fingerprint_sha256,
        )
        .expect("trust");
        let text = fs::read_to_string(&path).expect("read");
        assert!(text.contains("[localhost]:2222"));
    }
}
