//! An off-thread `.snxprj` write and what it reports back (#1202, #859).
//!
//! `update` plans the write from the project as it is now; one worker job
//! creates the pending libraries, encodes the project with them listed and
//! writes the file. The `.snxprj` must come after the libraries it lists, so
//! they share one job instead of two chained ones: a single completion then
//! carries both results, and `update` applies them to the project only if it
//! is still the same one (`docs/architecture/path-ops.md`, "Project file save").

use std::path::PathBuf;

use signex_types::project::ProjectData;
use uuid::Uuid;

use super::{DocumentSaveJob, SaveOutcome};
use crate::app::CaseFold;
use crate::library::commands::{
    PendingLibrarySpec, create_pending_library, register_library_entry,
};
use crate::library::state::NewLibraryDefaults;

/// Everything the worker needs, captured by `update` before it starts.
pub struct ProjectWritePlan {
    pub project_id: crate::app::ProjectId,
    pub path: PathBuf,
    /// The project as it was when the save began.
    pub snapshot: ProjectData,
    pub fold: CaseFold,
    /// Libraries to create first, in a fixed order.
    pub pending: Vec<(Uuid, PendingLibrarySpec)>,
    pub defaults: NewLibraryDefaults,
}

/// One pending library the worker tried to create.
#[derive(Debug, Clone)]
pub struct LibraryCreated {
    pub library_id: Uuid,
    pub spec: PendingLibrarySpec,
    pub result: Result<(), String>,
}

/// Outcome of one `.snxprj` write, carried back to `update`.
#[derive(Debug, Clone)]
pub struct ProjectSaveDone {
    /// The loaded project the snapshot was taken from (never reused, so a
    /// project closed and reopened under the same path is a different one).
    pub project_id: crate::app::ProjectId,
    pub path: PathBuf,
    /// The write's ledger sequence: only the newest write begun for the
    /// file may mark the project clean.
    pub sequence: u64,
    /// The bytes handed to the writer: the project is clean afterwards only
    /// if it still encodes to exactly these. Empty when encoding failed.
    pub bytes: Vec<u8>,
    /// The libraries the worker tried to create, in order.
    pub libraries: Vec<LibraryCreated>,
    pub result: Result<SaveOutcome, String>,
}

/// The blocking half of a project save. Run it off the UI thread.
pub fn run_project_write(plan: ProjectWritePlan, job: DocumentSaveJob) -> ProjectSaveDone {
    let ProjectWritePlan {
        project_id,
        path,
        snapshot,
        fold,
        pending,
        defaults,
    } = plan;
    let sequence = job.sequence();
    let mut data = snapshot;
    let mut libraries = Vec::with_capacity(pending.len());
    for (library_id, spec) in pending {
        let result = create_pending_library(library_id, &spec, &defaults);
        if result.is_ok() {
            register_library_entry(&mut data, &spec.lib_path, library_id, fold);
        }
        libraries.push(LibraryCreated {
            library_id,
            spec,
            result: result.map_err(|error| error.to_string()),
        });
    }
    let (bytes, result) = match signex_types::project::encode_project(&data) {
        Ok(bytes) => {
            let result = job.with_bytes(bytes.clone()).run();
            (bytes, result)
        }
        Err(error) => (
            Vec::new(),
            Err(format!("encode project {}: {error}", path.display())),
        ),
    };
    if matches!(result, Ok(SaveOutcome::Written)) {
        sync_journal_ignore(&path, &data);
    }
    ProjectSaveDone {
        project_id,
        path,
        sequence,
        bytes,
        libraries,
        result,
    }
}

/// With the project file written, bring the managed `.gitignore` block in line
/// with "keep agent journals in git" (#2253). Not on toggle: the setting is not
/// saved until now, and the person may still discard it. A failure is said in
/// Messages; the save itself stands.
fn sync_journal_ignore(project_file: &std::path::Path, data: &ProjectData) {
    let Some(dir) = project_file.parent() else {
        return;
    };
    if let Some(notice) = journal_ignore_notice(dir, data.agent_journal.keep_in_git) {
        crate::diagnostics::log_warning(notice);
    }
}

/// The sync of the managed block for `dir`, and what to tell the person when
/// it failed or could not apply (`None` when it did, or there is nothing to do).
pub(crate) fn journal_ignore_notice(dir: &std::path::Path, keep: bool) -> Option<String> {
    use signex_types::project_vcs_files::{JournalIgnoreSync, sync_agent_journal_ignore};
    match sync_agent_journal_ignore(dir, keep) {
        Ok(JournalIgnoreSync::RepositoryAbove) if !keep => Some(format!(
            "[save] {} is inside a repository whose root is above it, so \"Keep agent journals in git\" \
             (off) was not applied: add .signex/agent-*.jsonl to that repository's .gitignore yourself",
            dir.display()
        )),
        Ok(_) => None,
        Err(error) => Some(format!(
            "[save] The project was saved, but its .gitignore could not be updated for the agent journal setting: {error}"
        )),
    }
}
