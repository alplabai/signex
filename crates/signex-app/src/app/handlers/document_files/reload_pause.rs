//! A reload of the sheet under a paused placement ends the pause (mockup row
//! 157); a reload of any other document leaves it.

use iced::Task;

use super::super::super::*;
use crate::app::restore_job::{RestoreFailure, RestoreJobDocument};

impl Signex {
    /// [`Self::install_reload_outcome`], then the end of a pause on that
    /// sheet. A failed read drops the sheet, so it ends the pause too; an
    /// engine that would not start keeps the older sheet and the pause.
    pub(crate) fn apply_reloaded_reporting(
        &mut self,
        idx: usize,
        outcome: Result<RestoreJobDocument, RestoreFailure>,
        restored_bytes: Option<crate::persist::SharedBytes>,
        commit: bool,
    ) -> (Task<Message>, bool, bool) {
        let path = self
            .document_state
            .tabs
            .get(idx)
            .map(|tab| tab.path.clone());
        let is_dropped = matches!(outcome, Err(RestoreFailure::NotReloaded { .. }));
        let (task, installed, repaired) =
            self.install_reload_outcome(idx, outcome, restored_bytes, commit);
        let ended = match path {
            Some(path) if installed || is_dropped => self.end_pause_on_sheet(|sheet| sheet == path),
            _ => Task::none(),
        };
        (Task::batch([task, ended]), installed, repaired)
    }
}
