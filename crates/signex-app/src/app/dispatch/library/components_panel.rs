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
        }
        Task::none()
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
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Add Library (*.snxlib)")
                    .pick_folder()
                    .await
                    .map(|f| f.path().to_path_buf())
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
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(
                    format!("Cannot add library {}: {e}", path.display()),
                ));
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
            ComponentsMountSource::Project => {}
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
    fn report_global_libraries_save_failure(&mut self, action: &str, err: &dyn std::fmt::Display) {
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

    /// "Place into Schematic" on a Components Panel row — routes through
    /// the existing place handler.
    pub(super) fn handle_components_panel_place(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        // Stage 9 stub — the full ghost-component drag is polish
        // work. Dispatch through the existing place handler so
        // the row at least lands on the canvas via the picker
        // path until ghost-drag ships.
        Task::done(Message::Library(LibraryMessage::PlaceLibraryComponent {
            library_path,
            table,
            row_id,
        }))
    }
}
