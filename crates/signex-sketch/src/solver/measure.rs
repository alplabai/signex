//! What a dimension measures on the geometry (mockup row 187, mockup row 188).
//!
//! A driven (reference) dimension never drives the solve; the drawing shows
//! the value it measures instead. The value is the dimension's own residual
//! with its target set to zero, so each kind keeps one geometry formula:
//! the residual is `measured - target` for every dimension kind.

use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::EntityKind;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::lm::SolveResult;
use crate::solver::residual::residual;
use crate::solver::state::{EntityIndex, pack};

/// The value dimension `constraint` measures at `solve` (mm, or rad for an
/// angle), or on the geometry as authored when there is no solve. `None`
/// for a constraint that is not a dimension, or whose geometry cannot be
/// read (a missing entity, a zero-length line).
pub fn measured_value(
    sketch: &SketchData,
    solve: Option<&SolveResult>,
    constraint: &Constraint,
) -> Option<f64> {
    match solve {
        Some(solve) => measured_value_at(sketch, &solve.state, &solve.index, constraint),
        None => {
            let packed = pack(sketch);
            measured_value_at(sketch, &packed.vector, &packed.index, constraint)
        }
    }
}

/// [`measured_value`] at a given state vector and its index.
pub fn measured_value_at(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    constraint: &Constraint,
) -> Option<f64> {
    use ConstraintKind::*;
    let mut zeroed = constraint.clone();
    match &mut zeroed.kind {
        DistancePtPt { target, .. }
        | DistancePtLine { target, .. }
        | DistancePtCircle { target, .. }
        | Angle { target, .. } => *target = DimTarget::Literal(0.0),
        _ => return None,
    }
    let rows = residual(&zeroed, state, index, sketch, &HashMap::new()).ok()?;
    rows.first().copied().filter(|v| v.is_finite())
}

/// A sketch point in plane mm.
type Xy = (f64, f64);

/// Where dimension `kind`'s line runs: between the two points, from a
/// point to its foot on a line, or from a point to the nearest point of a
/// circle or arc. `point` places a sketch point and `solved_radius` gives
/// a circle's solved radius (`None` keeps the authored one), so the sketch
/// canvas and the bake preview draw the same line from their own
/// positions. `None` for an angle, a relation, or geometry with no line
/// (a zero-length line, a point at a circle's centre).
pub fn dimension_span(
    kind: &ConstraintKind,
    sketch: &SketchData,
    point: impl Fn(SketchEntityId) -> Option<Xy>,
    solved_radius: impl Fn(SketchEntityId) -> Option<f64>,
) -> Option<(Xy, Xy)> {
    let entity = |id: SketchEntityId| sketch.entities.iter().find(|e| e.id == id);
    match *kind {
        ConstraintKind::DistancePtPt { p1, p2, .. } => Some((point(p1)?, point(p2)?)),
        ConstraintKind::DistancePtLine { point: p, line, .. } => {
            let EntityKind::Line { start, end } = entity(line)?.kind else {
                return None;
            };
            let p = point(p)?;
            Some((p, foot_on_line(p, point(start)?, point(end)?)?))
        }
        ConstraintKind::DistancePtCircle {
            point: p, circle, ..
        } => {
            let (centre, radius) = match entity(circle)?.kind {
                EntityKind::Circle { center, radius } => {
                    (point(center)?, solved_radius(circle).unwrap_or(radius))
                }
                EntityKind::Arc { center, start, .. } => {
                    let (c, s) = (point(center)?, point(start)?);
                    (c, (s.0 - c.0).hypot(s.1 - c.1))
                }
                _ => return None,
            };
            let p = point(p)?;
            Some((p, nearest_on_circle(p, centre, radius)?))
        }
        _ => None,
    }
}

/// The foot of the perpendicular from `p` to the line through `a` and `b`;
/// `None` when the two ends meet.
fn foot_on_line(p: Xy, a: Xy, b: Xy) -> Option<Xy> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        return None;
    }
    let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2;
    Some((a.0 + t * dx, a.1 + t * dy))
}

/// The point of the circle at `centre` with `radius` nearest `p`; `None`
/// when `p` is the centre.
fn nearest_on_circle(p: Xy, centre: Xy, radius: f64) -> Option<Xy> {
    let d = (p.0 - centre.0).hypot(p.1 - centre.1);
    if d <= f64::EPSILON {
        return None;
    }
    let k = radius / d;
    Some((
        centre.0 + (p.0 - centre.0) * k,
        centre.1 + (p.1 - centre.1) * k,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, EntityKind};
    use crate::id::{ConstraintId, SketchEntityId};
    use crate::plane::{Plane, PlaneId, PlaneKind};

    struct Rig {
        sketch: SketchData,
        plane: PlaneId,
    }

    impl Rig {
        fn new() -> Self {
            let plane = PlaneId::new();
            Self {
                sketch: SketchData {
                    planes: vec![Plane {
                        id: plane,
                        kind: PlaneKind::BoardTop,
                    }],
                    ..SketchData::default()
                },
                plane,
            }
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
            let (start, end) = (self.point(a.0, a.1), self.point(b.0, b.1));
            self.add(EntityKind::Line { start, end })
        }

        /// The value `kind` measures on the authored geometry.
        fn measure(&self, kind: ConstraintKind) -> Option<f64> {
            let c = Constraint {
                id: ConstraintId::new(),
                kind,
            };
            measured_value(&self.sketch, None, &c)
        }
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn a_distance_between_points_measures_their_separation() {
        let mut rig = Rig::new();
        let (p1, p2) = (rig.point(1.0, 1.0), rig.point(4.0, 5.0));
        let got = rig.measure(ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(99.0),
        });
        assert!(close(got, 5.0), "the target plays no part: {got:?}");
    }

    #[test]
    fn a_distance_to_a_line_measures_the_perpendicular() {
        let mut rig = Rig::new();
        let line = rig.line((0.0, 0.0), (10.0, 0.0));
        let point = rig.point(3.0, 2.5);
        let got = rig.measure(ConstraintKind::DistancePtLine {
            point,
            line,
            target: DimTarget::Literal(1.0),
        });
        assert!(close(got.map(f64::abs), 2.5), "{got:?}");
    }

    #[test]
    fn a_distance_to_a_circle_measures_the_gap_to_its_boundary() {
        let mut rig = Rig::new();
        let center = rig.point(0.0, 0.0);
        let circle = rig.add(EntityKind::Circle {
            center,
            radius: 2.0,
        });
        let point = rig.point(5.0, 0.0);
        let got = rig.measure(ConstraintKind::DistancePtCircle {
            point,
            circle,
            target: DimTarget::Literal(0.0),
        });
        assert!(close(got, 3.0), "{got:?}");
    }

    #[test]
    fn an_angle_measures_the_turn_between_its_lines() {
        let mut rig = Rig::new();
        let l1 = rig.line((0.0, 0.0), (1.0, 0.0));
        let l2 = rig.line((0.0, 0.0), (1.0, 1.0));
        let got = rig.measure(ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Expr("no_such_parameter".into()),
        });
        assert!(
            close(got.map(f64::abs), std::f64::consts::FRAC_PI_4),
            "an unresolvable target does not matter: {got:?}"
        );
    }

    #[test]
    fn a_relation_measures_nothing() {
        let mut rig = Rig::new();
        let line = rig.line((0.0, 0.0), (1.0, 0.0));
        assert_eq!(rig.measure(ConstraintKind::Horizontal { line }), None);
    }

    #[test]
    fn a_solved_state_is_read_over_the_authored_one() {
        let mut rig = Rig::new();
        let (p1, p2) = (rig.point(0.0, 0.0), rig.point(1.0, 0.0));
        let fixed = Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point: p1 },
        };
        let driving = Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(3.0),
            },
        };
        let reference = Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(1.0),
            },
        };
        rig.sketch
            .constraints
            .extend([fixed, driving, reference.clone()]);
        rig.sketch.set_driven(reference.id, true);
        let out = crate::Solver::default()
            .solve(&rig.sketch, &HashMap::new())
            .expect("solves");
        let got = measured_value(&rig.sketch, Some(&out.result), &reference);
        assert!(close(got, 3.0), "it follows the driving value: {got:?}");
    }

    impl Rig {
        /// The dimension line of `kind` on the authored geometry.
        fn span(&self, kind: ConstraintKind) -> Option<(Xy, Xy)> {
            dimension_span(&kind, &self.sketch, |id| self.authored(id), |_| None)
        }

        /// Where point `id` was drawn.
        fn authored(&self, id: SketchEntityId) -> Option<Xy> {
            match self.sketch.entities.iter().find(|e| e.id == id)?.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            }
        }
    }

    fn near(got: Option<(Xy, Xy)>, a: Xy, b: Xy) -> bool {
        let at = |p: Xy, q: Xy| (p.0 - q.0).hypot(p.1 - q.1) < 1e-9;
        got.is_some_and(|(p, q)| at(p, a) && at(q, b))
    }

    #[test]
    fn a_distance_to_a_line_runs_to_the_foot_of_the_perpendicular() {
        let mut rig = Rig::new();
        let line = rig.line((0.0, 0.0), (10.0, 0.0));
        let point = rig.point(3.0, 2.5);
        let got = rig.span(ConstraintKind::DistancePtLine {
            point,
            line,
            target: DimTarget::Literal(1.0),
        });
        assert!(near(got, (3.0, 2.5), (3.0, 0.0)), "{got:?}");
    }

    #[test]
    fn a_distance_to_a_circle_runs_to_its_nearest_point() {
        let mut rig = Rig::new();
        let center = rig.point(0.0, 0.0);
        let circle = rig.add(EntityKind::Circle {
            center,
            radius: 2.0,
        });
        let point = rig.point(0.0, 5.0);
        let kind = ConstraintKind::DistancePtCircle {
            point,
            circle,
            target: DimTarget::Literal(0.0),
        };
        assert!(near(rig.span(kind.clone()), (0.0, 5.0), (0.0, 2.0)));
        let solved = dimension_span(&kind, &rig.sketch, |id| rig.authored(id), |_| Some(4.0));
        assert!(
            near(solved, (0.0, 5.0), (0.0, 4.0)),
            "the solved radius wins"
        );
    }

    #[test]
    fn a_distance_between_points_runs_between_them_and_an_angle_has_no_line() {
        let mut rig = Rig::new();
        let (p1, p2) = (rig.point(1.0, 1.0), rig.point(4.0, 5.0));
        let got = rig.span(ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        });
        assert!(near(got, (1.0, 1.0), (4.0, 5.0)), "{got:?}");
        let l1 = rig.line((0.0, 0.0), (1.0, 0.0));
        let l2 = rig.line((0.0, 0.0), (1.0, 1.0));
        let angle = ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(0.5),
        };
        assert_eq!(rig.span(angle), None);
    }
}
