//! Triangle meshes for 3D views (#2390): plain data the mesh pipeline
//! uploads and a ray can hit. Positions are millimetres in a right-handed
//! frame, z up out of the view's top face.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs, Moller-Trumbore ray-triangle test (1997).

use std::ops::Range;

/// One mesh vertex: where it is and the colour it is drawn in.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Indexed triangles. Each part a caller appends gets the range of
/// `indices` it owns, which is what a highlight draws again and a pick
/// tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene3d {
    pub vertices: Vec<MeshVertex>,
    /// Three per triangle, into `vertices`.
    pub indices: Vec<u32>,
}

impl Scene3d {
    /// Append `positions` as triangles `triangles` (indices into
    /// `positions`), all in `color`. Returns the range of `indices` they
    /// take. A triangle naming a vertex past `positions` is skipped.
    pub fn append(
        &mut self,
        positions: &[[f32; 3]],
        triangles: &[[u32; 3]],
        color: [f32; 4],
    ) -> Range<u32> {
        let start = self.indices.len() as u32;
        let base = self.vertices.len() as u32;
        let count = positions.len() as u32;
        self.vertices.extend(
            positions
                .iter()
                .map(|&position| MeshVertex { position, color }),
        );
        for tri in triangles {
            if tri.iter().all(|&i| i < count) {
                self.indices.extend(tri.iter().map(|&i| base + i));
            }
        }
        start..self.indices.len() as u32
    }

    /// The flat polygon `outline` with `holes` cut out, at height `z`.
    /// Holes under 3 points are skipped; holes that do not leave the area
    /// they should (overlapping or touching the outline) are dropped and
    /// the outline fills whole, as the flat view draws such a zone.
    pub fn append_face(
        &mut self,
        outline: &[[f32; 2]],
        holes: &[Vec<[f32; 2]>],
        z: f32,
        color: [f32; 4],
    ) -> Range<u32> {
        let (points, triangles) = triangulate(outline, holes);
        let positions: Vec<[f32; 3]> = points.iter().map(|p| [p[0], p[1], z]).collect();
        self.append(&positions, &triangles, color)
    }

    /// The solid between heights `z0` and `z1` over `outline` minus
    /// `holes`: both faces in `face_color`, the walls of the outline and of
    /// every hole in `wall_color`.
    pub fn append_prism(
        &mut self,
        outline: &[[f32; 2]],
        holes: &[Vec<[f32; 2]>],
        (z0, z1): (f32, f32),
        face_color: [f32; 4],
        wall_color: [f32; 4],
    ) -> Range<u32> {
        let start = self.indices.len() as u32;
        let (points, triangles) = triangulate(outline, holes);
        for z in [z0, z1] {
            let positions: Vec<[f32; 3]> = points.iter().map(|p| [p[0], p[1], z]).collect();
            self.append(&positions, &triangles, face_color);
        }
        let rings = std::iter::once(outline).chain(
            holes
                .iter()
                .map(Vec::as_slice)
                .filter(|hole| hole.len() >= 3),
        );
        for ring in rings.filter(|ring| ring.len() >= 3) {
            self.append_walls(ring, z0, z1, wall_color);
        }
        start..self.indices.len() as u32
    }

    /// A quad from each edge of the closed `ring` between `z0` and `z1`.
    fn append_walls(&mut self, ring: &[[f32; 2]], z0: f32, z1: f32, color: [f32; 4]) {
        let mut positions = Vec::with_capacity(ring.len() * 4);
        let mut triangles = Vec::with_capacity(ring.len() * 2);
        for (i, a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            let base = positions.len() as u32;
            positions.extend([
                [a[0], a[1], z0],
                [b[0], b[1], z0],
                [b[0], b[1], z1],
                [a[0], a[1], z1],
            ]);
            triangles.extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
        }
        self.append(&positions, &triangles, color);
    }

    /// The number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The corners of the box around the vertices `range` uses; `None` for
    /// an empty range.
    pub fn bounds_of(&self, range: Range<u32>) -> Option<([f32; 3], [f32; 3])> {
        let mut out: Option<([f32; 3], [f32; 3])> = None;
        for &i in self.indices.get(range.start as usize..range.end as usize)? {
            let p = self.vertices.get(i as usize)?.position;
            out = Some(match out {
                None => (p, p),
                Some((lo, hi)) => (
                    [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                    [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
                ),
            });
        }
        out
    }

    /// The corners of the box around every vertex; `None` when empty.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.bounds_of(0..self.indices.len() as u32)
    }

    /// The distance along `dir` (from `origin`) of the nearest triangle in
    /// `range` the ray meets, either face; `None` when it meets none.
    pub fn ray_hit(&self, range: Range<u32>, origin: [f32; 3], dir: [f32; 3]) -> Option<f32> {
        let indices = self.indices.get(range.start as usize..range.end as usize)?;
        let mut nearest: Option<f32> = None;
        for tri in indices.chunks_exact(3) {
            let corner = |k: usize| self.vertices.get(tri[k] as usize).map(|v| v.position);
            let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
                continue;
            };
            if let Some(t) = ray_triangle(origin, dir, a, b, c)
                && nearest.is_none_or(|n| t < n)
            {
                nearest = Some(t);
            }
        }
        nearest
    }
}

/// Triangles filling `outline` minus `holes`: the points and the index
/// triples into them. The same ear clip the flat view fills with; when the
/// holes do not leave the area they should, the outline fills whole, and a
/// contour the clip refuses fills as a fan.
fn triangulate(outline: &[[f32; 2]], holes: &[Vec<[f32; 2]>]) -> (Vec<[f32; 2]>, Vec<[u32; 3]>) {
    use signex_sketch::geom::{Point2, ear_clip, ear_clip_with_holes, signed_area};
    if outline.len() < 3 {
        return (Vec::new(), Vec::new());
    }
    let to_2d = |p: &[f32; 2]| Point2::new(f64::from(p[0]), f64::from(p[1]));
    let outer: Vec<Point2> = outline.iter().map(to_2d).collect();
    let rings: Vec<Vec<Point2>> = holes
        .iter()
        .filter(|hole| hole.len() >= 3)
        .map(|hole| hole.iter().map(to_2d).collect())
        .collect();
    let index = |t: [usize; 3]| [t[0] as u32, t[1] as u32, t[2] as u32];
    if !rings.is_empty() {
        let (merged, triangles) = ear_clip_with_holes(&outer, &rings);
        let expected =
            signed_area(&outer).abs() - rings.iter().map(|h| signed_area(h).abs()).sum::<f64>();
        let filled: f64 = triangles
            .iter()
            .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
            .sum();
        if !triangles.is_empty()
            && filled.is_finite()
            && expected.is_finite()
            && (filled - expected).abs() <= expected.abs() * 1e-6 + 1e-9
        {
            let points = merged.iter().map(|p| [p.x as f32, p.y as f32]).collect();
            return (points, triangles.into_iter().map(index).collect());
        }
    }
    let triangles = ear_clip(&outer);
    if !triangles.is_empty() {
        return (outline.to_vec(), triangles.into_iter().map(index).collect());
    }
    let fan = (1..outline.len() as u32 - 1)
        .map(|i| [0, i, i + 1])
        .collect();
    (outline.to_vec(), fan)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The ray parameter where `origin + t * dir` meets triangle `abc`, for
/// `t > 0`; both faces count.
fn ray_triangle(
    origin: [f32; 3],
    dir: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> Option<f32> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, a);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t > 0.0 && t.is_finite()).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];

    fn square(z: f32) -> Scene3d {
        let mut scene = Scene3d::default();
        scene.append(
            &[[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]],
            &[[0, 1, 2], [0, 2, 3]],
            GREY,
        );
        scene
    }

    #[test]
    fn append_returns_the_index_range_of_the_part() {
        let mut scene = square(0.0);
        let second = scene.append(
            &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[[0, 1, 2]],
            GREY,
        );
        assert_eq!(second, 6..9);
        assert_eq!(&scene.indices[6..9], &[4, 5, 6]);
        assert_eq!(scene.triangle_count(), 3);
    }

    #[test]
    fn a_triangle_past_its_positions_is_skipped() {
        let mut scene = Scene3d::default();
        let range = scene.append(&[[0.0; 3]], &[[0, 1, 2]], GREY);
        assert!(range.is_empty());
    }

    #[test]
    fn a_ray_down_hits_the_square_at_its_height() {
        let scene = square(2.0);
        let t = scene.ray_hit(0..6, [0.5, 0.5, 10.0], [0.0, 0.0, -1.0]);
        assert_eq!(t, Some(8.0));
        let up = scene.ray_hit(0..6, [0.5, 0.5, -10.0], [0.0, 0.0, 1.0]);
        assert_eq!(up, Some(12.0), "the back face counts too");
        assert_eq!(
            scene.ray_hit(0..6, [2.0, 2.0, 10.0], [0.0, 0.0, -1.0]),
            None
        );
    }

    /// A slab with a hole: a ray straight down through the hole meets
    /// nothing, one beside it meets the top face.
    #[test]
    fn a_prism_is_open_through_its_holes() {
        let mut scene = Scene3d::default();
        let outline = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]];
        let range = scene.append_prism(&outline, &[hole], (0.0, 1.6), GREY, GREY);
        assert_eq!(range, 0..scene.indices.len() as u32);
        assert_eq!(scene.bounds(), Some(([0.0, 0.0, 0.0], [10.0, 10.0, 1.6])));
        let down = [0.0, 0.0, -1.0];
        assert_eq!(scene.ray_hit(range.clone(), [5.0, 5.0, 10.0], down), None);
        let t = scene
            .ray_hit(range, [1.0, 1.0, 10.0], down)
            .expect("the top face");
        assert!((t - 8.4).abs() < 1e-4, "{t}");
    }

    #[test]
    fn bounds_cover_the_vertices_a_range_uses() {
        let scene = square(3.0);
        assert_eq!(scene.bounds(), Some(([0.0, 0.0, 3.0], [1.0, 1.0, 3.0])));
        assert_eq!(scene.bounds_of(0..0), None);
    }
}
