//! The end of a multi-sheet run (replace, re-prefix, assign footprints, a
//! linked-part change): every changed sheet other than the active one reads
//! unsaved and repaints, and the run's undo group is recorded. The active
//! sheet's own change is the gateway's active tail.

use std::path::PathBuf;

use signex_engine::{DocumentPatch, UndoGroupId};

use super::DocumentState;
use crate::app::UndoGroupSheets;
use crate::app::core::event::{ChangeCause, CoreEvent};

/// A finished multi-sheet run, as its finisher hands it to the edit tail.
#[derive(Debug)]
pub(crate) struct SheetRun {
    pub(crate) group: UndoGroupId,
    pub(crate) recorded: UndoGroupSheets,
    /// The changed sheets other than the active one, with their patches.
    pub(crate) members: Vec<(PathBuf, DocumentPatch)>,
    /// Move the ERC epoch even when the active sheet took no change.
    pub(crate) notes_edit: bool,
}

impl DocumentState {
    /// Settle `run`'s members and record its group. `active_changed`: the
    /// active sheet took a change, whose own settle notes the edit.
    #[must_use = "project the events"]
    pub(crate) fn settle_run(&mut self, run: SheetRun, active_changed: bool) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        for (path, patch) in run.members {
            // Recorded in the sheet's history: undo back to the save point
            // clears the mark. A member with no engine still reads unsaved.
            self.mark_history_edit_dirty(&path);
            self.show_member(&path, patch, ChangeCause::Edit, &mut events);
        }
        self.record_undo_group(run.group, run.recorded);
        if run.notes_edit && !active_changed {
            self.note_sheet_edit();
        }
        events
    }
}
