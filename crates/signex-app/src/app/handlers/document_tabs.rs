use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_document_tab_message(&mut self, msg: TabMessage) {
        match msg {
            TabMessage::Select(idx) => {
                if idx < self.document_state.tabs.len() && idx != self.document_state.active_tab {
                    self.park_active_schematic_session();
                    self.document_state.active_tab = idx;
                    self.sync_active_tab();
                }
            }
            TabMessage::Close(idx) => {
                if idx < self.document_state.tabs.len() {
                    if self.document_state.tabs[idx].dirty {
                        // Ask the user before discarding edits. Modal is
                        // rendered as an overlay by `view_close_tab_confirm`.
                        self.ui_state.close_tab_confirm = Some(idx);
                        return;
                    }
                    self.close_tab_now(idx);
                }
            }
        }
    }

    pub(crate) fn close_tab_now(&mut self, idx: usize) {
        if idx >= self.document_state.tabs.len() {
            return;
        }
        if idx == self.document_state.active_tab {
            self.document_state.engine = None;
        }
        self.document_state.tabs.remove(idx);
        if self.document_state.active_tab >= self.document_state.tabs.len()
            && self.document_state.active_tab > 0
        {
            self.document_state.active_tab -= 1;
        }
        self.sync_active_tab();
    }

    pub(crate) fn handle_close_tab_confirm(&mut self, choice: CloseTabChoice) -> Task<Message> {
        let Some(idx) = self.ui_state.close_tab_confirm.take() else {
            return Task::none();
        };
        match choice {
            CloseTabChoice::Cancel => Task::none(),
            CloseTabChoice::DiscardAndClose => {
                if idx < self.document_state.tabs.len() {
                    self.document_state.tabs[idx].dirty = false;
                    self.close_tab_now(idx);
                }
                Task::none()
            }
            CloseTabChoice::SaveAndClose => {
                // Save path: only meaningful for the active tab with a live
                // engine. For background tabs (which we'd have to activate
                // first) we fall back to discard-and-close; the save flow
                // across parked sessions is v0.7 scope.
                if idx == self.document_state.active_tab {
                    if let Some(engine) = self.document_state.engine.as_mut()
                        && engine.save().is_ok()
                    {
                        if let Some(tab) = self.document_state.tabs.get_mut(idx) {
                            tab.dirty = false;
                        }
                    }
                }
                self.close_tab_now(idx);
                Task::none()
            }
        }
    }
}
