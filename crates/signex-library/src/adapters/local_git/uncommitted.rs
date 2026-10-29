//! A save whose file write succeeded is a successful save, even when the git
//! commit after it fails (#858).
//!
//! The failed commit is reported as [`CommitOutcome::NotCommitted`], never an
//! error: the bytes are on disk, so rolling the save back or refusing it
//! would lose work for a problem the user did not cause. Every commit path of
//! the adapter goes through [`LocalGitAdapter::commit_change_with`], which
//! keeps a ledger of the files whose commit failed. The next commit, of any
//! file of the library, records the whole ledger with it; an explicit
//! [`LocalGitAdapter::commit_pending`] does the same on its own.
//!
//! A failed commit changes nothing but the ledger. The commit is built from
//! HEAD's tree and the on-disk index is touched only after it succeeded (see
//! `helpers::stage_and_commit_with`), so there is nothing to restore after a
//! failure, and HEAD moved by another process or adapter in between cannot
//! make a later commit record a revert.

use std::collections::{BTreeSet, HashSet};

use super::helpers::*;
use super::*;
use crate::adapter::UncommittedChanges;

/// Files saved but not committed, and what was already reported about them.
#[derive(Debug, Default)]
pub(super) struct UncommittedLedger {
    /// Library-relative paths, forward slashes. A path that no longer
    /// exists is committed as a removal.
    paths: BTreeSet<String>,
    /// Why the latest attempt failed.
    reason: Option<String>,
    /// Failure causes already warned about this session: one warning per
    /// cause, not one per save.
    reported: HashSet<String>,
}

impl LocalGitAdapter {
    fn ledger(&self) -> std::sync::MutexGuard<'_, UncommittedLedger> {
        self.uncommitted.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// [`LibraryAdapter::uncommitted_changes`].
    pub(super) fn pending_changes(&self) -> Option<UncommittedChanges> {
        let ledger = self.ledger();
        if ledger.paths.is_empty() {
            return None;
        }
        Some(UncommittedChanges {
            paths: ledger.paths.iter().cloned().collect(),
            reason: ledger.reason.clone().unwrap_or_default(),
        })
    }

    /// [`LibraryAdapter::commit_pending`]: commit the ledger on its own.
    pub(super) fn commit_pending_changes(&self, message: &str) -> CommitOutcome {
        let Some(first) = self.ledger().paths.iter().next().cloned() else {
            return CommitOutcome::NothingToCommit;
        };
        // The commit needs one anchor file; prefer one that still exists.
        let anchor = {
            let ledger = self.ledger();
            ledger
                .paths
                .iter()
                .find(|p| self.root_dir.join(p).symlink_metadata().is_ok())
                .cloned()
        }
        .unwrap_or(first);
        let n = self.ledger().paths.len();
        let fallback = format!("commit {n} saved change(s)");
        self.commit_change_with(None, &anchor, &[], message, &fallback)
    }

    /// Stage `rel_path` (and `extra_rel_paths`, dropping `removed_rel_path`)
    /// and commit, together with every file the ledger still holds. A
    /// failure is the returned outcome, never an `Err`: the caller's write
    /// is already on disk.
    pub(super) fn commit_change_with(
        &self,
        removed_rel_path: Option<&str>,
        rel_path: &str,
        extra_rel_paths: &[String],
        message: &str,
        fallback_message: &str,
    ) -> CommitOutcome {
        if !self.root_dir.join(".git").exists() {
            return CommitOutcome::NothingToCommit;
        }
        // HI-11: serialise concurrent commits in this process. Two
        // threads each opening their own `git2::Repository` and
        // calling `index().add_path()` race on `.git/index.lock`; the
        // loser would surface as a `git add: error` failure to the
        // user despite no real conflict.
        let _git_guard = self.git_lock.lock().unwrap_or_else(|e| e.into_inner());
        let removed: Vec<String> = removed_rel_path.map(str::to_owned).into_iter().collect();
        let message = if message.is_empty() {
            fallback_message
        } else {
            message
        };
        match self.try_commit(&removed, rel_path, extra_rel_paths, message) {
            Ok(Some(_)) => {
                self.clear_ledger();
                CommitOutcome::Committed
            }
            Ok(None) => {
                self.clear_ledger();
                CommitOutcome::NothingToCommit
            }
            Err(reason) => self.note_not_committed(&removed, rel_path, extra_rel_paths, reason),
        }
    }

    fn clear_ledger(&self) {
        let mut ledger = self.ledger();
        ledger.paths.clear();
        ledger.reason = None;
    }

    /// One attempt, under the in-process git lock. `Err` is the reason.
    fn try_commit(
        &self,
        removed: &[String],
        rel_path: &str,
        extra_rel_paths: &[String],
        message: &str,
    ) -> Result<Option<git2::Oid>, String> {
        let repo = git2::Repository::open(&self.root_dir).map_err(|e| format!("git open: {e}"))?;
        let (sig_name, sig_email) = identity_for_repo(&repo).map_err(|e| e.to_string())?;
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| format!("git signature: {e}"))?;
        let _commit_lock = super::commit_lock::lock_repo_for_commit(repo.path())?;

        // Files an earlier failed commit left behind ride along. A file
        // that is gone is committed as a removal, so a rename whose commit
        // failed is recorded whole.
        let mut removed: Vec<String> = removed.to_vec();
        let mut extras: Vec<String> = extra_rel_paths.to_vec();
        for pending in self.ledger().paths.iter() {
            if pending == rel_path || extras.contains(pending) || removed.contains(pending) {
                continue;
            }
            if self.root_dir.join(pending).symlink_metadata().is_ok() {
                extras.push(pending.clone());
            } else {
                removed.push(pending.clone());
            }
        }
        let removed: Vec<&Path> = removed.iter().map(Path::new).collect();
        let extras: Vec<&Path> = extras.iter().map(Path::new).collect();
        retry_on_git_lock(repo.path(), || {
            stage_and_commit_with(
                &repo,
                &removed,
                Path::new(rel_path),
                None,
                &extras,
                &sig,
                message,
            )
        })
        .map_err(|e| e.message().to_owned())
    }

    /// Remember the files of a failed commit and, once per cause, say so.
    fn note_not_committed(
        &self,
        removed: &[String],
        rel_path: &str,
        extra_rel_paths: &[String],
        reason: String,
    ) -> CommitOutcome {
        let first_time = {
            let mut ledger = self.ledger();
            ledger.paths.insert(rel_path.to_owned());
            ledger.paths.extend(extra_rel_paths.iter().cloned());
            ledger.paths.extend(removed.iter().cloned());
            ledger.reason = Some(reason.clone());
            ledger.reported.insert(reason.clone())
        };
        let library = self
            .file_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if first_time {
            tracing::warn!(
                target: "signex::library",
                "Saved, not committed: {reason} (library {library}; the next commit of the library includes it)"
            );
        } else {
            tracing::debug!(
                target: "signex::library",
                "saved, still not committed ({library}): {reason}"
            );
        }
        CommitOutcome::NotCommitted { reason }
    }
}
