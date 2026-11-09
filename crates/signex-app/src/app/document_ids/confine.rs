//! A headless run does not read a file a project merely names when that file
//! sits outside the project's folder (a reference like `../outside.snxsch`
//! or an absolute path) or reaches outside through a link (#2172).
//!
//! A project file is data a person or an agent may not trust: reading the
//! head or the whole of a sheet it points at would let it aim the app at any
//! file the user can read. The GUI opens such a sheet only when a person asks;
//! a headless run never reads it for a query, and says why.

use std::path::Path;

use crate::app::Signex;
use crate::app::state::LoadedProject;

/// Why `path` may not be read for `project`, or `Ok`: it must resolve, links
/// followed, to a place inside the folder the project file sits in.
pub(super) fn inside_project(project: &LoadedProject, path: &Path) -> Result<(), String> {
    let folder = std::fs::canonicalize(project.dir())
        .map_err(|e| format!("the project folder cannot be read: {e}"))?;
    // A file that is not there yet cannot be read either, so it is no leak.
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return Ok(());
    };
    if resolved.starts_with(&folder) {
        return Ok(());
    }
    Err(format!(
        "{} is outside the project folder (or a link out of it); a headless run does not read it",
        path.display()
    ))
}

/// Why `path` may not be read: not a regular file (a FIFO or device would
/// hang the read) or larger than [`crate::headless::MAX_FILE_BYTES`]. A file
/// that is not there is no leak.
fn readable_file(path: &Path) -> Result<(), String> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(());
    };
    if !meta.is_file() {
        return Err(format!(
            "{} is not a regular file; a headless run does not read it",
            path.display()
        ));
    }
    if meta.len() > crate::headless::MAX_FILE_BYTES {
        return Err(format!(
            "{} is larger than {} bytes; a headless run does not read it",
            path.display(),
            crate::headless::MAX_FILE_BYTES
        ));
    }
    Ok(())
}

impl Signex {
    /// Whether a query may read `path`, a file `project` references. Always
    /// for a person in a GUI session; for a headless run or an agent on the
    /// app's agent channel (#1924) only inside the project folder. A refusal
    /// is named in Messages.
    pub(crate) fn may_read_referenced(&self, project: &LoadedProject, path: &Path) -> bool {
        if self.headless_answers().is_none() && !self.agent.serving {
            return true;
        }
        match inside_project(project, path).and_then(|()| readable_file(path)) {
            Ok(()) => true,
            Err(reason) => {
                crate::diagnostics::log_warning(reason);
                false
            }
        }
    }
}
