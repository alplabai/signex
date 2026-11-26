//! Library converters run through contract v1
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md`, #3113), and the imported library
//! turned into a committed, mounted snapshot (#3114).
//!
//! UI-free: the Import Library dialog calls into this module and shows what
//! it returns. Everything here that touches a process, the disk or git is
//! blocking and runs on a worker (`spawn_blocking`), never in `update`.
//!
//! - [`discovery`]: which converters are known and where they are.
//! - [`describe`]: `<converter> describe --json` and the picker filters.
//! - [`events`] / [`stream`]: the NDJSON progress stream and its summary.
//! - [`report`]: `import-report.json`.
//! - [`run`]: one `library` run, with the 120 s silence rule.
//! - [`snapshot`]: git setup, the import commit, the default location.
//! - [`run_state`]: what the app keeps about a run, and its worker half.
//! - [`error`]: every refusal, as translatable text.

pub mod confine;
pub mod describe;
pub mod discovery;
pub mod error;
pub mod events;
pub mod report;
pub mod run;
pub mod run_state;
pub mod snapshot;
pub mod stream;

#[cfg(all(test, unix))]
mod run_tests;
#[cfg(all(test, unix))]
mod snapshot_tests;

pub use describe::{ConverterDescription, InputKind, InputKindClass, PickerFilter};
pub use discovery::{KNOWN_CONVERTERS, KnownConverter, Located};
pub use error::ImportError;
pub use events::{ExitKind, ProgressEvent, Summary};
pub use report::ImportReport;
pub use run::{ConverterCommand, LibraryRunRequest, ModelsMode, RunFailure, RunLimits, RunMode, RunOutcome};
pub use snapshot::{ImportJob, ImportJobOutcome, MountScope, UpdateCheck};
