//! The sketch fixes Properties offers in the editor in view: Remove and
//! Convert to reference on a conflict row (Caner 2026-11-20), and Close gap on an open
//! outline's check line (mockup row 190).

use super::*;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::SketchFixMsg;

impl Signex {
    /// `Some(task)` when `panel_msg` is a sketch fix, else `None`.
    pub(super) fn handle_sketch_fix_message(
        &mut self,
        panel_msg: &PanelMsg,
    ) -> Option<Task<Message>> {
        let PanelMsg::SketchFix(fix) = panel_msg else {
            return None;
        };
        Some(match *fix {
            SketchFixMsg::ConflictRemoveClicked { constraint } => {
                self.fp_editor_remove_conflicting_constraint(constraint)
            }
            SketchFixMsg::ConflictConvertClicked(constraint) => {
                self.fp_editor_convert_conflicting_constraint(constraint)
            }
            SketchFixMsg::CloseGapClicked { keep, merge } => {
                self.send_to_footprint_editor(FootprintEditorMsg::SketchCloseGapRequested {
                    keep,
                    merge,
                })
            }
            SketchFixMsg::AddFilletsClicked => {
                self.send_to_footprint_editor(FootprintEditorMsg::SketchAddFilletsRequested)
            }
            SketchFixMsg::ProfileRolePicked { seed, role } => {
                self.send_to_footprint_editor(FootprintEditorMsg::SketchProfileRolePicked {
                    seed,
                    role,
                })
            }
            SketchFixMsg::BakePreviewClicked => {
                self.send_to_footprint_editor(FootprintEditorMsg::SketchBakePreviewToggled)
            }
            SketchFixMsg::BakeClicked => {
                self.send_to_footprint_editor(FootprintEditorMsg::SketchBakeRequested)
            }
            SketchFixMsg::EditInSketchClicked => {
                self.send_to_footprint_editor(FootprintEditorMsg::SetMode(
                    crate::library::editor::footprint::EditorMode::Sketch,
                ))
            }
        })
    }

    /// `msg` for the footprint editor in view; nothing without one.
    fn send_to_footprint_editor(&mut self, msg: FootprintEditorMsg) -> Task<Message> {
        let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) else {
            return Task::none();
        };
        self.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(msg),
        }))
    }
}
