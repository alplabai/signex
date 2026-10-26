//! Footprint-editor handlers for the #1073 pad-stack offer: restore paste
//! and thermal relief on pre-#1052 sketch pads, or dismiss the offer.

use crate::app::Signex;
use crate::library::editor::footprint::pad_stack_check;

impl Signex {
    /// Apply the one-time restore as a single undo step. The editor is left
    /// dirty; the user still has to save.
    pub(crate) fn fp_editor_restore_pad_stack_defaults(&mut self) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            pad_stack_check::restore_pad_stack_defaults(editor);
        }
        self.refresh_panel_ctx();
        true
    }

    /// Withdraw the offer for this tab. Nothing in the document changes.
    pub(crate) fn fp_editor_dismiss_pad_stack_offer(&mut self) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.legacy_pad_stack_offered = false;
        }
        self.refresh_panel_ctx();
        true
    }
}
