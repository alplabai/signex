//! How the journal touches the disk (#2253): the `.signex/` folder opened once,
//! without following a link, and every journal file opened relative to that
//! handle, created private, and refused unless it is a plain file with one
//! link. A planted link, device or hard link is a refusal that is reported,
//! never a write through it; and a folder swapped for a link after it was
//! opened changes nothing, because the files are opened through the handle,
//! not through the path. See `docs/AGENT_JOURNALS.md`, "Append".

use std::fs::File;
use std::path::{Path, PathBuf};

/// The folder of a project's agent files.
pub(super) const SIGNEX_DIR: &str = ".signex";

/// An opened `.signex/` folder: the one seam every journal file goes through.
pub(crate) struct SignexDir {
    /// The folder as a path, for messages (and, off Unix, to open files).
    path: PathBuf,
    /// Unix: the folder itself, opened `O_DIRECTORY | O_NOFOLLOW`.
    #[cfg(unix)]
    handle: File,
    /// Windows: the folder's final path as the system resolved it, which every
    /// opened file must lie under.
    #[cfg(windows)]
    resolved: PathBuf,
    /// Windows: the folder's own handle, held for as long as this value lives
    /// and opened without `FILE_SHARE_DELETE`, so the folder cannot be renamed
    /// or replaced (by a junction, say) while it is held. Windows has no
    /// `openat`; this plus the final-path check on every file opened, and on the
    /// temporary file before anything is written to it, is the equivalent. The
    /// rename of the temporary file goes by path (`SetFileInformationByHandle`
    /// would rename by handle, but adds unsafe code for no more than the held
    /// handle already gives). See `docs/AGENT_JOURNALS.md`, "Append".
    #[cfg(windows)]
    _folder: File,
}

impl SignexDir {
    /// `<project_dir>/.signex`, made if missing and opened. A link or a file
    /// of that name is refused: the folder is created, never followed.
    pub(crate) fn create(project_dir: &Path) -> Result<Self, String> {
        let folder = project_dir.join(SIGNEX_DIR);
        match std::fs::create_dir(&folder) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("cannot create {}: {error}", folder.display())),
        }
        Self::open_folder(&folder)?
            .ok_or_else(|| format!("{} disappeared while it was opened", folder.display()))
    }

    /// The folder at `folder` (a `.signex`), `Ok(None)` when it is not there.
    pub(crate) fn open_folder(folder: &Path) -> Result<Option<Self>, String> {
        imp::open_folder(folder)
    }

    /// The folder as a path, for messages.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Open `name` to append to and to read the end of, creating it private.
    pub(crate) fn append_file(&self, name: &str) -> Result<File, String> {
        let file = imp::open_file(self, name, imp::Mode::Append)?;
        self.require_plain(&file, name)?;
        Ok(file)
    }

    /// Open the lock file `name`, creating it private; its content is unused.
    pub(crate) fn lock_file(&self, name: &str) -> Result<File, String> {
        let file = imp::open_file(self, name, imp::Mode::Lock)?;
        self.require_plain(&file, name)?;
        Ok(file)
    }

    /// Open `name` to read; `Ok(None)` when it is not there.
    pub(crate) fn read_file(&self, name: &str) -> Result<Option<File>, String> {
        match imp::open_file(self, name, imp::Mode::Read) {
            Ok(file) => self.require_plain(&file, name).map(|()| Some(file)),
            Err(_) if !imp::exists(self, name) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// A new private file `name` that must not exist yet, to be renamed over a
    /// journal.
    pub(crate) fn create_temp(&self, name: &str) -> Result<File, String> {
        let file = imp::open_file(self, name, imp::Mode::Temp)?;
        // Before anything is written to it: it must lie inside the folder.
        if let Err(why) = imp::check_inside(self, &file) {
            drop(file);
            self.remove(name);
            return Err(format!(
                "{}: {why}; no temporary file was written",
                self.path.join(name).display()
            ));
        }
        Ok(file)
    }

    /// Rename `from` over `to`, inside the folder, in one step.
    pub(crate) fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        imp::rename(self, from, to).map_err(|error| {
            format!(
                "cannot rename {from} over {to} in {}: {error}",
                self.path.display()
            )
        })
    }

    /// Remove `name` if it is there.
    pub(crate) fn remove(&self, name: &str) {
        imp::remove(self, name).ok();
    }

    /// Refuse a handle that is not a plain file with one link (a link opened
    /// as itself on Windows, a directory, a pipe, a device or a hard link to
    /// a file that lives somewhere else).
    fn require_plain(&self, file: &File, name: &str) -> Result<(), String> {
        let shown = self.path.join(name);
        let meta = file
            .metadata()
            .map_err(|error| format!("cannot look at {}: {error}", shown.display()))?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(format!(
                "{} is not a regular file; it was not written",
                shown.display()
            ));
        }
        let links = imp::link_count(file, &meta)
            .map_err(|error| format!("cannot look at {}: {error}", shown.display()))?;
        if links != 1 {
            return Err(format!(
                "{} has {links} links, not one; it was not written",
                shown.display()
            ));
        }
        imp::check_inside(self, file)
            .map_err(|why| format!("{}: {why}; it was not written", shown.display()))
    }
}

#[cfg(unix)]
mod imp {
    use std::ffi::CString;
    use std::fs::File;
    use std::os::fd::{AsRawFd as _, FromRawFd as _};
    use std::os::unix::fs::MetadataExt as _;
    use std::path::Path;

    use super::SignexDir;

    pub(super) enum Mode {
        Append,
        Lock,
        Read,
        Temp,
    }

    fn c_string(text: &str) -> std::io::Result<CString> {
        CString::new(text)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "a NUL in a name"))
    }

    pub(super) fn open_folder(folder: &Path) -> Result<Option<SignexDir>, String> {
        use std::os::unix::ffi::OsStrExt as _;
        let c = CString::new(folder.as_os_str().as_bytes())
            .map_err(|_| format!("{} has a NUL in it", folder.display()))?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: `c` is a valid NUL-terminated path that outlives the call.
        let fd = unsafe { libc::open(c.as_ptr(), flags) };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ENOENT) => Ok(None),
                Some(libc::ELOOP | libc::ENOTDIR) => Err(format!(
                    "{} is a link or not a folder; no agent journal was written",
                    folder.display()
                )),
                _ => Err(format!("cannot open {}: {error}", folder.display())),
            };
        }
        // SAFETY: `fd` was just returned by `open` and is owned by nothing else.
        let handle = unsafe { File::from_raw_fd(fd) };
        Ok(Some(SignexDir {
            path: folder.to_path_buf(),
            handle,
        }))
    }

    pub(super) fn open_file(dir: &SignexDir, name: &str, mode: Mode) -> Result<File, String> {
        let shown = dir.path.join(name);
        let fail = |error: std::io::Error| format!("cannot open {}: {error}", shown.display());
        let c = c_string(name).map_err(fail)?;
        let base = libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
        // The file is made by an exclusive create (EEXIST is fine) and then
        // opened without `O_CREAT`: a create-open raced by another creator of
        // the same new name can fail with ENOENT on macOS (see
        // `docs/AGENT_JOURNALS.md`, "Append").
        let (flags, creates_first) = match mode {
            Mode::Append => (libc::O_RDWR | libc::O_APPEND, true),
            Mode::Lock => (libc::O_WRONLY, true),
            Mode::Read => (libc::O_RDONLY, false),
            Mode::Temp => (libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, false),
        };
        if creates_first {
            match openat(
                dir,
                &c,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | base,
            ) {
                Ok(created) => drop(created),
                Err(error) if error.raw_os_error() == Some(libc::EEXIST) => {}
                Err(error) => return Err(fail(error)),
            }
        }
        openat(dir, &c, flags | base).map_err(fail)
    }

    /// `openat` on the folder handle, with the private mode.
    fn openat(dir: &SignexDir, name: &CString, flags: libc::c_int) -> std::io::Result<File> {
        let private: libc::c_uint = 0o600;
        // SAFETY: the folder handle is open, `name` is a valid NUL-terminated
        // name, and the mode is passed as the unsigned int `openat` reads.
        let fd = unsafe { libc::openat(dir.handle.as_raw_fd(), name.as_ptr(), flags, private) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `fd` was just returned by `openat` and is owned by nothing else.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    pub(super) fn rename(dir: &SignexDir, from: &str, to: &str) -> std::io::Result<()> {
        let (from, to) = (c_string(from)?, c_string(to)?);
        let fd = dir.handle.as_raw_fd();
        // SAFETY: the folder handle is open and both names are valid C strings.
        let done = unsafe { libc::renameat(fd, from.as_ptr(), fd, to.as_ptr()) };
        if done != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // Make the rename itself durable, as `atomic_write` does; the data was
        // synced before. Best effort: the rename has happened.
        // SAFETY: the folder handle is open.
        unsafe { libc::fsync(fd) };
        Ok(())
    }

    pub(super) fn remove(dir: &SignexDir, name: &str) -> std::io::Result<()> {
        let c = c_string(name)?;
        // SAFETY: the folder handle is open and `c` is a valid C string.
        let done = unsafe { libc::unlinkat(dir.handle.as_raw_fd(), c.as_ptr(), 0) };
        if done == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn exists(dir: &SignexDir, name: &str) -> bool {
        let Ok(c) = c_string(name) else {
            return false;
        };
        // SAFETY: a zeroed `stat` is a valid out-parameter; the handle is open
        // and `c` is a valid C string.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let found = unsafe {
            libc::fstatat(
                dir.handle.as_raw_fd(),
                c.as_ptr(),
                &raw mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        found == 0
    }

    pub(super) fn link_count(_file: &File, meta: &std::fs::Metadata) -> std::io::Result<u64> {
        Ok(meta.nlink())
    }

    /// Opened through the folder handle, a file cannot lie outside it.
    pub(super) fn check_inside(_dir: &SignexDir, _file: &File) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(windows)]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle, GetFinalPathNameByHandleW,
    };

    use super::SignexDir;

    /// `FILE_FLAG_OPEN_REPARSE_POINT`: open a link itself, not what it points at.
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    /// `FILE_FLAG_BACKUP_SEMANTICS`: needed to open a folder.
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    /// Share modes: read and write, never delete.
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    /// File attributes of an opened handle.
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    pub(super) enum Mode {
        Append,
        Lock,
        Read,
        Temp,
    }

    /// The path the system resolves `file` to.
    fn final_path(file: &File) -> std::io::Result<PathBuf> {
        use std::os::windows::ffi::OsStringExt as _;
        let handle = file.as_raw_handle() as HANDLE;
        let mut buffer = vec![0_u16; 1024];
        loop {
            // SAFETY: `handle` is open and `buffer` is writable for its length.
            let length = unsafe {
                GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
            };
            if length == 0 {
                return Err(std::io::Error::last_os_error());
            }
            if length as usize > buffer.len() {
                buffer.resize(length as usize, 0);
                continue;
            }
            return Ok(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
    }

    pub(super) fn open_folder(folder: &Path) -> Result<Option<SignexDir>, String> {
        let meta = match std::fs::symlink_metadata(folder) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("cannot look at {}: {error}", folder.display())),
        };
        if !meta.file_type().is_dir() {
            return Err(format!(
                "{} is a link or not a folder; no agent journal was written",
                folder.display()
            ));
        }
        // No `FILE_SHARE_DELETE`: while this handle is held the folder cannot
        // be renamed or replaced.
        let handle = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(BACKUP_SEMANTICS | OPEN_REPARSE_POINT)
            .open(folder)
            .map_err(|error| format!("cannot open {}: {error}", folder.display()))?;
        // The handle's own attributes: a folder, and not a reparse point (a
        // junction or symlink swapped in after the check above). Before any
        // file is created.
        let attributes = information(&handle)
            .map_err(|error| format!("cannot look at {}: {error}", folder.display()))?
            .dwFileAttributes;
        if attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(format!(
                "{} is a link or not a folder; no agent journal was written",
                folder.display()
            ));
        }
        let resolved = final_path(&handle)
            .map_err(|error| format!("cannot resolve {}: {error}", folder.display()))?;
        Ok(Some(SignexDir {
            path: folder.to_path_buf(),
            resolved,
            _folder: handle,
        }))
    }

    pub(super) fn open_file(dir: &SignexDir, name: &str, mode: Mode) -> Result<File, String> {
        let path = dir.path.join(name);
        let mut options = OpenOptions::new();
        match mode {
            Mode::Append => options.read(true).append(true).create(true),
            Mode::Lock => options.write(true).create(true).truncate(false),
            Mode::Read => options.read(true),
            Mode::Temp => options.write(true).create_new(true),
        };
        options
            .custom_flags(OPEN_REPARSE_POINT)
            .open(&path)
            .map_err(|error| format!("cannot open {}: {error}", path.display()))
    }

    pub(super) fn rename(dir: &SignexDir, from: &str, to: &str) -> std::io::Result<()> {
        std::fs::rename(dir.path.join(from), dir.path.join(to))
    }

    pub(super) fn remove(dir: &SignexDir, name: &str) -> std::io::Result<()> {
        std::fs::remove_file(dir.path.join(name))
    }

    pub(super) fn exists(dir: &SignexDir, name: &str) -> bool {
        std::fs::symlink_metadata(dir.path.join(name)).is_ok()
    }

    /// What the system says about the opened handle itself.
    fn information(file: &File) -> std::io::Result<BY_HANDLE_FILE_INFORMATION> {
        // SAFETY: a zeroed information struct is a valid out-parameter and the
        // handle is open.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        let done =
            unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &raw mut info) };
        if done == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(info)
        }
    }

    pub(super) fn link_count(file: &File, _meta: &std::fs::Metadata) -> std::io::Result<u64> {
        information(file).map(|info| u64::from(info.nNumberOfLinks))
    }

    /// The opened file must resolve to a path under the folder as it was
    /// resolved when it was opened: a folder or file swapped for a link after
    /// that lands elsewhere and is refused.
    pub(super) fn check_inside(dir: &SignexDir, file: &File) -> Result<(), String> {
        let resolved = final_path(file).map_err(|error| error.to_string())?;
        if resolved.starts_with(&dir.resolved) {
            Ok(())
        } else {
            Err(format!(
                "it resolves to {}, outside {}",
                resolved.display(),
                dir.resolved.display()
            ))
        }
    }
}
