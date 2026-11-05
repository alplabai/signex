//! The page model draws power ports in the power colour and every stored
//! stroke by the one width rule (#2091).

use std::path::PathBuf;

use signex_types::schematic::{
    ChildSheet, Point, SCHEMATIC_DEFAULT_WIRE_STROKE_MM, SchematicSheet, Symbol, TextProp,
};
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;

const MM_TO_UNIT: f32 = 72.0 / 25.4;

fn sheet() -> SheetSnapshot {
    SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        },
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    }
}

fn elements_of(sheet: &SheetSnapshot, opts: &PdfOptions) -> Vec<SvgElement> {
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    SvgRenderContext::from_sheet(sheet, opts, w_mm, h_mm, MM_TO_UNIT, None).elements
}

fn port(is_power: bool) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(11),
        lib_id: "power:GND".to_string(),
        reference: "#PWR01".to_string(),
        value: "GND".to_string(),
        is_power,
        position: Point::new(50.0, 50.0),
        val_text: Some(TextProp {
            position: Point::new(50.0, 56.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: Default::default(),
            justify_v: Default::default(),
            hidden: false,
        }),
        ..Default::default()
    }
}

/// The strokes and the "GND" text the symbol adds to the page.
fn symbol_marks(is_power: bool, opts: &PdfOptions) -> (Vec<SvgStyle>, Vec<(f32, f32, f32)>) {
    let mut with = sheet();
    with.schematic.symbols.push(port(is_power));
    let base = elements_of(&sheet(), opts).len();
    let elements = elements_of(&with, opts);
    let mut styles = Vec::new();
    let mut texts = Vec::new();
    for element in elements.into_iter().skip(base) {
        match element {
            SvgElement::Path { style, .. } => styles.push(style),
            SvgElement::Text { fill_rgb, text, .. } if text == "GND" => texts.push(fill_rgb),
            SvgElement::Text { .. } => {}
        }
    }
    (styles, texts)
}

#[test]
fn a_power_port_strokes_and_labels_in_the_power_colour() {
    let opts = PdfOptions::default();
    let power = opts.palette.power_label;
    let (styles, texts) = symbol_marks(true, &opts);
    assert!(!styles.is_empty());
    assert!(styles.iter().all(|s| s.stroke_rgb == Some(power)));
    assert_eq!(texts, vec![power], "the Value text");
}

#[test]
fn a_power_port_strokes_at_the_default_width_like_a_stock_symbol() {
    let opts = PdfOptions::default();
    let (styles, _) = symbol_marks(true, &opts);
    let expected = SCHEMATIC_DEFAULT_WIRE_STROKE_MM as f32 * MM_TO_UNIT;
    assert!(
        styles
            .iter()
            .all(|s| (s.stroke_width - expected).abs() < 1e-4)
    );
}

#[test]
fn a_normal_part_keeps_the_body_and_value_colours() {
    let opts = PdfOptions::default();
    let (styles, texts) = symbol_marks(false, &opts);
    // A part that is not a power port with this lib id has no built-in body,
    // so the Value text is the only mark to check.
    assert!(
        styles
            .iter()
            .all(|s| s.stroke_rgb != Some(opts.palette.power_label))
    );
    assert_eq!(texts, vec![opts.palette.value]);
}

fn child(stroke_width: f64) -> ChildSheet {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(12),
        "name": "Child",
        "filename": "child.snxsch",
        "position": { "x": 20.0, "y": 20.0 },
        "size": [40.0, 30.0],
        "stroke_width": stroke_width,
    }))
    .expect("child sheet")
}

fn frame_width(stroke_width: f64) -> f32 {
    let opts = PdfOptions::default();
    let mut sheet = sheet();
    sheet.schematic.child_sheets.push(child(stroke_width));
    elements_of(&sheet, &opts)
        .into_iter()
        .find_map(|e| match e {
            SvgElement::Path { style, .. }
                if style.stroke_rgb == Some(opts.palette.child_sheet_stroke) =>
            {
                Some(style.stroke_width)
            }
            _ => None,
        })
        .expect("the child sheet frame")
}

/// A stored 0 is 0.254 mm and an explicit width is floored at 0.15 mm, the
/// same rule the canvas applies to the frame.
#[test]
fn a_child_sheet_frame_follows_the_one_stroke_rule() {
    let unit = |mm: f64| mm as f32 * MM_TO_UNIT;
    assert!((frame_width(0.0) - unit(0.254)).abs() < 1e-4);
    assert!((frame_width(0.5) - unit(0.5)).abs() < 1e-4);
    assert!((frame_width(0.05) - unit(0.15)).abs() < 1e-4);
}
