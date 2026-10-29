//! Debounced, off-thread persistence building blocks shared by the dock
//! layout and the search-box preferences (#692, #859).

mod debounced;
mod document_save;
mod primitive_save;
mod project_save;
mod search_prefs;

pub use debounced::{DebouncedStore, DebouncedSync, PendingWrite, WriteBarrier};
pub use document_save::{
    AtomicFileSink, DocumentSaveDone, DocumentSaveJob, DocumentSaves, DocumentSink, InFlight,
    SameFile, SaveBarrier, SaveOutcome, SharedBytes,
};
pub use primitive_save::{PrimitiveSaveDone, PrimitiveWritten, SaveAsOrigin, WrittenContainer};
pub use project_save::{LibraryCreated, ProjectSaveDone, ProjectWritePlan, run_project_write};
pub use search_prefs::{
    SEARCH_PREFS_SAVE_DEBOUNCE, SearchPrefsChange, SearchPrefsMsg, SearchPrefsSync,
    SearchPrefsWrite,
};

#[cfg(test)]
pub(crate) mod run_task;
