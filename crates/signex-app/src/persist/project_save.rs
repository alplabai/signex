//! What an off-thread `.snxprj` write reports back (#1202).
//!
//! The UI thread encodes the project and asks the document-save ledger for a
//! write; this carries the outcome to `update`, which decides from the
//! project as it is then whether it is still unsaved.

use std::path::PathBuf;

use super::SaveOutcome;

/// Outcome of one `.snxprj` write, carried back to `update`.
#[derive(Debug, Clone)]
pub struct ProjectSaveDone {
    /// The loaded project the snapshot was taken from (never reused, so a
    /// project closed and reopened under the same path is a different one).
    pub project_id: crate::app::ProjectId,
    pub path: PathBuf,
    /// The bytes handed to the writer: the project is clean afterwards only
    /// if it still encodes to exactly these.
    pub bytes: Vec<u8>,
    pub result: Result<SaveOutcome, String>,
}
