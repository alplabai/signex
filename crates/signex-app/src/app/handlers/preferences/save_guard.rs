//! The drafts Preferences Save refuses before it commits anything.

use super::super::super::*;

impl Signex {
    /// Whether Save must stop because a draft cannot be committed: a keymap
    /// trigger that does not parse, or a fast grid (#1672) that is refused.
    /// Each refusal is reported where its fields are; the dialog stays
    /// dirty and open with the typed text kept, and nothing is committed.
    pub(super) fn refuse_invalid_save_drafts(&mut self) -> bool {
        let keymap_invalid = self
            .ui_state
            .preferences_keymap_editor
            .has_invalid_trigger_drafts();
        if keymap_invalid {
            self.ui_state.preferences_keymap_status =
                "Fix invalid keyboard shortcuts before saving.".to_string();
        }
        let fast_grid_invalid = self.ui_state.refuse_invalid_fast_grid_drafts();
        if keymap_invalid || fast_grid_invalid {
            self.recompute_preferences_dirty();
            return true;
        }
        false
    }
}
