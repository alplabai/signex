//! #859 — a library's first commit never builds on another repo's history or
//! index. Real git2 repos in temp dirs.

#![cfg(feature = "local-git")]

use std::path::{Path, PathBuf};

use git2::Repository;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

const GIT: LibraryInitOptions = LibraryInitOptions {
    enable_git: true,
    use_lfs: false,
};
const LFS: LibraryInitOptions = LibraryInitOptions {
    enable_git: true,
    use_lfs: true,
};
const PLAIN: LibraryInitOptions = LibraryInitOptions {
    enable_git: false,
    use_lfs: false,
};

fn manifest() -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

fn plain_library(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    let lib = dir.join("lib.snxlib");
    LocalGitAdapter::init(&lib, manifest(), PLAIN).expect("plain library");
    lib
}

/// A repo at `dir` with one commit (`base.txt`) and `staged.txt` staged.
fn repo_with_staged_work(dir: &Path) -> (Repository, git2::Oid) {
    signex_library::git_identity::test_identity::install();
    let repo = Repository::init(dir).unwrap();
    std::fs::write(dir.join("base.txt"), "base").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("base.txt")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@example.org").unwrap();
    let head = repo
        .commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
        .unwrap();
    std::fs::write(dir.join("staged.txt"), "unrelated staged work").unwrap();
    index.add_path(Path::new("staged.txt")).unwrap();
    index.write().unwrap();
    drop(tree);
    (repo, head)
}

fn head(dir: &Path) -> git2::Oid {
    Repository::open(dir)
        .unwrap()
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id()
}

fn head_tracks(dir: &Path, name: &str) -> bool {
    let repo = Repository::open(dir).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    tree.get_name(name).is_some()
}

#[test]
fn init_never_commits_another_repos_staged_work() {
    let tmp = tempfile::tempdir().unwrap();
    let (_repo, before) = repo_with_staged_work(tmp.path());

    let result = LocalGitAdapter::init(tmp.path().join("lib.snxlib"), manifest(), GIT);

    assert!(
        result.is_err(),
        "trunk refuses: the repo already has history"
    );
    assert_eq!(head(tmp.path()), before, "history untouched");
    assert!(!head_tracks(tmp.path(), "staged.txt"));
}

#[test]
fn complete_init_refuses_a_repo_that_does_not_track_the_library() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    let (_repo, before) = repo_with_staged_work(tmp.path());

    let result = LocalGitAdapter::complete_init(&lib, LFS);

    assert!(result.is_err());
    assert_eq!(head(tmp.path()), before);
    assert!(!head_tracks(tmp.path(), "staged.txt"));
    assert!(
        !tmp.path().join(".gitattributes").exists(),
        "nothing was written into the other repo"
    );
}

#[test]
fn complete_init_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());

    LocalGitAdapter::complete_init(&lib, LFS).expect("first");
    let first = head(tmp.path());
    LocalGitAdapter::complete_init(&lib, LFS).expect("second");

    assert_eq!(head(tmp.path()), first, "nothing new committed");
    assert!(head_tracks(tmp.path(), "lib.snxlib"));
    assert!(head_tracks(tmp.path(), ".gitattributes"));
}

#[test]
fn the_lfs_rules_are_added_to_an_existing_gitattributes() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".gitattributes"), "*.png -text").unwrap();
    let lib = plain_library(tmp.path());

    LocalGitAdapter::complete_init(&lib, LFS).expect("completed");

    let attrs = std::fs::read_to_string(tmp.path().join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.png -text"), "{attrs}");
    assert!(attrs.contains("*.step filter=lfs"), "{attrs}");
}

#[test]
fn lfs_on_top_of_the_librarys_own_commit_leaves_other_staged_work_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    LocalGitAdapter::complete_init(&lib, GIT).expect("own first commit");
    let own_first = head(tmp.path());
    std::fs::write(tmp.path().join("staged.txt"), "unrelated").unwrap();
    let repo = Repository::open(tmp.path()).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("staged.txt")).unwrap();
    index.write().unwrap();

    LocalGitAdapter::complete_init(&lib, LFS).expect("lfs top-up");

    let new_head = head(tmp.path());
    assert_ne!(new_head, own_first);
    let commit = repo.find_commit(new_head).unwrap();
    assert_eq!(commit.parent_id(0).unwrap(), own_first);
    assert!(head_tracks(tmp.path(), ".gitattributes"));
    assert!(!head_tracks(tmp.path(), "staged.txt"));
}

#[test]
fn complete_init_without_version_control_does_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());

    LocalGitAdapter::complete_init(&lib, PLAIN).expect("no-op");

    assert!(!tmp.path().join(".git").exists());
    assert!(!tmp.path().join(".gitattributes").exists());
}
