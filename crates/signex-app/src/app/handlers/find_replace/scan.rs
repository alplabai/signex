//! Finding matches. Sheet scope reads the active tab's engine document.

use iced::Task;
use signex_engine::DesignatorScope;

use super::super::super::*;
use crate::find_replace::FindMatch;

impl Signex {
    /// The placement whose designators the active tab shows and edits, and
    /// whether the walk knows the sheet is placed at most once (#1200).
    pub(super) fn active_designator_scope(&self) -> DesignatorScope {
        let active = self.document_state.active_path.as_deref();
        let Some(placement) = active.and_then(|path| self.shown_placement(path)) else {
            return DesignatorScope::default();
        };
        let known_single = active.is_some_and(|path| {
            self.document_state
                .panel_ctx
                .navigator
                .hierarchy
                .as_ref()
                .is_some_and(|h| h.is_complete() && h.placements_of(path).len() <= 1)
        });
        DesignatorScope::at(placement, known_single)
    }

    /// Re-run the search for the current scope. Sheet scope answers at once;
    /// project scope returns the off-thread scan.
    pub(super) fn refresh_find_matches(&mut self) -> Task<Message> {
        use crate::find_replace::FindScope;
        match self.ui_state.find_replace.scope {
            FindScope::Sheet => {
                // A project scan still in flight must not land on top.
                self.ui_state.find_replace.scan_generation += 1;
                self.ui_state.find_replace.scanning = false;
                self.refresh_active_sheet_matches();
                Task::none()
            }
            FindScope::Project => self.start_project_scan(),
        }
    }

    /// Sheet-scope refresh, for callers that need the list current right now.
    pub(super) fn refresh_find_matches_now(&mut self) {
        self.refresh_active_sheet_matches();
    }

    fn refresh_active_sheet_matches(&mut self) {
        let query = self.ui_state.find_replace.query.trim().to_string();
        if query.is_empty() {
            self.ui_state.find_replace.matches.clear();
            self.ui_state.find_replace.selected_index = None;
            return;
        }

        let scope = self.active_designator_scope();
        let path = self.document_state.active_path.clone();
        let matches: Vec<FindMatch> = self
            .document_state
            .active_engine()
            .map(|engine| signex_engine::scan_sheet(engine.document(), &query, &scope))
            .unwrap_or_default()
            .into_iter()
            .map(|hit| FindMatch {
                sheet: path.clone(),
                hit,
                placement_label: None,
            })
            .collect();

        self.ui_state.find_replace.selected_index = if matches.is_empty() {
            None
        } else {
            Some(
                self.ui_state
                    .find_replace
                    .selected_index
                    .unwrap_or(0)
                    .min(matches.len() - 1),
            )
        };
        self.ui_state.find_replace.matches = matches;

        if let Some(index) = self.ui_state.find_replace.selected_index {
            self.select_find_result(index);
        }
    }
}
