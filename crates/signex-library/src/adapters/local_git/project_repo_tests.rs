//! Mockup rows 35 and 39: reading a project repository, switching its branch
//! and writing the commit author to its git config.

use std::fs;
use std::path::Path;

use super::*;

const PROJECT: &str = "p.snxprj";

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

/// A repository on `main` holding the project file and one sheet, with a
/// branch `other` one commit ahead that changed only the sheet.
fn repo_with_branch() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    fs::write(dir.path().join(PROJECT), "project").unwrap();
    fs::write(dir.path().join("a.snxsch"), "main\n").unwrap();
    let first = commit_all(&repo, "first");
    {
        let tip = repo.find_commit(first).unwrap();
        repo.branch("other", &tip, false).unwrap();
    }
    repo.set_head("refs/heads/other").unwrap();
    fs::write(dir.path().join("a.snxsch"), "other\n").unwrap();
    commit_all(&repo, "on other");
    repo.set_head("refs/heads/main").unwrap();
    let mut force = git2::build::CheckoutBuilder::new();
    force.force();
    repo.checkout_head(Some(&mut force)).unwrap();
    (dir, repo)
}

fn switch(root: &Path, name: &str) -> Result<(), SwitchRefusal> {
    let target = crate::repo_branches::BranchTarget::Local(name.into());
    crate::repo_branches::switch_branch(root, &target, &[]).map(|_| ())
}

#[test]
fn the_facts_name_the_branch_its_neighbours_and_the_last_commit() {
    let (dir, _repo) = repo_with_branch();
    let facts = read_repo_facts(dir.path()).unwrap();
    assert_eq!(facts.branch.as_deref(), Some("main"));
    let names: Vec<&str> = facts.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["main", "other"]);
    let other = &facts.branches[1];
    assert_eq!((other.is_current, other.ahead, other.behind), (false, 1, 0));
    let last = facts.last_commit.unwrap();
    assert_eq!(last.subject, "first");
    assert_eq!(last.short_sha.len(), 7);
}

#[test]
fn reading_a_folder_without_a_repository_does_not_create_one() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_repo_facts(dir.path()).is_err());
    assert!(!dir.path().join(".git").exists());
}

#[test]
fn a_switch_checks_the_branch_out() {
    let (dir, _repo) = repo_with_branch();
    switch(dir.path(), "other").unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "other\n"
    );
    let facts = read_repo_facts(dir.path()).unwrap();
    assert_eq!(facts.branch.as_deref(), Some("other"));
}

/// Data-loss guard: uncommitted work in a file the switch replaces stays.
#[test]
fn a_switch_never_overwrites_a_changed_file() {
    let (dir, _repo) = repo_with_branch();
    fs::write(dir.path().join("a.snxsch"), "uncommitted\n").unwrap();
    assert_eq!(
        switch(dir.path(), "other"),
        Err(SwitchRefusal::WouldOverwrite)
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "uncommitted\n"
    );
    let facts = read_repo_facts(dir.path()).unwrap();
    assert_eq!(facts.branch.as_deref(), Some("main"));
}

/// Decision 2026-11-22: the app closes and reopens the project, so the
/// repository layer no longer refuses a differing project file.
#[test]
fn a_branch_with_another_project_file_is_checked_out() {
    let (dir, repo) = repo_with_branch();
    repo.set_head("refs/heads/other").unwrap();
    let mut force = git2::build::CheckoutBuilder::new();
    force.force();
    repo.checkout_head(Some(&mut force)).unwrap();
    fs::write(dir.path().join(PROJECT), "changed").unwrap();
    commit_all(&repo, "project edit");
    repo.set_head("refs/heads/main").unwrap();
    repo.checkout_head(Some(&mut force)).unwrap();
    assert_eq!(switch(dir.path(), "other"), Ok(()));
    assert_eq!(
        fs::read_to_string(dir.path().join(PROJECT)).unwrap(),
        "changed"
    );
}

/// A checkout would leave the pointer where the model's bytes were, so a
/// pointer whose content is not stored refuses the switch.
#[test]
fn a_branch_differing_in_an_lfs_file_without_its_content_is_refused() {
    let (dir, repo) = repo_with_branch();
    repo.set_head("refs/heads/other").unwrap();
    let mut force = git2::build::CheckoutBuilder::new();
    force.force();
    repo.checkout_head(Some(&mut force)).unwrap();
    let (pointer, _) = super::super::lfs::pointer_for(b"model bytes");
    fs::write(dir.path().join("m.step"), pointer).unwrap();
    commit_all(&repo, "add model");
    repo.set_head("refs/heads/main").unwrap();
    repo.checkout_head(Some(&mut force)).unwrap();
    // `main` lacks the model; a forced checkout leaves it as an untracked file.
    let _ = fs::remove_file(dir.path().join("m.step"));
    assert!(matches!(
        switch(dir.path(), "other"),
        Err(SwitchRefusal::LfsContentMissing(path)) if path == "m.step"
    ));
    assert_eq!(repo.head().unwrap().name().unwrap(), "refs/heads/main");
    assert_eq!(
        fs::read_to_string(dir.path().join("a.snxsch")).unwrap(),
        "main\n"
    );
}

#[test]
fn an_unknown_branch_is_refused() {
    let (dir, _repo) = repo_with_branch();
    assert_eq!(
        switch(dir.path(), "nope"),
        Err(SwitchRefusal::UnknownBranch)
    );
}

#[test]
fn the_author_is_written_to_the_repository_config_and_removed_again() {
    let (dir, repo) = repo_with_branch();
    write_author(dir.path(), Some(("J. Berg", "j.berg@example.com"))).unwrap();
    let local = repo
        .config()
        .unwrap()
        .open_level(git2::ConfigLevel::Local)
        .unwrap();
    assert_eq!(local.get_string("user.name").unwrap(), "J. Berg");
    assert_eq!(
        local.get_string("user.email").unwrap(),
        "j.berg@example.com"
    );
    write_author(dir.path(), None).unwrap();
    let local = repo
        .config()
        .unwrap()
        .open_level(git2::ConfigLevel::Local)
        .unwrap();
    assert!(local.get_string("user.name").is_err());
    // Removing what is already gone is not an error.
    write_author(dir.path(), None).unwrap();
}

#[test]
fn an_author_reads_only_as_name_and_email() {
    assert_eq!(
        parse_author(" J. Berg <j.berg@example.com> "),
        Some(("J. Berg".into(), "j.berg@example.com".into()))
    );
    for bad in ["J. Berg", "<a@b>", "J <>", "J <a b@c>", "J a@b>"] {
        assert_eq!(parse_author(bad), None, "{bad}");
    }
}
