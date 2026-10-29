//! Off-thread library refresh (#1374).
//!
//! `LibraryState::refresh_components` re-reads a library's tables, prices
//! and primitive listings through its adapter — disk IO that must not run
//! inside `update`. [`refresh_task`] runs the same read on the blocking
//! pool and reports back as
//! [`LibraryMessage::RefreshFinished`](super::LibraryMessage::RefreshFinished);
//! `LibraryState::finish_refresh` drops the result when a newer refresh
//! superseded it or the library was closed.
//!
//! The payload is not `Clone` (`LibraryError` is not), so it travels in a
//! one-shot [`RefreshSnapshotCell`], the same shape as
//! [`PreparedMountCell`](super::mount::PreparedMountCell).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use signex_library::{DiskStamp, LibraryError, RowId};

use super::state::{LibrarySnapshot, LibraryState};

/// What the completion handler does besides applying the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshFollowUp {
    /// Nothing more.
    Nothing,
    /// Reload the clean Component Preview editors of these rows from the
    /// library, now that the caches hold the saved revision.
    CleanEditors(Vec<RowId>),
    /// The refresh answers an external edit seen by the poll.
    ExternalEdit {
        /// The file stamp the poll saw.
        stamp: DiskStamp,
        /// The change is the app's own class write, so reloading is quiet.
        own_edit: bool,
    },
    /// A Refresh All Pricing walk finished writing prices.
    PricingSaved,
    /// Scroll the browser to its selected row, which the refresh just
    /// brought into the cache (a freshly added row).
    RevealSelected,
}

/// One-shot carrier for a read snapshot across the `Task::perform`
/// boundary. `take` yields the payload once; a later clone sees `None`.
#[derive(Clone)]
pub struct RefreshSnapshotCell(Arc<Mutex<Option<Result<LibrarySnapshot, LibraryError>>>>);

impl RefreshSnapshotCell {
    pub(crate) fn new(result: Result<LibrarySnapshot, LibraryError>) -> Self {
        Self(Arc::new(Mutex::new(Some(result))))
    }

    /// Take the payload; `None` on a second read.
    pub fn take(&self) -> Option<Result<LibrarySnapshot, LibraryError>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl std::fmt::Debug for RefreshSnapshotCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RefreshSnapshotCell(..)")
    }
}

/// Refresh the library at `root` off the UI thread. A library that is not
/// mounted logs a warning and yields no task.
pub fn refresh_task(
    state: &mut LibraryState,
    root: &Path,
    follow_up: RefreshFollowUp,
) -> iced::Task<crate::app::Message> {
    let start = match state.begin_refresh(root, follow_up) {
        Ok(start) => start,
        Err(error) => {
            crate::diagnostics::log_warning(format!(
                "[library] refresh of {} not started ({error}); its cache may be stale",
                root.display()
            ));
            return iced::Task::none();
        }
    };
    let path: PathBuf = root.to_path_buf();
    let read_path = path.clone();
    let generation = start.generation;
    // Behind every write of this library started so far, so the read sees
    // them all and never a file half-way through a rewrite (#893).
    let turn = state.writes.turn(root);
    iced::Task::perform(
        async move {
            let result = tokio::task::spawn_blocking(move || {
                turn.wait();
                LibrarySnapshot::read(start.adapter.as_ref(), &read_path)
            })
            .await
            .unwrap_or_else(|e| Err(LibraryError::Backend(format!("spawn_blocking: {e}"))));
            RefreshSnapshotCell::new(result)
        },
        move |snapshot| {
            crate::app::Message::Library(super::LibraryMessage::RefreshFinished {
                path: path.clone(),
                generation,
                snapshot,
            })
        },
    )
}
