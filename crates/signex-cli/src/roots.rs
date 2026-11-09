//! The folder a run may name paths in (#1923): the project's own folder,
//! fixed once when the run starts. A project file is input an agent may have
//! edited, so nothing it lists (libraries, sheets) ever widens the root; a
//! run that needs another folder names it with `--allow-path`.

use std::path::{Path, PathBuf};

use crate::confine::canonical;

/// `project_dir` as the root, unless it is a filesystem root, `home` or a
/// folder above it: a project file kept loose in the home folder
/// (`signex-cli run ~/x.snxsch`) or at the top of a drive would otherwise make
/// the whole home folder or drive writable.
pub fn project_root(project_dir: &Path, home: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    // The path as given and as it resolves (a link to `/` is `/`).
    let resolved = canonical(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
    let too_broad = [project_dir, resolved.as_path()]
        .into_iter()
        .any(|dir| is_filesystem_root(dir) || home.is_some_and(|home| home.starts_with(dir)));
    if too_broad {
        return Err(format!(
            "{} is a filesystem root, the home folder or one above it, which is too broad to be \
             the project's folder; move the project into its own folder",
            project_dir.display()
        ));
    }
    Ok(vec![project_dir.to_path_buf()])
}

/// Every root form: `/`, `C:\`, `\\?\C:\`, a UNC share root (`\\server\share\`)
/// have no parent; a path that is not absolute (`\` or `C:` on Windows, or a
/// relative one) is not a folder this can vouch for.
fn is_filesystem_root(dir: &Path) -> bool {
    !dir.is_absolute() || dir.parent().is_none()
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

    #[test]
    fn a_folder_of_its_own_is_the_root() {
        let (_guard, base) = temp();
        let project = base.join("proj");
        let home = base.join("users").join("me");
        assert_eq!(
            project_root(&project, Some(&home)).unwrap(),
            std::slice::from_ref(&project)
        );
        assert_eq!(project_root(&project, None).unwrap(), [project]);
    }

    #[test]
    fn home_a_folder_above_it_and_a_filesystem_root_are_refused() {
        let (_guard, base) = temp();
        let home = base.join("users").join("me");
        for broad in [
            home.clone(),
            home.parent().unwrap().to_path_buf(),
            base.clone(),
            PathBuf::from(std::path::MAIN_SEPARATOR_STR),
        ] {
            let err = project_root(&broad, Some(&home)).unwrap_err();
            assert!(err.contains("too broad"), "{}: {err}", broad.display());
        }
    }

    #[test]
    fn a_filesystem_root_is_refused_even_with_no_home_known() {
        let root = Path::new(std::path::MAIN_SEPARATOR_STR);
        let err = project_root(root, None).unwrap_err();
        assert!(err.contains("filesystem root"), "{err}");
        let err = project_root(Path::new("relative/folder"), None).unwrap_err();
        assert!(err.contains("too broad"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_resolves_to_a_filesystem_root_is_refused() {
        let (_guard, base) = temp();
        let link = base.join("to-root");
        std::os::unix::fs::symlink("/", &link).unwrap();
        let err = project_root(&link, None).unwrap_err();
        assert!(err.contains("too broad"), "{err}");
    }

    /// The root forms Windows has: a drive root, the verbatim form, a UNC
    /// share root, the drive-relative `\` and `C:`.
    #[cfg(windows)]
    #[test]
    fn every_windows_root_form_is_refused() {
        for form in [
            r"\",
            r"C:\",
            r"C:",
            r"\\?\C:\",
            r"\\server\share\",
            r"\\?\UNC\server\share\",
        ] {
            let err = project_root(Path::new(form), None).unwrap_err();
            assert!(err.contains("too broad"), "{form}: {err}");
        }
        // A folder on a drive is fine, and so is a UNC folder below a share.
        for form in [r"C:\projects\demo", r"\\server\share\projects\demo"] {
            assert!(project_root(Path::new(form), None).is_ok(), "{form}");
        }
    }

    #[test]
    fn a_folder_inside_home_is_fine() {
        let (_guard, base) = temp();
        let home = base.join("users").join("me");
        let inside = home.join("designs").join("demo");
        assert_eq!(project_root(&inside, Some(&home)).unwrap(), [inside]);
    }
}
