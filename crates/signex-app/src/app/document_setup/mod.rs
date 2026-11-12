//! New PCB and New Schematic setup dialogs (#2232, 2026-11-12): a board or
//! sheet added to a project opens a dialog prefilled from the project's
//! setup, with the profile cards and the Customize fields of the New Project
//! dialog. Create makes the document with the values shown; Cancel makes
//! nothing. Design: `docs/PROJECT_SETUP.md`, "New PCB and New Schematic".

mod command;
#[cfg(test)]
mod tests;
mod update;

pub(crate) use command::{new_pcb_document, new_schematic_sheet};

use signex_types::designator::DesignatorStandard;
use signex_types::project::SheetDefaults;

use crate::app::new_project::{NewProjectMsg, NewProjectState};
use crate::app::state::ProjectId;

/// Which document the dialog adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupKind {
    Pcb,
    Sheet,
}

/// The open dialog: what it adds, to which project, and the setup draft it
/// shares with the New Project dialog.
#[derive(Debug, Clone)]
pub struct DocumentSetupState {
    pub kind: SetupKind,
    pub project: ProjectId,
    /// The project's name, for the intro line.
    pub(crate) project_name: String,
    pub(crate) draft: NewProjectState,
}

impl DocumentSetupState {
    /// Create is available: every shown value is valid.
    pub(crate) fn can_create(&self) -> bool {
        match self.kind {
            SetupKind::Pcb => {
                self.draft.rules_valid() && self.draft.setup.pcb.problems().is_empty()
            }
            SetupKind::Sheet => true,
        }
    }

    /// The setup the sheet is made with.
    pub(crate) fn sheet_choice(&self) -> SheetChoice {
        SheetChoice {
            sheet: self.draft.setup.sheet.clone(),
            designators: self.draft.setup.designators,
        }
    }
}

/// What a new sheet is made with: its size, orientation and title block,
/// and the project's designator standard.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetChoice {
    pub sheet: SheetDefaults,
    pub designators: DesignatorStandard,
}

/// What happened in the New PCB or New Schematic dialog.
#[derive(Debug, Clone)]
pub enum DocumentSetupMsg {
    /// New PCB or New Schematic was asked for, for `project` or (`None`)
    /// the project in view.
    OpenRequested {
        kind: SetupKind,
        project: Option<ProjectId>,
    },
    /// An edit of the shared setup draft (cards, fields, Save as profile).
    Setup(NewProjectMsg),
    SubmitRequested,
    CancelRequested,
}

impl From<DocumentSetupMsg> for crate::app::Message {
    fn from(msg: DocumentSetupMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::DocumentSetup(msg))
    }
}
