//! GH #685 — a conflicting constraint set must come back from
//! `Solver::solve` as an over-constraint that names the constraints,
//! not as a generic non-convergence.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

fn distance(
    p1: signex_sketch::id::SketchEntityId,
    p2: signex_sketch::id::SketchEntityId,
    mm: f64,
) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        },
    }
}

#[test]
fn conflicting_distances_are_reported_as_over_constrained() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    let (five, ten) = (distance(p1, p2, 5.0), distance(p1, p2, 10.0));
    let (five_id, ten_id) = (five.id, ten.id);
    s.data.constraints.push(five);
    s.data.constraints.push(ten);

    let err = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("5 mm and 10 mm cannot both hold");

    assert!(matches!(err, SolveError::OverConstrained(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains(&five_id.to_string()), "{msg}");
    assert!(msg.contains(&ten_id.to_string()), "{msg}");
}

#[test]
fn a_satisfiable_sketch_is_not_reported_as_over_constrained() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(distance(p1, p2, 5.0));

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("one distance is satisfiable");
    assert!(out.over_constraints.is_empty());
}

/// A and B fixed 8 mm apart, P on the segment's midpoint, 5 mm to each:
/// P = (4, ±3) satisfies both, but the start is a saddle (zero gradient
/// in y), so a plain LM run stalls. It must not be called a conflict.
#[test]
fn a_saddle_start_still_converges_to_the_solution() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(8.0, 0.0);
    let p = s.add_point(4.0, 0.0);
    for point in [a, b] {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point },
        });
    }
    s.data.constraints.push(distance(a, p, 5.0));
    s.data.constraints.push(distance(b, p, 5.0));

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("two 5 mm distances to P are satisfiable");

    let (x, y) =
        signex_sketch::solver::state::point_xy(p, &out.result.state, &out.result.index, &s.data)
            .expect("P is a free point");
    assert!((x - 4.0).abs() < 1e-6, "x = {x}");
    assert!((y.abs() - 3.0).abs() < 1e-6, "y = {y}");
}

/// Fixed A and B 1 mm apart, a 5 mm distance between them, and a point C
/// 2 mm from A and 2.5 mm from B. Only the 5 mm distance has to go for the
/// sketch to solve: C's distances are strained by the conflict but are not
/// part of it, so the report must name the 5 mm distance alone.
#[test]
fn the_reported_conflict_excludes_strained_bystanders() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let c = s.add_point(0.5, 1.5);
    for point in [a, b] {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point },
        });
    }
    let five = distance(a, b, 5.0);
    let five_id = five.id;
    s.data.constraints.push(five);
    s.data.constraints.push(distance(a, c, 2.0));
    s.data.constraints.push(distance(b, c, 2.5));

    let err = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("the 5 mm distance contradicts the fixed points");

    let SolveError::OverConstrained(ids) = err else {
        panic!("expected an over-constraint, got {err:?}");
    };
    assert_eq!(ids, vec![five_id], "only the 5 mm distance is the culprit");
}

/// Three mutually contradictory distances: no single removal resolves the
/// conflict, so all three are named.
#[test]
fn a_conflict_no_single_removal_resolves_names_the_whole_set() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    let trio = [
        distance(p1, p2, 5.0),
        distance(p1, p2, 7.0),
        distance(p1, p2, 10.0),
    ];
    let mut trio_ids: Vec<_> = trio.iter().map(|c| c.id).collect();
    s.data.constraints.extend(trio);

    let err = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("three different distances cannot all hold");

    let SolveError::OverConstrained(mut ids) = err else {
        panic!("expected an over-constraint, got {err:?}");
    };
    ids.sort_by_key(|id| id.to_string());
    trio_ids.sort_by_key(|id| id.to_string());
    assert_eq!(ids, trio_ids);
}

/// A free point no constraint touches must come back exactly where it
/// was, even when the solve needed the offset retry to leave a saddle.
#[test]
fn the_saddle_retry_does_not_drift_an_unrelated_free_point() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(8.0, 0.0);
    let p = s.add_point(4.0, 0.0);
    let free = s.add_point(100.123, -7.77);
    for point in [a, b] {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point },
        });
    }
    s.data.constraints.push(distance(a, p, 5.0));
    s.data.constraints.push(distance(b, p, 5.0));

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("the saddle sketch solves");

    let at = |id| {
        signex_sketch::solver::state::point_xy(id, &out.result.state, &out.result.index, &s.data)
            .expect("free point")
    };
    assert_eq!(at(free), (100.123, -7.77), "the retry offset was undone");
    let (_, y) = at(p);
    assert!(
        (y.abs() - 3.0).abs() < 1e-6,
        "the saddle still resolved: y = {y}"
    );
}
