use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::state::{circle_radius, pack, point_xy};
use signex_sketch::SketchData;

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
