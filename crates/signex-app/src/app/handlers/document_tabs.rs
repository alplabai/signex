use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_document_tab_message(
        &mut self,
        window_id: iced::window::Id,
        msg: TabMessage,
    ) -> Task<Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);
        match msg {
            TabMessage::Select(idx) => {
                // If a drag is in flight from a different tab in the
                // same bar, treat release-on-idx as a drop/reorder
                // instead of a tab switch. Matches Altium's drag-the-
                // tab behaviour and mirrors the dock-region reorder
                // below in `dock::mod.rs`. Reordering is only meaningful
                // on the main tab bar — undocked windows show a single
                // tab, so a drag ending on their own tab is a no-op.
                if is_main
                    && let Some((from, _, _)) = self.ui_state.tab_dragging
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
                // Tab switch only mutates the shared active_tab when the
                // main window drove the click. Undocked windows own one
                // tab and must not clobber the main bar's active index.
                if is_main
                    && idx < self.document_state.tabs.len()
                    && idx != self.document_state.active_tab
                {
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
                    return self.close_tab_now(idx);
                }
                Task::none()
            }
            TabMessage::Undock(idx) => Task::done(Message::UndockTab(idx)),
            TabMessage::StartDrag(idx, x, y) => {
                // Drag-to-reorder / drag-out-to-detach originates only
                // from the main tab bar. The single-tab bar inside an
                // undocked window has nothing to drag into.
                if !is_main {
                    return Task::none();
                }
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

    pub(crate) fn close_tab_now(&mut self, idx: usize) -> Task<Message> {
        if idx >= self.document_state.tabs.len() {
            return Task::none();
        }
        // Drop the engine for the tab being closed, whether it was the
        // active one or a background schematic. The HashMap keeps every
        // open tab's engine live — closing the tab is the only point
        // where we prune an entry.
        let closing_path = self.document_state.tabs[idx].path.clone();

        // If the tab has an undocked window open, close that window
        // too — otherwise it'd be an orphan showing "No document open"
        // indefinitely. The window's `SecondaryWindowClosed` cleans up
        // `canvases[id]` + `ui_state.windows[id]`.
        use crate::app::state::WindowKind;
        let orphan_window_ids: Vec<iced::window::Id> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                WindowKind::UndockedTab { path, .. } if path == &closing_path => Some(*id),
                _ => None,
            })
            .collect();

        self.document_state.engines.remove(&closing_path);
        if self.document_state.active_path.as_ref() == Some(&closing_path) {
            self.document_state.active_path = None;
        }
        self.document_state.tabs.remove(idx);
        if self.document_state.active_tab >= self.document_state.tabs.len()
            && self.document_state.active_tab > 0
        {
            self.document_state.active_tab -= 1;
        }
        self.sync_active_tab();

        if orphan_window_ids.is_empty() {
            Task::none()
        } else {
            Task::batch(orphan_window_ids.into_iter().map(iced::window::close))
        }
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
                    return self.close_tab_now(idx);
                }
                Task::none()
            }
            CloseTabChoice::SaveAndClose => {
                // Save path: only meaningful for the active tab with a live
                // engine. For background tabs (which we'd have to activate
                // first) we fall back to discard-and-close; the save flow
                // across parked sessions is v0.7 scope.
                if idx == self.document_state.active_tab
                    && let Some(engine) = self.document_state.active_engine_mut()
                    && engine.save().is_ok()
                    && let Some(tab) = self.document_state.tabs.get_mut(idx)
                {
                    tab.dirty = false;
                }
                self.close_tab_now(idx)
            }
        }
    }
}
