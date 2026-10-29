//! The result of a library write that a worker ran (#893): route it to the
//! handler of the mutation that started it, and the quit wait for writes
//! still running.

use iced::Task;

use super::browser::SavedCell;
use super::new_row::CreatedRow;
use super::*;
use crate::library::writes::{QUIT_WAIT, WriteDone, WriteKind};

impl Signex {
    /// A library write finished: apply its outcome to the Model.
    pub(super) fn handle_write_finished(&mut self, done: WriteDone) -> Task<Message> {
        let WriteDone {
            library_path,
            kind,
            outcome,
            row,
        } = done;
        let task = self.apply_write(library_path, kind, outcome, row);
        // A library whose close waited for its writes goes now.
        self.close_libraries_whose_writes_are_done();
        task
    }

    fn apply_write(
        &mut self,
        library_path: std::path::PathBuf,
        kind: WriteKind,
        outcome: crate::library::writes::WriteOutcome,
        row: Option<Box<signex_library::ComponentRow>>,
    ) -> Task<Message> {
        match kind {
            WriteKind::RowDeleted { table, row_id } => {
                self.row_deleted(library_path, table, row_id, outcome)
            }
            WriteKind::RowCreated {
                table,
                row_id,
                internal_pn,
                unbound,
                then,
            } => self.row_created(
                library_path,
                CreatedRow {
                    table,
                    row_id,
                    internal_pn,
                    unbound,
                    then,
                },
                outcome,
            ),
            WriteKind::ModalRowSaved { address } => self.modal_row_saved(address, outcome),
            WriteKind::CellSaved {
                table,
                row_id,
                column,
                typed,
                saved,
            } => self.cell_saved(
                library_path,
                SavedCell {
                    table,
                    row_id,
                    column,
                    typed,
                    saved,
                    written: row,
                },
                outcome,
            ),
            WriteKind::TableCreated { name } => self.table_created(library_path, name, outcome),
            WriteKind::TableRenamed { old, new } => {
                self.table_renamed(library_path, old, new, outcome)
            }
            WriteKind::TableDeleted { name } => self.table_deleted(library_path, name, outcome),
            WriteKind::BrowserRowBound { address, .. } => self.browser_row_bound(address, outcome),
            WriteKind::PreviewRowBound { address, .. } => self.preview_row_bound(address, outcome),
            WriteKind::PreviewRowSaved {
                address,
                generation,
                batch,
                saved,
            } => self.preview_row_saved(address, generation, batch, *saved, row, outcome),
        }
    }

    /// Quit path: wait (bounded) for the library writes still running on
    /// workers, then run `next`. A write is cut off by the exit between its
    /// file rewrite and its commit, which leaves `.git/index.lock` behind
    /// and the change uncommitted. The libraries are looked at on the
    /// worker, when the wait starts, so one that got a write while the
    /// saves before this step were awaited is waited for too.
    pub(crate) fn wait_for_library_writes_then(&mut self, next: Task<Message>) -> Task<Message> {
        let waiter = self.library.writes.idle_waiter();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || waiter.wait_all_idle(QUIT_WAIT))
                    .await
                    .unwrap_or(false)
            },
            |idle| {
                if !idle {
                    crate::diagnostics::log_warning(format!(
                        "Quit: a library write was still running after {} s; exiting anyway.",
                        QUIT_WAIT.as_secs()
                    ));
                }
                Message::noop()
            },
        )
        .chain(next)
    }

    /// Unmount the libraries whose close was asked for while a write was
    /// running, now that it has finished.
    pub(super) fn close_libraries_whose_writes_are_done(&mut self) {
        let done = self.library.closes_ready();
        if done.is_empty() {
            return;
        }
        for root in done {
            self.library.close_library(&root);
        }
        self.refresh_panel_ctx();
    }
}
