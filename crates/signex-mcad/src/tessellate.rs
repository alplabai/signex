//! Triangles for the 3D view.
//!
//! Each face is triangulated in its surface's UV domain: the boundary loops
//! are sampled along the edges' 3D curves, the same parameters give the
//! loop's UV points through the edge uses' trim curves, and the UV polygon
//! (outer loop and holes) is ear-clipped. The triangles take the 3D sample
//! points, so two faces that share an edge share its points exactly and the
//! mesh is watertight. Faces are then turned to face outward (`orient`).
//!
//! The ear clipper adds no interior points. That is exact for planes and
//! good for ruled faces (a cylinder's wall, an extruded side), which is what
//! a board body has; a doubly curved face gets coarse triangles.

use earcut::Earcut;
use solidcore::Kernel;
use solidcore::topology::edgeuse::p1::uv_trim_curve;
use solidcore::topology::{Brep, EdgeuseId, FaceId};

use crate::board::new_kernel;
use crate::error::{McadError, contained};
use crate::naming::FaceName;
use crate::orient::orient_outward;
use crate::solid::Solid;
use crate::topology::{edgeuse_edge, face_loops, runs_backward};

/// How closely the triangles follow curved edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessellationTolerance {
    /// Largest turn of a curve between two samples, degrees.
    pub angle_deg: f64,
    /// Largest distance between a curve and its chord, mm.
    pub chord_mm: f64,
}

impl Default for TessellationTolerance {
    fn default() -> Self {
        Self {
            angle_deg: 10.0,
            chord_mm: 0.01,
        }
    }
}

/// A triangle mesh of a solid, mm, z up, triangles counter-clockwise seen
/// from outside.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals, smooth within a face and split between faces.
    pub normals: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// Which triangles belong to which face of the solid, in face order.
    pub faces: Vec<MeshFace>,
}

/// The triangles of one face of the solid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshFace {
    /// The face's persistent names.
    pub names: Vec<FaceName>,
    pub first_triangle: u32,
    pub triangle_count: u32,
}

impl Mesh {
    /// True when the mesh has no triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The volume the triangles enclose, mm³; positive when they face
    /// outward.
    #[must_use]
    pub fn signed_volume(&self) -> f64 {
        self.triangles
            .iter()
            .filter_map(|t| {
                let [a, b, c] = t.map(|i| self.positions.get(i as usize).copied());
                Some(triple(widen(a?), widen(b?), widen(c?)) / 6.0)
            })
            .sum()
    }
}

/// One face triangulated, before it is merged into the mesh.
pub(crate) struct FacePatch {
    pub(crate) names: Vec<FaceName>,
    pub(crate) points: Vec<[f64; 3]>,
    pub(crate) triangles: Vec<[u32; 3]>,
}

/// Tessellates `solid` for the 3D view.
///
/// # Errors
///
/// [`McadError::InvalidInput`] for a tolerance that is not finite and
/// positive; [`McadError::Tessellation`] for a face whose boundary cannot be
/// sampled; [`McadError::Kernel`] / [`McadError::KernelPanicked`] when the
/// kernel fails.
pub fn tessellate(solid: &Solid, tolerance: TessellationTolerance) -> Result<Mesh, McadError> {
    const OP: &str = "tessellate";
    let valid = |x: f64| x.is_finite() && x > 0.0;
    if !valid(tolerance.angle_deg) || !valid(tolerance.chord_mm) {
        return Err(McadError::invalid(
            OP,
            "tolerances must be finite and positive",
        ));
    }
    contained(OP, || {
        let kernel = new_kernel();
        let brep = &solid.brep;
        let mut patches = Vec::with_capacity(brep.face_list().len());
        for (index, &f) in brep.face_list().iter().enumerate() {
            let mut patch = face_patch(&kernel, brep, f, index, tolerance)?;
            patch.names = solid.names.names(brep, f);
            patches.push(patch);
        }
        orient_outward(&mut patches);
        Ok(assemble(patches))
    })
}

fn face_patch(
    kernel: &Kernel,
    brep: &Brep,
    f: FaceId,
    index: usize,
    tolerance: TessellationTolerance,
) -> Result<FacePatch, McadError> {
    let mut rings = Vec::new();
    for edgeuses in face_loops(brep, f)? {
        let ring = sample_loop(kernel, brep, &edgeuses, index, tolerance)?;
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    // Ear clipping takes the outer loop first: the one enclosing most area.
    if let Some(outer) = rings
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| uv_area(a).abs().total_cmp(&uv_area(b).abs()))
        .map(|(i, _)| i)
    {
        rings.swap(0, outer);
    }

    let mut uv = Vec::new();
    let mut points = Vec::new();
    let mut hole_starts: Vec<u32> = Vec::new();
    for (n, ring) in rings.iter().enumerate() {
        if n > 0 {
            hole_starts.push(vertex_index(index, uv.len())?);
        }
        for sample in ring {
            uv.push(sample.uv);
            points.push(sample.point);
        }
    }
    let mut corners: Vec<u32> = Vec::new();
    Earcut::new().earcut(uv.iter().copied(), &hole_starts, &mut corners);
    let triangles = corners
        .chunks_exact(3)
        .map(|c| {
            let t = [c[0], c[1], c[2]];
            // Counter-clockwise in UV is along the surface normal.
            let [a, b, c] = t.map(|i| uv.get(i as usize).copied().unwrap_or_default());
            if cross2(a, b, c) < 0.0 {
                [t[0], t[2], t[1]]
            } else {
                t
            }
        })
        .collect();
    Ok(FacePatch {
        names: Vec::new(),
        points,
        triangles,
    })
}

/// One boundary sample: where it is on the surface and in space.
struct Sample {
    uv: [f64; 2],
    point: [f64; 3],
}

/// The loop's samples in traversal order, each edge's last sample left to
/// the next edge.
fn sample_loop(
    kernel: &Kernel,
    brep: &Brep,
    edgeuses: &[EdgeuseId],
    face: usize,
    tolerance: TessellationTolerance,
) -> Result<Vec<Sample>, McadError> {
    let fail = |reason: &str| McadError::Tessellation {
        face,
        reason: reason.to_owned(),
    };
    let mut ring = Vec::new();
    for &eu in edgeuses {
        let edge = brep
            .edge(edgeuse_edge(brep, eu)?)
            .map_err(McadError::kernel("read edge"))?;
        let curve = edge.curve.as_ref().ok_or_else(|| fail("edge without curve"))?;
        let trim = uv_trim_curve(brep, eu)
            .map_err(McadError::kernel("read trim curve"))?
            .0
            .ok_or_else(|| fail("edge use without trim curve"))?;
        let samples = curve
            .tessellate(
                kernel,
                &edge.interval,
                tolerance.chord_mm,
                tolerance.angle_deg,
                1,
            )
            .map_err(McadError::kernel("sample edge"))?;
        if samples.parameters.len() != samples.points.len() || samples.points.len() < 2 {
            return Err(fail("edge sampling gave no usable points"));
        }
        let mut pairs: Vec<_> = samples.parameters.iter().zip(&samples.points).collect();
        if runs_backward(brep, eu)? {
            pairs.reverse();
        }
        pairs.truncate(pairs.len() - 1);
        for (&t, p) in pairs {
            let uv = trim
                .evaluate_point(kernel, t)
                .map_err(McadError::kernel("evaluate trim curve"))?;
            ring.push(Sample {
                uv: [uv.x, uv.y],
                point: [p.x, p.y, p.z],
            });
        }
    }
    Ok(ring)
}

fn vertex_index(face: usize, n: usize) -> Result<u32, McadError> {
    u32::try_from(n).map_err(|_| McadError::Tessellation {
        face,
        reason: "too many boundary samples".to_owned(),
    })
}

fn uv_area(ring: &[Sample]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.uv[0] * b.uv[1] - b.uv[0] * a.uv[1])
        .sum::<f64>()
        / 2.0
}

fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Merges the patches into one mesh with per-face smooth normals.
fn assemble(patches: Vec<FacePatch>) -> Mesh {
    let mut mesh = Mesh::default();
    for patch in patches {
        let base = mesh.positions.len() as u32;
        let first_triangle = mesh.triangles.len() as u32;
        let mut normals = vec![[0.0_f64; 3]; patch.points.len()];
        for t in &patch.triangles {
            let [a, b, c] = t.map(|i| patch.points.get(i as usize).copied().unwrap_or_default());
            // Area-weighted: the unnormalised cross product.
            let n = cross3(sub(b, a), sub(c, a));
            for &i in t {
                if let Some(slot) = normals.get_mut(i as usize) {
                    *slot = add(*slot, n);
                }
            }
            mesh.triangles.push(t.map(|i| base + i));
        }
        mesh.positions
            .extend(patch.points.iter().map(|p| p.map(|x| x as f32)));
        mesh.normals
            .extend(normals.into_iter().map(|n| unit(n).map(|x| x as f32)));
        mesh.faces.push(MeshFace {
            names: patch.names,
            first_triangle,
            triangle_count: mesh.triangles.len() as u32 - first_triangle,
        });
    }
    mesh
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `a` scaled to length 1; zero stays zero.
pub(crate) fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot3(a, a).sqrt();
    if length > 0.0 {
        a.map(|x| x / length)
    } else {
        a
    }
}

fn widen(p: [f32; 3]) -> [f64; 3] {
    p.map(f64::from)
}

fn triple(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    dot3(a, cross3(b, c))
}
