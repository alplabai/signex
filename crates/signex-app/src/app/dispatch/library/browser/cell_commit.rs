//! Library Browser inline cell edit: re-hash the row with the new cell
//! value and write it back through the library adapter.

use super::*;

impl Signex {
    /// Commit a per-cell inline edit to the row. Re-hashes + persists.
    pub(in crate::app::dispatch::library) fn handle_browser_cell_commit(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        // Drop the buffer eagerly — if the save fails we re-insert below.
        let buf = match self
            .library
            .library_browsers
            .get_mut(&library_path)
            .and_then(|s| s.cell_edit.remove(&(row_id, column.clone())))
        {
            Some(v) => v,
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
                row.internal_pn = signex_library::InternalPn::new(buf.clone());
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
                match param_commit::param_value_for_commit(row.parameters.get(key), &buf) {
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
                        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                            state.cell_edit.insert((row_id, column), buf);
                        }
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
                    "browser cell commit: hash failed; reverting buffer"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), buf);
                }
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        }
        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id);
        let address = EditorAddress::new(library_path.clone(), table.clone(), row_id);
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&table, row, "edit cell"),
            None => Err(signex_library::LibraryError::NotFound(
                library_path.display().to_string(),
            )),
        };
        match result {
            // Another open editor of this row must not keep the old base
            // revision, or its next save would be refused as stale.
            Ok(revision) => self.library.note_row_saved(&address, revision),
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "Cell {column} of row {row_id} was not saved: {e} (table {table}, {})",
                    library_path.display()
                ));
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), buf);
                }
                // No `finish_update` here: republish so the refusal shows now.
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: refresh_components failed"
            );
        }
        Task::none()
    }
}
