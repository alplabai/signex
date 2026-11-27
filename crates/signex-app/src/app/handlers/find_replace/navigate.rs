//! Selecting a result: select it, and for a click or F3 open its sheet and
//! zoom to it.

use iced::Task;

use super::super::super::*;
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};

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
        self.replace_selection(vec![item]);
    }

    /// Go to result `index`: open its sheet at the placement the hit was read
    /// at, select the item and zoom to it.
    pub(super) fn focus_find_result(&mut self, index: usize) -> Task<Message> {
        if let Some(task) = self.refresh_stale_board_find() {
            return task;
        }
        // A board result selects and frames on the board (#2403).
        if self.focus_board_find_result(index) {
            return Task::none();
        }
        self.ui_state.find_replace.selected_index = Some(index);
        let Some(found) = self.ui_state.find_replace.matches.get(index).cloned() else {
            return Task::none();
        };
        let position = found.hit.position;
        // A hit inside a rich text note opens its editor at the match once
        // the note shows (#1114, mockup row 203); a plain note's is dropped.
        if found.hit.item.kind == signex_types::schematic::SelectedKind::TextNote {
            let at = (self.ui_state.find_replace.find_query().ok())
                .and_then(|query| query.ranges(&found.hit.text).first().map(|r| r.start))
                .unwrap_or(0);
            self.open_rich_note_at_search_hit(found.hit.item.uuid, at);
        }
        if let Some(sheet) = found.sheet.clone() {
            let held = self.resolve_held_spelling(sheet.clone());
            if self.document_state.active_path.as_deref() != Some(held.as_path()) {
                let placement = found.hit.placement.clone().or_else(|| {
                    let hierarchy = self.document_view.panel_ctx.navigator.hierarchy.as_ref()?;
                    crate::app::state::default_placement(hierarchy.placements_of(&held))
                });
                // The jump lands when the sheet is showing: at once for an
                // open sheet, after a worker read otherwise.
                let then = AfterOpen::FocusAt {
                    x: position.x,
                    y: position.y,
                    item: Some(found.hit.item),
                };
                let open = SheetOpen::at(placement).then(then);
                // A window cannot switch sheets inside its swap: the open is
                // queued to the main window, as a descent is.
                if self.interaction_state.acting_window.is_some() {
                    return Task::done(Message::Project(ProjectMsg::SheetOpenRequested {
                        path: sheet,
                        open: Box::new(open),
                    }));
                }
                return self.open_sheet(sheet, open);
            }
        }
        self.handle_focus_at(position.x, position.y, Some(found.hit.item))
    }

    /// Enter steps to the next result and Shift+Enter to the previous one
    /// while the dialog is open in Find mode, for a key in the window it
    /// paints in (`in_dialog`): the footer's Next and Previous (mockup row
    /// 67). `None` for every other key.
    pub(crate) fn find_step_key(
        &self,
        in_dialog: bool,
        event: &iced::keyboard::Event,
    ) -> Option<Message> {
        use crate::find_replace::FindReplaceMsg;
        use iced::keyboard::{Event, Key, Modifiers, key::Named};
        let find = &self.ui_state.find_replace;
        if !in_dialog || !find.open || find.replace_mode {
            return None;
        }
        let Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        if !matches!(key.as_ref(), Key::Named(Named::Enter)) {
            return None;
        }
        let step = if modifiers.is_empty() {
            FindReplaceMsg::NextRequested
        } else if *modifiers == Modifiers::SHIFT {
            FindReplaceMsg::PreviousRequested
        } else {
            return None;
        };
        Some(Message::FindReplaceMsg(step))
    }

    /// Find next / previous: step through the results, wrapping at the ends.
    /// Sheet scope re-reads the sheet first, so the list is never behind it.
    pub(super) fn step_find_result(&mut self, forward: bool) -> Task<Message> {
        if self.step_board_find_result(forward) {
            return Task::none();
        }
        if self.ui_state.find_replace.scope == crate::find_replace::FindScope::Sheet {
            self.refresh_find_matches_now();
        }
        let find = &self.ui_state.find_replace;
        let count = find.matches.len();
        if count == 0 {
            crate::diagnostics::log_info(format!(
                "Find: there are no results; search with {} first.",
                crate::keymap::shown("Mod+F")
            ));
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
