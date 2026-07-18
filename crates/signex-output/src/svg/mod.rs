//! Schematic -> SVG path-based intermediate context.
//!
//! This module is the canonical geometry bridge:
//! schematic page -> SVG path elements -> PDF / preview backends.

use std::collections::HashMap;

use signex_types::markup::{ExpressionEvalContext, evaluate_expressions};
use signex_types::schematic::FillType;
use tiny_skia::Color;

use crate::pdf::ColourMode;

mod document;
mod drawings;
mod geometry;
mod labels;
mod symbols;
mod text;

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

fn map_colour_mode(rgb: (f32, f32, f32), mode: ColourMode) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    match mode {
        ColourMode::Colour => (r, g, b),
        ColourMode::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
        ColourMode::BlackAndWhite => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            if y >= 0.5 {
                (1.0, 1.0, 1.0)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
    }
}

fn pt(x: f32, y: f32) -> SvgPoint {
    SvgPoint { x, y }
}

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
                uuid: Uuid::nil(),
                version: 0,
                generator: String::new(),
                generator_version: String::new(),
                paper_size: "A4".to_string(),
                root_sheet_page: "1".to_string(),
                symbols: vec![],
                wires: vec![],
                junctions: vec![],
                labels: vec![],
                child_sheets: vec![],
                no_connects: vec![],
                text_notes: vec![],
                buses: vec![],
                bus_entries: vec![],
                drawings: vec![],
                no_erc_directives: vec![],
                title_block: Default::default(),
                lib_symbols: Default::default(),
            },
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        }
    }

    #[test]
    fn svg_document_is_emitted() {
        let mut sheet = empty_sheet_snapshot();
        sheet.schematic.wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(25.0, 20.0),
            stroke_width: 0.15,
        });
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let svg = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None);
        assert!(svg.svg_document.starts_with("<svg"));
        assert!(svg.svg_document.contains("<path"));
        assert!(svg.svg_document.contains("</svg>"));
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
}
