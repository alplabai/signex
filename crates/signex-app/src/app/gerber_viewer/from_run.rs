//! The viewer opened from an Export Job run (mockup row 193b): exactly the
//! files the run wrote that the viewer reads, with a banner that names the
//! run (profile, variant, time) and leads back to its summary. Opening the
//! same run again focuses the tab; files moved since leave the empty state
//! with a line saying where they were.

use std::path::PathBuf;

use iced::Task;

use crate::app::{Message, Signex};

/// The Export Job run the viewer's files came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOrigin {
    /// The job file.
    pub job: PathBuf,
    pub profile: Option<String>,
    pub variant: String,
    /// Local time the run started.
    pub started: String,
    /// The folder the run wrote.
    pub folder: PathBuf,
}

impl Signex {
    /// Open `paths`, written by the run `origin`, on the viewer tab; the
    /// same run already shown is focused instead.
    pub(crate) fn open_gerber_paths_from_run(
        &mut self,
        paths: Vec<PathBuf>,
        origin: RunOrigin,
    ) -> Task<Message> {
        let viewer = &self.document_state.gerber_viewer;
        let shown = viewer.from_run.as_ref() == Some(&origin)
            && (viewer.open.is_some() || viewer.reading.is_some());
        if shown && self.gerber_viewer_tab().is_some() {
            self.open_gerber_viewer_tab();
            return Task::none();
        }
        let task = self.open_gerber_paths(paths);
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.from_run = Some(origin);
        viewer.run_missing = false;
        task
    }

    /// The banner's close: the tab stays open with its files.
    pub(super) fn close_gerber_run_banner(&mut self) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.from_run = None;
        viewer.run_missing = false;
        Task::none()
    }

    /// Back to run summary: the run's Export Job tab, on its summary.
    pub(super) fn back_to_gerber_run(&mut self) -> Task<Message> {
        let Some(origin) = self.document_state.gerber_viewer.from_run.clone() else {
            return Task::none();
        };
        let task = self.open_output_job_tab(origin.job.clone());
        if let Some(ui) = self.output_jobs_mut().tabs.get_mut(&origin.job) {
            ui.page = crate::app::output_job::TabPage::Summary;
        }
        task
    }
}
