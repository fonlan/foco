//! Resolve `RemoteServerProfile` + optional `~/.ssh/config` into a connection profile.
//!
//! `host_alias` remains the stored field name (historical JSON) but means connection
//! target: hostname, IP, or an OpenSSH config Host alias.

use std::path::{Path, PathBuf};
use std::time::Duration;

use foco_store::config::{RemoteAuthMethod, RemoteServerProfile};

use super::error::{SshError, SshErrorKind};

const DEFAULT_SSH_PORT: u16 = 22;

/// How unknown host keys are handled (OpenSSH StrictHostKeyChecking subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrictHostKeyChecking {
    /// Prompt / structured error until the user confirms (default).
    Ask,
    /// Reject unknown keys without confirmation path in-band.
    Yes,
    /// Automatically accept unknown keys (logs a security warning). Only when
    /// OpenSSH config sets `StrictHostKeyChecking no`.
    No,
}

/// Authentication material for a resolved profile.
///
/// Password content is never included in `Debug` output.
#[derive(Clone)]
pub struct SshAuthConfig {
    pub method: RemoteAuthMethod,
    /// Ordered private key paths (profile IdentityFile overrides config list).
    pub identity_files: Vec<PathBuf>,
    /// Optional password (sensitive; not logged).
    pub password: Option<String>,
    /// Attempt SSH agent when key auth is in use.
    pub use_agent: bool,
    /// When true, also try common `~/.ssh/id_*` after configured identity files.
    pub try_default_identities: bool,
}

impl std::fmt::Debug for SshAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshAuthConfig")
            .field("method", &self.method)
            .field("identity_files", &self.identity_files)
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .field("use_agent", &self.use_agent)
            .field("try_default_identities", &self.try_default_identities)
            .finish()
    }
}

/// Fully resolved SSH endpoint + trust/auth settings.
#[derive(Debug, Clone)]
pub struct ResolvedSshProfile {
    /// Original connection target (`hostAlias` storage field).
    pub host_alias: String,
    /// TCP connect host after HostName resolution.
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub connect_timeout: Duration,
    pub known_hosts_path: PathBuf,
    pub strict_host_key_checking: StrictHostKeyChecking,
    pub auth: SshAuthConfig,
}

/// Optional overrides applied when resolving (e.g. password not yet on profile).
#[derive(Debug, Clone, Default)]
pub struct ResolveSshOptions {
    /// Password supplied by the caller (overrides profile password when set).
    pub password: Option<String>,
    /// Prefer agent authentication when true (default true for key auth).
    pub use_agent: Option<bool>,
    /// Custom path to `ssh_config` for tests; `None` uses `~/.ssh/config`.
    pub ssh_config_path: Option<PathBuf>,
    /// When true, skip loading OpenSSH config entirely (direct hostname/IP only).
    pub skip_ssh_config: bool,
}

/// Merge `RemoteServerProfile` with OpenSSH config. Profile fields win over config.
pub fn resolve_ssh_profile(
    server: &RemoteServerProfile,
    options: ResolveSshOptions,
) -> Result<ResolvedSshProfile, SshError> {
    let host_alias = server.host_alias.trim();
    if host_alias.is_empty() {
        return Err(SshError::new(
            SshErrorKind::Config,
            "remote server host alias must not be empty",
        ));
    }

    let file_config = if options.skip_ssh_config {
        None
    } else {
        Some(load_ssh_config(
            host_alias,
            options.ssh_config_path.as_deref(),
        )?)
    };

    if let Some(cfg) = file_config.as_ref() {
        if cfg
            .host_config
            .proxy_command
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        {
            return Err(SshError::new(
                SshErrorKind::ProxyUnsupported,
                "ProxyCommand is not supported by the pure-Rust SSH client; remove ProxyCommand or use a direct host",
            ));
        }
        if cfg
            .host_config
            .proxy_jump
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        {
            return Err(SshError::new(
                SshErrorKind::ProxyUnsupported,
                "ProxyJump is not supported yet by the pure-Rust SSH client; nested jump will not silently connect to the final host",
            ));
        }
    }

    let hostname = file_config
        .as_ref()
        .and_then(|cfg| {
            cfg.host_config
                .hostname
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| host_alias.to_string());

    // Explicit profile fields take priority over ssh_config.
    let port = server
        .port
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|cfg| cfg.port.or(cfg.host_config.port))
        })
        .unwrap_or(DEFAULT_SSH_PORT);

    let user = server
        .user
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            file_config.as_ref().and_then(|cfg| {
                cfg.user
                    .as_deref()
                    .or(cfg.host_config.user.as_deref())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(default_ssh_user);

    // Stable key order: explicit profile IdentityFile → ssh_config IdentityFile →
    // (agent and defaults applied later in session auth).
    let mut identity_files = Vec::new();
    if let Some(path) = server.identity_file.as_ref() {
        identity_files.push(expand_user_path(path));
    }
    if let Some(cfg_paths) = file_config
        .as_ref()
        .and_then(|cfg| cfg.host_config.identity_file.clone())
    {
        for path in cfg_paths {
            let expanded = expand_user_path(&path);
            if !identity_files.iter().any(|existing| existing == &expanded) {
                identity_files.push(expanded);
            }
        }
    }

    let known_hosts_path = file_config
        .as_ref()
        .and_then(|cfg| cfg.host_config.user_known_hosts_file.clone())
        .map(|path| expand_user_path(&path))
        .unwrap_or_else(default_known_hosts_path);

    let strict_host_key_checking = match file_config
        .as_ref()
        .and_then(|cfg| cfg.host_config.strict_host_key_checking)
    {
        Some(true) => StrictHostKeyChecking::Yes,
        Some(false) => StrictHostKeyChecking::No,
        None => StrictHostKeyChecking::Ask,
    };

    let connect_timeout = Duration::from_millis(server.connect_timeout_ms.max(1));

    let password = options
        .password
        .filter(|value| !value.is_empty())
        .or_else(|| {
            server
                .password
                .clone()
                .filter(|value| !value.trim().is_empty())
        });

    let method = server.auth_method;
    let use_agent = match method {
        RemoteAuthMethod::Password => false,
        RemoteAuthMethod::Key => options.use_agent.unwrap_or(true),
    };

    Ok(ResolvedSshProfile {
        host_alias: host_alias.to_string(),
        hostname,
        port,
        user,
        connect_timeout,
        known_hosts_path,
        strict_host_key_checking,
        auth: SshAuthConfig {
            method,
            identity_files,
            password: match method {
                RemoteAuthMethod::Password => password,
                RemoteAuthMethod::Key => None,
            },
            use_agent,
            // Always allow bounded default id_* after configured identities + agent.
            try_default_identities: true,
        },
    })
}

fn load_ssh_config(
    host_alias: &str,
    custom_path: Option<&Path>,
) -> Result<russh_config::Config, SshError> {
    match custom_path {
        Some(path) => russh_config::parse_path(path, host_alias).map_err(|source| {
            SshError::new(
                SshErrorKind::Config,
                format!("failed to parse ssh config {}: {source}", path.display()),
            )
        }),
        None => {
            // Missing default config is fine: treat as empty host entry.
            match russh_config::parse_home(host_alias) {
                Ok(config) => Ok(config),
                Err(russh_config::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                    Ok(russh_config::Config::default(host_alias))
                }
                Err(russh_config::Error::HostNotFound) | Err(russh_config::Error::NoHome) => {
                    Ok(russh_config::Config::default(host_alias))
                }
                Err(source) => Err(SshError::new(
                    SshErrorKind::Config,
                    format!("failed to parse ~/.ssh/config: {source}"),
                )),
            }
        }
    }
}

fn default_ssh_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "root".to_string())
}

fn default_known_hosts_path() -> PathBuf {
    home_dir()
        .map(|home| home.join(".ssh").join("known_hosts"))
        .unwrap_or_else(|| PathBuf::from("known_hosts"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn expand_user_path(path: &Path) -> PathBuf {
    let raw = path.as_os_str();
    let text = raw.to_string_lossy();
    if text == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn profile(host: &str) -> RemoteServerProfile {
        RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: host.to_string(),
            ..RemoteServerProfile::default()
        }
    }

    #[test]
    fn direct_hostname_skips_ssh_config() {
        let resolved = resolve_ssh_profile(
            &profile("203.0.113.10"),
            ResolveSshOptions {
                skip_ssh_config: true,
                ..Default::default()
            },
        )
        .expect("resolve");
        assert_eq!(resolved.hostname, "203.0.113.10");
        assert_eq!(resolved.port, 22);
        assert!(resolved.auth.identity_files.is_empty());
        assert_eq!(
            resolved.strict_host_key_checking,
            StrictHostKeyChecking::Ask
        );
    }

    #[test]
    fn profile_fields_override_ssh_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config");
        let mut file = fs::File::create(&config_path).expect("create");
        writeln!(
      file,
      "Host build-box\n  HostName 10.0.0.5\n  User deploy\n  Port 2222\n  IdentityFile /tmp/from-config\n  UserKnownHostsFile /tmp/kh\n  StrictHostKeyChecking no\n"
    )
    .expect("write");

        let mut server = profile("build-box");
        server.user = Some("root".to_string());
        server.port = Some(22);
        server.identity_file = Some(PathBuf::from("/tmp/from-profile"));

        let resolved = resolve_ssh_profile(
            &server,
            ResolveSshOptions {
                ssh_config_path: Some(config_path),
                ..Default::default()
            },
        )
        .expect("resolve");

        assert_eq!(resolved.hostname, "10.0.0.5");
        assert_eq!(resolved.user, "root");
        assert_eq!(resolved.port, 22);
        assert_eq!(
            resolved.auth.identity_files,
            vec![
                PathBuf::from("/tmp/from-profile"),
                PathBuf::from("/tmp/from-config"),
            ]
        );
        assert!(resolved.auth.try_default_identities);
        assert_eq!(resolved.known_hosts_path, PathBuf::from("/tmp/kh"));
        assert_eq!(resolved.strict_host_key_checking, StrictHostKeyChecking::No);
    }

    #[test]
    fn ssh_config_fills_when_profile_fields_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config");
        let mut file = fs::File::create(&config_path).expect("create");
        writeln!(
      file,
      "Host legacy\n  HostName legacy.example\n  User ubuntu\n  Port 2201\n  IdentityFile ~/.ssh/id_ed25519\n  StrictHostKeyChecking yes\n"
    )
    .expect("write");

        let resolved = resolve_ssh_profile(
            &profile("legacy"),
            ResolveSshOptions {
                ssh_config_path: Some(config_path),
                ..Default::default()
            },
        )
        .expect("resolve");

        assert_eq!(resolved.hostname, "legacy.example");
        assert_eq!(resolved.user, "ubuntu");
        assert_eq!(resolved.port, 2201);
        assert_eq!(
            resolved.strict_host_key_checking,
            StrictHostKeyChecking::Yes
        );
        assert_eq!(resolved.auth.identity_files.len(), 1);
    }

    #[test]
    fn proxy_command_is_hard_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config");
        let mut file = fs::File::create(&config_path).expect("create");
        writeln!(
            file,
            "Host jump\n  HostName 10.0.0.1\n  ProxyCommand nc %h %p\n"
        )
        .expect("write");

        let err = resolve_ssh_profile(
            &profile("jump"),
            ResolveSshOptions {
                ssh_config_path: Some(config_path),
                ..Default::default()
            },
        )
        .expect_err("proxy");
        assert_eq!(err.kind, SshErrorKind::ProxyUnsupported);
        assert!(err.message().contains("ProxyCommand"));
    }

    #[test]
    fn proxy_jump_is_hard_error_not_silent_direct() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config");
        let mut file = fs::File::create(&config_path).expect("create");
        writeln!(
            file,
            "Host final\n  HostName 10.0.0.9\n  ProxyJump bastion\n"
        )
        .expect("write");

        let err = resolve_ssh_profile(
            &profile("final"),
            ResolveSshOptions {
                ssh_config_path: Some(config_path),
                ..Default::default()
            },
        )
        .expect_err("jump");
        assert_eq!(err.kind, SshErrorKind::ProxyUnsupported);
        assert!(err.message().contains("ProxyJump"));
    }

    #[test]
    fn password_is_redacted_in_debug() {
        let auth = SshAuthConfig {
            method: RemoteAuthMethod::Password,
            identity_files: vec![],
            password: Some("s3cret".to_string()),
            use_agent: false,
            try_default_identities: false,
        };
        let rendered = format!("{auth:?}");
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("s3cret"));
    }

    #[test]
    fn password_mode_loads_password_from_profile() {
        let mut server = profile("203.0.113.10");
        server.auth_method = RemoteAuthMethod::Password;
        server.password = Some("s3cret".to_string());
        let resolved = resolve_ssh_profile(
            &server,
            ResolveSshOptions {
                skip_ssh_config: true,
                ..Default::default()
            },
        )
        .expect("resolve");
        assert_eq!(resolved.auth.method, RemoteAuthMethod::Password);
        assert_eq!(resolved.auth.password.as_deref(), Some("s3cret"));
        assert!(!resolved.auth.use_agent);
        assert!(!format!("{:?}", resolved.auth).contains("s3cret"));
    }
}
