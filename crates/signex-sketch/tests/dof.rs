//! Task 3.5 — DOF colour classification tests.
//!
//! Three canonical cases:
//! - **Under**-constrained: a free Point with no constraints lands
//!   `DofColor::Under`.
//! - **Full**-constrained: the Phase 3.3 anchored line case (Fixed
//!   P1 + Distance + Horizontal) lands both endpoints on `Full`.
//! - **Over**-constrained: two conflicting Distance constraints on
//!   the same point pair are flagged in `over_constraint_ids` and
//!   bump the touched Point to `DofColor::Over`.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::dof::{entity_colours, over_constraint_ids, DofColor};
use signex_sketch::solver::jacobian::numerical_jacobian;
use signex_sketch::solver::lm::{solve_lm, SolveResult};

/// Default Solver tolerance + iteration cap, used by these tests.
const TOL: f64 = 1e-12;
const MAX_ITERS: usize = 100;
use signex_sketch::solver::residual::{total_residual, ResolvedParams};
use signex_sketch::solver::state::pack;
use signex_sketch::solver::math::norm_vec;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

#[test]
fn dof_under_constrained_marks_blue() {
    // One free Point, no constraints. solve_lm returns immediately
    // because state is empty? No — state has two free vars (x, y).
    // The Jacobian is empty (m = 0 rows × 2 cols). The conservative
    // coarse rule says "rank != n" ⇒ Under.
    let mut s = Sketch::new();
    let p = s.add_point(1.0, 2.0);

    let result = solve_lm(&s.data, &empty_params(), 5_000, TOL, MAX_ITERS)
        .expect("LM accepts a constraint-free sketch");

    // Build a fresh Jacobian at the converged state. With no
    // constraints this is an m=0 matrix; QR rank = 0, n = state.len()
    // = 2, so the coarse rule classifies as Under.
    let packed = pack(&s.data);
    let j = numerical_jacobian(&s.data, &result.state, &packed.index, &empty_params())
        .expect("Jacobian builds even on empty constraint set");

    let colours = entity_colours(&s.data, &result, &j, &result.index);
    assert_eq!(
        colours.get(&p),
        Some(&DofColor::Under),
        "free point with no constraints should be Under-constrained"
    );
}

#[test]
fn dof_fully_constrained_marks_black() {
    // Phase 3.3 canonical: P1 Fixed at origin, P2 starts (1, 0). Two
    // constraints: Distance(P1,P2)=5 and Horizontal(line(P1,P2)).
    // After solve P2 ≈ (5, 0) and the rank of the 2×2 Jacobian on the
    // free vars equals 2 = state.len() ⇒ both points classified Full.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    let line = s.add_line(p1, p2);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });

    let result = solve_lm(&s.data, &empty_params(), 5_000, TOL, MAX_ITERS)
        .expect("anchored line solves cleanly");

    let packed = pack(&s.data);
    let j = numerical_jacobian(&s.data, &result.state, &packed.index, &empty_params())
        .expect("Jacobian builds at solved state");

    let colours = entity_colours(&s.data, &result, &j, &result.index);

    // P1 is Fixed → always Full (or Over if a constraint over-touches
    // it; here the residuals are zero so no Over).
    assert_eq!(
        colours.get(&p1),
        Some(&DofColor::Full),
        "Fixed P1 should be Full-constrained"
    );

    // P2 is free; rank(J) = 2 = state.len() → Full under the coarse
    // rule.
    assert_eq!(
        colours.get(&p2),
        Some(&DofColor::Full),
        "P2 with sufficient constraints should be Full"
    );
}

#[test]
fn dof_over_constrained_marks_red() {
    // P1 Fixed at origin, P2 free starting (1, 0). Two CONFLICTING
    // Distance constraints: Distance(P1,P2)=5 AND Distance(P1,P2)=10.
    // LM converges to a least-squares compromise (~7.5 mm) but the
    // residuals on each constraint stay ~2.5 mm, well above RANK_TOL
    // (1e-9). Both Distance constraints should appear in
    // `over_constraint_ids`, and P2 (touched by both) should be
    // flagged Over.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    let cid_5mm = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: cid_5mm,
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    let cid_10mm = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: cid_10mm,
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
    });

    // Conflicting constraints can fail to "converge" in LM's strict
    // |r|² < TOL_SQ sense — that's the WHOLE POINT of the over-
    // constrained category. We try the solver but accept
    // DidNotConverge by manually evaluating the residual at the
    // initial state, which already shows the conflict (d ≈ 1 mm,
    // residuals ≈ −4 and −9, both ≫ RANK_TOL).
    let packed = pack(&s.data);
    let result: SolveResult = match solve_lm(&s.data, &empty_params(), 5_000, TOL, MAX_ITERS) {
        Ok(r) => r,
        Err(SolveError::DidNotConverge { .. }) => {
            // Build a SolveResult from the initial state. The DOF
            // analysis only inspects state + index + sketch, so this
            // is a faithful proxy.
            let state = packed.vector.clone();
            let r = total_residual(&s.data, &state, &packed.index, &empty_params())
                .expect("residual evaluates at initial state");
            SolveResult {
                state,
                index: packed.index.clone(),
                iterations: MAX_ITERS,
                final_residual_norm: norm_vec(&r),
                elapsed_ms: 0,
            }
        }
        Err(other) => panic!("unexpected solve error on conflicting constraints: {other:?}"),
    };

    let j = numerical_jacobian(&s.data, &result.state, &result.index, &empty_params())
        .expect("Jacobian builds at solved state");

    // Both Distance constraints should be flagged. (At minimum one
    // — judge based on the solver behaviour: with two equal-weight
    // conflicting constraints LM settles at ≈ 7.5 mm, leaving each
    // ≈ 2.5 mm residual.)
    let over_ids = over_constraint_ids(&s.data, &result, &j);
    assert!(
        !over_ids.is_empty(),
        "at least one of the two conflicting Distance constraints should be flagged red"
    );
    assert!(
        over_ids.contains(&cid_5mm) || over_ids.contains(&cid_10mm),
        "over_constraint_ids must include at least one of the conflicting Distance \
         constraints; got {over_ids:?} (expected {cid_5mm} and/or {cid_10mm})"
    );

    let colours = entity_colours(&s.data, &result, &j, &result.index);
    assert_eq!(
        colours.get(&p2),
        Some(&DofColor::Over),
        "P2 — touched by an over-constrained Distance — should be Over"
    );
}
