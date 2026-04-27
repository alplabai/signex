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
                // Always clear the drag state on release. The
                // press-handler arms it on every mouse-down; if
                // the cursor never moved past the threshold (which
                // gates the ghost in `view`), we still need to
                // reset state so the next render doesn't paint the
                // ghost on the resting cursor.
                self.ui_state.tab_dragging = None;
                Task::none()
            }
            TabMessage::ContextMenu(idx) => {
                // Right-click on a tab — route to the overlay
                // dispatcher so the menu opens with all the standard
                // hover/dismiss plumbing (mutex with the canvas /
                // project-tree menus, hover-tick subscription armed,
                // etc.). Only the main tab bar produces these — the
                // single-tab undocked window has nothing useful to
                // offer in a context menu.
                if !is_main {
                    return Task::none();
                }
                Task::done(Message::ShowTabContextMenu(idx))
            }
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
        let closing_path = self.document_state.tabs[idx].path.clone();
        // WS-I: tab-not-window — capture before removal so the
        // editor-state cleanup below can run after the tab is dropped.
        let closing_kind = self.document_state.tabs[idx].kind.clone();

        // If the tab has an undocked window open, close that window
        // too — otherwise it'd be an orphan showing "No document open"
        // indefinitely. The window's `SecondaryWindowClosed` cleans up
        // `canvases[id]` + `ui_state.windows[id]`.
        use crate::app::state::WindowKind;
        let orphan_window_ids: Vec<iced::window::Id> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match (kind, &closing_kind) {
                (WindowKind::UndockedTab { path, .. }, _) if path == &closing_path => Some(*id),
                // WS-I: tab-not-window
                (
                    WindowKind::ComponentEditor {
                        library_path,
                        component_id,
                    },
                    crate::app::TabKind::ComponentEditor(ce),
                ) if library_path == &ce.library_path && component_id == &ce.component_id => {
                    Some(*id)
                }
                _ => None,
            })
            .collect();

        // Park dirty engines: when the file is in `dirty_paths`, keep its
        // engine entry alive so reopening the tab restores the in-memory
        // edits. Drop the engine only when the file is clean.
        if !self.document_state.dirty_paths.contains(&closing_path) {
            self.document_state.engines.remove(&closing_path);
        }
        if self.document_state.active_path.as_ref() == Some(&closing_path) {
            self.document_state.active_path = None;
        }

        // WS-I: tab-not-window — drop the editor state when the tab is
        // a Component Editor. The library subsystem doesn't have a
        // dirty-park mechanism yet, so closing the tab discards the
        // draft (matches Wave 2's window-close behaviour).
        if let crate::app::TabKind::ComponentEditor(ref ce) = closing_kind {
            self.library
                .editors
                .remove(&crate::library::state::EditorAddress::new(
                    ce.library_path.clone(),
                    ce.component_id,
                ));
        }
        self.document_state.tabs.remove(idx);
        if self.document_state.active_tab >= self.document_state.tabs.len()
            && self.document_state.active_tab > 0
        {
            self.document_state.active_tab -= 1;
        }
        self.sync_active_tab();
        // Refresh so the open-dot drops immediately on tab close. The
        // dirty dot stays since `dirty_paths` is untouched.
        self.refresh_panel_ctx();

        if orphan_window_ids.is_empty() {
            Task::none()
        } else {
            Task::batch(orphan_window_ids.into_iter().map(iced::window::close))
        }
    }

    pub(crate) fn handle_tab_context_action(
        &mut self,
        action: crate::app::TabContextAction,
    ) -> Task<Message> {
        use crate::app::TabContextAction as A;
        // Dismiss the menu first — every action takes effect
        // immediately and a lingering menu is always wrong after.
        self.interaction_state.tab_context_menu = None;

        match action {
            A::Close(idx) => self.close_tab_now(idx),
            A::CloseAllOthers(keep_idx) => {
                // Close every tab except `keep_idx`, descending so
                // each `close_tab_now(i)` removes a tab without
                // shifting the indices of tabs we haven't visited
                // yet. The kept tab's index does shift as tabs
                // below it close, but `close_tab_now`'s active-tab
                // adjuster (`if active_tab >= len ... -= 1`) tracks
                // the live position correctly without us needing
                // to thread the kept path through this loop.
                let mut indices: Vec<usize> = (0..self.document_state.tabs.len())
                    .filter(|&i| i != keep_idx)
                    .collect();
                indices.sort_unstable_by(|a, b| b.cmp(a));
                let mut tasks = Vec::with_capacity(indices.len());
                for i in indices {
                    tasks.push(self.close_tab_now(i));
                }
                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            A::CloseAll => {
                let mut tasks = Vec::new();
                for idx in (0..self.document_state.tabs.len()).rev() {
                    tasks.push(self.close_tab_now(idx));
                }
                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            A::Undock(idx) => Task::done(Message::UndockTab(idx)),
        }
    }

}
