//! What a project's repository holds right now: reading it, and setting the
//! commit author in its git config (mockup rows 35 and 39). The branch switch
//! is `crate::repo_branches::switch_branch`; [`SwitchRefusal`] is why one did
//! not happen.

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

/// Why a branch switch did not happen. The project and the working tree are
/// as they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchRefusal {
    /// No branch of that name.
    UnknownBranch,
    /// No commit yet, so there is nothing to switch from.
    NoCommits,
    /// A file the app holds in memory without watching it differs on the
    /// target branch (an open library or primitive); its path.
    HeldFileDiffers(String),
    /// The target branch has no readable project file for the open project;
    /// its path.
    ProjectFileUnreadable(String),
    /// A Git LFS file's content is not in `.git/lfs/objects`; its path.
    LfsContentMissing(String),
    /// A file changed since its last commit would be overwritten.
    WouldOverwrite,
    /// git failed; the system's reason.
    Failed(String),
}

fn backend(what: &str, e: git2::Error) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
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

#[cfg(test)]
#[path = "project_repo_tests.rs"]
mod tests;
