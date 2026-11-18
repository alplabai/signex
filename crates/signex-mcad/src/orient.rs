//! Turns every face's triangles to face out of the solid.
//!
//! Each face is triangulated along its surface normal, which may point into
//! the solid. For each face a point just off its largest triangle, on the
//! normal side, is tested against the whole mesh: a ray from it crossing the
//! surface an odd number of times starts inside, so that face is flipped.

use crate::tessellate::{FacePatch, cross3, dot3, sub, unit};

/// Ray skew off the face normal, so the ray does not run along an edge or
/// graze a face that contains the normal.
const SKEW: [f64; 3] = [0.000_37, 0.000_61, 0.000_71];

pub(crate) fn orient_outward(patches: &mut [FacePatch]) {
    let triangles: Vec<[[f64; 3]; 3]> = patches
        .iter()
        .flat_map(|patch| {
            patch
                .triangles
                .iter()
                .filter_map(|t| corners(&patch.points, *t))
        })
        .collect();
    let offset = 1e-6 * extent(&triangles).max(1e-3);
    let flips: Vec<bool> = patches
        .iter()
        .map(|patch| points_inward(patch, &triangles, offset))
        .collect();
    for (patch, flip) in patches.iter_mut().zip(flips) {
        if flip {
            for t in &mut patch.triangles {
                t.swap(1, 2);
            }
        }
    }
}

fn points_inward(patch: &FacePatch, all: &[[[f64; 3]; 3]], offset: f64) -> bool {
    let largest = patch
        .triangles
        .iter()
        .filter_map(|t| corners(&patch.points, *t))
        .map(|[a, b, c]| ([a, b, c], cross3(sub(b, a), sub(c, a))))
        .max_by(|(_, m), (_, n)| dot3(*m, *m).total_cmp(&dot3(*n, *n)));
    let Some(([a, b, c], normal)) = largest else {
        return false;
    };
    let normal = unit(normal);
    if dot3(normal, normal) == 0.0 {
        return false;
    }
    let centroid = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
    let start = [0, 1, 2].map(|k| centroid[k] + normal[k] * offset);
    let direction = unit([0, 1, 2].map(|k| normal[k] + SKEW[k]));
    let crossings = all
        .iter()
        .filter(|t| ray_hits(start, direction, t))
        .count();
    crossings % 2 == 1
}

fn corners(points: &[[f64; 3]], t: [u32; 3]) -> Option<[[f64; 3]; 3]> {
    let [a, b, c] = t.map(|i| points.get(i as usize).copied());
    Some([a?, b?, c?])
}

/// The diagonal of the triangles' bounding box.
fn extent(triangles: &[[[f64; 3]; 3]]) -> f64 {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in triangles.iter().flatten() {
        for ((l, h), &x) in low.iter_mut().zip(high.iter_mut()).zip(p) {
            *l = l.min(x);
            *h = h.max(x);
        }
    }
    let d = sub(high, low);
    let length = dot3(d, d).sqrt();
    if length.is_finite() { length } else { 0.0 }
}

/// Möller–Trumbore: does the ray from `start` along `direction` cross the
/// triangle ahead of its start?
fn ray_hits(start: [f64; 3], direction: [f64; 3], [a, b, c]: &[[f64; 3]; 3]) -> bool {
    const PARALLEL: f64 = 1e-14;
    let e1 = sub(*b, *a);
    let e2 = sub(*c, *a);
    let p = cross3(direction, e2);
    let det = dot3(e1, p);
    if det.abs() < PARALLEL {
        return false;
    }
    let inv = 1.0 / det;
    let s = sub(start, *a);
    let u = dot3(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross3(s, e1);
    let v = dot3(direction, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    dot3(e2, q) * inv > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_crosses_a_triangle_ahead_and_not_behind() {
        let t = [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]];
        assert!(ray_hits([0.2, 0.2, 0.0], [0.0, 0.0, 1.0], &t));
        assert!(!ray_hits([0.2, 0.2, 2.0], [0.0, 0.0, 1.0], &t));
        assert!(!ray_hits([0.8, 0.8, 0.0], [0.0, 0.0, 1.0], &t));
    }
}
