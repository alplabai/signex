//! Newest-first git history of one path, shared by the library adapter, the
//! project-repo history and the History panel's free function so the three
//! cannot drift on what "no history" means.

use std::path::Path;

use crate::adapter::{HistoryEntry, LibraryError};

/// A revwalk over HEAD, newest first (time, then topological so commits that
/// share a one-second timestamp still list children before parents).
///
/// `None` for an unborn HEAD (fresh repo, no commits yet): that is "no
/// history", not an error. libgit2 reports it as `UnbornBranch` on POSIX,
/// `NotFound` on some macOS builds and a bare `Reference`-class error on
/// Windows, where HEAD names a branch that does not exist yet.
pub(crate) fn head_walk(
    repo: &git2::Repository,
) -> Result<Option<git2::Revwalk<'_>>, LibraryError> {
    let mut walk = repo
        .revwalk()
        .map_err(|e| LibraryError::Backend(format!("git revwalk: {e}")))?;
    walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
        .map_err(|e| LibraryError::Backend(format!("git revwalk sort: {e}")))?;
    match walk.push_head() {
        Ok(()) => Ok(Some(walk)),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) || e.class() == git2::ErrorClass::Reference =>
        {
            Ok(None)
        }
        Err(e) => Err(LibraryError::Backend(format!("git push head: {e}"))),
    }
}

/// True if `commit` modified `rel_path` relative to *any* of its parents
/// (or, for the root commit, if the path exists in its tree). Mirrors
/// `git log -- <path>` for the non-rename case.
pub(crate) fn commit_touches_path(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_path: &str,
) -> Result<bool, LibraryError> {
    let new_tree = commit
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;

    if commit.parent_count() == 0 {
        return Ok(new_tree.get_path(Path::new(rel_path)).is_ok());
    }

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(rel_path);

    for parent in commit.parents() {
        let old_tree = parent
            .tree()
            .map_err(|e| LibraryError::Backend(format!("git parent tree: {e}")))?;
        let diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))
            .map_err(|e| LibraryError::Backend(format!("git diff: {e}")))?;
        if diff.deltas().len() > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The newest `limit` commits that touched `rel_path` (forward slashes).
/// Empty for an unborn HEAD or a path never committed.
pub(crate) fn path_history(
    repo: &git2::Repository,
    rel_path: &str,
    limit: usize,
) -> Result<Vec<HistoryEntry>, LibraryError> {
    let Some(walk) = head_walk(repo)? else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for oid_res in walk {
        if entries.len() >= limit {
            break;
        }
        let oid = oid_res.map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;
        if commit_touches_path(repo, &commit, rel_path)? {
            entries.push(HistoryEntry::from_commit(&commit));
        }
    }
    Ok(entries)
}
