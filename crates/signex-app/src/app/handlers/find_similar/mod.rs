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
    under_cursor.or_else(|| match selected {
        [only] => Some(*only),
        _ => None,
    })
}

impl Signex {
    pub(crate) fn handle_find_similar_message(&mut self, msg: FindSimilarMsg) -> Task<Message> {
        match msg {
            FindSimilarMsg::OpenRequested { at } => self.open_find_similar(at),
            FindSimilarMsg::PropertyToggled(property) => {
                if let Some(state) = self.ui_state.find_similar.as_mut()
                    && !state.same.remove(&property)
                {
                    state.same.insert(property);
                }
            }
            FindSimilarMsg::ScopeChosen(scope) => {
                if let Some(state) = self.ui_state.find_similar.as_mut() {
                    state.scope = scope;
                }
            }
            FindSimilarMsg::ApplyPressed => return self.apply_find_similar(),
            FindSimilarMsg::Closed => self.close_find_similar(),
            FindSimilarMsg::ProjectSearched { generation, result } => {
                self.apply_project_search(generation, &result);
            }
        }
        Task::none()
    }

    /// Dismiss the dialog; a search still running is superseded.
    pub(crate) fn close_find_similar(&mut self) {
        if let Some(state) = self.ui_state.find_similar.take() {
            // The generation dies with the state, so a late result finds none.
            drop(state);
        }
    }

    fn open_find_similar(&mut self, at: Option<(f64, f64)>) {
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
                self.ui_state.net_class_styles.styles(),
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
        self.ui_state.find_similar = Some(FindSimilarState::new(describe(sheet, &seed), values));
    }

    /// OK: select the matches on this sheet, or start the project search.
    fn apply_find_similar(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.find_similar.as_ref() else {
            return Task::none();
        };
        if state.running {
            return Task::none();
        }
        if state.scope == SimilarScope::Project
            && let Some(task) = self.start_project_search()
        {
            return task;
        }
        self.select_similar_on_this_sheet(None);
        Task::none()
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
            let canvas = self.interaction_state.active_canvas_mut();
            canvas.selected = items.clone();
            canvas.clear_overlay_cache();
            self.update_selection_info();
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
