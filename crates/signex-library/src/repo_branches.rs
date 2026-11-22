//! Branch and tag writes for the shared branch switcher: create a branch or
//! tag, and switch the working tree to a branch. A switch never overwrites
//! local work, and a failed one leaves the tree as it was.

use std::path::{Path, PathBuf};

use crate::adapter::LibraryError;
use crate::adapters::local_git::branch_switch::failed;
use crate::adapters::local_git::project_repo::SwitchRefusal;

/// The branch a switch goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchTarget {
    Local(String),
    /// `origin/feature`: switches to the local `feature`, created to track
    /// it when there is none.
    Remote(String),
}

fn backend(what: &str, e: git2::Error) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
}

/// The commit `at_sha` names, or HEAD's.
fn start_commit<'r>(
    repo: &'r git2::Repository,
    at_sha: Option<&str>,
) -> Result<git2::Commit<'r>, LibraryError> {
    match at_sha {
        Some(sha) => {
            let oid = git2::Oid::from_str(sha).map_err(|e| backend("git sha", e))?;
            repo.find_commit(oid)
                .map_err(|e| backend("git find commit", e))
        }
        None => repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|e| backend("git head", e)),
    }
}

/// Create the local branch `name` at `at_sha` (HEAD when `None`). The working
/// tree and HEAD stay as they are; an existing branch is an error.
pub fn create_branch(
    project_dir: &Path,
    name: &str,
    at_sha: Option<&str>,
) -> Result<(), LibraryError> {
    let repo = git2::Repository::open(project_dir).map_err(|e| backend("git open", e))?;
    let commit = start_commit(&repo, at_sha)?;
    repo.branch(name, &commit, false)
        .map_err(|e| backend("git branch", e))?;
    Ok(())
}

/// Create the lightweight tag `name` on `sha`; an existing tag is an error.
pub fn create_tag(project_dir: &Path, name: &str, sha: &str) -> Result<(), LibraryError> {
    let repo = git2::Repository::open(project_dir).map_err(|e| backend("git open", e))?;
    let commit = start_commit(&repo, Some(sha))?;
    repo.tag_lightweight(name, commit.as_object(), false)
        .map_err(|e| backend("git tag", e))?;
    Ok(())
}

/// The local branch name a target switches to.
pub(crate) fn local_name(target: &BranchTarget) -> &str {
    match target {
        BranchTarget::Local(name) => name,
        BranchTarget::Remote(full) => full.split_once('/').map_or(full.as_str(), |(_, n)| n),
    }
}

fn branch_tip<'r>(branch: &git2::Branch<'r>) -> Result<git2::Commit<'r>, SwitchRefusal> {
    branch
        .get()
        .peel_to_commit()
        .map_err(|e| failed("git branch", e))
}

/// The commit a switch to `target` checks out: the local branch's tip, or
/// the remote's when no local branch of that name exists yet.
pub(crate) fn target_commit<'r>(
    repo: &'r git2::Repository,
    target: &BranchTarget,
) -> Result<git2::Commit<'r>, SwitchRefusal> {
    if let Ok(branch) = repo.find_branch(local_name(target), git2::BranchType::Local) {
        return branch_tip(&branch);
    }
    let BranchTarget::Remote(full) = target else {
        return Err(SwitchRefusal::UnknownBranch);
    };
    let branch = repo
        .find_branch(full, git2::BranchType::Remote)
        .map_err(|_| SwitchRefusal::UnknownBranch)?;
    branch_tip(&branch)
}

/// Switch the working tree and HEAD to `target`, restoring Git LFS content;
/// a refused or failed switch leaves them as they were. `held` lists the
/// paths, relative to the repository root, that the app holds in memory
/// without watching the disk: a switch that would change one is refused.
/// Returns the local branch now checked out.
pub fn switch_branch(
    project_dir: &Path,
    target: &BranchTarget,
    held: &[PathBuf],
) -> Result<String, SwitchRefusal> {
    crate::adapters::local_git::branch_switch::switch(project_dir, target, held)
}

/// The paths, relative to the repository root, a switch to `target` would
/// change. Reads only.
pub fn changed_paths(
    project_dir: &Path,
    target: &BranchTarget,
) -> Result<Vec<PathBuf>, SwitchRefusal> {
    let repo = git2::Repository::open(project_dir).map_err(|e| failed("git open", e))?;
    let to = target_commit(&repo, target)?
        .tree()
        .map_err(|e| failed("git tree", e))?;
    let from = repo
        .head()
        .and_then(|head| head.peel_to_tree())
        .map_err(|_| SwitchRefusal::NoCommits)?;
    let diff = repo
        .diff_tree_to_tree(Some(&from), Some(&to), None)
        .map_err(|e| failed("git diff", e))?;
    Ok(diff
        .deltas()
        .filter_map(|delta| delta.new_file().path().or(delta.old_file().path()))
        .map(Path::to_path_buf)
        .collect())
}

/// The bytes `rel` holds on `target`; `None` when the branch lacks it.
pub fn file_on_branch(
    project_dir: &Path,
    target: &BranchTarget,
    rel: &Path,
) -> Result<Option<Vec<u8>>, SwitchRefusal> {
    let repo = git2::Repository::open(project_dir).map_err(|e| failed("git open", e))?;
    let tree = target_commit(&repo, target)?
        .tree()
        .map_err(|e| failed("git tree", e))?;
    let Ok(entry) = tree.get_path(rel) else {
        return Ok(None);
    };
    let blob = repo
        .find_blob(entry.id())
        .map_err(|e| failed("git blob", e))?;
    Ok(Some(blob.content().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit_file(repo: &git2::Repository, path: &str, body: &str, msg: &str) -> git2::Oid {
        let root = repo.workdir().unwrap();
        std::fs::write(root.join(path), body).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
    }

    /// `a.snxsch` holds "main" on the first branch and "side" on `side`,
    /// and the first branch is checked out. Returns its name.
    fn two_branches(repo: &git2::Repository) -> String {
        let base = commit_file(repo, "a.snxsch", "main", "base");
        let first = repo.head().unwrap().shorthand().unwrap().to_owned();
        repo.branch("side", &repo.find_commit(base).unwrap(), false)
            .unwrap();
        repo.set_head("refs/heads/side").unwrap();
        commit_file(repo, "a.snxsch", "side", "side work");
        repo.set_head(&format!("refs/heads/{first}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .unwrap();
        first
    }

    fn head_name(repo: &git2::Repository) -> String {
        repo.head().unwrap().shorthand().unwrap().to_owned()
    }

    #[test]
    fn a_switch_moves_head_and_the_working_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        two_branches(&repo);

        let name = switch_branch(dir.path(), &BranchTarget::Local("side".into()), &[]).unwrap();

        assert_eq!(name, "side");
        assert_eq!(head_name(&repo), "side");
        let body = std::fs::read_to_string(dir.path().join("a.snxsch")).unwrap();
        assert_eq!(body, "side");
    }

    /// The data-loss case: an edit on disk the commit does not hold survives
    /// a refused switch, byte for byte, and HEAD stays.
    #[test]
    fn a_switch_over_a_local_change_is_refused_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let first = two_branches(&repo);
        std::fs::write(dir.path().join("a.snxsch"), "unsaved to git").unwrap();

        let result = switch_branch(dir.path(), &BranchTarget::Local("side".into()), &[]);

        assert_eq!(result, Err(SwitchRefusal::WouldOverwrite));
        assert_eq!(head_name(&repo), first);
        let body = std::fs::read_to_string(dir.path().join("a.snxsch")).unwrap();
        assert_eq!(body, "unsaved to git");
    }

    #[test]
    fn a_switch_that_changes_a_held_file_is_refused_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let first = two_branches(&repo);

        let result = switch_branch(
            dir.path(),
            &BranchTarget::Local("side".into()),
            &[PathBuf::from("a.snxsch")],
        );

        assert_eq!(
            result,
            Err(SwitchRefusal::HeldFileDiffers("a.snxsch".into()))
        );
        assert_eq!(head_name(&repo), first);
        let body = std::fs::read_to_string(dir.path().join("a.snxsch")).unwrap();
        assert_eq!(body, "main");
    }

    /// A library that is the repository itself is held as an empty path.
    #[test]
    fn an_empty_held_path_holds_every_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let first = two_branches(&repo);

        let result = switch_branch(
            dir.path(),
            &BranchTarget::Local("side".into()),
            &[PathBuf::new()],
        );

        assert_eq!(
            result,
            Err(SwitchRefusal::HeldFileDiffers("a.snxsch".into()))
        );
        assert_eq!(head_name(&repo), first);
    }

    #[test]
    fn a_new_branch_leaves_head_where_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let first = two_branches(&repo);

        create_branch(dir.path(), "ripple-fix", None).unwrap();

        assert_eq!(head_name(&repo), first);
        assert!(
            repo.find_branch("ripple-fix", git2::BranchType::Local)
                .is_ok()
        );
        assert!(create_branch(dir.path(), "ripple-fix", None).is_err());
    }

    #[test]
    fn a_tag_lands_on_the_named_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sha = commit_file(&repo, "a.snxsch", "one", "base");

        create_tag(dir.path(), "v1.0", &sha.to_string()).unwrap();

        let tagged = repo.revparse_single("refs/tags/v1.0").unwrap();
        assert_eq!(tagged.id(), sha);
    }

    #[test]
    fn a_remote_target_switches_to_its_local_name() {
        assert_eq!(
            local_name(&BranchTarget::Remote("origin/feature/x".into())),
            "feature/x"
        );
        assert_eq!(local_name(&BranchTarget::Local("main".into())), "main");
    }
}
