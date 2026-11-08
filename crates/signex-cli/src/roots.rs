//! The folders a run may name paths in (#1923), fixed once when the run
//! starts: the project's folder, and the libraries the app mounted from the
//! project's library list. A project file is input an agent may have edited,
//! so what it lists never widens the roots after the start, and never widens
//! them to a folder that holds more than a library.

use std::path::{Path, PathBuf};

use crate::confine::resolve;

/// `project_dir` plus each of `listed` (the project's library entries, a
/// relative one against `project_dir`) that resolves to an existing folder,
/// was really `mounted`, and is not too broad: not a filesystem root, not
/// `home`, and not a folder that holds the project. Anything else a run needs
/// is named with `--allow-path`.
pub fn allowed_roots(
    project_dir: &Path,
    listed: &[PathBuf],
    mounted: &[PathBuf],
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let mounted: Vec<PathBuf> = mounted
        .iter()
        .filter_map(|path| resolve(path).ok())
        .collect();
    let libraries = listed
        .iter()
        .filter_map(|entry| resolve(&project_dir.join(entry)).ok())
        .filter(|root| root.is_dir())
        .filter(|root| mounted.contains(root))
        .filter(|root| root.parent().is_some())
        .filter(|root| home != Some(root.as_path()))
        .filter(|root| !project_dir.starts_with(root));
    std::iter::once(project_dir.to_path_buf())
        .chain(libraries)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confine::canonical;

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let guard = tempfile::tempdir().unwrap();
        let root = canonical(guard.path()).unwrap();
        (guard, root)
    }

    /// A project folder with a sibling library folder, both under `base`.
    fn layout() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let (guard, base) = temp();
        let project = base.join("proj");
        let library = base.join("libs").join("parts.snxlib");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        (guard, base, project, library)
    }

    #[test]
    fn a_mounted_library_next_to_the_project_is_a_root() {
        let (_guard, _base, project, library) = layout();
        let listed = [PathBuf::from("../libs/parts.snxlib")];
        let roots = allowed_roots(&project, &listed, std::slice::from_ref(&library), None);
        assert_eq!(roots, [project, library]);
    }

    #[test]
    fn a_listed_library_that_was_not_mounted_is_not_a_root() {
        let (_guard, _base, project, _library) = layout();
        let listed = [PathBuf::from("../libs/parts.snxlib")];
        assert_eq!(allowed_roots(&project, &listed, &[], None), [project]);
    }

    #[test]
    fn parent_filesystem_root_home_and_ancestors_never_widen_the_roots() {
        let (_guard, base, project, _library) = layout();
        let home = base.join("libs");
        let broad = [
            PathBuf::from(".."),
            PathBuf::from("../.."),
            PathBuf::from(std::path::MAIN_SEPARATOR_STR),
            PathBuf::from("."),
            PathBuf::from("../libs"),
        ];
        // Even when the app reported every one of them as mounted.
        let mounted: Vec<PathBuf> = broad.iter().map(|entry| project.join(entry)).collect();
        let roots = allowed_roots(&project, &broad, &mounted, Some(&home));
        assert_eq!(roots, [project.clone()], "{roots:?}");
    }

    #[test]
    fn a_missing_listed_folder_is_skipped() {
        let (_guard, _base, project, library) = layout();
        let listed = [PathBuf::from("../libs/gone.snxlib")];
        let roots = allowed_roots(&project, &listed, &[library], None);
        assert_eq!(roots, [project]);
    }

    #[cfg(unix)]
    #[test]
    fn a_listed_link_to_outside_is_resolved_and_needs_to_be_the_mounted_folder() {
        let (_guard, base, project, _library) = layout();
        let (_other_guard, other) = temp();
        std::os::unix::fs::symlink(&other, base.join("proj").join("lib-link")).unwrap();
        let listed = [PathBuf::from("lib-link")];
        // The app mounted nothing there: the link does not widen the roots.
        assert_eq!(
            allowed_roots(&project, &listed, &[], None),
            [project.clone()]
        );
        // Mounted under its resolved path, it is a library like any other.
        let roots = allowed_roots(&project, &listed, std::slice::from_ref(&other), None);
        assert_eq!(roots, [project, other]);
    }
}
