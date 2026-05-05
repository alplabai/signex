mod common;
use common::Sketch;

use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::state::{circle_radius, pack, point_xy};

fn make_plane() -> Plane {
    Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    }
}

fn point(plane: PlaneId, id: SketchEntityId, x: f64, y: f64) -> Entity {
    Entity::new(id, plane, EntityKind::Point { x, y })
}

#[test]
fn pack_two_points_lays_out_xy_in_order() {
    let plane = make_plane();
    let p1_id = SketchEntityId::new();
    let p2_id = SketchEntityId::new();

    let mut sketch = SketchData::default();
    sketch.planes.push(plane.clone());
    sketch.entities.push(point(plane.id, p1_id, 1.0, 2.0));
    sketch.entities.push(point(plane.id, p2_id, 3.0, 4.0));

    let packed = pack(&sketch);
    assert_eq!(packed.vector, vec![1.0, 2.0, 3.0, 4.0]);

    let (x, y) = point_xy(p1_id, &packed.vector, &packed.index, &sketch).unwrap();
    assert!((x - 1.0).abs() < 1e-12);
    assert!((y - 2.0).abs() < 1e-12);
}

#[test]
fn pack_excludes_fixed_points() {
    use signex_sketch::constraint::{Constraint, ConstraintKind};
    use signex_sketch::id::ConstraintId;

    let plane = make_plane();
    let p_fixed = SketchEntityId::new();
    let p_free = SketchEntityId::new();

    let mut sketch = SketchData::default();
    sketch.planes.push(plane.clone());
    sketch.entities.push(point(plane.id, p_fixed, 5.0, 7.0));
    sketch.entities.push(point(plane.id, p_free, 1.0, 2.0));
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p_fixed },
    });

    let packed = pack(&sketch);
    assert_eq!(packed.vector, vec![1.0, 2.0]); // only the free point is packed
    assert!(packed.index.fixed.contains(&p_fixed));
    assert!(!packed.index.points.contains_key(&p_fixed));

    // The fixed point's coords are still readable through point_xy.
    let (x, y) = point_xy(p_fixed, &packed.vector, &packed.index, &sketch).unwrap();
    assert_eq!((x, y), (5.0, 7.0));
}

#[test]
fn pack_circle_radius_is_a_free_var() {
    let plane = make_plane();
    let center = SketchEntityId::new();
    let circle_id = SketchEntityId::new();

    let mut sketch = SketchData::default();
    sketch.planes.push(plane.clone());
    sketch.entities.push(point(plane.id, center, 0.0, 0.0));
    sketch.entities.push(Entity::new(
        circle_id,
        plane.id,
        EntityKind::Circle {
            center,
            radius: 1.5,
        },
    ));

    let packed = pack(&sketch);
    // Layout: [center.x, center.y, circle.r]
    assert_eq!(packed.vector, vec![0.0, 0.0, 1.5]);
    let r = circle_radius(circle_id, &packed.vector, &packed.index).unwrap();
    assert!((r - 1.5).abs() < 1e-12);
}

// ─── Task 3.1: numerical Jacobian via central differences ───────────────────
//
// The analytical Jacobian rows below are derived by hand from the per-
// constraint residuals in `crate::solver::residual::residual`:
//
// * `DistancePtPt` residual is r = sqrt((x2-x1)^2 + (y2-y1)^2) − target,
//   so dr/dx1 = -(x2-x1)/d, dr/dy1 = -(y2-y1)/d, etc.
// * `Coincident` residual is the 2-vector (x2-x1, y2-y1), so each row
//   has exactly one −1 and one +1 entry.
// * `Horizontal` residual is r = y2 − y1.
//
// State-vector layout for two free points p1, p2 is [x1, y1, x2, y2]
// in the order they were added via `Sketch::add_point`. With H = 1e-7
// and a relative tolerance of 1e-5 the central-difference truncation
// (O(h^2) ≈ 1e-14) plus double-precision roundoff (~ε/h ≈ 1.7e-9
// scaled by residual magnitude) sit well inside the bound.

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::jacobian::numerical_jacobian;
use signex_sketch::solver::residual::ResolvedParams;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

/// Compare two scalars with a mixed absolute/relative tolerance —
/// avoids spurious failures near zero where pure relative tolerance
/// is meaningless.
fn approx_eq(actual: f64, expected: f64, rel_tol: f64) {
    let abs_tol = 1e-9_f64.max(rel_tol * expected.abs());
    let diff = (actual - expected).abs();
    assert!(
        diff <= abs_tol,
        "expected {expected}, got {actual} (diff {diff}, tol {abs_tol})"
    );
}

#[test]
fn jacobian_distance_pt_pt_matches_analytical() {
    // r(x) = sqrt((x2-x1)^2 + (y2-y1)^2) − 5
    // At (0,0)−(3,4) the distance is 5 and the analytical row is
    //   [-(x2-x1)/d, -(y2-y1)/d, (x2-x1)/d, (y2-y1)/d]
    // = [-3/5, -4/5, 3/5, 4/5].
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    let packed = pack(&s.data);

    let j = numerical_jacobian(&s.data, &packed.vector, &packed.index, &empty_params())
        .expect("jacobian computes");
    assert_eq!(j.len(), 1, "DistancePtPt produces 1 residual row");
    assert_eq!(j[0].len(), 4, "two free points = 4 state coordinates");

    let expected = [-3.0 / 5.0, -4.0 / 5.0, 3.0 / 5.0, 4.0 / 5.0];
    for (k, want) in expected.iter().enumerate() {
        approx_eq(j[0][k], *want, 1e-5);
    }
}

#[test]
fn jacobian_coincident_matches_analytical() {
    // r(x) = (x2 - x1, y2 - y1).
    // Analytical Jacobian (constant):
    //   [[-1, 0, 1, 0],
    //    [ 0,-1, 0, 1]]
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(2.0, 5.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 },
    });
    let packed = pack(&s.data);

    let j = numerical_jacobian(&s.data, &packed.vector, &packed.index, &empty_params())
        .expect("jacobian computes");
    assert_eq!(j.len(), 2, "Coincident produces 2 residual rows");
    assert_eq!(j[0].len(), 4);

    let expected = [[-1.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 1.0]];
    for row in 0..2 {
        for col in 0..4 {
            approx_eq(j[row][col], expected[row][col], 1e-5);
        }
    }
}

#[test]
fn jacobian_horizontal_matches_analytical() {
    // r(x) = y2 − y1, so the row is [0, -1, 0, 1].
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 1.0);
    let b = s.add_point(5.0, 4.0); // diagonal — non-zero residual
    let line = s.add_line(a, b);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });
    let packed = pack(&s.data);

    let j = numerical_jacobian(&s.data, &packed.vector, &packed.index, &empty_params())
        .expect("jacobian computes");
    assert_eq!(j.len(), 1);
    assert_eq!(j[0].len(), 4);

    let expected = [0.0, -1.0, 0.0, 1.0];
    for (k, want) in expected.iter().enumerate() {
        approx_eq(j[0][k], *want, 1e-5);
    }
}

#[test]
fn jacobian_empty_sketch_is_zero_rows() {
    // No constraints → m = 0. n is just the number of free coordinates.
    let mut s = Sketch::new();
    let _p1 = s.add_point(1.0, 2.0);
    let _p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);
    assert_eq!(packed.vector.len(), 4);

    let j = numerical_jacobian(&s.data, &packed.vector, &packed.index, &empty_params())
        .expect("empty Jacobian computes");
    assert_eq!(j.len(), 0, "no constraints ⇒ zero rows");
    // Each row would have width n=4, but with 0 rows we cannot index.
}
