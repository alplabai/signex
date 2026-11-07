//! #1863 — the bytes `save_with_extents` returns are exactly the bytes it
//! wrote, so a caller fingerprints them instead of re-reading the file.

use signex_engine::Engine;
use signex_types::schematic::SchematicSheet;

#[test]
fn save_with_extents_returns_the_bytes_it_wrote() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("doc.snxsch");
    let mut engine = Engine::new(SchematicSheet::default()).expect("engine");
    engine.set_path(Some(path.clone()));

    let written = engine.save_with_extents(|_| None).expect("save");

    assert_eq!(written, std::fs::read(&path).expect("read back"));
}
