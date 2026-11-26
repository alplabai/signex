//! The drafts Preferences Save refuses before it commits anything.

use super::super::super::*;

impl Signex {
    /// Whether the snap preview's open pitch label holds text it refuses.
    pub(super) fn snap_label_holds_refused_text(&self) -> bool {
        let ui = &self.ui_state;
        ui.preferences_pages
            .pages_view
            .snap_label_refused(&ui.preferences_draft_fast_grids, &ui.i18n)
    }

    /// Whether Save must stop because a draft cannot be committed: a keymap
    /// trigger that does not parse, a fast grid (#1672) that is refused, or
    /// an open pitch label holding refused text (#2792), which stays open
    /// and outlined. Each refusal is reported where its fields are; the
    /// dialog stays open with the typed text kept, and nothing is committed.
    pub(super) fn refuse_invalid_save_drafts(&mut self) -> bool {
        if self.snap_label_holds_refused_text() {
            self.recompute_preferences_dirty();
            return true;
        }
        let keymap_invalid = self
            .ui_state
            .preferences_keymap_editor
            .has_invalid_trigger_drafts();
        if keymap_invalid {
            self.ui_state.preferences_keymap_status =
                "Fix invalid keyboard shortcuts before saving.".to_string();
        }
        let fast_grid_invalid = self.ui_state.refuse_invalid_fast_grid_drafts();
        // #2868: a PCB Defaults field holding text it refuses shows why.
        let pcb_defaults_invalid = !self.ui_state.preferences_pcb_defaults.refused.is_empty();
        if keymap_invalid || fast_grid_invalid || pcb_defaults_invalid {
            self.recompute_preferences_dirty();
            return true;
        }
        false
    }
}
