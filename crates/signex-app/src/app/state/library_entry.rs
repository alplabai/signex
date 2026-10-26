//! How a `.snxlib` is recorded on a project: the one decision every
//! registration site shares.
//!
//! Inside the project directory the entry is stored relative to it
//! (`ProjectLocal`, so the project stays portable); anywhere else it is stored
//! by absolute path (`Shared`). "Inside" is judged under the project's own
//! [`CaseFold`], so `LIBS/Lib.snxlib` and `libs/Lib.snxlib` are one library on
//! a case-insensitive volume, and a picker returning `C:\Proj\..` for a project
//! opened as `c:\proj` still counts as inside.

use std::path::{Path, PathBuf};

use signex_types::project::{LibraryEntryKind, ProjectData};

use super::scope::{CaseFold, path_is_under, path_key_with};

/// True when `path` lies strictly below `project_dir` under `fold`. An empty
/// `project_dir` (a project with no directory yet) contains nothing.
pub(crate) fn is_inside_project(project_dir: &Path, path: &Path, fold: CaseFold) -> bool {
    !project_dir.as_os_str().is_empty()
        && path_key_with(path, fold) != path_key_with(project_dir, fold)
        && path_is_under(path, project_dir, fold)
}

/// The path to store and the kind to record for a library at `lib_path`.
pub(crate) fn library_entry_location(
    project_dir: &Path,
    lib_path: &Path,
    fold: CaseFold,
) -> (PathBuf, LibraryEntryKind) {
    if is_inside_project(project_dir, lib_path, fold) {
        // Under `fold` the two spell one prefix, so it is as many components
        // long as the project directory even when the casing differs.
        let rel: PathBuf = lib_path
            .components()
            .skip(project_dir.components().count())
            .collect();
        (rel, LibraryEntryKind::ProjectLocal)
    } else {
        (lib_path.to_path_buf(), LibraryEntryKind::Shared)
    }
}

/// True when an entry of `project` already resolves to `lib_path`, compared
/// under `fold`.
pub(crate) fn is_library_listed(project: &ProjectData, lib_path: &Path, fold: CaseFold) -> bool {
    let wanted = path_key_with(lib_path, fold);
    project
        .libraries
        .iter()
        .any(|e| path_key_with(&project.resolve_library_path(e), fold) == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    const APFS: CaseFold = CaseFold::case_only();

    #[test]
    fn a_case_variant_of_the_project_dir_is_still_inside() {
        let dir = Path::new("/p/proj");
        let lib = Path::new("/P/PROJ/LIBS/Lib.snxlib");
        let (path, kind) = library_entry_location(dir, lib, APFS);
        assert_eq!(kind, LibraryEntryKind::ProjectLocal);
        assert_eq!(path, Path::new("LIBS/Lib.snxlib"));
    }

    #[test]
    fn a_case_sensitive_volume_keeps_the_variant_outside() {
        let (path, kind) = library_entry_location(
            Path::new("/p/proj"),
            Path::new("/P/PROJ/Lib.snxlib"),
            CaseFold::PRESERVE,
        );
        assert_eq!(kind, LibraryEntryKind::Shared);
        assert_eq!(path, Path::new("/P/PROJ/Lib.snxlib"));
    }

    #[test]
    fn an_outside_library_and_an_empty_project_dir_are_shared() {
        let lib = Path::new("/elsewhere/Lib.snxlib");
        let (path, kind) = library_entry_location(Path::new("/p/proj"), lib, APFS);
        assert_eq!((path.as_path(), kind), (lib, LibraryEntryKind::Shared));
        let (_, kind) = library_entry_location(Path::new(""), lib, APFS);
        assert_eq!(kind, LibraryEntryKind::Shared);
    }

    #[test]
    fn the_project_dir_itself_is_not_a_library_location() {
        assert!(!is_inside_project(Path::new("/p"), Path::new("/P"), APFS));
    }
}
