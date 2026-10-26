//! `shrink_conflict` under a draining work pool: it may over-name a
//! conflict, never under-name it.

use crate::SketchData;
use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::plane::{Plane, PlaneId, PlaneKind};

use super::Solver;
use super::residual::ResolvedParams;
use super::work::WorkMeter;

/// A fixed origin and a chain of two unit links whose ends are asked to be
/// 5 mm apart: removing any one of the three distances resolves it.
fn triangle_that_cannot_close() -> (SketchData, Vec<ConstraintId>) {
    let plane = PlaneId::new();
    let [a, b, c] =
        [(0.0, 0.0), (1.0, 0.1), (2.0, 0.3)].map(|(x, y)| (SketchEntityId::new(), x, y));
    let distance = |p1: SketchEntityId, p2: SketchEntityId, mm: f64| Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        },
    };
    let links = vec![
        distance(a.0, b.0, 1.0),
        distance(b.0, c.0, 1.0),
        distance(a.0, c.0, 5.0),
    ];
    let named = links.iter().map(|k| k.id).collect();
    let mut constraints = vec![Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a.0 },
    }];
    constraints.extend(links);
    let data = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: [a, b, c]
            .iter()
            .map(|&(id, x, y)| Entity::new(id, plane, EntityKind::Point { x, y }))
            .collect(),
        constraints,
        ..SketchData::default()
    };
    (data, named)
}

#[test]
fn a_pool_that_runs_out_mid_narrowing_never_drops_a_named_constraint() {
    let (sketch, named) = triangle_that_cannot_close();
    let solver = Solver::default();
    let params = ResolvedParams::new();

    let mut unlimited = WorkMeter::new(u64::MAX);
    let narrowed = solver.shrink_conflict(&sketch, &params, &mut unlimited, named.clone());
    let total = unlimited.spent();
    assert!(narrowed.len() >= 2, "precondition: {narrowed:?}");
    assert!(total > 0, "precondition: narrowing costs work");

    for budget in 0..=total {
        let mut meter = WorkMeter::new(budget);
        let result = solver.shrink_conflict(&sketch, &params, &mut meter, named.clone());
        assert!(
            narrowed.iter().all(|id| result.contains(id)),
            "budget {budget} of {total}: {result:?} leaves out part of {narrowed:?}"
        );
    }
}
