//! Line, rectangle, circle and arc commits as plain-value methods (#1930).
//! Points are already snapped; `width` and `fill` come from the
//! pre-placement form. The click handler keeps the anchor state.

use signex_types::schematic::{FillType, Point, SchDrawing};

use super::super::super::*;

/// Below this a circle has no visible size and is not placed.
const MIN_CIRCLE_RADIUS_MM: f64 = 0.01;

impl Signex {
    pub(crate) fn place_line_at(&mut self, start: Point, end: Point, width: f64) -> bool {
        self.place_drawing(SchDrawing::Line {
            uuid: uuid::Uuid::now_v7(),
            start,
            end,
            width,
            stroke_color: None,
            locked: false,
        })
    }

    pub(crate) fn place_rect_at(
        &mut self,
        start: Point,
        end: Point,
        width: f64,
        fill: FillType,
    ) -> bool {
        self.place_drawing(SchDrawing::Rect {
            uuid: uuid::Uuid::now_v7(),
            start,
            end,
            width,
            fill,
            stroke_color: None,
            locked: false,
        })
    }

    /// Not placed (`false`) when `radius` is too small to see.
    pub(crate) fn place_circle_at(
        &mut self,
        center: Point,
        radius: f64,
        width: f64,
        fill: FillType,
    ) -> bool {
        if radius <= MIN_CIRCLE_RADIUS_MM {
            return false;
        }
        self.place_drawing(SchDrawing::Circle {
            uuid: uuid::Uuid::now_v7(),
            center,
            radius,
            width,
            fill,
            stroke_color: None,
            locked: false,
        })
    }

    pub(crate) fn place_arc_at(
        &mut self,
        [start, mid, end]: [Point; 3],
        width: f64,
        fill: FillType,
    ) -> bool {
        self.place_drawing(SchDrawing::Arc {
            uuid: uuid::Uuid::now_v7(),
            start,
            mid,
            end,
            width,
            fill,
            stroke_color: None,
            locked: false,
        })
    }

    fn place_drawing(&mut self, drawing: SchDrawing) -> bool {
        self.apply_engine_command(
            signex_engine::Command::PlaceSchDrawing { drawing },
            false,
            false,
        )
    }
}
