//! The Location of the pre-placement form (#2532): typed coordinates and
//! the justification anchor, the cursor the untyped fields follow, and OK
//! placing the next object at the typed point.

use iced::Task;
use signex_types::schematic::Point;
use signex_widgets::LengthFormat;

use crate::panels::{PrePlacementKind, PrePlacementPositionMsg};

use super::super::super::*;

impl Signex {
    /// A typed coordinate, read in the panel's unit (Y from the sheet's page
    /// origin, #2034), or a picked justification anchor, which the ghost of
    /// a label or text follows.
    pub(super) fn handle_pre_placement_position(&mut self, msg: &PrePlacementPositionMsg) {
        let ctx = &self.document_view.panel_ctx;
        match msg {
            PrePlacementPositionMsg::FieldTyped(field, text) => {
                let stored = LengthFormat::new(&ctx.i18n, ctx.unit)
                    .parse(text)
                    .map(|mm| if field.is_y() { ctx.shown_y(mm) } else { mm });
                if let Some(pp) = self.document_view.panel_ctx.pre_placement.as_mut() {
                    pp.position.typed(*field, text.clone(), stored);
                }
            }
            PrePlacementPositionMsg::AnchorPicked(h, v) => {
                if let Some(pp) = self.document_view.panel_ctx.pre_placement.as_mut() {
                    pp.justify_h = *h;
                    pp.justify_v = *v;
                }
                let canvas = self.interaction_state.active_canvas_mut();
                if let Some(g) = &mut canvas.ghost_label {
                    g.justify = *h;
                }
                if let Some(g) = &mut canvas.ghost_text {
                    g.justify_h = *h;
                }
            }
        }
    }

    /// The anchor of the wire being drawn, if one is.
    pub(crate) fn wire_anchor(&self) -> Option<Point> {
        if self.interaction_state.wire_drawing {
            self.interaction_state.wire_points.last().copied()
        } else {
            None
        }
    }

    /// The cursor moved over the sheet: the untyped Location fields follow
    /// it, snapped as a click there would place.
    pub(crate) fn pre_placement_cursor_moved(&mut self, x: f64, y: f64) {
        if self.document_view.panel_ctx.pre_placement.is_none() {
            return;
        }
        let (x, y) = if self.interaction_state.current_tool == Tool::Wire {
            self.snap_wire_point(self.active_render_snapshot(), x, y)
        } else {
            self.snap_to_grid(x, y)
        };
        let anchor = self.wire_anchor();
        if let Some(pp) = self.document_view.panel_ctx.pre_placement.as_mut() {
            pp.cursor_x_mm = x;
            pp.cursor_y_mm = y;
            pp.position.wire_anchor = anchor;
        }
    }

    /// Forget the typed position, so the next object starts from the cursor.
    pub(crate) fn clear_typed_position(&mut self) {
        if let Some(pp) = self.document_view.panel_ctx.pre_placement.as_mut() {
            pp.position.clear_typed();
        }
    }

    /// OK / Enter with a typed position: place the next object there, as a
    /// click at that point would (no snapping: the typed point is exact).
    /// The wire tool places one straight wire from Start to End. The typed
    /// position is then forgotten.
    pub(crate) fn place_at_typed_position(&mut self) -> Task<Message> {
        let anchor = self.wire_anchor();
        let Some(pp) = self.document_view.panel_ctx.pre_placement.as_ref() else {
            return Task::none();
        };
        if !pp.position.is_typed() {
            return Task::none();
        }
        // A field that does not read would place at a point it does not
        // show: OK is refused, the field stays marked and the draft kept.
        if pp.position.has_invalid() {
            crate::diagnostics::log_warning(
                "Properties: a Location field is not a length; fix it or clear it to place",
            );
            return Task::none();
        }
        let cursor = Point::new(pp.cursor_x_mm, pp.cursor_y_mm);
        let is_wire = pp.kind == PrePlacementKind::Wire
            && self.interaction_state.current_tool == Tool::Wire
            && self.interaction_state.placement.is_none();
        let task = if is_wire {
            let start = pp.position.start(anchor.unwrap_or(cursor));
            let end = pp.position.end(cursor);
            if self.place_wire_segment(start, end) && self.interaction_state.wire_drawing {
                self.interaction_state.wire_points = vec![end];
                self.interaction_state.active_canvas_mut().wire_preview = vec![end];
            }
            Task::none()
        } else {
            let at = pp.position.start(cursor);
            self.click_with_tool(
                (at.x, at.y),
                (at.x, at.y),
                crate::app::handlers::canvas::WirePoint::EndsWire,
            )
        };
        self.clear_typed_position();
        task
    }
}
