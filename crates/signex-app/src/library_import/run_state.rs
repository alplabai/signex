//! What the app keeps about a library import, and the worker half that
//! runs one and registers the result as a global library.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::events::ProgressEvent;
use super::run::{RunFailure, RunMode};
use super::snapshot::{ImportJob, ImportJobOutcome};

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
    /// Whether recording the library as global worked; `None` when it was
    /// not asked for or nothing was imported.
    pub registered_global: Option<Result<(), String>>,
}

/// The import part of the library state.
#[derive(Debug, Default)]
pub struct ImportState {
    pub running: Option<ImportRun>,
    /// The last finished run, kept for the dialog's report view.
    pub last: Option<Box<ImportFinished>>,
}

/// Run `job` and, for a global import, record the library in
/// `global_libraries.toml`. Blocking; call from `spawn_blocking`.
pub fn run_job(
    job: &ImportJob,
    target: MountTarget,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> ImportFinished {
    let result = job.run(cancel, on_event);
    let registered_global = match (&result, &target) {
        (Ok(outcome), MountTarget::Global) => outcome.library_file.as_ref().map(|file| {
            crate::panels::components_panel::global_prefs::add_path(file.clone()).map(|_| ())
        }),
        _ => None,
    };
    ImportFinished {
        result,
        target,
        registered_global,
    }
}
