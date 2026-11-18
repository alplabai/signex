//! Length tuning (#2389): an accordion of square bumps on one side of a
//! straight track. Each bump rises `height` square to the track, runs
//! `pitch` along it and comes back, then a gap of `pitch` before the next,
//! so each adds exactly `2 * height` to the length. The bumps sit in the
//! middle of the track with at least `pitch` of straight track at each end.

use crate::units::PointNm;

/// How many bumps of `pitch` fit along a straight track of `length` nm.
pub fn accordion_room(length: i64, pitch: i64) -> usize {
    if pitch <= 0 {
        return 0;
    }
    // n bumps take (2n - 1) pitches, plus a pitch at each end.
    let room = (length - pitch) / (2 * pitch);
    usize::try_from(room).unwrap_or(0)
}

/// The corners of the track from `a` to `b` with `bumps` bumps of `height`
/// on its left (`left`) or right, `a` and `b` included. `None` when they do
/// not fit ([`accordion_room`]) or the track has no length.
pub fn accordion(
    a: PointNm,
    b: PointNm,
    bumps: usize,
    height: i64,
    pitch: i64,
    left: bool,
) -> Option<Vec<PointNm>> {
    let axis = b.minus(a);
    let length = (axis.x as f64).hypot(axis.y as f64);
    if axis == PointNm::ORIGIN || bumps == 0 || bumps > accordion_room(length.floor() as i64, pitch)
    {
        return None;
    }
    let along = (axis.x as f64 / length, axis.y as f64 / length);
    let side = if left { 1.0 } else { -1.0 };
    let across = (-along.1 * side, along.0 * side);
    let at = |s: f64, t: f64| {
        PointNm::new(
            a.x + (along.0 * s + across.0 * t).round() as i64,
            a.y + (along.1 * s + across.1 * t).round() as i64,
        )
    };
    let (pitch, height) = (pitch as f64, height as f64);
    let used = (2 * bumps - 1) as f64 * pitch;
    let first = ((length - used) / 2.0).floor();
    let mut corners = vec![a];
    for k in 0..bumps {
        let rise = first + 2.0 * k as f64 * pitch;
        corners.extend([
            at(rise, 0.0),
            at(rise, height),
            at(rise + pitch, height),
            at(rise + pitch, 0.0),
        ]);
    }
    corners.push(b);
    Some(corners)
}
