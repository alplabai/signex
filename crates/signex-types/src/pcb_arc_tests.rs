use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

use super::*;

fn arc(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> TrackArc {
    TrackArc {
        uuid: Uuid::from_u128(1),
        start: Point::new(start.0, start.1),
        mid: Point::new(mid.0, mid.1),
        end: Point::new(end.0, end.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 3,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_quarter_turn_through_its_mid_turns_counter_clockwise() {
    let h = SQRT_2 / 2.0;
    let quarter = arc((1.0, 0.0), (h, h), (0.0, 1.0));
    let sweep = quarter.sweep().expect("three points on a circle");
    assert!(close(sweep.center.x, 0.0) && close(sweep.center.y, 0.0));
    assert!(close(sweep.radius, 1.0));
    assert!(close(sweep.sweep, FRAC_PI_2), "{}", sweep.sweep);
    assert!(close(quarter.length_mm(), FRAC_PI_2));
}

#[test]
fn a_mid_on_the_far_side_turns_the_long_way_clockwise() {
    let h = SQRT_2 / 2.0;
    let long = arc((1.0, 0.0), (-h, -h), (0.0, 1.0));
    let sweep = long.sweep().unwrap();
    assert!(close(sweep.sweep, -3.0 * FRAC_PI_2), "{}", sweep.sweep);
    assert!(close(long.length_mm(), 3.0 * FRAC_PI_2));
    let half = arc((1.0, 0.0), (0.0, -1.0), (-1.0, 0.0));
    assert!(close(half.sweep().unwrap().sweep, -PI));
}

#[test]
fn points_on_one_line_are_a_straight_track() {
    let straight = arc((0.0, 0.0), (1.0, 0.0), (3.0, 0.0));
    assert!(straight.sweep().is_none());
    assert_eq!(straight.points(0.01), [straight.start, straight.end]);
    assert!(close(straight.length_mm(), 3.0));
    let closed = arc((1.0, 0.0), (0.0, 1.0), (1.0, 0.0));
    assert!(closed.sweep().is_none(), "start and end the same");
}

#[test]
fn the_polyline_ends_on_the_arc_ends_and_keeps_each_chord_within_the_error() {
    let h = SQRT_2 / 2.0;
    let quarter = arc((10.0, 0.0), (10.0 * h, 10.0 * h), (0.0, 10.0));
    let error = 0.01;
    let points = quarter.points(error);
    assert_eq!(points.first(), Some(&quarter.start));
    assert_eq!(points.last(), Some(&quarter.end));
    assert!(points.len() > 2, "a bend needs more than one chord");
    for pair in points.windows(2) {
        let middle = Point::new((pair[0].x + pair[1].x) / 2.0, (pair[0].y + pair[1].y) / 2.0);
        let from_center = middle.x.hypot(middle.y);
        assert!(10.0 - from_center <= error + 1e-9, "{from_center}");
        for p in pair {
            assert!(close(p.x.hypot(p.y), 10.0));
        }
    }
    let coarse = quarter.points(100.0);
    assert_eq!(coarse.len(), 2, "one chord when the error allows it");
    assert_eq!(quarter.points(0.0).len(), ARC_MAX_CHORDS + 1);
    assert_eq!(quarter.points(f64::NAN).len(), ARC_MAX_CHORDS + 1);
}

#[test]
fn the_layer_reads_as_its_typed_layer() {
    use crate::pcb_layer::CopperLayer;
    let mut quarter = arc((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0));
    assert_eq!(
        quarter.pcb_layer(),
        Some(PcbLayer::Copper(CopperLayer::Top))
    );
    quarter.layer = "NotALayer".into();
    assert_eq!(quarter.pcb_layer(), None);
}

#[test]
fn an_arc_round_trips_through_json() {
    let quarter = arc((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0));
    let json = serde_json::to_string(&quarter).unwrap();
    let back: TrackArc = serde_json::from_str(&json).unwrap();
    assert_eq!(back, quarter);
}

#[test]
fn the_three_points_of_a_sweep_rebuild_the_same_arc() {
    let h = SQRT_2 / 2.0;
    let quarter = arc((1.0, 0.0), (h, h), (0.0, 1.0));
    let sweep = quarter.sweep().unwrap();
    let (start, mid, end) = sweep.three_points();
    let again = TrackArc {
        start,
        mid,
        end,
        ..quarter.clone()
    };
    let back = again.sweep().unwrap();
    assert!(close(back.radius, sweep.radius) && close(back.sweep, sweep.sweep));
    assert!(
        close(start.x, 1.0) && close(end.y, 1.0),
        "{start:?} {end:?}"
    );
    let _ = PI;
}
