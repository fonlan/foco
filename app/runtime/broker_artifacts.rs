//! Bounded broker file-transfer artifacts (image_gen, web cache, etc.).
//!
//! Wire shape is shared; each kind applies its own MIME, size, filename, and
//! destination-directory rules. Credentials must never appear in payload or logs.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Shared wire envelope for brokered binary/text files (camelCase JSON).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BrokeredTransferFile {
    pub(crate) file_name: String,
    pub(crate) mime_type: String,
    pub(crate) bytes: usize,
    pub(crate) sha256: String,
    pub(crate) data_base64: String,
}

/// Image transfer limits (must stay aligned with image_gen materialization).
pub(crate) const MAX_BROKERED_IMAGE_FILE_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const MAX_BROKERED_IMAGE_TOTAL_BYTES: usize = 24 * 1024 * 1024;
pub(crate) const MAX_BROKERED_IMAGE_FILE_COUNT: usize = 4;

/// Web result cache transfer: single credential-free text file under `.foco/web-results/`.
/// Bound matches `MAX_WEB_FETCH_BYTES` so host-side fetch cannot smuggle larger payloads.
pub(crate) const MAX_BROKERED_WEB_RESULT_FILE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_BROKERED_WEB_RESULT_FILE_COUNT: usize = 1;
pub(crate) const BROKERED_WEB_RESULT_MIME: &str = "text/plain; charset=utf-8";
pub(crate) const WEB_RESULTS_RELATIVE_DIR: &str = ".foco/web-results";

/// Decode base64, enforce declared size and max size, verify SHA-256.
pub(crate) fn decode_and_verify_transfer_file(
    file: &BrokeredTransferFile,
    max_file_bytes: usize,
) -> Result<Vec<u8>, String> {
    if file.bytes == 0 || file.bytes > max_file_bytes {
        return Err(format!(
            "brokered transfer file '{}' has invalid size {} (max {max_file_bytes})",
            file.file_name, file.bytes
        ));
    }
    let bytes = BASE64_STANDARD
        .decode(&file.data_base64)
        .map_err(|source| format!("failed to decode brokered transfer file: {source}"))?;
    if bytes.len() != file.bytes {
        return Err(format!(
            "brokered transfer file '{}' size does not match transfer metadata",
            file.file_name
        ));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha256 != file.sha256 {
        return Err(format!(
            "brokered transfer file '{}' checksum does not match",
            file.file_name
        ));
    }
    Ok(bytes)
}

/// Package a workspace-relative file into a broker transfer envelope.
///
/// `relative_path` must already be workspace-relative (no absolute / host paths).
/// `allowed_root` is a workspace-relative directory prefix the file must stay under
/// (e.g. `.foco/web-results` or a generated image directory).
pub(crate) fn package_workspace_file_for_transfer(
    workspace_path: &Path,
    relative_path: &str,
    mime_type: &str,
    max_file_bytes: usize,
    allowed_root: &str,
) -> Result<BrokeredTransferFile, String> {
    let relative_path = normalize_workspace_relative_path(relative_path)?;
    ensure_path_under_allowed_root(&relative_path, allowed_root)?;

    let path = workspace_path.join(&relative_path);
    let canonical_path = fs::canonicalize(&path)
        .map_err(|source| format!("failed to resolve brokered transfer file: {source}"))?;
    let canonical_workspace = fs::canonicalize(workspace_path)
        .map_err(|source| format!("failed to resolve transfer workspace: {source}"))?;
    if !canonical_path.starts_with(&canonical_workspace) {
        return Err("brokered transfer file escaped the transfer directory".to_string());
    }

    let bytes = fs::read(&canonical_path)
        .map_err(|source| format!("failed to read brokered transfer file: {source}"))?;
    if bytes.is_empty() || bytes.len() > max_file_bytes {
        return Err(format!(
            "brokered transfer file exceeds the {max_file_bytes} byte limit"
        ));
    }

    let file_name = canonical_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "brokered transfer file name is invalid".to_string())?
        .to_string();
    if !is_safe_transfer_file_name(&file_name) {
        return Err("brokered transfer file name is unsafe".to_string());
    }

    Ok(BrokeredTransferFile {
        file_name,
        mime_type: mime_type.to_string(),
        bytes: bytes.len(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        data_base64: BASE64_STANDARD.encode(bytes),
    })
}

/// Atomically publish bytes to `destination` without replacing an existing file.
///
/// Uses a unique temporary name (so concurrent writers and leftover temps cannot
/// collide on a fixed path) then same-directory `hard_link` so an existing
/// destination fails with `AlreadyExists` instead of being overwritten by
/// POSIX `rename`. On any failure only the temporary file created by this call
/// is cleaned up; the destination is never deleted.
pub(crate) fn atomic_write_bytes(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "brokered transfer destination has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|source| {
        format!(
            "failed to create brokered transfer directory '{}': {source}",
            parent.display()
        )
    })?;

    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "brokered transfer destination file name is invalid".to_string())?;

    // Unique temp per attempt so concurrent writers / leftover temps do not share one path.
    const MAX_TEMP_ATTEMPTS: usize = 8;
    let mut last_error = String::from("failed to allocate a unique broker transfer temporary file");
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let temporary_path = parent.join(unique_broker_transfer_temp_name(file_name));
        match write_unique_temp_then_publish(&temporary_path, destination, bytes) {
            Ok(()) => return Ok(()),
            Err(PublishError::TempExists(message)) => {
                last_error = message;
                continue;
            }
            Err(PublishError::Other(message)) => {
                // Only this attempt's temp is cleaned; never touch destination.
                let _ = fs::remove_file(&temporary_path);
                return Err(message);
            }
        }
    }
    Err(last_error)
}

enum PublishError {
    TempExists(String),
    Other(String),
}

fn unique_broker_transfer_temp_name(file_name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let mut entropy = [0u8; 4];
    let _ = getrandom::fill(&mut entropy);
    let entropy_hex = entropy
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(".{file_name}.broker-transfer.{nanos}-{entropy_hex}.tmp")
}

fn write_unique_temp_then_publish(
    temporary_path: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), PublishError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary_path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                PublishError::TempExists(format!(
                    "brokered transfer temporary path already exists '{}'",
                    temporary_path.display()
                ))
            } else {
                PublishError::Other(format!(
                    "failed to create brokered transfer temporary file '{}': {source}",
                    temporary_path.display()
                ))
            }
        })?;
    file.write_all(bytes).map_err(|source| {
        PublishError::Other(format!(
            "failed to write brokered transfer temporary file '{}': {source}",
            temporary_path.display()
        ))
    })?;
    file.sync_all().map_err(|source| {
        PublishError::Other(format!(
            "failed to sync brokered transfer temporary file '{}': {source}",
            temporary_path.display()
        ))
    })?;
    drop(file);

    // No-clobber publish: hard_link fails with AlreadyExists if dest exists
    // (POSIX rename would replace). Same-directory hard links work on local
    // and typical remote filesystems used by workspaces.
    match fs::hard_link(temporary_path, destination) {
        Ok(()) => {
            let _ = fs::remove_file(temporary_path);
            Ok(())
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(temporary_path);
            Err(PublishError::Other(format!(
                "brokered transfer destination already exists '{}'",
                destination.display()
            )))
        }
        Err(source) => Err(PublishError::Other(format!(
            "failed to finalize brokered transfer file '{}': {source}",
            destination.display()
        ))),
    }
}

/// Safe single-component file names only (no path separators, no `..`).
pub(crate) fn is_safe_transfer_file_name(file_name: &str) -> bool {
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return false;
    }
    if file_name.contains('/') || file_name.contains('\\') {
        return false;
    }
    file_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

/// Normalize a workspace-relative path to forward slashes without leading `./`.
pub(crate) fn normalize_workspace_relative_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("brokered transfer path must not be empty".to_string());
    }
    if Path::new(trimmed).is_absolute() {
        return Err("brokered transfer path must be workspace-relative".to_string());
    }
    // Reject Windows drive / UNC style absolute paths even when Path::is_absolute is false on Unix.
    if trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || (trimmed.len() >= 2
            && trimmed.as_bytes()[0].is_ascii_alphabetic()
            && trimmed.as_bytes()[1] == b':')
    {
        return Err("brokered transfer path must be workspace-relative".to_string());
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| "brokered transfer path contains invalid UTF-8".to_string())?;
                if part == ".." || part == "." {
                    return Err("brokered transfer path must not contain '.' or '..'".to_string());
                }
                normalized.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("brokered transfer path must be workspace-relative".to_string());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("brokered transfer path must not be empty".to_string());
    }
    Ok(normalized.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn ensure_path_under_allowed_root(
    relative_path: &str,
    allowed_root: &str,
) -> Result<(), String> {
    let root = normalize_workspace_relative_path(allowed_root)?;
    let path = normalize_workspace_relative_path(relative_path)?;
    if path == root || path.starts_with(&format!("{root}/")) {
        Ok(())
    } else {
        Err(format!(
            "brokered transfer path '{path}' is outside allowed directory '{root}'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_absolute_and_parent_paths() {
        assert!(normalize_workspace_relative_path("/tmp/x").is_err());
        assert!(normalize_workspace_relative_path("C:\\tmp\\x").is_err());
        assert!(normalize_workspace_relative_path("../escape").is_err());
        assert!(normalize_workspace_relative_path(".foco/../etc/passwd").is_err());
        assert_eq!(
            normalize_workspace_relative_path(".foco/web-results/a.txt").expect("ok"),
            ".foco/web-results/a.txt"
        );
    }

    #[test]
    fn package_and_decode_roundtrip() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let relative = ".foco/web-results/web-fetch-1.txt";
        let abs = workspace.path().join(relative);
        fs::create_dir_all(abs.parent().expect("parent")).expect("mkdir");
        fs::write(&abs, b"hello cache").expect("write");

        let packaged = package_workspace_file_for_transfer(
            workspace.path(),
            relative,
            BROKERED_WEB_RESULT_MIME,
            MAX_BROKERED_WEB_RESULT_FILE_BYTES,
            WEB_RESULTS_RELATIVE_DIR,
        )
        .expect("package");
        assert_eq!(packaged.file_name, "web-fetch-1.txt");
        assert_eq!(packaged.mime_type, BROKERED_WEB_RESULT_MIME);
        assert_eq!(packaged.bytes, 11);

        let bytes = decode_and_verify_transfer_file(&packaged, MAX_BROKERED_WEB_RESULT_FILE_BYTES)
            .expect("decode");
        assert_eq!(bytes, b"hello cache");
    }

    #[test]
    fn package_rejects_path_outside_allowed_root() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let relative = ".foco/image-gen/x.png";
        let abs = workspace.path().join(relative);
        fs::create_dir_all(abs.parent().expect("parent")).expect("mkdir");
        fs::write(&abs, b"x").expect("write");

        let error = package_workspace_file_for_transfer(
            workspace.path(),
            relative,
            BROKERED_WEB_RESULT_MIME,
            MAX_BROKERED_WEB_RESULT_FILE_BYTES,
            WEB_RESULTS_RELATIVE_DIR,
        )
        .expect_err("outside root");
        assert!(error.contains("outside allowed directory"), "{error}");
    }

    #[test]
    fn decode_rejects_checksum_mismatch() {
        let file = BrokeredTransferFile {
            file_name: "a.txt".to_string(),
            mime_type: BROKERED_WEB_RESULT_MIME.to_string(),
            bytes: 4,
            sha256: "deadbeef".to_string(),
            data_base64: BASE64_STANDARD.encode(b"test"),
        };
        let error = decode_and_verify_transfer_file(&file, MAX_BROKERED_WEB_RESULT_FILE_BYTES)
            .expect_err("checksum");
        assert!(error.contains("checksum"), "{error}");
    }

    #[test]
    fn atomic_write_creates_file() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let dest = workspace
            .path()
            .join(".foco/web-results/web-fetch-atomic.txt");
        atomic_write_bytes(&dest, b"payload").expect("write");
        assert_eq!(fs::read(&dest).expect("read"), b"payload");
    }

    #[test]
    fn atomic_write_does_not_overwrite_or_delete_existing_destination() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let dest = workspace
            .path()
            .join(".foco/web-results/web-fetch-existing.txt");
        fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
        fs::write(&dest, b"original").expect("seed");

        // Pre-create a fixed-style leftover temp to ensure unique temps still succeed
        // for the new write path, while destination remains protected.
        let leftover = dest
            .parent()
            .expect("parent")
            .join(".web-fetch-existing.txt.broker-transfer.tmp");
        fs::write(&leftover, b"stale").expect("leftover temp");

        let error = atomic_write_bytes(&dest, b"replacement").expect_err("no clobber");
        assert!(
            error.contains("already exists"),
            "expected already-exists error, got: {error}"
        );
        assert_eq!(fs::read(&dest).expect("read"), b"original");
        // Leftover fixed temp must not be removed by a failed write (not ours).
        assert!(leftover.exists(), "must not delete unrelated temp files");
    }

    #[test]
    fn atomic_write_failure_on_temp_collision_does_not_delete_destination() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let dest = workspace
            .path()
            .join(".foco/web-results/web-fetch-keep.txt");
        fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
        fs::write(&dest, b"keep-me").expect("seed");

        // Destination already present: publish must fail without removing it.
        let error = atomic_write_bytes(&dest, b"new").expect_err("exists");
        assert!(error.contains("already exists"), "{error}");
        assert_eq!(fs::read(&dest).expect("read"), b"keep-me");
    }
}
