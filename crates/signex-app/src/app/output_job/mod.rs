//! Export Jobs and PNG pages (#1299, mockup rows 167 and 193b).
//! `run_output_job` runs the outputs a `.snxout` file lists (every enabled
//! one, or the one it names) into the job's folder; `export_sheet_png`
//! writes the pages of a schematic's project as PNG files. The job file is
//! read and every file written off the UI thread; the bytes come from the
//! same scope as the PDF, BOM and netlist dialogs
//! (`handlers::menu::export::job_outputs`). Rules: `docs/OUTPUT_JOBS.md`.

mod operands;
mod png;
mod run;
mod write;

#[cfg(test)]
mod png_tests;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use iced::Task;
use signex_types::output_job::OutputJob;

use crate::app::agent::WaitingCommand;
use crate::app::{ExportMsg, Message, Signex};

pub(crate) use operands::{export_sheet_png, run_output_job};
pub use write::{JobReport, WrittenOutput};

impl From<OutputJobMsg> for Message {
    fn from(msg: OutputJobMsg) -> Self {
        Self::Export(ExportMsg::OutputJob(msg))
    }
}

/// What happened to an Export Job run or a PNG export, namespaced under
/// `ExportMsg::OutputJob` and routed to `dispatch_output_job_message`.
#[derive(Debug, Clone)]
pub enum OutputJobMsg {
    /// `run_output_job` from a key or the palette: pick the job file.
    RunRequested,
    /// The picker answered; `None` when it was cancelled.
    JobPicked(Option<PathBuf>),
    /// The job file was read off the UI thread.
    JobRead(Box<JobRead>),
    /// The job's files were built and written off the UI thread.
    JobWritten(Box<JobWritten>),
    /// `export_sheet_png` from a key or the palette: pick the folder.
    PngRequested,
    /// The picker answered for the pages of `source`; `None` when cancelled.
    PngFolderPicked {
        source: PathBuf,
        folder: Option<PathBuf>,
    },
    /// The PNG pages were written off the UI thread.
    PngWritten(Box<JobWritten>),
}

/// A job file read for a run.
#[derive(Debug, Clone)]
pub struct JobRead {
    pub(crate) job: PathBuf,
    /// The one output to write, by name.
    pub(crate) only: Option<String>,
    /// The headless or agent call answered when the run ends.
    pub(crate) waiting: Option<WaitingCommand>,
    pub(crate) result: Result<OutputJob, String>,
}

/// A run's files were written, or why not.
#[derive(Debug, Clone)]
pub struct JobWritten {
    /// The job file, or the folder of a PNG export.
    pub(crate) target: PathBuf,
    pub(crate) waiting: Option<WaitingCommand>,
    pub(crate) result: Result<JobReport, String>,
}

impl Signex {
    /// Route an Export Job or PNG export message.
    pub(crate) fn dispatch_output_job_message(&mut self, msg: OutputJobMsg) -> Task<Message> {
        match msg {
            OutputJobMsg::RunRequested => self.pick_output_job(),
            OutputJobMsg::JobPicked(Some(job)) => self.start_picked_output_job(job),
            OutputJobMsg::JobPicked(None) | OutputJobMsg::PngFolderPicked { folder: None, .. } => {
                Task::none()
            }
            OutputJobMsg::JobRead(read) => self.handle_output_job_read(*read),
            OutputJobMsg::JobWritten(written) => self.handle_output_job_written(*written),
            OutputJobMsg::PngRequested => self.pick_sheet_png_folder(),
            OutputJobMsg::PngFolderPicked {
                source,
                folder: Some(folder),
            } => self.start_picked_sheet_png(&source, folder),
            OutputJobMsg::PngWritten(written) => self.handle_sheet_png_written(*written),
        }
    }
}
