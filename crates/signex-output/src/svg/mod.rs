//! Schematic -> SVG path-based intermediate context.
//!
//! This module is the canonical geometry bridge:
//! schematic page -> SVG path elements -> PDF / preview backends.

use std::collections::HashMap;

use signex_types::markup::{ExpressionEvalContext, evaluate_expressions};
use signex_types::schematic::FillType;
use tiny_skia::Color;

use crate::pdf::ColourMode;

#[cfg(test)]
mod bus_entry_tests;
mod document;
mod drawings;
mod family;
mod geometry;
mod labels;
mod markers;
mod symbols;
mod text;
mod wire_style;

#[derive(Debug, Clone, Copy)]
pub enum SvgTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub enum SvgTextVAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct SvgPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub enum SvgPathCommand {
    MoveTo(SvgPoint),
    LineTo(SvgPoint),
    CubicTo(SvgPoint, SvgPoint, SvgPoint),
    Close,
}

#[derive(Debug, Clone, Copy)]
pub struct SvgStyle {
    pub stroke_rgb: Option<(f32, f32, f32)>,
    pub fill_rgb: Option<(f32, f32, f32)>,
    pub stroke_width: f32,
}

#[derive(Debug, Clone)]
pub enum SvgElement {
    Path {
        commands: Vec<SvgPathCommand>,
        style: SvgStyle,
    },
    Text {
        x: f32,
        y: f32,
        font_alias: &'static str,
        size_pt: f32,
        align: SvgTextAlign,
        v_align: SvgTextVAlign,
        rotation_deg: f32,
        fill_rgb: (f32, f32, f32),
        text: String,
    },
}

#[derive(Debug, Clone)]
pub struct SvgRenderContext {
    pub width: f32,
    pub height: f32,
    pub elements: Vec<SvgElement>,
    pub svg_document: String,
    /// The theme's paper colour, which Black & White maps against.
    pub paper: (f32, f32, f32),
}

pub struct SvgEvaluatorInputs<'a> {
    pub global_refdes: &'a HashMap<String, String>,
    pub net_name_by_symbol_pin: &'a HashMap<String, HashMap<String, String>>,
    pub cell: &'a str,
}

fn fill_to_rgb(
    fill: FillType,
    stroke: (f32, f32, f32),
    body_fill: (f32, f32, f32),
) -> Option<(f32, f32, f32)> {
    match fill {
        FillType::None => None,
        // Standard's "Outline" fill means "fill with the stroke
        // colour" — produces solid-shape glyphs like the anode
        // triangle of a diode. "Background" fills with the
        // theme's symbol body tint.
        FillType::Outline => Some(stroke),
        FillType::Background => Some(body_fill),
    }
}

fn normalize_standard_text(input: &str) -> String {
    normalize_standard_text_with_ctx(input, &ExpressionEvalContext::default())
}

fn normalize_standard_text_with_ctx(input: &str, ctx: &ExpressionEvalContext<'_>) -> String {
    // Standard-specific char-escape expansion (`{slash}` → `/`, etc.) was removed
    // in Phase 2.3 of the Apache-clean remediation. Inputs no longer carry
    // those tokens because the main repo no longer parses Standard files.
    evaluate_expressions(input, ctx)
}

fn rgb_to_color(r: f32, g: f32, b: f32) -> Color {
    Color::from_rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0)
        .unwrap_or(Color::BLACK)
}

/// Rec.601 luma — the one luminance the preview and the PDF both use.
pub(crate) fn luma((r, g, b): (f32, f32, f32)) -> f32 {
    0.299 * r + 0.587 * g + 0.114 * b
}

/// How far a colour's luma may sit from the paper's and still print as
/// paper in Black & White.
const BW_PAPER_TOLERANCE: f32 = 0.25;

/// Black & White is black ink on white paper whatever the theme (#1342).
/// A colour close to the theme's paper tone prints as paper (white) and
/// anything else is ink (black). Measuring against the paper rather than a
/// fixed mid-grey cut keeps a dark theme's page white and its light marks
/// solid.
pub(crate) fn map_black_and_white(rgb: (f32, f32, f32), paper: (f32, f32, f32)) -> (f32, f32, f32) {
    if (luma(rgb) - luma(paper)).abs() < BW_PAPER_TOLERANCE {
        (1.0, 1.0, 1.0)
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn map_colour_mode(
    rgb: (f32, f32, f32),
    mode: ColourMode,
    paper: (f32, f32, f32),
) -> (f32, f32, f32) {
    match mode {
        ColourMode::Colour => rgb,
        ColourMode::Grayscale => {
            let y = luma(rgb);
            (y, y, y)
        }
        ColourMode::BlackAndWhite => map_black_and_white(rgb, paper),
    }
}

fn pt(x: f32, y: f32) -> SvgPoint {
    SvgPoint { x, y }
}

#[cfg(test)]
mod net_class_tests;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::{Point, SchematicSheet, Wire};
    use uuid::Uuid;

    use super::*;
    use crate::SheetSnapshot;
    use crate::pdf::PdfOptions;

    fn empty_sheet_snapshot() -> SheetSnapshot {
        SheetSnapshot {
            path: PathBuf::from("test.standard_sch"),
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

    #[test]
    fn svg_document_is_emitted() {
        let mut sheet = empty_sheet_snapshot();
        sheet.schematic.wires.push(Wire {
            uuid: Uuid::new_v4(),
            end: Point::new(25.0, 20.0),
            stroke_width: 0.15,
            ..Default::default()
        });
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let svg = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None);
        assert!(svg.svg_document.starts_with("<svg"));
        assert!(svg.svg_document.contains("<path"));
        assert!(svg.svg_document.contains("</svg>"));
    }

    /// The note drawn by the exporter's rasterizer, which is where the face
    /// is chosen (the SVG text carries no family).
    fn pixels_of_note_in(family: Option<&str>) -> Vec<u8> {
        let mut sheet = empty_sheet_snapshot();
        sheet
            .schematic
            .text_notes
            .push(signex_types::schematic::TextNote {
                uuid: Uuid::from_u128(1),
                text: "Wiggle 0123".to_string(),
                position: Point::new(30.0, 30.0),
                rotation: 0.0,
                font_size: 2.0,
                font_family: family.map(str::to_string),
                justify_h: Default::default(),
                justify_v: Default::default(),
            });
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 96.0 / 25.4, None)
            .rasterize_rgba((w_mm * 96.0 / 25.4) as u32, (h_mm * 96.0 / 25.4) as u32)
            .expect("rasterized")
    }

    /// #510 - the export can only outline the bundled faces, so a family it
    /// cannot embed exports in Iosevka, the same face the canvas falls back
    /// to, and a bundled family exports in its own face.
    #[test]
    fn export_text_follows_a_bundled_family_and_falls_back_to_iosevka() {
        let unset = pixels_of_note_in(None);
        let iosevka = pixels_of_note_in(Some("Iosevka"));
        let unembeddable = pixels_of_note_in(Some("Fira Mono"));
        let roboto = pixels_of_note_in(Some("Roboto"));

        assert!(unembeddable == iosevka, "fallback is the bundled default");
        assert!(iosevka != unset, "an unset note keeps its existing face");
        assert!(
            roboto == unset,
            "notes were Roboto before they could name a family"
        );
    }

    #[test]
    fn can_rasterize_context() {
        let sheet = empty_sheet_snapshot();
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let svg = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 96.0 / 25.4, None);
        let rgba = svg.rasterize_rgba((w_mm * 96.0 / 25.4) as u32, (h_mm * 96.0 / 25.4) as u32);
        assert!(rgba.is_some());
    }

    fn texts(sheet: &SheetSnapshot) -> Vec<String> {
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        SvgRenderContext::from_sheet(sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None)
            .elements
            .into_iter()
            .filter_map(|e| match e {
                SvgElement::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    /// One file, two placements: each page copy prints the designator its
    /// placement resolves to, not the file's own.
    #[test]
    fn page_copies_print_their_placements_designator() {
        use signex_types::schematic::{InstancePath, Symbol, SymbolInstance, TextProp};
        let at = |n: u128| InstancePath::from_chain(vec![Uuid::from_u128(n)]);
        let entry = |n: u128, reference: &str| SymbolInstance {
            project: String::new(),
            path: at(n).to_string(),
            reference: reference.to_string(),
            unit: 1,
        };
        let mut base = empty_sheet_snapshot();
        base.schematic.symbols.push(Symbol {
            uuid: Uuid::from_u128(7),
            reference: "R1".to_string(),
            ref_text: Some(TextProp {
                position: Point::new(10.0, 10.0),
                rotation: 0.0,
                font_size: 1.27,
                justify_h: Default::default(),
                justify_v: Default::default(),
                hidden: false,
                font_family: None,
            }),
            instances: vec![entry(1, "R1"), entry(2, "R101")],
            ..Default::default()
        });
        let a = base.clone().for_placement(at(1), "A");
        let b = base.clone().for_placement(at(2), "B");
        assert!(texts(&a).contains(&"R1".to_string()), "{:?}", texts(&a));
        assert!(!texts(&a).contains(&"R101".to_string()));
        assert!(texts(&b).contains(&"R101".to_string()), "{:?}", texts(&b));
        assert!(!texts(&b).contains(&"R1".to_string()));
        assert_eq!(b.sheet_name, "Sheet1 (B)");
        assert_eq!(b.placement, at(2));
    }
}
