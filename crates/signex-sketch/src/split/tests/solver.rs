//! End-to-end solver acceptance for `split_line` (issue #360 blocker 3):
//! a split followed by a re-solve must converge, and the duplicated
//! `Horizontal` / `Vertical` carry-over must pull a perturbed mid
//! Point back onto the line without disturbing untouched geometry.

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::plane::PlaneId;
use crate::split::*;

/// Perturbed axis-aligned rectangle — `p1` Fixed at the origin, width
/// 10 mm, height 5 mm — plus the constraint set that pins it there:
/// Horizontal on the bottom/top edges, Vertical on the left/right
/// edges, and one `DistancePtPt` per axis. 6 residuals over 6 free
/// scalars (`p1` is Fixed and excluded from the state vector) — the
/// system is exactly determined, so it converges to a unique solution
/// near the perturbed initial guess rather than trivially no-op-ing.
/// Returns `(sketch, p1, p2, p3, p4, l1)`; `l1` (the bottom edge,
/// `p1 -> p2`) is the one the test below splits.
fn perturbed_rectangle() -> (
    SketchData,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(crate::plane::Plane {
        id: plane,
        kind: crate::plane::PlaneKind::BoardTop,
    });

    let pts = [(0.0, 0.0), (9.8, 0.15), (9.85, 4.8), (0.2, 5.15)];
    let ids: Vec<SketchEntityId> = pts
        .iter()
        .map(|&(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        })
        .collect();
    let (p1, p2, p3, p4) = (ids[0], ids[1], ids[2], ids[3]);

    let edges = [(p1, p2), (p2, p3), (p3, p4), (p4, p1)];
    let line_ids: Vec<SketchEntityId> = edges
        .iter()
        .map(|&(start, end)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Line { start, end }));
            id
        })
        .collect();
    let (l1, l2, l3, l4) = (line_ids[0], line_ids[1], line_ids[2], line_ids[3]);

    use ConstraintKind::*;
    let kinds = [
        Fixed { point: p1 },
        Horizontal { line: l1 },
        Vertical { line: l2 },
        Horizontal { line: l3 },
        Vertical { line: l4 },
        DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
        DistancePtPt {
            p1,
            p2: p4,
            target: DimTarget::Literal(5.0),
        },
    ];
    for kind in kinds {
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind,
        });
    }

    (sketch, p1, p2, p3, p4, l1)
}

#[test]
fn split_then_solve_leaves_rectangle_visually_unchanged() {
    use crate::solver::Solver;
    use crate::solver::residual::ResolvedParams;
    use crate::solver::state::point_xy;

    let (mut sketch, p1, p2, p3, p4, l1) = perturbed_rectangle();
    let solver = Solver::default();
    let params = ResolvedParams::new();
    const TOL: f64 = 1e-6;

    // Baseline solve, and commit the solved positions back onto the
    // entities — split_line reads raw entity coordinates, not solver
    // state, so a caller must commit before splitting (mirrors what
    // the app does after a solve).
    let solved = solver
        .solve(&sketch, &params)
        .expect("baseline solve must converge");
    let expect_xy = [
        (p1, (0.0, 0.0)),
        (p2, (10.0, 0.0)),
        (p3, (10.0, 5.0)),
        (p4, (0.0, 5.0)),
    ];
    for (id, (ex, ey)) in expect_xy {
        let (x, y) = point_xy(id, &solved.result.state, &solved.result.index, &sketch).unwrap();
        assert!(
            (x - ex).abs() < TOL && (y - ey).abs() < TOL,
            "baseline solve did not converge {id} to ({ex},{ey}), got ({x},{y})"
        );
        sketch
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .kind = EntityKind::Point { x, y };
    }

    // Split the bottom edge off-centre, then knock the new mid point's
    // AND p2's y off-line — exactly the scenario the duplicated
    // Horizontal pair exists to correct on the next solve. Perturbing
    // p2 specifically exercises `line_b`'s own copy: p2 is only tied
    // to `mid.y` through it (p2's other two constraints, Vertical and
    // DistancePtPt, don't pin p2.y on their own), so a carry-over that
    // silently dropped or misdirected `line_b`'s copy would leave p2
    // free to drift instead of settling back onto the line.
    let result = split_line(&mut sketch, l1, 0.3).expect("split of a solved edge must succeed");
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == result.mid_point)
        .unwrap()
        .kind = EntityKind::Point { x: 3.0, y: 0.2 };
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == p2)
        .unwrap()
        .kind = EntityKind::Point { x: 10.0, y: 0.15 };

    let resolved = solver.solve(&sketch, &params).expect(
        "post-split solve must converge — a broken carry-over would over- or \
                 under-constrain this and fail here",
    );
    let solved_xy =
        |id| point_xy(id, &resolved.result.state, &resolved.result.index, &sketch).unwrap();

    // The three untouched corners are visually unchanged.
    for (id, (ex, ey)) in expect_xy {
        let (x, y) = solved_xy(id);
        assert!(
            (x - ex).abs() < TOL && (y - ey).abs() < TOL,
            "post-split corner {id} moved to ({x},{y}), expected ({ex},{ey})"
        );
    }
    // The duplicated Horizontal pair pulls the perturbed mid point
    // back onto the line (y -> 0)...
    let (mx, my) = solved_xy(result.mid_point);
    assert!(my.abs() < TOL, "mid.y = {my}, should have settled to 0");
    // ...without moving its x: nothing constrains mid.x, so it stays
    // exactly where the split placed it.
    assert!(
        (mx - 3.0).abs() < TOL,
        "mx drifted to {mx}, expected 3.0 (unconstrained)"
    );
}
