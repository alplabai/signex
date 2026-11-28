//! What a library folder's version control is in (#2591): off, a clean git
//! repository, or a repository that needs attention, with the one reason the
//! Start screen's Libraries page shows inline. Read off the UI thread.

use std::ffi::OsStr;
use std::path::Path;

use crate::import_snapshot::{find_executable_in, uncommitted_paths, uses_lfs};

/// The version-control state of one library folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryVcsState {
    /// The folder has no `.git`: version control can be turned on.
    Off,
    /// A git repository with nothing to report.
    Git,
    /// A git repository the user has to look at, and why.
    NeedsAttention(VcsAttention),
}

/// Why a library's repository needs attention. One reason is reported, the
/// first of: no `git`, no `git-lfs` for a library that uses LFS, unreadable,
/// uncommitted changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsAttention {
    /// No `git` executable on `PATH`: the repository cannot be pushed or
    /// pulled outside Signex.
    GitNotFound,
    /// The library stores files with Git LFS and no `git-lfs` is on `PATH`,
    /// so a push would stop at the LFS hook.
    LfsNotFound,
    /// The repository could not be read; the reason as git gave it.
    Unreadable(String),
    /// This many working-tree paths differ from the last commit.
    Uncommitted(usize),
}

/// The command-line tools found on `PATH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolsOnPath {
    pub git: bool,
    pub git_lfs: bool,
}

impl ToolsOnPath {
    /// Look the tools up on this process's `PATH`. Blocking.
    pub fn find() -> Self {
        Self::find_in(std::env::var_os("PATH").as_deref())
    }

    /// Look the tools up on the `PATH` value `path_var`. Blocking.
    pub fn find_in(path_var: Option<&OsStr>) -> Self {
        let found =
            |stem: &str| find_executable_in(path_var, None, &executable_name(stem)).is_some();
        Self {
            git: found("git"),
            git_lfs: found("git-lfs"),
        }
    }
}

/// The file name of the executable `stem` on this platform.
fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// Read the state of the library folder `dir` against `tools`. The host
/// says whether `dir` has a `.git` entry (`has_git_dir`); only a folder
/// without one is off. Blocking: opens the repository and runs a
/// `git status`.
pub fn read_library_vcs_state(
    dir: &Path,
    has_git_dir: bool,
    tools: ToolsOnPath,
) -> LibraryVcsState {
    if !has_git_dir {
        return LibraryVcsState::Off;
    }
    // A `.git` git cannot open (HEAD, objects or refs missing reads as
    // NotFound too) is a repository that cannot be read, never off.
    let opened = git2::Repository::open(dir);
    let attention = LibraryVcsState::NeedsAttention;
    if !tools.git {
        return attention(VcsAttention::GitNotFound);
    }
    if uses_lfs(dir) && !tools.git_lfs {
        return attention(VcsAttention::LfsNotFound);
    }
    let changed = opened
        .map_err(|e| e.message().to_owned())
        .and_then(|repo| uncommitted_paths(&repo).map_err(|e| e.to_string()));
    match changed {
        Err(reason) => attention(VcsAttention::Unreadable(reason)),
        Ok(paths) if !paths.is_empty() => attention(VcsAttention::Uncommitted(paths.len())),
        Ok(_) => LibraryVcsState::Git,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_TOOLS: ToolsOnPath = ToolsOnPath {
        git: true,
        git_lfs: true,
    };

    /// The state of `dir`, with its `.git` presence read as the host reads
    /// it.
    fn read(dir: &Path, tools: ToolsOnPath) -> LibraryVcsState {
        let has_git_dir = std::fs::symlink_metadata(dir.join(".git")).is_ok();
        read_library_vcs_state(dir, has_git_dir, tools)
    }

    fn committed_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        std::fs::write(dir.path().join("lib.snxlib"), "x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("lib.snxlib")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let who = git2::Signature::now("Test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &who, &who, "first", &tree, &[])
            .unwrap();
        dir
    }

    #[test]
    fn a_folder_without_git_is_off() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path(), ALL_TOOLS), LibraryVcsState::Off);
    }

    /// Review #3235: a `.git` that git cannot open (here, no HEAD) needs
    /// attention; only a missing `.git` is off.
    #[test]
    fn a_damaged_repository_needs_attention_and_is_not_off() {
        let dir = committed_repo();
        std::fs::remove_file(dir.path().join(".git").join("HEAD")).unwrap();
        assert!(matches!(
            read(dir.path(), ALL_TOOLS),
            LibraryVcsState::NeedsAttention(VcsAttention::Unreadable(_))
        ));
    }

    #[test]
    fn a_clean_repository_is_git_and_an_edit_needs_attention() {
        let dir = committed_repo();
        assert_eq!(read(dir.path(), ALL_TOOLS), LibraryVcsState::Git);
        std::fs::write(dir.path().join("lib.snxlib"), "edited").unwrap();
        std::fs::write(dir.path().join("new.tsv"), "row").unwrap();
        assert_eq!(
            read(dir.path(), ALL_TOOLS),
            LibraryVcsState::NeedsAttention(VcsAttention::Uncommitted(2))
        );
    }

    #[test]
    fn missing_tools_need_attention_git_first_then_lfs() {
        let dir = committed_repo();
        let no_git = ToolsOnPath {
            git: false,
            git_lfs: false,
        };
        assert_eq!(
            read(dir.path(), no_git),
            LibraryVcsState::NeedsAttention(VcsAttention::GitNotFound)
        );
        let no_lfs = ToolsOnPath {
            git: true,
            git_lfs: false,
        };
        // LFS matters only for a library that uses it.
        assert_eq!(read(dir.path(), no_lfs), LibraryVcsState::Git);
        std::fs::write(dir.path().join(".gitattributes"), "*.step filter=lfs\n").unwrap();
        assert_eq!(
            read(dir.path(), no_lfs),
            LibraryVcsState::NeedsAttention(VcsAttention::LfsNotFound)
        );
        assert_eq!(
            read(dir.path(), ALL_TOOLS),
            LibraryVcsState::NeedsAttention(VcsAttention::Uncommitted(1)),
            "with git-lfs present the new attributes file is the reason"
        );
    }

    #[test]
    fn tools_are_found_on_an_absolute_path_entry_only() {
        let bin = tempfile::tempdir().unwrap();
        let git = bin.path().join(executable_name("git"));
        std::fs::write(&git, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::join_paths([bin.path()]).unwrap();
        assert_eq!(
            ToolsOnPath::find_in(Some(&path)),
            ToolsOnPath {
                git: true,
                git_lfs: false
            }
        );
        assert_eq!(
            ToolsOnPath::find_in(None),
            ToolsOnPath {
                git: false,
                git_lfs: false
            }
        );
    }
}
