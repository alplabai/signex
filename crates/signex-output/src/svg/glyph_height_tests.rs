//! #1229 — a text's stored size is the height of its capitals, in every
//! family, measured on the exporter's own rasterized glyph outlines.

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;
use signex_types::schematic::{Point, SchematicSheet};
use std::path::PathBuf;
use uuid::Uuid;

const LABEL_MM: f64 = 1.27;
const PX_PER_MM: f64 = 24.0;

/// Height of the rows inked by a note reading `HEH` on an otherwise empty page, in mm.
fn capital_height_mm(family: &str) -> f64 {
    let mut sheet = SheetSnapshot {
        path: PathBuf::from("t.snxsch"),
        schematic: SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        },
        sheet_name: "S".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    sheet
        .schematic
        .text_notes
        .push(signex_types::schematic::TextNote {
            uuid: Uuid::from_u128(1),
            text: "HEH".to_string(),
            position: Point::new(30.0, 30.0),
            rotation: 0.0,
            font_size: LABEL_MM,
            font_family: Some(family.to_string()),
            justify_h: Default::default(),
            justify_v: Default::default(),
            locked: false,
        });
    let opts = PdfOptions {
        scale: crate::pdf::PdfScale::OneToOne,
        include_title_block: false,
        ..PdfOptions::default()
    };
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let w = (w_mm * PX_PER_MM) as u32;
    let h = (h_mm * PX_PER_MM) as u32;
    let rgba = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, PX_PER_MM, None)
        .rasterize_rgba(w, h)
        .expect("rasterized");
    // The page border is 1 unit wide; the note is the only other ink.
    let (x0, x1, y0, y1) = (4, w - 4, 4, h - 4);
    let inked_rows: Vec<u32> = (y0..y1)
        .filter(|&y| {
            (x0..x1).any(|x| {
                let at = ((y * w + x) * 4) as usize;
                rgba[at] < 200 && rgba[at + 3] > 128
            })
        })
        .collect();
    let (top, bottom) = (
        *inked_rows.first().expect("ink"),
        *inked_rows.last().unwrap(),
    );
    f64::from(bottom - top + 1) / PX_PER_MM
}

#[test]
fn capitals_of_a_label_measure_its_size_in_roboto_and_iosevka() {
    for family in ["Roboto", "Iosevka"] {
        let cap = capital_height_mm(family);
        assert!(
            (cap - LABEL_MM).abs() < LABEL_MM * 0.04,
            "{family}: capitals measure {cap} mm, want {LABEL_MM}"
        );
    }
}
