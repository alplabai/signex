//! Net-class styling in the SVG page model (and so in the PDF and preview).

use std::path::PathBuf;

use signex_types::net_class::{NetClassStyle, NetLineStyle};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use signex_types::theme::Color;
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;

const WIRE_ID: Uuid = Uuid::from_u128(5);

fn sheet_with_wire() -> SheetSnapshot {
    let mut sheet = SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        },
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    sheet.schematic.wires.push(Wire {
        uuid: WIRE_ID,
        start: Point::new(20.0, 20.0),
        end: Point::new(80.0, 20.0),
        stroke_width: 0.15,
        ..Default::default()
    });
    sheet
}

/// The path element of the first wire (the page fill and border come first).
fn wire_path(sheet: &SheetSnapshot, opts: &PdfOptions) -> (Vec<SvgPathCommand>, SvgStyle) {
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let ctx = SvgRenderContext::from_sheet(sheet, opts, w_mm, h_mm, 72.0 / 25.4, None);
    ctx.elements
        .into_iter()
        .filter_map(|element| match element {
            SvgElement::Path { commands, style } if style.fill_rgb.is_none() => {
                Some((commands, style))
            }
            _ => None,
        })
        .nth(1)
        .expect("the wire path, after the sheet border")
}

fn options_with(style: NetClassStyle) -> PdfOptions {
    let mut opts = PdfOptions::default();
    opts.net_class_styles.insert(WIRE_ID, style);
    opts
}

#[test]
fn without_classes_a_wire_is_one_solid_palette_coloured_segment() {
    let opts = PdfOptions::default();

    let (commands, style) = wire_path(&sheet_with_wire(), &opts);

    assert_eq!(commands.len(), 2);
    assert_eq!(style.stroke_rgb, Some(opts.palette.wire));
}

#[test]
fn a_dashed_class_splits_the_wire_into_marks() {
    let opts = options_with(NetClassStyle {
        color: None,
        wire_width_mm: None,
        line: NetLineStyle::Dashed,
    });

    let (commands, style) = wire_path(&sheet_with_wire(), &opts);

    assert!(commands.len() > 2, "several MoveTo/LineTo pairs");
    assert_eq!(commands.len() % 2, 0);
    assert_eq!(style.stroke_rgb, Some(opts.palette.wire));
}

#[test]
fn class_colour_and_width_reach_the_stroke() {
    let opts = options_with(NetClassStyle {
        color: Some(Color::new(255, 0, 0, 255)),
        wire_width_mm: Some(0.5),
        line: NetLineStyle::Solid,
    });

    let (commands, style) = wire_path(&sheet_with_wire(), &opts);

    assert_eq!(commands.len(), 2);
    assert_eq!(style.stroke_rgb, Some((1.0, 0.0, 0.0)));
    // The page scale is the same for both, so the widths keep the mm ratio.
    let (_, plain) = wire_path(&sheet_with_wire(), &PdfOptions::default());
    let expected = plain.stroke_width * (0.5 / 0.15) as f32;
    assert!((style.stroke_width - expected).abs() < 1e-3);
}

#[test]
fn per_net_colour_override_wins_over_the_class_colour_in_export() {
    let opts = options_with(NetClassStyle {
        color: Some(Color::new(255, 0, 0, 255)),
        wire_width_mm: None,
        line: NetLineStyle::Dotted,
    });
    let mut sheet = sheet_with_wire();
    sheet
        .schematic
        .net_colors
        .insert(WIRE_ID, Color::new(0, 255, 0, 255));

    let (commands, style) = wire_path(&sheet, &opts);

    assert_eq!(style.stroke_rgb, Some((0.0, 1.0, 0.0)));
    assert!(commands.len() > 2, "the class line style still applies");
}
