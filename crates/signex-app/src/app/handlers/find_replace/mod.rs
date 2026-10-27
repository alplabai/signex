//! Find / Replace: the message router. Scanning is in `scan`, writing in
//! `replace`, result selection in `navigate`.

mod navigate;
mod replace;
mod scan;

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
        self.refresh_find_matches();
        Task::none()
    }

    pub(crate) fn handle_find_replace_message(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;

        match msg {
            FindReplaceMsg::Close => {
                self.ui_state.find_replace.open = false;
            }
            FindReplaceMsg::QueryChanged(query) => {
                self.ui_state.find_replace.query = query;
                self.refresh_find_matches();
            }
            FindReplaceMsg::ReplacementChanged(value) => {
                self.ui_state.find_replace.replacement = value;
            }
            FindReplaceMsg::SelectResult(index) => self.select_find_result(index),
            FindReplaceMsg::ReplaceCurrent => self.replace_current_find_match(),
            FindReplaceMsg::ReplaceAll => self.replace_all_find_matches(),
        }

        Task::none()
    }
}
