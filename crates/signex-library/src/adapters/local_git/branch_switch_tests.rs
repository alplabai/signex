//! The switch restores LFS content and leaves nothing behind when it fails.

use std::fs;

use super::*;
use crate::adapters::local_git::lfs::pointer_for;

const MODEL_MAIN: &[u8] = b"ISO-10303-21; main";
const MODEL_SIDE: &[u8] = b"ISO-10303-21; side";

fn store_object(repo: &git2::Repository, content: &[u8]) -> Vec<u8> {
    let (pointer, oid) = pointer_for(content);
    let object = lfs::object_path(repo, &oid);
    fs::create_dir_all(object.parent().unwrap()).unwrap();
    fs::write(object, content).unwrap();
    pointer
}

fn commit_all(repo: &git2::Repository, message: &str) -> git2::Oid {
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("T", "t@example.com").unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .unwrap()
}

fn check_out(repo: &git2::Repository, branch: &str) {
    repo.set_head(&format!("refs/heads/{branch}")).unwrap();
    let mut force = git2::build::CheckoutBuilder::new();
    force.force().remove_untracked(true);
    repo.checkout_head(Some(&mut force)).unwrap();
}

/// `main` and `side` hold different LFS pointers for `m.step` and different
/// text in `a.snxsch`. `main` is checked out, with the model's real bytes in
/// the working file, as a commit through LFS leaves it.
fn lfs_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    let main_pointer = store_object(&repo, MODEL_MAIN);
    fs::write(dir.path().join("m.step"), &main_pointer).unwrap();
    fs::write(dir.path().join("a.snxsch"), "main\n").unwrap();
    let first = commit_all(&repo, "first");
    repo.branch("side", &repo.find_commit(first).unwrap(), false)
        .unwrap();
    check_out(&repo, "side");
    let side_pointer = store_object(&repo, MODEL_SIDE);
    fs::write(dir.path().join("m.step"), &side_pointer).unwrap();
    fs::write(dir.path().join("a.snxsch"), "side\n").unwrap();
    commit_all(&repo, "on side");
    check_out(&repo, "main");
    fs::write(dir.path().join("m.step"), MODEL_MAIN).unwrap();
    (dir, repo)
}

fn side() -> BranchTarget {
    BranchTarget::Local("side".into())
}

fn head(repo: &git2::Repository) -> String {
    repo.head().unwrap().shorthand().unwrap().to_owned()
}

#[test]
fn a_switch_restores_lfs_content_from_the_object_store() {
    let (dir, repo) = lfs_repo();
    assert_eq!(switch(dir.path(), &side(), &[]).unwrap(), "side");
    assert_eq!(head(&repo), "side");
    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_SIDE);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "side\n"
    );
}

/// The index keeps the pointer, so the next commit and switch see no change.
#[test]
fn the_index_records_the_pointer_and_a_second_switch_comes_back() {
    let (dir, repo) = lfs_repo();
    switch(dir.path(), &side(), &[]).unwrap();
    let index = repo.index().unwrap();
    let entry = index.get_path(Path::new("m.step"), 0).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    assert_eq!(entry.id, tree.get_path(Path::new("m.step")).unwrap().id());

    switch(dir.path(), &BranchTarget::Local("main".into()), &[]).unwrap();
    assert_eq!(head(&repo), "main");
    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_MAIN);
}

#[test]
fn a_missing_object_refuses_before_any_file_is_written() {
    let (dir, repo) = lfs_repo();
    let (_, oid) = pointer_for(MODEL_SIDE);
    fs::remove_file(lfs::object_path(&repo, &oid)).unwrap();

    let result = switch(dir.path(), &side(), &[]);

    assert_eq!(
        result,
        Err(SwitchRefusal::LfsContentMissing("m.step".into()))
    );
    assert_eq!(head(&repo), "main");
    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_MAIN);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "main\n"
    );
}

/// An object whose bytes do not hash to its name is not content to restore.
#[test]
fn a_corrupt_object_refuses_the_switch() {
    let (dir, repo) = lfs_repo();
    let (_, oid) = pointer_for(MODEL_SIDE);
    fs::write(lfs::object_path(&repo, &oid), b"truncated").unwrap();
    assert_eq!(
        switch(dir.path(), &side(), &[]),
        Err(SwitchRefusal::LfsContentMissing("m.step".into()))
    );
    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_MAIN);
}

/// Data-loss guard: an edited LFS file is not replaced by the branch's.
#[test]
fn an_edited_lfs_file_refuses_the_switch_and_stays() {
    let (dir, repo) = lfs_repo();
    fs::write(dir.path().join("m.step"), b"edited model").unwrap();

    let result = switch(dir.path(), &side(), &[]);

    assert_eq!(result, Err(SwitchRefusal::WouldOverwrite));
    assert_eq!(head(&repo), "main");
    assert_eq!(
        fs::read(dir.path().join("m.step")).unwrap(),
        b"edited model"
    );
}

/// A plain file's conflict comes after the LFS files moved aside: they return.
#[test]
fn a_conflict_on_a_plain_file_puts_the_lfs_file_back() {
    let (dir, repo) = lfs_repo();
    fs::write(dir.path().join("a.snxsch"), "uncommitted\n").unwrap();

    let result = switch(dir.path(), &side(), &[]);

    assert_eq!(result, Err(SwitchRefusal::WouldOverwrite));
    assert_eq!(head(&repo), "main");
    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_MAIN);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "uncommitted\n"
    );
}

/// Data-loss guard: with its object pruned, the working file is the only
/// copy of the model, so the switch stores it before replacing it.
#[test]
fn a_switch_keeps_content_whose_object_was_pruned() {
    let (dir, repo) = lfs_repo();
    let (_, oid) = pointer_for(MODEL_MAIN);
    fs::remove_file(lfs::object_path(&repo, &oid)).unwrap();

    switch(dir.path(), &side(), &[]).unwrap();
    switch(dir.path(), &BranchTarget::Local("main".into()), &[]).unwrap();

    assert_eq!(fs::read(dir.path().join("m.step")).unwrap(), MODEL_MAIN);
}

#[test]
fn a_pointer_names_only_a_sha256_hash() {
    let (pointer, oid) = pointer_for(b"x");
    assert_eq!(lfs::parse_pointer(&pointer).unwrap().oid, oid);
    let hostile = b"version https://git-lfs.github.com/spec/v1\noid sha256:../../evil\nsize 1\n";
    assert_eq!(lfs::parse_pointer(hostile), None);
}
