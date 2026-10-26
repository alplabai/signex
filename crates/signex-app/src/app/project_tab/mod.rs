//! The Project document tab (#1202, epic #1084): edits the project's
//! `.snxprj` like any other document. Design and the rule that keeps it from
//! clobbering the other `.snxprj` writers:
//! `docs/superpowers/specs/2026-10-26-project-tab-shell-design.md`.
//!
//! The tab holds no copy of the project: it edits `LoadedProject::data` in
//! place, through `update`, and its undo history covers only the fields its
//! sections own. One module per section (`general`); the shell (`view`)
//! lays out the section list beside the selected section.

mod general;
mod open;
mod update;
mod view;

pub use general::ProjectGeneralFacts;
pub(crate) use general::project_stem_problem;
pub(crate) use open::project_tab_title;

use crate::app::state::ProjectId;

/// A section of the Project tab. Later slices add Parameters, Sheets,
/// Variants, Libraries, ERC and annotation, Version control (#1084).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSection {
    #[default]
    General,
}

impl ProjectSection {
    /// Every section, in list order.
    pub const ALL: [ProjectSection; 1] = [ProjectSection::General];

    pub fn label(self) -> &'static str {
        match self {
            ProjectSection::General => "General",
        }
    }
}

/// Per-project Project tab state. Lives as long as the project is loaded,
/// not as long as the tab: a closed tab with unsaved edits is parked like
/// any other document, and reopening it brings its undo history back.
#[derive(Debug, Clone, Default)]
pub struct ProjectTabState {
    pub section: ProjectSection,
    /// Undo/redo of the General section's fields (engine-owned).
    pub general_history: signex_engine::ProjectGeneralHistory,
}

/// Messages of a Project tab, namespaced under `ProjectMsg::Tab`. Each
/// carries the project it was produced for, so an edit can never land on a
/// project that replaced it.
#[derive(Debug, Clone)]
pub enum ProjectTabMsg {
    SectionSelected {
        project_id: ProjectId,
        section: ProjectSection,
    },
    NameEdited {
        project_id: ProjectId,
        text: String,
    },
    DescriptionEdited {
        project_id: ProjectId,
        text: String,
    },
}
