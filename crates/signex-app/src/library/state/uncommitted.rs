//! Libraries with saved changes no git commit holds yet (#858).
//!
//! The adapter keeps the truth (`LibraryAdapter::uncommitted_changes`); this
//! is the copy `view` reads, because `view` must not call the adapter. It is
//! refreshed after every update (see `sync_view_caches`), so a save from any
//! surface shows up on the next frame, and the "Commit now" retry runs off
//! the update thread like every other git write.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{CommitOutcome, LibraryAdapter, UncommittedChanges};

use super::LibraryState;

/// Per-library "saved, not committed" state, keyed by the `.snxlib` path.
#[derive(Debug, Default)]
pub struct UncommittedState {
    pending: HashMap<PathBuf, UncommittedChanges>,
    /// Libraries with a "Commit now" running, so a second press is ignored.
    committing: HashSet<PathBuf>,
}

/// What a finished "Commit now" says, for the Messages panel.
#[derive(Debug, PartialEq, Eq)]
pub enum CommitNowReport {
    Committed,
    NothingToCommit,
    StillNotCommitted(String),
    Failed(String),
}

impl LibraryState {
    /// The saved-but-uncommitted changes of the library at `root`.
    pub fn uncommitted_in(&self, root: &Path) -> Option<&UncommittedChanges> {
        self.uncommitted.pending.get(root)
    }

    /// A "Commit now" of the library at `root` is running.
    pub fn is_committing(&self, root: &Path) -> bool {
        self.uncommitted.committing.contains(root)
    }

    /// The library roots with a "Commit now" running.
    pub fn committing_roots(&self) -> impl Iterator<Item = &Path> {
        self.uncommitted.committing.iter().map(PathBuf::as_path)
    }

    /// Re-read every mounted library's uncommitted state from its adapter,
    /// and report in Messages each pending file a commit had to leave out
    /// (#1729). One short lock per library; no IO.
    pub fn sync_uncommitted(&mut self) {
        let mut pending = HashMap::new();
        for lib in &self.open_libraries {
            let Some(adapter) = self.set.get(lib.library_id) else {
                continue;
            };
            crate::library::held_back::report_held_back(&lib.display_name, adapter);
            if let Some(changes) = adapter.uncommitted_changes() {
                pending.insert(lib.root.clone(), changes);
            }
        }
        if pending != self.uncommitted.pending {
            self.uncommitted.pending = pending;
        }
        let open: HashSet<&PathBuf> = self.open_libraries.iter().map(|l| &l.root).collect();
        self.uncommitted
            .committing
            .retain(|root| open.contains(root));
    }

    /// Start a "Commit now": the adapter to run it on, or `None` when the
    /// library has nothing pending, is unmounted or is already committing.
    pub fn begin_commit_pending(&mut self, root: &Path) -> Option<Arc<dyn LibraryAdapter>> {
        if self.uncommitted.committing.contains(root)
            || !self.uncommitted.pending.contains_key(root)
        {
            return None;
        }
        let library_id = self.library_at(root)?.library_id;
        let adapter = self.set.get_shared(library_id)?;
        self.uncommitted.committing.insert(root.to_path_buf());
        Some(adapter)
    }

    /// A "Commit now" finished: refresh the state and say what happened.
    pub fn finish_commit_pending(
        &mut self,
        root: &Path,
        result: Result<CommitOutcome, String>,
    ) -> CommitNowReport {
        self.uncommitted.committing.remove(root);
        self.sync_uncommitted();
        match result {
            Ok(CommitOutcome::Committed) => CommitNowReport::Committed,
            Ok(CommitOutcome::NothingToCommit) => CommitNowReport::NothingToCommit,
            Ok(CommitOutcome::NotCommitted { reason }) => {
                CommitNowReport::StillNotCommitted(reason)
            }
            Err(error) => CommitNowReport::Failed(error),
        }
    }
}
