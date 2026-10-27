//! Find / Replace: the message router. Scanning is in `scan`, writing in
//! `replace`, result selection in `navigate`.

mod navigate;
mod project_scan;
mod replace;
mod scan;

#[cfg(test)]
mod tests;

use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_find_replace_open_requested(
        &mut self,
        replace_mode: bool,
    ) -> Task<Message> {
        self.ui_state.find_replace.open = true;
        self.ui_state.find_replace.replace_mode = replace_mode;
        self.interaction_state.context_menu = None;
        self.ui_state.panel_list_open = false;
        self.ui_state.find_replace.skips_reported.clear();
        self.refresh_find_matches()
    }

    pub(crate) fn handle_find_replace_message(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;

        match msg {
            FindReplaceMsg::Close => {
                self.ui_state.find_replace.open = false;
                // A scan that lands after Close has nobody to show it to.
                self.ui_state.find_replace.scan_generation += 1;
                self.ui_state.find_replace.scanning = false;
            }
            FindReplaceMsg::QueryChanged(query) => {
                self.ui_state.find_replace.query = query;
                return self.refresh_find_matches();
            }
            FindReplaceMsg::ScopeChanged(scope) => {
                let find = &mut self.ui_state.find_replace;
                if find.scope != scope {
                    find.scope = scope;
                    find.skips_reported.clear();
                    find.skipped_note = None;
                    find.matches.clear();
                    find.selected_index = None;
                }
                return self.refresh_find_matches();
            }
            FindReplaceMsg::ProjectScanFinished { generation, result } => {
                self.apply_project_scan(generation, &result);
            }
            FindReplaceMsg::ReplacementChanged(value) => {
                self.ui_state.find_replace.replacement = value;
            }
            FindReplaceMsg::SelectResult(index) => self.select_find_result(index),
            FindReplaceMsg::ReplaceCurrent => return self.replace_current_find_match(),
            FindReplaceMsg::ReplaceAll => return self.replace_all_find_matches(),
        }

        Task::none()
    }
}
