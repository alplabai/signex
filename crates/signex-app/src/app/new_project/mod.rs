//! File > New Project (#2232): the setup dialog, its draft and the command
//! that creates the same project without it. Design and file formats:
//! `docs/PROJECT_SETUP.md`.

mod card_edits;
mod command;
pub(crate) mod preview;
mod profiles;
mod state;
mod target;
mod update;

pub(crate) use card_edits::profile_name_id;
pub(crate) use command::new_project_with_profile;
pub use preview::{DialogPreview, SetupPreview, SetupPreviewMsg};
pub use profiles::ProfileUndo;
pub(crate) use profiles::{ProfileStored, store_profile};
pub use state::NewProjectState;
pub(crate) use state::{Blocker, NameProblem, RULE_FIELDS, SaveProfileProblem, check_name};
pub(crate) use target::{TargetProblem, TargetStatus, check_target};
pub(crate) use update::{NAME_INPUT_ID, import_rule_file};

#[cfg(test)]
mod card_tests;
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

/// The profile a card's pencil or menu acts on (#2536): a built-in tier, or
/// a saved profile by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileTarget {
    BuiltIn(SetupProfile),
    Saved(String),
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
    /// The pointer entered (`inside`) or left a built-in card: the card's
    /// pencil shows while it is hovered.
    CardHovered {
        card: SetupProfile,
        inside: bool,
    },
    /// The copper layer stepper on a card moved by this many layers; the
    /// card is picked on the new count (#2537).
    CardLayersStepped(SetupProfile, i8),
    /// A card's pencil or a saved profile's menu button opened its menu
    /// (`Some`), or the menu closed.
    ProfileMenuToggled(Option<ProfileTarget>),
    /// Edit profile...: the profile's values load into the fields below for
    /// editing (#2536).
    ProfileEditRequested(ProfileTarget),
    /// The edit bar's Save profile: the values shown are stored as the
    /// profile (a built-in's become its override).
    ProfileEditSaved,
    ProfileEditCancelled,
    /// Reset to default on an edited built-in: its override is dropped.
    ProfileResetRequested(SetupProfile),
    /// Duplicate (as my profile): a new saved profile with the source's
    /// values, named from `name`.
    ProfileDuplicateRequested {
        source: ProfileTarget,
        name: String,
    },
    /// Rename on a saved profile, by id: its name field opens.
    ProfileRenameRequested(String),
    ProfileRenameEdited(String),
    ProfileRenameConfirmed,
    ProfileRenameCancelled,
    /// A value, handle or expand button of a preview (mockup rows 21, 22).
    Preview(SetupPreviewMsg),
    /// The disk check of generation `generation` finished.
    TargetChecked {
        generation: u64,
        status: TargetStatus,
    },
    SubmitRequested,
    CancelRequested,
}
