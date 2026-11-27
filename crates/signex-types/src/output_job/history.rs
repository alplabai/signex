//! Run history kept in the `.snxout` (mockup row 114, "History"): one record
//! per run from the window, newest first, at most [`MAX_HISTORY`]. A
//! headless or agent run leaves the file as it is.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{OutputJob, OutputJobError, read_output_job, write_output_job};
use crate::format::UnknownFields;

/// The most runs a job keeps.
pub const MAX_HISTORY: usize = 20;

/// The most warning lines one record keeps.
pub const MAX_RECORD_WARNINGS: usize = 20;

/// What started a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    /// Run profile, or a run from the palette.
    Manual,
    /// Run Selected: the ticked outputs only.
    Selected,
    OnSave,
    KeepUpToDate,
    BeforeClose,
    /// A CLI or agent call.
    Command,
}

impl RunTrigger {
    pub fn id(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Selected => "selected",
            Self::OnSave => "on_save",
            Self::KeepUpToDate => "keep_up_to_date",
            Self::BeforeClose => "before_close",
            Self::Command => "command",
        }
    }

    /// Whether nobody pressed anything: the run works in the background.
    pub fn is_auto(self) -> bool {
        matches!(self, Self::OnSave | Self::KeepUpToDate | Self::BeforeClose)
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    Complete,
    /// Nothing was written.
    Stopped,
}

/// One run in the history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Local time the run started, `YYYY-MM-DD HH:MM:SS`.
    pub started: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub variant: String,
    pub trigger: RunTrigger,
    pub result: RunResult,
    #[serde(default)]
    pub outputs: u32,
    #[serde(default)]
    pub files: u32,
    #[serde(default)]
    pub warnings: u32,
    #[serde(default)]
    pub errors: u32,
    #[serde(default)]
    pub seconds: u32,
    /// Where the files went, `/`-separated, relative to the job file's
    /// folder.
    #[serde(default)]
    pub folder: String,
    /// The zip written beside the folder, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    /// Where a stopped run stopped: `validate`, or an output's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The files the Gerber Viewer reads, relative to `folder`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub viewable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warning_lines: Vec<String>,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl OutputJob {
    /// Keep `record` as the newest run; the oldest past [`MAX_HISTORY`] go.
    pub fn record_run(&mut self, mut record: RunRecord) {
        record.warning_lines.truncate(MAX_RECORD_WARNINGS);
        self.history.insert(0, record);
        self.history.truncate(MAX_HISTORY);
    }
}

/// Add `record` to the history of the job file at `path`, keeping the rest
/// of the file as it is on disk. Blocking IO: the app calls it from a task.
pub fn append_run_record(path: &Path, record: RunRecord) -> Result<(), OutputJobError> {
    let mut job: OutputJob = read_output_job(path)?;
    job.record_run(record);
    write_output_job(path, &job)
}
