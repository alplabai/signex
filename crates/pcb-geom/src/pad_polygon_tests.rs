//! Trapezoid and custom pads have their true outline and turn with their
//! footprint and by themselves (#2242).

use super::pad_shape_tests::{board_of, fp, p, pad};
use super::*;
use crate::{BoardIndex, ItemRef, ratsnest};
use signex_types::pcb::{Pad, Point};
use signex_types::pcb_pad_shape::PadShapeFault;

/// A 2 x 1 mm trapezoid at `x` whose +y edge is 1 mm longer: own corners
/// (-0.75, -0.5), (0.75, -0.5), (1.25, 0.5), (-1.25, 0.5).
fn trapezoid(x: f64, own: f64) -> Pad {
    let mut t = pad(1, x, (2.0, 1.0), PadShape::Trapezoid, own);
    t.trapezoid_delta = Point::new(1.0, 0.0);
    t
}

/// An L: the 1 x 1 mm square from the pad centre up and right, less the
/// square above (0.4, 0.4).
fn l_outline() -> Vec<Point> {
    [
        (0.0, 0.0),
        (1.0, 0.0),
        (1.0, 0.4),
        (0.4, 0.4),
        (0.4, 1.0),
        (0.0, 1.0),
    ]
    .map(|(x, y)| Point::new(x, y))
    .to_vec()
}

fn custom(n: u128, x: f64, own: f64) -> Pad {
    let mut c = pad(n, x, (1.0, 1.0), PadShape::Custom, own);
    c.custom_outline = l_outline();
    c
}

fn ring(f: &Footprint) -> Vec<PointNm> {
    pad_shape(f, &f.pads[0]).unwrap().outline(8)
}

#[test]
fn a_trapezoid_turned_a_quarter_has_exact_corners() {
    let want = vec![
        p(500_000, -750_000),
        p(500_000, 750_000),
        p(-500_000, 1_250_000),
        p(-500_000, -1_250_000),
    ];
    // The footprint's turn, the pad's own turn and their sum agree.
    for (footprint, own) in [(90.0, 0.0), (0.0, 90.0), (180.0, -90.0), (270.0, 180.0)] {
        let f = fp(footprint, "TopCopper", vec![trapezoid(0.0, own)]);
        assert_eq!(ring(&f), want, "{footprint}+{own}");
    }
}

#[test]
fn a_trapezoid_turns_about_its_own_centre_and_moves_with_the_footprint() {
    // The pad 1 mm along the footprint's x: a quarter turn of the
    // footprint carries its centre to (0, 1) mm.
    let f = fp(90.0, "TopCopper", vec![trapezoid(1.0, 0.0)]);
    let want: Vec<PointNm> = [
        (500_000, -750_000),
        (500_000, 750_000),
        (-500_000, 1_250_000),
        (-500_000, -1_250_000),
    ]
    .map(|(x, y)| p(x, y + 1_000_000))
    .to_vec();
    assert_eq!(ring(&f), want);
    // The pad's own turn leaves its centre where it is.
    let own = fp(0.0, "TopCopper", vec![trapezoid(1.0, 90.0)]);
    let shifted: Vec<PointNm> = [
        (500_000, -750_000),
        (500_000, 750_000),
        (-500_000, 1_250_000),
        (-500_000, -1_250_000),
    ]
    .map(|(x, y)| p(x + 1_000_000, y))
    .to_vec();
    assert_eq!(ring(&own), shifted);
}

#[test]
fn a_bottom_footprint_mirrors_a_trapezoid_and_keeps_its_ring_counter_clockwise() {
    // Delta y: the +x edge is the long one; mirrored, the −x edge is.
    let mut t = pad(1, 0.0, (2.0, 1.0), PadShape::Trapezoid, 0.0);
    t.trapezoid_delta = Point::new(0.0, 1.0);
    let top = fp(0.0, "TopCopper", vec![t.clone()]);
    let bottom = fp(0.0, "BottomCopper", vec![t]);
    let near_right = p(950_000, 700_000);
    let near_left = p(-950_000, 700_000);
    let top_shape = pad_shape(&top, &top.pads[0]).unwrap();
    assert!(top_shape.contains(near_right, 0) && !top_shape.contains(near_left, 0));
    let bottom_shape = pad_shape(&bottom, &bottom.pads[0]).unwrap();
    assert!(bottom_shape.contains(near_left, 0) && !bottom_shape.contains(near_right, 0));
    let corners = ring(&bottom);
    let twice_area: i128 = (0..corners.len())
        .map(|i| corners[i].cross(corners[(i + 1) % corners.len()]))
        .sum();
    assert!(twice_area > 0, "{corners:?}");
}

#[test]
fn a_custom_pad_is_hit_on_its_outline_not_its_box() {
    // Turned a quarter: the L runs from (0, 0) to (-1, 1), notch above
    // (-0.4, 0.4) cut away.
    let f = fp(0.0, "TopCopper", vec![custom(1, 0.0, 90.0)]);
    let shape = pad_shape(&f, &f.pads[0]).unwrap();
    assert!(shape.contains(p(-200_000, 800_000), 0), "the upright arm");
    assert!(shape.contains(p(-800_000, 200_000), 0), "the flat arm");
    assert!(!shape.contains(p(-700_000, 700_000), 0), "the notch");
    // Within reach of the notch's inner edge counts.
    assert!(shape.contains(p(-500_000, 700_000), 100_000));
    let index = BoardIndex::build(&board_of(vec![f]));
    let pad_ref = ItemRef::Pad {
        footprint: uuid::Uuid::from_u128(99),
        pad: uuid::Uuid::from_u128(1),
    };
    assert_eq!(index.hit_point(p(-200_000, 800_000), 0), vec![pad_ref]);
    assert!(index.hit_point(p(-700_000, 700_000), 0).is_empty());
}

#[test]
fn copper_in_a_custom_pads_notch_neither_overlaps_nor_is_measured_to_its_box() {
    let f = fp(0.0, "TopCopper", vec![custom(1, 0.0, 0.0)]);
    let l = pad_shape(&f, &f.pads[0]).unwrap();
    let in_notch = Shape::Disc {
        center: p(750_000, 750_000),
        radius: 300_000,
    };
    assert!(!l.overlaps(&in_notch));
    // 0.35 mm to either inner edge, less the disc's 0.3 mm.
    assert_eq!(l.separation(&in_notch).distance_nm(), 50_000);
    let on_arm = Shape::Disc {
        center: p(750_000, 300_000),
        radius: 50_000,
    };
    assert!(l.overlaps(&on_arm));
    // Grown by a clearance, its covering ring holds every point that near.
    let ring = l.clone().inflated(100_000).covering_ring(8);
    assert!(crate::shape_overlap::point_in_polygon(
        &ring,
        p(1_099_000, 200_000)
    ));
    assert!(!crate::shape_overlap::point_in_polygon(
        &ring,
        p(700_000, 700_000)
    ));
}

#[test]
fn the_ratsnest_joins_through_a_trapezoids_long_edge() {
    // A 1 x 1 mm square resting on the trapezoid's long +y edge (y = 0.5)
    // joins it; lifted 0.1 mm it does not.
    let mut above = pad(2, 0.0, (1.0, 1.0), PadShape::Rect, 0.0);
    above.position = Point::new(0.0, 1.0);
    let joined = board_of(vec![fp(
        0.0,
        "TopCopper",
        vec![trapezoid(0.0, 0.0), above.clone()],
    )]);
    assert!(ratsnest(&joined).is_empty(), "the edges touch at y = 0.5");
    above.position = Point::new(0.0, 1.1);
    let apart = board_of(vec![fp(0.0, "TopCopper", vec![trapezoid(0.0, 0.0), above])]);
    assert_eq!(ratsnest(&apart).len(), 1);
}

#[test]
fn parameters_that_make_no_copper_are_refused_naming_the_fault() {
    let mut both = trapezoid(0.0, 0.0);
    both.trapezoid_delta = Point::new(0.5, 0.5);
    let f = fp(0.0, "TopCopper", vec![both]);
    assert_eq!(
        pad_shape(&f, &f.pads[0]),
        Err(GeomError::PadShape(PadShapeFault::DeltaOnBothAxes))
    );
    let mut bow_tie = custom(1, 0.0, 0.0);
    bow_tie.custom_outline = [(0.0, 0.0), (2.0, 2.0), (2.0, 0.0), (0.0, 1.0)]
        .map(|(x, y)| Point::new(x, y))
        .to_vec();
    let f = fp(0.0, "TopCopper", vec![bow_tie]);
    assert_eq!(
        pad_shape(&f, &f.pads[0]),
        Err(GeomError::PadShape(PadShapeFault::OutlineCrossesItself))
    );
    assert!(
        GeomError::PadShape(PadShapeFault::OutlineCrossesItself)
            .to_string()
            .contains("custom_outline")
    );
}
