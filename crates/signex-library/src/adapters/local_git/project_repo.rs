//! What a project's repository holds right now, and the three things the
//! app does to it outside a commit: read it, switch its branch and set the
//! commit author in its git config (mockup rows 35 and 39).
//!
//! A switch never overwrites work and never leaves a file the app cannot
//! follow: see [`switch_branch`].

use std::path::Path;

use crate::adapter::LibraryError;
use crate::git_identity::identity_for_repo;

/// One local branch, counted against the branch checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchFacts {
    pub name: String,
    pub is_current: bool,
    /// Commits on this branch the current one lacks.
    pub ahead: usize,
    /// Commits on the current branch this one lacks.
    pub behind: usize,
}

/// The newest commit of the branch checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastCommit {
    pub short_sha: String,
    pub subject: String,
    /// Commit time, in seconds since the Unix epoch.
    pub seconds: i64,
}

/// A project repository as last read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoFacts {
    /// The branch checked out; `None` on a detached HEAD.
    pub branch: Option<String>,
    /// Local branches by name. An unborn current branch is listed too.
    pub branches: Vec<BranchFacts>,
    /// `None` before the first commit.
    pub last_commit: Option<LastCommit>,
    /// `Name <email>` commits are written as; `None` when git has no
    /// identity for this repository.
    pub author: Option<String>,
}

/// Why a branch switch did not happen. Nothing was changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchRefusal {
    /// No local branch of that name.
    UnknownBranch,
    /// No commit yet, so there is nothing to switch from.
    NoCommits,
    /// The two branches hold different project files, which the open
    /// project cannot follow.
    ProjectFileDiffers,
    /// A Git LFS file differs; a checkout would leave its pointer in place
    /// of the content.
    LfsFileDiffers(String),
    /// A file changed since its last commit would be overwritten.
    WouldOverwrite,
    /// git failed; the system's reason.
    Failed(String),
}

fn backend(what: &str, e: git2::Error) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
}

fn failed(what: &str, e: git2::Error) -> SwitchRefusal {
    SwitchRefusal::Failed(format!("{what}: {e}"))
}

/// The branch HEAD names, even when it has no commit yet.
fn current_branch(repo: &git2::Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    let target = head.symbolic_target().ok()??;
    target.strip_prefix("refs/heads/").map(str::to_owned)
}

fn local_branches(
    repo: &git2::Repository,
    current: Option<&str>,
    head: Option<git2::Oid>,
) -> Result<Vec<BranchFacts>, LibraryError> {
    let listed = repo
        .branches(Some(git2::BranchType::Local))
        .map_err(|e| backend("git branches", e))?;
    let mut out = Vec::new();
    for entry in listed {
        let (branch, _) = entry.map_err(|e| backend("git branch", e))?;
        let Ok(Some(name)) = branch.name() else {
            continue;
        };
        let name = name.to_owned();
        let tip = branch.get().target();
        let (ahead, behind) = match (tip, head) {
            (Some(tip), Some(head)) if tip != head => repo
                .graph_ahead_behind(tip, head)
                .map_err(|e| backend("git ahead/behind", e))?,
            _ => (0, 0),
        };
        out.push(BranchFacts {
            is_current: current == Some(name.as_str()),
            name,
            ahead,
            behind,
        });
    }
    if let Some(unborn) = current.filter(|c| !out.iter().any(|b| b.name == *c)) {
        out.push(BranchFacts {
            name: unborn.to_owned(),
            is_current: true,
            ahead: 0,
            behind: 0,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Read the repository at `project_root`. Never creates one.
pub fn read_repo_facts(project_root: &Path) -> Result<RepoFacts, LibraryError> {
    let repo = git2::Repository::open(project_root).map_err(|e| backend("git open", e))?;
    let branch = current_branch(&repo);
    let head = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let branches = local_branches(&repo, branch.as_deref(), head.as_ref().map(|c| c.id()))?;
    let last_commit = head.as_ref().map(|commit| LastCommit {
        short_sha: commit.id().to_string().chars().take(7).collect(),
        subject: commit
            .summary()
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_owned(),
        seconds: commit.time().seconds(),
    });
    let author = identity_for_repo(&repo)
        .ok()
        .map(|(name, email)| format!("{name} <{email}>"));
    Ok(RepoFacts {
        branch,
        branches,
        last_commit,
        author,
    })
}

/// Read `Name <email>` as the two values git stores. `None` for any other
/// shape, or a value git would refuse in a commit.
pub fn parse_author(text: &str) -> Option<(String, String)> {
    let inner = text.trim().strip_suffix('>')?;
    let (name, email) = inner.rsplit_once('<')?;
    let (name, email) = (name.trim(), email.trim());
    if name.is_empty() || email.is_empty() || email.contains(char::is_whitespace) {
        return None;
    }
    git2::Signature::now(name, email).ok()?;
    Some((name.to_owned(), email.to_owned()))
}

/// Write the commit author to this repository's own git config. `None`
/// removes it, so git's user-level identity applies again.
pub fn write_author(project_root: &Path, author: Option<(&str, &str)>) -> Result<(), LibraryError> {
    let repo = git2::Repository::open(project_root).map_err(|e| backend("git open", e))?;
    let config = repo.config().map_err(|e| backend("git config", e))?;
    let mut local = config
        .open_level(git2::ConfigLevel::Local)
        .map_err(|e| backend("git config", e))?;
    match author {
        Some((name, email)) => {
            local
                .set_str("user.name", name)
                .map_err(|e| backend("git config user.name", e))?;
            local
                .set_str("user.email", email)
                .map_err(|e| backend("git config user.email", e))?;
        }
        None => {
            for key in ["user.name", "user.email"] {
                match local.remove(key) {
                    Err(e) if e.code() != git2::ErrorCode::NotFound => {
                        return Err(backend("git config", e));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Refuse a switch whose changed files the app could not follow.
fn check_changed_files(
    repo: &git2::Repository,
    from: &git2::Tree<'_>,
    to: &git2::Tree<'_>,
    project_file: &Path,
) -> Result<(), SwitchRefusal> {
    let diff = repo
        .diff_tree_to_tree(Some(from), Some(to), None)
        .map_err(|e| failed("git diff", e))?;
    for delta in diff.deltas() {
        for path in [delta.old_file().path(), delta.new_file().path()]
            .into_iter()
            .flatten()
        {
            if path == project_file {
                return Err(SwitchRefusal::ProjectFileDiffers);
            }
            if super::lfs::is_lfs_path(repo, path).map_err(|e| failed("git attributes", e))? {
                return Err(SwitchRefusal::LfsFileDiffers(path.display().to_string()));
            }
        }
    }
    Ok(())
}

/// Check out the local branch `name`. `project_file` is the `.snxprj`,
/// relative to `project_root`.
///
/// The checkout is git's safe one: a file changed since its last commit that
/// the switch would replace refuses the whole switch before anything is
/// written. Unsaved edits in the app are the caller's to refuse.
pub fn switch_branch(
    project_root: &Path,
    name: &str,
    project_file: &Path,
) -> Result<(), SwitchRefusal> {
    let repo = git2::Repository::open(project_root).map_err(|e| failed("git open", e))?;
    let _lock =
        super::commit_lock::lock_repo_for_commit(repo.path()).map_err(SwitchRefusal::Failed)?;
    let branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| SwitchRefusal::UnknownBranch)?;
    let refname = branch
        .get()
        .name()
        .map_err(|e| failed("git branch name", e))?
        .to_owned();
    let to = branch
        .get()
        .peel_to_tree()
        .map_err(|e| failed("git branch tree", e))?;
    let from = repo
        .head()
        .and_then(|head| head.peel_to_tree())
        .map_err(|_| SwitchRefusal::NoCommits)?;
    check_changed_files(&repo, &from, &to, project_file)?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(to.as_object(), Some(&mut checkout))
        .map_err(|e| match e.code() {
            git2::ErrorCode::Conflict => SwitchRefusal::WouldOverwrite,
            _ => failed("git checkout", e),
        })?;
    repo.set_head(&refname).map_err(|e| failed("git set head", e))
}

#[cfg(test)]
#[path = "project_repo_tests.rs"]
mod tests;
