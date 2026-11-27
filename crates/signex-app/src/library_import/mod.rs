//! Library converters run through contract v1
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md`, #3113), and the imported library
//! turned into a committed, mounted snapshot (#3114).
//!
//! UI-free: the Import Library dialog calls into this module and shows what
//! it returns. Everything here that touches a process, the disk or git is
//! blocking and runs on a worker (`spawn_blocking`), never in `update`.
//!
//! - [`discovery`] — which converters are installed, found by what they answer.
//! - [`describe`] — `<converter> describe --json` and the picker filters.
//! - [`events`] / [`stream`] — the NDJSON progress stream and its summary.
//! - [`report`] — `import-report.json`.
//! - [`run`] — one `library` run, with the 120 s silence rule.
//! - [`output`] — the folder a new run takes, and what a failed one removes.
//! - [`snapshot`] — git setup, the import commit, the default location.
//! - [`staging`] — a new import's staged preview and its move into place.
//! - [`run_state`] — what the app keeps about a run.
//! - [`error`] — every refusal, as translatable text.

pub mod confine;
pub mod describe;
pub mod discovery;
mod empty_dir;
#[cfg(all(test, windows))]
mod empty_dir_tests;
pub mod error;
pub mod events;
#[cfg(windows)]
mod job;
#[cfg(all(test, windows))]
mod job_tests;
pub mod output;
mod place;
mod process;
pub mod report;
pub mod run;
pub mod run_state;
pub mod snapshot;
pub mod staging;
pub mod stream;

#[cfg(all(test, unix))]
mod discovery_tests;
#[cfg(test)]
mod output_tests;
#[cfg(all(test, unix))]
pub(crate) mod run_tests;
#[cfg(all(test, unix))]
mod snapshot_tests;
#[cfg(test)]
mod staging_tests;

pub use describe::{ConverterDescription, InputKind, InputKindClass, PickerFilter};
pub use discovery::{DiscoveredConverter, Discovery, DiscoveryCache, DiscoveryEnv};
pub use error::ImportError;
pub use events::{ExitKind, ProgressEvent, Summary};
pub use process::ProcessGroup;
pub use report::ImportReport;
pub use run::{
    ConverterCommand, LibraryRunRequest, ModelsMode, RunFailure, RunLimits, RunMode, RunOutcome,
};
pub use snapshot::{ImportJob, ImportJobOutcome, MountScope, UpdateCheck};
pub use staging::StagedLibrary;
