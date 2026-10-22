//! Deciding whether a footprint-editor edit leaves a trace.
//!
//! Every mutating message is an attempt: history is recorded and the dirty
//! flag set only when the edit is kept. A sketch edit the solver rejects is
//! reverted by the dispatcher; recording it would clear the redo stack, may
//! evict the oldest undo step, and mark a document dirty that did not
//! change.

use crate::app::FootprintEditorState as Editor;
use crate::app::FootprintHistorySnapshot;
use crate::library::messages::FootprintEditorMsg;

pub(super) struct EditAttempt {
    /// Pre-edit snapshot; `None` for messages that are pure UI state.
    snapshot: Option<FootprintHistorySnapshot>,
    rejections_before: u32,
    dirty_before: bool,
}

impl EditAttempt {
    pub(super) fn begin(editor: &Editor, msg: &FootprintEditorMsg) -> Self {
        Self {
            snapshot: super::mutates_footprint_state(msg).then(|| editor.begin_edit()),
            rejections_before: editor.state.conflict.rejections(),
            dirty_before: editor.dirty,
        }
    }

    /// Record the undo step, unless the edit was rejected and changed
    /// nothing, in which case history, redo and the dirty flag are exactly
    /// as they were before the message.
    pub(super) fn settle(self, editor: &mut Editor) {
        let Some(snapshot) = self.snapshot else {
            return;
        };
        let rejected = editor.state.conflict.rejections() != self.rejections_before;
        if rejected && editor.is_unchanged_since(&snapshot) {
            editor.dirty = self.dirty_before;
            return;
        }
        editor.commit_history(snapshot);
    }
}
