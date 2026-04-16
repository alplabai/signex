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
                        eprintln!(
                            "[tab] Close blocked: tab '{}' has unsaved changes",
                            self.document_state.tabs[idx].title
                        );
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
            }
        }
    }
}