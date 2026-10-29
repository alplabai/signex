//! Rectangle field edits keep the rectangle's extent (#1367).

use signex_types::schematic::{FillType, Point, SchDrawing};
use uuid::Uuid;

use super::apply_drawing_edit;
use crate::app::contracts::DrawingFieldEdit;

fn rect(start: Point, end: Point) -> SchDrawing {
    SchDrawing::Rect {
        uuid: Uuid::new_v4(),
        start,
        end,
        width: 0.0,
        fill: FillType::default(),
        stroke_color: None,
        locked: false,
    }
}

#[test]
fn rect_start_x_keeps_height_when_start_is_below_end() {
    // start.y (20) > end.y (10): the rectangle is 10 mm tall.
    let current = rect(Point::new(0.0, 20.0), Point::new(5.0, 10.0));

    let edited = apply_drawing_edit(current, DrawingFieldEdit::RectStartX(3.0)).expect("rect edit");

    let SchDrawing::Rect { start, end, .. } = edited else {
        panic!("expected a rect");
    };
    assert_eq!((start.x, end.x), (3.0, 8.0));
    assert_eq!((start.y, end.y), (10.0, 20.0));
}
