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
//! per section (`general`, `sheets`, `variants`, ...); the shell (`view`)
//! lays out the section list beside the selected section, and `commands`
//! is the catalog surface of the sections (#1689-#1693).

mod agent_journals;
mod commands;
mod erc_annotation;
mod general;
mod libraries;
mod look;
mod open;
mod parameters;
mod requirements;
mod schematic_defaults;
mod setup;
mod sheets;
mod table;
mod update;
mod variants;
mod version_control;
mod view;

#[cfg(test)]
mod agent_journals_tests;
#[cfg(test)]
mod commands_tests;
#[cfg(test)]
mod open_command_tests;
#[cfg(test)]
mod save_point_tests;
#[cfg(test)]
mod schematic_defaults_tests;
#[cfg(test)]
mod sections_tests;
#[cfg(test)]
mod setup_tests;
#[cfg(test)]
mod undo_routing_tests;

pub(crate) use commands::{
    project_add_variant, project_move_library, project_move_sheet, project_remove_variant,
    project_rename_variant, project_set_active_variant, project_set_annotate_undo,
    project_set_version_control,
};
pub use general::ProjectGeneralFacts;
pub(crate) use open::project_tab_title;
pub use parameters::ParameterGroup;
pub use requirements::{RequirementsMsg, RequirementsTabState};
pub(crate) use requirements::{
    project_add_requirement, project_enable_requirement, project_remove_requirement,
    project_run_requirements, project_set_requirement,
};

use crate::app::state::ProjectId;

/// A section of the Project tab, in list order (#1084).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSection {
    #[default]
    General,
    /// Project parameters the sheets inherit (#1080).
    Parameters,
    /// Page order and what a new sheet starts from (#1689).
    Sheets,
    /// Variant definitions and the active variant (#1690).
    Variants,
    /// Mounted libraries, their state and order (#1691).
    Libraries,
    /// The project's ERC rules and annotate undo choice (#1692).
    ErcAnnotation,
    /// The project's repository and commit on save (#1693).
    VersionControl,
    /// Widths and sizes of unset schematic items (#2026).
    SchematicDefaults,
    /// What new PCBs and sheets start from (#2232).
    Setup,
    /// The project's requirements and their checks (#2339).
    Requirements,
}

impl ProjectSection {
    /// Every section, in list order.
    pub const ALL: [ProjectSection; 10] = [
        ProjectSection::General,
        ProjectSection::Parameters,
        ProjectSection::Sheets,
        ProjectSection::Variants,
        ProjectSection::Libraries,
        ProjectSection::ErcAnnotation,
        ProjectSection::VersionControl,
        ProjectSection::SchematicDefaults,
        ProjectSection::Setup,
        ProjectSection::Requirements,
    ];

    /// The section's name in the list.
    pub fn title(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            ProjectSection::General => ids::PROJECT_SECTION_GENERAL,
            ProjectSection::Parameters => ids::PROJECT_SECTION_PARAMETERS,
            ProjectSection::Sheets => ids::PROJECT_SECTION_SHEETS,
            ProjectSection::Variants => ids::PROJECT_SECTION_VARIANTS,
            ProjectSection::Libraries => ids::PROJECT_SECTION_LIBRARIES,
            ProjectSection::ErcAnnotation => ids::PROJECT_SECTION_ERC_ANNOTATION,
            ProjectSection::VersionControl => ids::PROJECT_SECTION_VERSION_CONTROL,
            ProjectSection::SchematicDefaults => ids::PROJECT_SECTION_SCHEMATIC_DEFAULTS,
            ProjectSection::Setup => ids::PROJECT_SECTION_SETUP,
            ProjectSection::Requirements => ids::PROJECT_SECTION_REQUIREMENTS,
        }
    }
}

/// A variant name being typed over in the Variants section (#1690): the
/// variant it renames and the text so far, committed on Enter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantRenameDraft {
    pub from: String,
    pub text: String,
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
    /// The project's Rules tab (#2398): sub-tab, search, the cell being
    /// typed in, the text view and the cached flags.
    pub rules: crate::app::RulesTabState,
    /// The newest edit of the project went to its rule history, not to
    /// `general_history`: what `undo document=<.snxprj>` steps first.
    pub rules_step_newest: bool,
    /// The name typed for a new variant, not yet added (#1690).
    pub new_variant_name: String,
    /// The variant name being typed over, until Enter renames it.
    pub variant_rename: Option<VariantRenameDraft>,
    /// Why the last variant edit was refused; cleared by the next edit.
    pub variant_problem: Option<signex_types::project::VariantProblem>,
    /// The Requirements section's drafts, refusal and last run (#2339).
    pub requirements: RequirementsTabState,
    /// The Parameters filter as typed (Graphite mockup m30).
    pub parameter_filter: String,
    /// The Parameters Title block group is folded shut.
    pub parameter_title_block_closed: bool,
    /// The Parameters Variables group is folded shut.
    pub parameter_variables_closed: bool,
    /// A variable name being typed over, `(from, text)`, until Enter
    /// renames it.
    pub parameter_rename: Option<(String, String)>,
    /// Enter refused the typed variable name; the reason is read again
    /// from the draft. Cleared by the next keystroke.
    pub parameter_rename_refused: bool,
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
    /// The Parameters filter as typed (Graphite mockup m30).
    ParameterFilterEdited {
        project_id: ProjectId,
        text: String,
    },
    /// A Parameters group header clicked: fold the group or open it.
    ParameterGroupToggled {
        project_id: ProjectId,
        group: ParameterGroup,
    },
    /// A variable's name as typed over in its row.
    ParameterRenameEdited {
        project_id: ProjectId,
        from: String,
        text: String,
    },
    /// Enter in a variable's name box: rename it to what was typed.
    ParameterRenameSubmitted {
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
    /// A Move up / Move down button of the Sheets table (#1689).
    SheetMoved {
        project_id: ProjectId,
        filename: String,
        step: signex_types::project::ListMove,
    },
    /// Add New ▸ Schematic in an empty Sheets section (#31): the project
    /// tree's New Schematic flow for the project.
    AddSchematicRequested {
        project_id: ProjectId,
    },
    /// The new variant name as typed (#1690).
    NewVariantNameEdited {
        project_id: ProjectId,
        text: String,
    },
    /// Add the typed variant.
    VariantAdded {
        project_id: ProjectId,
    },
    /// A variant's name as typed over in its row.
    VariantRenameEdited {
        project_id: ProjectId,
        from: String,
        text: String,
    },
    /// Enter in a variant's name box: rename it to what was typed.
    VariantRenameSubmitted {
        project_id: ProjectId,
    },
    VariantRemoved {
        project_id: ProjectId,
        name: String,
    },
    /// The Active check of a row; `None` is Base.
    ActiveVariantPicked {
        project_id: ProjectId,
        name: Option<String>,
    },
    /// A Move up / Move down button of the Libraries table (#1691), with the
    /// library's path as the project stores it.
    LibraryMoved {
        project_id: ProjectId,
        path: std::path::PathBuf,
        step: signex_types::project::ListMove,
    },
    /// The annotate undo choice (#1692); `None` follows Preferences.
    AnnotateUndoPicked {
        project_id: ProjectId,
        choice: Option<signex_types::project::AnnotateUndo>,
    },
    /// "Store ERC rules in this project": a copy of this machine's rules.
    ErcRulesStoreRequested {
        project_id: ProjectId,
    },
    /// The Commit every save check (#1693).
    VersionControlToggled {
        project_id: ProjectId,
        on: bool,
    },
    /// Enable Version Control…: the project tree's dialog.
    EnableVersionControlRequested {
        project_id: ProjectId,
    },
    /// The Requirements section (#2339).
    Requirements {
        project_id: ProjectId,
        msg: RequirementsMsg,
    },
}
