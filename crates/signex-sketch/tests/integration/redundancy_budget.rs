//! The redundancy pass draws on the solve's work budget: a sketch whose scan
//! would not fit skips it, says so, and reports no ids. Asserts on work
//! units, never on the clock.

use crate::common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::DEFAULT_WORK_BUDGET;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::work::WorkMeter;
use signex_sketch::solver::{REDUNDANCY_SKIPPED, Solver};

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind,
    }
}

/// `points` points, each at a fixed distance from two shared, free datums.
/// Every row shares the datums' columns, so the sketch is ONE block, and
/// the points start where the constraints put them (no LM steps).
fn datum_fan(points: usize) -> Sketch {
    let mut s = Sketch::new();
    let datums = [(0.0, 0.0), (10.0, 3.0)].map(|(x, y)| (s.add_point(x, y), x, y));
    for i in 0..points {
        let (x, y) = ((i + 1) as f64, 0.5);
        let p = s.add_point(x, y);
        for (datum, dx, dy) in datums {
            s.data
                .constraints
                .push(constraint(ConstraintKind::DistancePtPt {
                    p1: datum,
                    p2: p,
                    target: DimTarget::Literal((x - dx).hypot(y - dy)),
                }));
        }
    }
    s
}

/// Repeat the first constraint, so the pass has something to find.
fn with_repeats(mut s: Sketch, times: usize) -> Sketch {
    let kind = s.data.constraints[0].kind.clone();
    for _ in 0..times {
        s.data.constraints.push(constraint(kind.clone()));
    }
    s
}

#[test]
fn a_large_single_block_sketch_skips_the_pass_within_the_default_budget() {
    let s = with_repeats(datum_fan(199), 2);
    assert_eq!(s.data.constraints.len(), 400);
    let params = ResolvedParams::new();

    // What the pass would cost: the same solve with a pool nothing can exhaust.
    let mut unlimited = WorkMeter::new(u64::MAX);
    let full = Solver::default()
        .solve_with_meter(&s.data, &params, &mut unlimited)
        .expect("solves");
    assert!(full.warnings.is_empty());
    assert_eq!(full.redundant_constraints.len(), 2);

    let mut meter = WorkMeter::new(DEFAULT_WORK_BUDGET);
    let out = Solver::default()
        .solve_with_meter(&s.data, &params, &mut meter)
        .expect("the solve itself fits the default budget");

    // The pass would not have fit what the LM left, so it did not run.
    let lm_spent = meter.spent();
    let pass_cost = unlimited.spent() - lm_spent;
    assert!(
        pass_cost > DEFAULT_WORK_BUDGET - lm_spent,
        "pass {pass_cost} fits the {} units left",
        DEFAULT_WORK_BUDGET - lm_spent
    );
    assert!(meter.spent() <= DEFAULT_WORK_BUDGET);
    assert_eq!(out.warnings, vec![REDUNDANCY_SKIPPED.to_string()]);
    assert!(out.redundant_constraints.is_empty());
}

#[test]
fn a_small_sketch_is_charged_and_still_reports_its_redundant_constraints() {
    let s = with_repeats(datum_fan(3), 1);
    let params = ResolvedParams::new();
    let mut meter = WorkMeter::new(DEFAULT_WORK_BUDGET);
    let out = Solver::default()
        .solve_with_meter(&s.data, &params, &mut meter)
        .expect("solves");
    assert!(out.warnings.is_empty());
    assert_eq!(out.redundant_constraints.len(), 1);
    assert!(meter.spent() > 0 && meter.spent() < DEFAULT_WORK_BUDGET);
}
