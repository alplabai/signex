//! Library Browser Edit Component Details: the modal's Save runs on a
//! worker (#893).
//!
//! The modal stays open while the row is written and keeps whatever the
//! user types meanwhile: the result closes it only when the draft is still
//! the copy that was written (ADR-0003 D2/D3).

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::state::SavingSnapshot;
use crate::library::writes::{RowEdit, RowStart, WriteFailure, WriteKind, row_edit_task};

impl Signex {
    /// Save `draft` (the modal's draft after its pending parameter and tag
    /// edits were folded in) and mark the modal as saving.
    pub(super) fn start_modal_row_save(
        &mut self,
        library_path: &std::path::Path,
        address: EditorAddress,
        draft: signex_library::ComponentRow,
    ) -> Task<Message> {
        let mut written = draft.clone();
        match signex_library::hash_row_content(&written) {
            Ok(hash) => written.content_hash = hash,
            Err(e) => {
                if let Some(modal) = self.edit_modal_mut(library_path) {
                    modal.error = Some(format!("hash failed: {e}"));
                }
                return Task::none();
            }
        }
        if let Some(modal) = self.edit_modal_mut(library_path) {
            modal.error = None;
            modal.saving = Some(SavingSnapshot {
                draft: draft.clone(),
                tags_buf: modal.tags_buf.clone(),
                param_buf: modal.param_buf.clone(),
            });
        }
        let table = address.table.clone();
        let kind = WriteKind::ModalRowSaved {
            address: address.clone(),
        };
        let row_id = address.row_id;
        let edit = RowEdit {
            table,
            row_id,
            start: RowStart::Full(Box::new(written)),
            patch: None,
            assets: Vec::new(),
            message: "edit row",
        };
        row_edit_task(&mut self.library, library_path, kind, edit)
    }

    fn edit_modal_mut(
        &mut self,
        library_path: &std::path::Path,
    ) -> Option<&mut crate::library::state::EditRowModalState> {
        self.library
            .library_browsers
            .get_mut(library_path)
            .and_then(|state| state.edit_modal.as_mut())
    }

    /// The modal's row write finished.
    pub(in crate::app::dispatch::library) fn modal_row_saved(
        &mut self,
        address: EditorAddress,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        let path = address.library_path.clone();
        // The modal this write belongs to, if it is still open on that row.
        let open = self
            .edit_modal_mut(&path)
            .filter(|modal| modal.address == address && modal.saving.is_some());
        match outcome {
            Ok(revision) => {
                let mut close = true;
                if let Some(modal) = open {
                    let snapshot = modal.saving.take();
                    let edited_since = snapshot.is_some_and(|s| {
                        s.draft != modal.draft
                            || s.tags_buf != modal.tags_buf
                            || s.param_buf != modal.param_buf
                    });
                    if edited_since {
                        // What was typed since is not stored: the modal
                        // stays, on the new base revision.
                        modal.draft.server_revision = revision;
                        modal.stale_base = false;
                        close = false;
                    }
                }
                if close
                    && let Some(state) = self.library.library_browsers.get_mut(&path)
                    && state
                        .edit_modal
                        .as_ref()
                        .is_some_and(|modal| modal.address == address)
                {
                    state.edit_modal = None;
                }
                self.refresh_library_task(
                    &path,
                    RefreshFollowUp::CleanEditors(vec![address.row_id]),
                )
            }
            Err(failure) => {
                if let Some(modal) = open {
                    modal.saving = None;
                    modal.stale_base = failure.row_changed;
                    modal.error = Some(failure.reason.clone());
                }
                crate::diagnostics::log_warning(format!(
                    "Row {} was not saved: {} ({})",
                    address.row_id,
                    failure.reason,
                    path.display()
                ));
                self.sync_diagnostics_panel_ctx();
                // A refused write leaves the cache on the stale base: read
                // the stored rows again so the grid shows what is on disk.
                self.refresh_library_task(&path, RefreshFollowUp::Nothing)
            }
        }
    }
}
