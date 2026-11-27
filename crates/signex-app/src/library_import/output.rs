//! The `--output` folder of a new import or a dry run (#3117, #3123): what
//! the run takes before the converter starts, and what a failed or
//! cancelled run removes. A failed run removes only what it created: a
//! folder it made goes with what is in it, an empty folder that was there
//! is emptied and kept, and the parent folders it made go while they are
//! empty. An update's folder is never touched here.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::error::ImportError;
use super::run::{LibraryRunRequest, RunMode};

/// How the run holds its output folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// An update's folder: never this run's to remove or empty.
    NotOwned,
    /// This run created the folder.
    Created,
    /// An empty folder that was there: emptied after a failure, kept.
    Adopted,
}

/// The output folder as a run has taken it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedOutput {
    folder: PathBuf,
    pub held: Held,
    /// The highest parent folder this run created, when it created any.
    pub created_parent: Option<PathBuf>,
}

/// Take `--output` for a new import or a dry run. An absent folder is
/// created with its parents; an empty one is adopted as it is, keeping its
/// permissions. One that holds anything, a link, or anything that is not a
/// folder is refused before the converter starts. An update holds nothing.
pub fn claim_output(request: &LibraryRunRequest) -> Result<ClaimedOutput, ImportError> {
    let out = &request.output;
    let claimed = |held, created_parent| ClaimedOutput {
        folder: out.clone(),
        held,
        created_parent,
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
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(ImportError::OutputIsLink { path: out.clone() });
        }
        Ok(meta) if meta.is_dir() => {
            let mut entries = std::fs::read_dir(out).map_err(unusable)?;
            return match entries.next() {
                None => Ok(claimed(Held::Adopted, None)),
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
    let held = claimed(Held::Created, created_parent);
    // create_dir fails when anything appeared there since.
    if let Err(e) = std::fs::create_dir(out) {
        held.remove_created_parents();
        return Err(if e.kind() == ErrorKind::AlreadyExists {
            taken()
        } else {
            unusable(e)
        });
    }
    Ok(held)
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
            Held::Adopted => self.empty_adopted(),
        };
        self.remove_created_parents();
        released
    }

    /// Remove what the run wrote inside the adopted folder, never following
    /// a link: a link inside is removed itself, and a folder that has
    /// become a link is left alone.
    fn empty_adopted(&self) -> bool {
        let folder = &self.folder;
        let warn = |what: &Path, e: &std::io::Error| {
            log::warn!("library import: could not remove {}: {e}", what.display());
        };
        match std::fs::symlink_metadata(folder) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                log::warn!(
                    "library import: {} is no longer a folder; left as it is",
                    folder.display()
                );
                return false;
            }
            Err(e) => {
                warn(folder, &e);
                return false;
            }
        }
        let entries = match std::fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(e) => {
                warn(folder, &e);
                return false;
            }
        };
        let mut emptied = true;
        for entry in entries {
            let result = entry.and_then(|entry| remove_entry(&entry.path()));
            if let Err(e) = result {
                warn(folder, &e);
                emptied = false;
            }
        }
        emptied
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

/// Remove one entry of the adopted folder. A link is removed as a link
/// (on Windows a link to a folder is removed as a folder); a real folder
/// goes with what is in it, without following the links inside.
fn remove_entry(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return std::fs::remove_file(path).or_else(|e| std::fs::remove_dir(path).map_err(|_| e));
    }
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}
