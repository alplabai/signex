//! Back Annotate (#1312): the board's designators and values carried to the
//! project's schematic sheets, as a reviewable change list. The mirror of
//! Update PCB: the list is `signex_engine::pcb::eco::diff_schematic`, read on
//! a worker (`read.rs`); Execute writes the ticked rows into every changed
//! sheet as one undo group, loading a sheet that is not open into memory and
//! leaving it unsaved (`apply.rs`, `app/sheet_write.rs`). Nothing is written
//! to disk. Entry points: Tools ▸ Annotation ▸ Back Annotate Schematics...,
//! Design ▸ Update Schematics in `<project>`, the palette. A rename that
//! would put a designator on two parts starts unticked (`duplicates.rs`).

mod apply;
pub(crate) mod duplicates;
pub(crate) mod read;
mod update;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::eco::{BackChangeList, BackGroup, BackKey};

pub use read::BackRead;

use crate::app::state::ProjectId;
use crate::app::update_pcb::Phase;

impl From<BackAnnotateMsg> for crate::app::Message {
    fn from(msg: BackAnnotateMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::BackAnnotate(msg))
    }
}

/// What happened in the Back Annotate dialog, namespaced under
/// `PcbMsg::BackAnnotate` and routed to `dispatch_back_annotate_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum BackAnnotateMsg {
    /// `back_annotate_schematics` ran for the document in view.
    Requested,
    /// The read of job `job` landed.
    Read {
        job: u64,
        result: Result<Arc<BackRead>, String>,
    },
    RowToggled {
        key: BackKey,
        on: bool,
    },
    GroupToggled {
        group: BackGroup,
        on: bool,
    },
    /// A group's heading was clicked: fold or open it.
    GroupFolded {
        group: BackGroup,
        folded: bool,
    },
    /// The Notes group's heading was clicked: fold or open it.
    NotesFolded(bool),
    /// The banner's Show link: the flagged rename row is brought into view
    /// (its group opens) and highlighted.
    DuplicateShown(BackKey),
    RefreshRequested,
    ValidateRequested,
    ExecuteRequested,
    /// The closed sheets of job `job` were stated again: the files that
    /// changed on disk since the read, or why the check failed.
    DiskChecked {
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    },
    CloseRequested,
}

/// The dialog, while open.
#[derive(Debug, Clone)]
pub struct BackAnnotateState {
    pub(crate) project: ProjectId,
    /// The project's file name, for the subtitle.
    pub(crate) project_name: String,
    /// The project's board; `None` when it names none.
    pub(crate) board: Option<PathBuf>,
    pub(crate) phase: Phase,
    /// The newest worker job; a result of another job is dropped.
    pub(crate) job: u64,
    pub(crate) read: Option<Arc<BackRead>>,
    /// The list shown, with the user's ticks.
    pub(crate) list: Option<BackChangeList>,
    /// The last Validate or a refused Execute: `Ok` or why.
    pub(crate) validation: Option<Result<(), String>>,
    /// The closed sheets are being stated before Execute writes. The ticks
    /// and Refresh are off meanwhile.
    pub(crate) checking: bool,
    /// The edits Execute planned, written exactly as planned once the
    /// closed sheets were stated again.
    pub(crate) pending: Option<PendingWrite>,
    /// Groups the user folded (`true`) or opened; every group starts open.
    pub(crate) folds: std::collections::BTreeMap<BackGroup, bool>,
    /// The Notes group was folded.
    pub(crate) notes_folded: bool,
    /// The row the banner's Show link points at, drawn highlighted.
    pub(crate) focused: Option<BackKey>,
}

impl BackAnnotateState {
    pub(crate) fn new(project: ProjectId, project_name: String, board: Option<PathBuf>) -> Self {
        Self {
            project,
            project_name,
            phase: if board.is_some() {
                Phase::Opening
            } else {
                Phase::NoBoard
            },
            board,
            job: 0,
            read: None,
            list: None,
            validation: None,
            checking: false,
            pending: None,
            folds: std::collections::BTreeMap::new(),
            notes_folded: false,
            focused: None,
        }
    }

    /// Whether `group` is folded now.
    pub fn is_folded(&self, group: BackGroup) -> bool {
        self.folds.get(&group).copied().unwrap_or(false)
    }

    /// The renames of the list shown that would put a designator on two
    /// parts, flagged on their rows (AV-11).
    pub fn duplicate_renames(
        &self,
    ) -> std::collections::BTreeMap<BackKey, duplicates::DuplicateRename> {
        self.list
            .as_ref()
            .map(duplicates::duplicate_renames)
            .unwrap_or_default()
    }

    /// The file name of the sheet that holds `part`, for the flag's caption.
    pub(crate) fn part_sheet_name(&self, part: &duplicates::PartKey) -> Option<String> {
        let read = self.read.as_ref()?;
        let file = read.files.get(part)?;
        Some(crate::app::update_pcb::file_name(&file.file))
    }

    /// The board's file name, for labels.
    pub(crate) fn board_name(&self) -> String {
        self.board
            .as_deref()
            .map(crate::app::update_pcb::file_name)
            .unwrap_or_default()
    }

    /// The list, while the dialog shows one.
    pub(crate) fn ready_list(&self) -> Option<&BackChangeList> {
        matches!(self.phase, Phase::Ready)
            .then_some(self.list.as_ref())
            .flatten()
    }

    /// The list shown, ticks included.
    pub fn list(&self) -> Option<&BackChangeList> {
        self.list.as_ref()
    }

    /// The list is ready to review.
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    /// Why there is no list, when the read or a Refresh failed.
    pub fn failure(&self) -> Option<&str> {
        match &self.phase {
            Phase::Failed(why) => Some(why),
            _ => None,
        }
    }

    /// Execute is stating the closed sheets; the ticks and Refresh wait.
    pub fn is_checking(&self) -> bool {
        self.checking
    }

    /// The newest worker job.
    pub fn job(&self) -> u64 {
        self.job
    }

    /// The last Validate or refused Execute.
    pub fn validation(&self) -> Option<&Result<(), String>> {
        self.validation.as_ref()
    }

    /// Start a new worker job: results of earlier jobs are dropped.
    pub(crate) fn next_job(&mut self) -> u64 {
        self.job += 1;
        self.job
    }
}

/// The writes Execute planned: the edits per sheet file (whether the file
/// is placed once, and its edits), the closed files it stated again, and how
/// many rows they come from.
#[derive(Debug, Clone)]
pub(crate) struct PendingWrite {
    pub(crate) edits:
        std::collections::BTreeMap<PathBuf, (bool, Vec<signex_engine::pcb::eco::SheetEdit>)>,
    pub(crate) checked: std::collections::BTreeSet<PathBuf>,
    pub(crate) rows: usize,
}
