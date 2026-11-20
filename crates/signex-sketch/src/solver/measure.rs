//! What a dimension measures on the geometry (#187, #188).
//!
//! A driven (reference) dimension never drives the solve; the drawing shows
//! the value it measures instead. The value is the dimension's own residual
//! with its target set to zero, so each kind keeps one geometry formula:
//! the residual is `measured - target` for every dimension kind.

use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
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
}
