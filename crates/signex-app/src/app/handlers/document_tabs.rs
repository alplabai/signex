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
                Task::done(Message::ContextMenu(ContextMenuMsg::ShowTab(idx)))
            }
            TabMessage::StartDrag(idx) => {
                // Drag-to-reorder / drag-out-to-detach originates only
                // from the main tab bar. The single-tab bar inside an
                // undocked window has nothing to drag into.
                if !is_main {
                    return Task::none();
                }
                // Seed the drag anchor from the live cursor: the press
                // message carries no position, so `last_mouse_pos` is the
                // only real coordinate available for the next DragMove's
                // threshold check.
                let (mx, my) = self.interaction_state.last_mouse_pos;
                self.ui_state.tab_dragging = Some((idx, mx, my));
                Task::none()
            }
        }
    }

    /// Activate the next (or previous) schematic tab, wrapping at the ends
    /// and stepping over non-schematic tabs. A no-op unless a schematic tab
    /// is active, and when it is the only one.
    pub(crate) fn step_schematic_sheet(&mut self, forward: bool) -> Task<Message> {
        let tabs = &self.document_state.tabs;
        let sheets: Vec<usize> = tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| matches!(tab.kind, TabKind::Schematic))
            .map(|(idx, _)| idx)
            .collect();
        let active = self.document_state.active_tab;
        let Some(pos) = sheets.iter().position(|idx| *idx == active) else {
            return Task::none();
        };
        let next = if forward {
            sheets[(pos + 1) % sheets.len()]
        } else {
            sheets[(pos + sheets.len() - 1) % sheets.len()]
        };
        if next != active {
            self.park_active_schematic_session();
            self.document_state.active_tab = next;
            self.sync_active_tab();
        }
        Task::none()
    }

    /// Activate the next / previous document tab of any kind, wrapping.
    pub(crate) fn step_document_tab(&mut self, forward: bool) -> Task<Message> {
        let count = self.document_state.tabs.len();
        let active = self.document_state.active_tab;
        if count < 2 || active >= count {
            return Task::none();
        }
        let next = if forward {
            (active + 1) % count
        } else {
            (active + count - 1) % count
        };
        self.park_active_schematic_session();
        self.document_state.active_tab = next;
        self.sync_active_tab();
        Task::none()
    }

    /// Close a tab and discard its in-flight draft. Callers that already
    /// resolved unsaved work (Close Project / Remove / Discard All) rely on
    /// this dropping the draft.
    pub(crate) fn close_tab_now(&mut self, idx: usize) -> Task<Message> {
        self.close_tab(idx, false)
    }

    /// Close a tab the user asked to close, keeping a dirty symbol,
    /// footprint or Component Preview draft alive in the background like a
    /// dirty schematic engine (#110). Reopening the tab restores the draft;
    /// Save All / Close Library still see it, so nothing is lost silently.
    pub(crate) fn close_tab_parking_dirty(&mut self, idx: usize) -> Task<Message> {
        self.close_tab(idx, true)
    }

    fn close_tab(&mut self, idx: usize, park_dirty: bool) -> Task<Message> {
        // Footprint edits reach `dirty_paths` lazily; settle that before
        // deciding whether a draft is dirty.
        self.sync_editor_dirty_paths();
        if idx >= self.document_state.tabs.len() {
            tracing::warn!(
                target: "signex::tabs",
                idx,
                len = self.document_state.tabs.len(),
                "close tab: index out of range, ignored",
            );
            return Task::none();
        }
        let closing_path = self.document_state.tabs[idx].path.clone();
        // A History restore still running for this document must not land in
        // a tab reopened later, but its worker may still write the file, so
        // the claim stays (and saves stay refused) until it reports (#859).
        self.document_state
            .pending_restores
            .cancel_retaining(&closing_path);
        // Capture the tab kind before removal so the editor-state
        // cleanup below can run after the tab is dropped.
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
                // Component Preview windows are addressed by the DBLib
                // row tier `(library_path, table, row_id)`.
                (
                    WindowKind::ComponentEditor {
                        library_path,
                        table,
                        row_id,
                    },
                    crate::app::TabKind::ComponentEditor(ce),
                ) if library_path == &ce.library_path
                    && table == &ce.table
                    && row_id == &ce.row_id =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .collect();

        // Park dirty engines: when the file is in `dirty_paths`, keep its
        // engine entry alive so reopening the tab restores the in-memory
        // edits. Drop the engine only when the file is clean.
        if !self.document_state.dirty_paths.contains(&closing_path) {
            self.forget_net_colors_of(&closing_path);
            self.document_state.engines.remove(&closing_path);
        }
        if self.document_state.active_path.as_ref() == Some(&closing_path) {
            self.document_state.active_path = None;
        }
        // A loose sheet's designator note is keyed by its own path; forget it
        // so reopening the sheet says it again (a project's is forgotten on
        // project close).
        if self
            .document_state
            .document_project(&closing_path)
            .is_none()
        {
            self.document_state
                .instance_note_scopes
                .remove(&closing_path);
        }

        // Drop the editor state when the tab is a Component Preview,
        // unless the user asked to close a dirty one: that draft is parked
        // (it stays in `library.editors`, where Save All / Close Library
        // find it).
        if let crate::app::TabKind::ComponentEditor(ref ce) = closing_kind {
            let address = crate::library::state::EditorAddress::new(
                ce.library_path.clone(),
                ce.table.clone(),
                ce.row_id,
            );
            let parked = park_dirty
                && self
                    .library
                    .editors
                    .get(&address)
                    .is_some_and(|editor| editor.dirty);
            if !parked {
                self.library.editors.remove(&address);
            }
        }
        // Standalone primitive editor tabs — drop the per-tab editor
        // state on close, unless the user asked to close a dirty one: that
        // draft is parked with its dirty marker, so Save All can still
        // write it. A dropped draft takes its marker with it: a stale
        // `dirty_paths` entry with no editor behind it would make quit's
        // Save All fail on a file nobody can save.
        if let crate::app::TabKind::SymbolEditor(ref p)
        | crate::app::TabKind::FootprintEditor(ref p) = closing_kind
        {
            if park_dirty && self.primitive_draft_is_dirty(p) {
                self.document_state.dirty_paths.insert(p.clone());
            } else {
                self.document_state.symbol_editors.remove(p);
                self.document_state.footprint_editors.remove(p);
                self.document_state.dirty_paths.remove(p);
            }
        }
        // A PCB has no parked draft, so its marker must not outlive the tab:
        // Save All has no PCB branch and would fail on it.
        // ponytail: valid only while a PCB can't be edited; park or prompt once it can.
        if matches!(closing_kind, crate::app::TabKind::Pcb) {
            self.document_state.dirty_paths.remove(&closing_path);
        }
        // Library Browser tabs — drop the per-browser state on close.
        // The library itself stays mounted; only the tab-scoped UI
        // state (active table, search buffer, selection) goes away.
        if let crate::app::TabKind::LibraryBrowser(ref p) = closing_kind {
            self.library.library_browsers.remove(p);
        }
        self.document_state.tabs.remove(idx);
        self.document_state.tab_placements.forget(&closing_path);
        // The remembered page setup is session memory for an open document;
        // once no tab holds the path any more it has nothing to restore into.
        // A dirty tab parked in the background comes back with its unsaved
        // edits when reopened, so it gets its page setup back too.
        let parked_dirty = park_dirty && self.document_state.dirty_paths.contains(&closing_path);
        let no_tab_left = !self
            .document_state
            .tabs
            .iter()
            .any(|tab| tab.path == closing_path);
        // Nothing of the document is left: its save failure or queued save
        // must not name a later document at the same path.
        if no_tab_left && !self.document_state.dirty_paths.contains(&closing_path) {
            self.document_state
                .document_saves
                .forget_where(|p| p == closing_path);
        }
        if !parked_dirty && no_tab_left {
            crate::app::state::forget_page_setup(
                &mut self.document_state.page_setup_memory,
                self.document_state.preview.as_mut(),
                &closing_path,
            );
        }
        // A tab left of the active one shifted it down a slot: follow it
        // so focus stays on the same document.
        // Removing the tail while active also clamps one slot down.
        let active = self.document_state.active_tab;
        if active > 0 && (idx < active || active >= self.document_state.tabs.len()) {
            self.document_state.active_tab -= 1;
        }
        self.sync_active_tab();
        // Refresh so the open-dot drops immediately on tab close. The
        // dirty dot stays since `dirty_paths` is untouched.
        self.refresh_panel_ctx();

        // A document that is gone (or was discarded before the close) has
        // nothing left to recover, whether or not autosave is still on.
        let sweep = self.autosave_sweep();
        Task::batch(
            orphan_window_ids
                .into_iter()
                .map(iced::window::close)
                .chain(std::iter::once(sweep)),
        )
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
            A::Close(idx) => self.close_tab_parking_dirty(idx),
            A::CloseAllOthers(keep_idx) => {
                // Close every tab except `keep_idx`, descending so
                // each `close_tab_now(i)` removes a tab without
                // shifting the indices of tabs we haven't visited
                // yet. The kept tab's index does shift as tabs
                // below it close, but `close_tab_now` re-points
                // `active_tab` at the same document after each
                // removal, so no kept path is threaded through.
                let mut indices: Vec<usize> = (0..self.document_state.tabs.len())
                    .filter(|&i| i != keep_idx)
                    .collect();
                indices.sort_unstable_by(|a, b| b.cmp(a));
                let mut tasks = Vec::with_capacity(indices.len());
                for i in indices {
                    tasks.push(self.close_tab_parking_dirty(i));
                }
                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            A::CloseAll => {
                self.cancel_in_flight_reads();
                let mut tasks = Vec::new();
                for idx in (0..self.document_state.tabs.len()).rev() {
                    tasks.push(self.close_tab_parking_dirty(idx));
                }
                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            A::Undock(idx) => Task::done(Message::Window(WindowMsg::UndockTab(idx))),
        }
    }
}
