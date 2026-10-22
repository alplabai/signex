//! One-time repair of arcs whose end point is off the circle.
//!
//! The solver ties an arc's end to the circle through its start about its
//! centre (`solver::residuals::arc_closure`). A sketch saved before that
//! row existed can hold an end point anywhere. Left alone, the first solve
//! would pull centre and start toward the stray end. This snaps the end
//! onto the circle instead, keeping its angle, so centre and start — the
//! geometry the user drew — never move.

use crate::entity::EntityKind;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;

/// Relative tolerance below which an end point counts as on the circle.
const ON_CIRCLE_REL_TOL: f64 = 1e-9;

/// Snap every off-circle arc end point onto its circle. Returns the ids of
/// the arcs that were adjusted, in entity order. Centre and start points
/// are never touched.
pub fn snap_arc_ends_onto_circles(sketch: &mut SketchData) -> Vec<SketchEntityId> {
    let mut adjusted = Vec::new();
    let arcs: Vec<(
        SketchEntityId,
        SketchEntityId,
        SketchEntityId,
        SketchEntityId,
    )> = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Arc {
                center, start, end, ..
            } => Some((e.id, center, start, end)),
            _ => None,
        })
        .collect();
    for (arc, center, start, end) in arcs {
        let (Some(c), Some(s), Some(e)) = (
            point_at(sketch, center),
            point_at(sketch, start),
            point_at(sketch, end),
        ) else {
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
        set_point(sketch, end, (c.0 + ux * radius, c.1 + uy * radius));
        adjusted.push(arc);
    }
    adjusted
}

fn point_at(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    sketch.entities.iter().find_map(|e| match e.kind {
        EntityKind::Point { x, y } if e.id == id => Some((x, y)),
        _ => None,
    })
}

fn set_point(sketch: &mut SketchData, id: SketchEntityId, (nx, ny): (f64, f64)) {
    if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id)
        && let EntityKind::Point { x, y } = &mut e.kind
    {
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

    fn xy(sketch: &SketchData, id: SketchEntityId) -> (f64, f64) {
        point_at(sketch, id).expect("point")
    }

    #[test]
    fn off_circle_end_lands_on_the_circle_keeping_centre_start_and_angle() {
        let (mut data, arc, [c, s, e]) = arc_sketch((0.0, 0.0), (5.0, 0.0), (0.0, 7.0));

        let adjusted = snap_arc_ends_onto_circles(&mut data);

        assert_eq!(adjusted, vec![arc]);
        assert_eq!(xy(&data, c), (0.0, 0.0));
        assert_eq!(xy(&data, s), (5.0, 0.0));
        let (ex, ey) = xy(&data, e);
        assert!(ex.abs() < 1e-12 && (ey - 5.0).abs() < 1e-12, "({ex}, {ey})");
    }

    #[test]
    fn an_on_circle_arc_is_left_alone() {
        let (mut data, _arc, [_, _, e]) = arc_sketch((1.0, 1.0), (6.0, 1.0), (1.0, 6.0));

        assert!(snap_arc_ends_onto_circles(&mut data).is_empty());
        assert_eq!(xy(&data, e), (1.0, 6.0));
    }

    #[test]
    fn a_later_solve_leaves_centre_and_start_unmoved() {
        let (mut data, _arc, [c, s, _]) = arc_sketch((2.0, 3.0), (7.0, 3.0), (2.0, 10.0));
        snap_arc_ends_onto_circles(&mut data);

        let out = Solver::default()
            .solve(&data, &ResolvedParams::new())
            .expect("a snapped arc alone is satisfiable");

        let at = |p| point_xy(p, &out.result.state, &out.result.index, &data).unwrap();
        assert_eq!(at(c), (2.0, 3.0));
        assert_eq!(at(s), (7.0, 3.0));
    }
}
