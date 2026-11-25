use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_text_edit_message(&mut self, message: TextEditMsg) -> Task<Message> {
        match message {
            TextEditMsg::Changed(text) => {
                if let Some(ref mut state) = self.interaction_state.editing_text {
                    state.text = text;
                }
                self.finish_update()
            }
            TextEditMsg::Submit => {
                if let Some(window) = self.text_edit_undocked_window() {
                    return self.run_in_window_document(window, |app| {
                        app.dispatch_text_edit_message(TextEditMsg::Submit)
                    });
                }
                if let Some(state) = self.interaction_state.editing_text.take() {
                    self.commit_text_edit(state);
                }
                self.finish_update()
            }
            TextEditMsg::Requested(target) => {
                let focus = self.request_text_edit(target);
                Task::batch([focus, self.finish_update()])
            }
            TextEditMsg::NextFieldRequested => {
                if let Some(window) = self.text_edit_undocked_window() {
                    return self.run_in_window_document(window, |app| {
                        app.dispatch_text_edit_message(TextEditMsg::NextFieldRequested)
                    });
                }
                let focus = self.advance_text_edit();
                Task::batch([focus, self.finish_update()])
            }
        }
    }

    /// The undocked window an open editor belongs to: its commit runs on
    /// that window's sheet, as an untagged message would find main's.
    fn text_edit_undocked_window(&self) -> Option<iced::window::Id> {
        self.interaction_state
            .editing_text
            .as_ref()
            .and_then(|state| state.sheet.as_deref())
            .and_then(|sheet| self.undocked_window_of_other_sheet(sheet))
    }
}
