//! The blocking half of a rename or delete (#859).
//!
//! Each job owns plain data, touches no `Signex` state, and runs on a worker
//! thread; its result goes back to `update` as a completion message that does
//! the in-memory re-key (see `path_ops` for why that split needs a guard).

use std::path::{Path, PathBuf};

use signex_types::project::{LibraryRetarget, retarget_library_entry};

use crate::app::path_ops::{DeleteIo, LibraryRenameIo, ParentSheets, RenameIoError};
use crate::app::project_entry_disk::{EntryPatch, retarget_sheet_entry};
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
}

/// The saved project a rename commits its entry change to.
pub struct ProjectEntryPatch {
    pub project_file: PathBuf,
    pub new_name: String,
    pub fold: crate::app::state::scope::CaseFold,
}

impl RenameJob {
    pub fn run(self) -> (Result<EntryPatch, RenameIoError>, ParentSheets) {
        // A write still running for this file would recreate the old name or
        // land after the move; let it finish first. New writes are refused
        // while the operation is registered, so nothing can start after this.
        self.saves.wait();
        let parents = read_parent_sheets(&self.parents_to_read);
        (self.move_and_patch(), parents)
    }

    /// Move the file, then re-point the saved project's entry. When the
    /// project cannot follow the move is undone, so disk never holds a
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
            reason: format!("the project file could not be updated ({e}); nothing was changed"),
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
/// checked first, as the synchronous code did.
fn move_file(from: &Path, to: &Path) -> Result<(), RenameIoError> {
    if to.exists() {
        return Err(RenameIoError::TargetExists);
    }
    std::fs::rename(from, to).map_err(|e| RenameIoError::Os(e.to_string()))
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
                reason: format!("the project file could not be updated ({e}); nothing was changed"),
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
}

impl DeleteJob {
    pub fn run(self) -> DeleteIo {
        match self.target.try_exists() {
            Ok(true) => {}
            Ok(false) => return DeleteIo::Orphan,
            Err(e) => return DeleteIo::CheckFailed(e.to_string()),
        }
        if !self.is_deletable() {
            return DeleteIo::Refused;
        }
        // A write still running for this file would recreate it after the
        // delete; let it finish first.
        self.saves.wait();
        let removed = if self.target.is_dir() {
            std::fs::remove_dir_all(&self.target)
        } else {
            std::fs::remove_file(&self.target)
        };
        match removed {
            Ok(()) => DeleteIo::Deleted,
            Err(e) => DeleteIo::Failed(e.to_string()),
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
