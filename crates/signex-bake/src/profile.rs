//! Closed-profile walker — given a starting Line entity, trace a
//! connected loop of edge entities through shared endpoint Points and
//! emit the boundary as a Polygon (Vec of `[x_mm, y_mm]` vertices).
//!
//! Used by the v0.14 silk / courtyard / mask / pour / keepout /
//! cutout / 3D-extrude bake modules to convert sketch profiles into
//! baked library polygons.
//!
//! v0.14.1 scope:
//! - Lines and Arcs both participate. Arc segments are tessellated
//!   into [`ARC_SAMPLES`] interior vertices using
//!   `(center, start, end, sweep_ccw)` from the solved state.
//! - Circles are still rejected up front; the bake module is
//!   expected to handle a Circle entity directly (a Circle is an
//!   already-closed primitive without start / end endpoints, so the
//!   walker has nothing to walk).
//! - Construction entities are skipped silently — they're solver
//!   scaffolding and never participate in the baked geometry.
//! - Branching topology (a vertex with 3+ incident edges) returns
//!   [`TraceError::Branching`]; the bake skips with a warning.
//!
//! Cleanroom: traversal is a textbook depth-first walk over the
//! endpoint-incidence graph; arc tessellation is a textbook polar
//! sample (Hearn & Baker §3.13 "Drawing Circular Arcs"). No third-
//! party CAD-tooling source consulted.

use std::collections::{HashMap, HashSet};

use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::state::point_xy;
use signex_sketch::solver::FullSolveOutput;

/// Trace failure modes — the bake site decides whether to warn or
/// error per-attr.
#[derive(Clone, Debug, PartialEq)]
pub enum TraceError {
    /// `start` is not a Line / Arc / Circle in the sketch (or doesn't
    /// exist at all).
    NotAnEdge,
    /// One of the trace's endpoints couldn't be resolved to a position
    /// — usually because the endpoint Point isn't in the sketch.
    MissingEndpoint(SketchEntityId),
    /// Trace ran off the open end of a chain (the next endpoint has
    /// no continuing edge).
    OpenChain,
    /// A vertex has 3+ non-construction incident edges — ambiguous
    /// continuation.
    Branching,
    /// Profile contains a Circle — Circle is a closed primitive on
    /// its own; the bake module should special-case it without going
    /// through the walker.
    CircleInProfile,
    /// Walker exceeded a sanity cap on iterations (broken topology).
    Runaway,
}

/// Number of interior sample vertices generated per Arc segment when
/// the walker tessellates an arc into the polygon. 16 strikes a
/// balance between fidelity and Vec size; PCB silk-grade arcs render
/// smoothly at this density up to 25 mm radius.
pub const ARC_SAMPLES: usize = 16;

/// Result of a trace: either a closed polygon (vertices in mm,
/// CCW or CW depending on starting direction) or a [`TraceError`].
pub type TraceResult = Result<Vec<[f64; 2]>, TraceError>;

/// Trace a closed boundary starting at `start`.
///
/// Algorithm:
/// 1. Build endpoint → list-of-edge adjacency over non-construction
///    Lines (Arcs / Circles are rejected with the matching error).
/// 2. Pick an arbitrary endpoint of `start` as the loop anchor;
///    push its position.
/// 3. Walk: from the current endpoint, find the unique non-visited
///    incident edge (excluding the entity we just came from). If
///    there's exactly one, advance; otherwise return Branching /
///    OpenChain.
/// 4. Loop closes when the next endpoint equals the anchor.
pub fn trace_closed_profile(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    start: SketchEntityId,
) -> TraceResult {
    let start_entity = sketch
        .entities
        .iter()
        .find(|e| e.id == start)
        .ok_or(TraceError::NotAnEdge)?;
    match start_entity.kind {
        EntityKind::Line { .. } | EntityKind::Arc { .. } => {}
        EntityKind::Circle { .. } => return Err(TraceError::CircleInProfile),
        EntityKind::Point { .. } => return Err(TraceError::NotAnEdge),
    }

    let edges = collect_edges(sketch);
    let adj = build_adjacency(&edges);

    let (start_a, start_b) = edge_endpoints(start_entity).ok_or(TraceError::NotAnEdge)?;

    let pos_a = point_xy(start_a, &solve.result.state, &solve.result.index, sketch)
        .ok_or(TraceError::MissingEndpoint(start_a))?;
    let pos_b = point_xy(start_b, &solve.result.state, &solve.result.index, sketch)
        .ok_or(TraceError::MissingEndpoint(start_b))?;

    let mut vertices: Vec<[f64; 2]> = Vec::new();
    vertices.push([pos_a.0, pos_a.1]);
    // If the seed entity is an Arc, sample its interior between
    // start_a and start_b before pushing start_b.
    push_arc_interior_if_arc(
        &mut vertices,
        sketch,
        solve,
        start_entity,
        start_a,
        start_b,
    )?;
    vertices.push([pos_b.0, pos_b.1]);

    let mut visited: HashSet<SketchEntityId> = HashSet::new();
    visited.insert(start);
    let mut current_endpoint = start_b;
    let max_steps = edges.len() + 2;

    for _ in 0..max_steps {
        let candidates: Vec<SketchEntityId> = adj
            .get(&current_endpoint)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|eid| !visited.contains(eid))
            .collect();

        match candidates.len() {
            0 => return Err(TraceError::OpenChain),
            1 => {}
            _ => return Err(TraceError::Branching),
        }

        let next_id = candidates[0];
        let next_entity = edges[&next_id];
        let (next_a, next_b) = edge_endpoints(next_entity).ok_or(TraceError::NotAnEdge)?;
        let other = if next_a == current_endpoint {
            next_b
        } else {
            next_a
        };

        // Sample arc interior between current_endpoint and `other` if
        // this edge is an arc — applies whether we're closing the loop
        // or continuing.
        push_arc_interior_if_arc(
            &mut vertices,
            sketch,
            solve,
            next_entity,
            current_endpoint,
            other,
        )?;

        if other == start_a {
            // Closed loop.
            return Ok(vertices);
        }

        let pos = point_xy(other, &solve.result.state, &solve.result.index, sketch)
            .ok_or(TraceError::MissingEndpoint(other))?;
        vertices.push([pos.0, pos.1]);
        visited.insert(next_id);
        current_endpoint = other;
    }

    Err(TraceError::Runaway)
}

/// If `entity` is an [`EntityKind::Arc`], append [`ARC_SAMPLES`]
/// interior vertices interpolated along the arc from `from` to `to`.
/// Lines are no-ops.
///
/// Sampling: compute the arc's center, radius, start_angle, end_angle.
/// Walk N intermediate angles in the direction the arc sweeps from
/// `from` to `to` (which depends on `sweep_ccw` AND on whether `from`
/// is the arc's `start` or `end` Point — if the trace approaches the
/// arc from the `end` side we walk the arc backwards).
fn push_arc_interior_if_arc(
    out: &mut Vec<[f64; 2]>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    entity: &Entity,
    from: SketchEntityId,
    to: SketchEntityId,
) -> Result<(), TraceError> {
    let (center, arc_start, arc_end, sweep_ccw) = match entity.kind {
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => (center, start, end, sweep_ccw),
        _ => return Ok(()),
    };

    let c = point_xy(center, &solve.result.state, &solve.result.index, sketch)
        .ok_or(TraceError::MissingEndpoint(center))?;
    let f = point_xy(from, &solve.result.state, &solve.result.index, sketch)
        .ok_or(TraceError::MissingEndpoint(from))?;
    let t = point_xy(to, &solve.result.state, &solve.result.index, sketch)
        .ok_or(TraceError::MissingEndpoint(to))?;

    let radius = ((f.0 - c.0).powi(2) + (f.1 - c.1).powi(2)).sqrt();
    let from_angle = (f.1 - c.1).atan2(f.0 - c.0);
    let to_angle = (t.1 - c.1).atan2(t.0 - c.0);

    // If we're walking from the arc's `end` Point to its `start` Point,
    // invert the recorded sweep direction.
    let walking_forward = from == arc_start && to == arc_end;
    let walking_reverse = from == arc_end && to == arc_start;
    if !walking_forward && !walking_reverse {
        // Topology bug — `from`/`to` don't match the arc's endpoints.
        return Err(TraceError::MissingEndpoint(from));
    }
    let effective_ccw = if walking_forward { sweep_ccw } else { !sweep_ccw };

    // Compute the signed sweep magnitude in [0, 2π) given the direction.
    let raw_delta = to_angle - from_angle;
    let sweep_magnitude = if effective_ccw {
        // CCW: angle should increase from `from_angle` to `from_angle + Δ`
        // where Δ ∈ (0, 2π].
        let mut d = raw_delta;
        while d <= 0.0 {
            d += std::f64::consts::TAU;
        }
        d
    } else {
        // CW: angle should decrease.
        let mut d = -raw_delta;
        while d <= 0.0 {
            d += std::f64::consts::TAU;
        }
        -d
    };

    // Sample (ARC_SAMPLES) interior points (excluding the endpoints
    // themselves — those are already pushed by the caller).
    for i in 1..ARC_SAMPLES {
        let frac = i as f64 / ARC_SAMPLES as f64;
        let theta = from_angle + sweep_magnitude * frac;
        let x = c.0 + radius * theta.cos();
        let y = c.1 + radius * theta.sin();
        out.push([x, y]);
    }
    Ok(())
}

/// Collect non-construction edge entities (Lines + Arcs). Circles are
/// excluded — they're already-closed primitives that the bake module
/// handles separately.
fn collect_edges(sketch: &SketchData) -> HashMap<SketchEntityId, &Entity> {
    let mut out = HashMap::new();
    for e in &sketch.entities {
        if e.construction {
            continue;
        }
        if matches!(e.kind, EntityKind::Line { .. } | EntityKind::Arc { .. }) {
            out.insert(e.id, e);
        }
    }
    out
}

fn build_adjacency(
    edges: &HashMap<SketchEntityId, &Entity>,
) -> HashMap<SketchEntityId, Vec<SketchEntityId>> {
    let mut adj: HashMap<SketchEntityId, Vec<SketchEntityId>> = HashMap::new();
    for (eid, ent) in edges {
        if let Some((a, b)) = edge_endpoints(ent) {
            adj.entry(a).or_default().push(*eid);
            adj.entry(b).or_default().push(*eid);
        }
    }
    adj
}

/// Endpoint Points of an edge — Line `(start, end)` or Arc
/// `(start, end)` (the Arc's `center` Point is NOT a topology vertex).
fn edge_endpoints(entity: &Entity) -> Option<(SketchEntityId, SketchEntityId)> {
    match entity.kind {
        EntityKind::Line { start, end } => Some((start, end)),
        EntityKind::Arc { start, end, .. } => Some((start, end)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_sketch::solver::Solver;

    /// Build a sketch with one rectangle (4 Points + 4 Lines), solve,
    /// trace from the first Line, expect a 4-vertex polygon.
    fn rectangle_sketch() -> (SketchData, SketchEntityId) {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });

        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        let p4 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 1.0, y: 1.0 }));
        data.entities
            .push(Entity::new(p4, plane, EntityKind::Point { x: 0.0, y: 1.0 }));

        let l1 = SketchEntityId::new();
        let l2 = SketchEntityId::new();
        let l3 = SketchEntityId::new();
        let l4 = SketchEntityId::new();
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
        data.entities.push(Entity::new(
            l2,
            plane,
            EntityKind::Line { start: p2, end: p3 },
        ));
        data.entities.push(Entity::new(
            l3,
            plane,
            EntityKind::Line { start: p3, end: p4 },
        ));
        data.entities.push(Entity::new(
            l4,
            plane,
            EntityKind::Line { start: p4, end: p1 },
        ));

        (data, l1)
    }

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        Solver::default().solve(sketch, &ResolvedParams::new()).unwrap()
    }

    #[test]
    fn trace_rectangle_closes() {
        let (sketch, l1) = rectangle_sketch();
        let solved = solve(&sketch);
        let polygon = trace_closed_profile(&sketch, &solved, l1).expect("rectangle should close");
        assert_eq!(polygon.len(), 4);
    }

    #[test]
    fn trace_open_chain_returns_open_error() {
        // Three points, two lines, no closure.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
        let l1 = SketchEntityId::new();
        let l2 = SketchEntityId::new();
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
        data.entities.push(Entity::new(
            l2,
            plane,
            EntityKind::Line { start: p2, end: p3 },
        ));

        let solved = solve(&data);
        assert_eq!(
            trace_closed_profile(&data, &solved, l1),
            Err(TraceError::OpenChain)
        );
    }

    #[test]
    fn trace_d_shape_line_plus_arc_closes() {
        // A "D" shape: Line p1→p2 (chord) + semicircular Arc back from
        // p2→p1 around centre pc. Walker should close the loop with
        // 1 line endpoint + tessellated arc interior + 1 line endpoint.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let pc = SketchEntityId::new();
        // p1 = (0, 0), p2 = (2, 0), pc = (1, 0). Arc has radius 1
        // sweeping CCW from p2 to p1 through the upper half-plane.
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        let l1 = SketchEntityId::new();
        let arc = SketchEntityId::new();
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
        data.entities.push(Entity::new(
            arc,
            plane,
            EntityKind::Arc {
                center: pc,
                start: p2,
                end: p1,
                sweep_ccw: true,
            },
        ));
        let solved = solve(&data);
        let polygon = trace_closed_profile(&data, &solved, l1).expect("D-shape should close");
        // Endpoints (2) + arc interior (ARC_SAMPLES - 1) — the start
        // endpoint is pushed once and the closing endpoint matches it.
        assert_eq!(polygon.len(), 2 + (ARC_SAMPLES - 1));
        // First two vertices are the line endpoints.
        assert_eq!(polygon[0], [0.0, 0.0]);
        assert_eq!(polygon[1], [2.0, 0.0]);
        // The arc samples should all sit on the upper half-circle of
        // radius 1 centred at (1, 0).
        for sample in &polygon[2..] {
            let dx = sample[0] - 1.0;
            let dy = sample[1];
            let r = (dx * dx + dy * dy).sqrt();
            assert!((r - 1.0).abs() < 1e-9, "arc vertex {sample:?} not on r=1 circle");
            assert!(dy >= -1e-9, "arc vertex {sample:?} should be in upper half-plane");
        }
    }

    #[test]
    fn trace_d_shape_cw_arc_closes_lower_half() {
        // Same D-shape but with the arc swept CW — the tessellated
        // interior should now sit in the lower half-plane.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let pc = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        let l1 = SketchEntityId::new();
        let arc = SketchEntityId::new();
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
        data.entities.push(Entity::new(
            arc,
            plane,
            EntityKind::Arc {
                center: pc,
                start: p2,
                end: p1,
                sweep_ccw: false,
            },
        ));
        let solved = solve(&data);
        let polygon = trace_closed_profile(&data, &solved, l1).expect("CW D-shape should close");
        for sample in &polygon[2..] {
            let dy = sample[1];
            assert!(dy <= 1e-9, "CW arc vertex {sample:?} should be in lower half-plane");
        }
    }

    #[test]
    fn trace_arc_seed_walks_back_through_line() {
        // Same D-shape, but seed the walker with the Arc instead of
        // the Line — should still close.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let pc = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        let l1 = SketchEntityId::new();
        let arc = SketchEntityId::new();
        data.entities.push(Entity::new(
            arc,
            plane,
            EntityKind::Arc {
                center: pc,
                start: p2,
                end: p1,
                sweep_ccw: true,
            },
        ));
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
        let solved = solve(&data);
        let polygon = trace_closed_profile(&data, &solved, arc)
            .expect("D-shape seeded from arc should close");
        assert!(polygon.len() > 4, "arc seed should yield tessellated polygon");
    }

    #[test]
    fn trace_branching_topology_errors() {
        // 3 lines all sharing the centre point pc — walker walks
        // outward from p1 toward pc, finds 2 candidates (l2, l3),
        // returns Branching.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let pc = SketchEntityId::new();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: -1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 0.0, y: 1.0 }));
        let l1 = SketchEntityId::new();
        let l2 = SketchEntityId::new();
        let l3 = SketchEntityId::new();
        // l1 oriented so the walker exits via pc (start_b = pc).
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: pc },
        ));
        data.entities.push(Entity::new(
            l2,
            plane,
            EntityKind::Line { start: pc, end: p2 },
        ));
        data.entities.push(Entity::new(
            l3,
            plane,
            EntityKind::Line { start: pc, end: p3 },
        ));
        let solved = solve(&data);
        assert_eq!(
            trace_closed_profile(&data, &solved, l1),
            Err(TraceError::Branching)
        );
    }

    #[test]
    fn trace_construction_lines_skipped() {
        // Rectangle with one extra construction line — walker ignores
        // the construction line and still closes the rectangle.
        let (mut sketch, l1) = rectangle_sketch();
        // Find the first Point entity.
        let p1_id = sketch
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Point { .. }))
            .unwrap()
            .id;
        // Add a Point + construction Line that touches p1 — this would
        // create branching if not skipped.
        let pc = SketchEntityId::new();
        let plane = sketch.planes[0].id;
        sketch
            .entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: -1.0, y: -1.0 }));
        let mut construction_line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line {
                start: p1_id,
                end: pc,
            },
        );
        construction_line.construction = true;
        sketch.entities.push(construction_line);
        let solved = solve(&sketch);
        let polygon =
            trace_closed_profile(&sketch, &solved, l1).expect("rectangle still closes");
        assert_eq!(polygon.len(), 4);
    }
}
