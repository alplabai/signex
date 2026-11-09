//! File > New Project (#2232): the setup dialog, its draft and the command
//! that creates the same project without it. Design and file formats:
//! `docs/PROJECT_SETUP.md`.

mod command;
mod state;
mod target;
mod update;

pub(crate) use command::new_project_with_profile;
pub use state::NewProjectState;
pub(crate) use state::{Blocker, NameProblem, RULE_FIELDS, check_name};
pub(crate) use target::{TargetProblem, TargetStatus};
pub(crate) use update::{NAME_INPUT_ID, import_rule_file};

#[cfg(test)]
mod tests;

use signex_types::designator::DesignatorStandard;
use signex_types::project::{FabTemplate, PcbField, RuleTemplate, SetupProfile};

impl From<NewProjectMsg> for crate::app::Message {
    fn from(msg: NewProjectMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::Setup(msg))
    }
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
    RulesFileRead(Option<Result<RuleTemplate, String>>),
    /// The stepper moved by this many layers.
    LayersStepped(i8),
    RuleEdited(PcbField, String),
    SheetSizePicked(String),
    TitleBlockPicked(bool),
    DesignatorsPicked(DesignatorStandard),
    /// The disk check of generation `generation` finished.
    TargetChecked {
        generation: u64,
        status: TargetStatus,
    },
    SubmitRequested,
    CancelRequested,
}
