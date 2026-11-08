//! The folder a run may name paths in (#1923): the project's own folder,
//! fixed once when the run starts. A project file is input an agent may have
//! edited, so nothing it lists (libraries, sheets) ever widens the root; a
//! run that needs another folder names it with `--allow-path`.

use std::path::{Path, PathBuf};

/// `project_dir` as the root, unless it is `home` or a folder above it: a
/// project file kept loose in the home folder (`signex-cli run ~/x.snxsch`)
/// would otherwise make the whole home folder writable.
pub fn project_root(project_dir: &Path, home: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    if home.is_some_and(|home| home.starts_with(project_dir)) {
        return Err(format!(
            "{} is the home folder or one above it, which is too broad to be the project's folder; \
             move the project into its own folder, or name the folder to allow with --allow-path",
            project_dir.display()
        ));
    }
    Ok(vec![project_dir.to_path_buf()])
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
            [project.clone()]
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
    fn a_folder_inside_home_is_fine() {
        let (_guard, base) = temp();
        let home = base.join("users").join("me");
        let inside = home.join("designs").join("demo");
        assert_eq!(project_root(&inside, Some(&home)).unwrap(), [inside]);
    }
}
