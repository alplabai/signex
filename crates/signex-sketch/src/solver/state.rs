use std::collections::{HashMap, HashSet};

use crate::constraint::ConstraintKind;
use crate::entity::{Entity, EntityKind};
use crate::id::SketchEntityId;
use crate::sketch::SketchData;

/// Maps a Point's `SketchEntityId` to its `(x, y)` offset in the
/// state vector, plus per-circle radius offsets and the two sets of
/// points whose coords are read directly from the `Entity` instead of
/// the state vector: Fixed-constrained ones, and free ones no residual
/// reads.
#[derive(Debug, Default, Clone)]
pub struct EntityIndex {
    pub points: HashMap<SketchEntityId, (usize, usize)>,
    pub radii: HashMap<SketchEntityId, usize>,
    pub fixed: HashSet<SketchEntityId>,
    /// Free points no residual reads (#1055): no constraint touches them
    /// and they belong to no arc still being closed. The solve cannot move
    /// them, so they stay out of the state and keep their coordinates; a
    /// sketch of minted pads with one constraint solves at the size of
    /// that constraint, not of every pad. Unlike [`Self::fixed`] they are
    /// still free for DOF purposes.
    pub frozen: HashSet<SketchEntityId>,
}

/// An arc no constraint touches is left out of the solve only if its end
/// point is already this close (mm) to its circle. Its closure row then
/// adds at most this much to the residual, far below the solver's
/// convergence tolerance; an arc further off stays in the solve to be
/// closed.
const FROZEN_ARC_CLOSURE_MM: f64 = 1e-9;

#[derive(Debug, Clone)]
pub struct PackedState {
    pub vector: Vec<f64>,
    pub index: EntityIndex,
}

/// Build a state vector from a sketch. Fixed-constrained Points, and free
/// Points no residual reads ([`EntityIndex::frozen`]), are excluded from
/// the state vector; their coordinates are read directly from the
/// [`Entity`] at residual time so the solver cannot move them.
pub fn pack(sketch: &SketchData) -> PackedState {
    let fixed: HashSet<SketchEntityId> = sketch
        .constraints
        .iter()
        .filter_map(|c| match c.kind {
            ConstraintKind::Fixed { point } => Some(point),
            _ => None,
        })
        .collect();

    let solved = solved_points(sketch);

    let mut vector = Vec::new();
    let mut index = EntityIndex {
        fixed: fixed.clone(),
        ..Default::default()
    };

    for entity in &sketch.entities {
        match entity.kind {
            EntityKind::Point { x, y } => {
                if fixed.contains(&entity.id) {
                    continue;
                }
                if !solved.contains(&entity.id) {
                    index.frozen.insert(entity.id);
                    continue;
                }
                let xi = vector.len();
                vector.push(x);
                let yi = vector.len();
                vector.push(y);
                index.points.insert(entity.id, (xi, yi));
            }
            EntityKind::Circle { radius, .. } => {
                let ri = vector.len();
                vector.push(radius);
                index.radii.insert(entity.id, ri);
            }
            _ => {}
        }
    }

    PackedState { vector, index }
}

/// Look up a Point's current `(x, y)` — either from the state vector
/// (free variable) or directly from the [`Entity`] (Fixed-constrained,
/// or frozen because no residual reads it).
pub fn point_xy(
    id: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Option<(f64, f64)> {
    if index.fixed.contains(&id) || index.frozen.contains(&id) {
        let entity = sketch.entities.iter().find(|e| e.id == id)?;
        if let EntityKind::Point { x, y } = entity.kind {
            return Some((x, y));
        }
        return None;
    }
    let (xi, yi) = index.points.get(&id)?;
    Some((state[*xi], state[*yi]))
}

/// Look up the radius of a [`EntityKind::Circle`] from the state
/// vector. Returns `None` for non-circle entities.
pub fn circle_radius(id: SketchEntityId, state: &[f64], index: &EntityIndex) -> Option<f64> {
    let ri = index.radii.get(&id)?;
    Some(state[*ri])
}

/// Resolve the start/end Point IDs for a [`EntityKind::Line`].
pub fn line_endpoints(
    id: SketchEntityId,
    sketch: &SketchData,
) -> Option<(SketchEntityId, SketchEntityId)> {
    let line = sketch.entities.iter().find(|e| e.id == id)?;
    match line.kind {
        EntityKind::Line { start, end } => Some((start, end)),
        _ => None,
    }
}

/// Resolve the centre/start/end Point IDs and CCW flag for an
/// [`EntityKind::Arc`].
pub fn arc_refs(
    id: SketchEntityId,
    sketch: &SketchData,
) -> Option<(SketchEntityId, SketchEntityId, SketchEntityId, bool)> {
    let arc = sketch.entities.iter().find(|e| e.id == id)?;
    match arc.kind {
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => Some((center, start, end, sweep_ccw)),
        _ => None,
    }
}

/// Resolve the centre Point ID for either an [`EntityKind::Arc`] or
/// [`EntityKind::Circle`].
pub fn center_of(id: SketchEntityId, sketch: &SketchData) -> Option<SketchEntityId> {
    let entity = sketch.entities.iter().find(|e| e.id == id)?;
    match entity.kind {
        EntityKind::Arc { center, .. } => Some(center),
        EntityKind::Circle { center, .. } => Some(center),
        _ => None,
    }
}

/// Lookup helper used by tests that need an [`Entity`] by ID.
pub fn find_entity(id: SketchEntityId, sketch: &SketchData) -> Option<&Entity> {
    sketch.entities.iter().find(|e| e.id == id)
}

/// Points some residual reads, so the solve must carry them: every point a
/// constraint touches, plus all three points of any arc whose implicit
/// closure row reaches one of those, or that is not closed yet (#1055).
/// An arc sharing a point with constrained geometry is pulled in whole,
/// repeatedly, until nothing more joins.
fn solved_points(sketch: &SketchData) -> HashSet<SketchEntityId> {
    let mut solved: HashSet<SketchEntityId> = sketch
        .constraints
        .iter()
        .flat_map(|c| points_touched(&c.kind, sketch))
        .collect();
    let arcs: Vec<[SketchEntityId; 3]> = sketch
        .entities
        .iter()
        .filter_map(|e| arc_refs(e.id, sketch).map(|(c, s, e, _)| [c, s, e]))
        .collect();
    loop {
        let mut grew = false;
        for refs in &arcs {
            if refs.iter().any(|p| solved.contains(p)) || !arc_is_closed(*refs, sketch) {
                for p in refs {
                    grew |= solved.insert(*p);
                }
            }
        }
        if !grew {
            return solved;
        }
    }
}

/// Whether an arc's end point already lies on its circle, read from the
/// entities. A missing point counts as open, so the arc stays in the solve
/// and the solver reports it as before.
fn arc_is_closed([center, start, end]: [SketchEntityId; 3], sketch: &SketchData) -> bool {
    let at = |id| match find_entity(id, sketch).map(|e| &e.kind) {
        Some(EntityKind::Point { x, y }) => Some((*x, *y)),
        _ => None,
    };
    match (at(center), at(start), at(end)) {
        (Some(c), Some(s), Some(e)) => {
            let r = |p: (f64, f64)| (p.0 - c.0).hypot(p.1 - c.1);
            (r(e) - r(s)).abs() <= FROZEN_ARC_CLOSURE_MM
        }
        _ => false,
    }
}

/// Points referenced (directly or via their parent entity) by a
/// constraint: the points its residual reads. Used to attribute
/// over-constrained constraints to per-entity colours, and to leave
/// points no residual reads out of the solve (#1055).
pub(crate) fn points_touched(kind: &ConstraintKind, sketch: &SketchData) -> Vec<SketchEntityId> {
    use ConstraintKind::*;
    let mut out = Vec::new();
    match kind {
        Coincident { p1, p2 } => {
            out.push(*p1);
            out.push(*p2);
        }
        DistancePtPt { p1, p2, .. } => {
            out.push(*p1);
            out.push(*p2);
        }
        Horizontal { line } | Vertical { line } => {
            extend_with_entity_points(*line, sketch, &mut out);
        }
        Parallel { l1, l2 } | Perpendicular { l1, l2 } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        Angle { l1, l2, .. } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        PointOnLine { point, line } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        PointOnArc { point, arc } => {
            out.push(*point);
            extend_with_entity_points(*arc, sketch, &mut out);
        }
        DistancePtLine { point, line, .. } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        DistancePtCircle { point, circle, .. } => {
            out.push(*point);
            extend_with_entity_points(*circle, sketch, &mut out);
        }
        EqualLength { l1, l2 } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        EqualRadius { e1, e2 } => {
            extend_with_entity_points(*e1, sketch, &mut out);
            extend_with_entity_points(*e2, sketch, &mut out);
        }
        TangentLineArc { line, arc } => {
            extend_with_entity_points(*line, sketch, &mut out);
            extend_with_entity_points(*arc, sketch, &mut out);
        }
        TangentArcArc { a1, a2, .. } => {
            extend_with_entity_points(*a1, sketch, &mut out);
            extend_with_entity_points(*a2, sketch, &mut out);
        }
        SymmetricAboutLine { p1, p2, line } => {
            out.push(*p1);
            out.push(*p2);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        SymmetricAboutPoint { p1, p2, center } => {
            out.push(*p1);
            out.push(*p2);
            out.push(*center);
        }
        Midpoint { point, line } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        Fixed { point } => {
            out.push(*point);
        }
    }
    out
}

/// Append every Point reachable from `entity_id` (through Line/Arc/
/// Circle endpoints) to `out`.
fn extend_with_entity_points(
    entity_id: SketchEntityId,
    sketch: &SketchData,
    out: &mut Vec<SketchEntityId>,
) {
    let Some(entity) = sketch.entities.iter().find(|e| e.id == entity_id) else {
        return;
    };
    for pid in entity.point_refs() {
        out.push(pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::{Constraint, DimTarget};
    use crate::id::ConstraintId;
    use crate::plane::{Plane, PlaneId, PlaneKind};
    use crate::solver::residual::{ResolvedParams, residual};

    /// A sketch with lines, arcs and a circle, all points at distinct,
    /// non-degenerate positions.
    struct Fixture {
        sketch: SketchData,
        plane: PlaneId,
    }

    impl Fixture {
        fn new() -> Self {
            let plane = Plane {
                id: PlaneId::new(),
                kind: PlaneKind::BoardTop,
            };
            let mut sketch = SketchData::default();
            let id = plane.id;
            sketch.planes.push(plane);
            Self { sketch, plane: id }
        }
        fn add(&mut self, kind: EntityKind) -> SketchEntityId {
            let id = SketchEntityId::new();
            self.sketch.entities.push(Entity::new(id, self.plane, kind));
            id
        }
        fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
            self.add(EntityKind::Point { x, y })
        }
        fn line(&mut self, a: (f64, f64), b: (f64, f64)) -> SketchEntityId {
            let start = self.point(a.0, a.1);
            let end = self.point(b.0, b.1);
            self.add(EntityKind::Line { start, end })
        }
        fn arc(&mut self, c: (f64, f64), r: f64, a0: f64, a1: f64) -> SketchEntityId {
            let center = self.point(c.0, c.1);
            let start = self.point(c.0 + r * a0.cos(), c.1 + r * a0.sin());
            let end = self.point(c.0 + r * a1.cos(), c.1 + r * a1.sin());
            self.add(EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw: true,
            })
        }
    }

    /// Every point of the sketch as a state variable, nothing frozen.
    fn full_index(sketch: &SketchData) -> (Vec<f64>, EntityIndex) {
        let mut state = Vec::new();
        let mut index = EntityIndex::default();
        for e in &sketch.entities {
            match e.kind {
                EntityKind::Point { x, y } => {
                    index.points.insert(e.id, (state.len(), state.len() + 1));
                    state.extend([x, y]);
                }
                EntityKind::Circle { radius, .. } => {
                    index.radii.insert(e.id, state.len());
                    state.push(radius);
                }
                _ => {}
            }
        }
        (state, index)
    }

    /// #1055 — leaving points out of the solve is only sound if
    /// `points_touched` lists every point a constraint's residual reads.
    /// For every constraint kind, moving any point it does not list must
    /// leave its residual unchanged.
    #[test]
    fn points_touched_lists_every_point_a_residual_reads() {
        use ConstraintKind::*;
        let mut f = Fixture::new();
        let l1 = f.line((0.0, 0.0), (10.0, 1.0));
        let l2 = f.line((2.0, 5.0), (3.0, 12.0));
        let a1 = f.arc((20.0, 3.0), 4.0, 0.3, 1.9);
        let a2 = f.arc((31.0, -2.0), 6.0, 2.1, 3.6);
        let center = f.point(40.0, 7.0);
        let circle = f.add(EntityKind::Circle {
            center,
            radius: 2.5,
        });
        let p1 = f.point(-4.0, 6.0);
        let p2 = f.point(5.0, -7.0);
        let p3 = f.point(13.0, 9.0);
        let lit = || DimTarget::Literal(3.0);
        let kinds = vec![
            Coincident { p1, p2 },
            PointOnLine {
                point: p1,
                line: l1,
            },
            PointOnArc { point: p1, arc: a1 },
            Horizontal { line: l1 },
            Vertical { line: l2 },
            Parallel { l1, l2 },
            Perpendicular { l1, l2 },
            DistancePtPt {
                p1,
                p2,
                target: lit(),
            },
            DistancePtLine {
                point: p1,
                line: l2,
                target: lit(),
            },
            DistancePtCircle {
                point: p1,
                circle,
                target: lit(),
            },
            DistancePtCircle {
                point: p2,
                circle: a2,
                target: lit(),
            },
            Angle {
                l1,
                l2,
                target: lit(),
            },
            EqualLength { l1, l2 },
            EqualRadius { e1: a1, e2: circle },
            EqualRadius { e1: a1, e2: a2 },
            TangentLineArc { line: l1, arc: a1 },
            TangentArcArc {
                a1,
                a2,
                internal: false,
            },
            TangentArcArc {
                a1,
                a2,
                internal: true,
            },
            SymmetricAboutLine { p1, p2, line: l2 },
            SymmetricAboutPoint { p1, p2, center: p3 },
            Midpoint {
                point: p3,
                line: l1,
            },
        ];
        let params = ResolvedParams::new();
        let (state, index) = full_index(&f.sketch);
        let points: Vec<SketchEntityId> = index.points.keys().copied().collect();
        for kind in kinds {
            let c = Constraint {
                id: ConstraintId::new(),
                kind: kind.clone(),
            };
            let at = |state: &[f64]| residual(&c, state, &index, &f.sketch, &params);
            let base = at(&state).unwrap_or_else(|e| panic!("{kind:?}: {e:?}"));
            let listed = points_touched(&kind, &f.sketch);
            for p in &points {
                if listed.contains(p) {
                    continue;
                }
                let (xi, yi) = index.points[p];
                let mut moved = state.clone();
                moved[xi] += 0.73;
                moved[yi] -= 0.41;
                let after = at(&moved).unwrap_or_else(|e| panic!("{kind:?}: {e:?}"));
                assert_eq!(
                    base, after,
                    "{kind:?} reads a point points_touched does not list"
                );
            }
        }
    }

    /// #1055 — an arc no constraint touches is left out once it is closed,
    /// kept in while it is open, and pulled in whole when it shares a point
    /// with constrained geometry.
    #[test]
    fn untouched_arcs_are_frozen_only_when_closed_and_unconnected() {
        let mut f = Fixture::new();
        let closed = f.arc((0.0, 0.0), 2.0, 0.0, 1.0);
        let open_center = f.point(10.0, 0.0);
        let open_start = f.point(12.0, 0.0);
        let open_end = f.point(10.0, 3.0); // radius 3 vs 2: not closed
        f.add(EntityKind::Arc {
            center: open_center,
            start: open_start,
            end: open_end,
            sweep_ccw: true,
        });
        let shared = f.arc((20.0, 0.0), 2.0, 0.0, 1.0);
        let (_, shared_start, _, _) = arc_refs(shared, &f.sketch).unwrap();
        let other = f.point(30.0, 5.0);
        f.sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Coincident {
                p1: shared_start,
                p2: other,
            },
        });

        let packed = pack(&f.sketch);
        let (cc, cs, ce, _) = arc_refs(closed, &f.sketch).unwrap();
        for p in [cc, cs, ce] {
            assert!(packed.index.frozen.contains(&p), "closed arc frozen");
        }
        for p in [open_center, open_start, open_end] {
            assert!(packed.index.points.contains_key(&p), "open arc solved");
        }
        let (sc, ss, se, _) = arc_refs(shared, &f.sketch).unwrap();
        for p in [sc, ss, se, other] {
            assert!(packed.index.points.contains_key(&p), "connected arc solved");
        }
    }
}
