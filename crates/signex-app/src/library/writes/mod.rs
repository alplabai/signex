//! Library mutations off the UI thread (#893): `update` snapshots the
//! inputs, [`write_task`] / [`row_edit_task`] run the adapter call on the
//! blocking pool, and the result returns as
//! [`LibraryMessage::WriteFinished`]. Design, ordering and the rebase rule:
//! `docs/architecture/library-writes.md`.

pub mod gate;
#[cfg(test)]
pub(crate) mod test_support;

pub use gate::{IdleWaiter, LibraryWrites, OwnRevisions, Turn, WriteGate};

use std::path::{Path, PathBuf};

use iced::Task;
use signex_library::{ComponentRow, LibraryAdapter, LibraryError, PrimitiveKind, RowId};

use super::messages::LibraryMessage;
use super::state::{EditorAddress, LibraryState};
use crate::app::Message;

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
        /// The Save All this save belongs to, whose settle step needs its
        /// outcome.
        batch: Option<u64>,
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
    /// The row a row edit wrote, as stored (after any rebase), carrying the
    /// revision the write produced.
    pub row: Option<Box<ComponentRow>>,
}

/// What a write hands back besides its outcome.
struct Written {
    revision: Option<u64>,
    row: Option<ComponentRow>,
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
    spawn(state, root, kind, move |adapter, _turn| {
        write(adapter).map(|revision| Written {
            revision,
            row: None,
        })
    })
}

/// A change to one field group of a row, applied to whatever copy of the
/// row the write finds stored.
pub type RowPatch = Box<dyn FnOnce(&mut ComponentRow) + Send>;

/// Where a row edit starts from.
pub enum RowStart {
    /// The row as the user's copy has it, with the change applied: written
    /// as it is when nothing of this app's own wrote the row meanwhile.
    Full(Box<ComponentRow>),
    /// [`Self::Full`] for a Component Preview's Save: when the only write
    /// since the copy was read is a primitive-save cascade of this app, the
    /// copy keeps its edits and takes the cascade's pins and revision
    /// instead of being refused as stale (#1840). Any other change to the
    /// row is still refused by the adapter (#1072).
    FullOverCascade(Box<ComponentRow>),
    /// The stored row, with `patch` applied and the base revision the
    /// user's copy was read at.
    FromStored { base_revision: Option<u64> },
}

/// A row edit that survives this app's own earlier writes to the row.
pub struct RowEdit {
    pub table: String,
    pub row_id: RowId,
    pub start: RowStart,
    /// Just the change. When an earlier write of this app moved the row on
    /// since the user's copy was read, it is applied on top of the row that
    /// write produced. `None` for an edit that replaces the whole row.
    pub patch: Option<RowPatch>,
    /// Library-relative paths of pinned PDFs committed with the row.
    pub assets: Vec<String>,
    pub message: &'static str,
}

/// [`write_task`] for a row edit; see [`RowEdit`]. Every row write of this
/// app goes through here, so the ledger of own revisions stays complete.
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
            start,
            patch,
            assets,
            message,
        } = edit;
        let own_revision = own.of(row_id);
        let hash = |row: &mut ComponentRow| -> Result<(), LibraryError> {
            row.content_hash = signex_library::hash_row_content(row)
                .map_err(|e| LibraryError::Backend(format!("hash row content: {e}")))?;
            Ok(())
        };
        let to_write = match start {
            RowStart::FullOverCascade(row) => {
                let mut to_write = *row;
                if let Ok(stored) = adapter.read_row(&table, row_id)
                    && own.only_cascaded_since(
                        row_id,
                        to_write.server_revision,
                        stored.server_revision,
                    )
                {
                    crate::library::preview_rebase::rebase_onto_cascade(&mut to_write, &stored);
                    hash(&mut to_write)?;
                    crate::diagnostics::log_info(format!(
                        "Row {row_id} was moved by a symbol or footprint save of this session; \
                         your edit was saved on top of it."
                    ));
                }
                to_write
            }
            RowStart::Full(row) => {
                let mut to_write = *row;
                if let Some(patch) = patch
                    && let Some(own_revision) = own_revision
                    && let Ok(mut stored) = adapter.read_row(&table, row_id)
                    && stored.server_revision == own_revision
                    && stored.server_revision != to_write.server_revision
                {
                    // The row is exactly as this app last wrote it: nobody
                    // else has touched it, so the edit goes on top of that
                    // copy.
                    patch(&mut stored);
                    hash(&mut stored)?;
                    to_write = stored;
                }
                to_write
            }
            RowStart::FromStored { base_revision } => {
                let mut stored = adapter.read_row(&table, row_id)?;
                if let Some(patch) = patch {
                    patch(&mut stored);
                }
                // The edit keeps the base the user's copy had, so a row
                // that moved on (even by this app's own write, which the
                // copy does not hold) is refused as stale.
                stored.server_revision = base_revision;
                hash(&mut stored)?;
                stored
            }
        };
        let revision =
            adapter.update_row_with_assets(&table, to_write.clone(), message, &assets)?;
        own.record(row_id, revision);
        let mut written = to_write;
        written.server_revision = revision;
        Ok(Written {
            revision,
            row: Some(written),
        })
    })
}

fn spawn<F>(state: &mut LibraryState, root: &Path, kind: WriteKind, run: F) -> Task<Message>
where
    F: FnOnce(&dyn LibraryAdapter, &Turn) -> Result<Written, LibraryError> + Send + 'static,
{
    let path = root.to_path_buf();
    let adapter = state
        .library_at(root)
        .and_then(|lib| state.set.get_shared(lib.library_id));
    let Some(adapter) = adapter else {
        let error = LibraryError::NotFound(root.display().to_string());
        return Task::done(finished(path, kind, Err(WriteFailure::of(&error)), None));
    };
    let turn = state.writes.turn(root);
    let done_path = path.clone();
    let done_kind = kind.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                turn.wait();
                match run(adapter.as_ref(), &turn) {
                    Ok(written) => (Ok(written.revision), written.row.map(Box::new)),
                    Err(e) => (Err(WriteFailure::of(&e)), None),
                }
            })
            .await
            .unwrap_or_else(|join_error| {
                (
                    Err(WriteFailure::other(format!(
                        "the library write task failed: {join_error}"
                    ))),
                    None,
                )
            })
        },
        move |(outcome, row)| finished(done_path.clone(), done_kind.clone(), outcome, row),
    )
}

fn finished(
    library_path: PathBuf,
    kind: WriteKind,
    outcome: WriteOutcome,
    row: Option<Box<ComponentRow>>,
) -> Message {
    Message::Library(LibraryMessage::WriteFinished(Box::new(WriteDone {
        library_path,
        kind,
        outcome,
        row,
    })))
}
