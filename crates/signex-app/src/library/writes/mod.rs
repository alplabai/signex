//! Library mutations off the UI thread (#893 part 1).
//!
//! `update` used to call `adapter.update_row` / `delete_row` /
//! `create_empty_table` ... itself. Each of those takes the library's
//! cross-process write lock (up to [`signex_library::WRITE_LOCK_WAIT`], 5 s,
//! when another Signex process holds it), rewrites a file and makes a git
//! commit, so the whole window froze for the duration.
//!
//! Now `update` snapshots what the write needs, hands it to [`write_task`]
//! and returns. The adapter call runs on the blocking pool, and its result
//! comes back as [`LibraryMessage::WriteFinished`], which `update` applies:
//! refresh the caches, close the form, or show the refusal in Messages.
//!
//! **Ordering (chosen: queue per library, not reject).** Each write takes a
//! numbered [`gate::Turn`] on its library while `update` still has it, and the
//! worker waits for its number. Rejecting while a write is in flight would
//! drop the second of two quick edits and make the user retype it; queueing
//! keeps both, in order. A queued edit still carries the row revision the
//! user saw, so the adapter refuses it as stale (`RowChanged`) if an earlier
//! write moved that row on: it is reported, never applied over the top.
//!
//! The forms that start a write keep the user's input until the result is
//! in (ADR-0003 D2/D3): an edit made while the write runs stays in the form
//! and is not mistaken for the saved copy.

pub mod gate;
#[cfg(test)]
pub(crate) mod test_support;

pub use gate::{LibraryWrites, OwnRevisions, Turn, WriteGate};

use std::path::{Path, PathBuf};

use iced::Task;
use signex_library::{ComponentRow, LibraryAdapter, LibraryError, PrimitiveKind, RowId};

use super::messages::LibraryMessage;
use super::state::{EditorAddress, LibraryState};
use crate::app::Message;

/// How long a library close waits for the writes of that library.
pub const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
/// How long quit waits for the writes still running.
pub const QUIT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What the completion handler does once the write is over.
#[derive(Debug, Clone)]
pub enum WriteKind {
    /// Browser: a confirmed Delete Row.
    RowDeleted { table: String, row_id: RowId },
    /// A draft row appended to a table.
    RowCreated {
        table: String,
        row_id: RowId,
        internal_pn: String,
        /// "symbol" / "footprint" the new row has nothing bound for.
        unbound: Vec<String>,
        then: AfterRowCreated,
    },
    /// Browser: the Edit Component Details modal's Save.
    ModalRowSaved { address: EditorAddress },
    /// Browser: an inline cell edit.
    CellSaved {
        table: String,
        row_id: RowId,
        column: String,
        /// What was typed into the cell.
        typed: String,
        /// The row that was written.
        saved: Box<ComponentRow>,
    },
    /// Browser: `+ Add Table`.
    TableCreated { name: String },
    /// Browser: rename a table.
    TableRenamed { old: String, new: String },
    /// Browser: delete an empty table.
    TableDeleted { name: String },
    /// A primitive bound to a row picked in the browser.
    BrowserRowBound {
        address: EditorAddress,
        kind: PrimitiveKind,
    },
    /// A primitive bound to the row of an open Component Preview.
    PreviewRowBound {
        address: EditorAddress,
        kind: PrimitiveKind,
    },
    /// A Component Preview's Save.
    PreviewRowSaved {
        address: EditorAddress,
        /// The editor's save generation, so a result for an editor closed
        /// and reopened meanwhile settles nothing.
        generation: u64,
        saved: Box<PreviewRowSave>,
    },
}

/// What a created row does for the user afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterRowCreated {
    /// Select it in the browser tab that added it.
    SelectInBrowser,
    /// Open its Component Preview.
    OpenPreview,
}

/// What a Component Preview's Save wrote, for settling the editor.
#[derive(Debug, Clone)]
pub struct PreviewRowSave {
    /// The row as written.
    pub row: ComponentRow,
    /// Hashes of the staged PDFs the row pins; spent once it is stored.
    pub saved_hashes: Vec<String>,
}

/// Why a write failed, in a form a message can carry.
#[derive(Debug, Clone)]
pub struct WriteFailure {
    pub reason: String,
    /// The row changed since the caller read it.
    pub row_changed: bool,
}

impl WriteFailure {
    pub fn of(error: &LibraryError) -> Self {
        Self {
            reason: error.to_string(),
            row_changed: matches!(error, LibraryError::RowChanged { .. }),
        }
    }

    fn other(reason: String) -> Self {
        Self {
            reason,
            row_changed: false,
        }
    }
}

/// The result of one library write: the new row revision, when the write
/// was a row save that reports one.
pub type WriteOutcome = Result<Option<u64>, WriteFailure>;

/// A finished write.
#[derive(Debug, Clone)]
pub struct WriteDone {
    pub library_path: PathBuf,
    pub kind: WriteKind,
    pub outcome: WriteOutcome,
}

/// Run `write` against the library at `root` on the blocking pool, after
/// every earlier write of that library, and report as
/// [`LibraryMessage::WriteFinished`].
///
/// A library that is not mounted reports its failure at once.
pub fn write_task<F>(
    state: &mut LibraryState,
    root: &Path,
    kind: WriteKind,
    write: F,
) -> Task<Message>
where
    F: FnOnce(&dyn LibraryAdapter) -> Result<Option<u64>, LibraryError> + Send + 'static,
{
    spawn(state, root, kind, move |adapter, _turn| write(adapter))
}

/// A change to one field group of a row, applied to whatever copy of the
/// row the write finds stored.
pub type RowPatch = Box<dyn FnOnce(&mut ComponentRow) + Send>;

/// A row edit that survives this app's own earlier writes to the row.
pub struct RowEdit {
    pub table: String,
    pub row_id: RowId,
    /// The row as the user's copy has it, with the change applied: written
    /// as it is when nothing of this app's own wrote the row meanwhile.
    pub row: ComponentRow,
    /// Just the change. When an earlier write of this app moved the row on
    /// since the user's copy was read, it is applied on top of the row that
    /// write produced. `None` for an edit that replaces the whole row.
    pub patch: Option<RowPatch>,
    pub message: &'static str,
}

/// [`write_task`] for a row edit; see [`RowEdit`].
pub fn row_edit_task(
    state: &mut LibraryState,
    root: &Path,
    kind: WriteKind,
    edit: RowEdit,
) -> Task<Message> {
    spawn(state, root, kind, move |adapter, turn| {
        let own = turn.own_revisions();
        let RowEdit {
            table,
            row_id,
            row,
            patch,
            message,
        } = edit;
        let mut to_write = row;
        if let Some(patch) = patch
            && let Some(own_revision) = own.of(row_id)
            && let Ok(mut stored) = adapter.read_row(&table, row_id)
            && stored.server_revision == own_revision
            && stored.server_revision != to_write.server_revision
        {
            // The row is exactly as this app last wrote it: nobody else has
            // touched it, so the edit goes on top of that copy.
            patch(&mut stored);
            stored.content_hash = signex_library::hash_row_content(&stored)
                .map_err(|e| LibraryError::Backend(format!("hash row content: {e}")))?;
            to_write = stored;
        }
        let revision = adapter.update_row(&table, to_write, message)?;
        own.record(row_id, revision);
        Ok(revision)
    })
}

fn spawn<F>(state: &mut LibraryState, root: &Path, kind: WriteKind, run: F) -> Task<Message>
where
    F: FnOnce(&dyn LibraryAdapter, &Turn) -> Result<Option<u64>, LibraryError> + Send + 'static,
{
    let path = root.to_path_buf();
    let adapter = state
        .library_at(root)
        .and_then(|lib| state.set.get_shared(lib.library_id));
    let Some(adapter) = adapter else {
        let error = LibraryError::NotFound(root.display().to_string());
        return Task::done(finished(path, kind, Err(WriteFailure::of(&error))));
    };
    let turn = state.writes.turn(root);
    let done_path = path.clone();
    let done_kind = kind.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                turn.wait();
                run(adapter.as_ref(), &turn).map_err(|e| WriteFailure::of(&e))
            })
            .await
            .unwrap_or_else(|join_error| {
                Err(WriteFailure::other(format!(
                    "the library write task failed: {join_error}"
                )))
            })
        },
        move |outcome| finished(done_path.clone(), done_kind.clone(), outcome),
    )
}

fn finished(library_path: PathBuf, kind: WriteKind, outcome: WriteOutcome) -> Message {
    Message::Library(LibraryMessage::WriteFinished(Box::new(WriteDone {
        library_path,
        kind,
        outcome,
    })))
}
