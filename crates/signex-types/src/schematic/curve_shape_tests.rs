//! The shared curve model (#1267, #2838): outlines, bounds, the quarter-turn
//! and mirror maps, the click gestures and the degenerate shapes.

use super::*;

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}

fn ellipse() -> CurveShape {
    CurveShape::Ellipse {
        center: Point::new(10.0, 20.0),
        radius_x: 6.0,
        radius_y: 2.0,
    }
}

fn arc(start_deg: f64, end_deg: f64) -> CurveShape {
    CurveShape::EllipticalArc {
        center: Point::new(0.0, 0.0),
        radius_x: 4.0,
        radius_y: 2.0,
        start_deg,
        end_deg,
    }
}

/// A quarter turn counter-clockwise about the origin: (x, y) -> (-y, x).
fn quarter_turn(p: Point) -> Point {
    Point::new(-p.y, p.x)
}

/// A mirror across the Y axis: (x, y) -> (-x, y).
fn mirror_x(p: Point) -> Point {
    Point::new(-p.x, p.y)
}

#[test]
fn an_ellipse_is_a_closed_ring_inside_its_radii() {
    let ring = ellipse().flatten();
    assert_eq!(ring.len(), super::curve_shape::ELLIPSE_SEGMENTS);
    let (lo, hi) = ellipse().bounds();
    assert!(close(lo.x, 4.0) && close(hi.x, 16.0), "{lo:?} {hi:?}");
    assert!(close(lo.y, 18.0) && close(hi.y, 22.0), "{lo:?} {hi:?}");
    assert!(ellipse().is_closed());
}

#[test]
fn an_elliptical_arc_runs_counter_clockwise_from_start_to_end() {
    let points = arc(0.0, 90.0).flatten();
    let (first, last) = (points[0], points[points.len() - 1]);
    assert!(close(first.x, 4.0) && close(first.y, 0.0), "{first:?}");
    assert!(close(last.x, 0.0) && close(last.y, 2.0), "{last:?}");
    assert!(points.iter().all(|p| p.x >= -EPS && p.y >= -EPS));
    assert!(!arc(0.0, 90.0).is_closed());
    // Wrapping: 270 to 90 sweeps through 0, on the right-hand side.
    assert!(arc(270.0, 90.0).flatten().iter().all(|p| p.x >= -EPS));
}

#[test]
fn a_new_round_rect_takes_the_default_radius_clamped_to_its_short_side() {
    let big = CurveShape::round_rect(Point::new(10.0, 10.0), Point::new(0.0, 0.0));
    assert_eq!(
        big,
        CurveShape::RoundRect {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 10.0),
            corner_radius: CurveShape::DEFAULT_CORNER_RADIUS_MM,
        }
    );
    let thin = CurveShape::round_rect(Point::new(0.0, 0.0), Point::new(10.0, 1.0));
    let CurveShape::RoundRect { corner_radius, .. } = thin else {
        panic!("a round rectangle");
    };
    assert!(close(corner_radius, 0.5));
}

#[test]
fn a_quarter_turn_swaps_the_radii_and_turns_the_angles() {
    let mut e = ellipse();
    e.map_points(quarter_turn);
    let CurveShape::Ellipse {
        center,
        radius_x,
        radius_y,
    } = e
    else {
        panic!("still an ellipse");
    };
    assert!(close(center.x, -20.0) && close(center.y, 10.0));
    assert!(close(radius_x, 2.0) && close(radius_y, 6.0));

    let mut a = arc(0.0, 90.0);
    a.map_points(quarter_turn);
    let CurveShape::EllipticalArc {
        radius_x,
        radius_y,
        start_deg,
        end_deg,
        ..
    } = a
    else {
        panic!("still an arc");
    };
    assert!(close(radius_x, 2.0) && close(radius_y, 4.0));
    assert!(close(start_deg, 90.0) && close(end_deg, 180.0), "{a:?}");
}

#[test]
fn a_mirror_swaps_the_arc_ends_so_it_covers_the_mirrored_points() {
    let before: Vec<Point> = arc(0.0, 90.0).flatten().into_iter().map(mirror_x).collect();
    let mut a = arc(0.0, 90.0);
    a.map_points(mirror_x);
    let CurveShape::EllipticalArc {
        start_deg, end_deg, ..
    } = a
    else {
        panic!("still an arc");
    };
    assert!(close(start_deg, 90.0) && close(end_deg, 180.0), "{a:?}");
    let after = a.flatten();
    // The same points, walked the other way.
    for (p, q) in before.iter().rev().zip(&after) {
        assert!(close(p.x, q.x) && close(p.y, q.y), "{p:?} {q:?}");
    }
}

#[test]
fn a_move_keeps_the_angles_exactly() {
    let mut a = arc(33.3, 211.1);
    a.map_points(|p| Point::new(p.x + 2.54, p.y - 1.27));
    assert_eq!(
        a,
        CurveShape::EllipticalArc {
            center: Point::new(2.54, -1.27),
            radius_x: 4.0,
            radius_y: 2.0,
            start_deg: 33.3,
            end_deg: 211.1,
        }
    );
}

#[test]
fn a_mapped_round_rect_keeps_from_as_its_minimum_corner() {
    let mut r = CurveShape::round_rect(Point::new(1.0, 2.0), Point::new(5.0, 4.0));
    r.map_points(mirror_x);
    let CurveShape::RoundRect { from, to, .. } = r else {
        panic!("still a round rectangle");
    };
    assert_eq!((from, to), (Point::new(-5.0, 2.0), Point::new(-1.0, 4.0)));
}

#[test]
fn the_ellipse_tool_takes_the_centre_then_a_corner() {
    let clicks = [Point::new(10.0, 10.0), Point::new(15.08, 7.46)];
    let placed = CurveTool::Ellipse.placed(&clicks).expect("placed");
    let CurveShape::Ellipse {
        center,
        radius_x,
        radius_y,
    } = placed
    else {
        panic!("an ellipse");
    };
    assert_eq!(center, clicks[0]);
    assert!(close(radius_x, 5.08) && close(radius_y, 2.54));
    assert!(CurveTool::Ellipse.placed(&clicks[..1]).is_none());
}

#[test]
fn the_elliptical_arc_tool_takes_five_clicks_at_geometric_angles() {
    let clicks = [
        Point::new(0.0, 0.0),
        Point::new(5.0, 9.0),
        Point::new(-7.0, 2.5),
        Point::new(3.0, 3.0),
        Point::new(-1.0, 0.0),
    ];
    for missing in 1..5 {
        assert!(
            CurveTool::EllipticalArc
                .placed(&clicks[..missing])
                .is_none()
        );
    }
    let placed = CurveTool::EllipticalArc.placed(&clicks).expect("placed");
    assert_eq!(
        placed,
        CurveShape::EllipticalArc {
            center: Point::new(0.0, 0.0),
            radius_x: 5.0,
            radius_y: 2.5,
            start_deg: 45.0,
            end_deg: 180.0,
        }
    );
    // Before the start click the preview is the whole ellipse.
    let preview = CurveTool::EllipticalArc.shape(&clicks[..3], Point::new(1.0, 1.0));
    assert!(matches!(preview, Some(CurveShape::Ellipse { .. })));
}

#[test]
fn the_round_rect_tool_takes_two_corners() {
    let clicks = [Point::new(20.0, 5.0), Point::new(0.0, 0.0)];
    assert_eq!(
        CurveTool::RoundRect.placed(&clicks),
        Some(CurveShape::round_rect(clicks[0], clicks[1]))
    );
}

#[test]
fn a_shape_with_no_size_or_no_sweep_is_not_placed() {
    let flat = [Point::new(0.0, 0.0), Point::new(5.0, 0.0)];
    assert!(CurveTool::Ellipse.placed(&flat).is_none());
    assert!(CurveTool::RoundRect.placed(&flat).is_none());
    assert!(arc(30.0, 30.0).is_degenerate());
    assert!(arc(30.0, 390.0).is_degenerate());
    let mut nan = ellipse();
    nan.map_points(|_| Point::new(f64::NAN, 0.0));
    assert!(nan.is_degenerate());
}

#[test]
fn a_pie_starts_its_ring_at_the_centre() {
    let pie = CurveShape::Pie {
        center: Point::new(1.0, 1.0),
        radius: 2.0,
        start_deg: 0.0,
        end_deg: 90.0,
    };
    let ring = pie.flatten();
    assert_eq!(ring[0], Point::new(1.0, 1.0));
    assert!(close(ring[1].x, 3.0) && close(ring[1].y, 1.0));
    let last = ring[ring.len() - 1];
    assert!(close(last.x, 1.0) && close(last.y, 3.0));
}

#[test]
fn a_closed_curve_has_a_closing_segment_and_an_arc_has_none() {
    let closed = SchCurve::new(uuid::Uuid::nil(), ellipse(), 0.0);
    assert_eq!(closed.segments().len(), closed.flatten().len());
    let open = SchCurve::new(uuid::Uuid::nil(), arc(0.0, 90.0), 0.0);
    assert_eq!(open.segments().len(), open.flatten().len() - 1);
    assert_eq!(open.drawn_fill(), FillType::None);
}
