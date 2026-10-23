//! `Default` for the test-fixture types must equal what serde yields for a
//! manifest holding only the required keys.

use super::*;
use serde::de::DeserializeOwned;
use serde_json::json;

fn assert_default_matches<T>(minimal: serde_json::Value)
where
    T: Default + Serialize + DeserializeOwned,
{
    let from_serde: T = serde_json::from_value(minimal).expect("minimal manifest deserializes");
    assert_eq!(
        serde_json::to_value(T::default()).expect("default serializes"),
        serde_json::to_value(from_serde).expect("deserialized value serializes"),
    );
}

#[test]
fn sheet_default_equals_deserialized_empty_manifest() {
    assert_default_matches::<SchematicSheet>(json!({ "uuid": Uuid::nil() }));
}

#[test]
fn symbol_default_equals_deserialized_minimal_manifest() {
    assert_default_matches::<Symbol>(json!({
        "uuid": Uuid::nil(),
        "lib_id": "",
        "position": { "x": 0.0, "y": 0.0 },
    }));
}

#[test]
fn wire_default_equals_deserialized_minimal_manifest() {
    let origin = json!({ "x": 0.0, "y": 0.0 });
    assert_default_matches::<Wire>(json!({
        "uuid": Uuid::nil(), "start": origin, "end": origin,
    }));
}

#[test]
fn junction_default_equals_deserialized_minimal_manifest() {
    assert_default_matches::<Junction>(json!({
        "uuid": Uuid::nil(),
        "position": { "x": 0.0, "y": 0.0 },
    }));
}
