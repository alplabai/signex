//! Tracks that run side by side (#2389): a differential pair or a bus is
//! routed as one centre line, and each member follows it at a fixed
//! sideways offset. Corners are mitred, so each member keeps its distance
//! from the centre line through a bend; a bend too sharp for that is
//! refused rather than drawn with a spike.

use crate::units::PointNm;

/// The longest a mitred corner may reach, in offsets: a turn of about 151°.
const MAX_MITER: f64 = 4.0;

/// `path` moved `distance` nm to its left (to its right for a negative
/// distance), corners mitred. Repeated points are dropped first. `None` for
/// a path with no length, or a turn too sharp to mitre.
pub fn offset_path(path: &[PointNm], distance: i64) -> Option<Vec<PointNm>> {
    let mut points: Vec<PointNm> = Vec::with_capacity(path.len());
    for &p in path {
        if points.last() != Some(&p) {
            points.push(p);
        }
    }
    if points.len() < 2 {
        return None;
    }
    let normals: Vec<(f64, f64)> = points.windows(2).map(|w| left_normal(w[0], w[1])).collect();
    let last = points.len() - 1;
    let d = distance as f64;
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let (nx, ny) = match i {
                0 => normals[0],
                i if i == last => normals[last - 1],
                i => miter(normals[i - 1], normals[i])?,
            };
            let x = p.x as f64 + d * nx;
            let y = p.y as f64 + d * ny;
            (x.is_finite() && y.is_finite())
                .then(|| PointNm::new(x.round() as i64, y.round() as i64))
        })
        .collect()
}

/// `path` moved by each of `offsets` (left positive): the members of a
/// pair or a bus, in the order of `offsets`. `None` as for [`offset_path`].
pub fn parallel_paths(path: &[PointNm], offsets: &[i64]) -> Option<Vec<Vec<PointNm>>> {
    offsets
        .iter()
        .map(|&offset| offset_path(path, offset))
        .collect()
}

/// The offsets of `count` members `pitch` nm apart, centred on the path,
/// leftmost first.
pub fn bus_offsets(count: usize, pitch: i64) -> Vec<i64> {
    let count = i64::try_from(count).unwrap_or(0);
    (0..count)
        .map(|k| (count - 1 - 2 * k) * pitch / 2)
        .collect()
}

/// The unit normal on the left of the leg from `a` to `b`.
fn left_normal(a: PointNm, b: PointNm) -> (f64, f64) {
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let length = dx.hypot(dy);
    (-dy / length, dx / length)
}

/// The corner's offset direction between legs of unit normals `a` and `b`,
/// scaled so the offset legs keep their distance; `None` past
/// [`MAX_MITER`].
fn miter(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let dot = a.0 * b.0 + a.1 * b.1;
    // |a + b| / (1 + a.b) is 1 / cos(half the turn).
    let scale = 1.0 + dot;
    if scale < 2.0 / (MAX_MITER * MAX_MITER) {
        return None;
    }
    Some(((a.0 + b.0) / scale, (a.1 + b.1) / scale))
}
