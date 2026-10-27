//! Writing replacements. Every replace re-scans the live document: the
//! scan's text is never reused.

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
        matches
            .into_iter()
            .flat_map(|hit| {
                replace_commands(engine.document(), &hit.hit, &find.query, &find.replacement)
            })
            .collect()
    }

    pub(super) fn replace_current_find_match(&mut self) {
        let find = &self.ui_state.find_replace;
        let Some(hit) = find
            .selected_index
            .and_then(|index| find.matches.get(index))
        else {
            return;
        };
        let commands = self.active_sheet_replace_commands([hit]);
        self.apply_engine_commands(commands, true, true);
        self.refresh_find_matches();
    }

    pub(super) fn replace_all_find_matches(&mut self) {
        if self.ui_state.find_replace.matches.is_empty() {
            return;
        }
        let commands =
            self.active_sheet_replace_commands(self.ui_state.find_replace.matches.iter());
        self.apply_engine_commands(commands, true, true);
        self.refresh_find_matches();
    }
}
