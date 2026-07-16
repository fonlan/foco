use std::{fs, io, path::Path};

#[cfg(unix)]
use std::os::unix::{fs::OpenOptionsExt, fs::PermissionsExt};

const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

pub(crate) fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    set_mode(path, PRIVATE_DIRECTORY_MODE)
}

pub(crate) fn prepare_private_file(path: &Path) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    options.mode(PRIVATE_FILE_MODE);
    drop(options.open(path)?);
    set_mode(path, PRIVATE_FILE_MODE)
}

pub(crate) fn restrict_private_file(path: &Path) -> io::Result<()> {
    if path.exists() {
        set_mode(path, PRIVATE_FILE_MODE)?;
    }
    Ok(())
}

pub(crate) fn restrict_sqlite_files(database_path: &Path) -> io::Result<()> {
    restrict_private_file(database_path)?;
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = database_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        restrict_private_file(Path::new(&sidecar))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let current = fs::metadata(path)?.permissions().mode() & 0o777;
    if current != mode {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}
