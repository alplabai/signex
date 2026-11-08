//! The folders a run may name paths in (#1923), fixed once when the run
//! starts: the project's folder, and the libraries the app mounted from the
//! project's library list. A project file is input an agent may have edited,
//! so what it lists never widens the roots after the start, and only a
//! library's own folder is ever added.

use std::fs;
use std::path::{Path, PathBuf};

use crate::confine::resolve;

/// The file a library folder holds directly (`signex_library`'s manifest).
const LIBRARY_MANIFEST: &str = "library.toml";

/// `project_dir` plus each of `listed` (the project's library entries, a
/// relative one against `project_dir`) that is a library's own folder: it
/// resolves to a folder that holds [`LIBRARY_MANIFEST`] as a plain file, the
/// app really `mounted` it, and it is neither the folder of `home` or one
/// above it, nor one that holds the project. Anything else a run needs is
/// named with `--allow-path`.
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
        .filter(|root| mounted.contains(root))
        .filter(|root| holds_manifest(root))
        .filter(|root| !home.is_some_and(|home| home.starts_with(root)))
        .filter(|root| !project_dir.starts_with(root));
    std::iter::once(project_dir.to_path_buf())
        .chain(libraries)
        .collect()
}

fn holds_manifest(folder: &Path) -> bool {
    fs::symlink_metadata(folder.join(LIBRARY_MANIFEST)).is_ok_and(|meta| meta.file_type().is_file())
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

    fn make_library(folder: &Path) {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join(LIBRARY_MANIFEST), "x").unwrap();
    }

    /// A project folder with a sibling library folder, both under `base`.
    fn layout() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let (guard, base) = temp();
        let project = base.join("proj");
        let library = base.join("libs").join("parts.snxlib");
        fs::create_dir_all(&project).unwrap();
        make_library(&library);
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
    fn a_folder_without_the_manifest_is_not_a_library_root_even_when_mounted() {
        let (_guard, base, project, _library) = layout();
        let plain = base.join("plain");
        fs::create_dir_all(&plain).unwrap();
        // The manifest has to be a file directly inside, not a folder or deeper.
        let nested = base.join("nested");
        fs::create_dir_all(nested.join("inner")).unwrap();
        fs::write(nested.join("inner").join(LIBRARY_MANIFEST), "x").unwrap();
        let dir_manifest = base.join("dir-manifest");
        fs::create_dir_all(dir_manifest.join(LIBRARY_MANIFEST)).unwrap();
        let folders = [plain, nested, dir_manifest];
        let listed: Vec<PathBuf> = folders.to_vec();
        assert_eq!(allowed_roots(&project, &listed, &folders, None), [project],);
    }

    #[test]
    fn filesystem_root_home_ancestors_of_home_and_of_the_project_are_never_roots() {
        let (_guard, base, project, _library) = layout();
        // Each of these holds a manifest, and the app reported it mounted.
        let home = base.join("users").join("me");
        fs::create_dir_all(&home).unwrap();
        let broad = [
            home.clone(),
            home.parent().unwrap().to_path_buf(),
            base.clone(),
            PathBuf::from(std::path::MAIN_SEPARATOR_STR),
            project.clone(),
        ];
        for folder in &broad {
            // A real filesystem root is not ours to write a manifest into.
            if folder.parent().is_some() {
                fs::write(folder.join(LIBRARY_MANIFEST), "x").unwrap();
            }
        }
        let roots = allowed_roots(&project, &broad, &broad, Some(&home));
        assert_eq!(roots, std::slice::from_ref(&project), "{roots:?}");
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
    fn a_link_to_a_library_is_resolved_and_a_manifest_link_does_not_count() {
        let (_guard, base, project, _library) = layout();
        let (_other_guard, other) = temp();
        make_library(&other);
        std::os::unix::fs::symlink(&other, project.join("lib-link")).unwrap();
        let listed = [PathBuf::from("lib-link")];
        // Not mounted: the link does not widen the roots.
        assert_eq!(
            allowed_roots(&project, &listed, &[], None),
            std::slice::from_ref(&project)
        );
        // Mounted under its resolved path, it is a library like any other.
        let roots = allowed_roots(&project, &listed, std::slice::from_ref(&other), None);
        assert_eq!(roots, [project.clone(), other]);

        // A manifest that is itself a link (to a file elsewhere) is not one.
        let linked = base.join("linked-manifest");
        fs::create_dir_all(&linked).unwrap();
        std::os::unix::fs::symlink(
            base.join("libs")
                .join("parts.snxlib")
                .join(LIBRARY_MANIFEST),
            linked.join(LIBRARY_MANIFEST),
        )
        .unwrap();
        assert_eq!(
            allowed_roots(
                &project,
                std::slice::from_ref(&linked),
                std::slice::from_ref(&linked),
                None
            ),
            [project]
        );
    }
}
