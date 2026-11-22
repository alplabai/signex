//! Branch and tag writes for the shared branch switcher: create a branch or
//! tag, and switch the working tree to a branch. A switch never overwrites
//! local work: it is a safe checkout, refused before any file is written.

use std::path::{Path, PathBuf};

use crate::adapter::LibraryError;
use crate::adapters::local_git::lfs::is_lfs_path;

/// The branch a switch goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchTarget {
    Local(String),
    /// `origin/feature`: switches to the local `feature`, created to track
    /// it when there is none.
    Remote(String),
}

/// Why a switch did not happen. No file was changed, except for
/// [`SwitchFailure::Git`] raised after the checkout began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchFailure {
    /// A file the app holds in memory without watching it differs on the
    /// target branch (a project file, an open library or primitive).
    HeldFileDiffers(String),
    /// A Git LFS file differs; a checkout here would write its pointer.
    LfsFileDiffers(String),
    /// A file on disk has changes the switch would overwrite.
    LocalChanges,
    Git(String),
}

fn git_failure(what: &str, e: &git2::Error) -> SwitchFailure {
    SwitchFailure::Git(format!("{what}: {e}"))
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
fn local_name(target: &BranchTarget) -> &str {
    match target {
        BranchTarget::Local(name) => name,
        BranchTarget::Remote(full) => full.split_once('/').map_or(full.as_str(), |(_, n)| n),
    }
}

/// The commit a switch to `target` checks out: the local branch's tip, or
/// the remote's when no local branch of that name exists yet.
fn target_commit<'r>(
    repo: &'r git2::Repository,
    target: &BranchTarget,
) -> Result<git2::Commit<'r>, git2::Error> {
    let local = repo.find_branch(local_name(target), git2::BranchType::Local);
    match (local, target) {
        (Ok(branch), _) => branch.get().peel_to_commit(),
        (Err(_), BranchTarget::Remote(full)) => repo
            .find_branch(full, git2::BranchType::Remote)?
            .get()
            .peel_to_commit(),
        (Err(e), BranchTarget::Local(_)) => Err(e),
    }
}

/// Refuse a switch that would change a held or LFS file.
fn check_differing_files(
    repo: &git2::Repository,
    from: &git2::Tree<'_>,
    to: &git2::Tree<'_>,
    held: &[PathBuf],
) -> Result<(), SwitchFailure> {
    let diff = repo
        .diff_tree_to_tree(Some(from), Some(to), None)
        .map_err(|e| git_failure("git diff", &e))?;
    for delta in diff.deltas() {
        for path in [delta.old_file().path(), delta.new_file().path()]
            .into_iter()
            .flatten()
        {
            let shown = || path.to_string_lossy().replace('\\', "/");
            if held.iter().any(|h| path.starts_with(h)) {
                return Err(SwitchFailure::HeldFileDiffers(shown()));
            }
            if is_lfs_path(repo, path).map_err(|e| git_failure("git attributes", &e))? {
                return Err(SwitchFailure::LfsFileDiffers(shown()));
            }
        }
    }
    Ok(())
}

/// Switch the working tree and HEAD to `target`. `held` lists the paths,
/// relative to the repository root, that the app holds in memory without
/// watching the disk: a switch that would change one is refused. Returns the
/// local branch now checked out.
pub fn switch_branch(
    project_dir: &Path,
    target: &BranchTarget,
    held: &[PathBuf],
) -> Result<String, SwitchFailure> {
    let repo = git2::Repository::open(project_dir).map_err(|e| git_failure("git open", &e))?;
    let commit = target_commit(&repo, target).map_err(|e| git_failure("git branch", &e))?;
    let to = commit.tree().map_err(|e| git_failure("git tree", &e))?;
    let from = repo
        .head()
        .and_then(|head| head.peel_to_tree())
        .map_err(|e| git_failure("git head", &e))?;
    check_differing_files(&repo, &from, &to, held)?;

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout))
        .map_err(|e| match e.code() {
            git2::ErrorCode::Conflict => SwitchFailure::LocalChanges,
            _ => git_failure("git checkout", &e),
        })?;

    let name = local_name(target).to_owned();
    if let BranchTarget::Remote(full) = target
        && repo.find_branch(&name, git2::BranchType::Local).is_err()
    {
        let mut branch = repo
            .branch(&name, &commit, false)
            .map_err(|e| git_failure("git branch", &e))?;
        branch
            .set_upstream(Some(full))
            .map_err(|e| git_failure("git upstream", &e))?;
    }
    repo.set_head(&format!("refs/heads/{name}"))
        .map_err(|e| git_failure("git set head", &e))?;
    Ok(name)
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

        assert_eq!(result, Err(SwitchFailure::LocalChanges));
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
            Err(SwitchFailure::HeldFileDiffers("a.snxsch".into()))
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
            Err(SwitchFailure::HeldFileDiffers("a.snxsch".into()))
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
