//! Export Jobs and PNG pages (#1299, mockup rows 114, 167 and 193b).
//! An Export Job (`.snxout`) opens as a document tab (`tab/`, `view/`,
//! `document.rs`) that edits its outputs and run profiles and runs them as
//! a pipeline (`pipeline/`): Validate, each output, Package, Summary.
//! Profiles run on save, to keep outputs up to date, or before close
//! (`auto.rs`). `run_output_job` runs a job from the palette or a command;
//! the catalog commands that edit a job are in `commands.rs`;
//! `export_sheet_png` writes the pages of a schematic's project as PNG
//! files. Every file is read and written off the UI thread. Rules:
//! `docs/OUTPUT_JOBS.md`.

mod auto;
mod board_work;
mod commands;
mod document;
pub(crate) mod file;
mod gerber_commands;
mod operands;
mod pipeline;
mod png;
mod run;
mod tab;
mod view;
mod write;

#[cfg(test)]
mod auto_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod gerber_tests;
#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod png_tests;
#[cfg(test)]
mod tab_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;

use std::path::PathBuf;

use iced::Task;
use signex_types::output_job::{OUTPUT_JOB_EXTENSION, OutputJob};

use crate::app::agent::WaitingCommand;
use crate::app::{ExportMsg, Message, Signex};

pub use auto::AutoMsg;
pub(crate) use board_work::render_board;
pub(crate) use commands::{
    add_job_output, new_output_job, open_output_job, read_output_job_command, set_job_output,
    set_job_profile, set_job_triggers, stop_output_job, view_output_job_run,
};
pub use document::{JobDocMsg, ListPurpose};
pub(crate) use gerber_commands::{
    set_job_gerber_apertures, set_job_gerber_format, set_job_gerber_layers, set_job_gerber_units,
    set_job_gerber_zeros,
};
pub(crate) use operands::{export_sheet_png, run_output_job};
pub use pipeline::{Built, Packaged, RunMsg, ValidateOutcome};
pub use run::RunAsk;
pub use tab::{JobTabMsg, NamingMode, OutputJobUi, PackagingSwitch, TabPage, TriggerEdit};
pub use write::{CheckCounts, JobReport, WrittenOutput};

impl From<OutputJobMsg> for Message {
    fn from(msg: OutputJobMsg) -> Self {
        Self::Export(ExportMsg::OutputJob(msg))
    }
}

/// Whether a file named `file_name` is an Export Job: a Projects row that
/// opens its tab (mockup row 114).
pub fn opens_by_name(file_name: &str) -> bool {
    std::path::Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(OUTPUT_JOB_EXTENSION))
}

/// What happened to an Export Job, its tab, a run or a PNG export,
/// namespaced under `ExportMsg::OutputJob` and routed to
/// `dispatch_output_job_message`.
#[derive(Debug, Clone)]
pub enum OutputJobMsg {
    /// `run_output_job` from a key or the palette: pick the job file.
    RunRequested,
    /// The picker answered; `None` when it was cancelled.
    JobPicked(Option<PathBuf>),
    /// The job file was read off the UI thread.
    JobRead(Box<JobRead>),
    /// `export_sheet_png` from a key or the palette: pick the folder.
    PngRequested,
    /// The picker answered for the pages of `source`; `None` when cancelled.
    PngFolderPicked {
        source: PathBuf,
        folder: Option<PathBuf>,
    },
    /// The PNG pages were written off the UI thread.
    PngWritten(Box<JobWritten>),
    /// `open_output_job` (View ▸ Panels ▸ Export Jobs) from a key or the
    /// palette: the project's job in its tab.
    OpenRequested,
    /// `new_output_job` from a key or the palette: a new job in its tab.
    NewRequested,
    /// A run's steps.
    Run(RunMsg),
    /// The Export Job tab.
    Tab(JobTabMsg),
    /// A job file read or written.
    Document(JobDocMsg),
    /// Runs on save and before close.
    Auto(AutoMsg),
}

/// A job file read for a run.
#[derive(Debug, Clone)]
pub struct JobRead {
    pub(crate) job: PathBuf,
    /// The profile, the one output, Run Selected and what started it.
    pub(crate) ask: RunAsk,
    /// The headless or agent call answered when the run ends.
    pub(crate) waiting: Option<WaitingCommand>,
    pub(crate) result: Result<OutputJob, String>,
}

/// PNG pages were written, or why not.
#[derive(Debug, Clone)]
pub struct JobWritten {
    /// The folder of the pages.
    pub(crate) target: PathBuf,
    pub(crate) waiting: Option<WaitingCommand>,
    pub(crate) result: Result<JobReport, String>,
}

impl Signex {
    /// Route an Export Job, run, tab or PNG export message.
    pub(crate) fn dispatch_output_job_message(&mut self, msg: OutputJobMsg) -> Task<Message> {
        match msg {
            OutputJobMsg::RunRequested => self.pick_output_job(),
            OutputJobMsg::JobPicked(Some(job)) => self.start_picked_output_job(job),
            OutputJobMsg::JobPicked(None) | OutputJobMsg::PngFolderPicked { folder: None, .. } => {
                Task::none()
            }
            OutputJobMsg::JobRead(read) => self.handle_output_job_read(*read),
            OutputJobMsg::PngRequested => self.pick_sheet_png_folder(),
            OutputJobMsg::PngFolderPicked {
                source,
                folder: Some(folder),
            } => self.start_picked_sheet_png(&source, folder),
            OutputJobMsg::PngWritten(written) => self.handle_sheet_png_written(*written),
            OutputJobMsg::OpenRequested => self.open_project_output_job(),
            OutputJobMsg::NewRequested => self.list_project_jobs(ListPurpose::New),
            OutputJobMsg::Run(msg) => self.dispatch_job_run_message(msg),
            OutputJobMsg::Tab(msg) => self.dispatch_job_tab_message(msg),
            OutputJobMsg::Document(msg) => self.dispatch_job_doc_message(msg),
            OutputJobMsg::Auto(msg) => self.dispatch_auto_message(msg),
        }
    }

    /// The job the main window's active tab shows, when it is an Export Job.
    pub(crate) fn output_job_tab_in_window(&self, is_main: bool) -> Option<PathBuf> {
        let docs = &self.document_state;
        docs.tabs
            .get(docs.active_tab)
            .filter(|tab| is_main && tab.kind.is_output_job())
            .map(|tab| tab.path.clone())
    }
}
