//! Find Similar Objects (#1257): resolving the seed, opening the dialog and
//! selecting the matches. A whole-project search runs off the UI thread in
//! `project_search`.

use iced::Task;
use signex_engine::{describe, find_similar, properties_of};
use signex_types::schematic::SelectedItem;

use super::super::*;
use crate::find_similar::{FindSimilarMsg, FindSimilarState, SimilarScope};

mod project_search;

/// The object a Find Similar opened for: the one under the cursor, else the
/// single selected object. `None` when neither names one object.
pub(crate) fn pick_seed(
    under_cursor: Option<SelectedItem>,
    selected: &[SelectedItem],
) -> Option<SelectedItem> {
    under_cursor.or(match selected {
        [only] => Some(*only),
        _ => None,
    })
}

impl Signex {
    pub(crate) fn handle_find_similar_message(&mut self, msg: FindSimilarMsg) -> Task<Message> {
        // The dialog paints in the main window, so its messages arrive untagged:
        // run them against the sheet it was opened on (#1910).
        if !matches!(msg, FindSimilarMsg::OpenRequested { .. })
            && let Some(origin) = self
                .ui_state
                .find_similar
                .as_ref()
                .filter(|s| !s.board)
                .and_then(|s| s.origin.clone())
            && self.document_state.active_path.as_ref() != Some(&origin)
            && let Some(window) = self.undocked_canvas_window(&origin)
        {
            return self.run_in_window_document(window, |app| app.handle_find_similar_message(msg));
        }
        self.close_find_similar_if_sheet_changed();
        match msg {
            FindSimilarMsg::OpenRequested { at } => {
                self.open_find_similar(at);
                self.recount_find_similar();
            }
            FindSimilarMsg::PropertySet(property, same) => {
                let changed = self.ui_state.find_similar.as_mut().is_some_and(|state| {
                    if same {
                        state.same.insert(property)
                    } else {
                        state.same.remove(&property)
                    }
                });
                if changed {
                    return self.find_similar_criteria_changed();
                }
            }
            FindSimilarMsg::ScopeChosen(scope) => {
                let changed = self.ui_state.find_similar.as_mut().is_some_and(|state| {
                    let changed = state.scope != scope;
                    state.scope = scope;
                    changed
                });
                if changed {
                    return self.find_similar_criteria_changed();
                }
            }
            FindSimilarMsg::ApplyPressed => self.apply_find_similar(),
            FindSimilarMsg::Closed => self.close_find_similar(),
            FindSimilarMsg::ProjectSearched { generation, result } => {
                self.apply_project_search(generation, &result);
            }
        }
        Task::none()
    }

    /// The criteria or the scope changed: count this sheet again and drop
    /// the project count made for the old ones; on a project scope, start
    /// a new one.
    fn find_similar_criteria_changed(&mut self) -> Task<Message> {
        self.recount_find_similar();
        // A count still running answers for the old criteria: its token
        // no longer matches.
        self.ui_state.find_similar_generation += 1;
        let Some(state) = self.ui_state.find_similar.as_mut() else {
            return Task::none();
        };
        state.running = false;
        state.other_sheets = None;
        if state.board || state.scope != SimilarScope::Project {
            return Task::none();
        }
        self.start_project_search().unwrap_or_else(Task::none)
    }

    /// Count what the dialog's criteria match on its sheet (or board): the
    /// number Select N shows.
    fn recount_find_similar(&mut self) {
        let Some(state) = self.ui_state.find_similar.as_ref() else {
            return;
        };
        let criteria = state.criteria();
        let count = if state.board {
            self.active_pcb()
                .map(|board| signex_engine::pcb::similar::find_similar(board, &criteria).len())
        } else {
            self.document_state
                .active_engine()
                .map(|engine| find_similar(engine.document(), &criteria).len())
        };
        if let Some(state) = self.ui_state.find_similar.as_mut() {
            state.sheet_matches = count.unwrap_or(0);
        }
    }

    /// Dismiss the dialog. The search token moves on, so a search still
    /// running finds nothing to apply to, even if the dialog is reopened.
    pub(crate) fn close_find_similar(&mut self) {
        self.ui_state.find_similar = None;
        self.ui_state.find_similar_generation += 1;
    }

    /// Close the dialog when the active sheet is not the one it was opened
    /// on, so a selection never lands on a different sheet. Run on every tab
    /// switch and on every dialog message.
    pub(crate) fn close_find_similar_if_sheet_changed(&mut self) {
        // A board's dialog belongs to the board tab (#2403).
        let moved = self.ui_state.find_similar.as_ref().is_some_and(|s| {
            let shown = if s.board {
                self.active_tab_path()
            } else {
                self.document_state.active_path.clone()
            };
            s.origin != shown
        });
        if moved {
            self.close_find_similar();
            crate::diagnostics::log_info("Find Similar closed: the active sheet changed");
        }
    }

    fn open_find_similar(&mut self, at: Option<(f64, f64)>) {
        if self.has_active_pcb() {
            self.open_board_find_similar();
            return;
        }
        let Some(engine) = self.document_state.active_engine() else {
            crate::diagnostics::log_info("Find Similar works on schematic sheets");
            return;
        };
        let under_cursor = at.or_else(|| self.cursor_world_now()).and_then(|(x, y)| {
            let snapshot = self.active_render_snapshot()?;
            crate::schematic_runtime::hit_test::hit_test_styled(
                snapshot,
                &self.interaction_state.active_canvas().designators,
                x,
                y,
                self.active_wire_widths(),
            )
        });
        let selected = &self.interaction_state.active_canvas().selected;
        let Some(seed) = pick_seed(under_cursor, selected) else {
            crate::diagnostics::log_warning(
                "Find Similar: hover one object or select a single object to use as the seed",
            );
            return;
        };
        let sheet = engine.document();
        let values = properties_of(sheet, &seed);
        if values.is_empty() {
            crate::diagnostics::log_warning(
                "Find Similar: the seed object is no longer on the sheet",
            );
            return;
        }
        self.ui_state.find_similar = Some(FindSimilarState::new(
            describe(sheet, &seed),
            values,
            self.document_state.active_path.clone(),
        ));
    }

    /// Select N: select the matches on this sheet. On a project scope the
    /// other sheets' counts go to the Messages panel with it; Select waits
    /// while they are being counted.
    fn apply_find_similar(&mut self) {
        let Some(state) = self.ui_state.find_similar.as_ref() else {
            return;
        };
        if state.running {
            return;
        }
        if state.board {
            self.select_similar_on_board();
            return;
        }
        let others = (state.scope == SimilarScope::Project)
            .then(|| state.other_sheets.clone())
            .flatten();
        if state.scope == SimilarScope::Project && !self.in_a_project() {
            crate::diagnostics::log_warning(
                "Find Similar: not in a project, so only this sheet was searched",
            );
        }
        self.select_similar_on_this_sheet(others.as_ref());
    }

    /// Select what the dialog's criteria match on the live sheet and close the
    /// dialog. `others` is the project search's per-sheet count, logged with it.
    pub(super) fn select_similar_on_this_sheet(
        &mut self,
        others: Option<&crate::find_similar::ProjectMatches>,
    ) {
        let Some(state) = self.ui_state.find_similar.take() else {
            return;
        };
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let items = find_similar(engine.document(), &state.criteria());
        if items.is_empty() {
            crate::diagnostics::log_warning("Find Similar: nothing matches those properties");
        } else {
            self.replace_selection(items.clone());
            crate::diagnostics::log_info(format!(
                "Find Similar: selected {} on this sheet",
                items.len()
            ));
        }
        if let Some(others) = others {
            self.report_other_sheets(others);
        }
    }
}

#[cfg(test)]
mod tests;
