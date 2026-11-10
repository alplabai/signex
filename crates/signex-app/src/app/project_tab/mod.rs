//! The Project document tab (#1202, epic #1084): edits the project's
//! `.snxprj` like any other document. Design and the rule that keeps it from
//! clobbering the other `.snxprj` writers:
//! `docs/superpowers/specs/2026-10-26-project-tab-shell-design.md`.
//!
//! The tab holds no copy of the project: it edits `LoadedProject::data` in
//! place, through `update`, and its undo history covers only the fields its
//! sections own. The project name is the file stem and is not a document
//! edit: Rename… runs the project-tree Rename flow. The tab is docked only
//! (an undocked window would take the main window's shortcuts). One module
//! per section (`general`); the shell (`view`) lays out the section list
//! beside the selected section.

mod agent_journals;
mod general;
mod open;
mod parameters;
mod schematic_defaults;
mod setup;
mod update;
mod view;

#[cfg(test)]
mod agent_journals_tests;
#[cfg(test)]
mod open_command_tests;
#[cfg(test)]
mod save_point_tests;
#[cfg(test)]
mod schematic_defaults_tests;
#[cfg(test)]
mod setup_tests;
#[cfg(test)]
mod undo_routing_tests;

pub use general::ProjectGeneralFacts;
pub(crate) use open::project_tab_title;

use crate::app::state::ProjectId;

/// A section of the Project tab. Later slices add Parameters, Sheets,
/// Variants, Libraries, ERC and annotation, Version control (#1084).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSection {
    #[default]
    General,
    /// Project parameters the sheets inherit (#1080).
    Parameters,
    /// Widths and sizes of unset schematic items (#2026).
    SchematicDefaults,
    /// What new PCBs and sheets start from (#2232).
    Setup,
}

impl ProjectSection {
    /// Every section, in list order.
    pub const ALL: [ProjectSection; 4] = [
        ProjectSection::General,
        ProjectSection::Parameters,
        ProjectSection::SchematicDefaults,
        ProjectSection::Setup,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProjectSection::General => "General",
            ProjectSection::Parameters => "Parameters",
            ProjectSection::SchematicDefaults => "Schematic defaults",
            ProjectSection::Setup => "Project setup",
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
    /// The name typed for a new custom parameter, not yet added.
    pub new_parameter_name: String,
    /// Schematic default fields as typed, until the next undo, redo or
    /// section switch shows the stored values again.
    pub default_drafts:
        std::collections::HashMap<signex_types::project::SchematicDefaultField, String>,
    /// The Project setup section as edited (#2232), until the next undo,
    /// redo or section switch shows the stored values again.
    pub setup_draft: Option<crate::app::NewProjectState>,
    /// The agent journal line cap as typed (#2253), until the next undo, redo
    /// or section switch shows the stored value again.
    pub journal_lines_draft: Option<String>,
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
    DescriptionEdited {
        project_id: ProjectId,
        text: String,
    },
    /// The designator standard picked in the General section (#1232);
    /// `None` is "Library designator" (no standard).
    DesignatorStandardPicked {
        project_id: ProjectId,
        standard: Option<signex_types::designator::DesignatorStandard>,
    },
    /// A parameter's value as typed in the Parameters section (#1080).
    ParameterEdited {
        project_id: ProjectId,
        name: String,
        value: String,
    },
    ParameterRemoved {
        project_id: ProjectId,
        name: String,
    },
    NewParameterNameEdited {
        project_id: ProjectId,
        text: String,
    },
    /// Add the typed custom parameter, with an empty value.
    ParameterAdded {
        project_id: ProjectId,
    },
    /// A schematic default as typed (#2026); empty clears it.
    SchematicDefaultEdited {
        project_id: ProjectId,
        field: signex_types::project::SchematicDefaultField,
        text: String,
    },
    /// The schematic default font as typed (#2026); empty clears it.
    SchematicDefaultFontEdited {
        project_id: ProjectId,
        text: String,
    },
    /// The wire line style default picked (#2026); `None` is built-in.
    SchematicDefaultLineStylePicked {
        project_id: ProjectId,
        line: Option<signex_types::net_class::NetLineStyle>,
    },
    /// The shape fill default picked (#2026); `None` is built-in.
    SchematicDefaultShapeFillPicked {
        project_id: ProjectId,
        fill: Option<signex_types::schematic::FillType>,
    },
    /// An edit in the Project setup section (#2232), in the New Project
    /// dialog's own message words.
    SetupEdited {
        project_id: ProjectId,
        msg: crate::app::NewProjectMsg,
    },
    /// A checkbox of the Agent journals group (#2253), with the settings it
    /// leads to.
    AgentJournalPicked {
        project_id: ProjectId,
        settings: signex_types::agent_journal::AgentJournalSettings,
    },
    /// The agent journal line cap as typed (#2253).
    AgentJournalLinesEdited {
        project_id: ProjectId,
        text: String,
    },
    /// Rename… in the General section: the project-tree Rename flow.
    RenameRequested {
        project_id: ProjectId,
    },
}
