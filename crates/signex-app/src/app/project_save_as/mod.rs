//! File ▸ Save Project As… (#2908): a copy of an open project in a new
//! folder, in this build's format, which then opens beside it. The copy is
//! planned in `update` (`plan`), written on the blocking pool (`write`) and
//! the original folder is never written. A newer-minor project's copy is
//! first compared with its source file (`dropped_keys`), and a key it would
//! leave out or change is named and agreed to before anything is written.
//! Rules: docs/PROJECT_SETUP.md, "Save Project As".

mod dropped_keys;
mod plan;
mod update;
mod write;

#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;

use std::path::PathBuf;

pub(crate) use update::NAME_INPUT_ID;

use crate::app::handlers::document_files::new_project::project_file_in;
use crate::app::new_project::{Blocker, NameProblem, TargetStatus, check_name};
use crate::app::state::ProjectId;

/// The open Save Project As dialog. Nothing is on disk until its Save.
#[derive(Debug, Clone)]
pub struct SaveProjectAsState {
    /// The project the copy is made of.
    pub(crate) project: ProjectId,
    /// Its name, for the subtitle.
    pub(crate) project_name: String,
    pub(crate) name: String,
    pub(crate) location: String,
    /// The token the project was read as, when it is read-only here (#2904).
    pub(crate) read_only_format: Option<String>,
    /// Listed documents with unsaved edits when the dialog opened: the copy
    /// takes them as last saved.
    pub(crate) unsaved: usize,
    pub(crate) target: TargetStatus,
    /// Bumped by every edit of the name or location; a check result for an
    /// older value is dropped.
    pub(crate) generation: u64,
    /// The `.snxprj` a Save is writing; Save and Cancel wait meanwhile.
    pub(crate) saving: Option<PathBuf>,
    /// Why the last Save failed, shown in the dialog.
    pub(crate) error: Option<String>,
    /// The keys of a newer-minor project's `.snxprj` the copy would leave
    /// out or change, found by the last Save, which wrote nothing. Shown as
    /// a warning; the copy is written only once they are agreed to.
    pub(crate) not_kept: Option<Vec<String>>,
}

impl SaveProjectAsState {
    /// The location as a path, `None` while empty.
    pub(crate) fn location_path(&self) -> Option<PathBuf> {
        let t = self.location.trim();
        (!t.is_empty()).then(|| PathBuf::from(t))
    }

    /// The `.snxprj` a Save writes: `<location>/<name>/<name>.snxprj`, the
    /// copy always in its own folder. `None` while the name is unusable or
    /// the location is empty.
    pub(crate) fn project_file(&self) -> Option<PathBuf> {
        let name = check_name(&self.name).ok()?;
        Some(project_file_in(&self.location_path()?, name, true))
    }

    /// Why Save is off, when the name or the disk check says so.
    pub(crate) fn blocked_by(&self) -> Option<Blocker> {
        if let Err(problem) = check_name(&self.name) {
            return Some(Blocker::Name(problem));
        }
        match &self.target {
            TargetStatus::Problem(p) => Some(Blocker::Target(p.clone())),
            _ => None,
        }
    }

    /// Save is available: a usable name, a checked and clear target, and no
    /// copy being written.
    pub(crate) fn can_save(&self) -> bool {
        self.blocked_by().is_none()
            && self.saving.is_none()
            && matches!(self.target, TargetStatus::Clear)
    }

    /// The name is empty: the path line asks for one instead of the
    /// reason line.
    pub(crate) fn name_is_empty(&self) -> bool {
        check_name(&self.name) == Err(NameProblem::Empty)
    }
}

/// What happened in the Save Project As dialog.
#[derive(Debug, Clone)]
pub enum SaveProjectAsMsg {
    /// Open the dialog on `project`, else on the active document's project.
    /// Also the Save Project As… of a read-only project's Save Failed card.
    Requested {
        project: Option<ProjectId>,
    },
    NameEdited(String),
    LocationEdited(String),
    BrowseRequested,
    LocationPicked(Option<PathBuf>),
    /// The disk check of generation `generation` finished.
    TargetChecked {
        generation: u64,
        status: TargetStatus,
    },
    SubmitRequested,
    /// The Save of a newer-minor project's copy stopped before writing: the
    /// copy would leave out or change `keys` of its `.snxprj`.
    KeysNotKeptFound {
        project_path: PathBuf,
        keys: Vec<String>,
    },
    /// The person agreed the copy goes without the keys the dialog names.
    KeysNotKeptAccepted,
    CancelRequested,
    /// The copy planned as `project_path` was written (the `.snxprj`), or
    /// not, with the reason.
    Written {
        project_path: PathBuf,
        result: Result<PathBuf, String>,
    },
}

impl From<SaveProjectAsMsg> for crate::app::Message {
    fn from(msg: SaveProjectAsMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::SaveAs(msg))
    }
}
