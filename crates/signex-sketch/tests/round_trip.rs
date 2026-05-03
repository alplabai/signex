use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use uuid::Uuid;

#[test]
fn entity_id_round_trip() {
    let id = SketchEntityId(Uuid::new_v4());
    let s = serde_json::to_string(&id).unwrap();
    let back: SketchEntityId = serde_json::from_str(&s).unwrap();
    assert_eq!(id, back);
}

#[test]
fn constraint_id_round_trip() {
    let id = ConstraintId(Uuid::new_v4());
    let s = serde_json::to_string(&id).unwrap();
    let back: ConstraintId = serde_json::from_str(&s).unwrap();
    assert_eq!(id, back);
}

#[test]
fn plane_board_top_round_trip() {
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let s = toml::to_string(&p).unwrap();
    let back: Plane = toml::from_str(&s).unwrap();
    assert_eq!(p.kind, back.kind);
}

#[test]
fn plane_body_top_round_trip() {
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BodyTop {
            offset_z_expr: "= body_h".to_string(),
        },
    };
    let s = toml::to_string(&p).unwrap();
    let back: Plane = toml::from_str(&s).unwrap();
    assert_eq!(p.kind, back.kind);
}
