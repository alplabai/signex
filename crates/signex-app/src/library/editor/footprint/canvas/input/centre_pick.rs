//! A centre pick armed from the Pattern form (Re-pick centre, or the kind
//! segment's Polar, #2596) takes the next left press under every sketch
//! tool, before any grab, glyph or stroke pick can (#3232). A press on a
//! Point picks that Point; elsewhere the press snaps as a placement does
//! and the centre is minted at the snapped place.

use iced::Point;
use iced::widget::canvas;

use crate::library::editor::footprint::snap;
use crate::library::editor::footprint::state::{EditorMode, ToolPending};
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::hit_test::sketch_snap;
use super::super::{FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// The press that answers an armed centre pick, as a sketch tool click
    /// naming the Point under the cursor. `None` when no pick is armed.
    pub(super) fn try_centre_pick(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if self.state.mode != EditorMode::Sketch
            || !matches!(
                self.state.tool_pending,
                ToolPending::RepickPolarCenter { .. }
            )
        {
            return None;
        }
        let raw_world = cstate.screen_to_world(cursor_pos);
        let point_hit = sketch_snap(self.sketch, cstate, raw_world);
        let picked =
            point_hit.and_then(|id| Some((id, snap::point_pos(id, self.sketch, self.state)?)));
        let (snap_id, (x_mm, y_mm)) = match picked {
            Some((id, at)) => (Some(id), at),
            None => {
                let result = snap::snap_cursor_among(
                    raw_world,
                    self.sketch,
                    self.state,
                    None,
                    false,
                    Some(self.object_targets(None)),
                );
                cstate.last_snap = Some(result);
                (None, result.pos)
            }
        };
        self.cache.clear();
        Some(
            canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::SketchToolClick {
                    x_mm,
                    y_mm,
                    snap_id,
                }),
            })
            .and_capture(),
        )
    }
}
