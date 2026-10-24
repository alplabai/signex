//! Properties-panel Name / File editing for a selected sheet symbol (#794).
//!
//! The text inputs buffer keystrokes; Enter commits one
//! `Command::ModifyChildSheet` through the mutation gateway (undo, dirty,
//! netlist invalidation). A refused edit leaves the document alone and
//! raises an [`ErrorNotice`]. Re-pointing only changes the reference — no
//! file is ever created, moved or renamed here.

use crate::app::state::ErrorNotice;

use super::super::super::*;

impl Signex {
    pub(super) fn type_child_sheet_name(&mut self, id: uuid::Uuid, text: &str) {
        self.document_state.panel_ctx.child_sheet_name_buf = Some((id, text.to_string()));
    }

    pub(super) fn type_child_sheet_file(&mut self, id: uuid::Uuid, text: &str) {
        self.document_state.panel_ctx.child_sheet_file_buf = Some((id, text.to_string()));
    }

    /// Commit the buffered Name for sheet `id`. Nothing typed, nothing done.
    pub(super) fn commit_child_sheet_name(&mut self, id: uuid::Uuid) {
        let Some(name) = take_buffer(&mut self.document_state.panel_ctx.child_sheet_name_buf, id)
        else {
            return;
        };
        if let Some(taken) = self.name_used_by_another_open_sheet(id, name.trim()) {
            self.refuse_child_sheet_edit(format!(
                "Another sheet symbol in this project is already named '{taken}'."
            ));
            return;
        }
        self.apply_child_sheet_edit(id, Some(name), None);
    }

    /// Commit the buffered File reference for sheet `id`.
    pub(super) fn commit_child_sheet_file(&mut self, id: uuid::Uuid) {
        let Some(filename) =
            take_buffer(&mut self.document_state.panel_ctx.child_sheet_file_buf, id)
        else {
            return;
        };
        let trimmed = filename.trim();
        // A blank reference is the engine's to refuse (it has the wording);
        // everything else must stay inside the sheet's folder first, by the
        // same resolver navigation and the netlist use.
        if !trimmed.is_empty() && self.resolve_child_sheet_path(trimmed).is_none() {
            self.refuse_child_sheet_edit(format!(
                "'{trimmed}' points outside the sheet's folder. \
                 A sheet file must live beside, or below, the sheet that references it."
            ));
            return;
        }
        self.apply_child_sheet_edit(id, None, Some(filename));
    }

    fn apply_child_sheet_edit(
        &mut self,
        id: uuid::Uuid,
        name: Option<String>,
        filename: Option<String>,
    ) {
        let (_, refusal) = self.apply_engine_command_reporting(
            signex_engine::Command::ModifyChildSheet {
                sheet_id: id,
                name,
                filename,
            },
            true,
            true,
        );
        if let Some(reason) = refusal {
            self.refuse_child_sheet_edit(reason);
        }
    }

    fn refuse_child_sheet_edit(&mut self, detail: String) {
        self.document_state.error_notice = Some(ErrorNotice::sheet_edit(detail));
    }

    /// The existing name that `name` would collide with in another open
    /// sheet of the active project, if any. The engine only sees its own
    /// document, so project-wide uniqueness is checked here.
    fn name_used_by_another_open_sheet(&self, id: uuid::Uuid, name: &str) -> Option<String> {
        self.document_state
            .engines_in_active_document_project()
            .into_iter()
            .flat_map(|engine| engine.document().child_sheets.iter())
            .find(|c| c.uuid != id && c.name.trim().eq_ignore_ascii_case(name))
            .map(|c| c.name.clone())
    }
}

/// The buffered text for sheet `id`, clearing the buffer either way.
fn take_buffer(buf: &mut Option<(uuid::Uuid, String)>, id: uuid::Uuid) -> Option<String> {
    buf.take()
        .and_then(|(owner, text)| (owner == id).then_some(text))
}
