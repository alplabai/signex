//! The project's Design Notebook (mockup rows 195, 197, 201 and 202): the
//! commands that open, generate and export it, the Notebook badges toggle,
//! and the Projects tree's notebook row. This slice resolves which project
//! and which file a request means; the notebook tab, the badges, the
//! generator and the PDF export act on it (`seams.rs`). Rows and ids:
//! `docs/MENU_BAR.md`, "Project ▸ Design Notebook".

use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

mod seams;
#[cfg(test)]
mod tests;

/// Messages of a project's Design Notebook, under `ProjectMsg::Notebook`.
/// A request without a project means the active document's project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebookMsg {
    /// Project ▸ Design Notebook ▸ Open Design Notebook, the palette, Add
    /// New ▸ Design Notebook or a double-click on the notebook row.
    OpenRequested { project: Option<ProjectId> },
    /// Project ▸ Design Notebook ▸ Generate from Project… (mockup row 201).
    GenerateRequested { project: Option<ProjectId> },
    /// File ▸ Export ▸ Design Notebook PDF… (mockup row 202).
    ExportPdfRequested { project: Option<ProjectId> },
    /// View ▸ Notebook Badges (mockup row 197).
    BadgesToggled,
}

impl From<NotebookMsg> for Message {
    fn from(msg: NotebookMsg) -> Self {
        Message::Project(crate::app::ProjectMsg::Notebook(msg))
    }
}

/// The notebook a request means: its project and its file, which may not
/// exist yet (a notebook is written only on Save, mockup row 195).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NotebookTarget {
    pub project: ProjectId,
    pub path: PathBuf,
}

impl Signex {
    pub(crate) fn dispatch_notebook_message(&mut self, msg: NotebookMsg) -> Task<Message> {
        match msg {
            NotebookMsg::OpenRequested { project } => {
                self.with_notebook(project, Self::show_design_notebook)
            }
            NotebookMsg::GenerateRequested { project } => {
                self.with_notebook(project, Self::generate_design_notebook)
            }
            NotebookMsg::ExportPdfRequested { project } => {
                self.with_notebook(project, Self::export_design_notebook_pdf)
            }
            NotebookMsg::BadgesToggled => self.toggle_notebook_badges(),
        }
    }

    /// The notebook of `project`, else of the active document's project:
    /// the same project rule as Design ▸ Rules.
    pub(crate) fn notebook_target(&self, project: Option<ProjectId>) -> Option<NotebookTarget> {
        let id = self.rules_project_for(project)?;
        let loaded = self.document_state.project_by_id(id)?;
        Some(NotebookTarget {
            project: id,
            path: notebook_file(loaded),
        })
    }

    /// Add New ▸ Design Notebook on the project at `tree_path[0]`: its
    /// notebook, written only when it is saved.
    pub(crate) fn open_design_notebook_at(&mut self, tree_path: &[usize]) -> Task<Message> {
        let project = tree_path
            .first()
            .and_then(|idx| self.document_state.projects.get(*idx))
            .map(|loaded| loaded.id);
        match project {
            Some(id) => {
                self.dispatch_notebook_message(NotebookMsg::OpenRequested { project: Some(id) })
            }
            None => Task::none(),
        }
    }

    fn with_notebook(
        &mut self,
        project: Option<ProjectId>,
        then: fn(&mut Self, NotebookTarget) -> Task<Message>,
    ) -> Task<Message> {
        match self.notebook_target(project) {
            Some(target) => then(self, target),
            None => {
                crate::diagnostics::log_info("Design Notebook: no project is open");
                Task::none()
            }
        }
    }
}

/// Whether the probe found the notebook file of `project`: the Projects
/// tree lists it under Notebook once it does (mockup row 19).
pub(crate) fn has_notebook(
    project: &crate::app::LoadedProject,
    facts: &crate::app::state::project_fs::ProjectFsFacts,
) -> bool {
    facts.knows_present(&notebook_file(project))
}

/// The path the probe asks about for `project`.
pub(crate) fn notebook_file(project: &crate::app::LoadedProject) -> PathBuf {
    signex_notebook::notebook_path(Path::new(&project.data.dir))
}

/// A tree row that opens the notebook: a `.snxnbk` file.
pub(crate) fn is_notebook_row(label: &str) -> bool {
    signex_notebook::is_notebook_file(label)
}
