//! Close gap (#190): join two open profile ends that are nearly touching.
//!
//! The bake walks a profile through shared endpoint points, so a gap is
//! closed by joining the two end points into one: every edge, constraint
//! and pattern that named the removed end names the kept one, and a
//! constraint left naming one point twice (a coincidence of the two ends)
//! goes. Only a gap up to [`HEAL_TOLERANCE_MM`] heals; a wider one is a
//! drawing error, not a rounding one. Rules: `docs/PCB_SKETCH.md` and
//! `docs/FOOTPRINT_SKETCH.md`, "Close gap".

use thiserror::Error;

use crate::constraint::ConstraintKind;
use crate::entity::EntityKind;
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;
use crate::solver::FullSolveOutput;
use crate::solver::state::point_xy;

/// The one heal tolerance (decision bs-11): the widest gap between two
/// profile ends that Close gap joins, in mm. DXF import heals with it too.
pub const HEAL_TOLERANCE_MM: f64 = 0.05;

/// Slack on the tolerance for a gap measured from solved floats.
const TOLERANCE_SLACK_MM: f64 = 1e-9;

/// Why Close gap did not join two ends.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum CloseGapError {
    /// The ends are further apart than the heal tolerance.
    #[error(
        "the gap is {gap_mm:.3} mm, wider than the {} mm heal tolerance",
        HEAL_TOLERANCE_MM
    )]
    TooWide { gap_mm: f64 },
    /// An end is not a point of the sketch.
    #[error("entity {0} is not a point of the sketch")]
    NotAPoint(SketchEntityId),
    /// An end has no solved position.
    #[error("point {0} has no solved position")]
    Unsolved(SketchEntityId),
    /// Both ends belong to one edge: joining them would collapse it.
    #[error("the two ends are one edge's ends; joining them would collapse it")]
    Collapses,
}

/// What Close gap changed.
#[derive(Debug, Clone, PartialEq)]
pub struct GapClosed {
    /// The point both ends are now.
    pub kept: SketchEntityId,
    /// The point that was joined into `kept` and removed.
    pub removed: SketchEntityId,
    /// Constraints that named both ends and were dropped.
    pub dropped_constraints: Vec<ConstraintId>,
}

/// Whether a gap of `gap_mm` is within the heal tolerance.
pub fn heals(gap_mm: f64) -> bool {
    gap_mm.is_finite() && gap_mm <= HEAL_TOLERANCE_MM + TOLERANCE_SLACK_MM
}

/// The distance between points `a` and `b` at the positions of `solve`.
pub fn gap_mm(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    a: SketchEntityId,
    b: SketchEntityId,
) -> Result<f64, CloseGapError> {
    let at = |id| {
        let is_point = sketch
            .entities
            .iter()
            .any(|e| e.id == id && matches!(e.kind, EntityKind::Point { .. }));
        if !is_point {
            return Err(CloseGapError::NotAPoint(id));
        }
        point_xy(id, &solve.result.state, &solve.result.index, sketch)
            .ok_or(CloseGapError::Unsolved(id))
    };
    let (ax, ay) = at(a)?;
    let (bx, by) = at(b)?;
    Ok((ax - bx).hypot(ay - by))
}

/// Join the open ends `end_a` and `end_b` of a profile when the gap
/// between them, at the positions of `solve`, is within
/// [`HEAL_TOLERANCE_MM`]. A fixed end is the one kept, else `end_a`. The
/// sketch is unchanged on an error; the caller solves it again after.
pub fn close_gap(
    sketch: &mut SketchData,
    solve: &FullSolveOutput,
    end_a: SketchEntityId,
    end_b: SketchEntityId,
) -> Result<GapClosed, CloseGapError> {
    let gap = gap_mm(sketch, solve, end_a, end_b)?;
    if !heals(gap) {
        return Err(CloseGapError::TooWide { gap_mm: gap });
    }
    if end_a == end_b || joins_one_edge(sketch, end_a, end_b) {
        return Err(CloseGapError::Collapses);
    }
    let is_fixed = |id| {
        sketch
            .constraints
            .iter()
            .any(|c| c.kind == ConstraintKind::Fixed { point: id })
    };
    let (kept, removed) = if is_fixed(end_b) && !is_fixed(end_a) {
        (end_b, end_a)
    } else {
        (end_a, end_b)
    };
    join_points(sketch, kept, removed);
    let mut dropped_constraints = Vec::new();
    sketch.constraints.retain(|c| {
        let ids: Vec<SketchEntityId> = c.kind.entities().collect();
        let twice = ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id));
        if twice {
            dropped_constraints.push(c.id);
        }
        !twice
    });
    Ok(GapClosed {
        kept,
        removed,
        dropped_constraints,
    })
}

/// Whether one edge has both `a` and `b` as its ends.
fn joins_one_edge(sketch: &SketchData, a: SketchEntityId, b: SketchEntityId) -> bool {
    sketch.entities.iter().any(|e| match e.kind {
        EntityKind::Line { start, end } | EntityKind::Arc { start, end, .. } => {
            (start == a && end == b) || (start == b && end == a)
        }
        EntityKind::Point { .. } | EntityKind::Circle { .. } => false,
    })
}

/// Make everything that names `removed` name `kept`, then remove it.
fn join_points(sketch: &mut SketchData, kept: SketchEntityId, removed: SketchEntityId) {
    let swap = |id: &mut SketchEntityId| {
        if *id == removed {
            *id = kept;
        }
    };
    for entity in &mut sketch.entities {
        match &mut entity.kind {
            EntityKind::Point { .. } => {}
            EntityKind::Line { start, end } => {
                swap(start);
                swap(end);
            }
            EntityKind::Arc {
                center, start, end, ..
            } => {
                swap(center);
                swap(start);
                swap(end);
            }
            EntityKind::Circle { center, .. } => swap(center),
        }
    }
    for constraint in &mut sketch.constraints {
        repoint_constraint(&mut constraint.kind, &swap);
    }
    for array in &mut sketch.arrays {
        if let crate::array::ArrayKind::Polar { center, .. } = &mut array.kind {
            swap(center);
        }
    }
    sketch.entities.retain(|e| e.id != removed);
}

/// Apply `swap` to every point a constraint names. Lines, arcs and circles
/// are never an end, so the kinds that name only those are left alone.
fn repoint_constraint(kind: &mut ConstraintKind, swap: &impl Fn(&mut SketchEntityId)) {
    use ConstraintKind::*;
    match kind {
        Coincident { p1, p2 } | DistancePtPt { p1, p2, .. } | SymmetricAboutLine { p1, p2, .. } => {
            swap(p1);
            swap(p2);
        }
        SymmetricAboutPoint { p1, p2, center } => {
            swap(p1);
            swap(p2);
            swap(center);
        }
        PointOnLine { point, .. }
        | PointOnArc { point, .. }
        | DistancePtLine { point, .. }
        | DistancePtCircle { point, .. }
        | Midpoint { point, .. }
        | Fixed { point } => swap(point),
        Horizontal { .. }
        | Vertical { .. }
        | Parallel { .. }
        | Perpendicular { .. }
        | Angle { .. }
        | EqualLength { .. }
        | EqualRadius { .. }
        | TangentLineArc { .. }
        | TangentArcArc { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Solver;
    use crate::constraint::Constraint;
    use crate::entity::Entity;
    use crate::parameter::resolve;
    use crate::plane::{Plane, PlaneId, PlaneKind};

    /// Two lines, `p0 - p1` and `p2 - p3`, with `p2` `gap` above `p1`.
    struct Open {
        sketch: SketchData,
        p: [SketchEntityId; 4],
        lines: [SketchEntityId; 2],
    }

    fn open(gap: f64) -> Open {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let mut add = |kind| {
            let id = SketchEntityId::new();
            sketch.entities.push(Entity::new(id, plane, kind));
            id
        };
        let p = [
            add(EntityKind::Point { x: 0.0, y: 0.0 }),
            add(EntityKind::Point { x: 10.0, y: 0.0 }),
            add(EntityKind::Point { x: 10.0, y: gap }),
            add(EntityKind::Point { x: 10.0, y: 10.0 }),
        ];
        let lines = [
            add(EntityKind::Line {
                start: p[0],
                end: p[1],
            }),
            add(EntityKind::Line {
                start: p[2],
                end: p[3],
            }),
        ];
        Open { sketch, p, lines }
    }

    fn solved(sketch: &SketchData) -> FullSolveOutput {
        let params = resolve(&sketch.parameters).unwrap();
        Solver::default().solve(sketch, &params).unwrap()
    }

    fn line_ends(sketch: &SketchData, id: SketchEntityId) -> (SketchEntityId, SketchEntityId) {
        match sketch.entities.iter().find(|e| e.id == id).unwrap().kind {
            EntityKind::Line { start, end } => (start, end),
            _ => unreachable!(),
        }
    }

    #[test]
    fn the_heal_tolerance_is_five_hundredths_of_a_millimetre() {
        assert_eq!(HEAL_TOLERANCE_MM, 0.05);
        assert!(heals(0.05));
        assert!(heals(0.0));
        assert!(!heals(0.0501));
        assert!(!heals(f64::NAN));
    }

    #[test]
    fn a_gap_within_the_tolerance_joins_the_two_ends_into_one_point() {
        let Open {
            mut sketch,
            p,
            lines,
        } = open(0.03);
        let solve = solved(&sketch);
        let closed = close_gap(&mut sketch, &solve, p[1], p[2]).unwrap();
        assert_eq!(closed.kept, p[1]);
        assert_eq!(closed.removed, p[2]);
        assert_eq!(line_ends(&sketch, lines[1]), (p[1], p[3]));
        assert!(sketch.entities.iter().all(|e| e.id != p[2]));
        assert_eq!(line_ends(&sketch, lines[0]), (p[0], p[1]));
    }

    #[test]
    fn a_gap_wider_than_the_tolerance_is_refused_with_its_width() {
        let Open { mut sketch, p, .. } = open(0.12);
        let before = sketch.clone();
        let solve = solved(&sketch);
        let err = close_gap(&mut sketch, &solve, p[1], p[2]).unwrap_err();
        let CloseGapError::TooWide { gap_mm } = err else {
            panic!("{err:?}");
        };
        assert!((gap_mm - 0.12).abs() < 1e-6, "{gap_mm}");
        assert_eq!(sketch, before, "a refusal changes nothing");
        assert_eq!(
            err.to_string(),
            "the gap is 0.120 mm, wider than the 0.05 mm heal tolerance"
        );
    }

    #[test]
    fn constraints_follow_the_kept_point_and_a_coincidence_of_the_ends_goes() {
        let Open { mut sketch, p, .. } = open(0.02);
        let coincident = ConstraintId::new();
        let distance = ConstraintId::new();
        sketch.constraints.push(Constraint {
            id: coincident,
            kind: ConstraintKind::Coincident { p1: p[1], p2: p[2] },
        });
        sketch.constraints.push(Constraint {
            id: distance,
            kind: ConstraintKind::DistancePtPt {
                p1: p[2],
                p2: p[3],
                target: crate::constraint::DimTarget::Literal(10.0),
            },
        });
        let solve = solved(&sketch);
        let closed = close_gap(&mut sketch, &solve, p[1], p[2]).unwrap();
        assert_eq!(closed.dropped_constraints, vec![coincident]);
        let kept = sketch
            .constraints
            .iter()
            .find(|c| c.id == distance)
            .unwrap();
        assert!(matches!(
            kept.kind,
            ConstraintKind::DistancePtPt { p1, p2, .. } if p1 == p[1] && p2 == p[3]
        ));
    }

    #[test]
    fn a_fixed_end_is_the_one_kept() {
        let Open {
            mut sketch,
            p,
            lines,
        } = open(0.01);
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point: p[2] },
        });
        let solve = solved(&sketch);
        let closed = close_gap(&mut sketch, &solve, p[1], p[2]).unwrap();
        assert_eq!(closed.kept, p[2]);
        assert_eq!(line_ends(&sketch, lines[0]), (p[0], p[2]));
    }

    #[test]
    fn the_two_ends_of_one_edge_are_never_joined() {
        let Open { mut sketch, p, .. } = open(0.01);
        let solve = solved(&sketch);
        assert_eq!(
            close_gap(&mut sketch, &solve, p[2], p[2]),
            Err(CloseGapError::Collapses)
        );
        // A short edge whose own ends are within the tolerance.
        let Open { mut sketch, p, .. } = open(0.01);
        let plane = sketch.planes[0].id;
        sketch.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line {
                start: p[1],
                end: p[2],
            },
        ));
        let solve = solved(&sketch);
        assert_eq!(
            close_gap(&mut sketch, &solve, p[1], p[2]),
            Err(CloseGapError::Collapses)
        );
    }

    #[test]
    fn an_end_that_is_not_a_point_is_refused() {
        let Open {
            mut sketch,
            p,
            lines,
        } = open(0.01);
        let solve = solved(&sketch);
        assert_eq!(
            close_gap(&mut sketch, &solve, p[1], lines[0]),
            Err(CloseGapError::NotAPoint(lines[0]))
        );
    }
}
