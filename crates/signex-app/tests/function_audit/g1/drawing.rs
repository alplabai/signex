//! Mockup row 48: a drawing shape's Properties.

use super::support::{Bench, bench, empty_sheet};
use signex_app::app::contracts::DrawingFieldEdit;
use signex_app::panels::{DrawingFieldId, PanelMsg};
use signex_types::schematic::{
    DrawingPlacement, FillType, Point, SchDrawing, SelectedItem, SelectedKind, StrokeColor,
};
use uuid::Uuid;

fn selected(drawing: SchDrawing) -> (Bench, Uuid) {
    let id = drawing.uuid();
    let mut sheet = empty_sheet();
    sheet.drawings.push(drawing);
    let mut b = bench(sheet);
    b.select_everything();
    assert_eq!(
        b.app.document_state.panel_ctx.selected_kind,
        Some(SelectedKind::Drawing),
        "the shape alone is selected"
    );
    (b, id)
}

fn rect_bench() -> (Bench, Uuid) {
    selected(SchDrawing::Rect {
        uuid: Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(30.0, 20.0),
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    })
}

/// The rectangle's width, height, border and fill.
fn rect(b: &Bench) -> (f64, f64, f64, FillType) {
    match &b.sheet().drawings[0] {
        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            ..
        } => (
            (end.x - start.x).abs(),
            (end.y - start.y).abs(),
            *width,
            *fill,
        ),
        other => panic!("not a rectangle: {other:?}"),
    }
}

fn typing(b: &mut Bench, field: DrawingFieldId, text: &str) {
    b.panel(PanelMsg::DrawingFieldTyping(field, text.to_string()));
}

#[test]
fn m48_a_typed_rectangle_width_resizes_it_and_undo_restores_it() {
    let (mut b, _) = rect_bench();
    typing(&mut b, DrawingFieldId::RectWidth, "40");
    assert_eq!(rect(&b).0, 40.0);
    assert!(b.dirty());
    b.undo();
    assert_eq!(rect(&b).0, 20.0);
    assert!(!b.dirty());
}

#[test]
fn m48_typing_a_width_in_several_keystrokes_is_one_undo_step() {
    let (mut b, _) = rect_bench();
    for text in ["4", "45", "45.", "45.5"] {
        typing(&mut b, DrawingFieldId::RectWidth, text);
    }
    assert_eq!(rect(&b).0, 45.5);
    b.undo();
    assert_eq!(rect(&b).0, 20.0, "one edit, one undo step");
    assert!(!b.dirty());
}

#[test]
fn m48_a_negative_size_is_refused() {
    let (mut b, _) = rect_bench();
    typing(&mut b, DrawingFieldId::RectWidth, "-5");
    assert_eq!(rect(&b).0, 20.0, "a negative width is not a width");
    typing(&mut b, DrawingFieldId::RectHeight, "-5");
    assert_eq!(rect(&b).1, 10.0, "a negative height is not a height");
    assert!(!b.dirty());
}

#[test]
fn m48_a_negative_border_is_refused() {
    let (mut b, _) = rect_bench();
    typing(&mut b, DrawingFieldId::RectBorder, "-1");
    assert_eq!(rect(&b).2, 0.2);
    assert!(!b.dirty());
}

#[test]
fn m48_text_that_is_not_a_number_changes_nothing_and_stays_in_the_box() {
    let (mut b, _) = rect_bench();
    typing(&mut b, DrawingFieldId::RectWidth, "wide");
    assert_eq!(rect(&b).0, 20.0);
    assert!(!b.dirty());
    let buffer = &b.app.document_state.panel_ctx.drawing_edit_buf;
    assert_eq!(
        buffer.get(&DrawingFieldId::RectWidth).map(String::as_str),
        Some("wide")
    );
}

#[test]
fn m48_fill_colour_and_placement_apply_and_each_undo() {
    let (mut b, id) = rect_bench();
    let red = StrokeColor {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    b.panel(PanelMsg::UpdateDrawingEdit(DrawingFieldEdit::Fill(
        FillType::Background,
    )));
    assert_eq!(rect(&b).3, FillType::Background);
    b.panel(PanelMsg::UpdateDrawingEdit(DrawingFieldEdit::StrokeColor(
        Some(red),
    )));
    assert!(matches!(
        &b.sheet().drawings[0],
        SchDrawing::Rect { stroke_color: Some(c), .. } if *c == red
    ));
    b.panel(PanelMsg::UpdateDrawingEdit(DrawingFieldEdit::Placement(
        DrawingPlacement::Above,
    )));
    assert_eq!(b.sheet().drawing_placement(id), DrawingPlacement::Above);

    b.undo();
    assert_eq!(b.sheet().drawing_placement(id), DrawingPlacement::Behind);
    b.undo();
    assert!(matches!(
        &b.sheet().drawings[0],
        SchDrawing::Rect {
            stroke_color: None,
            ..
        }
    ));
    b.undo();
    assert_eq!(rect(&b).3, FillType::None);
    assert!(!b.dirty());
}

#[test]
fn m48_a_locked_shape_refuses_size_and_style_until_unlocked() {
    let (mut b, id) = rect_bench();
    let item = SelectedItem::new(id, SelectedKind::Drawing);
    b.panel(PanelMsg::ToggleItemLocked(item));
    assert!(b.sheet().drawings[0].is_locked());
    typing(&mut b, DrawingFieldId::RectWidth, "40");
    b.panel(PanelMsg::UpdateDrawingEdit(DrawingFieldEdit::Fill(
        FillType::Background,
    )));
    assert_eq!(rect(&b).0, 20.0);
    assert_eq!(rect(&b).3, FillType::None);

    b.panel(PanelMsg::ToggleItemLocked(item));
    typing(&mut b, DrawingFieldId::RectWidth, "40");
    assert_eq!(rect(&b).0, 40.0);
}

#[test]
fn m48_a_line_end_moves_only_that_end() {
    let (mut b, _) = selected(SchDrawing::Line {
        uuid: Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(30.0, 10.0),
        width: 0.2,
        stroke_color: None,
        locked: false,
    });
    typing(&mut b, DrawingFieldId::LineEndX, "50");
    let SchDrawing::Line { start, end, .. } = &b.sheet().drawings[0] else {
        panic!("not a line");
    };
    assert_eq!((start.x, end.x), (10.0, 50.0));
}

#[test]
fn m48_a_circle_radius_applies_and_a_negative_one_is_refused() {
    let (mut b, _) = selected(SchDrawing::Circle {
        uuid: Uuid::now_v7(),
        center: Point::new(40.0, 40.0),
        radius: 5.0,
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
    let radius = |b: &Bench| match &b.sheet().drawings[0] {
        SchDrawing::Circle { radius, .. } => *radius,
        other => panic!("not a circle: {other:?}"),
    };
    typing(&mut b, DrawingFieldId::CircleRadius, "7");
    assert_eq!(radius(&b), 7.0);
    typing(&mut b, DrawingFieldId::CircleRadius, "-7");
    assert_eq!(radius(&b), 7.0, "a negative radius is not a radius");
}
