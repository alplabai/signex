//! #1116: placed images round-trip as `[[extras.sheet.images]]` rows, a
//! sheet without images writes nothing new, and a key a newer build adds to
//! a row is written back.

use uuid::Uuid;

use crate::format::SnxSchematic;
use crate::format::tests::empty_sheet;
use crate::schematic::{DrawingPlacement, Point, SchDrawing, SchImage, SchematicSheet};

const IMAGE: &str = "0192a8c0-0004-7000-8000-000000000001";

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn image() -> SchImage {
    let mut image = SchImage::new(
        Uuid::parse_str(IMAGE).unwrap(),
        Point::new(10.16, 20.32),
        (96.52, 46.97),
        "assets/images/power-path.svg".into(),
        "ab".repeat(32),
    );
    image.source_px = Some([1200, 584]);
    image
}

fn sheet_with_image() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.drawings.push(SchDrawing::Image(image()));
    sheet
}

#[test]
fn an_image_round_trips_as_a_row_and_a_second_save_is_identical() {
    let mut sheet = sheet_with_image();
    sheet
        .drawing_placements
        .insert(Uuid::parse_str(IMAGE).unwrap(), DrawingPlacement::Above);
    if let Some(SchDrawing::Image(image)) = sheet.drawings.first_mut() {
        image.locked = true;
        image.opacity = 60;
    }

    let text = save(&sheet);
    assert!(text.contains("[[extras.sheet.images]]"), "{text}");
    assert!(
        text.contains("file = \"assets/images/power-path.svg\""),
        "{text}"
    );
    assert!(!text.contains("[[extras.sheet.drawings]]"), "{text}");
    assert!(!text.contains("drawing_placements"), "{text}");
    assert!(!text.contains("locked_objects"), "{text}");
    let again = load(&text);
    assert_eq!(again.drawings, sheet.drawings);
    assert_eq!(again.drawing_placements, sheet.drawing_placements);
    assert_eq!(save(&again), text);
}

#[test]
fn an_image_needs_no_new_format_token() {
    let text = save(&sheet_with_image());
    assert!(text.contains("format = \"snxsch/1.1\""), "{text}");
}

#[test]
fn a_sheet_without_images_writes_no_image_rows() {
    let text = save(&empty_sheet());
    assert!(!text.contains("images"), "{text}");
    assert_eq!(save(&load(&text)), text);
}

#[test]
fn a_key_a_newer_build_adds_to_an_image_row_is_written_back() {
    let text = save(&sheet_with_image());
    let newer = text.replacen(
        "file = \"assets/images/power-path.svg\"",
        "file = \"assets/images/power-path.svg\"\ncrop = [1, 2, 3, 4]",
        1,
    );
    assert_ne!(newer, text);
    let again: toml::Value = toml::from_str(&save(&load(&newer))).unwrap();
    let crop: toml::Value = toml::from_str("v = [1, 2, 3, 4]").unwrap();
    assert_eq!(again["extras"]["sheet"]["images"][0]["crop"], crop["v"]);
}

#[test]
fn an_older_build_keeps_the_rows_as_an_unknown_key() {
    // What a build without image rows reads `[extras.sheet]` as: no typed
    // field, so `images` lands in its catch-all and is written back.
    #[derive(serde::Deserialize, serde::Serialize)]
    struct OlderSheetExtras {
        #[serde(flatten)]
        unknown: std::collections::BTreeMap<String, toml::Value>,
    }
    let text = save(&sheet_with_image());
    let doc: toml::Value = toml::from_str(&text).unwrap();
    let sheet = doc["extras"]["sheet"].clone();
    let older: OlderSheetExtras = sheet.clone().try_into().unwrap();
    assert!(older.unknown.contains_key("images"));
    assert_eq!(toml::Value::try_from(&older).unwrap(), sheet);
}
