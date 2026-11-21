//! The core half of the schematic load path (epic #2326, slice 3): register
//! a tab, install or replace a sheet's engine, settle an opened sheet, keep a
//! restored document's id. Each step touches `DocumentState` only and hands
//! back the Messages lines it raises as notices; the load wrappers draw the
//! result through `app::projector`.
//!
//! Only the load wrappers in `scripts/document-load-wrappers.txt` call in
//! here (#2704); `scripts/check_document_load_seam.sh` keeps it that way and
//! keeps this module off the GUI's state and the diagnostics log.

mod install;
mod open;
mod restore;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use super::DocumentState;
use crate::app::core::event::Notice;

impl DocumentState {
    /// The active tab's path. Private: the module's own reads only, so it
    /// stays off the wrapper guard's list of core functions.
    fn active_tab_path(&self) -> Option<PathBuf> {
        self.tabs.get(self.active_tab).map(|tab| tab.path.clone())
    }
}

/// An engine install: whether the new engine went in, and the lines for
/// Messages a failed one raises.
#[derive(Debug)]
#[must_use = "log the notices"]
pub(crate) struct SheetInstall {
    pub(crate) installed: bool,
    pub(crate) notices: Vec<Notice>,
}

impl SheetInstall {
    fn installed() -> Self {
        Self {
            installed: true,
            notices: Vec::new(),
        }
    }

    fn failed(notice: Option<Notice>) -> Self {
        Self {
            installed: false,
            notices: notice.into_iter().collect(),
        }
    }
}
