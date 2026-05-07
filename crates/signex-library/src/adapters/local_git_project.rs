//! Project-scoped git adapter — local version control for `.snxprj`
//! and its sibling design files (`.snxsch`, `.snxpcb`, `.snxmat`,
//! `.snxnet`, `.snxbom`, `.snxout`).
//!
//! Sister module to [`local_git::LocalGitAdapter`] (which manages the
//! `.snxlib` directory). This adapter operates at *project-root*
//! scope — the parent directory of the `.snxprj` file — so the same
//! repo covers schematic, PCB, simulation models, and exported
//! artefacts.
//!
//! ```text
//! my-project/                          (project_root — git working tree)
//! ├── my-project.snxprj                (manifest)
//! ├── sheets/<name>.snxsch
//! ├── pcb/<name>.snxpcb
//! ├── models/                          (3D models, optionally LFS-tracked)
//! ├── outputs/                         (.snxout, generated)
//! ├── .gitattributes                   (lf for .snx*, binary for .step etc)
//! └── .git/
//! ```
//!
//! Per-file commit semantics: every save dispatches a single
//! `commit_path(rel_path, message)` after the atomic write succeeds.
//! Failure surfaces as a non-modal status-bar warning — the user's
//! data is on disk regardless of whether git captures it.
//!
//! Public surface (mirrors the v0.22 PROJECT_GIT_PLAN.md spec):
//! - [`LocalGitProjectAdapter::open_or_init`]
//! - [`LocalGitProjectAdapter::commit_path`]
//! - [`LocalGitProjectAdapter::commit_external_change`]
//! - [`LocalGitProjectAdapter::file_history`]
//! - [`LocalGitProjectAdapter::restore_at`]
//!
//! Concurrency: per-instance `Mutex` serialises every git operation
//! that mutates `.git/index`. Mirrors the
//! [`local_git::LocalGitAdapter`] HI-11 fix.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::adapter::{HistoryEntry, LibraryError};

/// Project-scoped git adapter.
///
/// Construct via [`LocalGitProjectAdapter::open_or_init`]; the
/// adapter does *not* take ownership of the project file itself —
/// the app layer continues to write `.snxprj` / `.snxsch` etc.
/// atomically. The adapter is purely the version-control layer.
#[derive(Debug)]
pub struct LocalGitProjectAdapter {
    project_root: PathBuf,
    /// Serialises concurrent commits to avoid `.git/index.lock` races
    /// when two threads try to commit at the same time. Same fix as
    /// HI-11 on [`local_git::LocalGitAdapter`].
    git_lock: Mutex<()>,
}

impl LocalGitProjectAdapter {
    /// Open the git repo at `project_root`, or `git init` if no
    /// `.git/` directory exists yet. Returns the adapter.
    ///
    /// The initial commit is *not* created here — `commit_path` is
    /// the first commit on a freshly-initialised repo. The migration
    /// flow in the app layer is responsible for the "Initial commit
    /// (Signex import)" call after `git init` when the user first
    /// opts into version control.
    pub fn open_or_init(project_root: PathBuf) -> Result<Self, LibraryError> {
        if !project_root.is_dir() {
            return Err(LibraryError::Backend(format!(
                "project_root is not a directory: {}",
                project_root.display()
            )));
        }
        let dot_git = project_root.join(".git");
        if !dot_git.exists() {
            git2::Repository::init(&project_root)
                .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;
        } else {
            // Probe — fail early if the path exists but isn't a real
            // repo (e.g. a stray file named `.git`).
            git2::Repository::open(&project_root)
                .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        }
        Ok(Self {
            project_root,
            git_lock: Mutex::new(()),
        })
    }

    /// Commit a single file with the supplied message. `rel_path` is
    /// relative to the project root and uses forward slashes.
    /// Returns the commit OID on success.
    ///
    /// Uses an unborn-HEAD-tolerant parent-commit lookup so the very
    /// first commit on a fresh `git init` succeeds without the
    /// caller needing to handle that edge case.
    pub fn commit_path(
        &self,
        rel_path: &Path,
        message: &str,
    ) -> Result<git2::Oid, LibraryError> {
        let _guard = self.git_lock.lock().unwrap_or_else(|e| e.into_inner());
        let repo = git2::Repository::open(&self.project_root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(rel_path)
            .map_err(|e| LibraryError::Backend(format!("git add: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;

        let parent = match repo.head() {
            Ok(h) => h
                .peel_to_commit()
                .map_err(|e| LibraryError::Backend(format!("git peel: {e}")))
                .map(Some)?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
        };
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let final_message = if message.is_empty() {
            format!("save {}", rel_path.display())
        } else {
            message.to_string()
        };
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, &final_message, &tree, &parents)
            .map_err(|e| LibraryError::Backend(format!("git commit: {e}")))?;
        Ok(oid)
    }

    /// Commit an externally-edited file (user opened the file in a
    /// text editor and saved). Wraps [`commit_path`] with a
    /// reasonable default message when the caller doesn't have
    /// context.
    pub fn commit_external_change(
        &self,
        abs_path: &Path,
        message: &str,
    ) -> Result<git2::Oid, LibraryError> {
        let rel = abs_path.strip_prefix(&self.project_root).map_err(|_| {
            LibraryError::Backend(format!(
                "commit_external_change: {} is not under {}",
                abs_path.display(),
                self.project_root.display(),
            ))
        })?;
        let final_message = if message.is_empty() {
            format!("User edit (out of app): {}", rel.display())
        } else {
            message.to_string()
        };
        self.commit_path(rel, &final_message)
    }

    /// Per-file commit history newest-first. Returns up to `limit`
    /// entries. Walks the repo's commit graph filtering on commits
    /// whose tree differs from at least one parent at `rel_path`.
    /// Equivalent to `git log -- <rel_path>` semantics.
    ///
    /// Empty when no `.git/` exists, the path has never been
    /// committed, or HEAD is unborn (fresh `git init` before any
    /// commit).
    pub fn file_history(
        &self,
        rel_path: &Path,
        limit: usize,
    ) -> Result<Vec<HistoryEntry>, LibraryError> {
        if !self.project_root.join(".git").exists() {
            return Ok(Vec::new());
        }
        let repo = git2::Repository::open(&self.project_root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        // Fresh `git init` with no commits — head() reports either
        // `UnbornBranch` (the branch ref doesn't yet exist) OR
        // `NotFound` (no `refs/heads/*` at all). Both are legitimate
        // "no history" cases, not errors.
        match repo.head() {
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.code(),
                    git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
                ) =>
            {
                return Ok(Vec::new());
            }
            Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
        }
        let mut walk = repo
            .revwalk()
            .map_err(|e| LibraryError::Backend(format!("git revwalk: {e}")))?;
        walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
            .map_err(|e| LibraryError::Backend(format!("git revwalk sort: {e}")))?;
        match walk.push_head() {
            Ok(()) => {}
            Err(e)
                if matches!(
                    e.code(),
                    git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
                ) =>
            {
                return Ok(Vec::new());
            }
            Err(e) => return Err(LibraryError::Backend(format!("git push head: {e}"))),
        }

        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let mut entries: Vec<HistoryEntry> = Vec::new();
        for oid_res in walk {
            if entries.len() >= limit {
                break;
            }
            let oid = oid_res
                .map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;
            // Diff against the first parent (or empty tree for the
            // root commit). If `rel_path` is touched, include the
            // commit.
            let touched = commit_touches_path(&repo, &commit, &rel_str)?;
            if !touched {
                continue;
            }
            entries.push(history_entry_from_commit(&commit));
        }
        Ok(entries)
    }

    /// Restore `rel_path` to the state captured by `commit_oid`.
    /// Atomic write — staging file + rename, working tree stays
    /// consistent if the rename fails.
    ///
    /// Does NOT create a new commit — that's the app layer's call
    /// (it'll observe the dirty file on next save and commit through
    /// [`commit_path`] with a "Restore from <sha>" message).
    pub fn restore_at(
        &self,
        rel_path: &Path,
        commit_oid: git2::Oid,
    ) -> Result<(), LibraryError> {
        let repo = git2::Repository::open(&self.project_root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let commit = repo
            .find_commit(commit_oid)
            .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;
        let tree = commit
            .tree()
            .map_err(|e| LibraryError::Backend(format!("git tree: {e}")))?;
        let entry = tree.get_path(rel_path).map_err(|e| {
            LibraryError::Backend(format!(
                "{} not present at {}: {e}",
                rel_path.display(),
                commit_oid
            ))
        })?;
        let blob = repo
            .find_blob(entry.id())
            .map_err(|e| LibraryError::Backend(format!("git find blob: {e}")))?;
        let content = blob.content();

        let abs = self.project_root.join(rel_path);
        signex_types::atomic_io::atomic_write(&abs, content).map_err(|e| {
            LibraryError::Backend(format!("atomic write {}: {e}", abs.display()))
        })?;
        Ok(())
    }

    /// Project root that this adapter manages.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

fn commit_touches_path(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_str: &str,
) -> Result<bool, LibraryError> {
    let tree = commit
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;
    if commit.parent_count() == 0 {
        // Root commit — file appears here if the tree contains it.
        return Ok(tree.get_path(Path::new(rel_str)).is_ok());
    }
    let parent = commit
        .parent(0)
        .map_err(|e| LibraryError::Backend(format!("git commit parent: {e}")))?;
    let parent_tree = parent
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git parent tree: {e}")))?;
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(rel_str);
    let diff = repo
        .diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut opts))
        .map_err(|e| LibraryError::Backend(format!("git diff: {e}")))?;
    Ok(diff.deltas().count() > 0)
}

fn history_entry_from_commit(commit: &git2::Commit<'_>) -> HistoryEntry {
    use chrono::TimeZone;
    let author = commit.author();
    let when = author.when();
    let time = chrono::Utc
        .timestamp_opt(when.seconds(), 0)
        .single()
        .unwrap_or_else(chrono::Utc::now);
    let full = commit.message().unwrap_or_default();
    let (subject, body) = match full.find('\n') {
        Some(i) => {
            let s = &full[..i];
            let rest = full[i + 1..].trim_start_matches('\n').to_string();
            (s.to_string(), rest)
        }
        None => (full.to_string(), String::new()),
    };
    let parent_shas: Vec<String> = commit.parent_ids().map(|o| o.to_string()).collect();
    HistoryEntry {
        sha: commit.id().to_string(),
        author_name: author.name().unwrap_or("").to_string(),
        author_email: author.email().unwrap_or("").to_string(),
        time,
        subject,
        body,
        parent_shas,
        files_changed: Vec::new(),
        additions: 0,
        deletions: 0,
    }
}

fn identity_for_repo(repo: &git2::Repository) -> (String, String) {
    let cfg = repo.config().ok();
    let name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Signex Project".to_string());
    let email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "project@signex.local".to_string());
    (name, email)
}
