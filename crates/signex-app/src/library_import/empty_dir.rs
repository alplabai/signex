//! Empty a folder a run adopted (#3123) without ever deleting outside it.
//!
//! The folder's parent may be writable by someone else, who could swap the
//! folder for a link between a check and a delete. On Unix the folder is
//! opened once, without following a link, checked against the identity
//! recorded when the run took it, and emptied through that handle: every
//! entry is removed relative to its folder's descriptor, a link is unlinked
//! itself and never followed. Elsewhere the walk goes by path, and a narrow
//! race remains (see [`empty_folder`]).

use std::fs::Metadata;
use std::io;
use std::path::Path;

/// What tells the adopted folder from one swapped in at its path: device
/// and inode on Unix. Windows has no stable std API for the file index, so
/// nothing is recorded there and the walk relies on refusing reparse points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderId {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

/// The identity of the folder `meta` was read from.
#[cfg(unix)]
pub fn folder_id(meta: &Metadata) -> FolderId {
    use std::os::unix::fs::MetadataExt as _;
    FolderId {
        dev: meta.dev(),
        ino: meta.ino(),
    }
}

/// The identity of the folder `meta` was read from (none recorded here).
#[cfg(not(unix))]
pub fn folder_id(_meta: &Metadata) -> FolderId {
    FolderId {}
}

fn swapped(folder: &Path) -> io::Error {
    io::Error::other(format!(
        "{} is no longer the folder the import took",
        folder.display()
    ))
}

/// Remove everything inside `folder`, which must still be the folder
/// identified by `expected`; on a mismatch nothing is removed.
#[cfg(unix)]
pub fn empty_folder(folder: &Path, expected: &FolderId) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;
    let path = std::ffi::CString::new(folder.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let dir = unix::open_dir_at(libc::AT_FDCWD, &path)?;
    let found = folder_id(&dir.metadata()?);
    if found.dev != expected.dev || found.ino != expected.ino {
        return Err(swapped(folder));
    }
    unix::empty_through(&dir)
}

/// Remove everything inside `folder`, by path. Each folder is checked to
/// be a real folder (not a link or another reparse point) right before it
/// is read, but a narrow race remains between that check and the read: no
/// std API here empties a folder through its own handle.
#[cfg(not(unix))]
pub fn empty_folder(folder: &Path, _expected: &FolderId) -> io::Result<()> {
    walk::empty_by_path(folder)
}

#[cfg(unix)]
mod unix {
    use std::ffi::{CStr, CString};
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};

    /// Open the folder `name` relative to `parent`, refusing a link.
    pub(super) fn open_dir_at(parent: RawFd, name: &CStr) -> io::Result<File> {
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: `name` is a valid NUL-terminated string that openat only
        // reads; `parent` is an open folder descriptor or AT_FDCWD.
        let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` was just returned by openat and nothing else owns it.
        Ok(File::from(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    /// A `DIR*` stream, closed once when dropped.
    struct DirStream(*mut libc::DIR);

    impl Drop for DirStream {
        fn drop(&mut self) {
            // SAFETY: the stream came from fdopendir and is closed only here.
            unsafe { libc::closedir(self.0) };
        }
    }

    /// The names inside the folder `dir`, without `.` and `..`.
    fn entry_names(dir: &File) -> io::Result<Vec<CString>> {
        // SAFETY: dup only duplicates the open descriptor `dir` holds.
        let dup = unsafe { libc::dup(dir.as_raw_fd()) };
        if dup < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `dup` is a fresh descriptor this function owns; on success
        // fdopendir takes it over and closedir (DirStream) closes it.
        let stream = unsafe { libc::fdopendir(dup) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: fdopendir failed, so `dup` is still ours to close.
            drop(unsafe { OwnedFd::from_raw_fd(dup) });
            return Err(error);
        }
        let stream = DirStream(stream);
        let mut names = Vec::new();
        loop {
            // SAFETY: `stream.0` is a valid open DIR* until DirStream drops.
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                break;
            }
            // SAFETY: readdir returned a valid dirent whose `d_name` is
            // NUL-terminated and valid until the next readdir; it is copied.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name != c"." && name != c".." {
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }

    /// Remove everything inside the folder `dir`, through its descriptor.
    pub(super) fn empty_through(dir: &File) -> io::Result<()> {
        let mut first_error = None;
        for name in entry_names(dir)? {
            if let Err(e) = remove_at(dir, &name) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Remove `name` inside `dir`. A file or a link is unlinked itself; a
    /// folder is opened without following a link, emptied and removed.
    fn remove_at(dir: &File, name: &CStr) -> io::Result<()> {
        // SAFETY: `dir` is an open folder descriptor and `name` a valid
        // NUL-terminated string; without AT_REMOVEDIR this never removes a
        // folder, and a link is removed itself.
        if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // A folder: EISDIR on Linux, EPERM on macOS and the BSDs.
        if !matches!(error.raw_os_error(), Some(libc::EISDIR | libc::EPERM)) {
            return Err(error);
        }
        let sub = open_dir_at(dir.as_raw_fd(), name)?;
        empty_through(&sub)?;
        drop(sub);
        // SAFETY: as above; AT_REMOVEDIR removes only an empty folder, and
        // refuses a link swapped in meanwhile.
        if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(not(unix))]
mod walk {
    use std::fs::Metadata;
    use std::io;
    use std::path::Path;

    /// A link, a junction or any other reparse point.
    fn is_link(meta: &Metadata) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return true;
            }
        }
        meta.file_type().is_symlink()
    }

    /// `dir` is a real folder now.
    fn check_real_folder(dir: &Path) -> io::Result<()> {
        let meta = std::fs::symlink_metadata(dir)?;
        if is_link(&meta) || !meta.is_dir() {
            return Err(super::swapped(dir));
        }
        Ok(())
    }

    pub(super) fn empty_by_path(dir: &Path) -> io::Result<()> {
        check_real_folder(dir)?;
        let mut first_error = None;
        for entry in std::fs::read_dir(dir)? {
            if let Err(e) = entry.and_then(|entry| remove_by_path(&entry.path())) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// A link is removed itself (a link to a folder as a folder); a real
    /// folder is emptied, checked again and removed.
    fn remove_by_path(path: &Path) -> io::Result<()> {
        let meta = std::fs::symlink_metadata(path)?;
        if is_link(&meta) {
            return std::fs::remove_file(path).or_else(|e| std::fs::remove_dir(path).map_err(|_| e));
        }
        if !meta.is_dir() {
            return std::fs::remove_file(path);
        }
        empty_by_path(path)?;
        check_real_folder(path)?;
        std::fs::remove_dir(path)
    }
}
