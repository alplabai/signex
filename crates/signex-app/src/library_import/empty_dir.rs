//! Empty a folder a run adopted (#3123) without ever deleting outside it.
//!
//! The folder's parent may be writable by someone else, who could swap the
//! folder for a link between a check and a delete. On Unix the folder is
//! opened once, without following a link, checked against the identity
//! recorded when the run took it, and emptied through that handle: every
//! entry is removed relative to its folder's descriptor, a link is unlinked
//! itself and never followed. On Windows the run holds the adopted folder
//! open from the claim on ([`FolderHold`]), so it cannot be renamed or
//! deleted meanwhile; each folder of the walk is opened without following a
//! reparse point, checked, and kept open while its entries are removed by
//! path (`windows` below).

use std::fs::Metadata;
use std::io;
use std::path::Path;

/// What tells the adopted folder from one swapped in at its path: device
/// and inode on Unix, volume serial number and file index on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderId {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(windows)]
    volume: u32,
    #[cfg(windows)]
    index: u64,
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

/// The identity of the real folder at `path`, as a claim records it: from
/// its metadata on Unix (the caller has refused a link), from the folder
/// opened without following a reparse point on Windows, which refuses one
/// as `NotADirectory`.
pub fn folder_id_at(path: &Path) -> io::Result<FolderId> {
    #[cfg(unix)]
    {
        Ok(folder_id(&std::fs::symlink_metadata(path)?))
    }
    #[cfg(windows)]
    {
        Ok(windows::Dir::open(path)?.id)
    }
}

/// A link, a junction or any other reparse point.
pub(super) fn is_link(meta: &Metadata) -> bool {
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

/// The adopted folder held open (Windows): shared for reading and writing
/// but not for deleting, so nobody can rename, move or delete it while the
/// run holds it. Closed when the last clone drops.
#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct FolderHold {
    dir: std::sync::Arc<windows::Dir>,
}

#[cfg(windows)]
impl FolderHold {
    /// Open and hold `folder`. A reparse point (a junction, a link) or
    /// anything that is not a folder is refused as `NotADirectory`.
    pub fn open(folder: &Path) -> io::Result<Self> {
        let dir = windows::Dir::open(folder)?;
        Ok(Self {
            dir: std::sync::Arc::new(dir),
        })
    }

    pub fn id(&self) -> FolderId {
        self.dir.id
    }
}

/// Two holds are equal when they hold the same folder.
#[cfg(windows)]
impl PartialEq for FolderHold {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

#[cfg(windows)]
impl Eq for FolderHold {}

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

/// Remove everything inside `folder`, which must still be the folder
/// identified by `expected`; on a mismatch nothing is removed.
#[cfg(windows)]
pub fn empty_folder(folder: &Path, expected: &FolderId) -> io::Result<()> {
    let dir = windows::Dir::open(folder)?;
    if dir.id != *expected {
        return Err(swapped(folder));
    }
    windows::empty_held(folder, &dir)
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
        let sub = open_dir_at(dir.as_raw_fd(), name).map_err(|e| reported_error(error, e))?;
        empty_through(&sub)?;
        drop(sub);
        // SAFETY: as above; AT_REMOVEDIR removes only an empty folder, and
        // refuses a link swapped in meanwhile.
        if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// The error for an entry that unlinkat refused (`unlinked`) and that
    /// could not then be opened as a folder (`opened`). Not a folder after
    /// all (EPERM on a file in a sticky folder): the refusal is the reason.
    fn reported_error(unlinked: io::Error, opened: io::Error) -> io::Error {
        if opened.raw_os_error() == Some(libc::ENOTDIR) {
            unlinked
        } else {
            opened
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn errno(e: &io::Error) -> Option<i32> {
            e.raw_os_error()
        }

        #[test]
        fn a_refused_file_reports_the_refusal_not_that_it_is_no_folder() {
            let os = io::Error::from_raw_os_error;
            let file = reported_error(os(libc::EPERM), os(libc::ENOTDIR));
            assert_eq!(errno(&file), Some(libc::EPERM));
            let folder = reported_error(os(libc::EISDIR), os(libc::EACCES));
            assert_eq!(errno(&folder), Some(libc::EACCES));
            let link = reported_error(os(libc::EPERM), os(libc::ELOOP));
            assert_eq!(errno(&link), Some(libc::ELOOP));
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::fs::MetadataExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use std::path::Path;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        GetFileInformationByHandle, OPEN_EXISTING,
    };

    use super::FolderId;

    /// A real folder, open without `FILE_SHARE_DELETE`: while it is open
    /// it cannot be renamed, moved or deleted, so its path keeps naming it.
    #[derive(Debug)]
    pub(super) struct Dir {
        _handle: OwnedHandle,
        pub(super) id: FolderId,
    }

    impl Dir {
        /// Open `path` itself, never following a reparse point, and refuse
        /// it (`NotADirectory`) unless it is a real folder.
        pub(super) fn open(path: &Path) -> io::Result<Self> {
            // Extended-length, so a deep folder opens too (#3234).
            let wide = crate::library_import::long_path::wide(path)?;
            // SAFETY: `wide` is NUL-terminated and outlives the call; no
            // security attributes and no template.
            let raw = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    std::ptr::null_mut(),
                )
            };
            if raw == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: `raw` was just returned by CreateFileW; nothing else
            // owns it.
            let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            // SAFETY: the handle is open and `info` is the structure the
            // call fills.
            if unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &raw mut info) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let real_folder = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
                && info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0;
            if !real_folder {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is a link or not a folder", path.display()),
                ));
            }
            let id = FolderId {
                volume: info.dwVolumeSerialNumber,
                index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            };
            Ok(Self {
                _handle: handle,
                id,
            })
        }
    }

    /// Remove everything inside the folder at `path`, which `dir` holds.
    pub(super) fn empty_held(path: &Path, _dir: &Dir) -> io::Result<()> {
        let mut first_error = None;
        for entry in std::fs::read_dir(path)? {
            if let Err(e) = entry.and_then(|entry| remove_entry(&entry.path())) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// A reparse point (a junction, a link) is removed itself, never its
    /// target: `RemoveDirectoryW` on a junction removes the junction. A
    /// real folder is held, emptied and removed once empty.
    fn remove_entry(path: &Path) -> io::Result<()> {
        let attributes = std::fs::symlink_metadata(path)?.file_attributes();
        let is_folder = attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return if is_folder {
                std::fs::remove_dir(path)
            } else {
                std::fs::remove_file(path)
            };
        }
        if !is_folder {
            return std::fs::remove_file(path);
        }
        let sub = Dir::open(path)?;
        empty_held(path, &sub)?;
        // Our handle shares no delete, so it is closed first; a link swapped
        // in meanwhile is removed as a link, a full folder is refused.
        drop(sub);
        std::fs::remove_dir(path)
    }
}
