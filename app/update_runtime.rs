use std::{
    cmp::Ordering,
    env,
    ffi::{OsStr, OsString},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::{DateTime, SecondsFormat, Utc};
use foco_store::config::GlobalConfig;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::watch};

use crate::{ApiError, AppState, config_snapshot, config_update_snapshot, save_config};

const UPDATE_RELEASE_API_URL: &str = "https://api.github.com/repos/fonlan/foco/releases/latest";
const UPDATE_CHECK_STARTUP_DELAY_SECS: u64 = 30;
const UPDATE_CHECK_INTERVAL_SECS: u64 = 12 * 60 * 60;
const UPDATE_DOWNLOAD_TMP_SUFFIX: &str = ".download.tmp";
const UPDATE_SHUTDOWN_DELAY_MS: u64 = 250;
const UPDATE_FORCE_EXIT_DELAY_SECS: u64 = 10;
/// Keep the current version directory plus one recent historical version under `updates/`.
const UPDATE_VERSION_DIR_RETAIN_COUNT: usize = 2;
/// CLI flag used after a successful install to restart and trigger historical update cleanup.
pub(crate) const UPDATED_RESTART_ARG: &str = "--updated-restart";
/// Marker written by the platform update helper when install fails after the main process exits.
const LAST_INSTALL_FAILURE_FILE: &str = "last-install-failure.txt";
/// Marker written by a process started with `--updated-restart` after `/api/health` succeeds.
/// Platform update helpers wait for this before discarding the previous install.
const UPDATED_RESTART_READY_FILE: &str = "updated-restart-ready.txt";
/// How long the platform helper waits for the updated process to become ready before rolling back.
const UPDATED_RESTART_READY_TIMEOUT_SECS: u64 = 120;
/// How long the updated process itself waits for `/api/health` before giving up on the ready marker.
const UPDATED_RESTART_HEALTH_POLL_TIMEOUT_SECS: u64 = 90;
const UPDATED_RESTART_HEALTH_POLL_INTERVAL_MS: u64 = 250;
/// Marker written as soon as the detached macOS helper starts executing.
const UPDATE_HELPER_STARTED_FILE: &str = "helper-started.txt";
const UPDATE_HELPER_START_TIMEOUT_SECS: u64 = 5;
const UPDATE_HELPER_START_POLL_INTERVAL_MS: u64 = 25;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug, Default)]
pub(crate) struct UpdateState {
    // ponytail: update check history is intentionally in memory; persist it if users need history.
    last_checked_at: Option<DateTime<Utc>>,
    latest_release: Option<UpdateRelease>,
    /// Transient update-check / download error (cleared on the next successful check).
    error: Option<String>,
    /// One-shot install failure from the previous helper run (loaded at startup from disk).
    install_error: Option<String>,
    checking: bool,
}

#[derive(Clone, Debug)]
struct UpdateRelease {
    version: String,
    name: Option<String>,
    published_at: Option<String>,
    release_url: Option<String>,
    asset: Option<UpdateAsset>,
}

#[derive(Clone, Debug)]
struct UpdateAsset {
    name: String,
    download_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateStatusSummary {
    pub(crate) current_version: String,
    pub(crate) auto_check_enabled: bool,
    pub(crate) checking: bool,
    pub(crate) last_checked_at: Option<String>,
    pub(crate) update_available: bool,
    pub(crate) target_version: Option<String>,
    pub(crate) release_name: Option<String>,
    pub(crate) published_at: Option<String>,
    pub(crate) release_url: Option<String>,
    pub(crate) asset_name: Option<String>,
    pub(crate) asset_download_url: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

struct PreparedUpdateInstall {
    version: String,
    asset_name: String,
    asset_download_url: String,
    update_dir: PathBuf,
    archive_path: PathBuf,
}

pub(crate) fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Best-effort cleanup of historical update download dirs after a successful install restart.
///
/// Only runs when the process was started with `--updated-restart`. Failures are logged and never
/// abort startup.
pub(crate) fn maybe_cleanup_stale_update_assets_after_updated_restart(user_profile_dir: &Path) {
    cleanup_stale_update_assets_if_requested(
        user_profile_dir,
        env::args().any(|arg| arg == UPDATED_RESTART_ARG),
    );
}

/// Load a one-shot install failure left by the platform update helper, then clear the marker.
///
/// Used after a failed auto-update that rolled back to the previous app and relaunched it. The
/// reason is exposed via `UpdateStatusSummary.error` (About / settings). Failures never abort
/// startup.
pub(crate) fn load_last_install_failure_into_state(
    update_state: &mut UpdateState,
    user_profile_dir: &Path,
) {
    let Some(message) = take_last_install_failure_message(user_profile_dir) else {
        return;
    };
    tracing::warn!(error = %message, "loaded previous Foco update install failure");
    update_state.install_error = Some(message);
}

fn last_install_failure_path(user_profile_dir: &Path) -> PathBuf {
    user_profile_dir
        .join(".foco")
        .join("updates")
        .join(LAST_INSTALL_FAILURE_FILE)
}

fn take_last_install_failure_message(user_profile_dir: &Path) -> Option<String> {
    let path = last_install_failure_path(user_profile_dir);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return None,
        Err(source) => {
            tracing::warn!(
                path = %path.display(),
                error = %source,
                "failed to read Foco update install failure marker"
            );
            return None;
        }
    };
    if let Err(source) = std::fs::remove_file(&path) {
        tracing::warn!(
            path = %path.display(),
            error = %source,
            "failed to clear Foco update install failure marker"
        );
    }
    let message = raw.trim();
    if message.is_empty() {
        None
    } else {
        Some(message.chars().take(500).collect())
    }
}

fn clear_last_install_failure_marker(user_profile_dir: &Path) {
    let path = last_install_failure_path(user_profile_dir);
    if path.exists()
        && let Err(source) = std::fs::remove_file(&path)
    {
        tracing::warn!(
            path = %path.display(),
            error = %source,
            "failed to clear previous Foco update install failure marker before install"
        );
    }
}

fn updated_restart_ready_path(user_profile_dir: &Path) -> PathBuf {
    user_profile_dir
        .join(".foco")
        .join("updates")
        .join(UPDATED_RESTART_READY_FILE)
}

fn clear_updated_restart_ready_marker(user_profile_dir: &Path) {
    let path = updated_restart_ready_path(user_profile_dir);
    if path.exists()
        && let Err(source) = std::fs::remove_file(&path)
    {
        tracing::warn!(
            path = %path.display(),
            error = %source,
            "failed to clear previous Foco updated-restart ready marker before install"
        );
    }
}

fn write_updated_restart_ready_marker(user_profile_dir: &Path) {
    let path = updated_restart_ready_path(user_profile_dir);
    if let Some(parent) = path.parent()
        && let Err(source) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(
            path = %path.display(),
            error = %source,
            "failed to create directory for Foco updated-restart ready marker"
        );
        return;
    }
    let payload = format!(
        "pid={}\nstarted_at={}\n",
        std::process::id(),
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    );
    if let Err(source) = std::fs::write(&path, payload) {
        tracing::warn!(
            path = %path.display(),
            error = %source,
            "failed to write Foco updated-restart ready marker"
        );
        return;
    }
    tracing::info!(
        path = %path.display(),
        "wrote Foco updated-restart ready marker"
    );
}

/// After the HTTP server is accepting requests, signal the platform update helper that this
/// `--updated-restart` process is ready and the previous install may be discarded.
///
/// Polls `/api/health` so a process that only bound the port (or crashed before serve) never
/// clears the rollback backup.
pub(crate) fn spawn_mark_updated_restart_ready_when_serving(
    user_profile_dir: PathBuf,
    listen_addr: SocketAddr,
) {
    if !env::args().any(|arg| arg == UPDATED_RESTART_ARG) {
        return;
    }
    tokio::spawn(async move {
        let health_url = format!("http://{listen_addr}/api/health");
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()
        {
            Ok(client) => client,
            Err(source) => {
                tracing::warn!(
                    error = %source,
                    "failed to build HTTP client for Foco updated-restart ready probe"
                );
                return;
            }
        };
        let deadline = tokio::time::Instant::now()
            + Duration::from_secs(UPDATED_RESTART_HEALTH_POLL_TIMEOUT_SECS);
        loop {
            match client.get(&health_url).send().await {
                Ok(response) if response.status().is_success() => {
                    write_updated_restart_ready_marker(&user_profile_dir);
                    return;
                }
                Ok(response) => {
                    tracing::debug!(
                        status = %response.status(),
                        "updated-restart health probe not ready yet"
                    );
                }
                Err(source) => {
                    tracing::debug!(
                        error = %source,
                        "updated-restart health probe failed"
                    );
                }
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!(
                    url = %health_url,
                    "updated-restart ready marker not written: /api/health did not succeed in time"
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(
                UPDATED_RESTART_HEALTH_POLL_INTERVAL_MS,
            ))
            .await;
        }
    });
}

fn cleanup_stale_update_assets_if_requested(user_profile_dir: &Path, requested: bool) {
    if !requested {
        return;
    }
    let updates_root = user_profile_dir.join(".foco").join("updates");
    cleanup_stale_update_version_dirs(&updates_root, current_version());
}

/// Discover version dirs, apply retain policy, and best-effort delete excess directories.
fn cleanup_stale_update_version_dirs(updates_root: &Path, current_version: &str) {
    if !updates_root.exists() {
        return;
    }
    let candidates = match discover_update_version_dir_candidates(updates_root) {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::warn!(
                updates_root = %updates_root.display(),
                error = %error,
                "failed to discover Foco update version directories for cleanup"
            );
            return;
        }
    };
    let remove_names = select_update_version_dirs_to_remove(
        &candidates,
        current_version,
        UPDATE_VERSION_DIR_RETAIN_COUNT,
    );
    if remove_names.is_empty() {
        return;
    }
    tracing::info!(
        updates_root = %updates_root.display(),
        current_version,
        retain_count = UPDATE_VERSION_DIR_RETAIN_COUNT,
        remove_count = remove_names.len(),
        "cleaning historical Foco update version directories"
    );
    for name in remove_names {
        if let Err(error) = remove_update_version_dir(updates_root, &name) {
            tracing::warn!(
                updates_root = %updates_root.display(),
                version_dir = %name,
                error = %error,
                "failed to remove historical Foco update version directory"
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UpdateVersionDirCandidate {
    name: String,
    modified: SystemTime,
}

fn discover_update_version_dir_candidates(
    updates_root: &Path,
) -> Result<Vec<UpdateVersionDirCandidate>, String> {
    let read_dir = std::fs::read_dir(updates_root).map_err(|source| {
        format!(
            "failed to read Foco update directory {}: {source}",
            updates_root.display()
        )
    })?;
    let mut candidates = Vec::new();
    for entry in read_dir {
        // Best-effort: skip unreadable entries so one bad dirent cannot abort the whole cleanup.
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                tracing::warn!(
                    updates_root = %updates_root.display(),
                    error = %source,
                    "skipping unreadable Foco update directory entry during cleanup discovery"
                );
                continue;
            }
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !is_valid_update_version_dir_name(name) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(source) => {
                tracing::warn!(
                    path = %entry.path().display(),
                    error = %source,
                    "skipping Foco update entry that could not be inspected during cleanup discovery"
                );
                continue;
            }
        };
        // Do not follow symlinks; only real direct child directories are candidates.
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                tracing::warn!(
                    path = %entry.path().display(),
                    error = %source,
                    "skipping Foco update entry with unreadable metadata during cleanup discovery"
                );
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(source) => {
                tracing::warn!(
                    path = %entry.path().display(),
                    error = %source,
                    "skipping Foco update entry with unreadable mtime during cleanup discovery"
                );
                continue;
            }
        };
        candidates.push(UpdateVersionDirCandidate {
            name: name.to_string(),
            modified,
        });
    }
    Ok(candidates)
}

fn is_valid_update_version_dir_name(name: &str) -> bool {
    safe_update_path_component(name, "version").is_ok()
}

fn is_protected_update_version_dir(name: &str, current_version: &str) -> bool {
    normalized_version(name) == normalized_version(current_version)
}

/// Pure retain policy: always protect the running version; keep at most `retain_count` version
/// directories total (current + newest remaining by mtime). Equal mtimes sort by name ascending.
fn select_update_version_dirs_to_remove(
    candidates: &[UpdateVersionDirCandidate],
    current_version: &str,
    retain_count: usize,
) -> Vec<String> {
    if retain_count == 0 {
        return candidates
            .iter()
            .filter(|candidate| !is_protected_update_version_dir(&candidate.name, current_version))
            .map(|candidate| candidate.name.clone())
            .collect();
    }

    let mut protected = Vec::new();
    let mut others = Vec::new();
    for candidate in candidates {
        if is_protected_update_version_dir(&candidate.name, current_version) {
            protected.push(candidate);
        } else {
            others.push(candidate);
        }
    }

    others.sort_by(|left, right| {
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.name.cmp(&right.name))
    });

    let keep_other_count = retain_count.saturating_sub(protected.len());
    others
        .into_iter()
        .skip(keep_other_count)
        .map(|candidate| candidate.name.clone())
        .collect()
}

fn remove_update_version_dir(updates_root: &Path, name: &str) -> Result<(), String> {
    let validated_name =
        safe_update_path_component(name, "version").map_err(|error| error.message().to_string())?;
    let path = updates_root.join(validated_name);
    if !is_direct_child_path(updates_root, &path, validated_name) {
        return Err(format!(
            "refusing to remove path outside Foco update directory: {}",
            path.display()
        ));
    }

    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(format!(
                "failed to inspect Foco update version directory {}: {source}",
                path.display()
            ));
        }
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() || !file_type.is_dir() {
        return Err(format!(
            "refusing to remove non-directory Foco update path: {}",
            path.display()
        ));
    }

    std::fs::remove_dir_all(&path).map_err(|source| {
        format!(
            "failed to remove Foco update version directory {}: {source}",
            path.display()
        )
    })
}

fn is_direct_child_path(parent: &Path, child: &Path, expected_name: &str) -> bool {
    if expected_name.is_empty()
        || expected_name == "."
        || expected_name == ".."
        || expected_name.contains('/')
        || expected_name.contains('\\')
    {
        return false;
    }
    match child.strip_prefix(parent) {
        Ok(relative) => {
            let mut components = relative.components();
            match (components.next(), components.next()) {
                (Some(std::path::Component::Normal(name)), None) => name == expected_name,
                _ => false,
            }
        }
        Err(_) => false,
    }
}

pub(crate) fn update_status_summary(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<UpdateStatusSummary, ApiError> {
    let update_state = state
        .update_state
        .lock()
        .map_err(|_| ApiError::internal("update state lock was poisoned"))?;

    Ok(status_summary_from_state(&update_state, config))
}

pub(crate) async fn check_for_updates(
    state: &AppState,
    force: bool,
) -> Result<UpdateStatusSummary, ApiError> {
    let config = config_snapshot(state)?;
    if !force {
        let update_state = state
            .update_state
            .lock()
            .map_err(|_| ApiError::internal("update state lock was poisoned"))?;
        if update_state.checking || !update_check_due(&update_state) {
            return Ok(status_summary_from_state(&update_state, &config));
        }
    }

    {
        let mut update_state = state
            .update_state
            .lock()
            .map_err(|_| ApiError::internal("update state lock was poisoned"))?;
        if update_state.checking {
            return Ok(status_summary_from_state(&update_state, &config));
        }
        update_state.checking = true;
    }

    let checked_at = Utc::now();
    let check_result = fetch_latest_update_release().await;
    let config = config_snapshot(state)?;
    let mut update_state = state
        .update_state
        .lock()
        .map_err(|_| ApiError::internal("update state lock was poisoned"))?;
    update_state.checking = false;
    update_state.last_checked_at = Some(checked_at);
    match check_result {
        Ok(release) => {
            update_state.latest_release = release;
            update_state.error = None;
        }
        Err(error) => {
            tracing::warn!(error, "update check failed");
            update_state.error = Some(error);
        }
    }

    Ok(status_summary_from_state(&update_state, &config))
}

pub(crate) async fn save_update_settings(
    state: &AppState,
    auto_check_enabled: bool,
) -> Result<UpdateStatusSummary, ApiError> {
    let mut config = config_update_snapshot(state).await?;
    config.app.auto_update_check_enabled = auto_check_enabled;
    save_config(state, &mut config)?;
    update_status_summary(state, &config)
}

pub(crate) async fn install_update(state: &AppState) -> Result<UpdateStatusSummary, ApiError> {
    let _install_guard = state
        .update_install_lock
        .try_lock()
        .map_err(|_| ApiError::conflict("update installation is already running"))?;
    let config = config_snapshot(state)?;
    let summary = update_status_summary(state, &config)?;
    if !summary.update_available {
        return Err(ApiError::bad_request("no update is available"));
    }
    if cfg!(debug_assertions) {
        return Err(ApiError::bad_request(
            "update installation is not supported for development builds",
        ));
    }
    let prepared = prepare_update_install(state, &summary)?;

    download_update_asset(&prepared.asset_download_url, &prepared.archive_path).await?;
    validate_downloaded_update_asset(&prepared.asset_name, &prepared.archive_path)?;
    clear_last_install_failure_marker(&state.user_profile_dir);
    clear_updated_restart_ready_marker(&state.user_profile_dir);
    if let Ok(mut update_state) = state.update_state.lock() {
        update_state.install_error = None;
    }
    start_update_helper(&prepared).await?;
    request_shutdown_after_update_helper_started(state.app_shutdown_tx.clone());

    // Re-read after clearing install_error so the install response does not echo a stale failure.
    update_status_summary(state, &config)
}

pub(crate) fn spawn_update_check_scheduler(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown_rx = state.app_shutdown_rx.clone();
        if sleep_until_shutdown(
            &mut shutdown_rx,
            Duration::from_secs(UPDATE_CHECK_STARTUP_DELAY_SECS),
        )
        .await
        {
            return;
        }

        loop {
            match config_snapshot(&state) {
                Ok(config) if config.app.auto_update_check_enabled => {
                    if let Ok(summary) = check_for_updates(&state, false).await {
                        if let Some(error) = summary.error {
                            tracing::warn!(error, "scheduled update check failed");
                        }
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(error = %error.message(), "failed to read config for update scheduler")
                }
            }

            if sleep_until_shutdown(
                &mut shutdown_rx,
                Duration::from_secs(UPDATE_CHECK_INTERVAL_SECS),
            )
            .await
            {
                return;
            }
        }
    })
}

async fn fetch_latest_update_release() -> Result<Option<UpdateRelease>, String> {
    let release = reqwest::Client::new()
        .get(UPDATE_RELEASE_API_URL)
        .header(reqwest::header::USER_AGENT, "foco")
        .send()
        .await
        .map_err(|source| format!("failed to fetch latest Foco release: {source}"))?
        .error_for_status()
        .map_err(|source| format!("Foco release request failed: {source}"))?
        .json::<GithubReleaseResponse>()
        .await
        .map_err(|source| format!("failed to parse latest Foco release: {source}"))?;

    if compare_versions(&release.tag_name, current_version()) != Ordering::Greater {
        return Ok(None);
    }

    let asset = select_platform_asset(&release.assets).map(|asset| UpdateAsset {
        name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
    });

    Ok(Some(UpdateRelease {
        version: release.tag_name,
        name: release.name,
        published_at: release.published_at,
        release_url: release.html_url,
        asset,
    }))
}

fn status_summary_from_state(
    update_state: &UpdateState,
    config: &GlobalConfig,
) -> UpdateStatusSummary {
    let release = update_state.latest_release.as_ref();
    let asset = release.and_then(|release| release.asset.as_ref());
    UpdateStatusSummary {
        current_version: current_version().to_string(),
        auto_check_enabled: config.app.auto_update_check_enabled,
        checking: update_state.checking,
        last_checked_at: update_state
            .last_checked_at
            .map(|checked_at| checked_at.to_rfc3339_opts(SecondsFormat::Secs, true)),
        update_available: release.is_some(),
        target_version: release.map(|release| release.version.clone()),
        release_name: release.and_then(|release| release.name.clone()),
        published_at: release.and_then(|release| release.published_at.clone()),
        release_url: release.and_then(|release| release.release_url.clone()),
        asset_name: asset.map(|asset| asset.name.clone()),
        asset_download_url: asset.map(|asset| asset.download_url.clone()),
        // Prefer a durable install failure over a transient check error so About can explain
        // a rolled-back update after restart.
        error: update_state
            .install_error
            .clone()
            .or_else(|| update_state.error.clone()),
    }
}

fn prepare_update_install(
    state: &AppState,
    summary: &UpdateStatusSummary,
) -> Result<PreparedUpdateInstall, ApiError> {
    if !summary.update_available {
        return Err(ApiError::bad_request("no update is available"));
    }
    if !platform_supports_installation() {
        return Err(ApiError::bad_request(
            "update installation is not supported on this platform",
        ));
    }

    let version = summary
        .target_version
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("update version is missing"))?;
    let asset_name = summary
        .asset_name
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("no update asset is available for this platform"))?;
    let asset_download_url = summary
        .asset_download_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("update asset download URL is missing"))?;

    if !platform_asset_name(asset_name) {
        return Err(ApiError::bad_request(format!(
            "update asset '{asset_name}' does not match this platform"
        )));
    }
    let version_dir_name = safe_update_path_component(version, "version")?;
    let asset_file_name = safe_update_path_component(asset_name, "asset name")?;
    preflight_platform_installation()?;

    let update_dir = state
        .user_profile_dir
        .join(".foco")
        .join("updates")
        .join(version_dir_name);
    let archive_path = update_dir.join(asset_file_name);

    Ok(PreparedUpdateInstall {
        version: version.to_string(),
        asset_name: asset_name.to_string(),
        asset_download_url: asset_download_url.to_string(),
        update_dir,
        archive_path,
    })
}

fn update_check_due(update_state: &UpdateState) -> bool {
    update_state
        .last_checked_at
        .and_then(|checked_at| Utc::now().signed_duration_since(checked_at).to_std().ok())
        .is_none_or(|elapsed| elapsed >= Duration::from_secs(UPDATE_CHECK_INTERVAL_SECS))
}

async fn sleep_until_shutdown(shutdown_rx: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    if *shutdown_rx.borrow() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        changed = shutdown_rx.changed() => changed.is_err() || *shutdown_rx.borrow(),
    }
}

fn select_platform_asset(assets: &[GithubReleaseAsset]) -> Option<&GithubReleaseAsset> {
    assets.iter().find(|asset| platform_asset_name(&asset.name))
}

fn platform_asset_name(name: &str) -> bool {
    platform_asset_name_for(name, env::consts::OS, env::consts::ARCH)
}

fn platform_asset_name_for(name: &str, target_os: &str, target_arch: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if !name.starts_with("foco-v") {
        return false;
    }

    match (target_os, target_arch) {
        ("macos", "aarch64" | "arm64") => name.ends_with("-macos-arm64.dmg"),
        ("windows", "x86_64" | "x64") => name.ends_with("-windows-x64-setup.exe"),
        _ => false,
    }
}

fn platform_supports_installation() -> bool {
    cfg!(target_os = "macos") || cfg!(windows)
}

fn safe_update_path_component<'a>(value: &'a str, label: &str) -> Result<&'a str, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
    {
        return Err(ApiError::bad_request(format!(
            "update {label} contains unsupported path characters"
        )));
    }
    Ok(trimmed)
}

async fn download_update_asset(url: &str, destination: &Path) -> Result<(), ApiError> {
    tokio::fs::create_dir_all(destination.parent().ok_or_else(|| {
        ApiError::internal(format!(
            "failed to resolve Foco update directory for {}",
            destination.display()
        ))
    })?)
    .await
    .map_err(|source| {
        ApiError::internal(format!(
            "failed to create Foco update directory {}: {source}",
            destination.display()
        ))
    })?;

    let tmp_path = destination.with_file_name(format!(
        "{}{}",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("foco-update"),
        UPDATE_DOWNLOAD_TMP_SUFFIX
    ));
    let _ = tokio::fs::remove_file(&tmp_path).await;

    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::USER_AGENT, "foco")
        .send()
        .await
        .map_err(|source| {
            ApiError::internal(format!("failed to download Foco update: {source}"))
        })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::internal(format!(
            "Foco update download failed with HTTP {status}: {}",
            truncate_error_body(&body)
        )));
    }

    let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|source| {
        ApiError::internal(format!(
            "failed to create temporary Foco update file {}: {source}",
            tmp_path.display()
        ))
    })?;
    let bytes = response.bytes().await.map_err(|source| {
        ApiError::internal(format!("failed to read Foco update download: {source}"))
    })?;
    file.write_all(&bytes).await.map_err(|source| {
        ApiError::internal(format!(
            "failed to write temporary Foco update file {}: {source}",
            tmp_path.display()
        ))
    })?;
    file.flush().await.map_err(|source| {
        ApiError::internal(format!(
            "failed to flush temporary Foco update file {}: {source}",
            tmp_path.display()
        ))
    })?;
    drop(file);

    let _ = tokio::fs::remove_file(destination).await;
    tokio::fs::rename(&tmp_path, destination)
        .await
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to finalize Foco update download {}: {source}",
                destination.display()
            ))
        })
}

fn validate_downloaded_update_asset(asset_name: &str, path: &Path) -> Result<(), ApiError> {
    let metadata = std::fs::metadata(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to inspect downloaded Foco update {}: {source}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(ApiError::internal(format!(
            "downloaded Foco update is empty or not a file: {}",
            path.display()
        )));
    }

    // ponytail: first updater skips signatures/checksums until releases publish a manifest;
    // enforce sha256 here when that exists.
    if asset_name.to_ascii_lowercase().ends_with(".zip") && !zip_file_has_header(path)? {
        return Err(ApiError::internal(format!(
            "downloaded Foco update is not a zip archive: {}",
            path.display()
        )));
    }
    if asset_name.to_ascii_lowercase().ends_with(".exe") && !pe_file_has_header(path)? {
        return Err(ApiError::internal(format!(
            "downloaded Foco update is not a Windows executable: {}",
            path.display()
        )));
    }

    Ok(())
}

fn pe_file_has_header(path: &Path) -> Result<bool, ApiError> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to open downloaded Foco update {}: {source}",
            path.display()
        ))
    })?;
    let mut header = [0_u8; 2];
    file.read_exact(&mut header).map_err(|source| {
        ApiError::internal(format!(
            "failed to read downloaded Foco update {}: {source}",
            path.display()
        ))
    })?;
    Ok(matches!(header, [b'M', b'Z']))
}

fn zip_file_has_header(path: &Path) -> Result<bool, ApiError> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to open downloaded Foco update {}: {source}",
            path.display()
        ))
    })?;
    let mut header = [0_u8; 4];
    file.read_exact(&mut header).map_err(|source| {
        ApiError::internal(format!(
            "failed to read downloaded Foco update {}: {source}",
            path.display()
        ))
    })?;
    Ok(matches!(header, [b'P', b'K', 3, 4] | [b'P', b'K', 5, 6]))
}

fn truncate_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty response body".to_string();
    }
    trimmed.chars().take(200).collect()
}

fn preflight_platform_installation() -> Result<(), ApiError> {
    let current_exe = env::current_exe().map_err(|source| {
        ApiError::internal(format!(
            "failed to locate current Foco executable: {source}"
        ))
    })?;

    if cfg!(target_os = "macos") {
        current_macos_app_bundle_from_exe(&current_exe)
            .map(|_| ())
            .ok_or_else(|| {
                ApiError::bad_request(
                    "current installation shape does not support automatic installation",
                )
            })
    } else if cfg!(windows) {
        current_windows_install_dir_from_exe(&current_exe)
            .map(|_| ())
            .ok_or_else(|| ApiError::bad_request("failed to locate current Foco install directory"))
    } else {
        Err(ApiError::bad_request(
            "update installation is not supported on this platform",
        ))
    }
}

async fn start_update_helper(prepared: &PreparedUpdateInstall) -> Result<(), ApiError> {
    if cfg!(target_os = "macos") {
        start_macos_update_helper(prepared).await
    } else if cfg!(windows) {
        start_windows_update_helper(prepared)
    } else {
        Err(ApiError::bad_request(
            "update installation is not supported on this platform",
        ))
    }
}

async fn start_macos_update_helper(prepared: &PreparedUpdateInstall) -> Result<(), ApiError> {
    let current_exe = env::current_exe().map_err(|source| {
        ApiError::internal(format!(
            "failed to locate current Foco executable: {source}"
        ))
    })?;
    let app_bundle = current_macos_app_bundle_from_exe(&current_exe).ok_or_else(|| {
        ApiError::bad_request("current installation shape does not support automatic installation")
    })?;
    let script_path = prepared.update_dir.join("apply-macos-update.sh");
    std::fs::write(&script_path, macos_update_script()).map_err(|source| {
        ApiError::internal(format!(
            "failed to write Foco macOS update helper {}: {source}",
            script_path.display()
        ))
    })?;
    // prepared.update_dir = <profile>/.foco/updates/<version>
    let updates_root = prepared.update_dir.parent().ok_or_else(|| {
        ApiError::internal("failed to resolve Foco updates root path".to_string())
    })?;
    let result_path = updates_root.join(LAST_INSTALL_FAILURE_FILE);
    let ready_path = updates_root.join(UPDATED_RESTART_READY_FILE);
    let started_path = prepared.update_dir.join(UPDATE_HELPER_STARTED_FILE);
    match std::fs::remove_file(&started_path) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ApiError::internal(format!(
                "failed to clear previous Foco update helper marker {}: {source}",
                started_path.display()
            )));
        }
    }

    let pid = std::process::id();
    let label = macos_update_helper_label(pid);
    let launch_args = macos_update_launch_args(MacosUpdateLaunch {
        app_bundle: &app_bundle,
        label: &label,
        pid,
        prepared,
        ready_path: &ready_path,
        result_path: &result_path,
        script_path: &script_path,
        started_path: &started_path,
    });
    let output = Command::new("/bin/launchctl")
        .args(&launch_args)
        .output()
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to submit detached Foco macOS update helper: {source}"
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            output.status.to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(ApiError::internal(format!(
            "failed to submit detached Foco macOS update helper: {detail}"
        )));
    }

    if let Err(error) = wait_for_macos_update_helper_start(&started_path).await {
        let remove_output = Command::new("/bin/launchctl")
            .args(["remove", &label])
            .output();
        match remove_output {
            Ok(remove_output) if !remove_output.status.success() => {
                tracing::warn!(
                    label,
                    stderr = %String::from_utf8_lossy(&remove_output.stderr).trim(),
                    "failed to remove unacknowledged Foco update helper job"
                );
            }
            Err(source) => {
                tracing::warn!(
                    label,
                    error = %source,
                    "failed to invoke launchctl for unacknowledged Foco update helper cleanup"
                );
            }
            Ok(_) => {}
        }
        return Err(error);
    }

    tracing::info!(
        label,
        "detached Foco macOS update helper acknowledged startup"
    );
    Ok(())
}

struct MacosUpdateLaunch<'a> {
    app_bundle: &'a Path,
    label: &'a str,
    pid: u32,
    prepared: &'a PreparedUpdateInstall,
    ready_path: &'a Path,
    result_path: &'a Path,
    script_path: &'a Path,
    started_path: &'a Path,
}

fn macos_update_helper_label(pid: u32) -> String {
    format!("app.foco.update.{pid}")
}

fn macos_update_launch_args(launch: MacosUpdateLaunch<'_>) -> Vec<OsString> {
    let log_path = launch
        .prepared
        .update_dir
        .join(format!("apply-{}.log", launch.prepared.version));
    let pid = launch.pid.to_string();
    let ready_timeout_secs = UPDATED_RESTART_READY_TIMEOUT_SECS.to_string();
    vec![
        OsString::from("submit"),
        OsString::from("-l"),
        OsString::from(launch.label),
        OsString::from("-o"),
        OsString::from("/dev/null"),
        OsString::from("-e"),
        OsString::from("/dev/null"),
        OsString::from("--"),
        OsString::from("/usr/bin/env"),
        environment_assignment("FOCO_UPDATE_DMG", launch.prepared.archive_path.as_os_str()),
        environment_assignment("FOCO_UPDATE_APP", launch.app_bundle.as_os_str()),
        environment_assignment("FOCO_UPDATE_PID", OsStr::new(&pid)),
        environment_assignment("FOCO_UPDATE_LOG", log_path.as_os_str()),
        environment_assignment("FOCO_UPDATE_RESULT", launch.result_path.as_os_str()),
        environment_assignment("FOCO_UPDATE_READY", launch.ready_path.as_os_str()),
        environment_assignment("FOCO_UPDATE_STARTED", launch.started_path.as_os_str()),
        environment_assignment("FOCO_UPDATE_JOB_LABEL", OsStr::new(launch.label)),
        environment_assignment(
            "FOCO_UPDATE_READY_TIMEOUT_SECS",
            OsStr::new(&ready_timeout_secs),
        ),
        OsString::from("/bin/sh"),
        launch.script_path.as_os_str().to_os_string(),
    ]
}

fn environment_assignment(name: &str, value: &OsStr) -> OsString {
    let mut assignment = OsString::from(name);
    assignment.push("=");
    assignment.push(value);
    assignment
}

async fn wait_for_macos_update_helper_start(started_path: &Path) -> Result<(), ApiError> {
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(UPDATE_HELPER_START_TIMEOUT_SECS);
    loop {
        match std::fs::read_to_string(started_path) {
            Ok(_) => {
                if let Err(source) = std::fs::remove_file(started_path) {
                    tracing::warn!(
                        path = %started_path.display(),
                        error = %source,
                        "failed to clear Foco update helper startup marker"
                    );
                }
                return Ok(());
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ApiError::internal(format!(
                    "failed to read Foco update helper startup marker {}: {source}",
                    started_path.display()
                )));
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(ApiError::internal(format!(
                "detached Foco macOS update helper did not acknowledge startup within {} seconds",
                UPDATE_HELPER_START_TIMEOUT_SECS
            )));
        }
        tokio::time::sleep(Duration::from_millis(UPDATE_HELPER_START_POLL_INTERVAL_MS)).await;
    }
}

fn macos_update_script() -> &'static str {
    r#"#!/bin/sh
set -eu
: "${FOCO_UPDATE_JOB_LABEL:?}"

volume=""
cleanup() {
  if [ -n "$volume" ]; then
    hdiutil detach "$volume" >/dev/null 2>&1 || true
  fi
  /bin/launchctl remove "$FOCO_UPDATE_JOB_LABEL" >/dev/null 2>&1 || true
}
trap cleanup EXIT

: "${FOCO_UPDATE_DMG:?}"
: "${FOCO_UPDATE_APP:?}"
: "${FOCO_UPDATE_PID:?}"
: "${FOCO_UPDATE_LOG:?}"
: "${FOCO_UPDATE_RESULT:?}"
: "${FOCO_UPDATE_READY:?}"
: "${FOCO_UPDATE_STARTED:?}"
: "${FOCO_UPDATE_READY_TIMEOUT_SECS:?}"

# Logging must never prevent rollback/relaunch on failure paths.
mkdir -p "$(dirname "$FOCO_UPDATE_LOG")" 2>/dev/null || true
if ! exec >>"$FOCO_UPDATE_LOG" 2>&1; then
  :
fi

write_result() {
  message="$1"
  mkdir -p "$(dirname "$FOCO_UPDATE_RESULT")" 2>/dev/null || true
  printf '%s\n' "$message" >"$FOCO_UPDATE_RESULT" 2>/dev/null || true
}

mkdir -p "$(dirname "$FOCO_UPDATE_STARTED")" 2>/dev/null || true
if ! printf 'pid=%s\n' "$$" >"$FOCO_UPDATE_STARTED"; then
  write_result "Update failed: detached update helper could not write its startup marker."
  exit 1
fi
echo "detached update helper started; waiting for Foco process $FOCO_UPDATE_PID to exit"

relaunch_app() {
  target="$1"
  if [ -d "$target" ]; then
    open -n "$target" || return 1
    return 0
  fi
  return 1
}

# Restore the previous app bundle when present, then relaunch whatever is available.
# Only claim "rolled back" in the result marker after the old bundle is restored or launched.
rollback_and_relaunch() {
  reason="$1"
  echo "update failed: $reason"
  restored=0
  if [ -d "$old_app" ]; then
    rm -rf "$FOCO_UPDATE_APP" 2>/dev/null || true
    if mv "$old_app" "$FOCO_UPDATE_APP" 2>/dev/null; then
      restored=1
    else
      # Last resort: keep serving from the preserved .old bundle.
      if relaunch_app "$old_app"; then
        write_result "Update failed: $reason. Previous version is still available at $old_app but could not replace the current install; launched from the previous bundle."
      else
        write_result "Update failed: $reason. Previous version is still available at $old_app but could not replace the current install or launch."
      fi
      return 0
    fi
  fi
  if [ "$restored" -eq 1 ]; then
    if relaunch_app "$FOCO_UPDATE_APP"; then
      write_result "Update failed: $reason. Rolled back to the previous version."
    else
      write_result "Update failed: $reason. Rolled back to the previous version but could not relaunch it."
    fi
    return 0
  fi
  if relaunch_app "$FOCO_UPDATE_APP"; then
    write_result "Update failed: $reason. Could not restore the previous version; relaunched the current install."
  else
    write_result "Update failed: $reason. Could not restore the previous version or relaunch the app."
  fi
}

fail_early() {
  reason="$1"
  write_result "Update failed: $reason"
  relaunch_app "$FOCO_UPDATE_APP" || true
  exit 1
}

while kill -0 "$FOCO_UPDATE_PID" 2>/dev/null; do
  sleep 1
done
echo "Foco process $FOCO_UPDATE_PID exited; applying update"

mount_output="$(hdiutil attach "$FOCO_UPDATE_DMG" -nobrowse -readonly)" || {
  fail_early "could not mount the update disk image"
}
volume="$(printf '%s\n' "$mount_output" | awk '/\/Volumes\// { sub(/^.*\/Volumes\//, "/Volumes/"); print; exit }')"
if [ -z "$volume" ]; then
  fail_early "could not locate the mounted update volume"
fi
source_app="$(find "$volume" -maxdepth 2 -name "Foco.app" -type d | head -n 1)"
if [ -z "$source_app" ]; then
  fail_early "Foco.app was not found in the update disk image"
fi

new_app="$FOCO_UPDATE_APP.new"
old_app="$FOCO_UPDATE_APP.old"
# Best-effort cleanup of leftovers; never abort the helper under set -e if removal fails.
rm -rf "$new_app" 2>/dev/null || true
rm -rf "$old_app" 2>/dev/null || true
if [ -e "$new_app" ] || [ -e "$old_app" ]; then
  fail_early "could not clear leftover update staging directories (.new/.old); refusing to continue without a clean rollback path"
fi
rm -f "$FOCO_UPDATE_READY" 2>/dev/null || true

if ! ditto "$source_app" "$new_app"; then
  rollback_and_relaunch "could not copy the new app bundle from the update image"
  exit 1
fi

if [ -d "$FOCO_UPDATE_APP" ]; then
  if ! mv "$FOCO_UPDATE_APP" "$old_app"; then
    rm -rf "$new_app" 2>/dev/null || true
    fail_early "could not move the current app aside for replacement"
  fi
fi

if ! mv "$new_app" "$FOCO_UPDATE_APP"; then
  rollback_and_relaunch "could not install the new app bundle"
  exit 1
fi

# ponytail: trusted self-updates are unsigned; public releases should use Developer ID notarization instead.
if ! xattr -dr com.apple.quarantine "$FOCO_UPDATE_APP"; then
  rollback_and_relaunch "could not clear quarantine attributes on the new app (Gatekeeper may block launch)"
  exit 1
fi

# Recheck: any remaining quarantine attrs mean Gatekeeper can still block the relaunch.
if xattr -lr "$FOCO_UPDATE_APP" 2>/dev/null | grep -F 'com.apple.quarantine' >/dev/null 2>&1; then
  rollback_and_relaunch "quarantine attributes remain on the new app after clear (Gatekeeper may block launch)"
  exit 1
fi

# Launch the updated app, then wait until it proves readiness before discarding the previous bundle.
if ! open -n "$FOCO_UPDATE_APP" --args --updated-restart; then
  rollback_and_relaunch "could not launch the updated app after install"
  exit 1
fi

elapsed=0
timeout_secs="$FOCO_UPDATE_READY_TIMEOUT_SECS"
while [ ! -f "$FOCO_UPDATE_READY" ]; do
  if [ "$elapsed" -ge "$timeout_secs" ]; then
    rollback_and_relaunch "updated app did not become ready within ${timeout_secs}s after launch"
    exit 1
  fi
  sleep 1
  elapsed=$((elapsed + 1))
done

rm -rf "$old_app" 2>/dev/null || true
rm -f "$FOCO_UPDATE_READY" 2>/dev/null || true
hdiutil detach "$volume" >/dev/null 2>&1 || true
volume=""
"#
}

fn start_windows_update_helper(prepared: &PreparedUpdateInstall) -> Result<(), ApiError> {
    let current_exe = env::current_exe().map_err(|source| {
        ApiError::internal(format!(
            "failed to locate current Foco executable: {source}"
        ))
    })?;
    let install_dir = current_windows_install_dir_from_exe(&current_exe)
        .ok_or_else(|| ApiError::bad_request("failed to locate current Foco install directory"))?;
    let script_path = prepared.update_dir.join("apply-windows-update.ps1");
    std::fs::write(&script_path, windows_update_script()).map_err(|source| {
        ApiError::internal(format!(
            "failed to write Foco Windows update helper {}: {source}",
            script_path.display()
        ))
    })?;
    // prepared.update_dir = <profile>/.foco/updates/<version>
    let updates_root = prepared.update_dir.parent().ok_or_else(|| {
        ApiError::internal("failed to resolve Foco updates root path".to_string())
    })?;
    let result_path = updates_root.join(LAST_INSTALL_FAILURE_FILE);
    let ready_path = updates_root.join(UPDATED_RESTART_READY_FILE);

    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script_path.to_string_lossy().as_ref(),
        ])
        .env("FOCO_UPDATE_INSTALLER", &prepared.archive_path)
        .env("FOCO_UPDATE_INSTALL_DIR", &install_dir)
        .env("FOCO_UPDATE_EXE", &current_exe)
        .env("FOCO_UPDATE_PID", std::process::id().to_string())
        .env(
            "FOCO_UPDATE_LOG",
            prepared
                .update_dir
                .join(format!("apply-{}.log", prepared.version)),
        )
        .env("FOCO_UPDATE_RESULT", &result_path)
        .env("FOCO_UPDATE_READY", &ready_path)
        .env(
            "FOCO_UPDATE_READY_TIMEOUT_SECS",
            UPDATED_RESTART_READY_TIMEOUT_SECS.to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.spawn().map_err(|source| {
        ApiError::internal(format!(
            "failed to start Foco Windows update helper: {source}"
        ))
    })?;
    Ok(())
}

fn windows_update_script() -> &'static str {
    r#"$ErrorActionPreference = "Stop"
$installer = $env:FOCO_UPDATE_INSTALLER
$installDir = $env:FOCO_UPDATE_INSTALL_DIR
$currentExe = $env:FOCO_UPDATE_EXE
$pidToWait = [int]$env:FOCO_UPDATE_PID
$logPath = $env:FOCO_UPDATE_LOG
$resultPath = $env:FOCO_UPDATE_RESULT
$readyPath = $env:FOCO_UPDATE_READY
$readyTimeoutSecs = [int]$env:FOCO_UPDATE_READY_TIMEOUT_SECS
if (-not $installer -or -not $installDir -or -not $currentExe -or -not $pidToWait -or -not $logPath -or -not $resultPath -or -not $readyPath -or -not $readyTimeoutSecs) {
  throw "missing Foco update environment"
}

$transcriptStarted = $false
# Backup must live outside the install dir so restore can replace a partially overwritten tree.
# Use a unique sibling path so restore never confuses a nested Move-Item with a full install tree.
$backupParent = Split-Path -Parent $installDir
$backupDir = Join-Path $backupParent ("Foco.update-backup." + [guid]::NewGuid().ToString("N"))
$backupStaged = Join-Path $backupParent ("Foco.update-staging." + [guid]::NewGuid().ToString("N"))
$backupReady = $false

function Write-UpdateResult([string]$Message) {
  try {
    $directory = Split-Path -Parent $resultPath
    if ($directory -and -not (Test-Path -LiteralPath $directory)) {
      New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
    Set-Content -LiteralPath $resultPath -Value $Message -Encoding utf8
  } catch {
    Write-Host "failed to write update result marker: $_"
  }
}

function Remove-PathBestEffort([string]$Path) {
  if (-not $Path) { return }
  if (-not (Test-Path -LiteralPath $Path)) { return }
  try {
    Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
  } catch {
    Write-Host "failed to remove path ${Path}: $_"
  }
}

function Copy-InstallTree([string]$Source, [string]$Destination) {
  if (Test-Path -LiteralPath $Destination) {
    Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction Stop
  }
  New-Item -ItemType Directory -Path $Destination -Force | Out-Null
  Get-ChildItem -LiteralPath $Source -Force | ForEach-Object {
    $target = Join-Path $Destination $_.Name
    Copy-Item -LiteralPath $_.FullName -Destination $target -Recurse -Force -ErrorAction Stop
  }
}

function Stop-InstallProcesses {
  try {
    Get-CimInstance Win32_Process | Where-Object {
      $_.ExecutablePath -and (
        $_.ExecutablePath.Equals($currentExe, [System.StringComparison]::OrdinalIgnoreCase) -or
        $_.ExecutablePath.StartsWith(($installDir.TrimEnd('\') + '\'), [System.StringComparison]::OrdinalIgnoreCase)
      )
    } | ForEach-Object {
      try {
        Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop
      } catch {
        Write-Host "failed to stop process $($_.ProcessId): $_"
      }
    }
  } catch {
    Write-Host "failed to enumerate install processes: $_"
  }
  Start-Sleep -Milliseconds 500
}

function Start-InstallExe([string]$ExePath, [string[]]$ArgumentList) {
  if (-not (Test-Path -LiteralPath $ExePath)) {
    return $false
  }
  $workDir = Split-Path -Parent $ExePath
  if (-not $workDir -or -not (Test-Path -LiteralPath $workDir)) {
    $workDir = $installDir
  }
  try {
    if ($null -eq $ArgumentList -or $ArgumentList.Count -eq 0) {
      Start-Process -FilePath $ExePath -WorkingDirectory $workDir | Out-Null
    } else {
      Start-Process -FilePath $ExePath -ArgumentList $ArgumentList -WorkingDirectory $workDir | Out-Null
    }
    return $true
  } catch {
    Write-Host "failed to start Foco executable ${ExePath}: $_"
    return $false
  }
}

function Fail-BeforeInstall([string]$Reason) {
  Write-UpdateResult "Update failed: $Reason"
  [void](Start-InstallExe $currentExe @())
  exit 1
}

function Restore-PreviousInstall([string]$Reason) {
  $restored = $false
  Stop-InstallProcesses
  if ($backupReady -and (Test-Path -LiteralPath $backupDir)) {
    try {
      $brokenDir = Join-Path $backupParent ("Foco.update-broken." + [guid]::NewGuid().ToString("N"))
      if (Test-Path -LiteralPath $installDir) {
        Move-Item -LiteralPath $installDir -Destination $brokenDir -Force -ErrorAction Stop
      }
      Move-Item -LiteralPath $backupDir -Destination $installDir -Force -ErrorAction Stop
      $restored = $true
      $script:backupReady = $false
      Remove-PathBestEffort $brokenDir
    } catch {
      Write-Host "failed to restore install backup: $_"
      $backupExe = Join-Path $backupDir "foco.exe"
      if (Start-InstallExe $backupExe @()) {
        Write-UpdateResult "Update failed: $Reason. Previous version is still available at $backupDir but could not replace the current install; launched from the backup."
        return
      }
      Write-UpdateResult "Update failed: $Reason. Previous version is still available at $backupDir but could not replace the current install or launch."
      return
    }
  }

  if ($restored) {
    if (Start-InstallExe $currentExe @()) {
      Write-UpdateResult "Update failed: $Reason. Rolled back to the previous version."
    } else {
      Write-UpdateResult "Update failed: $Reason. Rolled back to the previous version but could not relaunch it."
    }
    return
  }

  if (Start-InstallExe $currentExe @()) {
    Write-UpdateResult "Update failed: $Reason. Could not restore the previous version; relaunched the current install."
  } else {
    Write-UpdateResult "Update failed: $Reason. Could not restore the previous version or relaunch the app."
  }
}

try {
  try {
    Start-Transcript -Path $logPath -Append | Out-Null
    $transcriptStarted = $true
  } catch {
    Write-Host "failed to start update transcript: $_"
  }

  Wait-Process -Id $pidToWait

  if (Test-Path -LiteralPath $readyPath) {
    Remove-Item -LiteralPath $readyPath -Force -ErrorAction SilentlyContinue
  }

  # Full install-tree backup before NSIS overwrites in place. Fail hard if backup cannot be staged.
  try {
    Copy-InstallTree -Source $installDir -Destination $backupStaged
    if (Test-Path -LiteralPath $backupDir) {
      throw "backup destination unexpectedly already exists: $backupDir"
    }
    Move-Item -LiteralPath $backupStaged -Destination $backupDir -Force -ErrorAction Stop
    $backupReady = $true
  } catch {
    Remove-PathBestEffort $backupStaged
    Remove-PathBestEffort $backupDir
    Fail-BeforeInstall "could not create a full install backup before running the installer: $($_.Exception.Message)"
  }

  $process = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
  if ($null -eq $process) {
    Restore-PreviousInstall "installer process could not be started"
    exit 1
  }
  if ($process.ExitCode -ne 0) {
    Restore-PreviousInstall "installer exited with code $($process.ExitCode)"
    exit 1
  }
  if (-not (Test-Path -LiteralPath $currentExe)) {
    Restore-PreviousInstall "updated executable was not found after install"
    exit 1
  }

  if (-not (Start-InstallExe $currentExe @('--updated-restart'))) {
    Restore-PreviousInstall "could not launch the updated app after install"
    exit 1
  }

  $elapsed = 0
  while (-not (Test-Path -LiteralPath $readyPath)) {
    if ($elapsed -ge $readyTimeoutSecs) {
      Restore-PreviousInstall "updated app did not become ready within ${readyTimeoutSecs}s after launch"
      exit 1
    }
    Start-Sleep -Seconds 1
    $elapsed++
  }

  Remove-PathBestEffort $backupDir
  $backupReady = $false
  if (Test-Path -LiteralPath $readyPath) {
    Remove-Item -LiteralPath $readyPath -Force -ErrorAction SilentlyContinue
  }
} catch {
  Restore-PreviousInstall $_.Exception.Message
  exit 1
} finally {
  if ($transcriptStarted) {
    try { Stop-Transcript | Out-Null } catch {}
  }
  if (Test-Path -LiteralPath $backupStaged) {
    Remove-PathBestEffort $backupStaged
  }
}
"#
}

fn request_shutdown_after_update_helper_started(shutdown_tx: watch::Sender<bool>) {
    let _ = std::thread::Builder::new()
        .name("foco-update-shutdown".to_string())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(UPDATE_SHUTDOWN_DELAY_MS));
            let _ = shutdown_tx.send(true);
            std::thread::sleep(Duration::from_secs(UPDATE_FORCE_EXIT_DELAY_SECS));
            std::process::exit(0);
        });
}

fn current_macos_app_bundle_from_exe(exe_path: &Path) -> Option<PathBuf> {
    let macos_dir = exe_path.parent()?;
    if macos_dir.file_name()? != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()? != "Contents" {
        return None;
    }
    let app_bundle = contents_dir.parent()?;
    (app_bundle.extension()? == "app").then(|| app_bundle.to_path_buf())
}

fn current_windows_install_dir_from_exe(exe_path: &Path) -> Option<PathBuf> {
    let file_name = exe_path.file_name()?.to_string_lossy().to_ascii_lowercase();
    (file_name == "foco.exe")
        .then(|| exe_path.parent().map(Path::to_path_buf))
        .flatten()
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let left = normalized_version(left);
    let right = normalized_version(right);
    match (numeric_version_parts(left), numeric_version_parts(right)) {
        (Some(left_parts), Some(right_parts)) => compare_numeric_parts(&left_parts, &right_parts),
        _ => left.cmp(right),
    }
}

fn normalized_version(version: &str) -> &str {
    version
        .trim()
        .strip_prefix('v')
        .or_else(|| version.trim().strip_prefix('V'))
        .unwrap_or_else(|| version.trim())
}

fn numeric_version_parts(version: &str) -> Option<Vec<u64>> {
    version
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok()
        })
        .collect()
}

fn compare_numeric_parts(left: &[u64], right: &[u64]) -> Ordering {
    let len = left.len().max(right.len());
    for index in 0..len {
        match left
            .get(index)
            .unwrap_or(&0)
            .cmp(right.get(index).unwrap_or(&0))
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_uses_numeric_dot_segments() {
        assert_eq!(compare_versions("v1.10.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.2", "v1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.0", "1.2.1"), Ordering::Less);
    }

    #[test]
    fn version_compare_falls_back_for_suffixes() {
        assert_eq!(compare_versions("1.2-beta", "1.2-alpha"), Ordering::Greater);
    }

    #[test]
    fn github_release_json_parses_expected_fields() {
        let release: GithubReleaseResponse = serde_json::from_str(
            r#"{
              "tag_name": "v1.2.3",
              "name": "Foco v1.2.3",
              "published_at": "2026-07-06T11:00:00Z",
              "html_url": "https://github.com/fonlan/foco/releases/tag/v1.2.3",
              "assets": [
                {
                  "name": "Foco-v1.2.3-macos-arm64.dmg",
                  "browser_download_url": "https://example.test/Foco-v1.2.3-macos-arm64.dmg"
                }
              ]
            }"#,
        )
        .expect("release JSON should parse");

        assert_eq!(release.tag_name, "v1.2.3");
        assert_eq!(release.name.as_deref(), Some("Foco v1.2.3"));
        assert_eq!(release.assets[0].name, "Foco-v1.2.3-macos-arm64.dmg");
    }

    #[test]
    fn platform_asset_selection_matches_current_platform() {
        let assets = vec![
            GithubReleaseAsset {
                name: "Foco-v1.2.3-macos-arm64.dmg".to_string(),
                browser_download_url: "https://example.test/macos".to_string(),
            },
            GithubReleaseAsset {
                name: "Foco-v1.2.3-windows-x64-setup.exe".to_string(),
                browser_download_url: "https://example.test/windows".to_string(),
            },
        ];
        let selected = select_platform_asset(&assets);

        if cfg!(target_os = "macos") || cfg!(windows) {
            assert!(selected.is_some());
        } else {
            assert!(selected.is_none());
        }
    }

    #[test]
    fn platform_asset_name_uses_platform_arch_suffix() {
        assert!(platform_asset_name_for(
            "Foco-v1.2.3-macos-arm64.dmg",
            "macos",
            "aarch64"
        ));
        assert!(platform_asset_name_for(
            "Foco-v1.2.3-windows-x64-setup.exe",
            "windows",
            "x86_64"
        ));
        assert!(!platform_asset_name_for(
            "Foco-v1.2.3-windows-installer.exe",
            "windows",
            "x86_64"
        ));
        assert!(!platform_asset_name_for(
            "Foco-v1.2.3-windows.zip",
            "windows",
            "x86_64"
        ));
    }

    #[test]
    fn downloaded_windows_update_requires_mz_header() {
        let dir = tempfile::tempdir().expect("tempdir");
        let valid = dir.path().join("Foco-v1.2.3-windows-x64-setup.exe");
        let invalid = dir.path().join("not-an-installer.exe");
        std::fs::write(&valid, b"MZpayload").expect("write valid exe");
        std::fs::write(&invalid, b"PK\x03\x04payload").expect("write invalid exe");

        assert!(
            validate_downloaded_update_asset("Foco-v1.2.3-windows-x64-setup.exe", &valid).is_ok()
        );
        assert!(
            validate_downloaded_update_asset("Foco-v1.2.3-windows-x64-setup.exe", &invalid)
                .is_err()
        );
    }

    #[test]
    fn update_path_components_reject_traversal() {
        assert!(safe_update_path_component("v1.2.3", "version").is_ok());
        assert!(safe_update_path_component("../v1.2.3", "version").is_err());
        assert!(safe_update_path_component("Foco-v1.2.3-macos-arm64.dmg", "asset name").is_ok());
        assert!(safe_update_path_component("nested/Foco.zip", "asset name").is_err());
    }

    #[test]
    fn macos_app_bundle_detection_requires_app_layout() {
        let bundle = Path::new("/Applications/Foco.app/Contents/MacOS/foco");
        assert_eq!(
            current_macos_app_bundle_from_exe(bundle),
            Some(PathBuf::from("/Applications/Foco.app"))
        );
        assert!(current_macos_app_bundle_from_exe(Path::new("/usr/local/bin/foco")).is_none());
    }

    #[test]
    fn macos_update_helper_launches_as_independent_launchd_job() {
        let prepared = PreparedUpdateInstall {
            version: "v1.2.3".to_string(),
            asset_name: "Foco-v1.2.3-macos-arm64.dmg".to_string(),
            asset_download_url: "https://example.test/Foco.dmg".to_string(),
            update_dir: PathBuf::from("/tmp/Foco Updates/v1.2.3"),
            archive_path: PathBuf::from("/tmp/Foco Updates/v1.2.3/Foco.dmg"),
        };
        let args = macos_update_launch_args(MacosUpdateLaunch {
            app_bundle: Path::new("/Applications/Foco.app"),
            label: "app.foco.update.42",
            pid: 42,
            prepared: &prepared,
            ready_path: Path::new("/tmp/Foco Updates/updated-restart-ready.txt"),
            result_path: Path::new("/tmp/Foco Updates/last-install-failure.txt"),
            script_path: Path::new("/tmp/Foco Updates/v1.2.3/apply-macos-update.sh"),
            started_path: Path::new("/tmp/Foco Updates/v1.2.3/helper-started.txt"),
        });
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            &args[..9],
            [
                "submit",
                "-l",
                "app.foco.update.42",
                "-o",
                "/dev/null",
                "-e",
                "/dev/null",
                "--",
                "/usr/bin/env",
            ]
        );
        assert!(args.contains(&"FOCO_UPDATE_PID=42".to_string()));
        assert!(args.contains(&"FOCO_UPDATE_DMG=/tmp/Foco Updates/v1.2.3/Foco.dmg".to_string()));
        assert!(args.contains(
            &"FOCO_UPDATE_STARTED=/tmp/Foco Updates/v1.2.3/helper-started.txt".to_string()
        ));
        assert!(args.contains(&"FOCO_UPDATE_JOB_LABEL=app.foco.update.42".to_string()));
        assert_eq!(
            &args[args.len() - 2..],
            ["/bin/sh", "/tmp/Foco Updates/v1.2.3/apply-macos-update.sh"]
        );
    }

    #[tokio::test]
    async fn macos_update_helper_start_acknowledgement_is_consumed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let marker = dir.path().join(UPDATE_HELPER_STARTED_FILE);
        std::fs::write(&marker, "pid=42\n").expect("write helper marker");

        wait_for_macos_update_helper_start(&marker)
            .await
            .expect("helper marker should be accepted");

        assert!(!marker.exists());
    }

    #[test]
    fn macos_update_helper_restarts_silently() {
        let script = macos_update_script();
        assert!(script.contains(r#"xattr -dr com.apple.quarantine "$FOCO_UPDATE_APP""#));
        assert!(script.contains(r#"open -n "$FOCO_UPDATE_APP" --args --updated-restart"#));
        assert!(script.contains("rollback_and_relaunch"));
        assert!(script.contains("FOCO_UPDATE_RESULT"));
        assert!(script.contains("FOCO_UPDATE_READY"));
        assert!(script.contains("did not become ready"));
        assert!(script.contains("quarantine attributes remain on the new app after clear"));
        assert!(script.contains("xattr -lr"));
        assert!(script.contains("fail_early"));
        assert!(script.contains("leftover update staging directories"));
        assert!(script.contains("FOCO_UPDATE_STARTED"));
        assert!(script.contains("FOCO_UPDATE_JOB_LABEL"));
        assert!(script.contains("detached update helper started"));
        assert!(script.contains("trap cleanup EXIT"));
        assert!(script.contains(r#"/bin/launchctl remove "$FOCO_UPDATE_JOB_LABEL""#));
        // Result marker must not claim rollback before restore succeeds.
        assert!(script.contains(
            r#"write_result "Update failed: $reason. Rolled back to the previous version.""#
        ));
        assert!(script.contains(r#"relaunch_app "$old_app""#));
        let replace_idx = script
            .find(r#"mv "$new_app" "$FOCO_UPDATE_APP""#)
            .expect("replace step");
        let acknowledged_idx = script
            .find(r#">"$FOCO_UPDATE_STARTED""#)
            .expect("startup acknowledgement");
        let parent_wait_idx = script
            .find(r#"while kill -0 "$FOCO_UPDATE_PID""#)
            .expect("parent wait");
        let launch_idx = script
            .find(r#"open -n "$FOCO_UPDATE_APP" --args --updated-restart"#)
            .expect("launch step");
        let ready_wait_idx = script
            .find(r#"[ ! -f "$FOCO_UPDATE_READY" ]"#)
            .expect("ready wait");
        // Prefer the final cleanup of .old after ready wait (earlier best-effort removals also match).
        let cleanup_idx = script
            .rfind(r#"rm -rf "$old_app" 2>/dev/null || true"#)
            .expect("old cleanup");
        assert!(replace_idx < launch_idx);
        assert!(launch_idx < ready_wait_idx);
        assert!(ready_wait_idx < cleanup_idx);
        assert!(acknowledged_idx < parent_wait_idx);
        // Staging cleanup must not use bare rm under set -e without recovery.
        assert!(!script.contains("rm -rf \"$new_app\" \"$old_app\"\n"));
        assert!(script.contains("leftover update staging directories"));
    }

    #[test]
    fn windows_update_helper_restarts_silently() {
        let script = windows_update_script();
        assert!(script.contains(
            "Start-Process -FilePath $ExePath -ArgumentList $ArgumentList -WorkingDirectory $workDir"
        ));
        assert!(script.contains("Restore-PreviousInstall"));
        assert!(script.contains("Fail-BeforeInstall"));
        assert!(script.contains("FOCO_UPDATE_RESULT"));
        assert!(script.contains("FOCO_UPDATE_READY"));
        assert!(script.contains("Write-UpdateResult"));
        assert!(script.contains("Foco.update-backup."));
        assert!(script.contains("Copy-InstallTree"));
        assert!(script.contains("did not become ready"));
        assert!(script.contains("Stop-InstallProcesses"));
        assert!(script.contains("could not create a full install backup"));
        let backup_idx = script
            .find("Copy-InstallTree -Source $installDir")
            .expect("backup");
        let installer_idx = script
            .find("Start-Process -FilePath $installer -ArgumentList '/S'")
            .expect("installer");
        let ready_idx = script
            .find("while (-not (Test-Path -LiteralPath $readyPath))")
            .expect("ready wait");
        let last_cleanup = script
            .rfind("Remove-PathBestEffort $backupDir")
            .expect("last cleanup");
        assert!(backup_idx < installer_idx);
        assert!(installer_idx < ready_idx);
        assert!(ready_idx < last_cleanup);
        // Fixed "$installDir.previous" can nest under Move-Item; require unique backup paths.
        assert!(!script.contains(r#"$backupDir = "$installDir.previous""#));
    }

    #[test]
    fn take_last_install_failure_reads_and_clears_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let profile = dir.path();
        let updates = profile.join(".foco").join("updates");
        std::fs::create_dir_all(&updates).expect("updates dir");
        let marker = updates.join(LAST_INSTALL_FAILURE_FILE);
        std::fs::write(
            &marker,
            "Update failed: quarantine clear failed. Rolled back.\n",
        )
        .expect("write marker");

        let message = take_last_install_failure_message(profile).expect("message");
        assert!(message.contains("quarantine clear failed"));
        assert!(!marker.exists());
        assert!(take_last_install_failure_message(profile).is_none());
    }

    #[test]
    fn load_last_install_failure_into_state_sets_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let profile = dir.path();
        let updates = profile.join(".foco").join("updates");
        std::fs::create_dir_all(&updates).expect("updates dir");
        std::fs::write(
            updates.join(LAST_INSTALL_FAILURE_FILE),
            "Update failed: could not launch the updated app after install. Rolled back to the previous version.\n",
        )
        .expect("write marker");

        let mut state = UpdateState::default();
        load_last_install_failure_into_state(&mut state, profile);
        assert_eq!(
            state.install_error.as_deref(),
            Some(
                "Update failed: could not launch the updated app after install. Rolled back to the previous version."
            )
        );
        assert!(state.error.is_none());
    }

    #[test]
    fn mark_updated_restart_ready_writes_marker_when_requested() {
        let dir = tempfile::tempdir().expect("tempdir");
        let profile = dir.path();
        // Function only writes when the current process args contain the flag; call the path
        // helper path via the public entry after simulating by writing through the same path.
        let path = updated_restart_ready_path(profile);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        // Direct path write contracts used by helpers.
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(UPDATED_RESTART_READY_FILE)
        );
        assert_eq!(
            last_install_failure_path(profile)
                .file_name()
                .and_then(|name| name.to_str()),
            Some(LAST_INSTALL_FAILURE_FILE)
        );
    }

    #[test]
    fn windows_install_dir_detection_requires_foco_exe() {
        assert_eq!(
            current_windows_install_dir_from_exe(Path::new("/opt/Foco/foco.exe")),
            Some(PathBuf::from("/opt/Foco"))
        );
        assert!(current_windows_install_dir_from_exe(Path::new("/opt/Foco/helper.exe")).is_none());
    }

    fn candidate(name: &str, secs: u64) -> UpdateVersionDirCandidate {
        UpdateVersionDirCandidate {
            name: name.to_string(),
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
        }
    }

    #[test]
    fn update_version_dir_selection_keeps_fewer_than_retain_count() {
        let candidates = vec![candidate("v1.0.0", 1), candidate("v1.1.0", 2)];
        let remove = select_update_version_dirs_to_remove(
            &candidates,
            "1.1.0",
            UPDATE_VERSION_DIR_RETAIN_COUNT,
        );
        assert!(remove.is_empty());
    }

    #[test]
    fn update_version_dir_selection_removes_excess_by_mtime() {
        let candidates = vec![
            candidate("v1.0.0", 1),
            candidate("v1.1.0", 2),
            candidate("v1.2.0", 3),
            candidate("v1.3.0", 4),
        ];
        let mut remove = select_update_version_dirs_to_remove(
            &candidates,
            "1.3.0",
            UPDATE_VERSION_DIR_RETAIN_COUNT,
        );
        remove.sort();
        assert_eq!(remove, vec!["v1.0.0".to_string(), "v1.1.0".to_string()]);
    }

    #[test]
    fn update_version_dir_selection_protects_current_when_not_newest_mtime() {
        let candidates = vec![
            candidate("v1.0.0", 10),
            candidate("v1.1.0", 1),
            candidate("v1.2.0", 5),
        ];
        // Current v1.1.0 is oldest mtime but must stay; keep one newest history (v1.0.0).
        let remove = select_update_version_dirs_to_remove(
            &candidates,
            "1.1.0",
            UPDATE_VERSION_DIR_RETAIN_COUNT,
        );
        assert_eq!(remove, vec!["v1.2.0".to_string()]);
    }

    #[test]
    fn update_version_dir_selection_breaks_mtime_ties_by_name() {
        let same = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
        let candidates = vec![
            UpdateVersionDirCandidate {
                name: "v1.0.0".to_string(),
                modified: same,
            },
            UpdateVersionDirCandidate {
                name: "v1.2.0".to_string(),
                modified: same,
            },
            UpdateVersionDirCandidate {
                name: "v1.1.0".to_string(),
                modified: same,
            },
        ];
        // Current is protected. Among equal-mtime others, sort by name ascending and keep one.
        let remove = select_update_version_dirs_to_remove(
            &candidates,
            "1.1.0",
            UPDATE_VERSION_DIR_RETAIN_COUNT,
        );
        assert_eq!(remove, vec!["v1.2.0".to_string()]);
    }

    #[test]
    fn update_version_dir_name_validation_rejects_illegal_names() {
        assert!(is_valid_update_version_dir_name("v1.2.3"));
        assert!(!is_valid_update_version_dir_name("../escape"));
        assert!(!is_valid_update_version_dir_name("nested/path"));
        assert!(!is_valid_update_version_dir_name(""));
        assert!(!is_valid_update_version_dir_name("."));
        assert!(!is_valid_update_version_dir_name(".."));
    }

    #[test]
    fn discover_update_version_dirs_skips_files_symlinks_and_illegal_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let updates = dir.path().join("updates");
        std::fs::create_dir_all(&updates).expect("updates root");
        std::fs::create_dir(updates.join("v1.0.0")).expect("version dir");
        std::fs::create_dir(updates.join("v1.1.0")).expect("version dir");
        std::fs::write(updates.join("notes.txt"), b"keep").expect("file");
        std::fs::create_dir(updates.join("not a version")).expect("illegal name dir");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(updates.join("v1.0.0"), updates.join("v-link"))
                .expect("symlink");
        }

        let candidates = discover_update_version_dir_candidates(&updates).expect("discover");
        let mut names: Vec<_> = candidates.into_iter().map(|c| c.name).collect();
        names.sort();
        assert_eq!(names, vec!["v1.0.0".to_string(), "v1.1.0".to_string()]);
        assert!(updates.join("notes.txt").is_file());
        assert!(updates.join("not a version").is_dir());
    }

    #[test]
    fn cleanup_update_version_dirs_retains_current_and_one_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let updates = dir.path().join("updates");
        std::fs::create_dir_all(&updates).expect("updates root");
        for name in ["v1.0.0", "v1.1.0", "v1.2.0", "v1.3.0"] {
            std::fs::create_dir(updates.join(name)).expect("version dir");
        }
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        for (name, offset) in [
            ("v1.0.0", 10),
            ("v1.1.0", 20),
            ("v1.2.0", 30),
            ("v1.3.0", 40),
        ] {
            let path = updates.join(name);
            let file = std::fs::File::open(&path).expect("open dir");
            file.set_modified(base + Duration::from_secs(offset))
                .expect("set mtime");
        }

        cleanup_stale_update_version_dirs(&updates, "1.3.0");

        let remaining: Vec<_> = std::fs::read_dir(&updates)
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let mut remaining = remaining;
        remaining.sort();
        assert_eq!(remaining, vec!["v1.2.0".to_string(), "v1.3.0".to_string()]);
    }

    #[test]
    fn cleanup_does_not_run_when_updated_restart_not_requested() {
        let dir = tempfile::tempdir().expect("tempdir");
        let profile = dir.path();
        let updates = profile.join(".foco").join("updates");
        std::fs::create_dir_all(updates.join("v1.0.0")).expect("v1");
        std::fs::create_dir_all(updates.join("v1.1.0")).expect("v2");
        std::fs::create_dir_all(updates.join("v1.2.0")).expect("v3");

        cleanup_stale_update_assets_if_requested(profile, false);

        assert!(updates.join("v1.0.0").is_dir());
        assert!(updates.join("v1.1.0").is_dir());
        assert!(updates.join("v1.2.0").is_dir());
    }

    #[test]
    fn cleanup_runs_when_updated_restart_requested() {
        let dir = tempfile::tempdir().expect("tempdir");
        let profile = dir.path();
        let updates = profile.join(".foco").join("updates");
        std::fs::create_dir_all(&updates).expect("updates");
        for name in ["v1.0.0", "v1.1.0", "v1.2.0"] {
            std::fs::create_dir(updates.join(name)).expect("version dir");
        }
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_100);
        for (name, offset) in [("v1.0.0", 1), ("v1.1.0", 2), ("v1.2.0", 3)] {
            let file = std::fs::File::open(updates.join(name)).expect("open");
            file.set_modified(base + Duration::from_secs(offset))
                .expect("mtime");
        }

        // Use current_version() so the protected dir matches the running package version.
        let current = current_version();
        let current_dir = format!("v{current}");
        std::fs::create_dir(updates.join(&current_dir)).expect("current version dir");
        let file = std::fs::File::open(updates.join(&current_dir)).expect("open current");
        file.set_modified(base + Duration::from_secs(0))
            .expect("mtime current");

        cleanup_stale_update_assets_if_requested(profile, true);

        assert!(
            updates.join(&current_dir).is_dir(),
            "current version must be protected"
        );
        // retain 2: current + newest history among v1.0.0/v1.1.0/v1.2.0 => v1.2.0
        assert!(updates.join("v1.2.0").is_dir());
        assert!(!updates.join("v1.0.0").exists());
        assert!(!updates.join("v1.1.0").exists());
    }

    #[test]
    fn remove_update_version_dir_refuses_path_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let updates = dir.path().join("updates");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&updates).expect("updates");
        std::fs::create_dir_all(&outside).expect("outside");
        std::fs::write(outside.join("secret.txt"), b"secret").expect("secret");

        let result = remove_update_version_dir(&updates, "../outside");
        assert!(result.is_err());
        assert!(outside.join("secret.txt").is_file());
        assert!(outside.is_dir());
    }

    #[test]
    fn remove_update_version_dir_refuses_symlink() {
        let dir = tempfile::tempdir().expect("tempdir");
        let updates = dir.path().join("updates");
        let target = dir.path().join("target");
        std::fs::create_dir_all(&updates).expect("updates");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join("keep.txt"), b"keep").expect("keep");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, updates.join("v9.9.9")).expect("symlink");
            let result = remove_update_version_dir(&updates, "v9.9.9");
            assert!(result.is_err());
            assert!(target.join("keep.txt").is_file());
            assert!(updates.join("v9.9.9").exists());
        }
        #[cfg(not(unix))]
        {
            let _ = (updates, target);
        }
    }

    #[test]
    fn cleanup_errors_do_not_panic_or_return() {
        // Missing root is a quiet no-op.
        cleanup_stale_update_version_dirs(
            Path::new("/definitely/missing/foco-updates-xyz"),
            "1.0.0",
        );
        // Root that is a file: discovery fails, must not panic.
        let dir = tempfile::tempdir().expect("tempdir");
        let file_root = dir.path().join("not-a-dir");
        std::fs::write(&file_root, b"x").expect("file root");
        cleanup_stale_update_version_dirs(&file_root, "1.0.0");
    }

    #[test]
    fn is_direct_child_path_rejects_nested_and_escape() {
        let parent = Path::new("/home/user/.foco/updates");
        assert!(is_direct_child_path(
            parent,
            &parent.join("v1.2.3"),
            "v1.2.3"
        ));
        assert!(!is_direct_child_path(
            parent,
            &parent.join("nested").join("v1.2.3"),
            "v1.2.3"
        ));
        assert!(!is_direct_child_path(
            parent,
            Path::new("/tmp/evil"),
            "evil"
        ));
        assert!(!is_direct_child_path(
            parent,
            &parent.join("v1.2.3"),
            "../x"
        ));
    }
}
