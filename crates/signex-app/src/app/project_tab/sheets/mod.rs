//! Sheets section of the Project tab (#1689): the project's sheets in list
//! order (a title block's Sheet N of M, the project tree) with Move up / Move
//! down on row hover, the root and missing sheets marked, then what a new
//! sheet starts from (size, orientation, title block), edited with the New
//! Project dialog's own fields through the Project setup flow, beside a
//! preview of that sheet (Graphite mockup m31). A move is one step of the
//! General history.

mod preview;
mod view;

#[cfg(test)]
mod tests;

pub(super) use view::view;

use std::path::Path;

use signex_types::project::ListMove;

use super::ProjectGeneralFacts;
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};

/// One row of the Sheets table: the entry as listed and what the app knows
/// of its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SheetRow {
    pub name: String,
    pub filename: String,
    /// The project's declared root schematic.
    pub is_root: bool,
    /// The last probe found no file at its path and no open or unsaved
    /// document holds it.
    pub is_missing: bool,
}

impl Signex {
    /// The project's sheets in list order, each with its root and missing
    /// marks read from the Model (the declared root and the last
    /// filesystem probe), never from the disk.
    pub(crate) fn project_sheet_rows(&self, project: &LoadedProject) -> Vec<SheetRow> {
        let root = ProjectGeneralFacts::of(project).schematic_root;
        let dir = project.dir();
        project
            .data
            .sheets
            .iter()
            .map(|sheet| SheetRow {
                name: sheet.name.clone(),
                filename: sheet.filename.clone(),
                is_root: root
                    .as_deref()
                    .is_some_and(|root| Path::new(root) == Path::new(&sheet.filename)),
                is_missing: self.listed_file_missing(&dir.join(&sheet.filename)),
            })
            .collect()
    }

    /// A listed file is missing when the probe found it absent and no open
    /// or unsaved document holds it: a sheet created this session is not on
    /// disk until it is saved (the project tree reads it the same way).
    fn listed_file_missing(&self, path: &Path) -> bool {
        let document = &self.document_state;
        document.project_fs.facts.is_missing(path)
            && !document.tabs.iter().any(|tab| tab.path == path)
            && !document.is_document_dirty(path)
    }

    /// Move up / Move down in the Sheets table: one step of its own.
    pub(super) fn move_project_sheet(
        &mut self,
        project_id: ProjectId,
        filename: String,
        step: ListMove,
    ) {
        let command = signex_engine::ProjectSectionCommand::MoveSheet { filename, step };
        // A move never refuses; at the end of the list it changes nothing.
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }

    /// Add New ▸ Schematic in an empty Sheets section: the New Schematic
    /// setup the project tree's Add New ▸ Schematic opens for the project.
    /// A project closed meanwhile opens nothing.
    pub(super) fn add_schematic_from_tab(&mut self, project_id: ProjectId) -> iced::Task<Message> {
        if self.document_state.project_by_id(project_id).is_none() {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; no schematic was added",
            );
            return iced::Task::none();
        }
        self.open_document_setup(crate::app::SetupKind::Sheet, Some(project_id))
    }
}
