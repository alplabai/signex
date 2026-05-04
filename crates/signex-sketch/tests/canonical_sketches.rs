//! Task 3.4 — Canonical sketch corpus.
//!
//! Four hand-known sketches that exercise the Phase 3.3 LM solver
//! end-to-end. The fifth sketch from the Task 3.4 spec ("anchored
//! line, length-constrained") already lives in
//! `tests/lm_basic.rs::lm_solves_anchored_horizontal_distance` so we
//! cover the remaining four here:
//!
//!   1. Rectangle (10 × 5).
//!   2. Parallelogram (base 10, side 5, interior angle 60°).
//!   3. Isosceles triangle with 60° apex (i.e. equilateral, side 10).
//!   4. Regular hexagon (circumradius 10).
//!
//! Coordinate expectations are derived from elementary geometry — no
//! third-party constraint-solver source / format docs / blog posts /
//! wiki pages were consulted.
//!
//! Sign-convention note (`solver/residuals/parallel_perp_angle.rs`):
//! the `Angle` residual measures `atan2(cross(d1,d2), dot(d1,d2))`,
//! which is the signed CCW angle FROM `d1` TO `d2` wrapped into
//! `(−π, π]`. The chosen target signs below are derived by working
//! out the expected `d1`, `d2` direction vectors at the solution and
//! plugging into that formula.

mod common;
use common::Sketch;

use std::f64::consts::PI;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::solve_lm;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;

/// Tolerance for asserting solved coordinates against hand-computed
/// expected values.
const COORD_TOL: f64 = 1e-6;

/// Tolerance on `final_residual_norm` — same magnitude as `COORD_TOL`
/// because at the solution every residual should be sub-microscopic.
const RESIDUAL_TOL: f64 = 1e-6;

/// LM iteration cap used by these tests — matches the
/// `Solver::default().max_iters` of 100.
const ITER_CAP: usize = 100;
/// Default convergence tolerance — matches `Solver::default().tolerance`.
const TOL: f64 = 1e-12;

#[test]
fn rectangle_10_by_5() {
    // Topology:
    //   P4 ---- L3 ---- P3
    //   |                |
    //   L4              L2
    //   |                |
    //   P1 ---- L1 ---- P2
    //
    // Lines walk CCW from P1: L1 = P1→P2 (bottom), L2 = P2→P3
    // (right), L3 = P3→P4 (top), L4 = P4→P1 (left). Sharing
    // endpoints by construction makes Coincident constraints
    // unnecessary.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0); // initial guess — solver pulls to (10,0)
    let p3 = s.add_point(1.0, 1.0); // initial guess — solver pulls to (10,5)
    let p4 = s.add_point(0.0, 1.0); // initial guess — solver pulls to (0,5)
    let l1 = s.add_line(p1, p2);
    let l2 = s.add_line(p2, p3);
    let l3 = s.add_line(p3, p4);
    let l4 = s.add_line(p4, p1);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line: l1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line: l3 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Vertical { line: l2 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Vertical { line: l4 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2: p4,
            target: DimTarget::Literal(5.0),
        },
    });

    let result =
        solve_lm(&s.data, &ResolvedParams::new(), 5_000, TOL, ITER_CAP)
            .expect("rectangle should converge");

    let (x1, y1) = point_xy(p1, &result.state, &result.index, &s.data).unwrap();
    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    let (x3, y3) = point_xy(p3, &result.state, &result.index, &s.data).unwrap();
    let (x4, y4) = point_xy(p4, &result.state, &result.index, &s.data).unwrap();

    // P1 anchored at origin.
    assert!((x1 - 0.0).abs() < COORD_TOL, "P1.x = {x1}, expected 0.0");
    assert!((y1 - 0.0).abs() < COORD_TOL, "P1.y = {y1}, expected 0.0");

    // P2 = (10, 0) — bottom-right.
    assert!((x2 - 10.0).abs() < COORD_TOL, "P2.x = {x2}, expected 10.0");
    assert!((y2 - 0.0).abs() < COORD_TOL, "P2.y = {y2}, expected 0.0");

    // P3 = (10, 5) — top-right.
    assert!((x3 - 10.0).abs() < COORD_TOL, "P3.x = {x3}, expected 10.0");
    assert!((y3 - 5.0).abs() < COORD_TOL, "P3.y = {y3}, expected 5.0");

    // P4 = (0, 5) — top-left.
    assert!((x4 - 0.0).abs() < COORD_TOL, "P4.x = {x4}, expected 0.0");
    assert!((y4 - 5.0).abs() < COORD_TOL, "P4.y = {y4}, expected 5.0");

    assert!(
        result.final_residual_norm < RESIDUAL_TOL,
        "final residual norm = {}, expected < {RESIDUAL_TOL}",
        result.final_residual_norm
    );
    assert!(
        result.iterations <= ITER_CAP,
        "rectangle took {} iterations, expected <= {ITER_CAP}",
        result.iterations
    );
}

#[test]
fn parallelogram_base10_side5_60deg() {
    // Topology mirrors the rectangle — same 4-Point / 4-Line CCW
    // boundary — but constraints relax horizontal/vertical and
    // instead enforce parallelism + equal opposite sides + a single
    // interior angle.
    //
    // Expected solved geometry:
    //   P1 = (0, 0)                                 (Fixed)
    //   P2 = (10, 0)                                (10-mm base on +x)
    //   P4 = (5·cos 60°, 5·sin 60°) = (2.5, 4.330127…)
    //   P3 = P2 + (P4 − P1)         = (12.5, 4.330127…)
    //
    // Sign-convention derivation for the Angle target:
    //   At the solution, L1 direction = P2 − P1 = (10, 0), angle = 0.
    //   L4 direction = P1 − P4 = (−2.5, −4.330127…),
    //   angle = atan2(−4.330127, −2.5) ≈ −2.094395 rad = −2π/3.
    //   So the residual `atan2(cross, dot)` for `Angle(L1, L4)` equals
    //   −2π/3 at the solution — that's the literal target we feed in.
    //
    // Without a `Horizontal(L1)` orientation-pin the parallelogram is
    // free to rotate about the Fixed P1 — `Angle(L1, L4)` only fixes
    // L1 and L4 *relative* to each other. Pinning L1 horizontal locks
    // the world-frame orientation so the LM solver converges to the
    // expected coordinates instead of a rotated congruent shape.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0); // pulled to (10, 0)
    let p3 = s.add_point(2.0, 1.0); // pulled to (12.5, 4.330)
    let p4 = s.add_point(1.0, 1.0); // pulled to (2.5, 4.330)
    let l1 = s.add_line(p1, p2);
    let l2 = s.add_line(p2, p3);
    let l3 = s.add_line(p3, p4);
    let l4 = s.add_line(p4, p1);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line: l1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1, l2: l3 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1: l2, l2: l4 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualLength { l1, l2: l3 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualLength { l1: l2, l2: l4 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2: p4,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2: l4,
            target: DimTarget::Literal(-2.0 * PI / 3.0),
        },
    });

    let result = solve_lm(&s.data, &ResolvedParams::new(), 5_000, TOL, ITER_CAP)
        .expect("parallelogram should converge");

    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    let (x3, y3) = point_xy(p3, &result.state, &result.index, &s.data).unwrap();
    let (x4, y4) = point_xy(p4, &result.state, &result.index, &s.data).unwrap();

    let expected_x4 = 5.0 * (PI / 3.0).cos(); // 2.5
    let expected_y4 = 5.0 * (PI / 3.0).sin(); // 4.330127...
    let expected_x3 = 10.0 + expected_x4; // 12.5
    let expected_y3 = expected_y4; // 4.330127...

    assert!((x2 - 10.0).abs() < COORD_TOL, "P2.x = {x2}, expected 10.0");
    assert!((y2 - 0.0).abs() < COORD_TOL, "P2.y = {y2}, expected 0.0");
    assert!(
        (x3 - expected_x3).abs() < COORD_TOL,
        "P3.x = {x3}, expected {expected_x3}"
    );
    assert!(
        (y3 - expected_y3).abs() < COORD_TOL,
        "P3.y = {y3}, expected {expected_y3}"
    );
    assert!(
        (x4 - expected_x4).abs() < COORD_TOL,
        "P4.x = {x4}, expected {expected_x4}"
    );
    assert!(
        (y4 - expected_y4).abs() < COORD_TOL,
        "P4.y = {y4}, expected {expected_y4}"
    );

    assert!(
        result.final_residual_norm < RESIDUAL_TOL,
        "final residual norm = {}, expected < {RESIDUAL_TOL}",
        result.final_residual_norm
    );
    assert!(
        result.iterations <= ITER_CAP,
        "parallelogram took {} iterations, expected <= {ITER_CAP}",
        result.iterations
    );
}

#[test]
fn isosceles_triangle_apex_60() {
    // Three Points, three Lines:
    //   P1 = apex (Fixed at origin)
    //   P2, P3 = base endpoints
    //   L1 = P1 → P2  (apex side 1)
    //   L2 = P1 → P3  (apex side 2)
    //   L3 = P2 → P3  (base, Horizontal)
    //
    // With EqualLength(L1, L2), Angle(L1, L2) = 60°, |L1| = 10, and
    // Horizontal(L3), the triangle is equilateral with side 10.
    //
    // Initial guess places P2 just east of the apex and P3 below-and-
    // east of the apex; this nudges LM toward the "apex above the
    // base" branch, i.e. the base sits below the x-axis.
    //
    // Sign-convention derivation:
    //   At the solution, with the base below the x-axis, L1 dir
    //   points to (5, −5√3) at angle −π/3, and L2 dir points to
    //   (−5, −5√3) at angle −2π/3. CCW angle from L1 to L2 is
    //   −2π/3 − (−π/3) = −π/3. So the literal target is −π/3.
    //
    // Expected solved coords:
    //   P2 = (5, −5·√3/2·? ) — actually for side=10 equilateral with
    //   apex at origin, base horizontal, apex above base:
    //   |P2 − P1| = 10 with angle −π/3 → P2 = (10·cos(−π/3),
    //   10·sin(−π/3)) = (5, −5·√3) = (5, −8.660254…).
    //   |P3 − P1| = 10 with angle −2π/3 → P3 = (10·cos(−2π/3),
    //   10·sin(−2π/3)) = (−5, −8.660254…).
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0); // pulled to (5, −8.660…)
    let p3 = s.add_point(0.5, -1.0); // pulled to (−5, −8.660…)
    let l1 = s.add_line(p1, p2);
    let l2 = s.add_line(p1, p3);
    let l3 = s.add_line(p2, p3);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualLength { l1, l2 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(-PI / 3.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line: l3 },
    });

    let result =
        solve_lm(&s.data, &ResolvedParams::new(), 5_000, TOL, ITER_CAP)
            .expect("triangle should converge");

    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    let (x3, y3) = point_xy(p3, &result.state, &result.index, &s.data).unwrap();

    let half_height = 10.0 * (PI / 3.0).sin(); // 8.660254...
    let expected_x2 = 5.0;
    let expected_y2 = -half_height;
    let expected_x3 = -5.0;
    let expected_y3 = -half_height;

    assert!(
        (x2 - expected_x2).abs() < COORD_TOL,
        "P2.x = {x2}, expected {expected_x2}"
    );
    assert!(
        (y2 - expected_y2).abs() < COORD_TOL,
        "P2.y = {y2}, expected {expected_y2}"
    );
    assert!(
        (x3 - expected_x3).abs() < COORD_TOL,
        "P3.x = {x3}, expected {expected_x3}"
    );
    assert!(
        (y3 - expected_y3).abs() < COORD_TOL,
        "P3.y = {y3}, expected {expected_y3}"
    );

    // Sanity: side length |P2 − P3| = 10 (the base of an
    // equilateral triangle with side 10).
    let base_dx = x3 - x2;
    let base_dy = y3 - y2;
    let base_len = (base_dx * base_dx + base_dy * base_dy).sqrt();
    assert!(
        (base_len - 10.0).abs() < COORD_TOL,
        "base length = {base_len}, expected 10.0"
    );

    assert!(
        result.final_residual_norm < RESIDUAL_TOL,
        "final residual norm = {}, expected < {RESIDUAL_TOL}",
        result.final_residual_norm
    );
    assert!(
        result.iterations <= ITER_CAP,
        "triangle took {} iterations, expected <= {ITER_CAP}",
        result.iterations
    );
}

#[test]
fn regular_hexagon_circumradius_10() {
    // Six vertex Points + one Fixed centre Point + six "spoke" Lines
    // from centre to each vertex. Pinning the geometry:
    //   - 6× DistancePtPt(centre, P_i) = 10  (circumradius)
    //   - 5× Angle(spoke_i, spoke_{i+1}) = +π/3 (consecutive spokes
    //     CCW separated by 60°)
    //   - Horizontal(spoke_0) puts P_0 on the +x axis (the 6th spoke
    //     pair angle is implied by closure).
    //
    // Expected solved vertices on a 10-mm circle:
    //   P_i = (10·cos(i·π/3), 10·sin(i·π/3))   for i = 0..6.
    //
    // Sign-convention note: with vertices in CCW order on the unit
    // circle (P_0 at 0°, P_1 at 60°, …), the CCW angle from spoke_i
    // to spoke_{i+1} is +π/3. Verified by `atan2(cross, dot)`.
    let mut s = Sketch::new();
    let centre = s.add_point(0.0, 0.0);

    // Initial guess — vertices spaced 60° apart on a unit circle.
    // Solver pulls each onto the 10-mm circle.
    let mut vertices = Vec::with_capacity(6);
    let mut spokes = Vec::with_capacity(6);
    for i in 0..6 {
        let theta = (i as f64) * PI / 3.0;
        let id = s.add_point(theta.cos(), theta.sin());
        vertices.push(id);
    }
    for &v in &vertices {
        spokes.push(s.add_line(centre, v));
    }

    // Anchor: centre Fixed at origin.
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: centre },
    });

    // Six circumradius constraints.
    for &v in &vertices {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1: centre,
                p2: v,
                target: DimTarget::Literal(10.0),
            },
        });
    }

    // Five inter-spoke angle constraints — the sixth (spoke_5 →
    // spoke_0) is implied by closure of the other five plus the
    // orientation pin.
    for i in 0..5 {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Angle {
                l1: spokes[i],
                l2: spokes[i + 1],
                target: DimTarget::Literal(PI / 3.0),
            },
        });
    }

    // Orientation pin — spoke_0 is horizontal (P_0 on +x axis).
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line: spokes[0] },
    });

    let result =
        solve_lm(&s.data, &ResolvedParams::new(), 5_000, TOL, ITER_CAP)
            .expect("hexagon should converge");

    for (i, &v) in vertices.iter().enumerate() {
        let (x, y) = point_xy(v, &result.state, &result.index, &s.data).unwrap();
        let theta = (i as f64) * PI / 3.0;
        let ex = 10.0 * theta.cos();
        let ey = 10.0 * theta.sin();
        assert!(
            (x - ex).abs() < COORD_TOL,
            "P_{i}.x = {x}, expected {ex}"
        );
        assert!(
            (y - ey).abs() < COORD_TOL,
            "P_{i}.y = {y}, expected {ey}"
        );
    }

    assert!(
        result.final_residual_norm < RESIDUAL_TOL,
        "final residual norm = {}, expected < {RESIDUAL_TOL}",
        result.final_residual_norm
    );
    assert!(
        result.iterations <= ITER_CAP,
        "hexagon took {} iterations, expected <= {ITER_CAP}",
        result.iterations
    );
}
