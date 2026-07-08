//! Half-edge mesh data structure for 2D planar polygons.
//!
//! Each undirected polygon edge becomes two oriented half-edges
//! (twin pair). Each half-edge belongs to exactly one face and
//! points to its successor around that face's boundary. Vertices
//! cache one outgoing half-edge so vertex-incidence walks don't
//! need a search.
//!
//! For a single closed polygon with N vertices the mesh has
//! `2N` half-edges and `2` faces — the inside and the unbounded
//! outer face. More complex inputs (multiple connected rings,
//! holes, planar subdivisions) extend the same structure with
//! additional faces.
//!
//! Use cases for this crate:
//! - Closed-loop walker for `signex-bake::profile` (uniform face
//!   traversal regardless of the ad-hoc adjacency map).
//! - Polygon boolean cleanup (Phase 2 follow-up).
//! - Multi-region pad-stack composition.

use super::Point2;
use super::predicates::signed_area;

/// Index types so signatures read clearly. `usize` underneath but
/// distinct so calls like `mesh.next(half_edge)` don't accidentally
/// take a vertex id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VertexId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HalfEdgeId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceId(pub usize);

#[derive(Debug, Clone)]
pub struct Vertex {
    pub pos: Point2,
    /// One half-edge with this vertex as its origin. Acts as a
    /// fast-path entry into vertex-incident walks.
    pub outgoing: HalfEdgeId,
}

#[derive(Debug, Clone)]
pub struct HalfEdge {
    pub origin: VertexId,
    pub twin: HalfEdgeId,
    pub next: HalfEdgeId,
    pub prev: HalfEdgeId,
    pub face: FaceId,
}

#[derive(Debug, Clone)]
pub struct Face {
    /// Any half-edge along this face's boundary.
    pub boundary: HalfEdgeId,
    /// `true` for the unbounded outer face — the half-edges along
    /// its boundary wind opposite to the polygon's orientation.
    pub outer: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub halfedges: Vec<HalfEdge>,
    pub faces: Vec<Face>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a mesh from a single closed polygon. The input must
    /// have ≥ 3 vertices and no last-vertex-equals-first repeat.
    /// Returns `None` for degenerate input. Output mesh has two
    /// faces — face 0 is the inside (CCW boundary), face 1 is the
    /// unbounded outer (CW boundary).
    pub fn from_polygon(polygon: &[Point2]) -> Option<Self> {
        if polygon.len() < 3 {
            return None;
        }
        // Normalise winding to CCW so face 0 is always the
        // interior.
        let area = signed_area(polygon);
        if area.abs() <= 1e-12 {
            return None;
        }
        let positions: Vec<Point2> = if area > 0.0 {
            polygon.to_vec()
        } else {
            polygon.iter().rev().copied().collect()
        };
        let n = positions.len();

        let mut mesh = Mesh::new();
        mesh.faces.push(Face {
            boundary: HalfEdgeId(0),
            outer: false,
        });
        mesh.faces.push(Face {
            boundary: HalfEdgeId(1),
            outer: true,
        });

        // Allocate vertices. `outgoing` points to the inner
        // half-edge originating at this vertex.
        for (i, p) in positions.iter().enumerate() {
            mesh.vertices.push(Vertex {
                pos: *p,
                outgoing: HalfEdgeId(i * 2),
            });
        }

        // For each polygon edge i → i+1 emit two half-edges:
        //   even index 2i+0 — inner face (face 0), origin = vertex i
        //   odd  index 2i+1 — outer face (face 1), origin = vertex i+1
        for i in 0..n {
            let inner = HalfEdgeId(i * 2);
            let outer = HalfEdgeId(i * 2 + 1);
            let next_inner = HalfEdgeId(((i + 1) % n) * 2);
            let prev_inner = HalfEdgeId(((i + n - 1) % n) * 2);
            let next_outer = HalfEdgeId(((i + n - 1) % n) * 2 + 1);
            let prev_outer = HalfEdgeId(((i + 1) % n) * 2 + 1);
            mesh.halfedges.push(HalfEdge {
                origin: VertexId(i),
                twin: outer,
                next: next_inner,
                prev: prev_inner,
                face: FaceId(0),
            });
            mesh.halfedges.push(HalfEdge {
                origin: VertexId((i + 1) % n),
                twin: inner,
                next: next_outer,
                prev: prev_outer,
                face: FaceId(1),
            });
        }
        Some(mesh)
    }

    pub fn vertex(&self, v: VertexId) -> &Vertex {
        &self.vertices[v.0]
    }

    pub fn half_edge(&self, h: HalfEdgeId) -> &HalfEdge {
        &self.halfedges[h.0]
    }

    pub fn face(&self, f: FaceId) -> &Face {
        &self.faces[f.0]
    }

    /// Iterate the half-edges along a face's boundary.
    pub fn face_halfedges(&self, f: FaceId) -> Vec<HalfEdgeId> {
        let start = self.faces[f.0].boundary;
        let mut out = Vec::with_capacity(self.halfedges.len());
        let mut cur = start;
        loop {
            out.push(cur);
            cur = self.halfedges[cur.0].next;
            if cur == start {
                break;
            }
            if out.len() > self.halfedges.len() {
                // Defensive — broken `next` chain.
                break;
            }
        }
        out
    }

    /// Vertices around a face in boundary order.
    pub fn face_vertices(&self, f: FaceId) -> Vec<VertexId> {
        self.face_halfedges(f)
            .into_iter()
            .map(|h| self.halfedges[h.0].origin)
            .collect()
    }

    /// Half-edges originating at a vertex. Walks via `twin.next`
    /// which advances around the vertex's incident half-edges.
    pub fn vertex_outgoing(&self, v: VertexId) -> Vec<HalfEdgeId> {
        let start = self.vertices[v.0].outgoing;
        let mut out = vec![start];
        let mut cur = start;
        loop {
            // Twin's next is the next outgoing half-edge from the
            // same vertex (rotating around the vertex CCW for an
            // interior vertex).
            let twin = self.halfedges[cur.0].twin;
            let next_out = self.halfedges[twin.0].next;
            if next_out == start {
                break;
            }
            out.push(next_out);
            cur = next_out;
            if out.len() > self.halfedges.len() {
                break;
            }
        }
        out
    }

    /// Convenience: positions of the boundary vertices of `f` in
    /// boundary order. The classic "extract the polygon outline
    /// from face X" primitive.
    pub fn face_polygon(&self, f: FaceId) -> Vec<Point2> {
        self.face_vertices(f)
            .into_iter()
            .map(|v| self.vertices[v.0].pos)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn empty_polygon_returns_none() {
        assert!(Mesh::from_polygon(&[]).is_none());
        assert!(Mesh::from_polygon(&[p(0.0, 0.0)]).is_none());
        assert!(Mesh::from_polygon(&[p(0.0, 0.0), p(1.0, 0.0)]).is_none());
    }

    #[test]
    fn degenerate_zero_area_returns_none() {
        assert!(Mesh::from_polygon(&[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]).is_none());
    }

    #[test]
    fn unit_square_has_two_faces_eight_halfedges() {
        let sq = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let mesh = Mesh::from_polygon(&sq).unwrap();
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.halfedges.len(), 8);
        assert_eq!(mesh.faces.len(), 2);
        assert!(!mesh.faces[0].outer);
        assert!(mesh.faces[1].outer);
    }

    #[test]
    fn face_polygon_round_trip() {
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let mesh = Mesh::from_polygon(&sq).unwrap();
        let inner = mesh.face_polygon(FaceId(0));
        // Same length as the input.
        assert_eq!(inner.len(), 4);
        // Same area (CCW = positive).
        assert!((signed_area(&inner) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cw_input_normalises_to_ccw_inner() {
        let cw = [p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0)];
        let mesh = Mesh::from_polygon(&cw).unwrap();
        let inner = mesh.face_polygon(FaceId(0));
        assert!(signed_area(&inner) > 0.0, "inner face must be CCW");
    }

    #[test]
    fn face_halfedge_ring_is_closed() {
        let sq = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let mesh = Mesh::from_polygon(&sq).unwrap();
        let ring = mesh.face_halfedges(FaceId(0));
        assert_eq!(ring.len(), 4);
        // Verify next/prev consistency.
        for h in &ring {
            let he = mesh.half_edge(*h);
            let nxt = mesh.half_edge(he.next);
            assert_eq!(nxt.prev, *h, "next/prev pointers must agree");
            // Twin's twin must be self.
            assert_eq!(mesh.half_edge(he.twin).twin, *h);
            // Twins are on opposite faces.
            assert_ne!(mesh.half_edge(he.twin).face, he.face);
        }
    }

    #[test]
    fn vertex_outgoing_yields_inner_and_outer() {
        // Each vertex of a simple closed polygon has exactly two
        // outgoing half-edges — one bounding the inner face, one
        // bounding the outer face.
        let sq = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let mesh = Mesh::from_polygon(&sq).unwrap();
        for i in 0..mesh.vertices.len() {
            let edges = mesh.vertex_outgoing(VertexId(i));
            assert_eq!(edges.len(), 2, "vertex {i} should have 2 outgoing edges");
            // The two should belong to different faces.
            let f0 = mesh.half_edge(edges[0]).face;
            let f1 = mesh.half_edge(edges[1]).face;
            assert_ne!(f0, f1);
        }
    }
}
