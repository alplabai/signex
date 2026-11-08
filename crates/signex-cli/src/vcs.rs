//! Git on save (#1923): a project with git enabled makes the app commit
//! after a save, and the app's git open follows a `.git` file (`gitdir:`), a
//! `.git` link, `commondir` and alternates to wherever they point. A save is
//! therefore allowed only when the repository is a real folder of the project.

use std::fs;
use std::path::{Path, PathBuf};

use crate::confine::confine;

/// Files inside a git folder that point the repository somewhere else.
const REDIRECTS: [&str; 2] = ["commondir", "objects/info/alternates"];

/// The repository folder of the project at `project_dir`, if it is a plain
/// folder inside `roots` that redirects nowhere. A missing one is refused
/// too: the app would run `git init` and commit, which nobody asked for.
pub fn check_repo(project_dir: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let git = project_dir.join(".git");
    let meta = fs::symlink_metadata(&git).map_err(|_| {
        format!(
            "{} has no .git folder; saving would run `git init` and commit",
            project_dir.display()
        )
    })?;
    if !meta.file_type().is_dir() {
        return Err(format!(
            "{} is a link or a `gitdir:` file, not a folder of this project; refusing to commit through it",
            git.display()
        ));
    }
    for redirect in REDIRECTS {
        if fs::symlink_metadata(git.join(redirect)).is_ok() {
            return Err(format!(
                "{} holds `{redirect}`, which points the repository elsewhere; refusing to commit",
                git.display()
            ));
        }
    }
    confine(&git, roots)?;
    Ok(git)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confine::canonical;

    fn project() -> (tempfile::TempDir, PathBuf) {
        let guard = tempfile::tempdir().unwrap();
        let dir = canonical(guard.path()).unwrap();
        (guard, dir)
    }

    #[test]
    fn a_real_git_folder_inside_the_project_is_accepted() {
        let (_guard, dir) = project();
        fs::create_dir_all(dir.join(".git").join("objects").join("info")).unwrap();
        let git = check_repo(&dir, std::slice::from_ref(&dir)).unwrap();
        assert_eq!(git, dir.join(".git"));
    }

    #[test]
    fn a_missing_git_folder_is_refused_because_saving_would_init_one() {
        let (_guard, dir) = project();
        let err = check_repo(&dir, std::slice::from_ref(&dir)).unwrap_err();
        assert!(err.contains("would run `git init`"), "{err}");
        assert!(!dir.join(".git").exists(), "nothing was created");
    }

    #[test]
    fn a_gitdir_file_is_refused() {
        let (_guard, dir) = project();
        let (_other, elsewhere) = project();
        fs::write(
            dir.join(".git"),
            format!("gitdir: {}", elsewhere.join(".git").display()),
        )
        .unwrap();
        let err = check_repo(&dir, std::slice::from_ref(&dir)).unwrap_err();
        assert!(err.contains("link or a `gitdir:` file"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_git_link_is_refused_even_to_a_folder_inside_the_project() {
        let (_guard, dir) = project();
        fs::create_dir_all(dir.join("repo")).unwrap();
        std::os::unix::fs::symlink(dir.join("repo"), dir.join(".git")).unwrap();
        let err = check_repo(&dir, std::slice::from_ref(&dir)).unwrap_err();
        assert!(err.contains("link or a `gitdir:` file"), "{err}");
    }

    #[test]
    fn commondir_and_alternates_are_refused() {
        for redirect in REDIRECTS {
            let (_guard, dir) = project();
            let target = dir.join(".git").join(redirect);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, "/elsewhere").unwrap();
            let err = check_repo(&dir, std::slice::from_ref(&dir)).unwrap_err();
            assert!(
                err.contains(redirect) && err.contains("points the repository elsewhere"),
                "{err}"
            );
        }
    }

    #[test]
    fn a_git_folder_outside_the_roots_is_refused() {
        let (_guard, dir) = project();
        let (_other, elsewhere) = project();
        fs::create_dir_all(dir.join(".git")).unwrap();
        let err = check_repo(&dir, std::slice::from_ref(&elsewhere)).unwrap_err();
        assert!(err.contains("outside the project"), "{err}");
    }
}
