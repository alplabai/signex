//! Messages of a library import (#3113, #3114), folded into
//! `LibraryMessage::Import`. The Import Library dialog starts a run through
//! `Signex::start_library_import`; these carry what the worker reports.

use crate::library_import::ProgressEvent;
use crate::library_import::run_state::ImportFinished;

#[derive(Debug, Clone)]
pub enum LibraryImportMsg {
    /// The converter reported progress.
    ProgressReported(ProgressEvent),
    /// The run and its git commit finished.
    Finished(Box<ImportFinished>),
    /// The imported library was (or could not be) recorded as global.
    GlobalRecorded {
        file: std::path::PathBuf,
        result: Result<(), String>,
    },
    /// The user asked to stop the run; its result still arrives as
    /// [`LibraryImportMsg::Finished`].
    CancelRequested,
}
