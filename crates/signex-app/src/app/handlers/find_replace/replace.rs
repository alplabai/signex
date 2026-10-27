//! Writing replacements. Every replace re-scans the live document: the
//! scan's text is never reused.

use iced::Task;
use signex_engine::{Command, replace_commands};

use super::super::super::*;

impl Signex {
    /// The commands replacing the query by the replacement at `matches`, all
    /// on the active sheet, computed from its live document.
    fn active_sheet_replace_commands<'a>(
        &self,
        matches: impl IntoIterator<Item = &'a crate::find_replace::FindMatch>,
    ) -> Vec<Command> {
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        let find = &self.ui_state.find_replace;
        let active = self.document_state.active_path.as_deref();
        matches
            .into_iter()
            // Only this sheet's results: a list left over from another tab
            // must not be applied to the wrong document.
            .filter(|hit| hit.sheet.as_deref() == active)
            .flat_map(|hit| {
                replace_commands(engine.document(), &hit.hit, &find.query, &find.replacement)
            })
            .collect()
    }

    pub(super) fn replace_current_find_match(&mut self) -> Task<Message> {
        let find = &self.ui_state.find_replace;
        let Some(hit) = find
            .selected_index
            .and_then(|index| find.matches.get(index))
        else {
            return Task::none();
        };
        if find.scope == crate::find_replace::FindScope::Project {
            let hit = hit.clone();
            return self.start_project_replace(vec![hit]);
        }
        let commands = self.active_sheet_replace_commands([hit]);
        self.apply_engine_commands(commands, true, true);
        self.refresh_find_matches()
    }

    pub(super) fn replace_all_find_matches(&mut self) -> Task<Message> {
        if self.ui_state.find_replace.matches.is_empty() {
            return Task::none();
        }
        if self.ui_state.find_replace.scope == crate::find_replace::FindScope::Project {
            let matches = self.ui_state.find_replace.matches.clone();
            return self.start_project_replace(matches);
        }
        let commands =
            self.active_sheet_replace_commands(self.ui_state.find_replace.matches.iter());
        self.apply_engine_commands(commands, true, true);
        self.refresh_find_matches()
    }
}
