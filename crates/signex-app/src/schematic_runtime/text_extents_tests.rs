//! #2029 - a text whose face is missing is boxed with the width the file
//! stored; an installed face measures live, and a save refreshes the file's
//! widths without touching the document.

use super::*;
use signex_types::schematic::TextExtents;

const MISSING: &str = "No Such Face 9f2";
const STORED_MM: f64 = 9.0;

fn label(label_type: LabelType, family: Option<&str>) -> Label {
    Label {
        uuid: uuid::Uuid::now_v7(),
        text: "SYSTEM_RESET_REQUEST_N".into(),
        position: Point::new(20.0, 30.0),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        font_family: family.map(str::to_string),
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    }
}

/// A sheet holding `label` and a stored width of `STORED_MM` for it.
fn sheet_with_stored(label: Label) -> SchematicSheet {
    crate::fonts::measure::install_hermetic();
    let mut sheet = SchematicSheet {
        labels: vec![label],
        ..Default::default()
    };
    sheet.text_extents = TextExtents::default().refreshed(&sheet, |_| Some(STORED_MM));
    sheet
}

fn box_width(sheet: &SchematicSheet) -> f64 {
    let b = label_aabb_with(Some(WidthSources::of(sheet)), &sheet.labels[0], None);
    b.max_x - b.min_x
}

fn shaped_mm(label: &Label) -> f64 {
    let size = text_height_mm(label.font_size);
    signex_types::font::family_width_mm(&label.text, None, size)
}

#[test]
fn a_missing_face_is_boxed_with_the_stored_width() {
    let sheet = sheet_with_stored(label(LabelType::Net, Some(MISSING)));
    assert!((box_width(&sheet) - STORED_MM).abs() < 1e-6);
}

#[test]
fn a_record_measured_from_other_text_is_ignored() {
    let mut sheet = sheet_with_stored(label(LabelType::Net, Some(MISSING)));
    sheet.labels[0].text.push('X');
    let substitute = shaped_mm(&sheet.labels[0]);
    assert!((box_width(&sheet) - substitute).abs() < 1e-6);
}

#[test]
fn an_installed_face_measures_live_not_from_the_file() {
    let sheet = sheet_with_stored(label(LabelType::Net, None));
    assert!((box_width(&sheet) - shaped_mm(&sheet.labels[0])).abs() < 1e-6);
}

/// The live cache holds the substitute width a sheet without the record
/// measured; a sheet that has the record must still answer with it.
#[test]
fn the_live_cache_does_not_shadow_a_stored_width() {
    let mut sheet = sheet_with_stored(label(LabelType::Net, Some(MISSING)));
    let stored = std::mem::take(&mut sheet.text_extents);
    let substitute = box_width(&sheet);
    assert!(
        (substitute - STORED_MM).abs() > 1e-3,
        "setup: widths differ"
    );
    sheet.text_extents = stored;
    assert!((box_width(&sheet) - STORED_MM).abs() < 1e-6);
}

#[test]
fn a_flag_body_follows_the_stored_width_too() {
    let sheet = sheet_with_stored(label(LabelType::Global, Some(MISSING)));
    let flag = &sheet.labels[0];
    let vertices = label::marker_vertices_with(Some(WidthSources::of(&sheet)), flag, None);
    let em_mm = text_height_mm(flag.font_size) / f64::from(signex_types::font::mm_per_em(None));
    let want = STORED_MM + em_mm * 0.325 * 2.0;
    assert!((vertices[2].0 - flag.position.x - want).abs() < 1e-6);
}

fn engine_of(sheet: SchematicSheet) -> signex_engine::Engine {
    signex_engine::Engine::new(sheet).expect("engine")
}

fn saved(engine: &signex_engine::Engine) -> SchematicSheet {
    let text = engine
        .serialize_with_extents(&Default::default(), text_width::live_extent_measure())
        .expect("serialise");
    signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet
}

#[test]
fn a_save_writes_the_live_width_of_an_installed_face_and_leaves_the_document() {
    let mut sheet = sheet_with_stored(label(LabelType::Net, None));
    sheet.text_extents = TextExtents::default();
    let engine = engine_of(sheet);
    let revision = engine.revision();

    let reloaded = saved(&engine);
    let record = reloaded.text_extents.iter().next().expect("a record").1;
    let want = shaped_mm(&engine.document().labels[0]);
    assert!((record.width_mm() - want).abs() < 1e-6);
    assert!(
        engine.document().text_extents.is_empty(),
        "document untouched"
    );
    assert_eq!(engine.revision(), revision, "not an edit");
}

#[test]
fn a_save_keeps_the_loaded_width_of_a_face_that_is_not_installed() {
    let engine = engine_of(sheet_with_stored(label(LabelType::Net, Some(MISSING))));
    let reloaded = saved(&engine);
    let record = reloaded.text_extents.iter().next().expect("a record").1;
    assert_eq!(
        record.width_nm, 9_000_000,
        "not overwritten by the substitute"
    );
}
