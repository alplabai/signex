//! Selecting a result: select it, and for a click or F3 open its sheet and
//! zoom to it.

use iced::Task;

use super::super::super::*;

impl Signex {
    /// Make result `index` the current one and select it on the canvas,
    /// without moving the view (a refresh must not jump around while typing).
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

    /// Go to result `index`: open its sheet at the placement the hit was read
    /// at, select the item and zoom to it.
    pub(super) fn focus_find_result(&mut self, index: usize) -> Task<Message> {
        self.ui_state.find_replace.selected_index = Some(index);
        let Some(found) = self.ui_state.find_replace.matches.get(index).cloned() else {
            return Task::none();
        };
        if let Some(sheet) = found.sheet.clone() {
            let held = self.resolve_held_spelling(sheet.clone());
            if self.document_state.active_path.as_deref() != Some(held.as_path()) {
                let placement = found.hit.placement.clone().or_else(|| {
                    let hierarchy = self
                        .document_state
                        .panel_ctx
                        .navigator
                        .current_hierarchy()?;
                    crate::app::state::default_placement(hierarchy.placements_of(&held))
                });
                self.open_sheet_with_placement(sheet.clone(), placement);
                let opened = self.resolve_held_spelling(sheet.clone());
                if self.document_state.active_path.as_deref() != Some(opened.as_path()) {
                    crate::diagnostics::log_warning(format!(
                        "Find: could not open {} to show the result.",
                        sheet.display()
                    ));
                    return Task::none();
                }
            }
        }
        let position = found.hit.position;
        self.handle_focus_at(position.x, position.y, Some(found.hit.item))
    }

    /// Find next / previous: step through the results, wrapping at the ends.
    /// Sheet scope re-reads the sheet first, so the list is never behind it.
    pub(super) fn step_find_result(&mut self, forward: bool) -> Task<Message> {
        if self.ui_state.find_replace.scope == crate::find_replace::FindScope::Sheet {
            self.refresh_find_matches_now();
        }
        let find = &self.ui_state.find_replace;
        let count = find.matches.len();
        if count == 0 {
            crate::diagnostics::log_info("Find: there are no results; search with Ctrl+F first.");
            return Task::none();
        }
        let next = match (find.selected_index, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(i), true) => (i + 1) % count,
            (Some(i), false) => (i + count - 1) % count,
        };
        self.focus_find_result(next)
    }
}
