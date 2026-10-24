//! Triangulate a planar STEP face that has inner loops (holes).
//!
//! The face plane comes from the Newell normal of the outline. Every loop is
//! projected onto a 2D basis of that plane, `signex_sketch::geom::ear_clip_with_holes`
//! cuts the holes, and the triangle corners are mapped back to the original 3D
//! vertices. A face that cannot be cut reports why, and the caller keeps the
//! outline fill.

use std::collections::HashMap;

use signex_sketch::geom::Point2;

use crate::vrml::parser::VrmlMesh;

/// A loop's allowed distance from the face plane, relative to the outline's
/// bounding-box diagonal.
const PLANAR_TOLERANCE: f64 = 1e-4;
/// Allowed mismatch between the triangulated area and outline minus holes.
const AREA_TOLERANCE: f64 = 1e-3;

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn widen(p: &[f32; 3]) -> V3 {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

/// Newell normal of a closed loop, counter-clockwise about its direction.
/// Its length is twice the loop area.
fn newell(points: &[V3]) -> V3 {
    let mut n = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

fn signed_area_2d(ring: &[Point2]) -> f64 {
    let mut s = 0.0;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

fn tri_area_2d(a: Point2, b: Point2, c: Point2) -> f64 {
    ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
}

/// Triangulate `outline` minus `holes`. Triangles keep the winding of the
/// outline loop, so they face the same way as the outline-only fan does.
pub fn triangulate_with_holes(
    outline: &[[f32; 3]],
    holes: &[Vec<[f32; 3]>],
) -> Result<VrmlMesh, String> {
    if outline.len() < 3 {
        return Err("outline has fewer than 3 vertices".to_owned());
    }
    if holes.iter().any(|h| h.len() < 3) {
        return Err("a hole loop has fewer than 3 vertices".to_owned());
    }
    let outer: Vec<V3> = outline.iter().map(widen).collect();
    let inner: Vec<Vec<V3>> = holes
        .iter()
        .map(|h| h.iter().map(widen).collect())
        .collect();

    let normal = newell(&outer);
    let len = dot(normal, normal).sqrt();
    let (mut lo, mut hi) = (outer[0], outer[0]);
    for p in &outer {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let size = dot(sub(hi, lo), sub(hi, lo)).sqrt();
    if !len.is_finite() || len <= 1e-12 * size * size {
        return Err("degenerate face normal".to_owned());
    }
    let n = [normal[0] / len, normal[1] / len, normal[2] / len];

    // Basis (u, v, n) is right-handed, so a loop that is counter-clockwise
    // about `n` stays counter-clockwise in (u, v).
    let axis = (0..3)
        .min_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs()))
        .unwrap_or(0);
    let mut seed = [0.0; 3];
    seed[axis] = 1.0;
    let u = cross(n, seed);
    let ulen = dot(u, u).sqrt();
    let u = [u[0] / ulen, u[1] / ulen, u[2] / ulen];
    let v = cross(n, u);

    let origin = outer[0];
    let mut lift: HashMap<(u64, u64), usize> = HashMap::new();
    let mut corners: Vec<[f32; 3]> = Vec::new();
    let mut project = |ring: &[V3], src: &[[f32; 3]]| -> Result<Vec<Point2>, String> {
        let mut out = Vec::with_capacity(ring.len());
        for (p, orig) in ring.iter().zip(src) {
            let d = sub(*p, origin);
            if dot(d, n).abs() > PLANAR_TOLERANCE * size {
                return Err("non-planar loop".to_owned());
            }
            let q = Point2::new(dot(d, u), dot(d, v));
            lift.entry((q.x.to_bits(), q.y.to_bits()))
                .or_insert_with(|| {
                    corners.push(*orig);
                    corners.len() - 1
                });
            out.push(q);
        }
        Ok(out)
    };
    let outer_2d = project(&outer, outline)?;
    let mut holes_2d = Vec::with_capacity(inner.len());
    for (ring, src) in inner.iter().zip(holes) {
        holes_2d.push(project(ring, src)?);
    }

    let (merged, triangles) = signex_sketch::geom::ear_clip_with_holes(&outer_2d, &holes_2d);
    if triangles.is_empty() {
        return Err("triangulation failed".to_owned());
    }

    let expected = signed_area_2d(&outer_2d).abs()
        - holes_2d
            .iter()
            .map(|h| signed_area_2d(h).abs())
            .sum::<f64>();
    let got: f64 = triangles
        .iter()
        .map(|t| tri_area_2d(merged[t[0]], merged[t[1]], merged[t[2]]))
        .sum();
    if (got - expected).abs() > AREA_TOLERANCE * expected.abs().max(f64::MIN_POSITIVE) {
        return Err("triangulation failed".to_owned());
    }

    let mut positions = Vec::with_capacity(triangles.len() * 9);
    let mut indices = Vec::with_capacity(triangles.len() * 3);
    for tri in &triangles {
        for &i in tri {
            let q = merged[i];
            let Some(&c) = lift.get(&(q.x.to_bits(), q.y.to_bits())) else {
                return Err("triangulation failed".to_owned());
            };
            indices.push((positions.len() / 3) as u32);
            positions.extend_from_slice(&corners[c]);
        }
    }
    Ok(VrmlMesh {
        positions,
        indices,
        color: [0.65, 0.65, 0.68, 1.0],
    })
}
