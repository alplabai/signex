//! "Reload from server" on the Library Browser edit modal: after a save
//! was refused because someone else saved the row first, replace the
//! modal's draft with the stored row and its revision.

use super::*;

impl Signex {
    /// Re-seed the open edit modal of `library_path` from the row the
    /// library stores now, dropping the draft. A row that cannot be read
    /// keeps the draft and says why.
    pub(super) fn reload_edit_modal_from_server(
        &mut self,
        library_path: &std::path::Path,
        address: EditorAddress,
    ) {
        let read = self
            .library
            .library_at(library_path)
            .map(|lib| lib.library_id)
            .and_then(|id| self.library.set.get(id))
            .ok_or_else(|| {
                signex_library::LibraryError::NotFound(library_path.display().to_string())
            })
            .and_then(|adapter| adapter.read_row(&address.table, address.row_id));
        match read {
            Ok(row) => {
                if let Some(state) = self.library.library_browsers.get_mut(library_path) {
                    state.edit_modal = Some(EditRowModalState::new(address, row));
                }
            }
            Err(e) => {
                if let Some(state) = self.library.library_browsers.get_mut(library_path)
                    && let Some(modal) = state.edit_modal.as_mut()
                {
                    modal.reload_confirming = false;
                    modal.error = Some(format!("could not reload the row: {e}"));
                }
            }
        }
    }
}
