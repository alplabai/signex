//! Driven (reference) dimensions in the solve (#187, #188): no residual,
//! no Jacobian row, no DOF, never a conflict or a redundancy.

use std::collections::HashMap;

use crate::SolveError;
use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::plane::{Plane, PlaneId, PlaneKind};
use crate::sketch::SketchData;
use crate::solver::Solver;
use crate::solver::dof::DofColor;
use crate::solver::jacobian::jacobian;
use crate::solver::measure::measured_value;
use crate::solver::residual::total_residual;
use crate::solver::state::pack;

struct Rig {
    sketch: SketchData,
    p1: SketchEntityId,
    p2: SketchEntityId,
}

/// P1 fixed at the origin, P2 at (4, 0.3), the line P1-P2 horizontal.
fn rig() -> Rig {
    let plane = PlaneId::new();
    let (p1, p2, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(p2, plane, EntityKind::Point { x: 4.0, y: 0.3 }),
            Entity::new(line, plane, EntityKind::Line { start: p1, end: p2 }),
        ],
        constraints: vec![
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: p1 },
            },
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Horizontal { line },
            },
        ],
        ..SketchData::default()
    };
    Rig { sketch, p1, p2 }
}

impl Rig {
    fn distance(&mut self, mm: f64) -> ConstraintId {
        let id = ConstraintId::new();
        self.sketch.constraints.push(Constraint {
            id,
            kind: ConstraintKind::DistancePtPt {
                p1: self.p1,
                p2: self.p2,
                target: DimTarget::Literal(mm),
            },
        });
        id
    }

    fn rows(&self) -> usize {
        let packed = pack(&self.sketch);
        total_residual(&self.sketch, &packed.vector, &packed.index, &HashMap::new())
            .expect("evaluates")
            .len()
    }

    fn constraint(&self, id: ConstraintId) -> &Constraint {
        self.sketch
            .constraints
            .iter()
            .find(|c| c.id == id)
            .expect("constraint")
    }
}

#[test]
fn a_driven_dimension_adds_no_residual_and_no_dof() {
    let mut rig = rig();
    let driving = rig.distance(5.0);
    let rows_before = rig.rows();
    let reference = rig.distance(5.0);
    assert_eq!(rig.rows(), rows_before + 1, "a driving copy adds a row");
    rig.sketch.set_driven(reference, true);
    assert_eq!(rig.rows(), rows_before, "a driven one adds none");

    let out = Solver::default()
        .solve(&rig.sketch, &HashMap::new())
        .expect("solves");
    let packed = pack(&rig.sketch);
    let j = jacobian(&rig.sketch, &packed.vector, &packed.index, &HashMap::new()).expect("j");
    assert_eq!(j.len(), rows_before, "no Jacobian row either");
    assert_eq!(
        out.colours.get(&rig.p2),
        Some(&DofColor::Full),
        "H and the driving distance pin P2; the reference takes no DOF"
    );
    assert!(out.over_constraints.is_empty());
    assert!(
        out.redundant_constraints.is_empty(),
        "an equal driving copy would be redundant; the reference is not"
    );
    let measured = measured_value(&rig.sketch, Some(&out.result), rig.constraint(reference));
    assert!(
        measured.is_some_and(|v| (v - 5.0).abs() < 1e-9),
        "{measured:?}"
    );
    assert!(!rig.sketch.is_driven(driving));
}

#[test]
fn a_point_only_a_driven_dimension_reads_stays_free_and_unmoved() {
    let mut rig = rig();
    // Drop the horizontal: only the fixed point and the reference remain.
    rig.sketch
        .constraints
        .retain(|c| !matches!(c.kind, ConstraintKind::Horizontal { .. }));
    let reference = rig.distance(1.0);
    rig.sketch.set_driven(reference, true);
    assert_eq!(rig.rows(), 0);
    let out = Solver::default()
        .solve(&rig.sketch, &HashMap::new())
        .expect("solves");
    assert_eq!(out.colours.get(&rig.p2), Some(&DofColor::Under));
    let measured = measured_value(&rig.sketch, Some(&out.result), rig.constraint(reference));
    let expected = 4.0_f64.hypot(0.3);
    assert!(
        measured.is_some_and(|v| (v - expected).abs() < 1e-9),
        "the target 1 mm never pulls P2: {measured:?}"
    );
}

#[test]
fn converting_the_conflicting_dimension_makes_the_sketch_solve() {
    let mut rig = rig();
    rig.distance(5.0);
    let clash = rig.distance(7.0);
    let err = Solver::default()
        .solve(&rig.sketch, &HashMap::new())
        .expect_err("5 mm and 7 mm cannot both hold");
    assert!(
        matches!(&err, SolveError::OverConstrained(ids) if ids.contains(&clash)),
        "{err:?}"
    );

    assert!(rig.sketch.set_driven(clash, true));
    let out = Solver::default()
        .solve(&rig.sketch, &HashMap::new())
        .expect("the reference no longer clashes");
    assert!(out.over_constraints.is_empty());
    let measured = measured_value(&rig.sketch, Some(&out.result), rig.constraint(clash));
    assert!(
        measured.is_some_and(|v| (v - 5.0).abs() < 1e-9),
        "it measures the driving 5 mm, not its own 7: {measured:?}"
    );
}
