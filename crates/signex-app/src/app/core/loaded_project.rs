//! One loaded project in the multi-project workspace.

use std::path::PathBuf;

use signex_types::project::ProjectData;

use crate::app::core::case_fold::CaseFold;
use crate::app::core::pending_library::PendingLibrarySpec;
use crate::app::core::project_id::ProjectId;

/// One loaded project in the multi-project workspace. `path` is the
/// canonical identity (`.snxprj` location on disk); `data`
/// is the parsed project contents. Multiple projects with different
/// `path`s coexist in `DocumentState.projects`; two identical `path`s
/// at once is a loader bug (existing `open_project_file` de-dupes).
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub id: ProjectId,
    pub path: PathBuf,
    pub data: ProjectData,
    /// Libraries the user has authored via the New Library flow but not
    /// yet committed to disk. The New Library dialog's Create button
    /// only registers an entry here + flips the project dirty bit;
    /// `commands::create_pending_library` runs at project-save time
    /// to actually write the `.snxlib`. Closes
    /// `feedback_no_disk_writes_without_user_save.md`'s "wait for
    /// explicit user save" invariant. Keyed by a temporary handle that
    /// becomes the eventual `library_id` once materialised.
    pub pending_libraries: std::collections::HashMap<uuid::Uuid, PendingLibrarySpec>,
    /// Path-key rules for this project's volume, probed once off-thread
    /// after load (#694); the host default until the answer arrives.
    pub case_fold: CaseFold,
    /// The volume probe has reported; the first answer wins.
    pub case_probed: bool,
    /// The probe DECIDED (not undecided, failed or a stat error): only then is
    /// `case_fold` a fact about the volume rather than the host default, and
    /// only then does document identity trust it (#1447).
    pub case_decided: bool,
}

impl LoadedProject {
    /// The project's directory — the *one* convention for "where this
    /// project's relative sheet filenames resolve from".
    ///
    /// Two conventions used to coexist: the parent of the `.snxprj` on disk,
    /// and the persisted `data.dir` string. The loader patches `data.dir` to
    /// the opened path's parent, so they agree for a freshly-loaded project —
    /// but a `LoadedProject` assembled any other way (project creation, a
    /// `.snxprj` that recorded an absolute `dir` that no longer matches) can
    /// have them disagree, and then ownership resolution succeeds while the
    /// sheet paths it produces do not exist. The export skips those pages
    /// silently. `path.parent()` is the only one that cannot be stale: it is
    /// derived from the file actually opened.
    pub fn dir(&self) -> &std::path::Path {
        self.path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
    }
}
