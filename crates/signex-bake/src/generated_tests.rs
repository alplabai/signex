//! The generated footprint features (mockup row 124, #2834).

use signex_library::primitive::footprint::{
    Footprint, FpGraphic, FpGraphicKind, Pad, PadShape, Polygon,
};

use super::{bake_generated, body_extent, pin_one_dot_is_missing};

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

/// A pin 1 notch dips into the body: only the arc's real sweep counts.
#[test]
fn a_partial_fab_arc_bounds_the_body_by_its_sweep() {
    let mut fp = part();
    fp.body_3d.outline = None;
    fp.fab_f.push(FpGraphic::drawn(
        FpGraphicKind::Rectangle {
            from: [-1.5, -2.45],
            to: [1.5, 2.45],
        },
        0.1,
        false,
    ));
    fp.fab_f.push(FpGraphic::drawn(
        FpGraphicKind::Arc {
            center: [0.0, 2.45],
            radius: 0.5,
            start_deg: 180.0,
            end_deg: 360.0,
        },
        0.1,
        false,
    ));
    let [x0, y0, x1, y1] = body_extent(&fp).unwrap();
    for (got, want) in [(x0, -1.5), (y0, -2.45), (x1, 1.5), (y1, 2.45)] {
        assert!((got - want).abs() < 1e-9, "{got} != {want}");
    }
}

#[test]
fn an_arc_across_the_wrap_reaches_the_extreme_inside_its_sweep() {
    let mut fp = part();
    fp.body_3d.outline = None;
    // 300 -> 60 runs through 0: it reaches +x, and neither +y nor -y.
    fp.fab_f.push(FpGraphic::drawn(
        FpGraphicKind::Arc {
            center: [0.0, 0.0],
            radius: 2.0,
            start_deg: 300.0,
            end_deg: 60.0,
        },
        0.1,
        false,
    ));
    let [x0, y0, x1, y1] = body_extent(&fp).unwrap();
    let half_chord = 2.0 * 60f64.to_radians().sin();
    assert!((x0 - 1.0).abs() < 1e-9, "{x0}");
    assert!((x1 - 2.0).abs() < 1e-9, "{x1}");
    assert!((y0 + half_chord).abs() < 1e-9, "{y0}");
    assert!((y1 - half_chord).abs() < 1e-9, "{y1}");
}

fn inline_part(pad_one_at: usize) -> Footprint {
    let mut fp = Footprint::empty("INLINE");
    fp.pads = [-1.0, 0.0, 1.0]
        .iter()
        .enumerate()
        .map(|(i, &x)| Pad {
            number: if i == pad_one_at {
                "1".into()
            } else {
                format!("{}", i + 2)
            },
            shape: PadShape::Rect,
            size: [0.6, 1.0],
            position: [x, 0.0],
            ..Pad::default()
        })
        .collect();
    fp.generated.silk_outline = true;
    fp.generated.silk_line_width_mm = 0.12;
    fp.generated.pad_clearance_mm = 0.2;
    fp
}

/// A central pad 1 has neighbours on its x sides: the dot goes above or
/// below it, clear of their copper.
#[test]
fn the_pin_one_dot_leaves_a_side_with_a_neighbouring_pad() {
    let mut fp = inline_part(1);
    bake_generated(&mut fp);
    let dot = dots(&fp.generated.silk);
    assert_eq!(dot.len(), 1);
    assert!(dot[0][0].abs() < 1e-9, "{dot:?}");
    assert!((dot[0][1] + 0.82).abs() < 1e-9, "{dot:?}");
}

/// Pads packed on every side of pad 1 leave no clear place for a dot.
#[test]
fn with_no_clear_side_there_is_no_dot() {
    let mut fp = inline_part(1);
    for (position, number) in [([0.0, 1.0], "8"), ([0.0, -1.0], "9")] {
        fp.pads.push(Pad {
            number: number.into(),
            shape: PadShape::Rect,
            size: [0.6, 1.0],
            position,
            ..Pad::default()
        });
    }
    bake_generated(&mut fp);
    assert!(dots(&fp.generated.silk).is_empty());
    assert!(pin_one_dot_is_missing(&fp), "the panel says so");
}

/// DECISIONS 2026-11-22, mockup row 124: the note follows the dot.
#[test]
fn a_missing_dot_is_reported_only_while_the_marker_is_on() {
    let mut fp = part();
    fp.generated.silk_outline = true;
    fp.generated.silk_line_width_mm = 0.1;
    fp.generated.pad_clearance_mm = 0.2;
    assert!(!pin_one_dot_is_missing(&fp), "a dot fits");

    fp.pads[0].number = "A".into();
    assert!(pin_one_dot_is_missing(&fp), "no pad 1");

    fp.generated.pin_one_marker = false;
    assert!(!pin_one_dot_is_missing(&fp), "the marker is off");

    fp.generated.pin_one_marker = true;
    fp.generated.silk_outline = false;
    assert!(!pin_one_dot_is_missing(&fp), "the outline is off");
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
