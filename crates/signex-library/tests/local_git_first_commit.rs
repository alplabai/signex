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
    tree.get_path(Path::new(name)).is_ok()
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

/// #1741: Re-init in a project folder commits the library's own files, not
/// the project's.
#[test]
fn recover_init_commits_only_the_librarys_files() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    std::fs::create_dir_all(tmp.path().join("symbols")).unwrap();
    std::fs::write(tmp.path().join("symbols/r.snxsym"), "sym").unwrap();
    std::fs::write(tmp.path().join("notes.txt"), "stray").unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), "project").unwrap();
    std::fs::write(tmp.path().join("main.snxsch"), "sheet").unwrap();

    LocalGitAdapter::recover_init(&lib).expect("re-init");

    assert!(head_tracks(tmp.path(), "lib.snxlib"));
    assert!(head_tracks(tmp.path(), "symbols/r.snxsym"));
    for stray in ["notes.txt", "demo.snxprj", "main.snxsch"] {
        assert!(!head_tracks(tmp.path(), stray), "{stray} must stay out");
    }
    assert!(head_tracks(tmp.path(), ".gitignore"));
}

/// #1741: a folder that is another repo with history, not tracking the
/// library, is refused with a reason and nothing is committed.
#[test]
fn recover_init_refuses_a_repo_that_does_not_track_the_library() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    let (_repo, before) = repo_with_staged_work(tmp.path());

    let Err(err) = LocalGitAdapter::recover_init(&lib) else {
        panic!("re-init must refuse");
    };

    assert!(
        err.to_string().contains("does not track this library"),
        "{err}"
    );
    assert_eq!(head(tmp.path()), before, "history untouched");
    assert!(!head_tracks(tmp.path(), "staged.txt"));
    assert!(!head_tracks(tmp.path(), "lib.snxlib"));
}

fn stage(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "staged by the user").unwrap();
    let repo = Repository::open(dir).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(name)).unwrap();
    index.write().unwrap();
}

fn index_has(dir: &Path, name: &str) -> bool {
    let repo = Repository::open(dir).unwrap();
    let index = repo.index().unwrap();
    index.get_path(Path::new(name), 0).is_some()
}

/// #1741: a half-made repo (unborn HEAD) whose user staged a project file:
/// Re-init commits the library, not what was staged, and leaves it staged.
#[test]
fn recover_init_on_an_unborn_repo_leaves_staged_work_out() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    Repository::init(tmp.path()).unwrap();
    stage(tmp.path(), "demo.snxprj");

    LocalGitAdapter::recover_init(&lib).expect("re-init");

    assert!(head_tracks(tmp.path(), "lib.snxlib"));
    assert!(!head_tracks(tmp.path(), "demo.snxprj"));
    assert!(index_has(tmp.path(), "demo.snxprj"), "still staged");
}

/// #1741: the library is tracked by a project repo (default layout), the user
/// has staged a project file, a primitive was deleted and a project `models/`
/// folder sits beside the library.
#[test]
fn recover_init_in_a_tracking_repo_commits_library_changes_only() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = plain_library(tmp.path());
    std::fs::create_dir_all(tmp.path().join("symbols")).unwrap();
    std::fs::write(tmp.path().join("symbols/keep.snxsym"), "k").unwrap();
    std::fs::write(tmp.path().join("symbols/gone.snxsym"), "g").unwrap();
    LocalGitAdapter::recover_init(&lib).expect("first re-init");
    assert!(head_tracks(tmp.path(), "symbols/gone.snxsym"));

    std::fs::remove_file(tmp.path().join("symbols/gone.snxsym")).unwrap();
    std::fs::write(tmp.path().join("symbols/new.snxsym"), "n").unwrap();
    std::fs::create_dir_all(tmp.path().join("models")).unwrap();
    std::fs::write(tmp.path().join("models/part.step"), "project model").unwrap();
    stage(tmp.path(), "demo.snxprj");
    let before = head(tmp.path());

    LocalGitAdapter::recover_init(&lib).expect("re-init");

    assert_ne!(head(tmp.path()), before, "a new commit");
    assert!(!head_tracks(tmp.path(), "demo.snxprj"));
    assert!(!head_tracks(tmp.path(), "models/part.step"));
    assert!(!head_tracks(tmp.path(), "symbols/gone.snxsym"), "deleted");
    assert!(head_tracks(tmp.path(), "symbols/keep.snxsym"));
    assert!(head_tracks(tmp.path(), "symbols/new.snxsym"));
    assert!(index_has(tmp.path(), "demo.snxprj"), "still staged");
    assert!(!index_has(tmp.path(), "symbols/gone.snxsym"));
}
