//! Selecting a result.

use super::super::super::*;

impl Signex {
    /// Make result `index` the current one and select it on the canvas.
    pub(super) fn select_find_result(&mut self, index: usize) {
        self.ui_state.find_replace.selected_index = Some(index);
        let Some(item) = self
            .ui_state
            .find_replace
            .matches
            .get(index)
            .map(|hit| hit.hit.item)
        else {
            return;
        };
        self.select_on_active_canvas(item);
    }

    /// Select `item` on the active canvas, repainting its overlay.
    pub(super) fn select_on_active_canvas(&mut self, item: signex_types::schematic::SelectedItem) {
        self.interaction_state.active_canvas_mut().selected = vec![item];
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        self.update_selection_info();
    }
}
