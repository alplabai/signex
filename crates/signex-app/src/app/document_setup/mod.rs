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
use signex_types::project::{
    ProjectData, ProjectDefaults, ProjectSetup, ResolvedSetup, SheetDefaults, UserProfile,
};

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
    /// The project stored a designator standard when the dialog opened.
    pub(crate) project_had_designators: bool,
    /// The user picked a designator standard in this dialog.
    pub(crate) designators_picked: bool,
    /// The user picked a saved profile card in this dialog.
    pub(crate) profile_picked: bool,
    /// The setup the project saved, when it saved one (#2538): the cards
    /// are folded behind one summary row of it.
    pub(crate) project_setup: Option<ProjectSetup>,
    /// "Use another profile..." unfolded the cards under the summary row.
    pub(crate) other_profile_open: bool,
}

impl DocumentSetupState {
    /// The summary row of the project's saved setup shows (#2538).
    pub fn shows_project_summary(&self) -> bool {
        self.project_setup.is_some()
    }

    /// The profile cards (or, for a sheet, Your profiles) show: the project
    /// saved no setup, or "Use another profile..." unfolded them.
    pub fn shows_cards(&self) -> bool {
        self.project_setup.is_none() || self.other_profile_open
    }

    /// Whether Create is available, as the footer button shows it.
    pub fn create_enabled(&self) -> bool {
        self.can_create()
    }

    /// Create is available: every shown value is valid.
    pub(crate) fn can_create(&self) -> bool {
        match self.kind {
            SetupKind::Pcb => {
                self.draft.rules_valid() && self.draft.setup.pcb.problems().is_empty()
            }
            SetupKind::Sheet => true,
        }
    }

    /// The designator standard the dialog shows.
    pub fn draft_designators(&self) -> DesignatorStandard {
        self.draft.setup.designators
    }

    /// The saved profile card picked in the dialog, by id.
    pub fn draft_user_profile(&self) -> Option<&str> {
        self.draft.setup.user_profile.as_deref()
    }

    /// The setup the sheet is made with. The designator standard is the
    /// project's, so it is carried only when the user chose it here: picked
    /// it, or picked a profile while the project stored none.
    pub(crate) fn sheet_choice(&self) -> SheetChoice {
        let chosen =
            self.designators_picked || (self.profile_picked && !self.project_had_designators);
        SheetChoice {
            sheet: self.draft.setup.sheet.clone(),
            designators: chosen.then_some(self.draft.setup.designators),
        }
    }
}

/// What a new sheet is made with: its size, orientation and title block,
/// and, when chosen, the project's designator standard.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetChoice {
    pub sheet: SheetDefaults,
    pub designators: Option<DesignatorStandard>,
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
    /// The summary row's "Use another profile..." unfolded the cards, or
    /// "Keep project setup" folded them and took the project's setup back.
    OtherProfileToggled,
    SubmitRequested,
    CancelRequested,
}

impl From<DocumentSetupMsg> for crate::app::Message {
    fn from(msg: DocumentSetupMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::DocumentSetup(msg))
    }
}

/// The setup a new document of the project `data` starts from. A project
/// that stores no sheet defaults and names no saved profile gets the blank
/// sheet Add New always made (A4), not the New Project default.
pub(crate) fn new_document_setup(data: &ProjectData, users: &[UserProfile]) -> ResolvedSetup {
    let mut resolved =
        ProjectSetup::for_project(&data.project_defaults, data.designator_standard(), users);
    if data.project_defaults.sheet.is_none() && resolved.setup.user_profile.is_none() {
        let blank = crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc();
        resolved.setup.sheet = SheetDefaults {
            paper_size: blank.paper_size,
            show_title_block: blank.frame.show_title_block,
        };
    }
    resolved
}

/// Whether `defaults` hold a saved setup for a `kind` document: a named
/// profile, or the part that document takes (#2538).
pub(crate) fn has_saved_setup(defaults: &ProjectDefaults, kind: SetupKind) -> bool {
    defaults.profile.is_some()
        || match kind {
            SetupKind::Pcb => defaults.pcb.is_some(),
            SetupKind::Sheet => defaults.sheet.is_some(),
        }
}
