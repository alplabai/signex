//! Paint order of drawings, on the GPU and the CPU replay alike (#1579).

use super::*;
use signex_types::schematic::StrokeColor;

const RED: StrokeColor = StrokeColor {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};

fn drawing_line(y: f64, width: f64) -> SchDrawing {
    SchDrawing::Line {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(6.0, y),
        end: Point::new(20.0, y),
        width,
        stroke_color: Some(StrokeColor {
            r: 0,
            g: 255,
            b: 0,
            a: 255,
        }),
        locked: false,
    }
}

/// An opaque red rectangle over `[10, 15] x [y0, y1]`.
fn red_rect(y0: f64, y1: f64) -> SchDrawing {
    SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, y0),
        end: Point::new(15.0, y1),
        width: 0.25,
        fill: FillType::Outline,
        stroke_color: Some(RED),
        locked: false,
    }
}

fn sheet_of(drawings: Vec<SchDrawing>) -> SchematicSheet {
    SchematicSheet {
        drawings,
        ..SchematicSheet::default()
    }
}

fn wire_across(y: f64) -> Wire {
    Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(6.0, y),
        end: Point::new(20.0, y),
        stroke_width: 0.4,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn is_red(px: [u8; 4]) -> bool {
    px[0] > 200 && px[1] < 60 && px[2] < 60
}

fn is_green(px: [u8; 4]) -> bool {
    px[1] > 200 && px[0] < 60 && px[2] < 60
}

/// A later filled rectangle covers an earlier line of another kind where they
/// overlap, and the earlier line shows where they do not: file order is paint
/// order, not pipeline order.
#[test]
fn a_later_rect_covers_an_earlier_drawing_line_on_gpu_and_cpu() {
    let mut gpu = gpu();
    let sheet = sheet_of(vec![drawing_line(12.0, 0.6), red_rect(10.0, 14.0)]);
    let inside = world(12.5, 12.0);
    let outside = world(8.0, 12.0);

    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let frame = frame_of(&mut gpu, layer, &sheet);
        assert!(
            is_red(frame.px(inside.0, inside.1)),
            "{name}: the later rect must cover the line, got {:?}",
            frame.px(inside.0, inside.1)
        );
        assert!(
            is_green(frame.px(outside.0, outside.1)),
            "{name}: the line shows outside the rect, got {:?}",
            frame.px(outside.0, outside.1)
        );
    }
}

/// The reverse file order puts the line on top: the order, not the kinds,
/// decides.
#[test]
fn an_earlier_rect_sits_under_a_later_drawing_line_on_gpu_and_cpu() {
    let mut gpu = gpu();
    let sheet = sheet_of(vec![red_rect(10.0, 14.0), drawing_line(12.0, 0.6)]);
    let inside = world(12.5, 12.0);

    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let frame = frame_of(&mut gpu, layer, &sheet);
        assert!(
            is_green(frame.px(inside.0, inside.1)),
            "{name}: the later line must cover the rect, got {:?}",
            frame.px(inside.0, inside.1)
        );
    }
}

/// Drawings are documentation: an opaque drawing rect never hides a wire.
#[test]
fn a_drawing_rect_sits_under_a_wire_on_gpu_and_cpu() {
    let mut gpu = gpu();
    let mut sheet = sheet_of(vec![red_rect(10.0, 14.0)]);
    sheet.wires.push(wire_across(12.0));
    let on_wire = world(12.5, 12.0);
    let on_rect = world(12.5, 10.8);

    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let frame = frame_of(&mut gpu, layer, &sheet);
        let wire = frame.px(on_wire.0, on_wire.1);
        assert!(!is_red(wire), "{name}: the rect covered the wire: {wire:?}");
        assert!(
            is_red(frame.px(on_rect.0, on_rect.1)),
            "{name}: the rect itself still draws"
        );
    }
}

/// A filled drawing circle keeps its fill and stroke colours apart.
#[test]
fn a_filled_drawing_circle_has_separate_fill_and_stroke_colours() {
    let mut gpu = gpu();
    let sheet = sheet_of(vec![SchDrawing::Circle {
        uuid: uuid::Uuid::now_v7(),
        center: Point::new(15.0, 12.0),
        radius: 3.0,
        width: 0.5,
        fill: FillType::Background,
        stroke_color: Some(RED),
        locked: false,
    }]);
    let centre = world(15.0, 12.0);
    let rim = world(18.0, 12.0);

    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
    let fill = to_u8(colors.body_fill);
    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let frame = frame_of(&mut gpu, layer, &sheet);
        let got = frame.px(centre.0, centre.1);
        assert!(
            (0..3).all(|c| got[c].abs_diff(fill[c]) <= 24),
            "{name}: centre {got:?} is not the fill colour {fill:?}"
        );
        assert!(
            is_red(frame.px(rim.0, rim.1)),
            "{name}: the rim is the stroke colour, got {:?}",
            frame.px(rim.0, rim.1)
        );
    }
}

/// A drawing placed above the circuit covers a wire; the same drawing left
/// behind does not.
#[test]
fn an_above_drawing_covers_a_wire_and_a_behind_one_does_not_on_gpu_and_cpu() {
    let mut gpu = gpu();
    let rect = red_rect(10.0, 14.0);
    let mut above = sheet_of(vec![rect.clone()]);
    above.drawing_placements.insert(
        rect.uuid(),
        signex_types::schematic::DrawingPlacement::Above,
    );
    above.wires.push(wire_across(12.0));
    let mut behind = sheet_of(vec![rect]);
    behind.wires.push(wire_across(12.0));
    let on_wire = world(12.5, 12.0);

    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let covered = frame_of(&mut gpu, layer, &above).px(on_wire.0, on_wire.1);
        assert!(
            is_red(covered),
            "{name}: the above rect must cover the wire: {covered:?}"
        );
    }
    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let under = frame_of(&mut gpu, layer, &behind).px(on_wire.0, on_wire.1);
        assert!(
            !is_red(under),
            "{name}: the behind rect covered the wire: {under:?}"
        );
    }
}

/// Above drawings still keep file order among themselves.
#[test]
fn above_drawings_keep_file_order_among_themselves_on_gpu_and_cpu() {
    let mut gpu = gpu();
    let (line, rect) = (drawing_line(12.0, 0.6), red_rect(10.0, 14.0));
    let mut sheet = sheet_of(vec![line.clone(), rect.clone()]);
    for d in [&line, &rect] {
        sheet
            .drawing_placements
            .insert(d.uuid(), signex_types::schematic::DrawingPlacement::Above);
    }
    let inside = world(12.5, 12.0);

    for (name, layer) in [("GPU", Layer::Stack), ("CPU", Layer::Cpu)] {
        let px = frame_of(&mut gpu, layer, &sheet).px(inside.0, inside.1);
        assert!(
            is_red(px),
            "{name}: later above drawing covers the earlier: {px:?}"
        );
    }
}
