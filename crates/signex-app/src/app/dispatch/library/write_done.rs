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
        } = done;
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
                saved,
            } => self.preview_row_saved(address, generation, *saved, outcome),
        }
    }

    /// Quit path: wait (bounded) for the library writes still running on
    /// workers, then run `next`. A write is cut off by the exit between its
    /// file rewrite and its commit, which leaves `.git/index.lock` behind
    /// and the change uncommitted.
    pub(crate) fn wait_for_library_writes_then(&mut self, next: Task<Message>) -> Task<Message> {
        let gates = self.library.writes.busy_gates();
        if gates.is_empty() {
            return next;
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let deadline = std::time::Instant::now() + QUIT_WAIT;
                    gates.iter().all(|gate| {
                        gate.wait_idle(
                            deadline.saturating_duration_since(std::time::Instant::now()),
                        )
                    })
                })
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
}
