//! What the History panel reads of a whole repository: the commits of every
//! branch with their parents, the branch and tag tips, and each branch's
//! distance from HEAD. Read only.

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::adapter::{HistoryEntry, LibraryError};
use crate::git_history::{commit_touches_path, head_walk};

/// Most commits one snapshot lists, newest first.
pub const MAX_GRAPH_COMMITS: usize = 500;

/// One commit of the repository graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoCommit {
    pub sha: String,
    /// First parent first.
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub time: DateTime<Utc>,
    pub subject: String,
    /// Reachable from HEAD.
    pub on_head: bool,
    /// Changed the file the snapshot was taken for.
    pub touches_file: bool,
    /// Holds that file, so it can be restored from this commit.
    pub has_file: bool,
}

/// What a ref tip is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoRefKind {
    Branch { is_current: bool },
    Tag,
}

/// A branch or tag and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRefTip {
    pub sha: String,
    /// Short name: `main`, `origin/main`, `v1.0`.
    pub name: String,
    pub kind: RepoRefKind,
}

/// One row of the branch switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoBranch {
    /// Short name: `main`, `origin/main`.
    pub name: String,
    pub is_remote: bool,
    pub is_current: bool,
    /// Commits on this branch that HEAD lacks.
    pub ahead: usize,
    /// Commits on HEAD that this branch lacks.
    pub behind: usize,
}

/// The repository as the History panel shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoSnapshot {
    pub commits: Vec<RepoCommit>,
    pub tips: Vec<RepoRefTip>,
    pub branches: Vec<RepoBranch>,
    /// `None` on a detached or unborn HEAD.
    pub current_branch: Option<String>,
    pub head_sha: Option<String>,
}

/// How a commit changed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoFileChangeKind {
    Modified,
    Added,
    Deleted,
}

/// One changed file of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFileChange {
    pub kind: RepoFileChangeKind,
    /// Relative to the repository root, forward slashes.
    pub path: String,
}

fn backend(what: &str, e: git2::Error) -> LibraryError {
    LibraryError::Backend(format!("{what}: {e}"))
}

/// Whether `name` can name a new local branch.
pub fn is_valid_branch_name(name: &str) -> bool {
    git2::Branch::name_is_valid(name).unwrap_or(false)
}

/// Whether `name` can name a new tag.
pub fn is_valid_tag_name(name: &str) -> bool {
    !name.is_empty() && git2::Reference::is_valid_name(&format!("refs/tags/{name}"))
}

/// Read the graph of the repository at `project_dir`, with each commit's
/// relation to `rel_path` (forward slashes). An unborn HEAD is an empty
/// snapshot.
pub fn repo_snapshot(project_dir: &Path, rel_path: &str) -> Result<RepoSnapshot, LibraryError> {
    let repo = git2::Repository::open(project_dir).map_err(|e| backend("git open", e))?;
    let Some(mut walk) = head_walk(&repo)? else {
        return Ok(RepoSnapshot::default());
    };
    for glob in ["refs/heads/*", "refs/remotes/*", "refs/tags/*"] {
        walk.push_glob(glob)
            .map_err(|e| backend("git walk refs", e))?;
    }
    let head = repo.head().map_err(|e| backend("git head", e))?;
    let head_oid = head
        .peel_to_commit()
        .map_err(|e| backend("git head commit", e))?
        .id();
    let current_branch = head
        .is_branch()
        .then(|| head.shorthand().ok().map(str::to_owned))
        .flatten();
    let on_head = commits_on_head(&repo)?;
    let mut commits = Vec::new();
    for oid in walk.take(MAX_GRAPH_COMMITS) {
        let oid = oid.map_err(|e| backend("git revwalk oid", e))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| backend("git find commit", e))?;
        commits.push(repo_commit(&repo, &commit, rel_path, &on_head)?);
    }
    let (tips, branches) = read_refs(&repo, head_oid, current_branch.as_deref())?;
    Ok(RepoSnapshot {
        commits,
        tips,
        branches,
        current_branch,
        head_sha: Some(head_oid.to_string()),
    })
}

/// The commits reachable from HEAD, as far as a snapshot can list them.
fn commits_on_head(repo: &git2::Repository) -> Result<HashSet<git2::Oid>, LibraryError> {
    let Some(walk) = head_walk(repo)? else {
        return Ok(HashSet::new());
    };
    walk.take(MAX_GRAPH_COMMITS)
        .map(|oid| oid.map_err(|e| backend("git revwalk oid", e)))
        .collect()
}

fn repo_commit(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_path: &str,
    on_head: &HashSet<git2::Oid>,
) -> Result<RepoCommit, LibraryError> {
    let entry = HistoryEntry::from_commit(commit);
    let touches_file = commit_touches_path(repo, commit, rel_path)?;
    let has_file = commit
        .tree()
        .map_err(|e| backend("git commit tree", e))?
        .get_path(Path::new(rel_path))
        .is_ok();
    Ok(RepoCommit {
        sha: entry.sha,
        parents: commit.parent_ids().map(|id| id.to_string()).collect(),
        author_name: entry.author_name,
        author_email: entry.author_email,
        time: entry.time,
        subject: entry.subject,
        on_head: on_head.contains(&commit.id()),
        touches_file,
        has_file,
    })
}

/// The ref tips and the switcher rows, local branches before remote ones.
fn read_refs(
    repo: &git2::Repository,
    head: git2::Oid,
    current: Option<&str>,
) -> Result<(Vec<RepoRefTip>, Vec<RepoBranch>), LibraryError> {
    let mut tips = Vec::new();
    let mut branches = Vec::new();
    for reference in repo.references().map_err(|e| backend("git refs", e))? {
        let reference = reference.map_err(|e| backend("git ref", e))?;
        let Ok(full) = reference.name() else {
            continue;
        };
        let Ok(commit) = reference.peel_to_commit() else {
            continue;
        };
        let sha = commit.id().to_string();
        if let Some(name) = full.strip_prefix("refs/tags/") {
            tips.push(RepoRefTip {
                sha,
                name: name.to_owned(),
                kind: RepoRefKind::Tag,
            });
            continue;
        }
        let (name, is_remote) = match (
            full.strip_prefix("refs/heads/"),
            full.strip_prefix("refs/remotes/"),
        ) {
            (Some(name), _) => (name, false),
            // `origin/HEAD` is an alias of a branch listed on its own.
            (_, Some(name)) if !name.ends_with("/HEAD") => (name, true),
            _ => continue,
        };
        let is_current = !is_remote && current == Some(name);
        let (ahead, behind) = repo
            .graph_ahead_behind(commit.id(), head)
            .map_err(|e| backend("git ahead/behind", e))?;
        tips.push(RepoRefTip {
            sha,
            name: name.to_owned(),
            kind: RepoRefKind::Branch { is_current },
        });
        branches.push(RepoBranch {
            name: name.to_owned(),
            is_remote,
            is_current,
            ahead,
            behind,
        });
    }
    branches.sort_by(|a, b| (a.is_remote, &a.name).cmp(&(b.is_remote, &b.name)));
    Ok((tips, branches))
}

/// The files `sha` changed against its first parent (against nothing for a
/// root commit).
pub fn commit_file_changes(
    project_dir: &Path,
    sha: &str,
) -> Result<Vec<RepoFileChange>, LibraryError> {
    let repo = git2::Repository::open(project_dir).map_err(|e| backend("git open", e))?;
    let oid = git2::Oid::from_str(sha).map_err(|e| backend("git sha", e))?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| backend("git find commit", e))?;
    let tree = commit.tree().map_err(|e| backend("git commit tree", e))?;
    let parent = match commit.parent(0) {
        Ok(parent) => Some(parent.tree().map_err(|e| backend("git parent tree", e))?),
        Err(_) => None,
    };
    let diff = repo
        .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)
        .map_err(|e| backend("git diff", e))?;
    Ok(diff
        .deltas()
        .filter_map(|delta| {
            let (kind, file) = match delta.status() {
                git2::Delta::Added => (RepoFileChangeKind::Added, delta.new_file()),
                git2::Delta::Deleted => (RepoFileChangeKind::Deleted, delta.old_file()),
                _ => (RepoFileChangeKind::Modified, delta.new_file()),
            };
            let path = file.path()?.to_string_lossy().replace('\\', "/");
            Some(RepoFileChange { kind, path })
        })
        .collect())
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

    #[test]
    fn a_fresh_repository_is_an_empty_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        assert_eq!(
            repo_snapshot(dir.path(), "a.snxsch").unwrap(),
            RepoSnapshot::default()
        );
    }

    /// A commit only another branch holds is listed, is not on HEAD, and
    /// that branch counts it as ahead.
    #[test]
    fn the_snapshot_lists_every_branch_with_its_distance_from_head() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let base = commit_file(&repo, "a.snxsch", "one", "base");
        let current = repo.head().unwrap().shorthand().unwrap().to_owned();
        let base_commit = repo.find_commit(base).unwrap();
        repo.branch("side", &base_commit, false).unwrap();
        repo.set_head("refs/heads/side").unwrap();
        let side = commit_file(&repo, "b.snxsch", "two", "side work");
        repo.set_head(&format!("refs/heads/{current}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .unwrap();

        let snapshot = repo_snapshot(dir.path(), "a.snxsch").unwrap();

        let listed = snapshot
            .commits
            .iter()
            .find(|c| c.sha == side.to_string())
            .expect("the side commit is listed");
        assert!(!listed.on_head);
        assert!(!listed.touches_file, "it changed another file");
        assert!(listed.has_file, "it still holds a.snxsch");
        let row = snapshot.branches.iter().find(|b| b.name == "side").unwrap();
        assert_eq!((row.ahead, row.behind), (1, 0));
        assert_eq!(snapshot.current_branch.as_deref(), Some(current.as_str()));
    }

    #[test]
    fn a_commit_lists_its_changed_files_with_their_status() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit_file(&repo, "a.snxsch", "one", "base");
        let second = commit_file(&repo, "a.snxsch", "two", "edit");
        let changes = commit_file_changes(dir.path(), &second.to_string()).unwrap();
        assert_eq!(
            changes,
            vec![RepoFileChange {
                kind: RepoFileChangeKind::Modified,
                path: "a.snxsch".into(),
            }]
        );
    }

    #[test]
    fn branch_and_tag_names_are_checked() {
        assert!(is_valid_branch_name("ripple-fix"));
        assert!(!is_valid_branch_name("has space"));
        assert!(!is_valid_branch_name(""));
        assert!(is_valid_tag_name("v1.0"));
        assert!(!is_valid_tag_name(""));
        assert!(!is_valid_tag_name("bad..name"));
    }
}
