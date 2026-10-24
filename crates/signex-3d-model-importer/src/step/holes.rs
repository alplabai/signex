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
/// A hole smaller than this fraction of the outline's area is degenerate.
/// A circle collapsed to its two end vertices (`[a, b, a]`) has zero area; the
/// clipper would drop it silently and return the bare outline as a success.
const MIN_HOLE_AREA_FRACTION: f64 = 1e-9;
/// Allowed mismatch between the triangulated area and outline minus holes.
const AREA_TOLERANCE: f64 = 1e-3;
/// Most vertices (outline plus holes) one face may have and still be cut.
/// A STEP file is untrusted input and ear clipping is superlinear, so a
/// crafted face must not stall the import; a bigger face keeps the outline
/// fill and says why.
pub const MAX_FACE_VERTICES: usize = 4096;
/// Most vertices cut across all faces of one file, for the same reason.
pub const MAX_FILE_CUT_VERTICES: usize = 65_536;

/// Vertices a face with `outline` and `holes` brings to the cut.
pub fn face_vertex_count(outline: &[[f32; 3]], holes: &[Vec<[f32; 3]>]) -> usize {
    outline.len() + holes.iter().map(Vec::len).sum::<usize>()
}

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
    if face_vertex_count(outline, holes) > MAX_FACE_VERTICES {
        return Err(format!("face has more than {MAX_FACE_VERTICES} vertices"));
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

    let outer_area = signed_area_2d(&outer_2d).abs();
    if holes_2d
        .iter()
        .any(|h| signed_area_2d(h).abs() <= MIN_HOLE_AREA_FRACTION * outer_area)
    {
        return Err("a hole loop has no area".to_owned());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f32, y0: f32, side: f32) -> Vec<[f32; 3]> {
        vec![
            [x0, y0, 0.0],
            [x0 + side, y0, 0.0],
            [x0 + side, y0 + side, 0.0],
            [x0, y0 + side, 0.0],
        ]
    }

    fn circle(cx: f32, cy: f32, r: f32, n: usize) -> Vec<[f32; 3]> {
        (0..n)
            .map(|i| {
                let a = -(i as f32) * std::f32::consts::TAU / n as f32;
                [cx + r * a.cos(), cy + r * a.sin(), 0.0]
            })
            .collect()
    }

    #[test]
    fn a_hole_with_no_area_is_refused_not_silently_dropped() {
        let (a, b) = ([2.0, 5.0, 0.0], [8.0, 5.0, 0.0]);
        let err = triangulate_with_holes(&square(0.0, 0.0, 10.0), &[vec![a, b, a]])
            .expect_err("zero-area hole");
        assert!(err.contains("no area"), "{err}");
    }

    #[test]
    fn a_face_within_the_vertex_cap_is_cut() {
        let hole = circle(5.0, 5.0, 2.0, 64);
        assert!(triangulate_with_holes(&square(0.0, 0.0, 10.0), &[hole]).is_ok());
    }

    /// The worst case the cap guards: a concave comb outline at the cap, plus
    /// a hole. The bound is generous (a regression to a runaway clipper takes
    /// minutes, not seconds), so it is not a flaky micro-benchmark.
    #[test]
    fn a_concave_face_at_the_vertex_cap_is_cut_in_bounded_time() {
        let teeth = (MAX_FACE_VERTICES - 4 - 4) / 4;
        let mut outline = vec![[0.0, 0.0, 0.0]];
        for i in 0..teeth {
            let x = i as f32 * 2.0;
            outline.extend([
                [x + 0.5, 0.0, 0.0],
                [x + 0.5, 5.0, 0.0],
                [x + 1.5, 5.0, 0.0],
                [x + 1.5, 0.0, 0.0],
            ]);
        }
        let end = teeth as f32 * 2.0;
        outline.extend([[end, 0.0, 0.0], [end, -5.0, 0.0], [0.0, -5.0, 0.0]]);
        let hole = square(1.0, -4.0, 2.0);
        assert!(face_vertex_count(&outline, std::slice::from_ref(&hole)) <= MAX_FACE_VERTICES);
        let start = std::time::Instant::now();
        let result = triangulate_with_holes(&outline, &[hole]);
        let took = start.elapsed();
        assert!(took.as_secs() < 30, "cut took {took:?}");
        assert!(result.is_ok(), "{result:?}");
    }

    /// A crafted STEP face must not stall the import in the superlinear ear
    /// clipper: past the cap the caller keeps the outline fill.
    #[test]
    fn a_face_over_the_vertex_cap_is_refused_before_clipping() {
        let hole = circle(5.0, 5.0, 2.0, MAX_FACE_VERTICES);
        let err =
            triangulate_with_holes(&square(0.0, 0.0, 10.0), &[hole]).expect_err("over the cap");
        assert!(err.contains("more than"), "{err}");
    }
}
