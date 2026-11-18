//! A triangle mesh as a faceted surface body: a `SHELL_BASED_SURFACE_MODEL`
//! of one `OPEN_SHELL`, each triangle an `ADVANCED_FACE` on its own
//! `PLANE`, bounded by an `EDGE_LOOP` of `LINE` edges shared with its
//! neighbours. Component models arrive as meshes (the 3D view's), which
//! need not be closed, so the body is a surface model, not a solid.

use std::collections::HashMap;

use super::StepMesh;
use super::context::placement_at;
use super::p21::{Data, Ref, flag, real, refs, string, triple};
use super::shape::perpendicular;
use crate::error::McadError;
use crate::tessellate::{cross3, dot3, sub, unit};

/// Points closer than this are welded into one vertex, mm.
const WELD_MM: f64 = 1e-6;

/// Writes `mesh` and returns its `SHELL_BASED_SURFACE_MODEL`, or `None`
/// when no triangle of it has an area.
pub(crate) fn write_mesh(data: &mut Data, mesh: &StepMesh) -> Result<Option<Ref>, McadError> {
    if !mesh.positions.iter().flatten().all(|x| x.is_finite()) {
        return Err(McadError::StepExport {
            reason: format!("body {} has a point that is not a number", mesh.name),
        });
    }
    let (points, welded) = weld(&mesh.positions);
    let mut writer = MeshWriter {
        data,
        points: &points,
        vertex_refs: HashMap::new(),
        edge_refs: HashMap::new(),
    };
    let mut faces = Vec::new();
    for t in &mesh.triangles {
        let corners = t.map(|i| welded.get(i as usize).copied());
        let [Some(a), Some(b), Some(c)] = corners else {
            return Err(McadError::StepExport {
                reason: format!("body {} has a triangle past its points", mesh.name),
            });
        };
        if a == b || b == c || c == a {
            continue;
        }
        let normal = unit(cross3(sub(points[b], points[a]), sub(points[c], points[a])));
        if dot3(normal, normal) == 0.0 {
            continue;
        }
        faces.push(writer.triangle([a, b, c], normal));
    }
    if faces.is_empty() {
        return Ok(None);
    }
    let shell = data.add(format!("OPEN_SHELL('',{})", refs(&faces)));
    Ok(Some(data.add(format!(
        "SHELL_BASED_SURFACE_MODEL({},{})",
        string(&mesh.name),
        refs(&[shell])
    ))))
}

struct MeshWriter<'a> {
    data: &'a mut Data,
    points: &'a [[f64; 3]],
    /// Per welded point: its `CARTESIAN_POINT` and `VERTEX_POINT`.
    vertex_refs: HashMap<usize, (Ref, Ref)>,
    /// Per edge, by its ends lowest first: its `EDGE_CURVE`.
    edge_refs: HashMap<(usize, usize), Ref>,
}

impl MeshWriter<'_> {
    fn vertex(&mut self, i: usize) -> (Ref, Ref) {
        if let Some(&r) = self.vertex_refs.get(&i) {
            return r;
        }
        let point = self
            .data
            .add(format!("CARTESIAN_POINT('',{})", triple(self.points[i])));
        let vertex = self.data.add(format!("VERTEX_POINT('',{point})"));
        self.vertex_refs.insert(i, (point, vertex));
        (point, vertex)
    }

    /// The edge between points `a` and `b`, and whether it runs from `a`.
    fn edge(&mut self, a: usize, b: usize) -> (Ref, bool) {
        let key = (a.min(b), a.max(b));
        if let Some(&r) = self.edge_refs.get(&key) {
            return (r, a == key.0);
        }
        let (start_point, start) = self.vertex(key.0);
        let (_, end) = self.vertex(key.1);
        let span = sub(self.points[key.1], self.points[key.0]);
        let length = dot3(span, span).sqrt();
        let direction = self
            .data
            .add(format!("DIRECTION('',{})", triple(span.map(|x| x / length))));
        let vector = self
            .data
            .add(format!("VECTOR('',{direction},{})", real(length)));
        let line = self.data.add(format!("LINE('',{start_point},{vector})"));
        let edge = self
            .data
            .add(format!("EDGE_CURVE('',{start},{end},{line},.T.)"));
        self.edge_refs.insert(key, edge);
        (edge, a == key.0)
    }

    fn triangle(&mut self, [a, b, c]: [usize; 3], normal: [f64; 3]) -> Ref {
        let mut oriented = Vec::with_capacity(3);
        for (from, to) in [(a, b), (b, c), (c, a)] {
            let (edge, forward) = self.edge(from, to);
            oriented.push(
                self.data
                    .add(format!("ORIENTED_EDGE('',*,*,{edge},{})", flag(forward))),
            );
        }
        let edge_loop = self
            .data
            .add(format!("EDGE_LOOP('',{})", refs(&oriented)));
        let bound = self
            .data
            .add(format!("FACE_OUTER_BOUND('',{edge_loop},.T.)"));
        let (point, _) = self.vertex(a);
        let axis = placement_at(self.data, point, normal, perpendicular(normal));
        let plane = self.data.add(format!("PLANE('',{axis})"));
        self.data.add(format!(
            "ADVANCED_FACE('',{},{plane},.T.)",
            refs(&[bound])
        ))
    }
}

/// The distinct points of `positions` (within [`WELD_MM`]) and, per
/// position, the index of its distinct point.
fn weld(positions: &[[f64; 3]]) -> (Vec<[f64; 3]>, Vec<usize>) {
    let mut points = Vec::new();
    let mut index: HashMap<[i64; 3], usize> = HashMap::new();
    let welded = positions
        .iter()
        .map(|p| {
            let key = p.map(|x| (x / WELD_MM).round() as i64);
            *index.entry(key).or_insert_with(|| {
                points.push(*p);
                points.len() - 1
            })
        })
        .collect();
    (points, welded)
}
