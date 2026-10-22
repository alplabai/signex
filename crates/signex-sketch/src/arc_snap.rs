//! One-time repair of arcs whose end point is off the circle.
//!
//! The solver ties an arc's end to the circle through its start about its
//! centre (`solver::residuals::arc_closure`). A sketch saved before that
//! row existed can hold an end point anywhere. Left alone, the first solve
//! would pull centre and start toward the stray end. This snaps the end
//! onto the circle instead, keeping its angle, so centre and start — the
//! geometry the user drew — never move.

use std::collections::HashMap;

use crate::entity::EntityKind;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;

/// Relative tolerance below which an end point counts as on the circle.
const ON_CIRCLE_REL_TOL: f64 = 1e-9;

/// Snap every off-circle arc end point onto its circle. Returns the ids of
/// the arcs that were adjusted, in entity order. Centre and start points
/// are never touched.
///
/// Linear in the number of entities: points are indexed once, so the sketch
/// may come from untrusted input without a quadratic scan per arc.
pub fn snap_arc_ends_onto_circles(sketch: &mut SketchData) -> Vec<SketchEntityId> {
    let point_index: HashMap<SketchEntityId, usize> = sketch
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e.kind, EntityKind::Point { .. }))
        .map(|(i, e)| (e.id, i))
        .collect();
    let arcs: Vec<(SketchEntityId, [SketchEntityId; 3])> = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Arc {
                center, start, end, ..
            } => Some((e.id, [center, start, end])),
            _ => None,
        })
        .collect();
    let mut adjusted = Vec::new();
    for (arc, [center, start, end]) in arcs {
        let (Some(&ci), Some(&si), Some(&ei)) = (
            point_index.get(&center),
            point_index.get(&start),
            point_index.get(&end),
        ) else {
            continue;
        };
        let (Some(c), Some(s), Some(e)) = (xy(sketch, ci), xy(sketch, si), xy(sketch, ei)) else {
            continue;
        };
        let radius = (s.0 - c.0).hypot(s.1 - c.1);
        let reach = (e.0 - c.0).hypot(e.1 - c.1);
        if (reach - radius).abs() <= ON_CIRCLE_REL_TOL * radius.max(1.0) {
            continue;
        }
        // An end sitting exactly on the centre has no angle; use +x.
        let (ux, uy) = if reach > 0.0 {
            ((e.0 - c.0) / reach, (e.1 - c.1) / reach)
        } else {
            (1.0, 0.0)
        };
        set_xy(sketch, ei, (c.0 + ux * radius, c.1 + uy * radius));
        adjusted.push(arc);
    }
    adjusted
}

fn xy(sketch: &SketchData, index: usize) -> Option<(f64, f64)> {
    match sketch.entities[index].kind {
        EntityKind::Point { x, y } => Some((x, y)),
        _ => None,
    }
}

fn set_xy(sketch: &mut SketchData, index: usize, (nx, ny): (f64, f64)) {
    if let EntityKind::Point { x, y } = &mut sketch.entities[index].kind {
        *x = nx;
        *y = ny;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;
    use crate::plane::{Plane, PlaneId, PlaneKind};
    use crate::solver::Solver;
    use crate::solver::residual::ResolvedParams;
    use crate::solver::state::point_xy;

    fn arc_sketch(
        c: (f64, f64),
        s: (f64, f64),
        e: (f64, f64),
    ) -> (SketchData, SketchEntityId, [SketchEntityId; 3]) {
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let mut data = SketchData::default();
        let pid = plane.id;
        data.planes.push(plane);
        let mut point = |(x, y)| {
            let id = SketchEntityId::new();
            data.entities
                .push(Entity::new(id, pid, EntityKind::Point { x, y }));
            id
        };
        let (cid, sid, eid) = (point(c), point(s), point(e));
        let arc = SketchEntityId::new();
        data.entities.push(Entity::new(
            arc,
            pid,
            EntityKind::Arc {
                center: cid,
                start: sid,
                end: eid,
                sweep_ccw: true,
            },
        ));
        (data, arc, [cid, sid, eid])
    }

    fn at(sketch: &SketchData, id: SketchEntityId) -> (f64, f64) {
        sketch
            .entities
            .iter()
            .find_map(|e| match e.kind {
                EntityKind::Point { x, y } if e.id == id => Some((x, y)),
                _ => None,
            })
            .expect("point")
    }

    #[test]
    fn off_circle_end_lands_on_the_circle_keeping_centre_start_and_angle() {
        let (mut data, arc, [c, s, e]) = arc_sketch((0.0, 0.0), (5.0, 0.0), (0.0, 7.0));

        let adjusted = snap_arc_ends_onto_circles(&mut data);

        assert_eq!(adjusted, vec![arc]);
        assert_eq!(at(&data, c), (0.0, 0.0));
        assert_eq!(at(&data, s), (5.0, 0.0));
        let (ex, ey) = at(&data, e);
        assert!(ex.abs() < 1e-12 && (ey - 5.0).abs() < 1e-12, "({ex}, {ey})");
    }

    #[test]
    fn an_on_circle_arc_is_left_alone() {
        let (mut data, _arc, [_, _, e]) = arc_sketch((1.0, 1.0), (6.0, 1.0), (1.0, 6.0));

        assert!(snap_arc_ends_onto_circles(&mut data).is_empty());
        assert_eq!(at(&data, e), (1.0, 6.0));
    }

    #[test]
    fn a_later_solve_leaves_centre_and_start_unmoved() {
        let (mut data, _arc, [c, s, _]) = arc_sketch((2.0, 3.0), (7.0, 3.0), (2.0, 10.0));
        snap_arc_ends_onto_circles(&mut data);

        let out = Solver::default()
            .solve(&data, &ResolvedParams::new())
            .expect("a snapped arc alone is satisfiable");

        let pos = |p| point_xy(p, &out.result.state, &out.result.index, &data).unwrap();
        assert_eq!(pos(c), (2.0, 3.0));
        assert_eq!(pos(s), (7.0, 3.0));
    }

    #[test]
    fn a_large_sketch_is_repaired_in_linear_time() {
        const ARCS: usize = 10_000;
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let pid = plane.id;
        let mut data = SketchData::default();
        data.planes.push(plane);
        let point = |data: &mut SketchData, x: f64, y: f64| {
            let id = SketchEntityId::new();
            data.entities
                .push(Entity::new(id, pid, EntityKind::Point { x, y }));
            id
        };
        let mut ends = Vec::with_capacity(ARCS);
        let mut arcs = Vec::with_capacity(ARCS);
        for i in 0..ARCS {
            let x = i as f64;
            let center = point(&mut data, x, 0.0);
            let start = point(&mut data, x + 5.0, 0.0);
            let end = point(&mut data, x, 9.0);
            ends.push((center, end));
            arcs.push(Entity::new(
                SketchEntityId::new(),
                pid,
                EntityKind::Arc {
                    center,
                    start,
                    end,
                    sweep_ccw: true,
                },
            ));
        }
        data.entities.extend(arcs);

        let started = std::time::Instant::now();
        let adjusted = snap_arc_ends_onto_circles(&mut data);
        let took = started.elapsed();

        assert_eq!(adjusted.len(), ARCS);
        assert!(
            took < std::time::Duration::from_secs(1),
            "30k points / 10k arcs took {took:?}"
        );
        let (c, e) = ends[ARCS - 1];
        let (cx, cy) = at(&data, c);
        let (ex, ey) = at(&data, e);
        assert!(((ex - cx).hypot(ey - cy) - 5.0).abs() < 1e-9);
    }
}
