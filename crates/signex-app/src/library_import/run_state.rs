//! What the app keeps about a library import.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::events::ProgressEvent;
use super::run::{RunFailure, RunMode};
use super::snapshot::ImportJobOutcome;

/// Where a finished import is mounted, settled when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountTarget {
    /// Recorded in `global_libraries.toml`.
    Global,
    /// Added to this project's (`.snxprj` path) libraries.
    Project(PathBuf),
}

/// The import in progress.
#[derive(Debug, Clone)]
pub struct ImportRun {
    pub output: PathBuf,
    pub mode: RunMode,
    pub target: MountTarget,
    pub cancel: Arc<AtomicBool>,
    /// The latest progress event, for the dialog's progress bar.
    pub progress: Option<ProgressEvent>,
}

/// A finished run, as it reaches `update`.
#[derive(Debug, Clone)]
pub struct ImportFinished {
    pub result: Result<ImportJobOutcome, RunFailure>,
    pub target: MountTarget,
}

/// The import part of the library state.
#[derive(Debug, Default)]
pub struct ImportState {
    pub running: Option<ImportRun>,
    /// The last finished run, kept for the dialog's report view.
    pub last: Option<Box<ImportFinished>>,
}
