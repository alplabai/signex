//! A triangle mesh as a faceted surface body: a `SHELL_BASED_SURFACE_MODEL`
//! of `OPEN_SHELL`s (one per connected, edge-consistent part), each
//! triangle an `ADVANCED_FACE` on its own `PLANE`, bounded by an
//! `EDGE_LOOP` of `LINE` edges shared with its neighbours. Component models
//! arrive as meshes (the 3D view's), which need not be closed, so the body
//! is a surface model, not a solid.

use std::collections::{HashMap, VecDeque};

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
    let mut kept: Vec<([usize; 3], [f64; 3])> = Vec::new();
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
        kept.push(([a, b, c], normal));
    }
    if kept.is_empty() {
        return Ok(None);
    }
    let corners: Vec<[usize; 3]> = kept.iter().map(|(t, _)| *t).collect();
    let mut shells = Vec::new();
    for shell in open_shells(&corners) {
        // An edge belongs to one shell: shells share no `EDGE_CURVE`.
        writer.edge_refs.clear();
        let faces: Vec<Ref> = shell
            .into_iter()
            .map(|i| writer.triangle(kept[i].0, kept[i].1))
            .collect();
        shells.push(writer.data.add(format!("OPEN_SHELL('',{})", refs(&faces))));
    }
    Ok(Some(data.add(format!(
        "SHELL_BASED_SURFACE_MODEL({},{})",
        string(&mesh.name),
        refs(&shells)
    ))))
}

/// The triangles of `triangles` (by index) grouped into shells an
/// `open_shell` accepts (ISO 10303-42 `connected_face_set`): each shell
/// is connected through shared edges, and uses each edge at most twice,
/// then in opposite senses. A triangle that would break that in one shell
/// (a third face on an edge, a flipped neighbour) starts or joins another.
/// Each shell lists its triangles in ascending order.
fn open_shells(triangles: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let sides = |[a, b, c]: [usize; 3]| [(a, b), (b, c), (c, a)];
    let key = |(a, b): (usize, usize)| (a.min(b), a.max(b));
    let mut by_edge: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (i, t) in triangles.iter().enumerate() {
        for side in sides(*t) {
            by_edge.entry(key(side)).or_default().push(i);
        }
    }
    let mut taken = vec![false; triangles.len()];
    let mut shells = Vec::new();
    for seed in 0..triangles.len() {
        if taken[seed] {
            continue;
        }
        // Per edge of this shell: the sense of its one use (true when it
        // runs from its lower point), or `None` once used twice.
        let mut uses: HashMap<(usize, usize), Option<bool>> = HashMap::new();
        let mut shell = Vec::new();
        let mut queue = VecDeque::from([seed]);
        while let Some(t) = queue.pop_front() {
            if taken[t] {
                continue;
            }
            let fits = sides(triangles[t])
                .iter()
                .all(|&(a, b)| match uses.get(&key((a, b))) {
                    None => true,
                    Some(Some(forward)) => *forward != (a < b),
                    Some(None) => false,
                });
            if !fits {
                continue;
            }
            taken[t] = true;
            shell.push(t);
            for (a, b) in sides(triangles[t]) {
                let edge = key((a, b));
                let sense = (!uses.contains_key(&edge)).then_some(a < b);
                uses.insert(edge, sense);
                let neighbours = by_edge.get(&edge).into_iter().flatten();
                queue.extend(neighbours.filter(|&&n| !taken[n]));
            }
        }
        shell.sort_unstable();
        shells.push(shell);
    }
    shells
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
        let direction = self.data.add(format!(
            "DIRECTION('',{})",
            triple(span.map(|x| x / length))
        ));
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
        let edge_loop = self.data.add(format!("EDGE_LOOP('',{})", refs(&oriented)));
        let bound = self
            .data
            .add(format!("FACE_OUTER_BOUND('',{edge_loop},.T.)"));
        let (point, _) = self.vertex(a);
        let axis = placement_at(self.data, point, normal, perpendicular(normal));
        let plane = self.data.add(format!("PLANE('',{axis})"));
        self.data
            .add(format!("ADVANCED_FACE('',{},{plane},.T.)", refs(&[bound])))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every edge of every shell is used at most twice, then in opposite
    /// senses.
    fn assert_edge_uses(triangles: &[[usize; 3]], shells: &[Vec<usize>]) {
        for shell in shells {
            let mut uses: HashMap<(usize, usize), Vec<bool>> = HashMap::new();
            for &t in shell {
                let [a, b, c] = triangles[t];
                for (p, q) in [(a, b), (b, c), (c, a)] {
                    uses.entry((p.min(q), p.max(q))).or_default().push(p < q);
                }
            }
            for (edge, senses) in uses {
                assert!(senses.len() <= 2, "edge {edge:?} used {senses:?}");
                if let [s, t] = senses[..] {
                    assert_ne!(s, t, "edge {edge:?} used twice in one sense");
                }
            }
        }
    }

    #[test]
    fn a_closed_mesh_is_one_shell() {
        let quad = [[0, 1, 2], [0, 2, 3]];
        assert_eq!(open_shells(&quad), [vec![0, 1]]);
    }

    #[test]
    fn disjoint_parts_are_separate_shells() {
        let two = [[0, 1, 2], [3, 4, 5], [0, 2, 6]];
        assert_eq!(open_shells(&two), [vec![0, 2], vec![1]]);
    }

    #[test]
    fn a_third_face_on_an_edge_goes_to_another_shell() {
        let fin = [[0, 1, 2], [1, 0, 3], [1, 0, 4]];
        let shells = open_shells(&fin);
        assert_eq!(shells, [vec![0, 1], vec![2]]);
        assert_edge_uses(&fin, &shells);
    }

    #[test]
    fn a_flipped_neighbour_goes_to_another_shell() {
        let flipped = [[0, 1, 2], [0, 1, 3]];
        let shells = open_shells(&flipped);
        assert_eq!(shells, [vec![0], vec![1]]);
        assert_edge_uses(&flipped, &shells);
    }
}
