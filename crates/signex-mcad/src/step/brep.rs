//! A [`Solid`] as exact B-rep entities (ISO 10303-42 topology and
//! geometry): one `MANIFOLD_SOLID_BREP` per connected shell, its faces
//! `ADVANCED_FACE`s on a `PLANE` or a `CYLINDRICAL_SURFACE`, its edges
//! `EDGE_CURVE`s along a `LINE` or a `CIRCLE`.
//!
//! The topology is the kernel's: every face, loop, edge use and vertex is
//! written as the kernel holds it. The geometry is read off it and
//! checked: a face is a cylinder when it is the wall of a round hole and
//! all its boundary lies on that hole's cylinder, otherwise a plane its
//! boundary lies in; an edge is a line or a hole's circle when its samples
//! lie on one. Anything else is refused, so the file never holds a shape
//! the solid does not have.
//!
//! Orientation follows the standard: a face's normal (its surface normal,
//! reversed when `same_sense` is false) points out of the solid, and each
//! bound is written so the face lies to its left seen from that side.
//! Outward comes from the solid's tessellation, whose triangles face out.

use std::collections::HashMap;

use solidcore::Kernel;
use solidcore::topology::{Brep, EdgeId, FaceId, VertexId};

use super::context::placement;
use super::p21::{Data, Ref, flag, real, refs, string, triple};
use super::shape::{
    ON_GEOMETRY_MM, as_line, facing, near_triangles, newell, on_circle, on_cylinder,
    perpendicular, winding,
};
use crate::board::new_kernel;
use crate::error::{McadError, contained};
use crate::naming::FaceName;
use crate::solid::Solid;
use crate::tessellate::{Mesh, TessellationTolerance, cross3, dot3, sub, tessellate, unit};
use crate::topology::{edge_faces, edgeuse_edge, face_loops, runs_backward};

/// How finely edges are sampled to read their shape, and faces tessellated
/// to tell their outside.
const SAMPLING: TessellationTolerance = TessellationTolerance {
    angle_deg: 5.0,
    chord_mm: 1e-3,
};

/// An edge's ends and its samples from start to end along its curve.
struct EdgeSamples {
    start: VertexId,
    end: VertexId,
    points: Vec<[f64; 3]>,
}

/// The surface a face lies on.
#[derive(Clone, Copy)]
enum Surface {
    /// `normal` is the face's outward normal.
    Plane { normal: [f64; 3] },
    /// `outward_from_axis`: the face's outside is away from the axis.
    Cylinder {
        center: [f64; 2],
        radius: f64,
        outward_from_axis: bool,
    },
}

/// One loop of a face: its edge uses (edge, runs along the edge) and the
/// closed polyline it traces.
struct Loop {
    uses: Vec<(EdgeId, bool)>,
    ring: Vec<[f64; 3]>,
}

fn refused(reason: impl Into<String>) -> McadError {
    McadError::StepExport {
        reason: reason.into(),
    }
}

/// Writes `solid` and returns its `MANIFOLD_SOLID_BREP`s, one per
/// connected shell, named `name` (numbered when there are several).
pub(crate) fn write_solid(
    data: &mut Data,
    solid: &Solid,
    name: &str,
) -> Result<Vec<Ref>, McadError> {
    contained("write STEP solid", || {
        let kernel = new_kernel();
        let mesh = tessellate(solid, SAMPLING)?;
        let edges = sample_edges(&kernel, &solid.brep)?;
        let mut writer = SolidWriter {
            data,
            solid,
            edges,
            vertex_refs: HashMap::new(),
            edge_refs: HashMap::new(),
        };
        let face_list = solid.brep.face_list().to_vec();
        let mut faces = Vec::with_capacity(face_list.len());
        for (index, &f) in face_list.iter().enumerate() {
            let triangles = face_triangles(&mesh, index);
            faces.push(writer.face(f, &triangles)?);
        }
        let groups = shells(&solid.brep, &face_list)?;
        let several = groups.len() > 1;
        let data = writer.data;
        Ok(groups
            .iter()
            .enumerate()
            .map(|(n, group)| {
                let members: Vec<Ref> = group.iter().map(|&i| faces[i]).collect();
                let shell = data.add(format!("CLOSED_SHELL('',{})", refs(&members)));
                let label = if several {
                    format!("{name} {}", n + 1)
                } else {
                    name.to_owned()
                };
                data.add(format!("MANIFOLD_SOLID_BREP({},{shell})", string(&label)))
            })
            .collect())
    })
}

struct SolidWriter<'a> {
    data: &'a mut Data,
    solid: &'a Solid,
    edges: HashMap<EdgeId, EdgeSamples>,
    vertex_refs: HashMap<VertexId, Ref>,
    edge_refs: HashMap<EdgeId, Ref>,
}

impl SolidWriter<'_> {
    fn brep(&self) -> &Brep {
        &self.solid.brep
    }

    fn vertex(&mut self, v: VertexId) -> Result<Ref, McadError> {
        if let Some(&r) = self.vertex_refs.get(&v) {
            return Ok(r);
        }
        let p = vertex_point(self.brep(), v)?;
        let point = self.data.add(format!("CARTESIAN_POINT('',{})", triple(p)));
        let r = self.data.add(format!("VERTEX_POINT('',{point})"));
        self.vertex_refs.insert(v, r);
        Ok(r)
    }

    fn edge(&mut self, e: EdgeId) -> Result<Ref, McadError> {
        if let Some(&r) = self.edge_refs.get(&e) {
            return Ok(r);
        }
        let samples = self
            .edges
            .get(&e)
            .ok_or_else(|| refused("a loop uses an edge the solid does not list"))?;
        let (start, end) = (samples.start, samples.end);
        let points = samples.points.clone();
        let (curve, same_sense) = self.curve(e, start == end, &points)?;
        let start = self.vertex(start)?;
        let end = self.vertex(end)?;
        let r = self.data.add(format!(
            "EDGE_CURVE('',{start},{end},{curve},{})",
            flag(same_sense)
        ));
        self.edge_refs.insert(e, r);
        Ok(r)
    }

    /// The curve edge `e` runs along, and whether it runs the curve's way.
    fn curve(
        &mut self,
        e: EdgeId,
        closed: bool,
        points: &[[f64; 3]],
    ) -> Result<(Ref, bool), McadError> {
        if !closed && let Some((direction, length)) = as_line(points) {
            let start = self
                .data
                .add(format!("CARTESIAN_POINT('',{})", triple(points[0])));
            let direction = self
                .data
                .add(format!("DIRECTION('',{})", triple(direction)));
            let vector = self
                .data
                .add(format!("VECTOR('',{direction},{})", real(length)));
            return Ok((self.data.add(format!("LINE('',{start},{vector})")), true));
        }
        let faces = edge_faces(self.brep(), e)?;
        for f in faces {
            let Some((center, radius)) = self.hole_cylinder(f) else {
                continue;
            };
            if !on_circle(points, center, radius) {
                continue;
            }
            let z = points[0][2];
            let axis = placement(
                self.data,
                [center[0], center[1], z],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
            );
            let circle = self
                .data
                .add(format!("CIRCLE('',{axis},{})", real(radius)));
            return Ok((circle, winding(points, center) > 0.0));
        }
        Err(refused(
            "an edge is neither a straight line nor the rim of a round hole",
        ))
    }

    /// The cylinder of the round hole whose wall `f` is, if it is one.
    fn hole_cylinder(&self, f: FaceId) -> Option<([f64; 2], f64)> {
        self.solid
            .names
            .names(self.brep(), f)
            .into_iter()
            .find_map(|name| match name {
                FaceName::HoleWall(n) => *self.solid.round_holes.get(n as usize)?,
                _ => None,
            })
    }

    fn face(&mut self, f: FaceId, triangles: &[[[f64; 3]; 3]]) -> Result<Ref, McadError> {
        let loops = self.loops(f)?;
        let surface = self.surface(f, &loops, triangles)?;
        let (flags, outer) = match surface {
            Surface::Plane { normal } => plane_bounds(&loops, normal),
            Surface::Cylinder {
                center,
                radius,
                outward_from_axis,
            } => (
                cylinder_bounds(&loops, center, radius, outward_from_axis, triangles)?,
                None,
            ),
        };
        let mut bounds = Vec::with_capacity(loops.len());
        for (n, l) in loops.iter().enumerate() {
            let mut oriented = Vec::with_capacity(l.uses.len());
            for &(e, forward) in &l.uses {
                let edge = self.edge(e)?;
                oriented.push(self.data.add(format!(
                    "ORIENTED_EDGE('',*,*,{edge},{})",
                    flag(forward)
                )));
            }
            let edge_loop = self
                .data
                .add(format!("EDGE_LOOP('',{})", refs(&oriented)));
            let kind = if outer == Some(n) {
                "FACE_OUTER_BOUND"
            } else {
                "FACE_BOUND"
            };
            bounds.push(
                self.data
                    .add(format!("{kind}('',{edge_loop},{})", flag(flags[n]))),
            );
        }
        let (geometry, same_sense) = match surface {
            Surface::Plane { normal } => {
                let origin = outer
                    .and_then(|n| loops.get(n))
                    .and_then(|l| l.ring.first().copied())
                    .unwrap_or_default();
                let axis = placement(self.data, origin, normal, perpendicular(normal));
                (self.data.add(format!("PLANE('',{axis})")), true)
            }
            Surface::Cylinder {
                center,
                radius,
                outward_from_axis,
            } => {
                let axis = placement(
                    self.data,
                    [center[0], center[1], 0.0],
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                );
                let cylinder = self.data.add(format!(
                    "CYLINDRICAL_SURFACE('',{axis},{})",
                    real(radius)
                ));
                (cylinder, outward_from_axis)
            }
        };
        let label = face_label(&self.solid.names.names(self.brep(), f));
        Ok(self.data.add(format!(
            "ADVANCED_FACE({},{},{geometry},{})",
            string(&label),
            refs(&bounds),
            flag(same_sense)
        )))
    }

    /// The loops of `f` in the kernel's order, each checked to close.
    fn loops(&self, f: FaceId) -> Result<Vec<Loop>, McadError> {
        let brep = self.brep();
        let mut out = Vec::new();
        for edgeuses in face_loops(brep, f)? {
            let mut uses = Vec::with_capacity(edgeuses.len());
            for eu in edgeuses {
                uses.push((edgeuse_edge(brep, eu)?, !runs_backward(brep, eu)?));
            }
            let mut ring = Vec::new();
            for (i, &(e, forward)) in uses.iter().enumerate() {
                let (_, to, points) = self.walk(e, forward)?;
                let (next, next_forward) = uses[(i + 1) % uses.len()];
                let (next_from, _, _) = self.walk(next, next_forward)?;
                if to != next_from {
                    return Err(refused("a face loop does not close"));
                }
                ring.extend_from_slice(&points[..points.len() - 1]);
            }
            out.push(Loop { uses, ring });
        }
        Ok(out)
    }

    /// Edge `e` walked forward or backward: its first and last vertex and
    /// its samples in walking order.
    fn walk(
        &self,
        e: EdgeId,
        forward: bool,
    ) -> Result<(VertexId, VertexId, Vec<[f64; 3]>), McadError> {
        let samples = self
            .edges
            .get(&e)
            .ok_or_else(|| refused("a loop uses an edge the solid does not list"))?;
        let mut points = samples.points.clone();
        if forward {
            Ok((samples.start, samples.end, points))
        } else {
            points.reverse();
            Ok((samples.end, samples.start, points))
        }
    }

    fn surface(
        &self,
        f: FaceId,
        loops: &[Loop],
        triangles: &[[[f64; 3]; 3]],
    ) -> Result<Surface, McadError> {
        let points: Vec<[f64; 3]> = loops.iter().flat_map(|l| l.ring.iter().copied()).collect();
        if let Some((center, radius)) = self.hole_cylinder(f) {
            if !on_cylinder(&points, center, radius) {
                return Err(refused("a hole wall does not lie on its hole's cylinder"));
            }
            let away: f64 = triangles
                .iter()
                .map(|[a, b, c]| {
                    let n = cross3(sub(*b, *a), sub(*c, *a));
                    let mid = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
                    n[0] * (mid[0] - center[0]) + n[1] * (mid[1] - center[1])
                })
                .sum();
            if away == 0.0 {
                return Err(refused("a hole wall has no triangles"));
            }
            return Ok(Surface::Cylinder {
                center,
                radius,
                outward_from_axis: away > 0.0,
            });
        }
        let widest = loops
            .iter()
            .map(|l| newell(&l.ring))
            .max_by(|a, b| dot3(*a, *a).total_cmp(&dot3(*b, *b)))
            .ok_or_else(|| refused("a face has no boundary"))?;
        let mut normal = unit(widest);
        if dot3(normal, normal) == 0.0 {
            return Err(refused("a face boundary encloses no area"));
        }
        let anchor = points.first().copied().unwrap_or_default();
        let flat = points
            .iter()
            .all(|&p| dot3(sub(p, anchor), normal).abs() <= ON_GEOMETRY_MM);
        if !flat {
            return Err(refused(
                "a face is neither flat nor the wall of a round hole",
            ));
        }
        let out = facing(triangles);
        if dot3(out, out) == 0.0 {
            return Err(refused("a flat face has no triangles"));
        }
        if dot3(normal, out) < 0.0 {
            normal = normal.map(|x| -x);
        }
        Ok(Surface::Plane { normal })
    }
}

/// Bound flags of a plane's loops: the widest loop is the outer bound and
/// turns counter-clockwise about the outward normal, the others clockwise.
fn plane_bounds(loops: &[Loop], normal: [f64; 3]) -> (Vec<bool>, Option<usize>) {
    let areas: Vec<[f64; 3]> = loops.iter().map(|l| newell(&l.ring)).collect();
    let outer = areas
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| dot3(**a, **a).total_cmp(&dot3(**b, **b)))
        .map(|(n, _)| n);
    let flags = areas
        .iter()
        .enumerate()
        .map(|(n, a)| (dot3(*a, normal) > 0.0) == (outer == Some(n)))
        .collect();
    (flags, outer)
}

/// Bound flags of a hole wall's loops. A loop on a cylinder has no
/// enclosed area to read its turn from, so each boundary step asks which
/// side of it the face is on: a point a little to its left and one a
/// little to its right, put back on the cylinder, are tested against the
/// face's triangles. Steps along a seam (face on both sides) do not vote.
fn cylinder_bounds(
    loops: &[Loop],
    center: [f64; 2],
    radius: f64,
    outward_from_axis: bool,
    triangles: &[[[f64; 3]; 3]],
) -> Result<Vec<bool>, McadError> {
    let (low, high) = triangles
        .iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[2]), hi.max(p[2]))
        });
    let step = 0.25 * (high - low).min(radius);
    if step.is_nan() || step <= 4.0 * SAMPLING.chord_mm {
        return Err(refused("a hole wall is too small to export"));
    }
    let onto_cylinder = |p: [f64; 3]| {
        let (dx, dy) = (p[0] - center[0], p[1] - center[1]);
        let r = dx.hypot(dy).max(f64::MIN_POSITIVE);
        [
            center[0] + dx * radius / r,
            center[1] + dy * radius / r,
            p[2],
        ]
    };
    loops
        .iter()
        .map(|l| {
            let mut votes = 0i64;
            for (i, &p) in l.ring.iter().enumerate() {
                let q = l.ring[(i + 1) % l.ring.len()];
                let mid = [0, 1, 2].map(|k| (p[k] + q[k]) / 2.0);
                let radial = unit([mid[0] - center[0], mid[1] - center[1], 0.0]);
                let normal = if outward_from_axis {
                    radial
                } else {
                    radial.map(|x| -x)
                };
                let left = unit(cross3(normal, sub(q, p)));
                if dot3(left, left) == 0.0 {
                    continue;
                }
                let side = |sign: f64| onto_cylinder([0, 1, 2].map(|k| mid[k] + sign * step * left[k]));
                let on_left = near_triangles(side(1.0), triangles, step / 2.0);
                let on_right = near_triangles(side(-1.0), triangles, step / 2.0);
                votes += i64::from(on_left && !on_right) - i64::from(on_right && !on_left);
            }
            if votes == 0 {
                return Err(refused("cannot tell which side of a hole wall's bound the wall is on"));
            }
            Ok(votes > 0)
        })
        .collect()
}

/// The triangles of face `index` of `mesh`.
fn face_triangles(mesh: &Mesh, index: usize) -> Vec<[[f64; 3]; 3]> {
    let Some(face) = mesh.faces.get(index) else {
        return Vec::new();
    };
    let first = face.first_triangle as usize;
    let count = face.triangle_count as usize;
    mesh.triangles
        .get(first..first + count)
        .unwrap_or_default()
        .iter()
        .filter_map(|t| {
            let corner = |i: u32| {
                mesh.positions
                    .get(i as usize)
                    .map(|p| p.map(f64::from))
            };
            Some([corner(t[0])?, corner(t[1])?, corner(t[2])?])
        })
        .collect()
}

/// Every edge's ends and samples, from start to end along its curve, the
/// first and last sample put exactly on its vertices.
fn sample_edges(kernel: &Kernel, brep: &Brep) -> Result<HashMap<EdgeId, EdgeSamples>, McadError> {
    let mut out = HashMap::new();
    for &e in brep.edge_list() {
        let record = brep.edge(e).map_err(McadError::kernel("read edge"))?;
        let curve = record
            .curve
            .as_ref()
            .ok_or_else(|| refused("an edge has no curve"))?;
        let samples = curve
            .tessellate(
                kernel,
                &record.interval,
                SAMPLING.chord_mm,
                SAMPLING.angle_deg,
                1,
            )
            .map_err(McadError::kernel("sample edge"))?;
        let mut points: Vec<[f64; 3]> = samples.points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
            return Err(refused("an edge gave no samples"));
        };
        if points.len() < 2 {
            return Err(refused("an edge gave one sample"));
        }
        let [a, b] = record.vertices;
        let (pa, pb) = (vertex_point(brep, a)?, vertex_point(brep, b)?);
        let near = |p: [f64; 3], q: [f64; 3]| {
            let d = sub(p, q);
            dot3(d, d).sqrt() <= 100.0 * ON_GEOMETRY_MM
        };
        let (start, end, ps, pe) = if near(first, pa) && near(last, pb) {
            (a, b, pa, pb)
        } else if near(first, pb) && near(last, pa) {
            (b, a, pb, pa)
        } else {
            return Err(refused("an edge does not end on its vertices"));
        };
        let n = points.len();
        points[0] = ps;
        points[n - 1] = pe;
        out.insert(e, EdgeSamples { start, end, points });
    }
    Ok(out)
}

fn vertex_point(brep: &Brep, v: VertexId) -> Result<[f64; 3], McadError> {
    let p = brep
        .vertex(v)
        .map_err(McadError::kernel("read vertex"))?
        .point;
    Ok([p.x, p.y, p.z])
}

/// The faces of `face_list` grouped by the edges they share: one group per
/// connected shell, in face order.
fn shells(brep: &Brep, face_list: &[FaceId]) -> Result<Vec<Vec<usize>>, McadError> {
    let index: HashMap<FaceId, usize> = face_list.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let mut parent: Vec<usize> = (0..face_list.len()).collect();
    for &e in brep.edge_list() {
        let faces: Vec<usize> = edge_faces(brep, e)?
            .into_iter()
            .filter_map(|f| index.get(&f).copied())
            .collect();
        for pair in faces.windows(2) {
            let (a, b) = (root(&mut parent, pair[0]), root(&mut parent, pair[1]));
            parent[a.max(b)] = a.min(b);
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of: HashMap<usize, usize> = HashMap::new();
    for i in 0..face_list.len() {
        let r = root(&mut parent, i);
        let g = *group_of.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    Ok(groups)
}

/// The set `i` belongs to, in a union-find forest.
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The face's persistent names as its STEP name, e.g. `Top` or
/// `Hole 3 wall`.
fn face_label(names: &[FaceName]) -> String {
    let parts: Vec<String> = names
        .iter()
        .map(|name| match name {
            FaceName::Top => "Top".to_owned(),
            FaceName::Bottom => "Bottom".to_owned(),
            FaceName::OutlineSide(n) => format!("Outline side {n}"),
            FaceName::HoleWall(n) => format!("Hole {n} wall"),
            FaceName::CutoutWall(n) => format!("Cut-out {n} wall"),
        })
        .collect();
    parts.join("; ")
}
