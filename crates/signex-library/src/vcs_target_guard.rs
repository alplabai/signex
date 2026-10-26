//! Which directories Enable Version Control must never `git init` (#1205).
//!
//! `git init` plus the initial commit stages every file under the directory,
//! so initialising a user's home, Desktop or a folder holding several projects
//! would put unrelated personal files into a repository. The check is pure
//! (the home directory is injected) and runs on the blocking worker, because
//! the nested-project scan reads the disk.

use std::path::{Path, PathBuf};

/// Folders under the home directory that are never a project of their own.
const WELL_KNOWN_USER_FOLDERS: [&str; 3] = ["Desktop", "Documents", "Downloads"];

/// How far below the target a foreign `.snxprj` is looked for.
const NESTED_PROJECT_SCAN_DEPTH: usize = 2;

/// Upper bound on entries read per directory, so a huge folder cannot stall
/// the worker.
const MAX_ENTRIES_PER_DIR: usize = 5_000;

/// The user's home directory from the environment, if one is set.
pub fn home_dir_from_env() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|p| !p.as_os_str().is_empty())
}

/// The refusal message when `dir` must not be turned into a repository, or
/// `None` when it is fine. `home` is the user's home directory.
pub fn version_control_refusal(dir: &Path, home: Option<&Path>) -> Option<String> {
    let target = canonical_or_given(dir);
    if target.parent().is_none() {
        return Some(format!(
            "{} is a filesystem root; version control is not enabled there. \
             Move the project into its own folder first.",
            dir.display()
        ));
    }
    if let Some(home) = home.map(canonical_or_given) {
        if target == home {
            return Some(format!(
                "{} is your home folder; version control would stage your personal files. \
                 Move the project into its own folder first.",
                dir.display()
            ));
        }
        if WELL_KNOWN_USER_FOLDERS
            .iter()
            .any(|name| target == home.join(name))
        {
            return Some(format!(
                "{} is a shared user folder; version control would stage unrelated files. \
                 Move the project into its own folder first.",
                dir.display()
            ));
        }
    }
    if let Some(found) = nested_project(&target, NESTED_PROJECT_SCAN_DEPTH) {
        return Some(format!(
            "{} contains another project ({}); version control would mix them. \
             Enable it from a folder that holds only this project.",
            dir.display(),
            found.display()
        ));
    }
    None
}

fn canonical_or_given(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A `.snxprj` found in a subdirectory of `dir`, at most `depth` levels down.
fn nested_project(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten().take(MAX_ENTRIES_PER_DIR) {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|t| t.is_dir()) || entry.file_name() == ".git" {
            continue;
        }
        if let Some(found) = project_file_in(&path).or_else(|| nested_project(&path, depth - 1)) {
            return Some(found);
        }
    }
    None
}

fn project_file_in(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .take(MAX_ENTRIES_PER_DIR)
        .map(|e| e.path())
        .find(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("snxprj"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn refuses_a_filesystem_root() {
        let root = Path::new(if cfg!(windows) { "C:\\" } else { "/" });
        assert!(version_control_refusal(root, None).is_some());
    }

    #[test]
    fn refuses_the_home_folder_and_its_well_known_folders() {
        let home = tempfile::tempdir().unwrap();
        for name in WELL_KNOWN_USER_FOLDERS {
            std::fs::create_dir_all(home.path().join(name)).unwrap();
        }
        assert!(version_control_refusal(home.path(), Some(home.path())).is_some());
        for name in WELL_KNOWN_USER_FOLDERS {
            let dir = home.path().join(name);
            assert!(version_control_refusal(&dir, Some(home.path())).is_some());
        }
    }

    #[test]
    fn allows_a_project_folder_under_the_home_folder() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("Desktop").join("Board");
        touch(&project.join("Board.snxprj"));
        assert_eq!(version_control_refusal(&project, Some(home.path())), None);
    }

    #[test]
    fn refuses_a_folder_that_holds_other_projects() {
        let parent = tempfile::tempdir().unwrap();
        touch(&parent.path().join("A").join("A.snxprj"));
        touch(&parent.path().join("B").join("deep").join("B.snxprj"));
        assert!(version_control_refusal(parent.path(), None).is_some());
    }

    #[test]
    fn a_project_with_its_own_marker_and_plain_subfolders_is_fine() {
        let project = tempfile::tempdir().unwrap();
        touch(&project.path().join("P.snxprj"));
        touch(&project.path().join("assets").join("a.png"));
        assert_eq!(version_control_refusal(project.path(), None), None);
    }
}
