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
    pub fn commit_path(&self, rel_path: &Path, message: &str) -> Result<git2::Oid, LibraryError> {
        // v0.23 — reject obviously dangerous `rel_path`. Internal
        // callers strip-prefix from absolute paths so this guards
        // against future callers that forget. Both absolute paths
        // and `..` components could land git2 outside the project
        // root, breaking the repo invariants.
        if rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(LibraryError::Backend(format!(
                "rel_path must be project-relative without `..`: {}",
                rel_path.display()
            )));
        }
        // Poison errors are recovered with `into_inner` because the
        // mutex protects only `.git/index.lock` ordering — a panic
        // mid-commit can't corrupt the lock state in a way that
        // matters for the next caller (libgit2's index re-validates
        // on each open).
        let _guard = self.git_lock.lock().unwrap_or_else(|e| e.into_inner());
        let repo = git2::Repository::open(&self.project_root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo)?;
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
        // v0.23 — DoS guard. `file_history` walks the entire commit
        // graph until `limit` matches, which is bounded for hand-
        // authored project repos but unbounded for forks that rarely
        // touch `rel_path`. Cap at 10× requested limit to avoid
        // pathological hangs; if more matches exist they'll surface
        // on a subsequent call with a higher `limit`.
        let max_visited = limit.saturating_mul(10).max(limit);
        let mut visited = 0usize;
        for oid_res in walk {
            if entries.len() >= limit || visited >= max_visited {
                break;
            }
            visited += 1;
            let oid =
                oid_res.map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;
            // Diff against the first parent (or empty tree for the
            // root commit). If `rel_path` is touched, include the
            // commit + populate diff stats.
            let stats = commit_diff_stats_for_path(&repo, &commit, &rel_str)?;
            if stats.is_none() {
                continue;
            }
            let stats = stats.unwrap();
            entries.push(history_entry_from_commit(&commit, stats));
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
    pub fn restore_at(&self, rel_path: &Path, commit_oid: git2::Oid) -> Result<(), LibraryError> {
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
        signex_types::atomic_io::atomic_write(&abs, content)
            .map_err(|e| LibraryError::Backend(format!("atomic write {}: {e}", abs.display())))?;
        Ok(())
    }

    /// Project root that this adapter manages.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// String-SHA-keyed alternative to [`restore_at`]. Parses the
    /// argument as a hex commit OID, then forwards. Convenient for
    /// callers (notably `signex-app`) that don't depend on `git2`
    /// directly and just have the short/full SHA string from the
    /// History panel widget.
    pub fn restore_at_from_sha(&self, rel_path: &Path, sha: &str) -> Result<(), LibraryError> {
        let oid = git2::Oid::from_str(sha)
            .map_err(|e| LibraryError::Backend(format!("invalid sha `{sha}`: {e}")))?;
        self.restore_at(rel_path, oid)
    }

    /// Write the project's `.gitattributes` file with the v0.22 spec:
    /// - `text eol=lf` for every `.snx*` extension so git stays
    ///   line-ending stable across Windows / macOS / Linux
    ///   collaborators.
    /// - `binary` for `.step` / `.wrl` / `.png` / `.pdf` so git
    ///   doesn't try to diff or merge them.
    /// - `filter=lfs diff=lfs merge=lfs -text` for everything under
    ///   `assets/3d-models/**` when `use_lfs` is on. Skipped when
    ///   off — the user can opt in later via the migration modal.
    ///
    /// **Destructive — overwrites any existing `.gitattributes`
    /// without merging.** Callers (today: the Enable Version Control
    /// modal in `app/handlers/dock/project_navigation.rs`) gate this
    /// behind explicit user action. Manual edits between Enable VC
    /// invocations are clobbered. Future v0.x can read+merge if
    /// the user demands it; today's contract is "Enable VC rewrites
    /// the file from scratch".
    ///
    /// The file is staged + committed by the migration flow's
    /// initial-commit step; callers don't need a separate commit.
    pub fn write_gitattributes(&self, use_lfs: bool) -> Result<(), LibraryError> {
        let path = self.project_root.join(".gitattributes");
        let mut text = String::new();
        text.push_str("# Generated by Signex when version control was enabled for this project.\n");
        text.push_str(
            "# Re-running \"Enable Version Control\" rewrites this file from scratch —\n",
        );
        text.push_str("# manual edits will be lost. Hand-edit only if you're not planning to\n");
        text.push_str("# re-run the modal.\n\n");
        text.push_str(
            "# Native Signex formats are line-based UTF-8 — keep LF endings everywhere\n",
        );
        text.push_str("# so cross-platform collaborators don't churn the diff with CRLF flips.\n");
        for ext in [
            "snxsch", "snxpcb", "snxprj", "snxmat", "snxnet", "snxbom", "snxout", "snxsym",
            "snxfpt", "snxlib", "snxmod",
        ] {
            text.push_str(&format!("*.{ext}\ttext eol=lf\n"));
        }
        text.push_str("\n# Binary attachments — git shouldn't diff or merge them.\n");
        for ext in ["step", "stp", "wrl", "iges", "png", "jpg", "jpeg", "pdf"] {
            text.push_str(&format!("*.{ext}\tbinary\n"));
        }
        if use_lfs {
            text.push_str("\n# 3D models opt-in via Git LFS so the working tree doesn't bloat.\n");
            text.push_str("assets/3d-models/**\tfilter=lfs diff=lfs merge=lfs -text\n");
        }
        signex_types::atomic_io::atomic_write(&path, text.as_bytes())
            .map_err(|e| LibraryError::Backend(format!("write .gitattributes: {e}")))?;
        Ok(())
    }
}

/// Diff-stat summary for one commit's touch on a single file.
/// `additions` / `deletions` are the line counts ; `files_changed`
/// records the path so the History panel can render it.
struct CommitPathStats {
    files_changed: Vec<String>,
    additions: u32,
    deletions: u32,
}

/// Returns `Some(stats)` when `commit` touched `rel_str`; `None` when
/// it didn't. Replaces the v0.22 `commit_touches_path` boolean
/// version — populating stats here avoids a second tree-walk per row.
fn commit_diff_stats_for_path(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_str: &str,
) -> Result<Option<CommitPathStats>, LibraryError> {
    let tree = commit
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;
    if commit.parent_count() == 0 {
        // Root commit — file appears here if the tree contains it.
        // Stats are derived from the file's blob size approximated as
        // newline count (additions only; no parent to diff against).
        let entry = match tree.get_path(Path::new(rel_str)) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };
        let blob = repo
            .find_blob(entry.id())
            .map_err(|e| LibraryError::Backend(format!("git find blob: {e}")))?;
        let additions = blob.content().iter().filter(|&&b| b == b'\n').count() as u32;
        return Ok(Some(CommitPathStats {
            files_changed: vec![rel_str.to_string()],
            additions,
            deletions: 0,
        }));
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
    if diff.deltas().count() == 0 {
        return Ok(None);
    }
    let stats = diff
        .stats()
        .map_err(|e| LibraryError::Backend(format!("git diff stats: {e}")))?;
    Ok(Some(CommitPathStats {
        files_changed: vec![rel_str.to_string()],
        additions: stats.insertions() as u32,
        deletions: stats.deletions() as u32,
    }))
}

fn history_entry_from_commit(commit: &git2::Commit<'_>, stats: CommitPathStats) -> HistoryEntry {
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
        files_changed: stats.files_changed,
        additions: stats.additions,
        deletions: stats.deletions,
    }
}

/// Resolve the user identity for a commit. Walks a chain of sources
/// so the user's real name lands on the commit even when the OS
/// distribution didn't pre-populate `~/.gitconfig`:
///
/// 1. Repo-local `user.name` / `user.email` (the standard git path).
/// 2. Process env vars `GIT_AUTHOR_NAME` / `GIT_AUTHOR_EMAIL` and
///    `GIT_COMMITTER_NAME` / `GIT_COMMITTER_EMAIL` — git's own
///    fallback hierarchy, which Signex honours so CI / scripted
///    flows don't need to mutate `~/.gitconfig`.
/// 3. POSIX-style env hints `USER` / `EMAIL` for the convenience case
///    of "developer hasn't configured git yet but their shell knows
///    their identity". Email derives a synthetic local-host address
///    when `EMAIL` is unset (`<user>@<hostname>`).
///
/// Returns `Err(LibraryError::Backend("git identity not configured…"))`
/// when none of the above resolves a name. CLAUDE.md durable rule:
/// commits must carry the user's identity — never a generic
/// "Signex Project" / "Signex bot" fallback. Surfaces through the
/// async commit pipeline as a `Message::ProjectGitCommitDone` error
/// the user can act on.
fn identity_for_repo(repo: &git2::Repository) -> Result<(String, String), LibraryError> {
    let cfg = repo.config().ok();
    // Layer 1 — git config.
    let cfg_name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .filter(|s| !s.trim().is_empty());
    let cfg_email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .filter(|s| !s.trim().is_empty());

    // Layer 2 — git's own env-var fallback.
    let env_name = std::env::var("GIT_AUTHOR_NAME")
        .ok()
        .or_else(|| std::env::var("GIT_COMMITTER_NAME").ok())
        .filter(|s| !s.trim().is_empty());
    let env_email = std::env::var("GIT_AUTHOR_EMAIL")
        .ok()
        .or_else(|| std::env::var("GIT_COMMITTER_EMAIL").ok())
        .filter(|s| !s.trim().is_empty());

    // Layer 3 — POSIX-style USER/EMAIL hint for the bare-shell case.
    let posix_name = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .filter(|s| !s.trim().is_empty());
    let posix_email = std::env::var("EMAIL").ok().filter(|s| !s.trim().is_empty());

    let name = cfg_name.or(env_name).or(posix_name).ok_or_else(|| {
        LibraryError::Backend(
            "git identity not configured — set user.name via \
             `git config --global user.name \"…\"` (or set GIT_AUTHOR_NAME). \
             Signex won't author commits with a generic identity."
                .to_string(),
        )
    })?;
    let email = cfg_email.or(env_email).or(posix_email).unwrap_or_else(|| {
        // Synthetic `<user>@<hostname>` lands the commit author in
        // git log without leaking a fake organisation domain. Users
        // who care can set `user.email` properly; we just don't
        // BLOCK the commit on its absence.
        let host = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "localhost".to_string());
        format!("{}@{}", name, host)
    });
    Ok((name, email))
}
