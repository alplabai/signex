//! A new import's drawn preview (#3171): the converter writes the library
//! into a staging folder beside the target (same parent, so the same file
//! system), the dialog draws its parts, and Import moves the folder into
//! place with no second converter run. Blocking; call from
//! `spawn_blocking`. See docs/architecture/library-dialogs.md, "Import
//! Library dialog".
//!
//! The staging folder follows the output rules of [`super::output`]: the
//! run creates it (an existing one is refused, never adopted) and it is
//! removed without following a link ([`StagedLibrary::discard`]). An empty
//! target folder the user chose is kept, with its permissions; the staged
//! entries are moved into it.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use super::empty_dir::FolderHold;
use super::empty_dir::{FolderId, folder_id_at, is_link};
use super::error::ImportError;
use super::output::ClaimedOutput;
use super::run::{LibraryRunRequest, RunOutcome};

/// What a staging folder's name holds after the library's name.
pub const STAGING_MARK: &str = ".signex-preview-";

/// The staging folder for `target`: `.<name>.signex-preview-<id>` in the
/// target's parent. `None` when the target has no parent or no name.
pub fn staging_dir_for(target: &Path) -> Option<PathBuf> {
    let parent = target.parent()?;
    let name = target.file_name()?.to_str()?;
    let id = uuid::Uuid::now_v7().simple();
    Some(parent.join(format!(".{name}{STAGING_MARK}{id}")))
}

/// What the target folder was when the preview was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBefore {
    Absent,
    /// An empty folder, with its identity then.
    Empty(FolderId),
}

impl TargetBefore {
    /// Read the target: absent or an empty folder. A link, a file or a
    /// folder that holds anything is refused.
    pub fn read(target: &Path) -> Result<Self, ImportError> {
        let unusable = |e: std::io::Error| ImportError::OutputUnusable {
            path: target.to_path_buf(),
            detail: e.to_string(),
        };
        let meta = match std::fs::symlink_metadata(target) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::Absent),
            Err(e) => return Err(unusable(e)),
            Ok(meta) => meta,
        };
        if is_link(&meta) {
            return Err(ImportError::OutputIsLink {
                path: target.to_path_buf(),
            });
        }
        let taken = || ImportError::OutputNotEmpty {
            path: target.to_path_buf(),
        };
        if !meta.is_dir() {
            return Err(taken());
        }
        let id = folder_id_at(target).map_err(|e| {
            if e.kind() == ErrorKind::NotADirectory {
                ImportError::OutputIsLink {
                    path: target.to_path_buf(),
                }
            } else {
                unusable(e)
            }
        })?;
        let mut entries = std::fs::read_dir(target).map_err(unusable)?;
        match entries.next() {
            None => Ok(Self::Empty(id)),
            Some(_) => Err(taken()),
        }
    }

    /// The target is still as it was read: still absent, or still the same
    /// empty folder.
    fn unchanged(&self, target: &Path) -> bool {
        match (Self::read(target), self) {
            (Ok(Self::Absent), Self::Absent) => true,
            (Ok(Self::Empty(now)), Self::Empty(then)) => now == *then,
            _ => false,
        }
    }
}

/// A finished staged preview. Whoever holds it (the dialog) owns the
/// staging folder until Import adopts it or [`Self::discard`] removes it.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedLibrary {
    /// The staging folder as the run took it.
    pub claimed: ClaimedOutput,
    /// Where Import moves it.
    pub target: PathBuf,
    pub target_before: TargetBefore,
    /// The request the preview ran (its `output` is the target), so a
    /// changed option can be told from the one previewed.
    pub request: LibraryRunRequest,
    /// The run, its paths inside the staging folder.
    pub run: RunOutcome,
    /// The `.snxlib` inside the staging folder.
    pub library_file: PathBuf,
}

/// Why a staged preview was not moved.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveFailure {
    pub error: ImportError,
    /// The staging folder is still whole, so Import can be tried again.
    pub preview_kept: bool,
}

impl StagedLibrary {
    /// The staging folder.
    pub fn dir(&self) -> &Path {
        self.claimed.folder()
    }

    /// Remove the staging folder, and the parent folders the preview made
    /// while they are empty, never following a link. True when it is gone.
    pub fn discard(&self) -> bool {
        self.claimed.discard()
    }

    /// Move the staged library to the target: refused, with the preview
    /// kept, unless the target is still as it was when the preview was
    /// staged. An absent target is the staging folder renamed (a copy then
    /// a removal across file systems); an empty one is kept as it is, with
    /// its permissions, and the staged entries are moved into it.
    pub fn move_into_place(&self) -> Result<(), MoveFailure> {
        self.move_into_place_with(&mut move_entry)
    }

    /// [`Self::move_into_place`], moving each entry into an empty target
    /// with `move_one` (tests make it fail half way).
    pub(super) fn move_into_place_with(
        &self,
        move_one: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), MoveFailure> {
        // The folder check again, just before anything moves.
        if !self.target_before.unchanged(&self.target) {
            return Err(kept(self.changed()));
        }
        match self.target_before {
            TargetBefore::Absent => self.rename_whole(),
            TargetBefore::Empty(id) => {
                // Windows: the target is held, so it cannot be renamed or
                // swapped while the entries move, and its held identity is
                // checked once more.
                #[cfg(windows)]
                let _hold = match FolderHold::open(&self.target) {
                    Ok(hold) if hold.id() == id => hold,
                    _ => return Err(kept(self.changed())),
                };
                #[cfg(not(windows))]
                let _ = id;
                self.move_contents(move_one)
            }
        }
    }

    fn changed(&self) -> ImportError {
        ImportError::TargetChanged {
            path: self.target.clone(),
        }
    }

    /// An absent target: the staging folder becomes it.
    fn rename_whole(&self) -> Result<(), MoveFailure> {
        match std::fs::rename(self.dir(), &self.target) {
            Ok(()) => Ok(()),
            Err(e) if crosses_devices(&e) => self.copy_into_place(),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
                ) =>
            {
                Err(kept(self.changed()))
            }
            Err(e) => Err(kept(ImportError::MoveFailed {
                path: self.target.clone(),
                detail: e.to_string(),
            })),
        }
    }

    /// An empty target the user chose, just checked again to be the same
    /// empty folder (not a link or other reparse point): each staged entry is moved into it and the
    /// empty staging folder removed. A move that fails half way moves what
    /// it moved back, as far as it can, and keeps the preview.
    fn move_contents(
        &self,
        move_one: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), MoveFailure> {
        let unreadable = |e: std::io::Error| {
            kept(ImportError::MoveFailed {
                path: self.dir().to_path_buf(),
                detail: e.to_string(),
            })
        };
        let names: Vec<std::ffi::OsString> = std::fs::read_dir(self.dir())
            .map_err(unreadable)?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<Result<_, _>>()
            .map_err(unreadable)?;
        let mut moved: Vec<&std::ffi::OsString> = Vec::new();
        for name in &names {
            let (from, to) = (self.dir().join(name), self.target.join(name));
            if let Err(e) = move_one(&from, &to) {
                self.move_back(&moved);
                return Err(kept(ImportError::MoveFailed {
                    path: to,
                    detail: e.to_string(),
                }));
            }
            moved.push(name);
        }
        if let Err(e) = std::fs::remove_dir(self.dir()) {
            log::warn!(
                "library import: the emptied preview {} was left behind: {e}",
                self.dir().display()
            );
        }
        Ok(())
    }

    /// Best effort after a failed move: the entries already moved go back
    /// into the staging folder, so the target is empty again.
    fn move_back(&self, moved: &[&std::ffi::OsString]) {
        for name in moved.iter().rev() {
            let (from, to) = (self.target.join(name), self.dir().join(name));
            if let Err(e) = move_entry(&from, &to) {
                log::error!(
                    "library import: {} not moved back into the preview: {e}",
                    from.display()
                );
            }
        }
    }

    /// The fallback across file systems: copy into a target this creates,
    /// then discard the staging folder. A copy that fails removes what it
    /// wrote and keeps the preview.
    fn copy_into_place(&self) -> Result<(), MoveFailure> {
        let failed = |e: std::io::Error| ImportError::MoveFailed {
            path: self.target.clone(),
            detail: e.to_string(),
        };
        if let Err(e) = std::fs::create_dir(&self.target) {
            return Err(kept(if e.kind() == ErrorKind::AlreadyExists {
                self.changed()
            } else {
                failed(e)
            }));
        }
        if let Err(e) = copy_tree(self.dir(), &self.target) {
            // The target was created just now, so all of it is the copy's.
            if let Err(why) = std::fs::remove_dir_all(&self.target) {
                log::error!(
                    "library import: the partial copy {} was not removed: {why}",
                    self.target.display()
                );
            }
            return Err(kept(failed(e)));
        }
        if !self.discard() {
            log::warn!(
                "library import: the copied preview {} was left behind",
                self.dir().display()
            );
        }
        Ok(())
    }

    /// The run as if it had written into the target: its folder, summary
    /// and file paths moved from the staging folder.
    pub fn run_in_target(&self) -> RunOutcome {
        let moved = |path: &Path| match path.strip_prefix(self.dir()) {
            Ok(rel) => self.target.join(rel),
            Err(_) => path.to_path_buf(),
        };
        let mut run = self.run.clone();
        run.library = Some(self.target.clone());
        run.summary.library = run.summary.library.as_deref().map(moved);
        run.summary.report = run.summary.report.as_deref().map(moved);
        run.summary.files = run
            .summary
            .files
            .iter()
            .map(|p| moved(p.as_path()))
            .collect();
        run
    }

    /// The `.snxlib` once moved into the target.
    pub fn library_file_in_target(&self) -> PathBuf {
        match self.library_file.strip_prefix(self.dir()) {
            Ok(rel) => self.target.join(rel),
            Err(_) => self.library_file.clone(),
        }
    }
}

/// A refusal that keeps the preview.
fn kept(error: ImportError) -> MoveFailure {
    MoveFailure {
        error,
        preview_kept: true,
    }
}

/// A rename that failed because the two paths are on different file
/// systems.
fn crosses_devices(e: &std::io::Error) -> bool {
    e.kind() == ErrorKind::CrossesDevices
}

/// Move one entry from `from` to `to`, which must not exist: a rename, or
/// a copy then a removal across file systems. A partial copy is removed.
fn move_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if crosses_devices(&e) => {}
        done => return done,
    }
    let meta = std::fs::symlink_metadata(from)?;
    if is_link(&meta) {
        return Err(std::io::Error::other(format!(
            "{} is a link",
            from.display()
        )));
    }
    let copied = if meta.is_dir() {
        std::fs::create_dir(to).and_then(|()| copy_tree(from, to))
    } else {
        std::fs::copy(from, to).map(|_| ())
    };
    if let Err(e) = copied {
        let undone = if meta.is_dir() {
            std::fs::remove_dir_all(to)
        } else {
            std::fs::remove_file(to)
        };
        if let Err(why) = undone
            && why.kind() != ErrorKind::NotFound
        {
            log::error!(
                "library import: the partial copy {} stays: {why}",
                to.display()
            );
        }
        return Err(e);
    }
    if meta.is_dir() {
        std::fs::remove_dir_all(from)
    } else {
        std::fs::remove_file(from)
    }
}

/// Copy the folder `from` into the existing empty folder `to`. A link is
/// refused, never followed: a converter writes plain files.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if kind.is_symlink() {
            return Err(std::io::Error::other(format!(
                "{} is a link",
                entry.path().display()
            )));
        }
        if kind.is_dir() {
            std::fs::create_dir(&dest)?;
            copy_tree(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}
