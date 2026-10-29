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
use crate::adapter::{HeldBackFile, UncommittedChanges};

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
    /// Pending files commits left out since the app last asked (#1729).
    held_back: Vec<HeldBackFile>,
}

/// One commit attempt that landed.
struct Landed {
    oid: Option<git2::Oid>,
    /// Ledger files it could not read; they stay pending.
    held: Vec<HeldBackFile>,
}

/// The ledger files a commit records besides the caller's own.
#[derive(Default)]
struct Sweep {
    extras: Vec<String>,
    removed: Vec<String>,
    held: Vec<HeldBackFile>,
}

impl Sweep {
    fn names(&self) -> Vec<String> {
        self.extras.iter().chain(&self.removed).cloned().collect()
    }
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

    /// [`LibraryAdapter::commit_pending`]: commit the ledger on its own. A
    /// ledger whose files are all gone is a commit of removals only.
    pub(super) fn commit_pending_changes(&self, message: &str) -> CommitOutcome {
        let n = self.ledger().paths.len();
        if n == 0 {
            return CommitOutcome::NothingToCommit;
        }
        let fallback = format!("commit {n} saved change(s)");
        self.commit_paths(None, None, &[], message, &fallback)
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
        self.commit_paths(
            removed_rel_path,
            Some(rel_path),
            extra_rel_paths,
            message,
            fallback_message,
        )
    }

    /// [`Self::commit_change_with`] where the main file is optional: `None`
    /// commits only the extras, the ledger and removals.
    fn commit_paths(
        &self,
        removed_rel_path: Option<&str>,
        rel_path: Option<&str>,
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
        // The managed ignore block rides with this commit; on a failure it
        // joins the ledger like the caller's files.
        let mut extras: Vec<String> = extra_rel_paths.to_vec();
        extras.extend(self.merge_ignore_rules_for_commit());
        match self.try_commit(&removed, rel_path, &extras, message) {
            Ok(landed) => {
                self.settle_ledger(landed.held);
                match landed.oid {
                    Some(_) => CommitOutcome::Committed,
                    None => CommitOutcome::NothingToCommit,
                }
            }
            Err(reason) => self.note_not_committed(&removed, rel_path, &extras, reason),
        }
    }

    /// [`LibraryAdapter::take_held_back_files`].
    pub(super) fn take_held_back(&self) -> Vec<HeldBackFile> {
        std::mem::take(&mut self.ledger().held_back)
    }

    /// Record `reason` as reported; `true` the first time.
    pub(super) fn note_reported(&self, reason: &str) -> bool {
        self.ledger().reported.insert(reason.to_owned())
    }

    /// Seed the ledger from the working tree: library-owned files that
    /// differ from HEAD (a save whose commit failed in an earlier session, an
    /// outside edit) are listed as uncommitted, so the banner and the next
    /// commit know about them after a restart. Best effort; a repository
    /// that cannot be read lists nothing.
    pub(super) fn seed_uncommitted_from_working_tree(&self) {
        if !self.root_dir.join(".git").exists() {
            return;
        }
        let found = match self.working_tree_changes() {
            Ok(found) => found,
            Err(e) => {
                tracing::debug!(
                    target: "signex::library",
                    "could not look for uncommitted library files: {e}"
                );
                return;
            }
        };
        if found.is_empty() {
            return;
        }
        let mut ledger = self.ledger();
        ledger.paths.extend(found);
        ledger.reason.get_or_insert_with(|| {
            "library files differ from the last commit (found when the library was opened)"
                .to_owned()
        });
    }

    /// Library-relative paths of library-owned files that differ between
    /// HEAD and the working tree, including untracked and deleted ones.
    fn working_tree_changes(&self) -> Result<BTreeSet<String>, git2::Error> {
        let repo = git2::Repository::open(&self.root_dir)?;
        super::ignore_rules::apply_managed_rules(&repo)?;
        let head_tree = match repo.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e),
        };
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_typechange(true);
        if let Some(name) = self.file_path.file_name().and_then(|n| n.to_str()) {
            opts.pathspec(name);
        }
        for dir in [
            SYMBOLS_DIR,
            FOOTPRINTS_DIR,
            SIMS_DIR,
            crate::datasheet_asset::DATASHEET_ASSET_DIR,
        ] {
            opts.pathspec(dir);
        }
        let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;
        Ok(diff
            .deltas()
            .filter_map(|delta| {
                let file = if delta.status() == git2::Delta::Deleted {
                    delta.old_file()
                } else {
                    delta.new_file()
                };
                file.path().map(|p| p.to_string_lossy().replace('\\', "/"))
            })
            .collect())
    }

    /// The library moved to another directory: the ledger named files of the
    /// old repository, so it is dropped and the new directory is looked at.
    pub(super) fn reset_uncommitted_for_new_root(&self) {
        *self.ledger() = UncommittedLedger::default();
        self.seed_uncommitted_from_working_tree();
    }

    /// A commit landed: only the files it held back stay pending, and each
    /// is queued for the app to report.
    fn settle_ledger(&self, held: Vec<HeldBackFile>) {
        let mut ledger = self.ledger();
        ledger.paths = held.iter().map(|file| file.path.clone()).collect();
        ledger.reason = held
            .first()
            .map(|file| format!("could not read {}: {}", file.path, file.reason));
        ledger.held_back.extend(held);
    }

    /// One attempt, under the in-process git lock. `Err` is the reason.
    fn try_commit(
        &self,
        removed: &[String],
        rel_path: Option<&str>,
        extra_rel_paths: &[String],
        message: &str,
    ) -> Result<Landed, String> {
        let repo = git2::Repository::open(&self.root_dir).map_err(|e| format!("git open: {e}"))?;
        let (sig_name, sig_email) = identity_for_repo(&repo).map_err(|e| e.to_string())?;
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| format!("git signature: {e}"))?;
        let _commit_lock = super::commit_lock::lock_repo_for_commit(repo.path())?;
        super::ignore_rules::apply_managed_rules(&repo).map_err(|e| format!("git ignore: {e}"))?;

        let own = Own {
            removed,
            rel_path,
            extras: extra_rel_paths,
        };
        let sweep = self.plan_sweep(&repo, &own);
        let swept = sweep.names();
        // History stays truthful: the commit says what else it holds.
        let full_message = if swept.is_empty() {
            message.to_owned()
        } else {
            format!("{message}\n\nalso commits pending: {}", swept.join(", "))
        };
        let with_sweep = own.commit(&repo, &sig, &full_message, &sweep);
        match with_sweep {
            Ok(oid) => Ok(Landed {
                oid,
                held: sweep.held,
            }),
            // A pending file failing must not take the caller's change down
            // with it: commit that alone and keep the sweep pending.
            Err(e) if !swept.is_empty() && own.has_changes() => {
                let oid = own
                    .commit(&repo, &sig, message, &Sweep::default())
                    .map_err(|_| e.message().to_owned())?;
                let mut held = sweep.held;
                held.extend(swept.into_iter().map(|path| HeldBackFile {
                    path,
                    reason: e.message().to_owned(),
                }));
                Ok(Landed { oid, held })
            }
            Err(e) => Err(e.message().to_owned()),
        }
    }

    /// The ledger files to record with this commit. Files an earlier failed
    /// commit left behind ride along; a file that is gone is committed as a
    /// removal, so a rename whose commit failed is recorded whole. An
    /// ignored file is dropped from the ledger; one that cannot be read is
    /// held back.
    fn plan_sweep(&self, repo: &git2::Repository, own: &Own<'_>) -> Sweep {
        let mut sweep = Sweep::default();
        for pending in self.ledger().paths.iter() {
            if own.holds(pending) || repo.is_path_ignored(Path::new(pending)).unwrap_or(false) {
                continue;
            }
            let path = self.root_dir.join(pending);
            if path.symlink_metadata().is_err() {
                sweep.removed.push(pending.clone());
                continue;
            }
            match fs::File::open(&path) {
                Ok(_) => sweep.extras.push(pending.clone()),
                Err(e) => sweep.held.push(HeldBackFile {
                    path: pending.clone(),
                    reason: e.to_string(),
                }),
            }
        }
        sweep
    }

    /// Remember the files of a failed commit and, once per cause, say so.
    fn note_not_committed(
        &self,
        removed: &[String],
        rel_path: Option<&str>,
        extra_rel_paths: &[String],
        reason: String,
    ) -> CommitOutcome {
        let first_time = {
            let mut ledger = self.ledger();
            ledger.paths.extend(rel_path.map(str::to_owned));
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

/// The caller's own part of a commit.
struct Own<'a> {
    removed: &'a [String],
    rel_path: Option<&'a str>,
    extras: &'a [String],
}

impl Own<'_> {
    /// Whether the caller asked to record anything of its own.
    fn has_changes(&self) -> bool {
        self.rel_path.is_some() || !self.extras.is_empty() || !self.removed.is_empty()
    }

    fn holds(&self, path: &str) -> bool {
        Some(path) == self.rel_path
            || self.extras.iter().any(|p| p == path)
            || self.removed.iter().any(|p| p == path)
    }

    /// Commit the caller's paths plus `sweep`'s.
    fn commit(
        &self,
        repo: &git2::Repository,
        sig: &git2::Signature<'_>,
        message: &str,
        sweep: &Sweep,
    ) -> Result<Option<git2::Oid>, git2::Error> {
        let removed: Vec<&Path> = self
            .removed
            .iter()
            .chain(&sweep.removed)
            .map(Path::new)
            .collect();
        let extras: Vec<&Path> = self
            .extras
            .iter()
            .chain(&sweep.extras)
            .map(Path::new)
            .collect();
        retry_on_git_lock(repo.path(), || {
            stage_and_commit_with(
                repo,
                &removed,
                self.rel_path.map(Path::new),
                None,
                &extras,
                sig,
                message,
            )
        })
    }
}
