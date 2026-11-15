//! File ▸ Fabrication Outputs (#2388): the dialog that writes the board's
//! Gerber, drill, pick-and-place and test netlist files with
//! `signex_output::fab::generate`. Nothing reaches the disk until Generate;
//! the files are built and written on a worker, from a copy of the board as
//! it is in memory. Formats and rules: `docs/FABRICATION_OUTPUTS.md`.

mod update;
mod write;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use signex_i18n::{I18n, Localized, ids, t};
use signex_output::fab::{FabError, FabNote, FabOptions};

pub use write::FabWritten;
pub(crate) use write::write_fab_outputs;

impl From<FabOutputsMsg> for crate::app::Message {
    fn from(msg: FabOutputsMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::FabOutputs(msg))
    }
}

/// One kind of file the dialog can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabOutput {
    Gerber,
    Drill,
    Centroid,
    TestNetlist,
}

impl FabOutput {
    /// Every kind, in the order the dialog lists them.
    pub const ALL: [FabOutput; 4] = [
        FabOutput::Gerber,
        FabOutput::Drill,
        FabOutput::Centroid,
        FabOutput::TestNetlist,
    ];

    /// Whether `options` writes this kind.
    pub fn is_on(self, options: &FabOptions) -> bool {
        match self {
            FabOutput::Gerber => options.gerber,
            FabOutput::Drill => options.drill,
            FabOutput::Centroid => options.centroid,
            FabOutput::TestNetlist => options.test_netlist,
        }
    }

    fn toggle(self, options: &mut FabOptions) {
        let flag = match self {
            FabOutput::Gerber => &mut options.gerber,
            FabOutput::Drill => &mut options.drill,
            FabOutput::Centroid => &mut options.centroid,
            FabOutput::TestNetlist => &mut options.test_netlist,
        };
        *flag = !*flag;
    }
}

/// What happened in the Fabrication Outputs dialog, namespaced under
/// `PcbMsg::FabOutputs` and routed to `dispatch_fab_outputs_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum FabOutputsMsg {
    /// `fabrication_outputs` ran on the board in view.
    Requested,
    OutputToggled(FabOutput),
    NameEdited(String),
    FolderEdited(String),
    BrowseRequested,
    /// The folder picker closed; `None` when cancelled.
    FolderPicked(Option<PathBuf>),
    GenerateRequested,
    /// Write `generation` finished.
    Written {
        generation: u64,
        result: Result<FabWritten, FabProblem>,
    },
    CancelRequested,
}

/// Why Generate wrote nothing, or not everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabProblem {
    /// The board's files could not be made (no outline, a bad name, ...).
    Refused(FabError),
    /// The board's tab was closed before Generate.
    BoardGone,
    /// The folder or a file could not be written; the reason as the OS gave
    /// it.
    WriteFailed(String),
}

/// The open dialog: the board, the files to write, where, and the last
/// Generate's outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct FabOutputsState {
    pub board: PathBuf,
    pub board_name: String,
    pub options: FabOptions,
    /// The output folder as typed or picked.
    pub folder: String,
    /// The generation of the write in flight; `None` when idle.
    pub writing: Option<u64>,
    pub outcome: Option<Result<FabWritten, FabProblem>>,
}

impl FabOutputsState {
    /// A dialog for the board at `board`: every kind of file, named after
    /// the board, into a `fabrication` folder beside it.
    pub fn new(board: PathBuf, board_name: String) -> Self {
        let stem = board
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = board
            .parent()
            .map(|dir| dir.join("fabrication").display().to_string())
            .unwrap_or_default();
        Self {
            board,
            board_name,
            options: FabOptions::all(stem),
            folder,
            writing: None,
            outcome: None,
        }
    }

    /// Whether Generate can run: no write in flight, a kind chosen, a folder
    /// and a name given.
    pub fn can_generate(&self) -> bool {
        self.writing.is_none()
            && FabOutput::ALL.iter().any(|k| k.is_on(&self.options))
            && !self.folder.trim().is_empty()
            && !self.options.name.trim().is_empty()
    }

    /// The notes of the last successful Generate.
    pub fn notes(&self) -> &[FabNote] {
        match &self.outcome {
            Some(Ok(written)) => &written.notes,
            _ => &[],
        }
    }
}

/// The line that says what Generate wrote and where.
pub(crate) fn written_text(written: &FabWritten, i18n: &I18n) -> Localized {
    let folder = written.folder.display().to_string();
    t!(
        i18n,
        ids::FAB_OUTPUTS_WRITTEN,
        "count" => written.files.len(),
        "folder" => folder.as_str()
    )
}

/// Why Generate wrote nothing.
pub(crate) fn problem_text(problem: &FabProblem, i18n: &I18n) -> Localized {
    match problem {
        FabProblem::Refused(FabError::NothingSelected) => {
            t!(i18n, ids::FAB_OUTPUTS_NOTHING_SELECTED)
        }
        FabProblem::Refused(FabError::BadName) => t!(i18n, ids::FAB_OUTPUTS_BAD_NAME),
        FabProblem::Refused(FabError::NoOutline) => t!(i18n, ids::FAB_OUTPUTS_NO_OUTLINE),
        FabProblem::BoardGone => t!(i18n, ids::FAB_OUTPUTS_BOARD_GONE),
        FabProblem::WriteFailed(why) => {
            t!(i18n, ids::FAB_OUTPUTS_WRITE_FAILED, "why" => why.as_str())
        }
    }
}

/// What the written files leave out.
pub(crate) fn note_text(note: FabNote, i18n: &I18n) -> Localized {
    let count = note.count();
    match note {
        FabNote::ZonesNotFilled { .. } => t!(i18n, ids::FAB_OUTPUTS_NOTE_ZONES, "count" => count),
        FabNote::TextsNotDrawn { .. } => t!(i18n, ids::FAB_OUTPUTS_NOTE_TEXTS, "count" => count),
        FabNote::VScoresNotWritten { .. } => {
            t!(i18n, ids::FAB_OUTPUTS_NOTE_V_SCORES, "count" => count)
        }
        FabNote::ItemsSkipped { .. } => t!(i18n, ids::FAB_OUTPUTS_NOTE_SKIPPED, "count" => count),
    }
}
