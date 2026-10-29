//! Library Browser inline cell edit: re-hash the row with the new cell
//! value and write it back through the library adapter.

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::writes::{RowEdit, RowPatch, WriteFailure, WriteKind, row_edit_task};

impl Signex {
    /// Commit a per-cell inline edit to the row. Re-hashes + persists.
    pub(in crate::app::dispatch::library) fn handle_browser_cell_commit(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        // The buffer stays until the write has landed: a failed or refused
        // commit keeps what the user typed, and typing on during the write
        // is not mistaken for the saved text (#893).
        let key = (row_id, column.clone());
        let buf = match self.library.library_browsers.get(&library_path) {
            Some(state) => match state.cell_edit.get(&key) {
                Some(buf) => buf.clone(),
                None => return Task::none(),
            },
            None => return Task::none(),
        };
        // Read the current row from the cache, mutate, re-hash, save.
        let mut row = match self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser cell commit: row not found in cache"
                );
                return Task::none();
            }
        };
        match column.as_str() {
            "internal_pn" => {
                row.internal_pn = signex_library::InternalPn::new(buf.trim());
            }
            "manufacturer" => {
                row.primary_mpn.manufacturer = buf.clone();
            }
            "mpn" => {
                row.primary_mpn.mpn = buf.clone();
            }
            other if other.starts_with("parameters.") => {
                let key = &other["parameters.".len()..];
                // Preserve unit AND type on commit by reading the
                // existing value. A typed cell that cannot read the
                // buffer refuses the commit rather than retyping the
                // parameter to text and dropping its unit (#599).
                let in_slot = param_commit::is_measurement_slot(
                    self.library.row_template(&library_path, row.class.as_str()),
                    key,
                );
                match param_commit::param_value_for_commit(row.parameters.get(key), &buf, in_slot) {
                    Ok(new_value) => {
                        row.parameters.insert(key.to_string(), new_value);
                    }
                    Err(refusal) => {
                        // Field order is load-bearing: the Messages
                        // panel compacts a record to 160 characters
                        // (`diagnostics::compact_message`), so the
                        // diagnosis goes first and the long, low-value
                        // fields (table, row, path) go last where a
                        // truncation costs least.
                        tracing::warn!(
                            target: "signex::library",
                            parameter = %key,
                            typed = %buf,
                            kept = %refusal.kept,
                            expected = %refusal.expected,
                            table = %table,
                            row = %row_id,
                            path = %library_path.display(),
                            "browser cell commit refused; the stored value is kept"
                        );
                        // This handler does not run `finish_update`, so
                        // republish here or the refusal sits in the ring
                        // buffer until some unrelated later message
                        // happens to flush it.
                        self.sync_diagnostics_panel_ctx();
                        return Task::none();
                    }
                }
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    column = %column,
                    "browser cell commit: unknown column"
                );
                return Task::none();
            }
        }
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser cell commit: hash failed; the buffer is kept"
                );
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        }
        let edited_column = column.clone();
        let kind = WriteKind::CellSaved {
            table: table.clone(),
            row_id,
            column,
            typed: buf,
            saved: Box::new(row.clone()),
        };
        let edited = row.clone();
        let patch: RowPatch = Box::new(move |stored| apply_column(&edited_column, &edited, stored));
        let edit = RowEdit {
            table,
            row_id,
            row,
            patch: Some(patch),
            message: "edit cell",
        };
        row_edit_task(&mut self.library, &library_path, kind, edit)
    }

    /// The cell's row write finished.
    pub(in crate::app::dispatch::library) fn cell_saved(
        &mut self,
        library_path: std::path::PathBuf,
        cell: SavedCell,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        let SavedCell {
            table,
            row_id,
            column,
            typed,
            saved,
        } = cell;
        let key = (row_id, column.clone());
        let revision = match outcome {
            Ok(revision) => revision,
            Err(failure) => {
                crate::diagnostics::log_warning(format!(
                    "Cell {column} of row {row_id} was not saved: {} (table {table}, {})",
                    failure.reason,
                    library_path.display()
                ));
                // A refused write leaves the cache on the stale revision, so
                // every retry would be refused again: re-read the rows. The
                // buffer is still there for the retry.
                let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
                // No `finish_update` here: republish so the refusal shows now.
                self.sync_diagnostics_panel_ctx();
                return refresh;
            }
        };
        // Done with the typed text, unless the user went on typing in the
        // cell while the write ran.
        if let Some(state) = self.library.library_browsers.get_mut(&library_path)
            && state.cell_edit.get(&key) == Some(&typed)
        {
            state.cell_edit.remove(&key);
        }
        // The re-read lands later; show the saved row (new revision
        // included) right away so the cell does not flash its old value and
        // a second commit starts from the saved revision.
        let edited = *saved;
        self.library
            .patch_cached_row_with(&library_path, &table, row_id, revision, |cached| {
                apply_column(&column, &edited, cached)
            });
        self.refresh_library_task(&library_path, RefreshFollowUp::CleanEditors(vec![row_id]))
    }
}

/// What a saved cell carries from its [`WriteKind::CellSaved`].
pub(in crate::app::dispatch::library) struct SavedCell {
    pub table: String,
    pub row_id: RowId,
    pub column: String,
    pub typed: String,
    pub saved: Box<signex_library::ComponentRow>,
}

/// Copy the field `column` names from the edited row `from` into `to`.
fn apply_column(
    column: &str,
    from: &signex_library::ComponentRow,
    to: &mut signex_library::ComponentRow,
) {
    match column {
        "internal_pn" => to.internal_pn = from.internal_pn.clone(),
        "manufacturer" => to.primary_mpn.manufacturer = from.primary_mpn.manufacturer.clone(),
        "mpn" => to.primary_mpn.mpn = from.primary_mpn.mpn.clone(),
        other => {
            if let Some(key) = other.strip_prefix("parameters.")
                && let Some(value) = from.parameters.get(key)
            {
                to.parameters.insert(key.to_string(), value.clone());
            }
        }
    }
}
