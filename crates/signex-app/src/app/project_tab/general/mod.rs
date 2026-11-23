//! General section of the Project tab: the read-only name (with a Rename…
//! action), the editable description, and the read-only facts about the
//! project's files.

mod designator_standard;
mod view;

pub(super) use view::{tab_message, view};

use crate::app::Signex;
use crate::app::state::{LoadedProject, ProjectId};

/// The read-only rows of the General section, read from the project as it
/// is now. Kept apart from the view so it is testable without widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGeneralFacts {
    pub directory: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    pub library_count: usize,
}

impl ProjectGeneralFacts {
    pub fn of(project: &LoadedProject) -> Self {
        Self {
            // `dir()` is derived from the file actually opened; the stored
            // `data.dir` string can be stale (see `LoadedProject::dir`).
            directory: project.dir().display().to_string(),
            schematic_root: project.data.schematic_root.clone(),
            pcb_file: project.data.pcb_file.clone(),
            library_count: project.data.libraries.len(),
        }
    }
}

impl Signex {
    /// Explore: the project's folder in the file manager, as the project
    /// tree's Explore shows it. A project closed meanwhile shows nothing.
    pub(in crate::app::project_tab) fn explore_project_folder(&self, project_id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return;
        };
        if let Err(err) = self.ui_state.io.opener.reveal(project.dir()) {
            crate::diagnostics::log_error(
                "Failed to reveal in file manager",
                &anyhow::anyhow!("{err}"),
            );
        }
    }
}
