//! General section of the Project tab: the read-only name (with a Rename…
//! action), the editable description, and the read-only facts about the
//! project's files.

mod designator_standard;
mod view;

pub(super) use view::{labelled, tab_message, view};

use crate::app::state::LoadedProject;

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
