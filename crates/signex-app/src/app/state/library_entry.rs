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

use crate::app::project_sheets::normalize_clamped;

use super::scope::{CaseFold, path_is_under, path_key_with};

/// `lib_path` relative to `project_dir`, or `None` when it is not strictly
/// below it under `fold`. Both sides are dot-collapsed first, because a project
/// directory can carry `..` (`cwd.join("../proj/P.snxprj")`) and counting raw
/// components would then cut the wrong prefix. The result is only returned when
/// it resolves back to `lib_path` under `fold`: an entry that would point at a
/// different file is never produced.
fn relative_inside(project_dir: &Path, lib_path: &Path, fold: CaseFold) -> Option<PathBuf> {
    if project_dir.as_os_str().is_empty()
        || path_key_with(lib_path, fold) == path_key_with(project_dir, fold)
        || !path_is_under(lib_path, project_dir, fold)
    {
        return None;
    }
    let dir = normalize_clamped(project_dir);
    let lib = normalize_clamped(lib_path);
    // Under `fold` the two spell one prefix, so it is as many components long
    // as the project directory even when the casing differs.
    let rel: PathBuf = lib.components().skip(dir.components().count()).collect();
    // `collect` joins with the host separator, so compare with `\` read as
    // one on both sides: only the file reached matters here.
    let lenient = CaseFold {
        backslash_is_separator: true,
        ..fold
    };
    let round_trips = !rel.as_os_str().is_empty()
        && path_key_with(&project_dir.join(&rel), lenient) == path_key_with(lib_path, lenient);
    round_trips.then_some(rel)
}

/// True when `path` lies strictly below `project_dir` under `fold`. An empty
/// `project_dir` (a project with no directory yet) contains nothing.
pub(crate) fn is_inside_project(project_dir: &Path, path: &Path, fold: CaseFold) -> bool {
    relative_inside(project_dir, path, fold).is_some()
}

/// The path to store and the kind to record for a library at `lib_path`.
pub(crate) fn library_entry_location(
    project_dir: &Path,
    lib_path: &Path,
    fold: CaseFold,
) -> (PathBuf, LibraryEntryKind) {
    match relative_inside(project_dir, lib_path, fold) {
        Some(rel) => (rel, LibraryEntryKind::ProjectLocal),
        None => (lib_path.to_path_buf(), LibraryEntryKind::Shared),
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
    use crate::app::handlers::dock::project_navigation::add_io::{ExistingPlan, plan_existing};

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

    /// `signex ../proj/P.snxprj` from `/home/u/work`: the stored directory
    /// keeps its `..`.
    const DOTTED: &str = "/home/u/work/../proj";

    #[test]
    fn a_dotted_project_dir_stores_the_path_that_resolves_to_the_library() {
        let dir = Path::new(DOTTED);
        for (lib, want) in [
            ("/home/u/proj/a/libs/L.snxlib", "a/libs/L.snxlib"),
            ("/home/u/proj/libs/L.snxlib", "libs/L.snxlib"),
        ] {
            let lib = Path::new(lib);
            let (path, kind) = library_entry_location(dir, lib, APFS);
            assert_eq!(kind, LibraryEntryKind::ProjectLocal, "{lib:?}");
            assert_eq!(path, Path::new(want), "{lib:?}");
            let sep = CaseFold {
                backslash_is_separator: true,
                ..APFS
            };
            assert_eq!(
                path_key_with(&dir.join(&path), sep),
                path_key_with(lib, sep),
                "the stored entry must resolve to the picked file"
            );
        }
    }

    #[test]
    fn a_dotted_project_dir_gives_one_answer_to_every_caller() {
        let dir = Path::new(DOTTED);
        for lib in [
            "/home/u/proj/a/libs/L.snxlib",
            "/home/u/proj/libs/L.snxlib",
            "/home/u/work/L.snxlib",
            "/home/u/proj",
        ] {
            let lib = Path::new(lib);
            let (path, kind) = library_entry_location(dir, lib, APFS);
            assert_eq!(
                is_inside_project(dir, lib, APFS),
                kind == LibraryEntryKind::ProjectLocal,
                "{lib:?}"
            );
            let plan = plan_existing(dir, lib.to_path_buf(), APFS);
            assert_eq!(
                matches!(plan, ExistingPlan::InProject(_)),
                kind == LibraryEntryKind::ProjectLocal,
                "{lib:?}"
            );
            if kind == LibraryEntryKind::Shared {
                assert_eq!(path, lib);
            }
        }
    }
}
