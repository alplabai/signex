//! Integration tests for `signex_library::project_file_history`.
//!
//! Mirrors the in-adapter `LocalGitAdapter::history` tests but
//! exercises the public helper used by `signex-app`'s right-dock
//! History panel. The helper walks any git repo (not just a
//! library-rooted one), so the fixtures here build a plain
//! `git2::Repository` and stage handful of commits manually.

#![cfg(feature = "local-git")]

use std::fs;
use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryError;
use signex_library::project_file_history;

/// Mint a signature for fixture commits without leaning on the
/// caller's `git` config (CI machines often have neither set).
fn fixture_signature() -> git2::Signature<'static> {
    git2::Signature::now("signex-test", "test@signex.local").unwrap()
}

/// Stage `rel_path` (under `repo`'s working tree) and create a
/// commit with `message`. Returns the new commit's OID for use in
/// follow-up assertions.
fn commit_file(
    repo: &git2::Repository,
    rel_path: &Path,
    contents: &str,
    message: &str,
) -> git2::Oid {
    let workdir = repo.workdir().unwrap();
    let abs = workdir.join(rel_path);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&abs, contents).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(rel_path).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = fixture_signature();
    let parent = match repo.head() {
        Ok(h) => h.peel_to_commit().ok(),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => panic!("repo.head(): {e}"),
    };
    let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .unwrap()
}

fn init_repo(dir: &Path) -> git2::Repository {
    fs::create_dir_all(dir).unwrap();
    git2::Repository::init(dir).unwrap()
}

#[test]
fn returns_not_found_when_no_dot_git() {
    let dir = tempfile::tempdir().unwrap();
    let plain_dir: PathBuf = dir.path().to_path_buf();
    fs::write(plain_dir.join("hello.txt"), "hi").unwrap();

    let err = project_file_history(&plain_dir, Path::new("hello.txt")).unwrap_err();
    assert!(
        matches!(err, LibraryError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

#[test]
fn returns_empty_for_path_with_no_commits() {
    // Fresh repo with one commit on a *different* file. Our query
    // should come back empty for the untracked path.
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    commit_file(&repo, Path::new("a.txt"), "alpha", "add a");

    let entries = project_file_history(dir.path(), Path::new("never-touched.txt")).unwrap();
    assert!(entries.is_empty(), "expected no history, got {entries:?}");
}

#[test]
fn returns_empty_on_unborn_head() {
    // `git init` but no commits yet — `walk.push_head` reports
    // unborn-branch and the helper must fold that to an empty Vec
    // (not an error) so the panel can render an "(no history)" card.
    let dir = tempfile::tempdir().unwrap();
    let _repo = init_repo(dir.path());
    let entries = project_file_history(dir.path(), Path::new("anything.txt")).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn returns_n_commits_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());

    let _ = commit_file(&repo, Path::new("file.txt"), "v1", "first change");
    let _ = commit_file(&repo, Path::new("file.txt"), "v2", "second change");
    // Touch a different file in between so the walker has to filter.
    let _ = commit_file(&repo, Path::new("other.txt"), "x", "unrelated");
    let _ = commit_file(&repo, Path::new("file.txt"), "v3", "third change");

    let entries = project_file_history(dir.path(), Path::new("file.txt")).unwrap();
    assert_eq!(entries.len(), 3, "got {entries:#?}");
    // Newest commit comes first (matches `git log` default order).
    assert_eq!(entries[0].subject, "third change");
    assert_eq!(entries[1].subject, "second change");
    assert_eq!(entries[2].subject, "first change");

    // Author identity from the fixture signature flows through.
    assert_eq!(entries[0].author_name, "signex-test");
    assert_eq!(entries[0].author_email, "test@signex.local");

    // The unrelated commit on `other.txt` is filtered out — confirm
    // nothing else snuck through.
    assert!(
        entries.iter().all(|e| e.subject != "unrelated"),
        "filter should drop commits that don't touch the target path"
    );
}

#[test]
fn accepts_absolute_path_under_project_dir() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    let _ = commit_file(&repo, Path::new("nested/sub.txt"), "x", "add nested");

    let abs = dir.path().join("nested/sub.txt");
    let entries = project_file_history(dir.path(), &abs).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].subject, "add nested");
}
