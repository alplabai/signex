//! #2337: index updates after a commit, under git's `index.lock`.

use std::fs;
use std::path::Path;

use super::helpers::{stage_and_commit, stage_and_commit_with};
use super::index_lock::set_index_update_hook;

fn signature() -> git2::Signature<'static> {
    git2::Signature::now("Test", "test@example.test").unwrap()
}

/// Commit the files `contents` names on top of HEAD (or as the first
/// commit) on `refname`, through the index.
fn commit_files(
    repo: &git2::Repository,
    refname: &str,
    parents: &[&git2::Commit<'_>],
    contents: &[(&str, &str)],
) -> git2::Oid {
    let root = repo.workdir().unwrap();
    let mut index = repo.index().unwrap();
    for (name, text) in contents {
        fs::write(root.join(name), text).unwrap();
        index.add_path(Path::new(name)).unwrap();
    }
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = signature();
    repo.commit(Some(refname), &sig, &sig, "commit", &tree, parents)
        .unwrap()
}

/// Another process holds `index.lock` while our save updates the index,
/// then publishes an index with a path it staged and unlocks. Our
/// update must not write back the index it saw before that publish.
#[test]
fn an_index_published_by_the_lock_holder_keeps_its_staged_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = signature();
    fs::write(dir.path().join("file.txt"), "one\n").unwrap();
    stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "first")
        .unwrap()
        .expect("first commit");

    // The index the lock holder will publish: ours plus `other.txt`.
    let git = repo.path().to_path_buf();
    fs::write(dir.path().join("other.txt"), "other\n").unwrap();
    let original = fs::read(git.join("index")).unwrap();
    {
        let holder = git2::Repository::open(dir.path()).unwrap();
        let mut index = holder.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        index.write().unwrap();
    }
    let published = fs::read(git.join("index")).unwrap();
    fs::write(git.join("index"), &original).unwrap();
    fs::write(git.join("index.lock"), b"").unwrap();

    let holder_git = git.clone();
    set_index_update_hook(move || {
        fs::write(holder_git.join("index.new"), &published).unwrap();
        fs::rename(holder_git.join("index.new"), holder_git.join("index")).unwrap();
        fs::remove_file(holder_git.join("index.lock")).unwrap();
    });
    fs::write(dir.path().join("file.txt"), "two\n").unwrap();
    stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "second")
        .unwrap()
        .expect("second commit");

    let after = git2::Repository::open(dir.path()).unwrap();
    let index = after.index().unwrap();
    assert!(
        index.get_path(Path::new("other.txt"), 0).is_some(),
        "the lock holder's staged path was dropped"
    );
    let head = after.head().unwrap().peel_to_tree().unwrap();
    let committed = head.get_path(Path::new("file.txt")).unwrap().id();
    assert_eq!(
        index.get_path(Path::new("file.txt"), 0).map(|e| e.id),
        Some(committed),
        "the saved path's index entry is not the committed blob"
    );
    assert!(!git.join("index.lock").exists(), "index.lock left behind");
    assert!(
        !git.join("index.signex-scratch").exists(),
        "scratch index left behind"
    );
}

/// A save of files left conflicted by a merge resolves them in the index,
/// for the saved path and for an extra path committed with it.
#[test]
fn a_save_of_conflicted_paths_leaves_no_unmerged_stages() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let both = |text| [("a.txt", text), ("b.txt", text)];
    let base_id = commit_files(&repo, "HEAD", &[], &both("base\n"));
    let base = repo.find_commit(base_id).unwrap();
    commit_files(&repo, "HEAD", &[&base], &both("ours\n"));
    // Theirs, built from base without touching the working tree.
    let mut builder = repo.treebuilder(Some(&base.tree().unwrap())).unwrap();
    let theirs_blob = repo.blob(b"theirs\n").unwrap();
    builder.insert("a.txt", theirs_blob, 0o100644).unwrap();
    builder.insert("b.txt", theirs_blob, 0o100644).unwrap();
    let theirs_tree = repo.find_tree(builder.write().unwrap()).unwrap();
    let sig = signature();
    let theirs = repo
        .commit(
            Some("refs/heads/theirs"),
            &sig,
            &sig,
            "theirs",
            &theirs_tree,
            &[&base],
        )
        .unwrap();
    let annotated = repo.find_annotated_commit(theirs).unwrap();
    repo.merge(&[&annotated], None, None).unwrap();
    assert!(repo.index().unwrap().has_conflicts(), "the merge conflicts");

    fs::write(dir.path().join("a.txt"), "resolved a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "resolved b\n").unwrap();
    stage_and_commit_with(
        &repo,
        &[],
        Some(Path::new("a.txt")),
        None,
        &[Path::new("b.txt")],
        &sig,
        "resolve",
    )
    .unwrap()
    .expect("a commit");

    let after = git2::Repository::open(dir.path()).unwrap();
    let index = after.index().unwrap();
    let unmerged: Vec<String> = index
        .iter()
        .filter(|e| (e.flags >> 12) & 0x3 != 0)
        .map(|e| String::from_utf8_lossy(&e.path).into_owned())
        .collect();
    assert!(unmerged.is_empty(), "unmerged stages left: {unmerged:?}");
    for name in ["a.txt", "b.txt"] {
        let status = after.status_file(Path::new(name)).unwrap();
        assert!(
            !status.contains(git2::Status::CONFLICTED),
            "{name} still reads as conflicted: {status:?}"
        );
    }
}
