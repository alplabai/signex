//! #768 item 2 — `Engine::serialize` on a sheet whose objects together carry
//! more foreign TSV columns than the cap must fail with an `io::Error` that
//! wraps the typed `FormatError`, not panic and not silently truncate the
//! columns. The app reads the offenders through `io::Error::get_ref`.

use signex_engine::{Engine, EngineError};
use signex_types::format::{FormatError, MAX_FOREIGN_COLUMNS};
use signex_types::schematic::{Junction, SchematicSheet};
use uuid::Uuid;

fn junction_with_columns(uuid: Uuid, range: std::ops::Range<usize>) -> Junction {
    let columns: Vec<(String, String)> = range.map(|i| (format!("c{i}"), "v".into())).collect();
    serde_json::from_value(serde_json::json!({
        "uuid": uuid,
        "position": { "x": 0.0, "y": 0.0 },
        "unknown_columns": columns,
    }))
    .expect("junction fixture")
}

/// Two junctions, each within the cap alone, together past it.
fn over_cap_engine(first: Uuid, second: Uuid) -> Engine {
    let half = MAX_FOREIGN_COLUMNS * 3 / 4;
    let sheet = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_string(),
        junctions: vec![
            junction_with_columns(first, 0..half),
            junction_with_columns(second, half..2 * half),
        ],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn typed(error: &EngineError) -> &FormatError {
    let EngineError::SaveFailed(io) = error else {
        panic!("expected SaveFailed, got {error:?}");
    };
    io.get_ref()
        .and_then(|inner| inner.downcast_ref::<FormatError>())
        .expect("the io error must wrap the typed FormatError")
}

#[test]
fn serialize_over_cap_returns_the_typed_error_in_an_io_error() {
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let engine = over_cap_engine(first, second);

    let error = engine
        .serialize()
        .expect_err("an over-cap sheet must not serialize");

    let FormatError::TsvTooManyForeignColumns {
        count,
        max,
        objects,
        ..
    } = typed(&error)
    else {
        panic!("expected TsvTooManyForeignColumns, got {error:?}");
    };
    assert_eq!(*max, MAX_FOREIGN_COLUMNS);
    assert_eq!(*count, MAX_FOREIGN_COLUMNS * 3 / 2);
    assert!(
        objects.iter().any(|o| o.contains(&second.to_string())),
        "the object holding the excess is named: {objects:?}"
    );
    assert!(
        !objects.iter().any(|o| o.contains(&first.to_string())),
        "an object entirely within the cap is not named: {objects:?}"
    );
}

#[test]
fn save_as_over_cap_writes_nothing() {
    let engine = over_cap_engine(Uuid::new_v4(), Uuid::new_v4());
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("Merged.snxsch");
    let mut engine = engine;

    assert!(engine.save_as(&path).is_err());
    assert!(!path.exists(), "a refused save must not create the file");
}
