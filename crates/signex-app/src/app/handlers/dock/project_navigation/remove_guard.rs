//! Containment guard for the Remove-from-Project "Delete file" choice (#139).
//!
//! A project can reference `.snxlib` packages that live outside its own
//! folder (`Shared` / `Global` libraries). Those belong to other projects
//! too, so the project tree must never delete them from disk.

use std::path::Path;

/// Is `target` a path strictly inside `project_root`, after symlinks and
/// `..` segments are resolved?
///
/// Both paths are canonicalised, so this fails closed: a target or root that
/// cannot be resolved (missing, unreadable) is reported as not contained.
pub(super) fn is_inside_project_root(target: &Path, project_root: &Path) -> bool {
    let (Ok(target), Ok(root)) = (target.canonicalize(), project_root.canonicalize()) else {
        return false;
    };
    target != root && target.starts_with(&root)
}

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

fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (a.canonicalize(), b.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
}
