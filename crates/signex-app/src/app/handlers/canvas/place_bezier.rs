//! The Bezier tool (#1267): click the start, C1, C2 and the end; the fourth
//! click places one cubic curve. A new curve takes the sheet's drawing
//! colour (no own colour; red only by an explicit pick; DECISIONS
//! 2026-11-19).

use signex_types::schematic::{Point, SchBezier, SchDrawing};

use super::super::super::*;
use super::pre_placement_shape;

/// The curve the four clicks `[start, c1, c2, end]` draw, at `width` mm.
pub(crate) fn bezier_from_clicks(points: [Point; 4], width: f64) -> SchBezier {
    let [start, c1, c2, end] = points;
    let mut b = SchBezier::new(uuid::Uuid::now_v7(), start, c1, c2, end);
    b.width = width;
    b
}

impl Signex {
    pub(super) fn bezier_tool_point(&mut self, p: Point) {
        let points = &mut self.interaction_state.annotation_tool.points;
        points.push(p);
        let clicked = if points.len() >= 4 {
            std::mem::take(points)
        } else {
            Vec::new()
        };
        if let [start, c1, c2, end] = clicked[..] {
            let (width, _) = pre_placement_shape(&self.document_state);
            let b = bezier_from_clicks([start, c1, c2, end], width);
            self.apply_engine_command(
                signex_engine::Command::PlaceSchDrawing {
                    drawing: SchDrawing::Bezier(b),
                },
                false,
                false,
            );
        }
        self.sync_annotation_draft();
    }
}
