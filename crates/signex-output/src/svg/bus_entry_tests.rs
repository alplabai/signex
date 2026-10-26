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

#[test]
fn bus_entry_is_stroked_in_its_own_colour() {
    let colors = canvas_colors(ThemeId::CatppuccinMocha);
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
        uuid: Uuid::new_v4(),
        position: Point::new(50.0, 50.0),
        size: (2.54, -2.54),
    });
    let sheet = SheetSnapshot {
        path: PathBuf::from("bus_entry.snxsch"),
        schematic,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
    };
    let opts = PdfOptions {
        palette,
        ..PdfOptions::default()
    };
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let svg =
        SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None).svg_document;

    assert!(
        svg.contains(&css(palette.bus_entry)),
        "entry colour missing"
    );
    assert!(
        !svg.contains(&css(palette.bus)),
        "the entry must not be painted in the bus colour"
    );
}
