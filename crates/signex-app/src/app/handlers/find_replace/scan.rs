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
        let known_single = active.is_some_and(|path| self.known_single_placement(path));
        DesignatorScope::at(placement, known_single)
    }

    /// Re-run the search for the current scope. Sheet scope answers at once;
    /// project scope returns the off-thread scan.
    pub(super) fn refresh_find_matches(&mut self) -> Task<Message> {
        use crate::find_replace::FindScope;
        // A new search outdates any replace still reading the disk.
        self.cancel_replace_job("the search changed");
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
        let mut hits = self
            .document_state
            .active_engine()
            .map(|engine| signex_engine::scan_sheet(engine.document(), &query, &scope))
            .unwrap_or_default();
        // With no placement known, a designator hit would write the shared
        // reference of a sheet that may be placed more than once (#2021).
        if let Some(why) = path
            .as_deref()
            .filter(|_| scope.placements.is_empty())
            .and_then(|p| self.designator_write_refusal(p))
        {
            let before = hits.len();
            hits.retain(|hit| hit.kind != signex_engine::FindKind::Designator);
            // Once per sheet until the Find state resets, not per keystroke.
            let first_time = path
                .as_ref()
                .is_some_and(|p| self.ui_state.find_replace.skips_reported.insert(p.clone()));
            if hits.len() != before && first_time {
                crate::diagnostics::log_warning(format!(
                    "Find/Replace skipped this sheet's designators: {why}."
                ));
            }
        }
        let matches: Vec<FindMatch> = hits
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
