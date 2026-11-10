//! How the journal touches the disk (#2253): the `.signex/` folder made
//! without following a link, and files opened without following one, created
//! private, and refused unless they are plain files. A planted link or device
//! is a refusal that is reported, never a write through it.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// The folder of a project's agent files.
pub(super) const SIGNEX_DIR: &str = ".signex";

/// `FILE_FLAG_OPEN_REPARSE_POINT`: open a link itself, not what it points at.
#[cfg(windows)]
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// `<project_dir>/.signex`, made if missing. A link or a file of that name is
/// refused: the folder is created, never followed.
pub(super) fn ensure_signex_dir(project_dir: &Path) -> Result<PathBuf, String> {
    let dir = project_dir.join(SIGNEX_DIR);
    match std::fs::create_dir(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("cannot create {}: {error}", dir.display())),
    }
    let meta = std::fs::symlink_metadata(&dir)
        .map_err(|error| format!("cannot look at {}: {error}", dir.display()))?;
    if !meta.file_type().is_dir() {
        return Err(format!(
            "{} is a link or not a folder; no agent journal was written",
            dir.display()
        ));
    }
    Ok(dir)
}

/// The options every journal file is opened with: private, and never through
/// a link or into a device that could block.
fn private_options(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(OPEN_REPARSE_POINT);
    }
    options
}

/// Refuse a handle that is not a plain file (a link opened as itself on
/// Windows, a directory, a pipe or a device).
fn require_plain_file(file: &File, path: &Path) -> Result<(), String> {
    let meta = file
        .metadata()
        .map_err(|error| format!("cannot look at {}: {error}", path.display()))?;
    if meta.is_file() && !meta.file_type().is_symlink() {
        return Ok(());
    }
    Err(format!(
        "{} is not a regular file; it was not written",
        path.display()
    ))
}

/// Open `path` to append to and to read the end of, creating it private.
pub(super) fn open_append(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).append(true).create(true);
    open_plain(&mut options, path)
}

/// Open the lock file `path`, creating it private; its content is never used.
pub(super) fn open_lock(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(false);
    open_plain(&mut options, path)
}

/// Open `path` to read; `Ok(None)` when it is not there.
pub(super) fn open_read(path: &Path) -> Result<Option<File>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    match private_options(&mut options).open(path) {
        Ok(file) => require_plain_file(&file, path).map(|()| Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(refusal(path, &error)),
    }
}

fn open_plain(options: &mut OpenOptions, path: &Path) -> Result<File, String> {
    let file = private_options(options)
        .open(path)
        .map_err(|error| refusal(path, &error))?;
    require_plain_file(&file, path)?;
    Ok(file)
}

fn refusal(path: &Path, error: &std::io::Error) -> String {
    format!("cannot open {}: {error}", path.display())
}
