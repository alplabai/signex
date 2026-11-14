//! The corner a wire or bus run bends at (#2372): the flip and the mode
//! change, and what the status bar says about them.

use signex_i18n::{ids, t};
use signex_types::corner::{Bend, CornerRule};

use super::super::super::*;

impl Signex {
    /// Take `rule` for the run being drawn and every later one. Only the
    /// preview changes; nothing reaches the document or the undo history
    /// until a click commits a segment.
    pub(crate) fn set_corner_rule(&mut self, rule: CornerRule) {
        self.interaction_state.corner_rule = rule;
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
    }

    /// Whether a wire or bus run is open: its first point is placed and the
    /// next click commits a segment.
    pub(crate) fn is_drawing_wire_run(&self) -> bool {
        self.interaction_state.wire_drawing
            && crate::active_bar::shows_draw_mode_pill(self.interaction_state.current_tool)
    }

    /// The status bar note while the Wire or Bus tool is active: the corner
    /// the next click will draw.
    pub(crate) fn corner_status(&self) -> Option<String> {
        if !crate::active_bar::shows_draw_mode_pill(self.interaction_state.current_tool) {
            return None;
        }
        let id = match self.interaction_state.corner_rule.bend() {
            Bend::HorizontalFirst => ids::EDITORS_WIRE_CORNER_HORIZONTAL_FIRST,
            Bend::VerticalFirst => ids::EDITORS_WIRE_CORNER_VERTICAL_FIRST,
            Bend::DiagonalFirst => ids::EDITORS_WIRE_CORNER_DIAGONAL_FIRST,
            Bend::DiagonalLast => ids::EDITORS_WIRE_CORNER_DIAGONAL_LAST,
            Bend::Straight => ids::EDITORS_WIRE_CORNER_ANY_ANGLE,
        };
        Some(t!(self.ui_state.i18n, id).into_string())
    }
}
