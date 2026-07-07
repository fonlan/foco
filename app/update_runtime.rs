use std::{
    cmp::Ordering,
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::{DateTime, SecondsFormat, Utc};
use foco_store::config::GlobalConfig;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::watch};

use crate::{ApiError, AppState, config_snapshot, save_config};

const UPDATE_RELEASE_API_URL: &str = "https://api.github.com/repos/fonlan/foco/releases/latest";
const UPDATE_CHECK_STARTUP_DELAY_SECS: u64 = 30;
const UPDATE_CHECK_INTERVAL_SECS: u64 = 12 * 60 * 60;
const UPDATE_DOWNLOAD_TMP_SUFFIX: &str = ".download.tmp";
const UPDATE_SHUTDOWN_DELAY_MS: u64 = 250;
const UPDATE_FORCE_EXIT_DELAY_SECS: u64 = 10;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug, Default)]
pub(crate) struct UpdateState {
    // ponytail: update check history is intentionally in memory; persist it if users need history.
    last_checked_at: Option<DateTime<Utc>>,
    latest_release: Option<UpdateRelease>,
    error: Option<String>,
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

pub(crate) fn save_update_settings(
    state: &AppState,
    auto_check_enabled: bool,
) -> Result<UpdateStatusSummary, ApiError> {
    let mut config = config_snapshot(state)?;
    config.app.auto_update_check_enabled = auto_check_enabled;
    save_config(state, config.clone())?;
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
    start_update_helper(&prepared)?;
    request_shutdown_after_update_helper_started(state.app_shutdown_tx.clone());

    Ok(summary)
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
        error: update_state.error.clone(),
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

fn start_update_helper(prepared: &PreparedUpdateInstall) -> Result<(), ApiError> {
    if cfg!(target_os = "macos") {
        start_macos_update_helper(prepared)
    } else if cfg!(windows) {
        start_windows_update_helper(prepared)
    } else {
        Err(ApiError::bad_request(
            "update installation is not supported on this platform",
        ))
    }
}

fn start_macos_update_helper(prepared: &PreparedUpdateInstall) -> Result<(), ApiError> {
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

    let mut command = Command::new("/bin/sh");
    command
        .arg(&script_path)
        .env("FOCO_UPDATE_DMG", &prepared.archive_path)
        .env("FOCO_UPDATE_APP", &app_bundle)
        .env("FOCO_UPDATE_PID", std::process::id().to_string())
        .env(
            "FOCO_UPDATE_LOG",
            prepared
                .update_dir
                .join(format!("apply-{}.log", prepared.version)),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn().map_err(|source| {
        ApiError::internal(format!(
            "failed to start Foco macOS update helper: {source}"
        ))
    })?;
    Ok(())
}

fn macos_update_script() -> &'static str {
    r#"#!/bin/sh
set -eu
: "${FOCO_UPDATE_DMG:?}"
: "${FOCO_UPDATE_APP:?}"
: "${FOCO_UPDATE_PID:?}"
: "${FOCO_UPDATE_LOG:?}"
exec >>"$FOCO_UPDATE_LOG" 2>&1
while kill -0 "$FOCO_UPDATE_PID" 2>/dev/null; do
  sleep 1
done
mount_output="$(hdiutil attach "$FOCO_UPDATE_DMG" -nobrowse -readonly)"
volume="$(printf '%s\n' "$mount_output" | awk '/\/Volumes\// { sub(/^.*\/Volumes\//, "/Volumes/"); print; exit }')"
if [ -z "$volume" ]; then
  echo "failed to locate mounted Foco update volume"
  exit 1
fi
trap 'hdiutil detach "$volume" >/dev/null 2>&1 || true' EXIT
source_app="$(find "$volume" -maxdepth 2 -name "Foco.app" -type d | head -n 1)"
if [ -z "$source_app" ]; then
  echo "Foco.app was not found in update dmg"
  exit 1
fi
new_app="$FOCO_UPDATE_APP.new"
old_app="$FOCO_UPDATE_APP.old"
rm -rf "$new_app" "$old_app"
ditto "$source_app" "$new_app"
if [ -d "$FOCO_UPDATE_APP" ]; then
  mv "$FOCO_UPDATE_APP" "$old_app"
fi
mv "$new_app" "$FOCO_UPDATE_APP"
rm -rf "$old_app"
hdiutil detach "$volume"
trap - EXIT
open -n "$FOCO_UPDATE_APP"
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
if (-not $installer -or -not $installDir -or -not $currentExe -or -not $pidToWait -or -not $logPath) { throw "missing Foco update environment" }
Start-Transcript -Path $logPath -Append | Out-Null
try {
  Wait-Process -Id $pidToWait
  $process = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
  if ($process.ExitCode -ne 0) { throw "Foco installer exited with code $($process.ExitCode)" }
  Start-Process -FilePath $currentExe -WorkingDirectory $installDir
} finally {
  Stop-Transcript | Out-Null
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
    fn windows_install_dir_detection_requires_foco_exe() {
        assert_eq!(
            current_windows_install_dir_from_exe(Path::new("/opt/Foco/foco.exe")),
            Some(PathBuf::from("/opt/Foco"))
        );
        assert!(current_windows_install_dir_from_exe(Path::new("/opt/Foco/helper.exe")).is_none());
    }
}
