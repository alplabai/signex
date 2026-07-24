//! Project-level context-menu actions for the project-navigation dock —
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    pub(crate) fn open_project_options_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        self.ui_state.project_options = Some(crate::app::ProjectOptionsState {
            project_idx,
            name: project.data.name.clone(),
            directory: project.data.dir.clone(),
            schematic_root: project.data.schematic_root.clone(),
            pcb_file: project.data.pcb_file.clone(),
            library_count: project.data.libraries.len(),
        });
    }

    /// Validate Project — promote the project to active, ensure its
    /// root schematic is open, then dispatch the existing ERC dialog.
    /// `tree_path[0]` is the owning project; we open the schematic
    /// root if no tab from this project is currently active so the
    /// ERC engine targets the right sheet.
    pub(crate) fn run_validate_project(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        let project = match self.document_state.projects.get(project_idx) {
            Some(p) => p,
            None => return iced::Task::none(),
        };
        let project_id = project.id;
        let project_dir = match project.path.parent() {
            Some(d) => d.to_path_buf(),
            None => return iced::Task::none(),
        };
        let schematic_root = project.data.schematic_root.clone();
        self.document_state.active_project = Some(project_id);

        // If the active tab already belongs to this project we're good;
        // otherwise open the schematic_root so ERC has a sheet to walk.
        // The open is now async (`open_schematic_file` reads+parses off
        // the UI thread), so batch its Task instead of assuming it has
        // landed before the ERC dialog opens — the dialog itself just
        // flips a flag and reads the active document at render time, so
        // it degrades gracefully if the schematic is still in flight.
        let active_belongs = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.project_id == Some(project_id))
            .unwrap_or(false);
        let open_task = if !active_belongs && let Some(root) = schematic_root {
            let path = project_dir.join(&root);
            if path.exists() {
                self.handle_document_file_opened(Some(path))
            } else {
                iced::Task::none()
            }
        } else {
            iced::Task::none()
        };
        self.refresh_panel_ctx();
        let erc_task = self.update(Message::Erc(ErcMsg::OpenDialog));
        iced::Task::batch([open_task, erc_task])
    }
}
