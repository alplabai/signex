use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// The upper half of the unit circle about the origin, counter-clockwise
/// from (1, 0) to (-1, 0).
fn upper_half() -> ArcSweep {
    ArcSweep {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
        start_angle: 0.0,
        sweep: PI,
    }
}

#[test]
fn a_line_crosses_an_arc_only_on_its_sweep() {
    let line = Curve::Segment(Point::new(0.0, -2.0), Point::new(0.0, 2.0));
    let hits = crossings(&line, &Curve::Arc(upper_half()));
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(close(hits[0], Point::new(0.0, 1.0)), "{hits:?}");
    // Either order.
    assert_eq!(crossings(&Curve::Arc(upper_half()), &line), hits);
    // A line that stops short of the circle does not cross it.
    let short = Curve::Segment(Point::new(0.0, -0.5), Point::new(0.0, 0.5));
    assert!(crossings(&short, &Curve::Arc(upper_half())).is_empty());
}

#[test]
fn two_circles_cross_twice_and_a_clockwise_arc_once() {
    let a = Curve::circle(Point::new(0.0, 0.0), 5.0).unwrap();
    let b = Curve::circle(Point::new(8.0, 0.0), 5.0).unwrap();
    let mut hits = crossings(&a, &b);
    hits.sort_by(|p, q| p.y.total_cmp(&q.y));
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!(close(hits[0], Point::new(4.0, -3.0)), "{hits:?}");
    assert!(close(hits[1], Point::new(4.0, 3.0)), "{hits:?}");
    // Clockwise from (5, 0) a quarter turn: through (4, -3) only.
    let lower = Curve::Arc(ArcSweep {
        center: Point::new(0.0, 0.0),
        radius: 5.0,
        start_angle: 0.0,
        sweep: -FRAC_PI_2,
    });
    let hits = crossings(&lower, &b);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(close(hits[0], Point::new(4.0, -3.0)), "{hits:?}");
}

#[test]
fn apart_circles_do_not_cross() {
    let a = Curve::circle(Point::new(0.0, 0.0), 1.0).unwrap();
    let b = Curve::circle(Point::new(5.0, 0.0), 1.0).unwrap();
    assert!(crossings(&a, &b).is_empty());
    assert!(Curve::circle(Point::new(0.0, 0.0), 0.0).is_none());
}

#[test]
fn the_nearest_point_of_an_arc_is_its_end_off_the_sweep() {
    let arc = Curve::Arc(upper_half());
    assert!(close(
        arc.nearest(Point::new(0.0, 3.0)),
        Point::new(0.0, 1.0)
    ));
    let below = arc.nearest(Point::new(0.9, -3.0));
    assert!(close(below, Point::new(1.0, 0.0)), "{below:?}");
}

#[test]
fn a_ring_has_an_edge_per_side() {
    let square = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ];
    assert_eq!(ring_edges(&square).count(), 4);
    assert_eq!(ring_edges(&square[..1]).count(), 0);
}
