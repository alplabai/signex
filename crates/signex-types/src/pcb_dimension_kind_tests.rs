//! #2507: what each dimension kind measures and draws, and a radial
//! dimension put back on the arc or circle it measures.

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn kind(kind: DimensionKind, start: Point, end: Point, offset: f64) -> LinearDimension {
    let mut d = LinearDimension::new(Uuid::from_u128(1), start, end, offset);
    d.kind = kind;
    d.text_height = 1.0;
    d
}

/// A circle about (20, 20) of radius 5.
fn circle() -> MeasuredCircle {
    MeasuredCircle {
        uuid: Uuid::from_u128(0x21),
        centre: p(20.0, 20.0),
        radius: 5.0,
    }
}

#[test]
fn each_kind_measures_its_own_length() {
    let (start, end) = (p(0.0, 0.0), p(-6.0, 8.0));
    assert_eq!(kind(DimensionKind::Aligned, start, end, 0.0).value_mm(), 10.0);
    assert_eq!(kind(DimensionKind::Horizontal, start, end, 0.0).value_mm(), 6.0);
    assert_eq!(kind(DimensionKind::Vertical, start, end, 0.0).value_mm(), 8.0);
    assert_eq!(kind(DimensionKind::Radial, start, end, 0.0).value_mm(), 10.0);
    let level = kind(DimensionKind::Vertical, start, p(5.0, 0.0), 1.0);
    assert_eq!(level.value_mm(), 0.0);
    assert!(level.drawing().is_none(), "nothing up or down to measure");
}

#[test]
fn a_horizontal_dimension_draws_a_level_line_with_extension_lines_to_it() {
    // From (0, 0) to (10, 4), the line 2 mm above the start.
    let d = kind(DimensionKind::Horizontal, p(0.0, 0.0), p(10.0, 4.0), 2.0);
    let drawing = d.drawing().unwrap();
    assert_eq!(drawing.value_mm, 10.0);
    assert_eq!(drawing.label_rotation_deg, 0.0);
    let s = &drawing.strokes;
    assert!(close(s[0].0, p(0.0, 0.0)) && close(s[0].1, p(0.0, 2.5)));
    // The end's extension line runs down from (10, 4) past the line.
    assert!(close(s[1].0, p(10.0, 4.0)) && close(s[1].1, p(10.0, 1.5)));
    assert!(close(s[2].0, p(0.0, 2.0)) && close(s[2].1, p(10.0, 2.0)));
    assert!(close(drawing.label_at, p(5.0, 3.0)), "{:?}", drawing.label_at);
}

#[test]
fn a_vertical_dimension_draws_an_upright_line() {
    let d = kind(DimensionKind::Vertical, p(0.0, 0.0), p(3.0, 10.0), -2.0);
    let drawing = d.drawing().unwrap();
    assert_eq!(drawing.value_mm, 10.0);
    assert_eq!(drawing.label_rotation_deg, 90.0);
    // Along +y the quarter turn is -x: a negative offset stands at x = 2.
    assert!(close(drawing.strokes[2].0, p(2.0, 0.0)));
    assert!(close(drawing.strokes[2].1, p(2.0, 10.0)));
}

#[test]
fn a_radial_dimension_draws_a_leader_with_one_arrow_and_reads_r() {
    let mut d = kind(DimensionKind::Radial, p(0.0, 0.0), p(4.0, 0.0), 3.0);
    d.style.arrow_style = crate::pcb_dimension_style::ArrowStyle::Filled;
    d.style.arrow_length = 1.0;
    let drawing = d.drawing().unwrap();
    assert_eq!(d.extension_strokes(), 0);
    assert_eq!(d.stroke_width(0), d.line_width, "the leader at the line width");
    assert_eq!(drawing.strokes, [(p(0.0, 0.0), p(4.0, 0.0))], "no offset");
    assert_eq!(drawing.filled_arrows.len(), 1);
    assert!(close(drawing.filled_arrows[0][0], p(4.0, 0.0)), "on the circle");
    assert!(drawing.filled_arrows[0][1].x < 4.0, "pointing out");
    assert_eq!(d.value_text(), "R4.00mm");
    d.style.prefix = "~".into();
    assert_eq!(d.value_text(), "~R4.00mm");
}

#[test]
fn an_aligned_dimension_draws_as_it_did_before_the_kinds() {
    let d = kind(DimensionKind::Aligned, p(0.0, 0.0), p(10.0, 0.0), 2.0);
    let mut before = d.clone();
    before.kind = DimensionKind::default();
    assert_eq!(d.drawing(), before.drawing());
    assert_eq!(DimensionKind::default(), DimensionKind::Aligned);
    assert_eq!(d.drawing().unwrap().strokes.len(), 7);
}

#[test]
fn a_circles_edge_and_the_point_toward_a_click() {
    let c = circle();
    assert_eq!(c.edge_distance(p(20.0, 25.5)), 0.5);
    assert_eq!(c.edge_distance(p(20.0, 20.0)), 5.0, "the centre is 5 mm off");
    assert!(close(c.point_toward(p(40.0, 20.0)), p(25.0, 20.0)));
    assert!(close(c.point_toward(p(20.0, 20.0)), p(25.0, 20.0)), "right");
}

#[test]
fn a_radial_dimension_is_put_back_on_its_circle_keeping_its_direction() {
    let d = LinearDimension::radial(Uuid::from_u128(2), &circle(), p(20.0, 30.0));
    assert!(close(d.start, p(20.0, 20.0)) && close(d.end, p(20.0, 25.0)));
    assert_eq!(
        (d.kind, d.measures),
        (DimensionKind::Radial, Some(Uuid::from_u128(0x21)))
    );
    assert!(d.refit_to(&circle()).is_none(), "on its circle");
    // The circle moves 10 mm left and grows to radius 6.
    let moved = MeasuredCircle {
        centre: p(10.0, 20.0),
        radius: 6.0,
        ..circle()
    };
    let refit = d.refit_to(&moved).expect("off its circle");
    assert!(close(refit.start, p(10.0, 20.0)));
    assert!(close(refit.end, p(10.0, 26.0)), "{:?}", refit.end);
    assert_eq!(refit.value_text(), "R6.00mm");
    let aligned = LinearDimension::new(Uuid::from_u128(3), p(0.0, 0.0), p(1.0, 0.0), 0.0);
    assert!(aligned.refit_to(&moved).is_none(), "only radial follows");
}
