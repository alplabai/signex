//! Geometry the STEP writer reads off sampled edges and faces: whether
//! samples lie on a line, a circle or a plane, which way a loop turns, and
//! how far a point is from a face's triangles.

use crate::tessellate::{cross3, dot3, sub, unit};

/// Samples within this distance of a line, circle, plane or cylinder lie
/// on it, mm: the length uncertainty the file declares. Far above the
/// kernel's own tolerance, far below any board feature.
pub(crate) const ON_GEOMETRY_MM: f64 = super::context::LENGTH_UNCERTAINTY_MM;

/// The unit direction and length of the line through `points` when every
/// point lies on the segment from the first to the last.
pub(crate) fn as_line(points: &[[f64; 3]]) -> Option<([f64; 3], f64)> {
    let (first, last) = (*points.first()?, *points.last()?);
    let span = sub(last, first);
    let length = dot3(span, span).sqrt();
    if length <= ON_GEOMETRY_MM {
        return None;
    }
    let direction = span.map(|x| x / length);
    let on_line = points.iter().all(|&p| {
        let offset = sub(p, first);
        let along = dot3(offset, direction);
        let off = sub(offset, direction.map(|x| x * along));
        dot3(off, off).sqrt() <= ON_GEOMETRY_MM
    });
    on_line.then_some((direction, length))
}

/// True when every point lies on the horizontal circle of `radius` about
/// `center` at the first point's height.
pub(crate) fn on_circle(points: &[[f64; 3]], center: [f64; 2], radius: f64) -> bool {
    let Some(first) = points.first() else {
        return false;
    };
    points.iter().all(|p| {
        (p[2] - first[2]).abs() <= ON_GEOMETRY_MM
            && (radial_distance(*p, center) - radius).abs() <= ON_GEOMETRY_MM
    })
}

/// True when every point lies on the vertical cylinder of `radius` about
/// `center`.
pub(crate) fn on_cylinder(points: &[[f64; 3]], center: [f64; 2], radius: f64) -> bool {
    points
        .iter()
        .all(|p| (radial_distance(*p, center) - radius).abs() <= ON_GEOMETRY_MM)
}

fn radial_distance(p: [f64; 3], center: [f64; 2]) -> f64 {
    (p[0] - center[0]).hypot(p[1] - center[1])
}

/// The total turn of `points` about the vertical axis through `center`,
/// radians; positive counter-clockwise seen from above.
pub(crate) fn winding(points: &[[f64; 3]], center: [f64; 2]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let a = (pair[0][1] - center[1]).atan2(pair[0][0] - center[0]);
            let b = (pair[1][1] - center[1]).atan2(pair[1][0] - center[0]);
            let mut turn = b - a;
            if turn > std::f64::consts::PI {
                turn -= std::f64::consts::TAU;
            } else if turn < -std::f64::consts::PI {
                turn += std::f64::consts::TAU;
            }
            turn
        })
        .sum()
}

/// Twice the vector area of the closed polygon `ring` (Newell): along the
/// normal it turns counter-clockwise about.
pub(crate) fn newell(ring: &[[f64; 3]]) -> [f64; 3] {
    let mut n = [0.0; 3];
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// A unit vector at right angles to the unit vector `n`.
pub(crate) fn perpendicular(n: [f64; 3]) -> [f64; 3] {
    let helper = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    unit(cross3(n, helper))
}

/// The area-weighted normal of `triangles`, pointing the way they face.
pub(crate) fn facing(triangles: &[[[f64; 3]; 3]]) -> [f64; 3] {
    triangles.iter().fold([0.0; 3], |sum, [a, b, c]| {
        let n = cross3(sub(*b, *a), sub(*c, *a));
        [sum[0] + n[0], sum[1] + n[1], sum[2] + n[2]]
    })
}

/// True when `p` lies within `distance` of one of `triangles`.
pub(crate) fn near_triangles(p: [f64; 3], triangles: &[[[f64; 3]; 3]], distance: f64) -> bool {
    let limit = distance * distance;
    triangles.iter().any(|t| {
        let q = closest_on_triangle(p, t);
        let d = sub(p, q);
        dot3(d, d) <= limit
    })
}

/// The point of triangle `t` nearest `p` (by the triangle's Voronoi
/// regions: a corner, an edge, or the inside).
fn closest_on_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> [f64; 3] {
    let [a, b, c] = *t;
    let at = |base: [f64; 3], dir: [f64; 3], s: f64| [0, 1, 2].map(|k| base[k] + dir[k] * s);
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot3(ab, ap), dot3(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot3(ab, bp), dot3(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return at(a, ab, d1 / (d1 - d3));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot3(ab, cp), dot3(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return at(a, ac, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return at(b, sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = va + vb + vc;
    if denom == 0.0 {
        return a;
    }
    let (v, w) = (vb / denom, vc / denom);
    [0, 1, 2].map(|k| a[k] + ab[k] * v + ac[k] * w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_and_a_circle_are_told_apart() {
        let line = [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 2.0, 0.0]];
        assert!(as_line(&line).is_some());
        let arc: Vec<[f64; 3]> = (0..=8)
            .map(|i| {
                let a = f64::from(i) * 0.25;
                [2.0 + a.cos(), 3.0 + a.sin(), 1.6]
            })
            .collect();
        assert!(as_line(&arc).is_none());
        assert!(on_circle(&arc, [2.0, 3.0], 1.0));
        assert!(!on_circle(&arc, [2.0, 3.0], 1.1));
        assert!(winding(&arc, [2.0, 3.0]) > 0.0);
    }

    #[test]
    fn closest_point_is_inside_on_an_edge_or_at_a_corner() {
        let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(near_triangles([0.2, 0.2, 0.001], &[t], 0.002));
        assert!(!near_triangles([0.2, 0.2, 0.01], &[t], 0.002));
        assert!(near_triangles([0.5, -0.001, 0.0], &[t], 0.002));
        assert!(!near_triangles([1.1, 1.1, 0.0], &[t], 0.002));
        assert!(near_triangles([-0.001, -0.001, 0.0], &[t], 0.002));
    }

    #[test]
    fn newell_points_along_the_turn() {
        let square = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        assert!(newell(&square)[2] > 0.0);
    }
}
