//! File > New Project (#2232): the setup dialog, its draft and the command
//! that creates the same project without it. Design and file formats:
//! `docs/PROJECT_SETUP.md`.

mod command;
mod profiles;
mod state;
mod target;
mod update;

pub(crate) use command::new_project_with_profile;
pub(crate) use profiles::{ProfileStored, store_profile};
pub use state::NewProjectState;
pub(crate) use state::{Blocker, NameProblem, RULE_FIELDS, SaveProfileProblem, check_name};
pub(crate) use target::{TargetProblem, TargetStatus};
pub(crate) use update::{NAME_INPUT_ID, import_rule_file};

#[cfg(test)]
mod tests;

use signex_types::designator::DesignatorStandard;
use signex_types::project::{FabTemplate, PcbField, RuleTemplate, SetupProfile};
use signex_types::schematic::SheetOrientation;

impl From<NewProjectMsg> for crate::app::Message {
    fn from(msg: NewProjectMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::Setup(msg))
    }
}

/// Why an imported rule file was not used.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleReadError {
    /// The file could not be read; `detail` is the system reason.
    Read {
        path: String,
        detail: String,
    },
    File(signex_types::project::RuleFileError),
}

/// What happened in the New Project dialog.
#[derive(Debug, Clone)]
pub enum NewProjectMsg {
    NameEdited(String),
    LocationEdited(String),
    BrowseRequested,
    LocationPicked(Option<std::path::PathBuf>),
    MakeFolderToggled(bool),
    ProfilePicked(SetupProfile),
    CustomizeToggled,
    FabPicked(FabTemplate),
    ImportRequested,
    /// The picked rule file was read: `None` when the pick was cancelled.
    RulesFileRead(Option<Result<RuleTemplate, RuleReadError>>),
    /// A dropdown of the dialog opened (`true`) or closed: while one is open,
    /// Enter belongs to it and does not submit.
    DropdownToggled(bool),
    /// The stepper moved by this many layers.
    LayersStepped(i8),
    RuleEdited(PcbField, String),
    SheetSizePicked(String),
    SheetOrientationPicked(SheetOrientation),
    TitleBlockPicked(bool),
    /// A saved user profile's card was picked, by id.
    UserProfilePicked(String),
    /// "Save as profile..." opened its name field.
    SaveProfileOpened,
    SaveProfileNameEdited(String),
    /// The name field's Save: store the current values as a user profile.
    SaveProfileConfirmed,
    SaveProfileCancelled,
    /// Remove on a user profile's card, by id: asks to confirm.
    UserProfileRemoveRequested(String),
    /// The confirm's Remove: the asked profile leaves the user's settings.
    UserProfileRemoveConfirmed,
    /// The confirm's Keep.
    UserProfileRemoveCancelled,
    DesignatorsPicked(DesignatorStandard),
    /// The disk check of generation `generation` finished.
    TargetChecked {
        generation: u64,
        status: TargetStatus,
    },
    SubmitRequested,
    CancelRequested,
}
