use signex_sketch::id::{ConstraintId, SketchEntityId};
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
