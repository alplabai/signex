//! Putting a staged library in place without replacing anything (#3171).
//! Every rename refuses to replace what is at its destination: Linux
//! `renameat2(RENAME_NOREPLACE)`, macOS `renameatx_np(RENAME_EXCL)`,
//! Windows `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`. Elsewhere,
//! and on a Linux file system without `RENAME_NOREPLACE`, the destination
//! is checked right before the rename, which leaves a narrow race. Any of
//! them reports a taken destination as `AlreadyExists`.
//!
//! A folder entries move between is held ([`HeldDir`]): on Unix it is
//! opened without following a link and the renames go relative to its
//! descriptor, so a link swapped in at its path is never written through;
//! on Windows it is held open without sharing delete, so it cannot be
//! renamed or replaced meanwhile.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use super::empty_dir::FolderHold;
use super::empty_dir::{FolderId, is_link};

/// A folder held while entries move into or out of it.
#[derive(Debug)]
pub(super) struct HeldDir {
    path: PathBuf,
    #[cfg(unix)]
    fd: std::fs::File,
    #[cfg(windows)]
    hold: FolderHold,
}

impl HeldDir {
    /// Hold the real folder at `path`. A link or anything that is not a
    /// folder is refused (`NotADirectory` on Windows, `ENOTDIR` or `ELOOP`
    /// on Unix).
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            #[cfg(unix)]
            fd: unix::open_dir(path)?,
            #[cfg(windows)]
            hold: FolderHold::open(path)?,
        })
    }

    /// The held folder's identity, read from what is held.
    pub(super) fn id(&self) -> io::Result<FolderId> {
        #[cfg(unix)]
        {
            Ok(super::empty_dir::folder_id(&self.fd.metadata()?))
        }
        #[cfg(windows)]
        {
            Ok(self.hold.id())
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Flush the folder's entries to disk (Unix). Windows has no folder
    /// flush through std; NTFS journals the rename itself.
    pub(super) fn sync(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.fd.sync_all()
        }
        #[cfg(windows)]
        {
            Ok(())
        }
    }
}

/// Rename the entry `name` of `from` into `to`, never replacing anything
/// there.
pub(super) fn rename_entry(from: &HeldDir, to: &HeldDir, name: &OsStr) -> io::Result<()> {
    #[cfg(unix)]
    {
        let name = unix::c_name(name)?;
        unix::renameat_noreplace(
            std::os::fd::AsRawFd::as_raw_fd(&from.fd),
            &name,
            std::os::fd::AsRawFd::as_raw_fd(&to.fd),
            &name,
        )
    }
    #[cfg(windows)]
    {
        windows::move_no_replace(&from.path.join(name), &to.path.join(name))
    }
}

/// Rename the folder `from` to the absent path `to`, never replacing
/// anything there.
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let (from, to) = (unix::c_path(from)?, unix::c_path(to)?);
        unix::renameat_noreplace(libc::AT_FDCWD, &from, libc::AT_FDCWD, &to)
    }
    #[cfg(windows)]
    {
        windows::move_no_replace(from, to)
    }
}

/// Flush the folder at `path` to disk (Unix; nothing on Windows, where
/// std cannot flush a folder).
#[cfg(unix)]
pub(super) fn sync_dir(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// Flush the folder at `path` to disk (Unix; nothing on Windows, where
/// std cannot flush a folder).
#[cfg(windows)]
pub(super) fn sync_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Copy the file `from` to the new path `to`, which must not exist (it is
/// never replaced), with its permissions, and flush it to disk. A copy that
/// fails after the file was created removes that file, which is its own.
pub(super) fn copy_file_synced(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = std::fs::File::open(from)?;
    let mut copy = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    let written = io::copy(&mut source, &mut copy)
        .and_then(|_| copy.set_permissions(source.metadata()?.permissions()))
        .and_then(|()| copy.sync_all());
    if written.is_err() {
        drop(copy);
        if let Err(why) = std::fs::remove_file(to) {
            log::error!(
                "library import: the partial copy {} stays: {why}",
                to.display()
            );
        }
    }
    written
}

/// Copy the folder `from` into the existing empty folder `to`, each file
/// flushed and each folder synced. A link is refused, never followed: a
/// converter writes plain files.
pub(super) fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let meta = std::fs::symlink_metadata(entry.path())?;
        let dest = to.join(entry.file_name());
        if is_link(&meta) {
            return Err(io::Error::other(format!(
                "{} is a link",
                entry.path().display()
            )));
        }
        if meta.is_dir() {
            std::fs::create_dir(&dest)?;
            copy_tree(&entry.path(), &dest)?;
        } else {
            copy_file_synced(&entry.path(), &dest)?;
        }
    }
    sync_dir(to)
}

/// Move `from` to the new path `to` across file systems: a synced copy,
/// then `remove_source`. Nothing at `to` is replaced; a copy that fails
/// removes only what it created and returns the error. Once the copy is whole the entry counts as moved: a source
/// that cannot be removed is logged and left.
pub(super) fn copy_across(
    from: &Path,
    to: &Path,
    remove_source: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if is_link(&meta) {
        return Err(io::Error::other(format!("{} is a link", from.display())));
    }
    if meta.is_dir() {
        // Created here, so all of it is the copy's to remove on a failure.
        std::fs::create_dir(to)?;
        if let Err(e) = copy_tree(from, to) {
            if let Err(why) = std::fs::remove_dir_all(to) {
                log::error!(
                    "library import: the partial copy {} stays: {why}",
                    to.display()
                );
            }
            return Err(e);
        }
    } else {
        copy_file_synced(from, to)?;
    }
    if let Err(why) = remove_source(from) {
        log::warn!(
            "library import: {} was copied to {} but stays behind: {why}",
            from.display(),
            to.display()
        );
    }
    Ok(())
}

/// Remove a copied source: a folder with what is in it (std never follows
/// a link inside it), or a file.
pub(super) fn remove_source(path: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(path)?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::{CStr, CString, OsStr};
    use std::fs::File;
    use std::io;
    use std::os::fd::{FromRawFd as _, OwnedFd, RawFd};
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::Path;

    pub(super) fn c_name(name: &OsStr) -> io::Result<CString> {
        CString::new(name.as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }

    pub(super) fn c_path(path: &Path) -> io::Result<CString> {
        c_name(path.as_os_str())
    }

    /// Open the folder at `path` itself, refusing a link.
    pub(super) fn open_dir(path: &Path) -> io::Result<File> {
        let path = c_path(path)?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: `path` is a valid NUL-terminated string open only reads.
        let fd = unsafe { libc::open(path.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` was just returned by open and nothing else owns it.
        Ok(File::from(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    fn last() -> io::Error {
        io::Error::last_os_error()
    }

    /// `renameat`, never replacing `new`.
    #[cfg(target_os = "linux")]
    pub(super) fn renameat_noreplace(
        old_dir: RawFd,
        old: &CStr,
        new_dir: RawFd,
        new: &CStr,
    ) -> io::Result<()> {
        // SAFETY: both names are NUL-terminated and only read; the folder
        // descriptors are open or AT_FDCWD.
        let done = unsafe {
            libc::renameat2(
                old_dir,
                old.as_ptr(),
                new_dir,
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if done == 0 {
            return Ok(());
        }
        let error = last();
        // A file system without RENAME_NOREPLACE.
        if matches!(error.raw_os_error(), Some(libc::EINVAL | libc::ENOSYS)) {
            return checked_renameat(old_dir, old, new_dir, new);
        }
        Err(error)
    }

    /// `renameat`, never replacing `new`.
    #[cfg(target_os = "macos")]
    pub(super) fn renameat_noreplace(
        old_dir: RawFd,
        old: &CStr,
        new_dir: RawFd,
        new: &CStr,
    ) -> io::Result<()> {
        // SAFETY: as for renameat2 above.
        let done = unsafe {
            libc::renameatx_np(
                old_dir,
                old.as_ptr(),
                new_dir,
                new.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        if done == 0 {
            return Ok(());
        }
        let error = last();
        if matches!(error.raw_os_error(), Some(libc::ENOTSUP | libc::EINVAL)) {
            return checked_renameat(old_dir, old, new_dir, new);
        }
        Err(error)
    }

    /// `renameat`, never replacing `new`: no flag for it here, so `new` is
    /// checked right before.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(super) fn renameat_noreplace(
        old_dir: RawFd,
        old: &CStr,
        new_dir: RawFd,
        new: &CStr,
    ) -> io::Result<()> {
        checked_renameat(old_dir, old, new_dir, new)
    }

    /// Check that `new` does not exist, then `renameat`. Not atomic: what
    /// appears in between is replaced (a file, or an empty folder).
    fn checked_renameat(old_dir: RawFd, old: &CStr, new_dir: RawFd, new: &CStr) -> io::Result<()> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `new` is NUL-terminated, `stat` is written by the call.
        let found = unsafe {
            libc::fstatat(
                new_dir,
                new.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if found == 0 {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        let error = last();
        if error.kind() != io::ErrorKind::NotFound {
            return Err(error);
        }
        // SAFETY: as above.
        if unsafe { libc::renameat(old_dir, old.as_ptr(), new_dir, new.as_ptr()) } != 0 {
            return Err(last());
        }
        Ok(())
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain([0]).collect()
    }

    /// Move `from` to `to` on the same volume; without
    /// `MOVEFILE_REPLACE_EXISTING` it fails when `to` exists, and without
    /// `MOVEFILE_COPY_ALLOWED` across volumes (`CrossesDevices`).
    pub(super) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (wide(from), wide(to));
        // SAFETY: both are NUL-terminated and outlive the call.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
