//! General section of the Project tab: the editable name and description,
//! and the read-only facts about the project's files.

mod view;

pub(super) use view::view;

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

/// Why `stem` cannot be a project name (the `.snxprj` file stem), or `None`
/// when it can. The one rule for the Rename dialog and the Project tab.
pub(crate) fn project_stem_problem(stem: &str) -> Option<&'static str> {
    if stem.trim().is_empty() {
        Some("Name cannot be empty.")
    } else if stem.contains(['/', '\\', '.']) {
        Some("Name cannot contain '/', '\\', or '.'.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::project_stem_problem;

    #[test]
    fn a_stem_must_be_non_empty_and_free_of_separators_and_dots() {
        assert_eq!(project_stem_problem("Oscar"), None);
        assert_eq!(project_stem_problem("Motor controller"), None);
        assert!(project_stem_problem("").is_some());
        assert!(project_stem_problem("   ").is_some());
        for bad in ["a/b", "a\\b", "a.b"] {
            assert!(project_stem_problem(bad).is_some(), "{bad}");
        }
    }
}
