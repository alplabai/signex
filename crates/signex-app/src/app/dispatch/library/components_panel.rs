//! Components Panel handlers (Stage 9) — section collapse, add-library
//! from the Installed / Global headers, promote-to-global, and the
//! Stage-9 stubs (manage-global, add-to-project, place-into-schematic).
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

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
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            error = %e,
                            "global_libraries.toml save failed"
                        );
                    }
                }
            }
            ComponentsMountSource::Project => {
                // Project section's "+ Add Library…" button is
                // not rendered in Stage 9 (project libs come
                // from `.snxprj`), but the dispatcher still
                // handles the variant for future wiring.
                tracing::info!(
                    target: "signex::library",
                    path = %path.display(),
                    "TODO: add-library to active project (ComponentsMountSource::Project)"
                );
            }
        }
        Task::none()
    }

    /// Promote an Installed library to Global.
    pub(super) fn handle_components_panel_promote_to_global(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(idx) = self
            .library
            .installed_libraries
            .iter()
            .position(|p| p == &path)
        {
            self.library.installed_libraries.remove(idx);
            match crate::panels::components_panel::global_prefs::add_path(path.clone()) {
                Ok(updated) => self.library.global_libraries = updated,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        error = %e,
                        "promote-to-global save failed"
                    );
                }
            }
        }
        Task::none()
    }

    /// "Manage…" on the Global section header. Stage 9 stub.
    pub(super) fn handle_components_panel_manage_global(&mut self) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            "TODO: open Global Libraries management dialog"
        );
        Task::none()
    }

    /// "Add to Project" on a Components Panel row. Stage 9 stub.
    pub(super) fn handle_components_panel_add_to_project(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            path = %library_path.display(),
            "TODO: add library to active project's Project.libraries"
        );
        Task::none()
    }

    /// "Place into Schematic" on a Components Panel row. Stage 9 stub —
    /// routes through the existing place handler.
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
