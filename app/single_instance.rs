use std::{
    fmt, fs,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(unix)]
use std::os::{
    fd::AsRawFd,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};

const LOCK_FILE_NAME: &str = ".foco-single-instance.lock";
const METADATA_FILE_NAME: &str = ".foco-single-instance.json";
const METADATA_VERSION: u8 = 1;
const PRIVATE_FILE_MODE: u32 = 0o600;
const METADATA_TEMP_FILE_ATTEMPTS: u64 = 16;

static NEXT_METADATA_TEMP_SUFFIX: AtomicU64 = AtomicU64::new(1);

/// Outcome of attempting to acquire the per-configuration-directory process lock.
#[must_use]
pub(crate) enum SingleInstanceAcquire {
    Acquired(SingleInstanceGuard),
    Contended,
}

/// Holds the OS file lock for one Foco configuration directory.
///
/// Dropping this guard closes the lock file and releases the operating-system lock. The metadata
/// file deliberately remains behind because it is diagnostic data, not a lock.
#[must_use]
pub(crate) struct SingleInstanceGuard {
    _lock_file: fs::File,
    metadata_path: PathBuf,
    pid: u32,
}

impl SingleInstanceGuard {
    /// Publishes the listener selected by the process that already owns the OS lock.
    pub(crate) fn publish_ready(
        &self,
        listen_addr: SocketAddr,
    ) -> Result<(), SingleInstanceMetadataError> {
        write_metadata_atomically(
            &self.metadata_path,
            &SingleInstanceMetadata::ready(self.pid, listen_addr),
        )
    }

    #[cfg(test)]
    fn metadata_path(&self) -> &Path {
        &self.metadata_path
    }
}

/// Minimal diagnostic metadata for a process that owns the single-instance lock.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SingleInstanceMetadata {
    pub(crate) version: u8,
    pub(crate) pid: u32,
    pub(crate) state: SingleInstanceState,
    pub(crate) listen_addr: Option<SocketAddr>,
}

impl SingleInstanceMetadata {
    fn starting(pid: u32) -> Self {
        Self {
            version: METADATA_VERSION,
            pid,
            state: SingleInstanceState::Starting,
            listen_addr: None,
        }
    }

    fn ready(pid: u32, listen_addr: SocketAddr) -> Self {
        Self {
            version: METADATA_VERSION,
            pid,
            state: SingleInstanceState::Ready,
            listen_addr: Some(listen_addr),
        }
    }

    fn validate(&self, path: &Path) -> Result<(), SingleInstanceMetadataError> {
        if self.version != METADATA_VERSION {
            return Err(SingleInstanceMetadataError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: self.version,
            });
        }

        let state_is_valid = match self.state {
            SingleInstanceState::Starting => self.listen_addr.is_none(),
            SingleInstanceState::Ready => self.listen_addr.is_some(),
        };
        if state_is_valid {
            Ok(())
        } else {
            Err(SingleInstanceMetadataError::Invalid {
                path: path.to_path_buf(),
                message: "metadata state and listenAddr are inconsistent".to_owned(),
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SingleInstanceState {
    Starting,
    Ready,
}

#[derive(Debug)]
pub(crate) enum SingleInstanceError {
    Io { path: PathBuf, source: io::Error },
    UnsupportedPlatform,
    Metadata(SingleInstanceMetadataError),
}

impl fmt::Display for SingleInstanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "single-instance lock failed for {}: {source}", path.display())
            }
            Self::UnsupportedPlatform => formatter.write_str(
                "single-instance locking is not supported on this platform; refusing to fall back to a port lock",
            ),
            Self::Metadata(source) => write!(formatter, "single-instance metadata failed: {source}"),
        }
    }
}

impl std::error::Error for SingleInstanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Metadata(source) => Some(source),
            Self::UnsupportedPlatform => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SingleInstanceMetadataError {
    Io { path: PathBuf, source: io::Error },
    Invalid { path: PathBuf, message: String },
    UnsupportedVersion { path: PathBuf, version: u8 },
}

impl fmt::Display for SingleInstanceMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "metadata I/O failed for {}: {source}",
                    path.display()
                )
            }
            Self::Invalid { path, message } => {
                write!(
                    formatter,
                    "metadata is invalid at {}: {message}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => write!(
                formatter,
                "metadata at {} uses unsupported version {version}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SingleInstanceMetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid { .. } | Self::UnsupportedVersion { .. } => None,
        }
    }
}

/// Acquires a non-blocking exclusive lock scoped to the canonical configuration directory.
///
/// A successful acquisition immediately replaces any stale metadata with `starting` metadata.
pub(crate) fn acquire_single_instance(
    config_root: impl AsRef<Path>,
) -> Result<SingleInstanceAcquire, SingleInstanceError> {
    let config_root = resolve_config_root(config_root.as_ref())?;
    let lock_path = config_root.join(LOCK_FILE_NAME);
    let metadata_path = config_root.join(METADATA_FILE_NAME);
    let lock_file = open_private_lock_file(&lock_path)?;

    match lock_file_nonblocking(&lock_file) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            return Err(SingleInstanceError::UnsupportedPlatform);
        }
        Err(error) if is_lock_contention(&error) => return Ok(SingleInstanceAcquire::Contended),
        Err(source) => {
            return Err(SingleInstanceError::Io {
                path: lock_path,
                source,
            });
        }
    }

    let guard = SingleInstanceGuard {
        _lock_file: lock_file,
        metadata_path,
        pid: std::process::id(),
    };
    write_metadata_atomically(
        &guard.metadata_path,
        &SingleInstanceMetadata::starting(guard.pid),
    )
    .map_err(SingleInstanceError::Metadata)?;

    Ok(SingleInstanceAcquire::Acquired(guard))
}

/// Reads diagnostic metadata without inferring whether another process still owns the lock.
pub(crate) fn read_single_instance_metadata(
    config_root: impl AsRef<Path>,
) -> Result<Option<SingleInstanceMetadata>, SingleInstanceMetadataError> {
    let config_root = match fs::canonicalize(config_root.as_ref()) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SingleInstanceMetadataError::Io {
                path: config_root.as_ref().to_path_buf(),
                source,
            });
        }
    };
    let metadata_path = config_root.join(METADATA_FILE_NAME);
    let content = match fs::read(&metadata_path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SingleInstanceMetadataError::Io {
                path: metadata_path,
                source,
            });
        }
    };
    let metadata: SingleInstanceMetadata = serde_json::from_slice(&content).map_err(|source| {
        SingleInstanceMetadataError::Invalid {
            path: metadata_path.clone(),
            message: source.to_string(),
        }
    })?;
    metadata.validate(&metadata_path)?;

    Ok(Some(metadata))
}

fn resolve_config_root(config_root: &Path) -> Result<PathBuf, SingleInstanceError> {
    fs::create_dir_all(config_root).map_err(|source| SingleInstanceError::Io {
        path: config_root.to_path_buf(),
        source,
    })?;
    fs::canonicalize(config_root).map_err(|source| SingleInstanceError::Io {
        path: config_root.to_path_buf(),
        source,
    })
}

fn open_private_lock_file(path: &Path) -> Result<fs::File, SingleInstanceError> {
    let mut options = fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    options.mode(PRIVATE_FILE_MODE);
    let file = options
        .open(path)
        .map_err(|source| SingleInstanceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    set_private_file_permissions(path).map_err(|source| SingleInstanceError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(file)
}

fn write_metadata_atomically(
    metadata_path: &Path,
    metadata: &SingleInstanceMetadata,
) -> Result<(), SingleInstanceMetadataError> {
    let encoded =
        serde_json::to_vec(metadata).map_err(|source| SingleInstanceMetadataError::Invalid {
            path: metadata_path.to_path_buf(),
            message: source.to_string(),
        })?;
    let (temporary_path, mut file) = create_metadata_temp_file(metadata_path)?;
    let write_result = file
        .write_all(&encoded)
        .map_err(|source| SingleInstanceMetadataError::Io {
            path: temporary_path.clone(),
            source,
        })
        .and_then(|()| {
            file.sync_all()
                .map_err(|source| SingleInstanceMetadataError::Io {
                    path: temporary_path.clone(),
                    source,
                })
        });
    drop(file);
    let write_result =
        write_result.and_then(|()| replace_metadata_file(&temporary_path, metadata_path));

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn create_metadata_temp_file(
    metadata_path: &Path,
) -> Result<(PathBuf, fs::File), SingleInstanceMetadataError> {
    let parent = metadata_path
        .parent()
        .ok_or_else(|| SingleInstanceMetadataError::Invalid {
            path: metadata_path.to_path_buf(),
            message: "metadata path has no parent directory".to_owned(),
        })?;

    for _ in 0..METADATA_TEMP_FILE_ATTEMPTS {
        let suffix = NEXT_METADATA_TEMP_SUFFIX.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(
            ".foco-single-instance-{}-{suffix}.tmp",
            std::process::id()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(PRIVATE_FILE_MODE);
        match options.open(&temporary_path) {
            Ok(file) => {
                set_private_file_permissions(&temporary_path).map_err(|source| {
                    SingleInstanceMetadataError::Io {
                        path: temporary_path.clone(),
                        source,
                    }
                })?;
                return Ok((temporary_path, file));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(SingleInstanceMetadataError::Io {
                    path: temporary_path,
                    source,
                });
            }
        }
    }

    Err(SingleInstanceMetadataError::Io {
        path: metadata_path.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique metadata temporary file",
        ),
    })
}

#[cfg(unix)]
fn replace_metadata_file(
    temporary_path: &Path,
    metadata_path: &Path,
) -> Result<(), SingleInstanceMetadataError> {
    fs::rename(temporary_path, metadata_path).map_err(|source| SingleInstanceMetadataError::Io {
        path: metadata_path.to_path_buf(),
        source,
    })
}

#[cfg(windows)]
fn replace_metadata_file(
    temporary_path: &Path,
    metadata_path: &Path,
) -> Result<(), SingleInstanceMetadataError> {
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    let destination_path = metadata_path.to_path_buf();
    let temporary_path: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let metadata_path: Vec<u16> = metadata_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let moved = unsafe {
        MoveFileExW(
            temporary_path.as_ptr(),
            metadata_path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved != 0 {
        Ok(())
    } else {
        Err(SingleInstanceMetadataError::Io {
            path: destination_path,
            source: io::Error::last_os_error(),
        })
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_metadata_file(
    temporary_path: &Path,
    metadata_path: &Path,
) -> Result<(), SingleInstanceMetadataError> {
    fs::rename(temporary_path, metadata_path).map_err(|source| SingleInstanceMetadataError::Io {
        path: metadata_path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    let current_mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if current_mode != PRIVATE_FILE_MODE {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn lock_file_nonblocking(file: &fs::File) -> io::Result<()> {
    const LOCK_EX: std::os::raw::c_int = 2;
    const LOCK_NB: std::os::raw::c_int = 4;
    let result = libc_flock(file.as_raw_fd(), LOCK_EX | LOCK_NB);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

#[cfg(unix)]
fn libc_flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int {
    unsafe { flock(fd, operation) }
}

#[cfg(windows)]
fn lock_file_nonblocking(file: &fs::File) -> io::Result<()> {
    use std::ptr;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut std::ffi::c_void,
    }

    unsafe extern "system" {
        fn LockFileEx(
            file: *mut std::ffi::c_void,
            flags: u32,
            reserved: u32,
            bytes_to_lock_low: u32,
            bytes_to_lock_high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }

    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x0000_0001;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: ptr::null_mut(),
    };
    let locked = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if locked != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
fn lock_file_nonblocking(_file: &fs::File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "single-instance file locking is not supported on this platform",
    ))
}

fn is_lock_contention(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::WouldBlock {
        return true;
    }

    #[cfg(unix)]
    {
        matches!(error.raw_os_error(), Some(11 | 35))
    }
    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(33))
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn acquired(root: &Path) -> SingleInstanceGuard {
        match acquire_single_instance(root).expect("single-instance acquisition should not error") {
            SingleInstanceAcquire::Acquired(guard) => guard,
            SingleInstanceAcquire::Contended => panic!("expected acquisition, found contention"),
        }
    }

    #[test]
    fn acquire_returns_contention_until_the_guard_is_dropped() {
        let directory = tempdir().expect("temporary configuration directory should be created");
        let first = acquired(directory.path());

        let second = acquire_single_instance(directory.path())
            .expect("second acquisition should report contention rather than error");
        assert!(matches!(second, SingleInstanceAcquire::Contended));

        drop(first);
        let _third = acquired(directory.path());
    }

    #[test]
    fn acquire_allows_independent_configuration_directories() {
        let first_directory =
            tempdir().expect("first temporary configuration directory should be created");
        let second_directory =
            tempdir().expect("second temporary configuration directory should be created");

        let _first = acquired(first_directory.path());
        let _second = acquired(second_directory.path());
    }

    #[test]
    fn acquire_replaces_stale_metadata_with_starting_state() {
        let directory = tempdir().expect("temporary configuration directory should be created");
        let lock_path = directory.path().join(LOCK_FILE_NAME);
        fs::write(&lock_path, "stale lockfile").expect("stale lock file should be written");
        let metadata_path = directory.path().join(METADATA_FILE_NAME);
        write_metadata_atomically(
            &metadata_path,
            &SingleInstanceMetadata::ready(42, "127.0.0.1:3210".parse().expect("valid address")),
        )
        .expect("stale metadata should be written");

        let guard = acquired(directory.path());
        let metadata = read_single_instance_metadata(directory.path())
            .expect("current metadata should be readable")
            .expect("current metadata should exist");

        assert_eq!(
            metadata,
            SingleInstanceMetadata::starting(std::process::id())
        );
        let expected_metadata_path = directory
            .path()
            .canonicalize()
            .expect("temporary directory should resolve")
            .join(METADATA_FILE_NAME);
        assert_eq!(guard.metadata_path(), expected_metadata_path);
    }

    #[test]
    fn publish_ready_replaces_starting_metadata_with_listener_address() {
        let directory = tempdir().expect("temporary configuration directory should be created");
        let guard = acquired(directory.path());
        let listen_addr = "127.0.0.1:4321".parse().expect("valid address");

        guard
            .publish_ready(listen_addr)
            .expect("ready metadata should be published");
        let metadata = read_single_instance_metadata(directory.path())
            .expect("ready metadata should be readable")
            .expect("ready metadata should exist");

        assert_eq!(
            metadata,
            SingleInstanceMetadata::ready(std::process::id(), listen_addr)
        );
    }

    #[test]
    fn read_metadata_rejects_partial_data_without_claiming_lock_ownership() {
        let directory = tempdir().expect("temporary configuration directory should be created");
        let metadata_path = directory.path().join(METADATA_FILE_NAME);
        fs::write(&metadata_path, b"{\"version\":1").expect("partial metadata should be written");

        let read_result = read_single_instance_metadata(directory.path());
        assert!(matches!(
            read_result,
            Err(SingleInstanceMetadataError::Invalid { .. })
        ));

        let _guard = acquired(directory.path());
    }

    #[test]
    fn read_metadata_rejects_unknown_version() {
        let directory = tempdir().expect("temporary configuration directory should be created");
        let metadata_path = directory.path().join(METADATA_FILE_NAME);
        fs::write(
            &metadata_path,
            r#"{"version":2,"pid":42,"state":"starting","listenAddr":null}"#,
        )
        .expect("unknown-version metadata should be written");

        let read_result = read_single_instance_metadata(directory.path());
        assert!(matches!(
            read_result,
            Err(SingleInstanceMetadataError::UnsupportedVersion { version: 2, .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn is_lock_contention_recognizes_unix_would_block_errors() {
        let error = io::Error::from_raw_os_error(11);

        assert!(is_lock_contention(&error));
    }

    #[cfg(windows)]
    #[test]
    fn is_lock_contention_recognizes_windows_lock_violation_errors() {
        let error = io::Error::from_raw_os_error(33);

        assert!(is_lock_contention(&error));
    }

    #[test]
    fn is_lock_contention_does_not_misclassify_permission_errors() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");

        assert!(!is_lock_contention(&error));
    }
}
