//! Components Panel handlers (Stage 9) — section collapse, add-library
//! from the Installed / Global headers, promote-to-global, and
//! place-into-schematic.
//!
//! Split out of the library dispatcher (`dispatch/library`).

use super::*;

impl Signex {
    /// Toggle the collapse flag for the named section.
    pub(super) fn handle_components_panel_toggle_section(
        &mut self,
        src: crate::library::state::ComponentsMountSource,
    ) -> Task<Message> {
        use crate::library::state::ComponentsMountSource;
        let p = &mut self.library.components_panel;
        match src {
            ComponentsMountSource::Project => p.collapsed_project = !p.collapsed_project,
            ComponentsMountSource::Installed => p.collapsed_installed = !p.collapsed_installed,
            ComponentsMountSource::Global => p.collapsed_global = !p.collapsed_global,
            ComponentsMountSource::Stock => p.collapsed_stock = !p.collapsed_stock,
        }
        Task::none()
    }

    /// Select `key` and start reading its details off-thread. Selecting the
    /// row already shown keeps what is there.
    fn select_components_panel_row(
        &mut self,
        key: crate::library::state::ComponentsPanelRowKey,
    ) -> Task<Message> {
        self.library.components_panel.selected = Some(key.clone());
        // The row already shown is read again only when it was saved since
        // or its last read failed, and not while a read is running.
        let stamp = self.components_row_stamp(&key);
        let details = &self.library.components_panel.details;
        if details.key.as_ref() == Some(&key)
            && (details.in_flight() || details.is_fresh(&key, stamp))
        {
            return Task::none();
        }
        self.start_components_details_load(key)
    }

    /// The saved state of the row `key` names, for the staleness check.
    fn components_row_stamp(
        &self,
        key: &crate::library::state::ComponentsPanelRowKey,
    ) -> Option<crate::panels::components_panel::details::RowStamp> {
        let row = self
            .library
            .library_at(&key.library_path)?
            .tables
            .get(&key.table)?
            .iter()
            .find(|r| r.row_id == key.row_id.0)?;
        Some((row.updated, row.content_hash))
    }

    /// Read the details of `key` off-thread; the result lands keyed by it.
    fn start_components_details_load(
        &mut self,
        key: crate::library::state::ComponentsPanelRowKey,
    ) -> Task<Message> {
        let libraries = self.library.shared_adapters();
        let stamp = self.components_row_stamp(&key);
        let Some(own) = self.library.shared_adapter_at(&key.library_path) else {
            let why = format!(
                "The library {} is not open, so the component details cannot be read.",
                key.library_path.display()
            );
            let details = &mut self.library.components_panel.details;
            let generation = details.begin(key.clone(), stamp);
            details.finish(&key, generation, Err(why.clone()));
            crate::diagnostics::log_warning(why);
            return Task::none();
        };
        let generation = self
            .library
            .components_panel
            .details
            .begin(key.clone(), stamp);
        let read_key = key.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::panels::components_panel::details::load::load_details(
                        &read_key,
                        own.as_ref(),
                        &libraries,
                    )
                })
                .await
                .unwrap_or_else(|e| Err(format!("details worker failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::ComponentsPanelDetailsLoaded {
                    key: key.clone(),
                    generation,
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// Reload the details when the row they show was saved since they were
    /// read. Runs after every library message, so a save, a refresh or an
    /// external edit all reach the pane.
    pub(super) fn refresh_components_details_if_stale(&mut self) -> Task<Message> {
        let details = &self.library.components_panel.details;
        let Some(key) = details.key.clone() else {
            return Task::none();
        };
        if details.in_flight() || details.stamp == self.components_row_stamp(&key) {
            return Task::none();
        }
        self.start_components_details_load(key)
    }

    /// The pointer went down on a row: select it and arm a drag. A click
    /// only selects; nothing is placed or opened from the row itself.
    pub(super) fn handle_components_panel_row_pressed(
        &mut self,
        key: crate::library::state::ComponentsPanelRowKey,
    ) -> Task<Message> {
        let panel = &mut self.library.components_panel;
        panel.row_menu = None;
        panel.drag = Some(crate::library::state::ComponentsRowDrag {
            key: key.clone(),
            start: self.interaction_state.last_mouse_pos,
            started: false,
            window: None,
            armed: false,
        });
        self.select_components_panel_row(key)
    }

    /// Right-click on a row: select it and open its menu at the pointer.
    pub(super) fn handle_components_panel_row_right_clicked(
        &mut self,
        key: crate::library::state::ComponentsPanelRowKey,
    ) -> Task<Message> {
        let (x, y) = self.interaction_state.last_mouse_pos;
        self.close_all_context_menus();
        let panel = &mut self.library.components_panel;
        panel.drag = None;
        panel.row_menu = Some(crate::library::state::ComponentsRowMenu {
            key: key.clone(),
            x,
            y,
            window: None,
        });
        self.select_components_panel_row(key)
    }

    /// A row menu item was picked: close the menu, then run the action.
    pub(super) fn handle_components_panel_row_menu_picked(
        &mut self,
        action: crate::library::state::ComponentsRowAction,
    ) -> Task<Message> {
        use crate::library::state::ComponentsRowAction;
        let Some(menu) = self.library.components_panel.row_menu.take() else {
            return Task::none();
        };
        let k = menu.key;
        match action {
            ComponentsRowAction::Place => {
                self.handle_components_panel_place(k.library_path, k.table, k.row_id)
            }
            ComponentsRowAction::Open => {
                Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                    library_path: k.library_path,
                    table: k.table,
                    row_id: k.row_id,
                }))
            }
        }
    }

    /// The pointer moved while a row is pressed: past the threshold the
    /// press becomes a drag.
    pub(crate) fn components_row_drag_moved(&mut self, x: f32, y: f32) {
        let Some(drag) = self.library.components_panel.drag.as_mut() else {
            return;
        };
        let (dx, dy) = (x - drag.start.0, y - drag.start.1);
        if dx.hypot(dy) > crate::panels::components_panel::DRAG_THRESHOLD {
            drag.started = true;
        }
    }

    /// The schematic sheet the pointer is over, when a Components row drop
    /// there would place: the pointer is on the canvas of a schematic tab.
    fn pointer_over_schematic_canvas(&self) -> bool {
        self.pointer_over_schematic_canvas_of(self.interaction_state.acting_window)
    }

    /// [`Self::pointer_over_schematic_canvas`] for the canvas of `window`
    /// (`None`: the main window's), without running inside its swap.
    fn pointer_over_schematic_canvas_of(&self, window: Option<iced::window::Id>) -> bool {
        let (x, y) = self.interaction_state.last_mouse_pos;
        let shown_tab = match window.and_then(|w| self.ui_state.windows.get(&w)) {
            Some(crate::app::state::WindowKind::UndockedTab { path, .. }) => self
                .document_state
                .tabs
                .iter()
                .find(|tab| tab.path == *path),
            _ => self.document_state.tabs.get(self.document_state.active_tab),
        };
        shown_tab.is_some_and(|tab| matches!(tab.kind, crate::app::TabKind::Schematic))
            && self
                .canvas_bounds_in(window)
                .contains(iced::Point::new(x, y))
    }

    /// A dragged row whose pointer first reaches the schematic canvas reads
    /// its part and attaches it to the cursor, so the symbol shows at the
    /// grid point the drop would use, like Place Component. Runs after every
    /// pointer move.
    pub(crate) fn arm_components_row_drag(&mut self) -> Task<Message> {
        let Some(drag) = self.library.components_panel.drag.as_ref() else {
            return Task::none();
        };
        if !drag.started || drag.armed {
            return Task::none();
        }
        let started_in = drag.window;
        // The pointer position is in the coordinates of the window it is in:
        // the ghost follows it only inside the window the row was pressed in.
        let pointer_in_start_window = self
            .interaction_state
            .pointer_window
            .is_none_or(|w| self.overlay_owner(w) == started_in);
        if !pointer_in_start_window {
            return Task::none();
        }
        // A row dragged in an undocked window arms on that window's sheet,
        // the one its canvas shows (#2037).
        match started_in {
            None => self.arm_components_row_drag_here(),
            // Cheap test first: the swap runs only once the pointer is over
            // the window's schematic canvas.
            Some(window)
                if self.is_document_window(window)
                    && self.pointer_over_schematic_canvas_of(Some(window)) =>
            {
                self.run_in_window_document(window, |app| app.arm_components_row_drag_here())
            }
            Some(_) => Task::none(),
        }
    }

    /// The ghost, against the active document of the window this runs in.
    fn arm_components_row_drag_here(&mut self) -> Task<Message> {
        let Some(drag) = self.library.components_panel.drag.as_ref() else {
            return Task::none();
        };
        if !self.pointer_over_schematic_canvas() {
            return Task::none();
        }
        let k = drag.key.clone();
        if let Some(drag) = self.library.components_panel.drag.as_mut() {
            drag.armed = true;
        }
        // A session already open (Place Component, a paste) is not this
        // drag's: end it so a release before the read lands cannot commit it.
        self.cancel_placement_session();
        self.handle_place_library_component(k.library_path, k.table, k.row_id)
    }

    /// The button went up. A row dragged past the threshold and released
    /// over the schematic canvas places its part at the ghost; released
    /// anywhere else (or over a tab that is not a schematic), or never
    /// dragged, it places nothing and the ghost goes.
    pub(crate) fn finish_components_row_drag(
        &mut self,
        released_in: iced::window::Id,
    ) -> Task<Message> {
        let Some(drag) = self.library.components_panel.drag.as_ref() else {
            return Task::none();
        };
        let started_in = drag.window;
        // The pointer position is in the coordinates of the window it is in,
        // so a drop counts only in the window the row was pressed in.
        if self.overlay_owner(released_in) != started_in {
            let armed = drag.armed;
            self.library.components_panel.drag = None;
            if armed {
                self.cancel_placement_session();
            }
            crate::diagnostics::log_info(
                "A Components drag was released in another window than it started in; nothing was placed.",
            );
            return Task::none();
        }
        let Some(window) = started_in else {
            return self.finish_components_row_drag_here();
        };
        if self.is_document_window(window) {
            return self
                .run_in_window_document(window, |app| app.finish_components_row_drag_here());
        }
        // The window the row was pressed in has closed: nothing to drop on.
        self.library.components_panel.drag = None;
        Task::none()
    }

    /// The drop, against the active document of the window this runs in.
    fn finish_components_row_drag_here(&mut self) -> Task<Message> {
        let Some(drag) = self.library.components_panel.drag.take() else {
            return Task::none();
        };
        let over_canvas = self.pointer_over_schematic_canvas();
        if !drag.started || !over_canvas {
            if drag.armed {
                self.cancel_placement_session();
            }
            return Task::none();
        }
        // Release point in world mm, through the live camera and the canvas
        // rectangle; the snap happens when the part is placed.
        let (x, y) = self.interaction_state.last_mouse_pos;
        let bounds = self.canvas_bounds_in(self.interaction_state.acting_window);
        let at = self
            .interaction_state
            .active_canvas()
            .screen_to_world(iced::Point::new(x - bounds.x, y - bounds.y));
        let world = (f64::from(at.x), f64::from(at.y));
        let k = drag.key;
        if drag.armed {
            // The ghost is on the cursor or its read is still running: the
            // drop commits when both the part and the sheet numbers are in.
            let epoch = self.interaction_state.placement_epoch;
            self.library.components_panel.pending_drop = Some((epoch, world));
            let ready = self
                .interaction_state
                .placement
                .as_ref()
                .is_some_and(|s| self.designators_current(&s.target_sheet));
            return if ready {
                self.commit_pending_drop()
            } else {
                Task::none()
            };
        }
        // Only a read that starts carries the drop: the same checks
        // `handle_place_library_component` refuses a pick with.
        let starts = self.document_state.active_path.is_some()
            && self.library.library_at(&k.library_path).is_some();
        let task = self.handle_place_library_component(k.library_path, k.table, k.row_id);
        if starts {
            let epoch = self.interaction_state.placement_epoch;
            self.library.components_panel.pending_drop = Some((epoch, world));
        }
        task
    }

    /// "+ Add Library…" — opens the `*.snxlib` directory picker,
    /// tagging the result with the section it was opened against.
    pub(super) fn handle_components_panel_add_library(
        &mut self,
        source: crate::library::state::ComponentsMountSource,
    ) -> Task<Message> {
        if source == crate::library::state::ComponentsMountSource::Project {
            return self.reject_project_add_library();
        }
        // Open `*.snxlib` directory picker — landing message is
        // `ComponentsPanelAddLibraryAt` so the dispatcher knows
        // which source bucket the result belongs to.
        let picker = self.ui_state.io.picker.clone();
        Task::perform(
            async move {
                picker
                    .dialog()
                    .set_title("Add Library (*.snxlib)")
                    .pick_folder()
                    .await
            },
            move |path| {
                Message::Library(LibraryMessage::ComponentsPanelAddLibraryAt { source, path })
            },
        )
    }

    /// Result of the Add Library file dialog — mount and record it in
    /// the matching source bucket.
    pub(super) fn handle_components_panel_add_library_at(
        &mut self,
        source: crate::library::state::ComponentsMountSource,
        path: Option<std::path::PathBuf>,
    ) -> Task<Message> {
        use crate::library::state::ComponentsMountSource;
        if source == ComponentsMountSource::Project {
            return self.reject_project_add_library();
        }
        let Some(path) = path else {
            return Task::none();
        };
        // Mount the library — same idempotent path the legacy
        // File ▸ Library ▸ Open Library… flow uses.
        if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                path = %path.display(),
                "components-panel add-library failed"
            );
            // Recoverable failures open their recovery dialog; anything
            // else must still reach the user, so raise the error card.
            if !route_open_error(&mut self.library, &path, &e) {
                self.document_state.error_notice =
                    Some(open_error_notice(&self.library, &path, "add library", &e));
            }
            return Task::none();
        }
        match source {
            ComponentsMountSource::Installed => {
                if !self.library.installed_libraries.contains(&path) {
                    self.library.installed_libraries.push(path);
                }
            }
            ComponentsMountSource::Global => {
                match crate::panels::components_panel::global_prefs::add_path(path.clone()) {
                    Ok(updated) => {
                        self.library.global_libraries = updated;
                    }
                    Err(e) => self.report_global_libraries_save_failure("add library", &e),
                }
            }
            // Neither section has an add button; nothing to record.
            ComponentsMountSource::Project | ComponentsMountSource::Stock => {}
        }
        Task::none()
    }

    /// Adding a library to the active project from the Components Panel
    /// has no backing flow yet (project libraries come from `.snxprj`),
    /// so say so instead of silently dropping the request.
    fn reject_project_add_library(&mut self) -> Task<Message> {
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::unavailable(
            "Adding a library to the project from the Components Panel is not yet available.",
        ));
        Task::none()
    }

    /// `global_libraries.toml` could not be written, so the change did not
    /// stick. The caller leaves the in-memory library lists as they were,
    /// and the user has to be told.
    pub(super) fn report_global_libraries_save_failure(
        &mut self,
        action: &str,
        err: &dyn std::fmt::Display,
    ) {
        tracing::warn!(
            target: "signex::library",
            error = %err,
            "global_libraries.toml save failed ({action})"
        );
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(format!(
            "Could not save the global library list ({action}): {err}"
        )));
    }

    /// Promote an Installed library to Global.
    pub(super) fn handle_components_panel_promote_to_global(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(idx) = self
            .library
            .installed_libraries
            .iter()
            .position(|p| p == &path)
        else {
            return Task::none();
        };
        // Persist first: the library leaves the Installed list only once
        // it is safely recorded as Global, otherwise a failed write would
        // leave it in neither list and gone from the panel.
        match crate::panels::components_panel::global_prefs::add_path(path) {
            Ok(updated) => {
                self.library.installed_libraries.remove(idx);
                self.library.global_libraries = updated;
            }
            Err(e) => self.report_global_libraries_save_failure("promote to global", &e),
        }
        Task::none()
    }

    /// "Place into Schematic" on a Components Panel row — the existing place
    /// handler, run here so a pick from an undocked window's row menu arms on
    /// that window's sheet (it runs in the window's swap).
    pub(super) fn handle_components_panel_place(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        // Same flow as the picker: the part attaches to the cursor.
        self.library.components_panel.row_menu = None;
        self.handle_place_library_component(library_path, table, row_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{Message, Signex};
    use crate::library::messages::LibraryMessage;
    use crate::library::state::ComponentsPanelRowKey;

    #[test]
    fn a_row_of_a_library_that_is_not_open_shows_an_error() {
        let (mut app, _boot) = Signex::new();
        let key = ComponentsPanelRowKey {
            library_path: std::path::PathBuf::from("gone.snxlib"),
            table: "parts".into(),
            row_id: signex_library::RowId::new(),
        };
        let _ = app.update(Message::Library(LibraryMessage::ComponentsPanelRowPressed(
            key,
        )));
        let details = &app.library.components_panel.details;
        assert!(details.error.is_some(), "no silent empty pane");
        assert!(!details.in_flight());
    }
}
