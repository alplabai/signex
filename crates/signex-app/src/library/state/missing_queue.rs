//! Library Missing dialogs waiting their turn (#1897).
//!
//! One recovery dialog shows at a time. A project that opens with several
//! absent libraries queues the later ones here; closing the open dialog
//! (any way) opens the next.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use super::LibraryState;
use crate::app::ProjectId;
use crate::library::recovery::RecoveryDialog;

/// The queue plus the bookkeeping the open Library Missing dialog needs.
#[derive(Debug, Default)]
pub struct MissingLibraries {
    waiting: VecDeque<(PathBuf, ProjectId)>,
    /// Project whose library the open dialog is about; `None` when the
    /// dialog came from a mount the user asked for.
    open_owner: Option<ProjectId>,
    /// Library a Locate file pick is in flight for.
    locating: Option<PathBuf>,
}

impl MissingLibraries {
    /// Remember which library the Locate pick in flight is for.
    pub fn start_locating(&mut self, path: PathBuf) {
        self.locating = Some(path);
    }

    /// Whether a Locate file pick is in flight.
    pub fn is_locating(&self) -> bool {
        self.locating.is_some()
    }

    /// The library the finished Locate pick was for.
    pub fn take_locating(&mut self) -> Option<PathBuf> {
        self.locating.take()
    }

    /// Number of dialogs still waiting.
    pub fn waiting_len(&self) -> usize {
        self.waiting.len()
    }
}

impl LibraryState {
    /// Ask for a Library Missing dialog for a library of project `owner`:
    /// open it now, or queue it behind the dialog that is open. The same
    /// path is never raised twice.
    pub fn raise_missing_library(&mut self, path: &Path, owner: ProjectId) {
        let already_open = matches!(
            &self.recovery,
            Some(RecoveryDialog::LibraryMissing { path: open }) if open == path
        );
        let queue = &mut self.missing;
        if already_open || queue.waiting.iter().any(|(p, _)| p == path) {
            return;
        }
        if self.recovery.is_some() {
            queue.waiting.push_back((path.to_path_buf(), owner));
        } else {
            queue.open_owner = Some(owner);
            self.recovery = Some(RecoveryDialog::LibraryMissing {
                path: path.to_path_buf(),
            });
        }
    }

    /// The open recovery dialog is done with: close it and open the next
    /// queued one, skipping a library that has been mounted meanwhile.
    pub fn finish_recovery(&mut self) {
        self.recovery = None;
        self.missing.open_owner = None;
        while let Some((path, owner)) = self.missing.waiting.pop_front() {
            if self.library_at(&path).is_some() {
                continue;
            }
            self.missing.open_owner = Some(owner);
            self.recovery = Some(RecoveryDialog::LibraryMissing { path });
            return;
        }
    }

    /// A project closed: its queued dialogs and the open one pass to
    /// `next_owner`, another open project that still lists the library,
    /// and are dropped when there is none.
    pub fn forget_missing_libraries_of(
        &mut self,
        project: ProjectId,
        next_owner: impl Fn(&Path) -> Option<ProjectId>,
    ) {
        let waiting = std::mem::take(&mut self.missing.waiting);
        self.missing.waiting = waiting
            .into_iter()
            .filter_map(|(path, owner)| {
                if owner != project {
                    return Some((path, owner));
                }
                next_owner(&path).map(|next| (path, next))
            })
            .collect();
        if self.missing.open_owner != Some(project) {
            return;
        }
        let open_path = match &self.recovery {
            Some(RecoveryDialog::LibraryMissing { path }) => path.clone(),
            _ => return,
        };
        match next_owner(&open_path) {
            Some(next) => self.missing.open_owner = Some(next),
            None => self.finish_recovery(),
        }
    }
}
