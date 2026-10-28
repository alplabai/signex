//! #104 — the engine opens a newer-minor `.snxsch` for viewing but refuses
//! `save` / `save_as` (even to a new path); a current file still saves with
//! the current format token.

use signex_engine::Engine;
use signex_types::format::{SNXSCH_FORMAT_V1, SnxSchematic};
use signex_types::schematic::SchematicSheet;

fn current_text() -> String {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    SnxSchematic::new(sheet).write_string().expect("serialise")
}

#[test]
fn newer_minor_opens_but_save_and_save_as_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("newer.snxsch");
    let newer = current_text().replacen(
        &format!("format = \"{SNXSCH_FORMAT_V1}\""),
        "format = \"snxsch/1.7\"",
        1,
    );
    std::fs::write(&path, &newer).expect("write");

    let mut engine = Engine::open(&path).expect("newer minor loads");
    assert!(engine.save().is_err());
    let other = dir.path().join("copy.snxsch");
    let err = engine.save_as(&other).expect_err("save_as is also lossy");
    assert!(err.to_string().contains("snxsch/1.7"), "{err}");
    assert!(!other.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
}

#[test]
fn current_file_saves_with_the_current_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cur.snxsch");
    std::fs::write(&path, current_text()).expect("write");
    let mut engine = Engine::open(&path).expect("open");
    engine.save().expect("save");
    let out = std::fs::read_to_string(&path).unwrap();
    assert!(
        out.starts_with(&format!("format = \"{SNXSCH_FORMAT_V1}\"")),
        "{out}"
    );
}
