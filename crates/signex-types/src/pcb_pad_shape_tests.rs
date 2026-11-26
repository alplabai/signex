//! The trapezoid and custom pad rules (#2242).

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[test]
fn a_delta_x_lengthens_the_top_edge_and_shortens_the_bottom_one() {
    let corners = trapezoid_corners(p(2.0, 1.0), p(1.0, 0.0)).unwrap();
    assert_eq!(
        corners,
        [p(-0.75, -0.5), p(0.75, -0.5), p(1.25, 0.5), p(-1.25, 0.5)]
    );
}

#[test]
fn a_negative_delta_y_makes_the_right_edge_the_shorter_one() {
    let corners = trapezoid_corners(p(2.0, 1.0), p(0.0, -1.0)).unwrap();
    assert_eq!(
        corners,
        [p(-1.0, -0.75), p(1.0, -0.25), p(1.0, 0.25), p(-1.0, 0.75)]
    );
}

#[test]
fn a_bad_delta_is_refused_naming_its_field() {
    let cases = [
        (p(0.0, 0.0), PadShapeFault::DeltaMissing),
        (p(0.5, 0.5), PadShapeFault::DeltaOnBothAxes),
        (p(4.0, 0.0), PadShapeFault::DeltaTooWide),
        (p(-4.5, 0.0), PadShapeFault::DeltaTooWide),
        (p(0.0, 2.0), PadShapeFault::DeltaTooWide),
        (p(f64::NAN, 0.0), PadShapeFault::DeltaNotStorable),
    ];
    for (delta, fault) in cases {
        assert_eq!(
            trapezoid_corners(p(2.0, 1.0), delta),
            Err(fault),
            "{delta:?}"
        );
        assert_eq!(fault.field(), "trapezoid_delta");
    }
    // Just under twice the side still leaves a narrow edge.
    assert!(trapezoid_corners(p(2.0, 1.0), p(3.999, 0.0)).is_ok());
}

#[test]
fn a_simple_outline_of_either_winding_passes() {
    let l_shape = [
        p(0.0, 0.0),
        p(2.0, 0.0),
        p(2.0, 1.0),
        p(1.0, 1.0),
        p(1.0, 2.0),
        p(0.0, 2.0),
    ];
    assert_eq!(outline_fault(&l_shape), None);
    let mut reversed = l_shape.to_vec();
    reversed.reverse();
    assert_eq!(outline_fault(&reversed), None);
    // A straight run through a corner is not a fold.
    assert_eq!(
        outline_fault(&[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)]),
        None
    );
}

#[test]
fn a_bad_outline_is_refused_naming_its_field() {
    let cases: [(&[Point], PadShapeFault); 5] = [
        (
            &[p(0.0, 0.0), p(1.0, 0.0)],
            PadShapeFault::OutlineTooFewCorners,
        ),
        (
            &[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)],
            PadShapeFault::OutlineHasNoArea,
        ),
        (
            // A bow tie.
            &[p(0.0, 0.0), p(2.0, 2.0), p(2.0, 0.0), p(0.0, 1.0)],
            PadShapeFault::OutlineCrossesItself,
        ),
        (
            // An edge that doubles back over the one before it.
            &[p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            PadShapeFault::OutlineCrossesItself,
        ),
        (
            &[p(0.0, 0.0), p(f64::INFINITY, 0.0), p(1.0, 1.0)],
            PadShapeFault::OutlineNotStorable,
        ),
    ];
    for (outline, fault) in cases {
        assert_eq!(outline_fault(outline), Some(fault), "{outline:?}");
        assert_eq!(fault.field(), "custom_outline");
    }
}
