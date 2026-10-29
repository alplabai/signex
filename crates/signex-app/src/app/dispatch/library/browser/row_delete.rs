//! Library Browser Delete Row: the confirmed delete runs on a worker (#893).

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::writes::{WriteFailure, WriteKind, write_task};

impl Signex {
    /// Confirm step: delete the row through `adapter.delete_row` on a
    /// worker. The confirm card closes at once (the decision is taken and
    /// its inputs are captured here); the selection and the caches follow
    /// the result.
    pub(in crate::app::dispatch::library) fn handle_browser_delete_row_confirm(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        if self.library.library_at(&library_path).is_none() {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                "browser delete: library not mounted"
            );
            return Task::none();
        }
        // The revision the user saw: the server refuses the delete if the
        // row has been saved by someone else since.
        let base_revision = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .and_then(|row| row.server_revision);
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.delete_confirm = None;
        }
        let kind = WriteKind::RowDeleted {
            table: table.clone(),
            row_id,
        };
        write_task(&mut self.library, &library_path, kind, move |adapter| {
            adapter
                .delete_row(&table, row_id, base_revision, "delete row")
                .map(|()| None)
        })
    }

    /// The delete finished.
    pub(in crate::app::dispatch::library) fn row_deleted(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
        match outcome {
            Ok(_) => {
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser delete: row removed"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    if state.selected_row == Some(row_id) {
                        state.selected_row = None;
                    }
                    // Drop any cached cell-edit buffers for the gone row.
                    state.cell_edit.retain(|(rid, _), _| *rid != row_id);
                }
            }
            Err(failure) => {
                crate::diagnostics::log_warning(format!(
                    "Row {row_id} was not deleted: {} (table {table}, {})",
                    failure.reason,
                    library_path.display()
                ));
                // The cache holds the revision the refusal was based on, so
                // a retry would be refused again: the refresh above re-reads
                // the rows.
                self.sync_diagnostics_panel_ctx();
            }
        }
        refresh
    }
}
