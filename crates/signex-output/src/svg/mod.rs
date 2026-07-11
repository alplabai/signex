//! Schematic -> SVG path-based intermediate context.
//!
//! This module is the canonical geometry bridge:
//! schematic page -> SVG path elements -> PDF / preview backends.

use std::collections::HashMap;
use std::fmt::Write as _;

use signex_types::markup::{
    ExpressionEvalContext, RichSegment, evaluate_expressions, parse_signex_markup,
};
use signex_types::schematic::{FillType, LabelType};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::SheetSnapshot;
use crate::pdf::layout::PageTransform;
use crate::pdf::{ColourMode, PdfOptions};

mod drawings;
mod geometry;
mod labels;
mod symbols;

use drawings::push_sch_drawing_path;
use geometry::{path_to_tiny_skia, rect_path};
use labels::{
    halign_to_svg, label_colour, label_size_pt, label_spin_style, schematic_text_offset_global,
    schematic_text_offset_hier, schematic_text_offset_net, spin_text_style, valign_to_svg,
};
use symbols::{
    field_effective_style, push_symbol_lib_graphics, push_symbol_pins, symbol_eval_variables,
};

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

impl SvgRenderContext {
    pub fn from_sheet(
        sheet: &SheetSnapshot,
        opts: &PdfOptions,
        page_w_mm: f64,
        page_h_mm: f64,
        units_per_mm: f64,
        eval_inputs: Option<&SvgEvaluatorInputs<'_>>,
    ) -> Self {
        let width = (page_w_mm * units_per_mm) as f32;
        let height = (page_h_mm * units_per_mm) as f32;

        let xform = PageTransform::new(
            sheet,
            page_w_mm,
            page_h_mm,
            &opts.margins,
            &opts.scale,
            units_per_mm,
        );

        let mut elements = Vec::new();
        let palette = &opts.palette;

        // Page background — fill the whole MediaBox with the
        // schematic paper colour so the PDF page matches what the
        // user sees on the canvas. Light themes still hit white,
        // dark themes hit their paper tone instead of relying on
        // the reader's white default.
        elements.push(rect_path(
            0.0,
            0.0,
            width,
            height,
            SvgStyle {
                stroke_rgb: None,
                fill_rgb: Some(palette.paper),
                stroke_width: 0.0,
            },
        ));

        // Always render the physical sheet boundary on top of the
        // page-fill so both preview and PDF clearly reference the
        // document page area.
        elements.push(rect_path(
            0.5,
            0.5,
            (width - 1.0).max(1.0),
            (height - 1.0).max(1.0),
            SvgStyle {
                stroke_rgb: Some(palette.sheet_border),
                fill_rgb: None,
                stroke_width: 1.0,
            },
        ));

        // Base primitives
        for wire in &sheet.schematic.wires {
            let stroke_mm = if wire.stroke_width > 0.0 {
                wire.stroke_width
            } else {
                0.15
            };
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(wire.start.x), xform.px_y(wire.start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(wire.end.x), xform.px_y(wire.end.y))),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(palette.wire),
                    fill_rgb: None,
                    stroke_width: (stroke_mm * xform.mm_to_unit) as f32,
                },
            });
        }

        for bus in &sheet.schematic.buses {
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(bus.start.x), xform.px_y(bus.start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(bus.end.x), xform.px_y(bus.end.y))),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(palette.bus),
                    fill_rgb: None,
                    stroke_width: (0.3 * xform.mm_to_unit) as f32,
                },
            });
        }

        for entry in &sheet.schematic.bus_entries {
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(
                        xform.x(entry.position.x),
                        xform.px_y(entry.position.y),
                    )),
                    SvgPathCommand::LineTo(pt(
                        xform.x(entry.position.x + entry.size.0),
                        xform.px_y(entry.position.y + entry.size.1),
                    )),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(palette.bus_entry),
                    fill_rgb: None,
                    stroke_width: (0.2 * xform.mm_to_unit) as f32,
                },
            });
        }

        for j in &sheet.schematic.junctions {
            let dia_mm = if j.diameter > 0.0 { j.diameter } else { 0.7 };
            let side = (dia_mm * xform.mm_to_unit) as f32;
            let half = side / 2.0;
            let cx = xform.x(j.position.x);
            let cy = xform.px_y(j.position.y);
            elements.push(rect_path(
                cx - half,
                cy - half,
                side,
                side,
                SvgStyle {
                    stroke_rgb: Some(palette.junction),
                    fill_rgb: Some(palette.junction),
                    stroke_width: 0.8,
                },
            ));
        }

        // Standard's "no_connect" X markers map to Altium's "No-ERC
        // Markers" — render them only when the user kept the toggle
        // on. Altium's checklist hides these from the printed PDF
        // for cleaner deliverables.
        if opts.include_no_erc_markers {
            for nc in &sheet.schematic.no_connects {
                let arm = (1.0 * xform.mm_to_unit) as f32;
                let cx = xform.x(nc.position.x);
                let cy = xform.px_y(nc.position.y);
                let style = SvgStyle {
                    stroke_rgb: Some(palette.no_connect),
                    fill_rgb: None,
                    stroke_width: 0.8,
                };
                elements.push(SvgElement::Path {
                    commands: vec![
                        SvgPathCommand::MoveTo(pt(cx - arm, cy - arm)),
                        SvgPathCommand::LineTo(pt(cx + arm, cy + arm)),
                    ],
                    style,
                });
                elements.push(SvgElement::Path {
                    commands: vec![
                        SvgPathCommand::MoveTo(pt(cx - arm, cy + arm)),
                        SvgPathCommand::LineTo(pt(cx + arm, cy - arm)),
                    ],
                    style,
                });
            }
        }

        for label in &sheet.schematic.labels {
            let size_pt = label_size_pt(label.font_size, xform.mm_to_unit, &opts.scale);
            let spin = label_spin_style(label.justify, label.rotation);
            let (off_x, off_y) = match label.label_type {
                LabelType::Net => schematic_text_offset_net(spin),
                LabelType::Global => schematic_text_offset_global(&label.shape, spin),
                LabelType::Hierarchical => schematic_text_offset_hier(
                    &label.text,
                    signex_types::schematic::SCHEMATIC_TEXT_MM,
                    spin,
                ),
                LabelType::Power => (0.0, 0.0),
            };
            let (align, v_align, rot) = spin_text_style(spin);
            elements.push(SvgElement::Text {
                x: xform.x(label.position.x + off_x),
                y: xform.px_y(label.position.y + off_y),
                font_alias: "F3",
                size_pt,
                align,
                v_align,
                rotation_deg: rot,
                fill_rgb: label_colour(label.label_type, palette),
                text: normalize_standard_text(&label.text),
            });
        }

        // Free-floating text annotations = Altium "Notes". Hidden
        // from the export when the user unchecks the Notes toggle.
        if opts.include_notes {
            for note in &sheet.schematic.text_notes {
                let size_pt = label_size_pt(note.font_size, xform.mm_to_unit, &opts.scale);
                elements.push(SvgElement::Text {
                    x: xform.x(note.position.x),
                    y: xform.px_y(note.position.y),
                    font_alias: "F1",
                    size_pt,
                    align: halign_to_svg(note.justify_h),
                    v_align: valign_to_svg(note.justify_v),
                    rotation_deg: note.rotation as f32,
                    fill_rgb: palette.note_text,
                    text: normalize_standard_text(&note.text),
                });
            }
        }

        for child in &sheet.schematic.child_sheets {
            let x = xform.x(child.position.x);
            let y = xform.px_y(child.position.y);
            let w = (child.size.0 * xform.mm_to_unit) as f32;
            let h = (child.size.1 * xform.mm_to_unit) as f32;
            elements.push(rect_path(
                x,
                y,
                w,
                h,
                SvgStyle {
                    stroke_rgb: Some(palette.child_sheet_stroke),
                    fill_rgb: None,
                    stroke_width: ((if child.stroke_width > 0.0 {
                        child.stroke_width
                    } else {
                        0.2
                    }) * xform.mm_to_unit) as f32,
                },
            ));
            if !child.name.is_empty() {
                elements.push(SvgElement::Text {
                    x: x + 4.0,
                    y: y + 12.0,
                    font_alias: "F1",
                    size_pt: 8.5,
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: 0.0,
                    fill_rgb: palette.child_sheet_text,
                    text: normalize_standard_text(&child.name),
                });
            }
            if !child.filename.is_empty() {
                elements.push(SvgElement::Text {
                    x: x + 4.0,
                    y: y + 22.0,
                    font_alias: "F1",
                    size_pt: 7.5,
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: 0.0,
                    fill_rgb: palette.child_sheet_stroke,
                    text: normalize_standard_text(&child.filename),
                });
            }
        }

        for drawing in &sheet.schematic.drawings {
            push_sch_drawing_path(&mut elements, drawing, &xform, palette);
        }

        // Full symbol body graphics from library definitions.
        for sym in &sheet.schematic.symbols {
            let symbol_vars = symbol_eval_variables(sym);
            let mut refdes_vars = HashMap::new();
            if !sym.uuid.is_nil() && !sym.reference.is_empty() {
                refdes_vars.insert(sym.uuid.to_string(), sym.reference.clone());
            }
            let symbol_eval_ctx = ExpressionEvalContext {
                current_refdes: (!sym.reference.is_empty()).then_some(sym.reference.as_str()),
                current_value: (!sym.value.is_empty()).then_some(sym.value.as_str()),
                cell: eval_inputs.map(|e| e.cell),
                at_variables: Some(&symbol_vars),
                refdes_variables: eval_inputs.map(|e| e.global_refdes).or(Some(&refdes_vars)),
                ..ExpressionEvalContext::default()
            };

            let symbol_pin_nets =
                eval_inputs.and_then(|e| e.net_name_by_symbol_pin.get(&sym.uuid.to_string()));

            if let Some(lib) = sheet
                .schematic
                .lib_symbols
                .values()
                .find(|ls| ls.id == sym.lib_id)
            {
                push_symbol_lib_graphics(&mut elements, sym, lib, &xform, palette);
                push_symbol_pins(
                    &mut elements,
                    sym,
                    lib,
                    &xform,
                    &symbol_eval_ctx,
                    symbol_pin_nets,
                    palette,
                );
            } else {
                // Fallback if library is missing.
                elements.push(rect_path(
                    xform.x(sym.position.x - 5.0),
                    xform.px_y(sym.position.y - 5.0),
                    (10.0 * xform.mm_to_unit) as f32,
                    (10.0 * xform.mm_to_unit) as f32,
                    SvgStyle {
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: None,
                        stroke_width: (0.1 * xform.mm_to_unit) as f32,
                    },
                ));
            }

            // Match canvas renderer parity:
            // - power-symbol references (#PWR...) are always hidden
            // - reference/value field visibility follows TextProp.hidden
            if !sym.reference.is_empty()
                && !sym.is_power
                && let Some(ref_text) = &sym.ref_text
                && !ref_text.hidden
            {
                elements.push(SvgElement::Text {
                    x: xform.x(ref_text.position.x),
                    y: xform.px_y(ref_text.position.y),
                    font_alias: "F3",
                    size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                    align: halign_to_svg(field_effective_style(ref_text, sym).1),
                    v_align: valign_to_svg(field_effective_style(ref_text, sym).2),
                    rotation_deg: field_effective_style(ref_text, sym).0 as f32,
                    fill_rgb: palette.reference,
                    text: normalize_standard_text_with_ctx(&sym.reference, &symbol_eval_ctx),
                });
            }

            if !sym.value.is_empty()
                && let Some(val_text) = &sym.val_text
                && !val_text.hidden
            {
                elements.push(SvgElement::Text {
                    x: xform.x(val_text.position.x),
                    y: xform.px_y(val_text.position.y),
                    font_alias: "F1",
                    size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                    align: halign_to_svg(field_effective_style(val_text, sym).1),
                    v_align: valign_to_svg(field_effective_style(val_text, sym).2),
                    rotation_deg: field_effective_style(val_text, sym).0 as f32,
                    fill_rgb: palette.value,
                    text: normalize_standard_text_with_ctx(&sym.value, &symbol_eval_ctx),
                });
            }
        }

        let svg_document = encode_svg_document(width, height, &elements);

        Self {
            width,
            height,
            elements,
            svg_document,
        }
    }

    pub fn rasterize_rgba(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        self.rasterize_rgba_with_colour_mode(width, height, ColourMode::Colour)
    }

    pub fn rasterize_rgba_with_colour_mode(
        &self,
        width: u32,
        height: u32,
        colour_mode: ColourMode,
    ) -> Option<Vec<u8>> {
        let mut pixmap = Pixmap::new(width, height)?;
        pixmap.fill(Color::WHITE);

        for element in &self.elements {
            match element {
                SvgElement::Path { commands, style } => {
                    if let Some(path) = path_to_tiny_skia(commands) {
                        if let Some((r, g, b)) = style.fill_rgb {
                            let mut paint = Paint::default();
                            let (mr, mg, mb) = map_colour_mode((r, g, b), colour_mode);
                            paint.set_color(rgb_to_color(mr, mg, mb));
                            pixmap.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                Default::default(),
                                None,
                            );
                        }

                        if let Some((r, g, b)) = style.stroke_rgb {
                            let mut paint = Paint::default();
                            let (mr, mg, mb) = map_colour_mode((r, g, b), colour_mode);
                            paint.set_color(rgb_to_color(mr, mg, mb));
                            let stroke = Stroke {
                                width: style.stroke_width.max(0.5),
                                ..Stroke::default()
                            };
                            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
                        }
                    }
                }
                SvgElement::Text {
                    x,
                    y,
                    align,
                    v_align,
                    rotation_deg,
                    size_pt,
                    fill_rgb,
                    text,
                    font_alias,
                } => {
                    let mapped_fill = map_colour_mode(*fill_rgb, colour_mode);
                    draw_text_outline(
                        &mut pixmap,
                        *x,
                        *y,
                        *size_pt,
                        *align,
                        *v_align,
                        *rotation_deg,
                        mapped_fill,
                        font_alias,
                        text,
                    );
                }
            }
        }

        Some(pixmap.data().to_vec())
    }
}

fn encode_svg_document(width: f32, height: f32, elements: &[SvgElement]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );

    for element in elements {
        match element {
            SvgElement::Path { commands, style } => {
                let d = to_svg_path_d(commands);
                let stroke = style
                    .stroke_rgb
                    .map(rgb_css)
                    .unwrap_or_else(|| "none".to_string());
                let fill = style
                    .fill_rgb
                    .map(rgb_css)
                    .unwrap_or_else(|| "none".to_string());
                let _ = writeln!(
                    out,
                    "  <path d=\"{}\" stroke=\"{}\" fill=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"miter\" stroke-linecap=\"square\" />",
                    d, stroke, fill, style.stroke_width
                );
            }
            SvgElement::Text {
                x,
                y,
                size_pt,
                align,
                v_align,
                rotation_deg,
                fill_rgb,
                text,
                ..
            } => {
                let anchor = match align {
                    SvgTextAlign::Left => "start",
                    SvgTextAlign::Center => "middle",
                    SvgTextAlign::Right => "end",
                };
                let baseline = match v_align {
                    SvgTextVAlign::Top => "hanging",
                    SvgTextVAlign::Center => "middle",
                    SvgTextVAlign::Bottom => "alphabetic",
                };
                let transform = if rotation_deg.abs() > 0.001 {
                    format!(" transform=\"rotate({} {} {})\"", rotation_deg, x, y)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "  <text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"{baseline}\"{transform} fill=\"{}\" font-size=\"{size_pt}\">{}</text>",
                    rgb_css(*fill_rgb),
                    escape_xml(text)
                );
            }
        }
    }

    out.push_str("</svg>\n");
    out
}

fn to_svg_path_d(commands: &[SvgPathCommand]) -> String {
    let mut out = String::new();
    for cmd in commands {
        match cmd {
            SvgPathCommand::MoveTo(p) => {
                let _ = write!(out, "M {} {} ", p.x, p.y);
            }
            SvgPathCommand::LineTo(p) => {
                let _ = write!(out, "L {} {} ", p.x, p.y);
            }
            SvgPathCommand::CubicTo(c1, c2, p) => {
                let _ = write!(
                    out,
                    "C {} {}, {} {}, {} {} ",
                    c1.x, c1.y, c2.x, c2.y, p.x, p.y
                );
            }
            SvgPathCommand::Close => {
                out.push_str("Z ");
            }
        }
    }
    out.trim().to_string()
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

fn rgb_css((r, g, b): (f32, f32, f32)) -> String {
    let to_u8 = |v: f32| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
    format!("rgb({},{},{})", to_u8(r), to_u8(g), to_u8(b))
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

fn draw_text_outline(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    size_pt: f32,
    align: SvgTextAlign,
    v_align: SvgTextVAlign,
    rotation_deg: f32,
    fill_rgb: (f32, f32, f32),
    font_alias: &str,
    text: &str,
) {
    let Some(face) = face_for_alias(font_alias) else {
        return;
    };
    let units_per_em = face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return;
    }
    let runs = markup_runs(text);
    let base_size = size_pt.max(1.0);
    let advance = measure_text_advance_runs(&face, &runs, base_size, units_per_em);

    let start_x = match align {
        SvgTextAlign::Left => x,
        SvgTextAlign::Center => x - advance * 0.5,
        SvgTextAlign::Right => x - advance,
    };

    let base_scale = base_size / units_per_em;
    let asc = face.ascender() as f32 * base_scale;
    let desc = face.descender() as f32 * base_scale;
    let baseline_y = match v_align {
        SvgTextVAlign::Top => y + asc,
        SvgTextVAlign::Center => y + (asc + desc) * 0.5,
        SvgTextVAlign::Bottom => y + desc,
    };

    let mut pen_x = start_x;
    let mut paint = Paint::default();
    paint.set_color(rgb_to_color(fill_rgb.0, fill_rgb.1, fill_rgb.2));

    for run in &runs {
        let run_start = pen_x;
        let run_size = base_size * run.scale;
        let run_scale = run_size / units_per_em;
        let run_baseline = baseline_y + base_size * run.baseline_offset;

        for ch in run.text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }

            if let Some(gid) = face.glyph_index(ch) {
                let mut builder =
                    TinyPathOutlineBuilder::new(pen_x, run_baseline, run_scale, x, y, rotation_deg);
                if face.outline_glyph(gid, &mut builder).is_some()
                    && let Some(path) = builder.finish()
                {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, Default::default(), None);
                }
                pen_x += glyph_advance(&face, gid, run_scale);
            } else {
                pen_x += run_size * 0.5;
            }
        }

        if run.overbar && pen_x > run_start {
            let overbar_y = run_baseline - run_size * 0.78;
            let (x1, y1) = rotate_about(run_start, overbar_y, x, y, rotation_deg);
            let (x2, y2) = rotate_about(pen_x, overbar_y, x, y, rotation_deg);
            let mut pb = PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            if let Some(path) = pb.finish() {
                let stroke = Stroke {
                    width: (run_size * 0.08).max(0.5),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
            }
        }
    }
}

fn measure_text_advance_runs(
    face: &Face<'_>,
    runs: &[MarkupRun],
    base_size: f32,
    units_per_em: f32,
) -> f32 {
    let mut advance = 0.0_f32;
    for run in runs {
        let run_scale = (base_size * run.scale) / units_per_em;
        for ch in run.text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            if let Some(gid) = face.glyph_index(ch) {
                advance += glyph_advance(face, gid, run_scale);
            } else {
                advance += 0.5 * run_scale * face.units_per_em() as f32;
            }
        }
    }
    advance
}

#[derive(Clone)]
struct MarkupRun {
    text: String,
    scale: f32,
    baseline_offset: f32,
    overbar: bool,
}

fn markup_runs(input: &str) -> Vec<MarkupRun> {
    let expanded = normalize_standard_text(input);
    let segments = parse_signex_markup(&expanded);
    if segments.is_empty() {
        return vec![MarkupRun {
            text: expanded,
            scale: 1.0,
            baseline_offset: 0.0,
            overbar: false,
        }];
    }

    segments
        .into_iter()
        .map(|seg| match seg {
            RichSegment::Normal(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
            // TODO(v0.x): visual decoration for bold/italic/strike
            RichSegment::Bold(t) | RichSegment::Italic(t) | RichSegment::Strike(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
            RichSegment::Overbar(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: true,
            },
            RichSegment::Subscript(t) => MarkupRun {
                text: t,
                scale: 0.72,
                baseline_offset: 0.26,
                overbar: false,
            },
            RichSegment::Superscript(t) => MarkupRun {
                text: t,
                scale: 0.72,
                baseline_offset: -0.34,
                overbar: false,
            },
            // Links render as plain label text on the canvas — URL is ignored
            // until link rendering ships in a later phase.
            RichSegment::Link { label, .. } => MarkupRun {
                text: label,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
        })
        .filter(|run| !run.text.is_empty())
        .collect()
}

fn rotate_about(px: f32, py: f32, ox: f32, oy: f32, rotation_deg: f32) -> (f32, f32) {
    let rad = rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let dx = px - ox;
    let dy = py - oy;
    (ox + dx * cos - dy * sin, oy + dx * sin + dy * cos)
}

fn glyph_advance(face: &Face<'_>, gid: GlyphId, scale: f32) -> f32 {
    face.glyph_hor_advance(gid)
        .map(|v| v as f32 * scale)
        .unwrap_or(0.5 * face.units_per_em() as f32 * scale)
}

fn face_for_alias(alias: &str) -> Option<Face<'static>> {
    match alias {
        "F1" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .ok(),
        "F2" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Bold.ttf"),
            0,
        )
        .ok(),
        "F3" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf"),
            0,
        )
        .ok(),
        "F4" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Bold.ttf"),
            0,
        )
        .ok(),
        _ => None,
    }
}

struct TinyPathOutlineBuilder {
    pb: PathBuilder,
    pen_x: f32,
    baseline_y: f32,
    scale: f32,
    anchor_x: f32,
    anchor_y: f32,
    rot_cos: f32,
    rot_sin: f32,
}

impl TinyPathOutlineBuilder {
    fn new(
        pen_x: f32,
        baseline_y: f32,
        scale: f32,
        anchor_x: f32,
        anchor_y: f32,
        rotation_deg: f32,
    ) -> Self {
        let rad = rotation_deg.to_radians();
        Self {
            pb: PathBuilder::new(),
            pen_x,
            baseline_y,
            scale,
            anchor_x,
            anchor_y,
            rot_cos: rad.cos(),
            rot_sin: rad.sin(),
        }
    }

    fn finish(self) -> Option<tiny_skia::Path> {
        self.pb.finish()
    }

    fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        let px = self.pen_x + x * self.scale;
        let py = self.baseline_y - y * self.scale;
        let dx = px - self.anchor_x;
        let dy = py - self.anchor_y;
        let rx = self.anchor_x + dx * self.rot_cos - dy * self.rot_sin;
        let ry = self.anchor_y + dx * self.rot_sin + dy * self.rot_cos;
        (rx, ry)
    }
}

impl OutlineBuilder for TinyPathOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (mx, my) = self.map_point(x, y);
        self.pb.move_to(mx, my);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (lx, ly) = self.map_point(x, y);
        self.pb.line_to(lx, ly);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (cx, cy) = self.map_point(x1, y1);
        let (px, py) = self.map_point(x, y);
        self.pb.quad_to(cx, cy, px, py);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.map_point(x1, y1);
        let (c2x, c2y) = self.map_point(x2, y2);
        let (px, py) = self.map_point(x, y);
        self.pb.cubic_to(c1x, c1y, c2x, c2y, px, py);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
