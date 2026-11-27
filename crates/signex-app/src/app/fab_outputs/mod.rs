//! File ▸ Fabrication Outputs (#2388): the dialog that writes the board's
//! Gerber, drill, pick-and-place and test netlist files with
//! `signex_output::fab::generate`, the IPC-2581 file and the ODB++ job, and
//! beside them the assembly drawings and the board STEP model (#2489).
//! Nothing reaches the disk until Generate; the files are built and written
//! on a worker, from a copy of the board as it is in memory. The command
//! with operands runs the same Generate with no dialog (#2470, `command`).
//! Formats and rules: `docs/FABRICATION_OUTPUTS.md`.

mod command;
mod documents;
mod groups;
mod notes;
mod update;
mod write;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_command;
#[cfg(test)]
mod tests_dialog;

use std::path::PathBuf;

use signex_output::fab::{FabError, FabNote, FabOptions, OdbForm};
pub use signex_types::project::FabOutputKinds;

pub use command::FabCommandRun;
pub(crate) use command::fabrication_outputs;
pub(crate) use documents::{DocumentsJob, assembly_pdf_options, step_file_name};
pub use groups::{FabGroup, ticked_count};
pub use notes::{BoardNote, NoteSeverity};
pub(crate) use notes::{board_note_text, note_text};
pub(crate) use notes::{outcome_notes, problem_text, written_message, written_text};
#[cfg(test)]
pub(crate) use write::write_fab_outputs;
pub use write::{FabWritten, Listed};
pub(crate) use write::{GenerateJob, make_generate};

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
    Ipc2581,
    Odb,
    AssemblyPdf,
    AssemblySvg,
    Step,
}

impl FabOutput {
    /// Every kind, in the order the dialog lists them.
    pub const ALL: [FabOutput; 9] = [
        FabOutput::Gerber,
        FabOutput::Drill,
        FabOutput::Centroid,
        FabOutput::TestNetlist,
        FabOutput::Ipc2581,
        FabOutput::Odb,
        FabOutput::AssemblyPdf,
        FabOutput::AssemblySvg,
        FabOutput::Step,
    ];

    /// Whether `kinds` writes this kind.
    pub fn is_on(self, kinds: &FabOutputKinds) -> bool {
        match self {
            FabOutput::Gerber => kinds.gerber,
            FabOutput::Drill => kinds.drill,
            FabOutput::Centroid => kinds.centroid,
            FabOutput::TestNetlist => kinds.test_netlist,
            FabOutput::Ipc2581 => kinds.ipc2581,
            FabOutput::Odb => kinds.odb,
            FabOutput::AssemblyPdf => kinds.assembly_pdf,
            FabOutput::AssemblySvg => kinds.assembly_svg,
            FabOutput::Step => kinds.step,
        }
    }

    fn toggle(self, kinds: &mut FabOutputKinds) {
        let flag = match self {
            FabOutput::Gerber => &mut kinds.gerber,
            FabOutput::Drill => &mut kinds.drill,
            FabOutput::Centroid => &mut kinds.centroid,
            FabOutput::TestNetlist => &mut kinds.test_netlist,
            FabOutput::Ipc2581 => &mut kinds.ipc2581,
            FabOutput::Odb => &mut kinds.odb,
            FabOutput::AssemblyPdf => &mut kinds.assembly_pdf,
            FabOutput::AssemblySvg => &mut kinds.assembly_svg,
            FabOutput::Step => &mut kinds.step,
        };
        *flag = !*flag;
    }
}

/// What `signex_output::fab::generate` writes for `kinds`, files named
/// `name`.
pub fn fab_options(kinds: &FabOutputKinds, name: &str) -> FabOptions {
    let form = if kinds.odb_folder {
        OdbForm::Directory
    } else {
        OdbForm::Archive
    };
    FabOptions {
        name: name.to_string(),
        gerber: kinds.gerber,
        drill: kinds.drill,
        centroid: kinds.centroid,
        test_netlist: kinds.test_netlist,
        ipc2581: kinds.ipc2581,
        odb: kinds.odb.then_some(form),
        designator_standard: None,
        gerber_settings: Default::default(),
    }
}

/// What happened in the Fabrication Outputs dialog, namespaced under
/// `PcbMsg::FabOutputs` and routed to `dispatch_fab_outputs_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum FabOutputsMsg {
    /// `fabrication_outputs` ran on the board in view.
    Requested,
    OutputToggled(FabOutput),
    /// The box under the ODB++ row: the job as its folder tree instead of
    /// one archive.
    OdbFolderToggled,
    /// The whole-panel box (#2410): write the board's panel instead of
    /// the board alone.
    WholePanelToggled,
    NameEdited(String),
    FolderEdited(String),
    BrowseRequested,
    /// The folder picker closed; `None` when cancelled.
    FolderPicked(Option<PathBuf>),
    /// Generate, or Generate again after a run.
    GenerateRequested,
    /// Write `generation` finished.
    Written {
        generation: u64,
        result: Result<FabWritten, FabProblem>,
    },
    /// `fabrication_outputs` run from its operands (#2470) finished
    /// writing; its caller is answered.
    CommandWritten {
        run: Box<FabCommandRun>,
        result: Result<Listed, FabProblem>,
    },
    /// The success banner's Open folder: the written folder in the file
    /// manager.
    FolderOpenRequested,
    /// The file manager launch for `folder` failed; the reason.
    FolderOpenFailed {
        folder: PathBuf,
        why: String,
    },
    /// Cancel before a run, Close after one, Esc or the header's close.
    CancelRequested,
}

/// Why Generate wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabProblem {
    /// The board's files could not be made (no outline, a bad name, ...).
    Refused(FabError),
    /// The board's tab was closed before Generate.
    BoardGone,
    /// The folder or a file could not be written; the reason as the OS gave
    /// it.
    WriteFailed(String),
    /// The board's panel does not lay out (#2410); the reason.
    PanelRefused(String),
    /// The assembly PDF could not be drawn; the reason.
    AssemblyRefused(String),
    /// The STEP model could not be made; the reason.
    StepRefused(String),
}

/// The open dialog: the board, the files to write, where, and the last
/// Generate's outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct FabOutputsState {
    pub board: PathBuf,
    pub board_name: String,
    /// The file-name stem every file starts with.
    pub name: String,
    /// The kinds of file ticked. For a board its project names, the
    /// project's stored kinds are the source (`Signex::fab_output_kinds`);
    /// this is their copy as the dialog last set or read them.
    pub kinds: FabOutputKinds,
    /// The output folder as typed or picked.
    pub folder: String,
    /// The generation of the write in flight; `None` when idle.
    pub writing: Option<u64>,
    pub outcome: Option<Result<FabWritten, FabProblem>>,
    /// The board's panel as columns and rows when it has one (#2410).
    pub panel: Option<(u32, u32)>,
    /// Write the whole panel rather than the board alone; on by default for
    /// a board with a panel.
    pub whole_panel: bool,
    /// The local time the last outcome arrived, read in `update` when it
    /// did; the footer's "Generated HH:MM".
    pub generated_at: Option<chrono::NaiveTime>,
}

impl FabOutputsState {
    /// A dialog for the board at `board`: the default kinds of file, named
    /// after the board, into a `fabrication` folder beside it.
    pub fn new(board: PathBuf, board_name: String) -> Self {
        let name = board
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
            name,
            kinds: FabOutputKinds::default(),
            folder,
            writing: None,
            outcome: None,
            panel: None,
            whole_panel: false,
            generated_at: None,
        }
    }

    /// Whether a Generate has run and its outcome shows: the footer then
    /// offers Generate again and Close instead of Cancel and Generate.
    pub fn has_run(&self) -> bool {
        self.outcome.is_some()
    }

    /// Whether Generate can run with `kinds` ticked: no write in flight, a
    /// kind chosen, a folder and a name given.
    pub fn can_generate(&self, kinds: &FabOutputKinds) -> bool {
        self.writing.is_none()
            && FabOutput::ALL.iter().any(|k| k.is_on(kinds))
            && !self.folder.trim().is_empty()
            && !self.name.trim().is_empty()
    }

    /// The notes of the last successful Generate's fabrication files.
    pub fn notes(&self) -> &[FabNote] {
        match &self.outcome {
            Some(Ok(written)) => &written.notes,
            _ => &[],
        }
    }

    /// The notes of the last successful Generate's drawings and model.
    pub fn board_notes(&self) -> &[BoardNote] {
        match &self.outcome {
            Some(Ok(written)) => &written.board_notes,
            _ => &[],
        }
    }
}
