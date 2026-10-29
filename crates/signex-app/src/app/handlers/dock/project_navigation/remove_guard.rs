//! Containment guard for the Remove-from-Project "Delete file" choice (#139).
//!
//! A project can reference `.snxlib` packages that live outside its own
//! folder (`Shared` / `Global` libraries). Those belong to other projects
//! too, so the project tree must never delete them from disk.

use std::path::Path;

use crate::app::path_op_jobs::{is_inside_project_root, same_path};

/// Should the dialog treat `target` as living outside the project? An
/// orphan (missing) target has nothing on disk to delete, so it is not
/// flagged — the exclude/delete choices are equivalent for it.
pub(super) fn is_outside_project(target: &Path, project_root: Option<&Path>) -> bool {
    if !target.exists() {
        return false;
    }
    match project_root {
        Some(root) => !is_inside_project_root(target, root),
        None => true,
    }
}

/// Does `target` resolve to a library the project references as `Shared` or
/// `Global`? Such a library belongs to other projects too, so it is outside
/// this project whatever its path says, even a Shared package that happens
/// to sit inside the project folder (#139).
pub(super) fn is_shared_or_global_library(
    project: &signex_types::project::ProjectData,
    target: &Path,
) -> bool {
    use signex_types::project::LibraryEntryKind;
    project.libraries.iter().any(|entry| {
        entry.kind != LibraryEntryKind::ProjectLocal
            && same_path(&project.resolve_library_path(entry), target)
    })
}
