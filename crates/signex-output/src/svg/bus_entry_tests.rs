//! A bus entry is painted in its own palette colour, not the bus colour.
//! The PDF is built from the same `SvgRenderContext`, so this covers both.

use std::path::PathBuf;

use signex_types::schematic::{BusEntry, Point, SchematicSheet};
use signex_types::theme::{ThemeId, canvas_colors};
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;
use crate::pdf::palette::SchematicPalette;

fn css((r, g, b): (f32, f32, f32)) -> String {
    let u8_of = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgb({},{},{})", u8_of(r), u8_of(g), u8_of(b))
}

/// The page width of the one bus path stroked in `rgb`.
fn exported_bus_stroke(
    stroke_width: f64,
    color: Option<signex_types::theme::Color>,
    rgb: (f32, f32, f32),
) -> f32 {
    let mut schematic = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    schematic.buses.push(signex_types::schematic::Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(50.0, 50.0),
        end: Point::new(90.0, 50.0),
        stroke_width,
        color,
    });
    let sheet = SheetSnapshot {
        path: PathBuf::from("bus_style.snxsch"),
        schematic,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions::default();
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let page = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None);
    page.elements
        .iter()
        .find_map(|element| match element {
            SvgElement::Path { style, .. } if style.stroke_rgb == Some(rgb) => {
                Some(style.stroke_width)
            }
            _ => None,
        })
        .expect("a path in the bus colour")
}

/// #2813: the page strokes a bus in its own colour and width, so SVG and
/// PDF match the canvas. Compared with an unstyled bus on the same page
/// scale: 1.0 mm against the built-in 0.45 mm.
#[test]
fn a_bus_exports_with_its_own_colour_and_width() {
    let own = signex_types::theme::Color::new(0x20, 0x80, 0xC0, 0xFF);
    let styled = exported_bus_stroke(1.0, Some(own), wire_style::rgb(own));
    let unset = exported_bus_stroke(0.0, None, PdfOptions::default().palette.bus);
    assert!(
        (styled / unset - 1.0 / 0.45).abs() < 1e-3,
        "{styled} vs {unset}"
    );
}

#[test]
fn bus_entry_is_stroked_in_its_own_colour() {
    let colors = canvas_colors(ThemeId::GraphiteDark);
    let palette = SchematicPalette::from(&colors);
    assert_ne!(
        palette.bus, palette.bus_entry,
        "fixture needs distinct colours"
    );

    let mut schematic = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    schematic.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(50.0, 50.0),
        size: (2.54, -2.54),
    });
    let sheet = SheetSnapshot {
        path: PathBuf::from("bus_entry.snxsch"),
        schematic,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions {
        palette,
        ..PdfOptions::default()
    };
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let svg =
        SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None).svg_document();

    assert!(
        svg.contains(&css(palette.bus_entry)),
        "entry colour missing"
    );
    assert!(
        !svg.contains(&css(palette.bus)),
        "the entry must not be painted in the bus colour"
    );
}
