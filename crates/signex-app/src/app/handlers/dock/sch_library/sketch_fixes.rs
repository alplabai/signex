//! The sketch fixes Properties offers in the editor in view: Remove and
//! Convert to reference on a conflict row (fp-15), and Close gap on an open
//! outline's check line (mockup row 190).

use super::*;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

impl Signex {
    /// `Some(task)` when `panel_msg` is a sketch fix, else `None`.
    pub(super) fn handle_sketch_fix_message(
        &mut self,
        panel_msg: &PanelMsg,
    ) -> Option<Task<Message>> {
        match panel_msg {
            PanelMsg::FpEditorConflictRemoveClicked { constraint } => {
                Some(self.fp_editor_remove_conflicting_constraint(*constraint))
            }
            PanelMsg::FpEditorConflictConvertClicked(constraint) => {
                Some(self.fp_editor_convert_conflicting_constraint(*constraint))
            }
            PanelMsg::FpEditorCloseGapClicked { keep, merge } => {
                let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) else {
                    return Some(Task::none());
                };
                Some(
                    self.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                        path,
                        msg: PrimitiveEdit::Footprint(
                            FootprintEditorMsg::SketchCloseGapRequested {
                                keep: *keep,
                                merge: *merge,
                            },
                        ),
                    })),
                )
            }
            _ => None,
        }
    }
}
