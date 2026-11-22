//! The generated footprint features (mockup row 124, #2834).

use signex_library::primitive::footprint::{
    Footprint, FpGraphic, FpGraphicKind, Pad, PadShape, Polygon,
};

use super::{bake_generated, body_extent};

fn pad(number: &str, position: [f64; 2]) -> Pad {
    Pad {
        number: number.into(),
        shape: PadShape::Rect,
        size: [1.0, 0.6],
        position,
        ..Pad::default()
    }
}

/// Two pads straddling the left and right edges of a 4 x 2 mm body.
fn part() -> Footprint {
    let mut fp = Footprint::empty("PART");
    fp.pads = vec![pad("1", [-2.0, 0.0]), pad("2", [2.0, 0.0])];
    fp.body_3d.outline = Some(Polygon::new(vec![
        [-2.0, -1.0],
        [2.0, -1.0],
        [2.0, 1.0],
        [-2.0, 1.0],
    ]));
    fp
}

fn lines(strokes: &[FpGraphic]) -> Vec<([f64; 2], [f64; 2])> {
    strokes
        .iter()
        .filter_map(|g| match g.kind {
            FpGraphicKind::Line { from, to } => Some((from, to)),
            _ => None,
        })
        .collect()
}

fn dots(strokes: &[FpGraphic]) -> Vec<[f64; 2]> {
    strokes
        .iter()
        .filter_map(|g| match g.kind {
            FpGraphicKind::Circle { center, .. } => Some(center),
            _ => None,
        })
        .collect()
}

#[test]
fn features_that_are_off_generate_nothing() {
    let mut fp = part();
    bake_generated(&mut fp);
    assert!(fp.generated.is_default());
}

#[test]
fn the_silk_outline_is_clipped_at_each_pad_by_the_clearance() {
    let mut fp = part();
    fp.generated.silk_outline = true;
    fp.generated.pin_one_marker = false;
    fp.generated.silk_line_width_mm = 0.1;
    fp.generated.pad_clearance_mm = 0.2;
    bake_generated(&mut fp);
    let strokes = lines(&fp.generated.silk);
    // Top, bottom, and two pieces on each side around the pad.
    assert_eq!(strokes.len(), 6, "{strokes:?}");
    // The pad reaches y = 0.3; clearance 0.2 plus half the 0.1 line.
    let left: Vec<_> = strokes
        .iter()
        .filter(|(from, to)| from[0] == -2.0 && to[0] == -2.0)
        .collect();
    assert_eq!(left.len(), 2);
    assert!((left[0].1[1] + 0.55).abs() < 1e-9, "{left:?}");
    assert!((left[1].0[1] - 0.55).abs() < 1e-9, "{left:?}");
}

#[test]
fn the_pin_one_dot_sits_outside_pad_one() {
    let mut fp = part();
    fp.generated.silk_outline = true;
    fp.generated.silk_line_width_mm = 0.1;
    fp.generated.pad_clearance_mm = 0.2;
    bake_generated(&mut fp);
    let dot = dots(&fp.generated.silk);
    assert_eq!(dot.len(), 1);
    // Pad 1's outer edge is x = -2.5; clearance 0.2 plus the dot's 0.1.
    assert!((dot[0][0] + 2.8).abs() < 1e-9, "{dot:?}");
    assert_eq!(dot[0][1], 0.0);
}

#[test]
fn without_a_pad_one_there_is_no_dot() {
    let mut fp = part();
    fp.pads[0].number = "A".into();
    fp.generated.silk_outline = true;
    bake_generated(&mut fp);
    assert!(dots(&fp.generated.silk).is_empty());
}

#[test]
fn the_assembly_outline_follows_the_body() {
    let mut fp = part();
    fp.generated.assembly_outline = true;
    bake_generated(&mut fp);
    assert_eq!(lines(&fp.generated.assembly).len(), 4);
    assert_eq!(body_extent(&fp), Some([-2.0, -1.0, 2.0, 1.0]));
}

#[test]
fn the_body_falls_back_to_the_drawn_fab_outline() {
    let mut fp = part();
    fp.body_3d.outline = None;
    fp.fab_f.push(FpGraphic::drawn(
        FpGraphicKind::Rectangle {
            from: [-1.0, -3.0],
            to: [1.0, 3.0],
        },
        0.1,
        false,
    ));
    assert_eq!(body_extent(&fp), Some([-1.0, -3.0, 1.0, 3.0]));
}

#[test]
fn without_a_body_no_outline_is_made_up() {
    let mut fp = part();
    fp.body_3d.outline = None;
    fp.generated.silk_outline = true;
    fp.generated.pin_one_marker = false;
    fp.generated.assembly_outline = true;
    bake_generated(&mut fp);
    assert!(fp.generated.silk.is_empty());
    assert!(fp.generated.assembly.is_empty());
}

#[test]
fn switching_a_feature_off_clears_its_strokes() {
    let mut fp = part();
    fp.generated.assembly_outline = true;
    bake_generated(&mut fp);
    fp.generated.assembly_outline = false;
    bake_generated(&mut fp);
    assert!(fp.generated.is_default());
}
