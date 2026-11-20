//! Arcs: the centre of a single-quadrant arc, an arc as chord points within
//! 1 µm of the true curve, and a round pen drawn along a chain of points.

use std::f64::consts::{FRAC_PI_2, TAU};

use pcb_geom::{PointNm, Shape};

/// The furthest a chord may fall inside its arc, in nm.
pub(crate) const CHORD_ERROR_NM: f64 = 1_000.0;
/// The most chords one arc is cut into.
const MAX_CHORDS: usize = 4_096;

/// How many chords an arc of `radius` nm and `sweep` radians needs.
pub(crate) fn chords(radius: f64, sweep: f64) -> usize {
    let sweep = sweep.abs();
    if !(radius.is_finite() && sweep.is_finite()) || sweep == 0.0 {
        return 1;
    }
    let step = if radius <= CHORD_ERROR_NM {
        FRAC_PI_2
    } else {
        2.0 * (1.0 - CHORD_ERROR_NM / radius).acos()
    };
    ((sweep / step).ceil() as usize).clamp(1, MAX_CHORDS)
}

fn angle_of(p: PointNm, center: PointNm) -> f64 {
    ((p.y - center.y) as f64).atan2((p.x - center.x) as f64)
}

fn radius_of(p: PointNm, center: PointNm) -> f64 {
    ((p.x - center.x) as f64).hypot((p.y - center.y) as f64)
}

/// The signed sweep from `from` to `to` about `center`: positive
/// counter-clockwise. A closed arc is a full turn when `full_when_closed`.
fn sweep(from: PointNm, to: PointNm, center: PointNm, ccw: bool, full_when_closed: bool) -> f64 {
    if from == to {
        return match (full_when_closed, ccw) {
            (false, _) => 0.0,
            (true, true) => TAU,
            (true, false) => -TAU,
        };
    }
    let (a0, a1) = (angle_of(from, center), angle_of(to, center));
    if ccw {
        (a1 - a0).rem_euclid(TAU)
    } else {
        -(a0 - a1).rem_euclid(TAU)
    }
}

/// The points of the arc after `from`, ending exactly on `to`. The radius
/// runs from the start's to the end's, so a file whose two radii differ a
/// little still meets both ends.
pub(crate) fn arc_points(
    from: PointNm,
    to: PointNm,
    center: PointNm,
    ccw: bool,
    full_when_closed: bool,
) -> Vec<PointNm> {
    let turn = sweep(from, to, center, ccw, full_when_closed);
    let (r0, r1) = (radius_of(from, center), radius_of(to, center));
    let n = chords(r0.max(r1), turn);
    let a0 = angle_of(from, center);
    let mut points: Vec<PointNm> = (1..n)
        .map(|k| {
            let t = k as f64 / n as f64;
            let (r, a) = (r0 + (r1 - r0) * t, a0 + turn * t);
            PointNm::new(
                center.x + (r * a.cos()).round() as i64,
                center.y + (r * a.sin()).round() as i64,
            )
        })
        .collect();
    points.push(to);
    points
}

/// The centre of a single-quadrant (`G74`) arc: the offsets are unsigned,
/// and of the four centres they give, the one whose arc turns at most a
/// quarter in the given sense and whose two radii agree best.
pub(crate) fn single_quadrant_center(
    from: PointNm,
    to: PointNm,
    offset: PointNm,
    ccw: bool,
) -> PointNm {
    let (i, j) = (offset.x.abs(), offset.y.abs());
    let candidates =
        [(i, j), (-i, j), (i, -j), (-i, -j)].map(|(x, y)| from.plus(PointNm::new(x, y)));
    let mismatch = |c: &PointNm| (radius_of(from, *c) - radius_of(to, *c)).abs();
    candidates
        .iter()
        .filter(|c| sweep(from, to, **c, ccw, false).abs() <= FRAC_PI_2 + 1e-9)
        .min_by(|a, b| mismatch(a).total_cmp(&mismatch(b)))
        .copied()
        .unwrap_or(candidates[0])
}

/// A round pen of `radius` drawn from `start` through `points`.
pub(crate) fn capsule_chain(start: PointNm, points: &[PointNm], radius: i64) -> Vec<Shape> {
    let mut at = start;
    points
        .iter()
        .map(|&p| {
            let shape = Shape::Capsule {
                a: at,
                b: p,
                radius,
            };
            at = p;
            shape
        })
        .collect()
}

/// `p` turned `degrees` counter-clockwise about the origin.
pub(crate) fn rotate(p: (f64, f64), degrees: f64) -> (f64, f64) {
    if degrees == 0.0 {
        return p;
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    (p.0 * cos - p.1 * sin, p.0 * sin + p.1 * cos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> PointNm {
        PointNm::new(x, y)
    }

    #[test]
    fn chords_keep_within_a_micrometre() {
        // A 10 mm radius half turn.
        let n = chords(10_000_000.0, std::f64::consts::PI);
        let step = std::f64::consts::PI / n as f64;
        let sag = 10_000_000.0 * (1.0 - (step / 2.0).cos());
        assert!(sag <= CHORD_ERROR_NM, "{n} chords sag {sag}");
        assert_eq!(chords(500.0, TAU), 4);
        assert_eq!(chords(1e6, 0.0), 1);
    }

    #[test]
    fn a_quarter_arc_ends_exactly_and_stays_on_its_circle() {
        let points = arc_points(p(1_000_000, 0), p(0, 1_000_000), p(0, 0), true, true);
        assert_eq!(points.last(), Some(&p(0, 1_000_000)));
        for q in &points {
            let r = radius_of(*q, p(0, 0));
            assert!((r - 1_000_000.0).abs() <= 1.0, "{q:?}");
            assert!(q.x >= 0 && q.y >= 0, "{q:?} stays in the first quadrant");
        }
        // Clockwise the other way round: three quarters.
        let cw = arc_points(p(1_000_000, 0), p(0, 1_000_000), p(0, 0), false, true);
        assert!(cw.iter().any(|q| q.x < 0 && q.y < 0));
    }

    #[test]
    fn a_closed_arc_is_a_full_circle_only_in_multi_quadrant_mode() {
        let full = arc_points(p(10_000, 0), p(10_000, 0), p(0, 0), true, true);
        assert!(full.len() > 4);
        assert!(full.iter().any(|q| q.x < 0));
        let none = arc_points(p(10_000, 0), p(10_000, 0), p(0, 0), true, false);
        assert_eq!(none, vec![p(10_000, 0)]);
    }

    #[test]
    fn the_single_quadrant_centre_is_the_one_making_a_quarter() {
        // From (10, 0) counter-clockwise to (0, 10) about the origin: the
        // offsets are |-10|, |0|.
        let c = single_quadrant_center(p(10, 0), p(0, 10), p(10, 0), true);
        assert_eq!(c, p(0, 0));
        // Clockwise from (0, 10) to (10, 0) about the origin.
        let c = single_quadrant_center(p(0, 10), p(10, 0), p(0, 10), false);
        assert_eq!(c, p(0, 0));
    }

    #[test]
    fn a_pen_chain_joins_each_point_to_the_next() {
        let shapes = capsule_chain(p(0, 0), &[p(5, 0), p(5, 5)], 2);
        assert_eq!(
            shapes,
            vec![
                Shape::Capsule {
                    a: p(0, 0),
                    b: p(5, 0),
                    radius: 2
                },
                Shape::Capsule {
                    a: p(5, 0),
                    b: p(5, 5),
                    radius: 2
                },
            ]
        );
        let (x, y) = rotate((1.0, 0.0), 90.0);
        assert!(x.abs() < 1e-12 && (y - 1.0).abs() < 1e-12);
    }
}
