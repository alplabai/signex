//! Drawing placement in the SVG page model (and so in the PDF and preview).

use std::path::PathBuf;

use signex_types::schematic::{DrawingPlacement, Point, SchDrawing, SchematicSheet, Symbol, Wire};
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;

const WIRE_MM: f64 = 0.15;

fn line(n: u128, width: f64) -> SchDrawing {
    SchDrawing::Line {
        uuid: Uuid::from_u128(n),
        start: Point::new(10.0, 10.0 + n as f64),
        end: Point::new(50.0, 10.0 + n as f64),
        width,
        stroke_color: None,
    }
}

/// First point in mm (x, y) of every unfilled path, in emitted order.
fn path_origins(sheet: SchematicSheet) -> Vec<(i64, i64)> {
    let snapshot = SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: sheet,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions::default();
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let scale = 72.0 / 25.4;
    SvgRenderContext::from_sheet(&snapshot, &opts, w_mm, h_mm, scale, None)
        .elements
        .into_iter()
        .filter_map(|e| match e {
            SvgElement::Path { commands, style } if style.fill_rgb.is_none() => {
                match commands.first() {
                    Some(SvgPathCommand::MoveTo(p)) => Some((
                        (p.x as f64 / scale).round() as i64,
                        (p.y as f64 / scale).round() as i64,
                    )),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

#[test]
fn behind_drawings_paint_before_wires_and_above_drawings_after_symbols() {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    // File order: Above first, then Behind, to prove placement beats it.
    sheet.drawings = vec![line(1, 0.5), line(2, 0.5)];
    sheet
        .drawing_placements
        .insert(Uuid::from_u128(1), DrawingPlacement::Above);
    sheet.wires.push(Wire {
        uuid: Uuid::from_u128(9),
        start: Point::new(20.0, 20.0),
        end: Point::new(80.0, 20.0),
        stroke_width: WIRE_MM,
        ..Default::default()
    });
    // No library body: the exporter draws a 0.1 mm fallback box.
    sheet.symbols.push(Symbol::default());

    // Origins on the page (mm, margin included): the Behind drawing, the wire,
    // the symbol fallback box, the Above drawing.
    let seen = path_origins(sheet);
    let at = |origin: (i64, i64)| {
        seen.iter()
            .position(|o| *o == origin)
            .unwrap_or_else(|| panic!("no path at {origin:?} in {seen:?}"))
    };
    let (behind, wire, symbol, above) = (at((23, 25)), at((32, 32)), at((10, 10)), at((23, 24)));
    assert!(behind < wire, "behind drawing before wires: {seen:?}");
    assert!(wire < symbol, "{seen:?}");
    assert!(symbol < above, "above drawing after symbols: {seen:?}");
}
