//! The `--output` folder of a new import or a dry run (#3117, #3123): what
//! the run takes before the converter starts, and what a failed or
//! cancelled run removes. A failed run removes only what it created: a
//! folder it made goes with what is in it, an empty folder that was there
//! is emptied and kept, and the parent folders it made go while they are
//! empty. An update's folder is never touched here. A staged preview's
//! folder (#3171) is only ever created, never adopted, and is discarded
//! through its own identity ([`ClaimedOutput::discard`]).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use super::empty_dir::FolderHold;
#[cfg(unix)]
use super::empty_dir::folder_id;
use super::empty_dir::{FolderId, empty_folder, folder_id_at};
use super::error::ImportError;
use super::preview_lock::PreviewLock;
use super::run::{LibraryRunRequest, RunMode};

/// How the run holds its output folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// An update's folder: never this run's to remove or empty.
    NotOwned,
    /// This run created the folder.
    Created,
    /// An empty folder that was there, with its identity when the run
    /// took it: emptied after a failure, kept.
    Adopted(FolderId),
}

/// The output folder as a run has taken it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedOutput {
    folder: PathBuf,
    pub held: Held,
    /// The highest parent folder this run created, when it created any.
    pub created_parent: Option<PathBuf>,
    /// The adopted folder, held open until the run lets go of it, so it
    /// cannot be renamed or deleted meanwhile (Windows).
    #[cfg(windows)]
    hold: Option<FolderHold>,
    /// The identity of the folder this run created, read right after it
    /// was made (volume and file index on Windows); `None` when it could
    /// not be read, and then `discard` removes nothing.
    created_id: Option<FolderId>,
    /// A staged preview's lock (#3234), held from right after the folder
    /// was made until it is discarded or the last clone drops, so another
    /// Signex's sweep leaves the folder alone. `None` for any other run, or
    /// when the lock file could not be made (the sweep then goes by age).
    preview_lock: Option<PreviewLock>,
}

/// Take `--output` for a new import or a dry run. An absent folder is
/// created with its parents; an empty one is adopted as it is, keeping its
/// permissions. One that holds anything, a link, or anything that is not a
/// folder is refused before the converter starts. An update holds nothing.
/// A staged preview's folder must not exist at all: anything at its path,
/// even an empty folder, is refused.
pub fn claim_output(request: &LibraryRunRequest) -> Result<ClaimedOutput, ImportError> {
    let out = &request.output;
    let claimed = |held, created_parent| ClaimedOutput {
        folder: out.clone(),
        held,
        created_parent,
        #[cfg(windows)]
        hold: None,
        created_id: None,
        preview_lock: None,
    };
    if request.mode == RunMode::Update {
        return Ok(claimed(Held::NotOwned, None));
    }
    let unusable = |e: std::io::Error| ImportError::OutputUnusable {
        path: out.clone(),
        detail: e.to_string(),
    };
    let taken = || ImportError::OutputNotEmpty { path: out.clone() };
    match std::fs::symlink_metadata(out) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(unusable(e)),
        Ok(_) if request.mode == RunMode::Stage => {
            return Err(ImportError::StagingExists { path: out.clone() });
        }
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(ImportError::OutputIsLink { path: out.clone() });
        }
        Ok(meta) if meta.is_dir() => {
            #[cfg(unix)]
            let adopted = claimed(Held::Adopted(folder_id(&meta)), None);
            #[cfg(windows)]
            let adopted = {
                let hold = hold_adopted(out)?;
                let mut adopted = claimed(Held::Adopted(hold.id()), None);
                adopted.hold = Some(hold);
                adopted
            };
            let mut entries = std::fs::read_dir(out).map_err(unusable)?;
            return match entries.next() {
                None => Ok(adopted),
                Some(_) => Err(taken()),
            };
        }
        Ok(_) => return Err(taken()),
    }
    let created_parent = match out.parent() {
        Some(parent) => {
            let highest = highest_missing(parent).map_err(unusable)?;
            std::fs::create_dir_all(parent).map_err(unusable)?;
            highest
        }
        None => None,
    };
    let mut held = claimed(Held::Created, created_parent);
    // create_dir fails when anything appeared there since.
    if let Err(e) = std::fs::create_dir(out) {
        held.remove_created_parents();
        return Err(match e.kind() {
            ErrorKind::AlreadyExists if request.mode == RunMode::Stage => {
                ImportError::StagingExists { path: out.clone() }
            }
            ErrorKind::AlreadyExists => taken(),
            _ => unusable(e),
        });
    }
    // Recorded for `discard`; on Windows read from the folder opened
    // without following a reparse point. The folder is not held: a hold
    // shares no delete, so it would block the run's own removal of it and
    // a staged preview's rename into place.
    held.created_id = folder_id_at(out).ok();
    if request.mode == RunMode::Stage {
        held.preview_lock = take_preview_lock(out);
    }
    Ok(held)
}

/// The lock of the staging folder `out`, just made. A file system that
/// cannot make or lock it is logged and the preview staged without one.
fn take_preview_lock(out: &Path) -> Option<PreviewLock> {
    PreviewLock::take(out)
        .inspect_err(|e| {
            log::warn!(
                "library import: the preview {} is not locked: {e}",
                out.display()
            );
        })
        .ok()
}

/// Hold the empty folder a run adopts, before it is checked for entries.
/// A reparse point std did not report as a link is refused as one.
#[cfg(windows)]
fn hold_adopted(out: &Path) -> Result<FolderHold, ImportError> {
    FolderHold::open(out).map_err(|e| {
        if e.kind() == ErrorKind::NotADirectory {
            ImportError::OutputIsLink {
                path: out.to_path_buf(),
            }
        } else {
            ImportError::OutputUnusable {
                path: out.to_path_buf(),
                detail: e.to_string(),
            }
        }
    })
}

/// The highest of `dir` and its ancestors that does not exist, walking up
/// to the first that does.
fn highest_missing(dir: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut highest = None;
    for ancestor in dir.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(e) if e.kind() == ErrorKind::NotFound => highest = Some(ancestor.to_path_buf()),
            Err(e) => return Err(e),
        }
    }
    Ok(highest)
}

impl ClaimedOutput {
    /// The folder the run took.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Remove a folder this run created and kept (a staged preview, #3171)
    /// with the parent folders it made while they are empty. The folder is
    /// emptied through the identity recorded when it was made, never
    /// following a link ([`empty_folder`]), then removed only while empty;
    /// a folder swapped in at its path is left alone. True when it is gone.
    pub fn discard(&self) -> bool {
        if self.held != Held::Created {
            return false;
        }
        let Some(id) = &self.created_id else {
            log::error!(
                "library import: {} not removed: its identity is unknown",
                self.folder.display()
            );
            return false;
        };
        let gone = match empty_folder(&self.folder, id) {
            Err(e) if e.kind() == ErrorKind::NotFound => true,
            Err(e) => {
                log::error!("library import: {} not emptied: {e}", self.folder.display());
                false
            }
            Ok(()) => match std::fs::remove_dir(&self.folder) {
                Ok(()) => true,
                Err(e) if e.kind() == ErrorKind::NotFound => true,
                Err(e) => {
                    log::warn!(
                        "library import: could not remove {}: {e}",
                        self.folder.display()
                    );
                    false
                }
            },
        };
        self.release_preview_lock();
        self.remove_created_parents();
        gone
    }

    /// After a failed or cancelled run: remove what this run created and
    /// empty a folder it adopted. True when nothing the run wrote is left.
    pub fn release_after_failure(&self) -> bool {
        let released = match self.held {
            Held::NotOwned => return false,
            Held::Created => match std::fs::remove_dir_all(&self.folder) {
                Ok(()) => true,
                Err(e) => {
                    log::warn!(
                        "library import: could not remove {}: {e}",
                        self.folder.display()
                    );
                    false
                }
            },
            Held::Adopted(id) => self.empty_adopted(&id),
        };
        self.release_preview_lock();
        self.remove_created_parents();
        released
    }

    /// Remove what the run wrote inside the adopted folder, never following
    /// a link (`empty_dir`); a folder swapped in at its path is left alone.
    fn empty_adopted(&self, id: &FolderId) -> bool {
        match empty_folder(&self.folder, id) {
            Ok(()) => true,
            Err(e) => {
                log::error!("library import: {} not emptied: {e}", self.folder.display());
                false
            }
        }
    }

    /// Let go of a staged preview's lock once its folder is removed or
    /// moved: the lock file goes first, so the parent folders the run made
    /// can.
    pub(super) fn release_preview_lock(&self) {
        if let Some(lock) = &self.preview_lock {
            lock.release();
        }
    }

    /// Remove the parent folders this run created, bottom-up, while each is
    /// empty. Never recursive: a folder someone put anything in stays.
    fn remove_created_parents(&self) {
        let Some(highest) = &self.created_parent else {
            return;
        };
        let Some(parent) = self.folder.parent() else {
            return;
        };
        for dir in parent.ancestors() {
            if std::fs::remove_dir(dir).is_err() || dir == highest.as_path() {
                return;
            }
        }
    }
}
