//! Find / Replace: the message router. Scanning is in `scan`, writing in
//! `replace`, result selection in `navigate`.

mod navigate;
mod project_replace;
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
        // Opened from an undocked window (its swap is on), Find searches that
        // window's sheet; a list read from another window's sheet is dropped.
        let window = self.interaction_state.acting_window;
        let find = &mut self.ui_state.find_replace;
        if find.window != window {
            find.matches.clear();
            find.selected_index = None;
            find.window = window;
        }
        find.open = true;
        find.replace_mode = replace_mode;
        self.interaction_state.context_menu = None;
        self.ui_state.panel_list_open = false;
        self.ui_state.find_replace.skips_reported.clear();
        self.refresh_find_matches()
    }

    /// F3 / Shift+F3 pressed in `window`. A Sheet-scope search belongs to the
    /// window it was opened in; a press in any other window takes it over
    /// without opening the dialog, the way Find Next in the main window
    /// searches the sheet with the last query while the dialog is closed.
    /// A Project-scope list is not tied to a sheet, so it is stepped as it
    /// is (a hit on another sheet opens on the main window).
    pub(crate) fn step_find_in_window(
        &mut self,
        window: iced::window::Id,
        step: Message,
    ) -> Task<Message> {
        let owner = self.overlay_owner(window);
        let find = &mut self.ui_state.find_replace;
        if find.window != owner && find.scope == crate::find_replace::FindScope::Sheet {
            find.matches.clear();
            find.selected_index = None;
            find.window = owner;
        }
        self.dispatch_update(step)
    }

    /// A Find message acts on the sheet of the window the dialog was opened
    /// in: it runs in that window's swap. A project scan answer holds no
    /// sheet, so it stays on the main window; a project replace answer applies
    /// in the window's swap (its project, its visible sheet) while it is open.
    pub(crate) fn handle_find_replace_message(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;
        let Some(window) = self.ui_state.find_replace.window else {
            return self.handle_find_replace_message_here(msg);
        };
        if matches!(
            msg,
            FindReplaceMsg::ProjectScanFinished { .. } | FindReplaceMsg::Close
        ) {
            return self.handle_find_replace_message_here(msg);
        }
        // A replace answer for a window that has closed is stale (closing
        // superseded the job): its generation check drops it quietly.
        if !self.is_document_window(window)
            && matches!(msg, FindReplaceMsg::ProjectReplacePrepared { .. })
        {
            return self.handle_find_replace_message_here(msg);
        }
        if !self.is_document_window(window) {
            crate::diagnostics::log_warning(
                "Find was dropped: the window it searches has closed; open Find again.",
            );
            return Task::none();
        }
        self.run_in_window_document(window, |app| app.handle_find_replace_message_here(msg))
    }

    fn handle_find_replace_message_here(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;

        match msg {
            FindReplaceMsg::Close => {
                self.ui_state.find_replace.open = false;
                // A scan that lands after Close has nobody to show it to, and
                // a replace still reading the disk must not apply unseen.
                self.cancel_replace_job("Find was closed");
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
            FindReplaceMsg::ProjectReplacePrepared { generation, result } => {
                return self.apply_project_replace(generation, &result);
            }
            FindReplaceMsg::ProjectScanFinished { generation, result } => {
                self.apply_project_scan(generation, &result);
            }
            FindReplaceMsg::ReplacementChanged(value) => {
                self.ui_state.find_replace.replacement = value;
            }
            FindReplaceMsg::SelectResult(index) => return self.focus_find_result(index),
            FindReplaceMsg::NextRequested => return self.step_find_result(true),
            FindReplaceMsg::PreviousRequested => return self.step_find_result(false),
            FindReplaceMsg::ReplaceCurrent => return self.replace_current_find_match(),
            FindReplaceMsg::ReplaceAll => return self.replace_all_find_matches(),
        }

        Task::none()
    }
}
