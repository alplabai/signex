//! Footprint Library panel handlers — the methods behind the
//! `FpLibrary*` dock-panel messages that manage the *envelope* of
//! internal footprints on the active `.snxfpt` editor (open sibling,
//! select / add / delete / edit / place an internal footprint). The
//! dispatcher in `mod.rs` routes these panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use iced::Task;

use super::*;

impl Signex {
    pub(super) fn handle_fp_library_open_sibling(
        &mut self,
        sibling_path: &std::path::Path,
    ) -> bool {
        // v0.14.2 — open the sibling .snxfpt as a new tab
        // (or activate an existing tab if it's already open)
        // via the existing primitive-open flow.
        let _ = self.handle_open_primitive(sibling_path.to_path_buf());
        self.refresh_panel_ctx();
        true
    }

    // v0.18.8 — Footprint Library panel internal-row select.
    // Stores `panel_selected_idx` on the active footprint
    // editor so the row tints + button row gates correctly.
    // Independent of `active_idx`: only the Edit button (or
    // a double-click hook later) promotes selection to active.
    pub(super) fn handle_fp_library_select_internal(&mut self, idx: &usize) -> bool {
        if let Some(path) = self.active_footprint_editor_path() {
            if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                let last = editor.file.footprints.len().saturating_sub(1);
                editor.panel_selected_idx = Some((*idx).min(last));
            }
            self.refresh_panel_ctx();
        }
        true
    }

    // v0.18.8 — `+ Add` button. Routes through the existing
    // `FootprintAddNewSibling` dispatcher which appends an
    // empty Footprint and switches `active_idx` onto it.
    pub(super) fn handle_fp_library_add_internal(&mut self) -> Task<Message> {
        let mut follow = Task::none();
        if let Some(path) = self.active_footprint_editor_path() {
            follow = self.update(Message::Library(
                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: crate::library::messages::PrimitiveEdit::Footprint(
                        crate::library::messages::FootprintEditorMsg::AddNewSibling,
                    ),
                },
            ));
            if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                // Mirror the panel selection onto the just-
                // added sibling so Delete/Edit immediately
                // operate on it.
                editor.panel_selected_idx = Some(editor.active_idx);
            }
            self.refresh_panel_ctx();
        }
        follow
    }

    // v0.18.8 — `Delete` button. Removes the selected
    // footprint from the envelope. Refuses to remove the
    // last remaining footprint (an empty FootprintFile would
    // fail to load on next open).
    pub(super) fn handle_fp_library_delete_internal(&mut self, idx: &usize) -> bool {
        if let Some(path) = self.active_footprint_editor_path() {
            if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                let last = editor.file.footprints.len();
                if last > 1 && *idx < last {
                    editor.file.footprints.remove(*idx);
                    // Clamp `active_idx` so the canvas keeps
                    // pointing at a valid sibling.
                    if editor.active_idx >= editor.file.footprints.len() {
                        editor.active_idx = editor.file.footprints.len().saturating_sub(1);
                    }
                    // Re-derive canvas-side state from the
                    // (possibly different) active primitive.
                    editor.state =
                        crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                            editor.primitive(),
                        );
                    editor.panel_selected_idx = None;
                    editor.canvas_cache.clear();
                    editor.dirty = true;
                    self.document_state.dirty_paths.insert(path.clone());
                } else if last == 1 {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        "Footprint Library: refused to delete the last footprint in the envelope",
                    );
                }
            }
            self.refresh_panel_ctx();
        }
        true
    }

    // v0.18.8 — `Edit` button. Promotes the panel selection
    // to `active_idx` via the existing
    // `FootprintSelectActiveIdx` dispatcher.
    pub(super) fn handle_fp_library_edit_internal(&mut self, idx: &usize) -> Task<Message> {
        let mut follow = Task::none();
        if let Some(path) = self.active_footprint_editor_path() {
            follow = self.update(Message::Library(
                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: crate::library::messages::PrimitiveEdit::Footprint(
                        crate::library::messages::FootprintEditorMsg::SelectActiveIdx(*idx),
                    ),
                },
            ));
            self.refresh_panel_ctx();
        }
        follow
    }
}
