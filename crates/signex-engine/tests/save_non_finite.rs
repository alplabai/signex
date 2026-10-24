//! #678 — a document holding a non-finite number must fail to save with an
//! error naming the block, row and field, and the file already on disk must
//! stay byte-for-byte intact. Before this the writer emitted `NaN` / `inf`
//! (or clamped a NaN coordinate to ~9.2e12 mm), which the reader then
//! refused, so a document could save and then fail to reopen.

use signex_engine::Engine;
use std::path::PathBuf;

use signex_types::schematic::{Junction, Point, SchematicSheet, Symbol, Wire};

/// A fresh scratch directory under the OS temp dir, unique per test.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("signex-678-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn save_error(sheet: SchematicSheet) -> (String, Vec<u8>) {
    let dir = scratch_dir(&format!("{:p}", &sheet));
    let path = dir.join("doc.snxsch");
    let original = b"previously saved good document\n".to_vec();
    std::fs::write(&path, &original).expect("seed file");

    let mut engine = Engine::new(sheet).expect("engine");
    let err = engine
        .save_as(&path)
        .expect_err("a non-finite value must fail the save");
    let on_disk = std::fs::read(&path).expect("file still readable");
    assert_eq!(on_disk, original, "the old file must stay intact");
    (err.to_string(), on_disk)
}

fn assert_names(message: &str, block: &str, row: usize, field: &str) {
    assert!(message.contains(block), "{message} lacks block {block}");
    assert!(
        message.contains(&format!("row {row}")),
        "{message} lacks row {row}"
    );
    assert!(message.contains(field), "{message} lacks field {field}");
}

#[test]
fn nan_coordinate_fails_the_save_and_names_the_field() {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire::default());
    sheet.wires.push(Wire {
        end: Point {
            x: 0.0,
            y: f64::NAN,
        },
        ..Wire::default()
    });
    let (message, _) = save_error(sheet);
    assert_names(&message, "sheets.wires", 1, "end_y");
}

#[test]
fn infinite_stroke_width_fails_the_save_and_names_the_field() {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        stroke_width: f64::INFINITY,
        ..Wire::default()
    });
    let (message, _) = save_error(sheet);
    assert_names(&message, "sheets.wires", 0, "stroke_width");
}

#[test]
fn infinite_junction_diameter_fails_the_save() {
    let mut sheet = SchematicSheet::default();
    sheet.junctions.push(Junction {
        diameter: f64::INFINITY,
        ..Junction::default()
    });
    let (message, _) = save_error(sheet);
    assert_names(&message, "sheets.junctions", 0, "diameter");
}

#[test]
fn nan_symbol_rotation_fails_the_save() {
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(Symbol {
        rotation: f64::NAN,
        ..Symbol::default()
    });
    let (message, _) = save_error(sheet);
    assert_names(&message, "sheets.components", 0, "rotation");
}

#[test]
fn a_finite_document_still_saves_and_reopens() {
    let dir = scratch_dir("finite");
    let path = dir.join("ok.snxsch");
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire::default());
    let mut engine = Engine::new(sheet).expect("engine");
    engine.save_as(&path).expect("finite document saves");
    Engine::open(&path).expect("and reopens");
}
