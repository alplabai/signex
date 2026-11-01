//! The blocking half of a rename or delete (#859).
//!
//! Each job owns plain data, touches no `Signex` state, and runs on a worker
//! thread; its result goes back to `update` as a completion message that does
//! the in-memory re-key (see `path_ops` for why that split needs a guard).

use std::path::{Path, PathBuf};

use signex_types::project::{LibraryRetarget, retarget_library_entry};

use crate::app::parent_reference_disk::{ParentDiskOutcomes, Renamed, patch_parents};
use crate::app::path_ops::ReferenceRewrite;
use crate::app::path_ops::{DeleteIo, LibraryRenameIo, ParentSheets, RenameIoError};
use crate::app::project_entry_disk::{EntryPatch, remove_entry, retarget_sheet_entry};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::persist::SaveBarrier;

/// Runs a blocking job on tokio's blocking pool. A worker that panicked or was
/// cancelled comes back as text so the completion can report it.
pub async fn run_blocking<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| format!("the background task failed: {e}"))
}

/// Moves one file. Also reads the parent sheets a reference rewrite will
/// need, so the completion never reads a file inside `update`.
pub struct RenameJob {
    pub from: PathBuf,
    pub to: PathBuf,
    pub saves: SaveBarrier,
    pub parents_to_read: Vec<PathBuf>,
    /// The owning project's saved `.snxprj` and case rules, with the new file
    /// name: its entry for `from` is re-pointed in the same job, so the saved
    /// project never names a file that is gone (#1821).
    pub project: Option<ProjectEntryPatch>,
    /// Every parent reference to the file: re-pointed on disk once the move
    /// and the project entry have landed (#1849).
    pub rewrites: Vec<ReferenceRewrite>,
    /// The saved parents to scan for references to the file, which may be
    /// more than `rewrites` names (#1849).
    pub parent_candidates: Vec<PathBuf>,
    pub fold: crate::app::state::scope::CaseFold,
}

/// A finished rename job: the move and project patch, the parents read for the
/// in-memory fallback, and what the saved parents took.
pub type RenameJobDone = (
    Result<EntryPatch, RenameIoError>,
    ParentSheets,
    ParentDiskOutcomes,
);

/// The saved project a rename commits its entry change to.
pub struct ProjectEntryPatch {
    pub project_file: PathBuf,
    pub new_name: String,
    pub fold: crate::app::state::scope::CaseFold,
}

impl RenameJob {
    pub fn run(self) -> RenameJobDone {
        // A write still running for this file would recreate the old name or
        // land after the move; let it finish first. New writes are refused
        // while the operation is registered, so nothing can start after this.
        self.saves.wait();
        let parents = read_parent_sheets(&self.parents_to_read);
        let result = self.move_and_patch();
        // The file moved and the project follows (or had nothing to patch):
        // the parents' saved references must follow it too, whatever the
        // project said. A failed move or a moved-back file changes none.
        let parent_disk = if result.is_ok() {
            let new_name = self.to.file_name().unwrap_or_default().to_string_lossy();
            let renamed = Renamed {
                old: &self.from,
                new_name: &new_name,
                fold: self.fold,
            };
            patch_parents(&self.parent_candidates, &self.rewrites, &renamed)
        } else {
            ParentDiskOutcomes::new()
        };
        (result, parents, parent_disk)
    }

    /// Move the file, then re-point the saved project's entry. When the
    /// project cannot be written the move is undone, so disk never holds a
    /// renamed file under an unrenamed project (#1821).
    fn move_and_patch(&self) -> Result<EntryPatch, RenameIoError> {
        move_file(&self.from, &self.to)?;
        let Some(project) = &self.project else {
            return Ok(EntryPatch::NotListed);
        };
        retarget_sheet_entry(
            &project.project_file,
            &self.from,
            &project.new_name,
            project.fold,
        )
        .map_err(|e| RenameIoError::ProjectNotPatched {
            reason: format!("the project file could not be written ({e})"),
            moved_back: std::fs::rename(&self.to, &self.from).map_err(|e| e.to_string()),
        })
    }
}

fn read_parent_sheets(paths: &[PathBuf]) -> ParentSheets {
    paths
        .iter()
        .map(|path| (path.clone(), read_sheet(path).map(Box::new)))
        .collect()
}

fn read_sheet(path: &Path) -> Result<signex_types::schematic::SchematicSheet, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not be read: {e}"))?;
    signex_types::format::SnxSchematic::parse(&text)
        .map(|parsed| parsed.sheet)
        .map_err(|e| format!("could not be parsed: {e}"))
}

/// `fs::rename` replaces an existing file on Unix, so the destination is
/// checked first, as the synchronous code did. On a case-insensitive volume a
/// case-only rename finds itself at the destination; that is the same file,
/// not a collision, so it moves through a temporary name (#1832).
fn move_file(from: &Path, to: &Path) -> Result<(), RenameIoError> {
    if to.exists() {
        if !is_case_only_rename_of_same_file(from, to) {
            return Err(RenameIoError::TargetExists);
        }
        return move_via_temp_name(from, to);
    }
    std::fs::rename(from, to).map_err(|e| RenameIoError::Os(e.to_string()))
}

/// Do `from` and `to` differ only in letter case and name the same file on
/// disk under one stored name? File identity is required, not just the folded
/// name, so a distinct file whose name folds the same (a case-sensitive
/// volume) is still a collision. Identity alone also holds for two hard links
/// to one file, so the folder must not list `to`'s spelling as an entry of
/// its own: a case-insensitive volume lists only the stored spelling, while a
/// hard link is listed under its own name. An identity or listing that cannot
/// be read counts as different.
fn is_case_only_rename_of_same_file(from: &Path, to: &Path) -> bool {
    let (Some(from_name), Some(to_name)) = (from.file_name(), to.file_name()) else {
        return false;
    };
    let fold = CaseFold::from_probe(true);
    let folded = |name: &std::ffi::OsStr| path_key_with(Path::new(name), fold);
    from.parent() == to.parent()
        && from_name != to_name
        && folded(from_name) == folded(to_name)
        && same_file::is_same_file(from, to).unwrap_or(false)
        && !folder_lists_name(to)
}

/// Whether `path`'s folder holds an entry spelled exactly as `path`'s file
/// name. A folder that cannot be listed answers yes, so the caller refuses.
fn folder_lists_name(path: &Path) -> bool {
    let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
        return true;
    };
    let folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let Ok(mut entries) = std::fs::read_dir(folder) else {
        return true;
    };
    // An entry that cannot be read might be `name`, so it counts as a match.
    entries.any(|entry| match entry {
        Ok(entry) => entry.file_name() == name,
        Err(_) => true,
    })
}

/// Move `from` to a fresh name beside it, then onto `to`, so a filesystem that
/// treats the two names as one still records the new spelling. A second step
/// that fails puts the file back under `from`.
fn move_via_temp_name(from: &Path, to: &Path) -> Result<(), RenameIoError> {
    let os = |e: std::io::Error| RenameIoError::Os(e.to_string());
    let temp = unused_sibling_name(from).ok_or_else(|| {
        RenameIoError::Os("no free temporary name for a case-only rename".to_string())
    })?;
    std::fs::rename(from, &temp).map_err(os)?;
    if let Err(e) = std::fs::rename(&temp, to) {
        let back = std::fs::rename(&temp, from).map_or_else(
            |b| format!("; it is left at {} ({b})", temp.display()),
            |()| String::new(),
        );
        return Err(RenameIoError::Os(format!("{e}{back}")));
    }
    Ok(())
}

/// A name beside `path` that nothing occupies, for the middle step of a
/// case-only rename.
fn unused_sibling_name(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy();
    (0..100u32)
        .map(|n| path.with_file_name(format!(".{name}.rename-{}-{n}", std::process::id())))
        .find(|candidate| matches!(candidate.try_exists(), Ok(false)))
}

/// Moves a `.snxlib` and re-points its saved `.snxprj` entry, undoing the
/// move when the project file cannot follow.
pub struct LibraryRenameJob {
    pub old: PathBuf,
    pub new: PathBuf,
    pub project_path: PathBuf,
    pub saves: SaveBarrier,
}

impl LibraryRenameJob {
    pub fn run(self) -> LibraryRenameIo {
        self.saves.wait();
        match move_file(&self.old, &self.new) {
            Err(RenameIoError::TargetExists) => return LibraryRenameIo::TargetExists,
            Err(RenameIoError::Os(e)) => return LibraryRenameIo::NotRenamed(e),
            Err(RenameIoError::ProjectNotPatched { reason, .. }) => {
                return LibraryRenameIo::NotRenamed(reason);
            }
            Ok(()) => {}
        }
        let new_name = self.new.file_name().unwrap_or_default();
        match retarget_library_entry(&self.project_path, &self.old, new_name) {
            Ok(outcome) => LibraryRenameIo::Moved(outcome),
            Err(e) => LibraryRenameIo::ProjectNotPatched {
                reason: format!("the project file could not be written ({e})"),
                moved_back: std::fs::rename(&self.new, &self.old).map_err(|e| e.to_string()),
            },
        }
    }
}

/// Undoes a library rename the app could not follow: points the saved
/// `.snxprj` back at the old name (when it was patched), then moves the
/// library back.
pub struct LibraryRollbackJob {
    pub old: PathBuf,
    pub new: PathBuf,
    pub project_path: PathBuf,
    pub patched: bool,
}

impl LibraryRollbackJob {
    pub fn run(self) -> (Result<(), String>, Result<(), String>) {
        let patch_undo = if self.patched {
            self.undo_patch()
        } else {
            Ok(())
        };
        let moved_back = std::fs::rename(&self.new, &self.old).map_err(|e| e.to_string());
        (patch_undo, moved_back)
    }

    fn undo_patch(&self) -> Result<(), String> {
        let old_name = self.old.file_name().unwrap_or_default();
        let outcome: Result<LibraryRetarget, _> =
            retarget_library_entry(&self.project_path, &self.new, old_name);
        outcome.map(|_| ()).map_err(|back| {
            format!(
                "; the project file {} still lists {}: {back}. Save the project to fix it",
                self.project_path.display(),
                self.new.display()
            )
        })
    }
}

/// Deletes a file or `.snxlib` directory, but only when it is provably inside
/// the owning project's folder and not a shared library (#139).
pub struct DeleteJob {
    pub target: PathBuf,
    pub project_root: Option<PathBuf>,
    /// Resolved paths of the project's Shared and Global libraries.
    pub shared_libraries: Vec<PathBuf>,
    pub saves: SaveBarrier,
    /// The owning project's saved `.snxprj` and case rules: its entry for the
    /// target is removed in the same job, so the saved project never names a
    /// file that is gone (#1849).
    pub project: Option<(PathBuf, crate::app::state::scope::CaseFold)>,
}

impl DeleteJob {
    pub fn run(self) -> (DeleteIo, EntryPatch) {
        match self.target.try_exists() {
            Ok(true) => {}
            // Nothing to delete, but the saved project still lists the row.
            Ok(false) => {
                return (
                    DeleteIo::Orphan,
                    self.drop_entry().unwrap_or_else(unwritten),
                );
            }
            Err(e) => return (DeleteIo::CheckFailed(e.to_string()), EntryPatch::NotListed),
        }
        if !self.is_deletable() {
            return (DeleteIo::Refused, EntryPatch::NotListed);
        }
        // A write still running for this file would recreate it after the
        // delete; let it finish first.
        self.saves.wait();
        self.delete_and_drop_entry()
    }

    /// Remove the project's entry first, then the file: a project that cannot
    /// be written deletes nothing, and a file that cannot be deleted gets its
    /// entry back, so disk never holds a project naming a deleted file nor an
    /// orphaned file the project forgot.
    fn delete_and_drop_entry(&self) -> (DeleteIo, EntryPatch) {
        let original = self
            .project
            .as_ref()
            .and_then(|(file, _)| std::fs::read(file).ok());
        let entry = match self.drop_entry() {
            Ok(entry) => entry,
            Err(e) => {
                let why =
                    format!("the project file could not be written ({e}); nothing was deleted");
                return (DeleteIo::Failed(why), EntryPatch::NotListed);
            }
        };
        let removed = if self.target.is_dir() {
            std::fs::remove_dir_all(&self.target)
        } else {
            std::fs::remove_file(&self.target)
        };
        let Err(e) = removed else {
            return (DeleteIo::Deleted, entry);
        };
        let mut why = e.to_string();
        if self.target.is_dir() {
            why.push_str("; some of its contents may already be gone");
        }
        let EntryPatch::Patched(_) = &entry else {
            return (DeleteIo::Failed(why), EntryPatch::NotListed);
        };
        let Some((file, _)) = &self.project else {
            return (DeleteIo::Failed(why), EntryPatch::NotListed);
        };
        // The entry is gone from the saved project: put the original bytes
        // back, and when that is impossible say the project is stale.
        let restored = match &original {
            Some(original) => signex_types::atomic_io::atomic_write(file, original)
                .map_err(|back| back.to_string()),
            None => Err("the original could not be read".to_string()),
        };
        match restored {
            Ok(()) => (DeleteIo::Failed(why), EntryPatch::NotListed),
            Err(back) => (
                DeleteIo::FailedProjectStale(format!(
                    "{why}; the project file {} no longer lists it and could not be restored \
                     ({back}). Save the project to list it again",
                    file.display()
                )),
                EntryPatch::NotListed,
            ),
        }
    }

    fn drop_entry(&self) -> Result<EntryPatch, signex_types::project::ProjectError> {
        match &self.project {
            Some((file, fold)) => remove_entry(file, &self.target, *fold),
            None => Ok(EntryPatch::NotListed),
        }
    }

    fn is_deletable(&self) -> bool {
        let contained = self
            .project_root
            .as_deref()
            .is_some_and(|root| is_inside_project_root(&self.target, root));
        contained
            && !self
                .shared_libraries
                .iter()
                .any(|library| same_path(library, &self.target))
    }
}

/// An orphan row whose project entry could not be dropped on disk: the
/// in-memory removal then waits for a save, as before.
fn unwritten(error: signex_types::project::ProjectError) -> EntryPatch {
    EntryPatch::Unreadable(error.to_string())
}

/// Is `target` a path strictly inside `project_root`, after symlinks and
/// `..` segments are resolved? Both paths are canonicalised, so this fails
/// closed: one that cannot be resolved is reported as not contained.
pub(crate) fn is_inside_project_root(target: &Path, project_root: &Path) -> bool {
    let (Ok(target), Ok(root)) = (target.canonicalize(), project_root.canonicalize()) else {
        return false;
    };
    target != root && target.starts_with(&root)
}

pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (a.canonicalize(), b.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
}
