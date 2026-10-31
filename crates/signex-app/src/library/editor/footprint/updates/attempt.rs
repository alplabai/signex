//! Deciding whether a footprint-editor edit leaves a trace.
//!
//! Every mutating message is an attempt: history is recorded and the dirty
//! flag set only when the edit is kept. A sketch edit the solver rejects is
//! reverted by the dispatcher; recording it would clear the redo stack, may
//! evict the oldest undo step, and mark a document dirty that did not
//! change.

use crate::app::FootprintDocument;
use crate::app::FootprintHistorySnapshot;
use crate::library::messages::FootprintEditorMsg;

pub(super) struct EditAttempt {
    /// Pre-edit snapshot; `None` for messages that are pure UI state.
    snapshot: Option<FootprintHistorySnapshot>,
    rejections_before: u32,
    dirty_before: bool,
    /// A sketch drag tick: when it changed nothing it is not an edit.
    is_drag_tick: bool,
}

/// Messages that must not settle an owed drag solve: the ticks that owe
/// it, the answer that pays it, and cursor tracking that never touches the
/// sketch (it arrives between the ticks of every drag).
fn leaves_owed_solve_alone(msg: &FootprintEditorMsg) -> bool {
    matches!(
        msg,
        FootprintEditorMsg::SketchMovePoint { .. }
            | FootprintEditorMsg::SketchMoveLine { .. }
            | FootprintEditorMsg::SketchSolved { .. }
            | FootprintEditorMsg::CursorAt { .. }
            | FootprintEditorMsg::ViewScaleChanged { .. }
    )
}

impl EditAttempt {
    pub(super) fn begin(editor: &mut FootprintDocument, msg: &FootprintEditorMsg) -> Self {
        let is_drag_tick = matches!(
            msg,
            FootprintEditorMsg::SketchMovePoint { .. } | FootprintEditorMsg::SketchMoveLine { .. }
        );
        // Everything but a drag tick, the answer to one, and hover tracking
        // may solve inline or read the sketch (a mode switch does), and an
        // inline solve discards a drag batch it has not settled. So the
        // owed solve is settled first, whether or not the message records
        // an undo step. A no-op when nothing is owed.
        if !leaves_owed_solve_alone(msg) {
            editor.settle_pending_solve();
        }
        let snapshot = super::mutates_footprint_state(msg).then(|| {
            if is_drag_tick {
                editor.begin_tick_edit()
            } else {
                editor.begin_edit()
            }
        });
        Self {
            snapshot,
            rejections_before: editor.state.conflict.rejections(),
            dirty_before: editor.is_dirty(),
            is_drag_tick,
        }
    }

    /// The dirty flag as it was before the message.
    pub(super) fn dirty_before(&self) -> bool {
        self.dirty_before
    }

    /// Record the undo step, unless the edit was rejected and changed
    /// nothing, in which case history, redo and the dirty flag are exactly
    /// as they were before the message. Returns whether a step was recorded,
    /// and the oldest step it pushed off a full history, if any.
    pub(super) fn settle(
        self,
        editor: &mut FootprintDocument,
    ) -> (bool, Option<crate::app::FootprintHistorySnapshot>) {
        let Some(snapshot) = self.snapshot else {
            return (false, None);
        };
        let rejected = editor.state.conflict.rejections() != self.rejections_before;
        if (rejected || self.is_drag_tick) && editor.is_unchanged_since(&snapshot) {
            // Rows a failed cascade still owes keep the editor unsaved.
            editor.set_dirty(self.dirty_before || !editor.cascade_owed.is_empty());
            return (false, None);
        }
        (true, editor.commit_history(snapshot))
    }
}
