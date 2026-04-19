use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_document_tab_message(&mut self, msg: TabMessage) -> Task<Message> {
        match msg {
            TabMessage::Select(idx) => {
                // If a drag is in flight from a different tab in the
                // same bar, treat release-on-idx as a drop/reorder
                // instead of a tab switch. Matches Altium's drag-the-
                // tab behaviour and mirrors the dock-region reorder
                // below in `dock::mod.rs`.
                if let Some((from, _, _)) = self.ui_state.tab_dragging
                    && from != idx
                    && from < self.document_state.tabs.len()
                    && idx < self.document_state.tabs.len()
                {
                    let tab = self.document_state.tabs.remove(from);
                    self.document_state.tabs.insert(idx, tab);
                    // Preserve the active tab visually — if the
                    // dragged tab was active, it follows the move;
                    // otherwise adjust the index to account for the
                    // shift.
                    let active = self.document_state.active_tab;
                    self.document_state.active_tab = if active == from {
                        idx
                    } else if from < active && idx >= active {
                        active - 1
                    } else if from > active && idx <= active {
                        active + 1
                    } else {
                        active
                    };
                    self.ui_state.tab_dragging = None;
                    return Task::none();
                }
                if idx < self.document_state.tabs.len() && idx != self.document_state.active_tab {
                    self.park_active_schematic_session();
                    self.document_state.active_tab = idx;
                    self.sync_active_tab();
                }
                Task::none()
            }
            TabMessage::Close(idx) => {
                if idx < self.document_state.tabs.len() {
                    if self.document_state.tabs[idx].dirty {
                        // Ask the user before discarding edits. Modal is
                        // rendered as an overlay by `view_close_tab_confirm`.
                        self.ui_state.close_tab_confirm = Some(idx);
                        return Task::none();
                    }
                    self.close_tab_now(idx);
                }
                Task::none()
            }
            TabMessage::Undock(idx) => Task::done(Message::UndockTab(idx)),
            TabMessage::StartDrag(idx, x, y) => {
                // Seed last_mouse_pos as (x, y) = (0, 0) wasn't real;
                // pull the live cursor from interaction_state so the
                // next DragMove delivers a correct position check.
                let (mx, my) = self.interaction_state.last_mouse_pos;
                let _ = (x, y);
                self.ui_state.tab_dragging = Some((idx, mx, my));
                Task::none()
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
                if idx == self.document_state.active_tab
                    && let Some(engine) = self.document_state.engine.as_mut()
                    && engine.save().is_ok()
                    && let Some(tab) = self.document_state.tabs.get_mut(idx)
                {
                    tab.dirty = false;
                }
                self.close_tab_now(idx);
                Task::none()
            }
        }
    }
}
