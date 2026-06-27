use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::{Deserialize, Serialize};

use crate::{ApiError, display_path};

// GitHub API endpoint used to find the latest ripgrep release for auto-install.
const RIPGREP_RELEASE_API_URL: &str =
    "https://api.github.com/repos/BurntSushi/ripgrep/releases/latest";
// Temporary archive filename used while downloading ripgrep.
const RIPGREP_DOWNLOAD_ARCHIVE_NAME: &str = "ripgrep-download.tmp";
// Temporary directory name used while extracting a downloaded ripgrep archive.
const RIPGREP_EXTRACT_DIR_NAME: &str = "ripgrep-extract";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug)]
pub(crate) struct RipgrepStatus {
    pub(crate) available: bool,
    pub(crate) path: Option<PathBuf>,
    pub(crate) install_dir: PathBuf,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RipgrepToolSummary {
    available: bool,
    path: Option<String>,
    install_dir: String,
}

#[derive(Deserialize)]
struct GithubReleaseResponse {
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
pub(crate) struct GithubReleaseAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
}

pub(crate) fn detect_ripgrep(foco_root_dir: &Path) -> RipgrepStatus {
    let install_dir = ripgrep_install_dir(foco_root_dir);
    let path = installed_ripgrep_path(&install_dir)
        .filter(|path| ripgrep_executable_works(path))
        .or_else(find_system_ripgrep);

    RipgrepStatus {
        available: path.is_some(),
        path,
        install_dir,
    }
}

pub(crate) fn ripgrep_install_dir(foco_root_dir: &Path) -> PathBuf {
    foco_root_dir.join("bin")
}

fn installed_ripgrep_path(install_dir: &Path) -> Option<PathBuf> {
    let candidate = install_dir.join(ripgrep_executable_name());

    candidate.is_file().then_some(candidate)
}

pub(crate) fn find_system_ripgrep() -> Option<PathBuf> {
    ["rg", "ripgrep"].into_iter().find_map(|command| {
        find_command_in_path(command).filter(|path| ripgrep_executable_works(path))
    })
}

fn find_command_in_path(command: &str) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 {
        return command_path.is_file().then(|| command_path.to_path_buf());
    }

    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            command_candidate_names(command)
                .into_iter()
                .map(|name| dir.join(name))
                .find(|candidate| candidate.is_file())
        })
    })
}

fn command_candidate_names(command: &str) -> Vec<String> {
    if cfg!(windows) && Path::new(command).extension().is_none() {
        vec![
            format!("{command}.exe"),
            format!("{command}.cmd"),
            format!("{command}.bat"),
        ]
    } else {
        vec![command.to_string()]
    }
}

pub(crate) fn ripgrep_executable_name() -> &'static str {
    if cfg!(windows) { "rg.exe" } else { "rg" }
}

fn ripgrep_executable_works(path: &Path) -> bool {
    let mut command = Command::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command.status().is_ok_and(|status| status.success())
}

pub(crate) async fn download_and_install_ripgrep(
    install_dir: &Path,
) -> Result<RipgrepStatus, ApiError> {
    fs::create_dir_all(install_dir).map_err(|source| {
        ApiError::internal(format!(
            "failed to create ripgrep install directory {}: {source}",
            install_dir.display()
        ))
    })?;

    let asset = select_ripgrep_asset(fetch_latest_ripgrep_release().await?.assets)?;
    let archive_path = install_dir.join(RIPGREP_DOWNLOAD_ARCHIVE_NAME);
    let extract_dir = install_dir.join(RIPGREP_EXTRACT_DIR_NAME);
    download_file(&asset.browser_download_url, &archive_path).await?;

    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).map_err(|source| {
            ApiError::internal(format!(
                "failed to clear temporary ripgrep extraction directory {}: {source}",
                extract_dir.display()
            ))
        })?;
    }
    fs::create_dir_all(&extract_dir).map_err(|source| {
        ApiError::internal(format!(
            "failed to create temporary ripgrep extraction directory {}: {source}",
            extract_dir.display()
        ))
    })?;

    extract_ripgrep_archive(&asset.name, &archive_path, &extract_dir)?;
    let extracted_rg = find_extracted_ripgrep(&extract_dir).ok_or_else(|| {
        ApiError::internal(format!(
            "ripgrep archive '{}' did not contain {}",
            asset.name,
            ripgrep_executable_name()
        ))
    })?;
    if !ripgrep_executable_works(&extracted_rg) {
        return Err(ApiError::internal(format!(
            "downloaded ripgrep executable failed --version: {}",
            extracted_rg.display()
        )));
    }

    let final_path = install_dir.join(ripgrep_executable_name());
    fs::copy(&extracted_rg, &final_path).map_err(|source| {
        ApiError::internal(format!(
            "failed to install ripgrep to {}: {source}",
            final_path.display()
        ))
    })?;
    #[cfg(unix)]
    set_executable_permissions(&final_path)?;

    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_dir_all(&extract_dir);

    if !ripgrep_executable_works(&final_path) {
        return Err(ApiError::internal(format!(
            "installed ripgrep executable failed --version: {}",
            final_path.display()
        )));
    }

    Ok(RipgrepStatus {
        available: true,
        path: Some(final_path),
        install_dir: install_dir.to_path_buf(),
    })
}

async fn fetch_latest_ripgrep_release() -> Result<GithubReleaseResponse, ApiError> {
    reqwest::Client::new()
        .get(RIPGREP_RELEASE_API_URL)
        .header(reqwest::header::USER_AGENT, "foco")
        .send()
        .await
        .map_err(|source| ApiError::internal(format!("failed to fetch ripgrep release: {source}")))?
        .error_for_status()
        .map_err(|source| ApiError::internal(format!("ripgrep release request failed: {source}")))?
        .json::<GithubReleaseResponse>()
        .await
        .map_err(|source| ApiError::internal(format!("failed to parse ripgrep release: {source}")))
}

async fn download_file(url: &str, destination: &Path) -> Result<(), ApiError> {
    let bytes = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::USER_AGENT, "foco")
        .send()
        .await
        .map_err(|source| ApiError::internal(format!("failed to download ripgrep: {source}")))?
        .error_for_status()
        .map_err(|source| ApiError::internal(format!("ripgrep download failed: {source}")))?
        .bytes()
        .await
        .map_err(|source| {
            ApiError::internal(format!("failed to read ripgrep download: {source}"))
        })?;

    fs::write(destination, bytes).map_err(|source| {
        ApiError::internal(format!(
            "failed to save ripgrep download to {}: {source}",
            destination.display()
        ))
    })
}

pub(crate) fn select_ripgrep_asset(
    assets: Vec<GithubReleaseAsset>,
) -> Result<GithubReleaseAsset, ApiError> {
    let target = ripgrep_asset_target()?;
    let archive_suffix = if cfg!(windows) { ".zip" } else { ".tar.gz" };

    assets
        .into_iter()
        .find(|asset| {
            let name = asset.name.as_str();
            name.starts_with("ripgrep-")
                && name.contains(target)
                && name.ends_with(archive_suffix)
                && !name.ends_with(".sha256")
        })
        .ok_or_else(|| {
            ApiError::internal(format!(
                "no ripgrep release asset matched platform target '{target}'"
            ))
        })
}

pub(crate) fn ripgrep_asset_target() -> Result<&'static str, ApiError> {
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Ok("aarch64-pc-windows-msvc"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        (os, arch) => Err(ApiError::internal(format!(
            "automatic ripgrep download is unsupported on {os}/{arch}"
        ))),
    }
}

fn extract_ripgrep_archive(
    asset_name: &str,
    archive_path: &Path,
    extract_dir: &Path,
) -> Result<(), ApiError> {
    if asset_name.ends_with(".tar.gz") {
        let archive_file = fs::File::open(archive_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to open ripgrep archive {}: {source}",
                archive_path.display()
            ))
        })?;
        let decoder = flate2::read::GzDecoder::new(archive_file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(extract_dir).map_err(|source| {
            ApiError::internal(format!(
                "failed to extract ripgrep archive {}: {source}",
                archive_path.display()
            ))
        })?;
        return Ok(());
    }

    if asset_name.ends_with(".zip") {
        return extract_zip_with_powershell(archive_path, extract_dir);
    }

    Err(ApiError::internal(format!(
        "unsupported ripgrep archive format: {asset_name}"
    )))
}

fn extract_zip_with_powershell(archive_path: &Path, extract_dir: &Path) -> Result<(), ApiError> {
    let output = Command::new("powershell.exe")
        .env("FOCO_RIPGREP_ARCHIVE", archive_path)
        .env("FOCO_RIPGREP_EXTRACT_DIR", extract_dir)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-Command",
            "Expand-Archive -LiteralPath $env:FOCO_RIPGREP_ARCHIVE -DestinationPath $env:FOCO_RIPGREP_EXTRACT_DIR -Force",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to launch PowerShell to extract ripgrep: {source}"
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(ApiError::internal(format!(
        "failed to extract ripgrep archive{}",
        if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        }
    )))
}

fn find_extracted_ripgrep(extract_dir: &Path) -> Option<PathBuf> {
    find_file_by_name(extract_dir, ripgrep_executable_name())
}

fn find_file_by_name(root: &Path, file_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|name| name == file_name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_by_name(&path, file_name) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<(), ApiError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to read ripgrep permissions {}: {source}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|source| {
        ApiError::internal(format!(
            "failed to set ripgrep executable permissions {}: {source}",
            path.display()
        ))
    })
}

pub(crate) fn ripgrep_tool_summary(status: &RipgrepStatus) -> RipgrepToolSummary {
    RipgrepToolSummary {
        available: status.available,
        path: status.path.as_deref().map(display_path),
        install_dir: display_path(&status.install_dir),
    }
}
