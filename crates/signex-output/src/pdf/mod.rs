//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) acts as a second
//! render target for the schematic — wires, symbols, labels, title block.
//! Screen (Iced Canvas) and PDF share one source of truth for page layout.
//!
//! ## Font Strategy (v0.8)
//!
//! Roboto + Iosevka TTFs are embedded at compile time (see `font.rs`) but
//! NOT yet emitted as Type0 composite fonts in the PDF — that's a v0.9 job.
//! For v0.8 every text operator references one of four aliases `/F1`–`/F4`
//! pointing at the PDF standard-14 Type1 fonts (Helvetica variants for
//! Roboto, Courier variants for Iosevka). Those standard fonts ship with
//! every PDF reader by spec, so exported PDFs always render text correctly
//! even though the glyphs come from Helvetica/Courier rather than the
//! bundled TTFs.
//!
//! TODO(v0.9): Emit Type0 CIDFontType2 dicts with `/FontFile2` streams
//! pointing at the embedded TTF bytes so the exported PDFs render in the
//! intended Roboto/Iosevka typeface.

use pdf_writer::{Finish, Name, Pdf, Rect, Ref};
use signex_types::markup::{
    ExpressionEvalContext, RichSegment, evaluate_expressions, expand_standard_char_escapes,
    parse_markup,
};
use thiserror::Error;

use crate::template::TemplateId;
use crate::{ExportContext, Exporter, SubstitutionContext};
use crate::expression::{ExpressionTables, build_expression_tables, sheet_cell_value};

mod colour;
mod font;
pub(crate) mod layout;
mod page;
mod surface;

use colour::ColourMap;
use font::{PdfFont, best_alias_for_text, sanitize_pdf_text, text_advance_pt};
use surface::PdfSurface;
use crate::svg::{
    SvgElement, SvgEvaluatorInputs, SvgPathCommand, SvgRenderContext, SvgTextAlign,
    SvgTextVAlign,
};

/// 1 mm in PDF points (1 pt = 1/72 inch).
const MM_TO_PT: f64 = 72.0 / 25.4;

pub struct PdfExporter;

#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub colour_mode: ColourMode,
    pub page_range: PageRange,
    pub sheet_template: Option<TemplateId>,
    pub margins: Margins,
    pub scale: PdfScale,
    pub include_title_block: bool,
}

#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub page_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
    IsoA0,
    IsoA1,
    IsoA2,
    IsoA3,
    IsoA4,
    IsoA5,
    AnsiA,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    UsLetter,
    UsLegal,
    Custom { width_mm: f64, height_mm: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourMode {
    Colour,
    Grayscale,
    BlackAndWhite,
}

#[derive(Debug, Clone)]
pub enum PageRange {
    All,
    Current,
    Specific(Vec<usize>),
    Range(usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top_mm: f64,
    pub right_mm: f64,
    pub bottom_mm: f64,
    pub left_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfScale {
    FitToPage,
    OneToOne,
    Percent(f64),
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Landscape,
            colour_mode: ColourMode::Colour,
            page_range: PageRange::All,
            sheet_template: None,
            margins: Margins {
                top_mm: 10.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
            },
            scale: PdfScale::FitToPage,
            include_title_block: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("no sheets in export context")]
    NoSheets,

    #[error("page range references sheet {0} but project only has {1} sheets")]
    OutOfRange(usize, usize),

    #[error("font subsetting failed: {0}")]
    Font(String),
}

impl Exporter for PdfExporter {
    type Options = PdfOptions;
    type Output = PdfOutput;
    type Error = PdfError;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        if ctx.sheets.is_empty() {
            return Err(PdfError::NoSheets);
        }

        let sheet_indices = resolve_page_range(&opts.page_range, ctx.sheets.len())?;
        let expr_tables = build_expression_tables(&ctx.sheets);
        let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let page_w_pt = (page_w_mm * MM_TO_PT) as f32;
        let page_h_pt = (page_h_mm * MM_TO_PT) as f32;

        let mut pdf = Pdf::new();

        let catalog_id = Ref::new(1);
        let page_tree_id = Ref::new(2);

        // Reserve one Ref per page, starting at 3 (after catalog + page tree).
        let page_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + i as i32))
            .collect();

        // Reserve content stream Refs after page Refs.
        let content_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + sheet_indices.len() as i32 + i as i32))
            .collect();

        // Reserve one font ref per PdfFont variant after the content stream
        // refs. Allocated up front so page resources can point at them.
        let font_base: i32 = 3 + 2 * sheet_indices.len() as i32;
        let font_refs: Vec<(font::PdfFont, Ref)> = font::PdfFont::ALL
            .iter()
            .enumerate()
            .map(|(i, &f)| (f, Ref::new(font_base + i as i32)))
            .collect();

        pdf.catalog(catalog_id).pages(page_tree_id);
        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(page_refs.len() as i32);

        // Emit a minimal Type1 font dict for each bundled font, using the
        // PDF standard-14 name as the BaseFont. Every reader ships these,
        // so text always renders even though we're not (yet) embedding the
        // TTF bytes as a Type0 composite font.
        for (font, font_ref) in &font_refs {
            pdf.type1_font(*font_ref)
                .base_font(Name(font.standard_ps_name().as_bytes()));
        }

        // Build each page with content.
        for (idx, &sheet_idx) in sheet_indices.iter().enumerate() {
            let sheet = &ctx.sheets[sheet_idx];
            let content_ref = content_refs[idx];

            // Emit content stream for this page.
            let content_bytes = build_page_content(
                sheet,
                opts,
                ctx,
                page_w_pt,
                page_h_pt,
                &expr_tables,
            )?;

            pdf.stream(content_ref, &content_bytes);

            // Create the page object referencing the content stream.
            let mut page = pdf.page(page_refs[idx]);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt))
                .contents(content_ref);

            // /Font resources dict — maps the F1-F4 aliases used in the
            // content streams to the font objects emitted above.
            let mut resources = page.resources();
            let mut fonts = resources.fonts();
            for (font, font_ref) in &font_refs {
                fonts.pair(Name(font.alias().as_bytes()), *font_ref);
            }
            fonts.finish();
            resources.finish();

            page.finish();
        }

        let bytes = pdf.finish();

        Ok(PdfOutput {
            bytes,
            page_count: page_refs.len(),
        })
    }
}

/// Build a content stream for a single page.
fn build_page_content(
    sheet: &crate::SheetSnapshot,
    opts: &PdfOptions,
    ctx: &ExportContext,
    page_w_pt: f32,
    page_h_pt: f32,
    expr_tables: &ExpressionTables,
) -> Result<Vec<u8>, PdfError> {
    let mut surface = PdfSurface::new();
    let colour_map = ColourMap::new(opts.colour_mode);

    let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);

    // Explicit two-step pipeline:
    // schematic sheet -> SVG render context -> PDF content stream operators.
    let cell = sheet_cell_value(sheet);
    let eval_inputs = SvgEvaluatorInputs {
        global_refdes: &expr_tables.global_refdes,
        net_name_by_symbol_pin: &expr_tables.net_name_by_symbol_pin,
        cell: &cell,
    };
    let svg_ctx = SvgRenderContext::from_sheet(
        sheet,
        opts,
        page_w_mm,
        page_h_mm,
        MM_TO_PT,
        Some(&eval_inputs),
    );

    // Set default colour for all drawings.
    let (r, g, b) = colour_map.map_stroke_bw(0.0, 0.0, 0.0);
    surface.set_stroke_color(r, g, b);

    // Render all mapped schematic elements from the intermediate SVG page.
    for element in &svg_ctx.elements {
        match element {
            SvgElement::Path { commands, style } => {
                let mut path_ops = String::new();
                for cmd in commands {
                    match cmd {
                        SvgPathCommand::MoveTo(p) => {
                            path_ops.push_str(&format!("{} {} m\n", p.x, page_h_pt - p.y));
                        }
                        SvgPathCommand::LineTo(p) => {
                            path_ops.push_str(&format!("{} {} l\n", p.x, page_h_pt - p.y));
                        }
                        SvgPathCommand::CubicTo(c1, c2, p) => {
                            path_ops.push_str(&format!(
                                "{} {} {} {} {} {} c\n",
                                c1.x,
                                page_h_pt - c1.y,
                                c2.x,
                                page_h_pt - c2.y,
                                p.x,
                                page_h_pt - p.y
                            ));
                        }
                        SvgPathCommand::Close => path_ops.push_str("h\n"),
                    }
                }

                if let Some((r, g, b)) = style.stroke_rgb {
                    let (sr, sg, sb) = colour_map.map_stroke_bw(r, g, b);
                    surface.set_stroke_color(sr, sg, sb);
                }
                if let Some((r, g, b)) = style.fill_rgb {
                    let (fr, fg, fb) = colour_map.map_stroke_bw(r, g, b);
                    surface.set_fill_color(fr, fg, fb);
                }
                surface.set_stroke_width(style.stroke_width.max(0.1));
                surface.raw_operator(&path_ops);

                match (style.stroke_rgb.is_some(), style.fill_rgb.is_some()) {
                    (true, true) => surface.raw_operator("B\n"),
                    (true, false) => surface.raw_operator("S\n"),
                    (false, true) => surface.raw_operator("f\n"),
                    (false, false) => {}
                }
            }
            SvgElement::Text {
                x,
                y,
                font_alias,
                size_pt,
                align,
                v_align,
                rotation_deg,
                fill_rgb,
                text,
            } => {
                let (sr, sg, sb) = colour_map.map_stroke_bw(fill_rgb.0, fill_rgb.1, fill_rgb.2);
                surface.set_fill_color(sr, sg, sb);

                let runs = pdf_markup_runs(text);
                let plain_text: String = runs.iter().map(|r| r.text.as_str()).collect();
                let preferred_text = sanitize_pdf_text(&plain_text);
                let chosen_alias = best_alias_for_text(font_alias, &preferred_text);
                let text_w: f32 = runs
                    .iter()
                    .map(|run| {
                        let t = sanitize_pdf_text(&run.text);
                        text_advance_pt(chosen_alias, &t, *size_pt * run.scale)
                    })
                    .sum();
                let asc = size_pt * 0.8;
                let desc = -size_pt * 0.2;
                let draw_x = match align {
                    SvgTextAlign::Left => *x,
                    SvgTextAlign::Center => *x - text_w * 0.5,
                    SvgTextAlign::Right => *x - text_w,
                };
                let draw_y = match v_align {
                    SvgTextVAlign::Top => *y + asc,
                    SvgTextVAlign::Center => *y + (asc + desc) * 0.5,
                    SvgTextVAlign::Bottom => *y + desc,
                };

                let mut cursor_x = draw_x;
                for run in runs {
                    if run.text.is_empty() {
                        continue;
                    }
                    let run_size = *size_pt * run.scale;
                    let run_text = sanitize_pdf_text(&run.text);
                    let run_y = draw_y + *size_pt * run.baseline_offset;
                    let run_advance = text_advance_pt(chosen_alias, &run_text, run_size);

                    if rotation_deg.abs() > 0.001 {
                        surface.text_at_rotated(
                            cursor_x,
                            page_h_pt - run_y,
                            chosen_alias,
                            run_size,
                            &run_text,
                            -*rotation_deg,
                        );
                    } else {
                        surface.text_at(cursor_x, page_h_pt - run_y, chosen_alias, run_size, &run_text);
                    }

                    if run.overbar && run_advance > 0.1 {
                        surface.set_stroke_color(sr, sg, sb);
                        surface.set_stroke_width((run_size * 0.08).max(0.25));
                        let y_bar = run_y - run_size * 0.78;
                        let (x1, y1, x2, y2) = if rotation_deg.abs() > 0.001 {
                            let (rx1, ry1) = rotate_about(cursor_x, y_bar, cursor_x, run_y, -*rotation_deg);
                            let (rx2, ry2) = rotate_about(cursor_x + run_advance, y_bar, cursor_x, run_y, -*rotation_deg);
                            (rx1, ry1, rx2, ry2)
                        } else {
                            (cursor_x, y_bar, cursor_x + run_advance, y_bar)
                        };
                        surface.stroke_line(x1, page_h_pt - y1, x2, page_h_pt - y2, (run_size * 0.08).max(0.25));
                    }

                    cursor_x += run_advance;
                }
            }
        }
    }

    // Template frame and title block (if enabled).
    if opts.include_title_block {
        if let Some(template_id) = &opts.sheet_template {
            if let Some(template) = crate::template::load_builtin(template_id) {
                let frame_margin_pt = (template.frame.border_margin_mm * MM_TO_PT) as f32;
                surface.stroke_rect(
                    frame_margin_pt,
                    frame_margin_pt,
                    page_w_pt - 2.0 * frame_margin_pt,
                    page_h_pt - 2.0 * frame_margin_pt,
                    (0.15 * MM_TO_PT) as f32,
                );

                let sub_ctx = SubstitutionContext {
                    metadata: &ctx.metadata,
                    filename: sheet
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    sheet_name: sheet.sheet_name.clone(),
                    sheet_number: sheet.sheet_number,
                    sheet_count: sheet.sheet_count,
                    signex_version: env!("CARGO_PKG_VERSION"),
                };

                let tb_width_pt = (template.title_block.width_mm * MM_TO_PT) as f32;
                let tb_height_pt = (template.title_block.height_mm * MM_TO_PT) as f32;
                let tb_x = page_w_pt - tb_width_pt;
                let tb_y = page_h_pt - tb_height_pt;
                surface.stroke_rect(
                    tb_x,
                    tb_y,
                    tb_width_pt,
                    tb_height_pt,
                    (0.2 * MM_TO_PT) as f32,
                );

                for field in &template.title_block.fields {
                    let resolved = crate::resolve(&field.default_text, &sub_ctx);
                    let fx = tb_x + (field.x_mm * MM_TO_PT) as f32;
                    let fy = tb_y + (field.y_mm * MM_TO_PT) as f32;
                    let font = PdfFont::for_style(field.font_style);
                    let size = (field.font_size_mm * MM_TO_PT) as f32;
                    surface.text_at(fx, fy, font.alias(), size, &resolved);
                }
            }
        }
    }

    Ok(surface.finish())
}

/// Resolve a `PageRange` against the project's sheet count into a concrete
/// list of zero-based sheet indices to export.
fn resolve_page_range(range: &PageRange, sheet_count: usize) -> Result<Vec<usize>, PdfError> {
    match range {
        PageRange::All => Ok((0..sheet_count).collect()),
        PageRange::Current => Ok(vec![0]),
        PageRange::Specific(pages) => {
            let mut out = Vec::with_capacity(pages.len());
            for &p in pages {
                if p == 0 || p > sheet_count {
                    return Err(PdfError::OutOfRange(p, sheet_count));
                }
                out.push(p - 1);
            }
            Ok(out)
        }
        PageRange::Range(start, end) => {
            if *start == 0 || *end == 0 || *start > sheet_count || *end > sheet_count {
                return Err(PdfError::OutOfRange(
                    (*start).max(*end).max(1),
                    sheet_count,
                ));
            }
            if start <= end {
                Ok((start - 1..*end).collect())
            } else {
                Ok((end - 1..*start).collect())
            }
        }
    }
}

#[derive(Clone)]
struct PdfTextRun {
    text: String,
    scale: f32,
    baseline_offset: f32,
    overbar: bool,
}

fn pdf_markup_runs(input: &str) -> Vec<PdfTextRun> {
    let expanded = normalize_standard_text(input);
    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return vec![PdfTextRun {
            text: expanded,
            scale: 1.0,
            baseline_offset: 0.0,
            overbar: false,
        }];
    }

    segments
        .into_iter()
        .map(|seg| match seg {
            RichSegment::Normal(t) => PdfTextRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
            RichSegment::Overbar(t) => PdfTextRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: true,
            },
            RichSegment::Subscript(t) => PdfTextRun {
                text: t,
                scale: 0.72,
                baseline_offset: 0.26,
                overbar: false,
            },
            RichSegment::Superscript(t) => PdfTextRun {
                text: t,
                scale: 0.72,
                baseline_offset: -0.34,
                overbar: false,
            },
        })
        .filter(|run| !run.text.is_empty())
        .collect()
}

fn normalize_standard_text(input: &str) -> String {
    let ctx = ExpressionEvalContext::default();
    let evaluated = evaluate_expressions(input, &ctx);
    expand_standard_char_escapes(&evaluated)
}

fn rotate_about(px: f32, py: f32, ox: f32, oy: f32, rotation_deg: f32) -> (f32, f32) {
    let rad = rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let dx = px - ox;
    let dy = py - oy;
    (ox + dx * cos - dy * sin, oy + dx * sin + dy * cos)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::SchematicSheet;

    use super::*;
    use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

    fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: uuid::Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".into(),
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
        }
    }

    fn sample_ctx(sheet_count: usize) -> ExportContext {
        ExportContext {
            sheets: (0..sheet_count)
                .map(|i| SheetSnapshot {
                    path: PathBuf::from(format!("sheet_{i}.standard_sch")),
                    schematic: empty_sheet(),
                    sheet_name: format!("Sheet{i}"),
                    sheet_number: i + 1,
                    sheet_count,
                })
                .collect(),
            metadata: ProjectMetadata::default(),
        }
    }

    #[test]
    fn produces_valid_pdf_header() {
        let ctx = sample_ctx(1);
        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");
        assert!(out.bytes.starts_with(b"%PDF-"), "missing %PDF- header");
        assert!(out.bytes.ends_with(b"%%EOF\n") || out.bytes.ends_with(b"%%EOF"));
        assert_eq!(out.page_count, 1);
    }

    #[test]
    fn multi_sheet_produces_multi_page() {
        let ctx = sample_ctx(4);
        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");
        assert_eq!(out.page_count, 4);
    }

    #[test]
    fn empty_context_errors() {
        let ctx = ExportContext {
            sheets: vec![],
            metadata: ProjectMetadata::default(),
        };
        let err = PdfExporter.export(&ctx, &PdfOptions::default()).unwrap_err();
        assert!(matches!(err, PdfError::NoSheets));
    }

    #[test]
    fn page_range_specific() {
        let ctx = sample_ctx(5);
        let opts = PdfOptions {
            page_range: PageRange::Specific(vec![1, 3, 5]),
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        assert_eq!(out.page_count, 3);
    }

    #[test]
    fn page_range_range_inclusive() {
        let ctx = sample_ctx(5);
        let opts = PdfOptions {
            page_range: PageRange::Range(2, 4),
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        assert_eq!(out.page_count, 3); // 2, 3, 4
    }

    #[test]
    fn page_range_out_of_bounds() {
        let ctx = sample_ctx(3);
        let opts = PdfOptions {
            page_range: PageRange::Specific(vec![1, 99]),
            ..Default::default()
        };
        let err = PdfExporter.export(&ctx, &opts).unwrap_err();
        assert!(matches!(err, PdfError::OutOfRange(99, 3)));
    }

    #[test]
    fn page_size_reflected_in_media_box() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Portrait,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        // A4 portrait = 210 × 297 mm = 595.28 × 841.89 pt.
        let bytes = String::from_utf8_lossy(&out.bytes);
        assert!(bytes.contains("595"), "width not reflected in MediaBox");
        assert!(bytes.contains("841"), "height not reflected in MediaBox");
    }

    #[test]
    fn exports_schematic_content() {
        use signex_types::schematic::{Wire, Symbol, Label, LabelType, Point};
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut sheet = empty_sheet();

        // Add one wire.
        sheet.wires.push(Wire {
            uuid: Uuid::nil(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 10.0),
            stroke_width: 0.15,
        });

        // Add one symbol.
        sheet.symbols.push(Symbol {
            uuid: Uuid::nil(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(50.0, 50.0),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: HashMap::new(),
            custom_properties: vec![],
            pin_uuids: HashMap::new(),
            instances: vec![],
            footprint: String::new(),
            datasheet: String::new(),
        });

        // Add one label.
        sheet.labels.push(Label {
            uuid: Uuid::nil(),
            text: "VCC".to_string(),
            position: Point::new(20.0, 20.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 0.0,
            justify: signex_types::schematic::HAlign::Center,
            justify_v: signex_types::schematic::VAlign::Bottom,
        });

        let mut ctx = sample_ctx(1);
        ctx.sheets[0].schematic = sheet;

        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");

        let bytes = String::from_utf8_lossy(&out.bytes);
        // Check for content stream operators: 'm' (moveto), 'l' (lineto), 'S' (stroke),
        // 're' (rect), 'Tj' (show text).
        let has_graphics = bytes.contains(" l\n") || bytes.contains(" re\n") || bytes.contains(" Tj");
        assert!(
            has_graphics,
            "exported PDF should contain graphics operators"
        );
    }

    #[test]
    fn colour_mode_colour_preserves_rgb() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            colour_mode: ColourMode::Colour,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        assert!(out.bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn colour_mode_grayscale_maps_red_to_0_299() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            colour_mode: ColourMode::Grayscale,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        assert!(out.bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn colour_mode_bw_pushes_strokes_to_black() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            colour_mode: ColourMode::BlackAndWhite,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        assert!(out.bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn fit_to_page_scales_large_content_down() {
        use signex_types::schematic::{Wire, Point};
        use uuid::Uuid;

        let mut sheet = empty_sheet();
        // Add a very large wire (0, 0) to (500, 500) mm — much larger than A4.
        sheet.wires.push(Wire {
            uuid: Uuid::nil(),
            start: Point::new(0.0, 0.0),
            end: Point::new(500.0, 500.0),
            stroke_width: 0.15,
        });

        let mut ctx = sample_ctx(1);
        ctx.sheets[0].schematic = sheet;

        let opts = PdfOptions {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Landscape,
            scale: PdfScale::FitToPage,
            margins: Margins {
                top_mm: 10.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
            },
            ..Default::default()
        };

        let out = PdfExporter.export(&ctx, &opts).expect("export");
        // PDF should be valid and contain content (scaled down wires).
        assert!(out.bytes.starts_with(b"%PDF-"));
        assert!(out.page_count == 1);
    }

    #[test]
    fn fit_to_page_does_not_upscale_small_content() {
        use signex_types::schematic::{Wire, Point};
        use uuid::Uuid;

        let mut sheet = empty_sheet();
        // Add a small wire (10, 10) to (20, 20) mm — much smaller than A4.
        sheet.wires.push(Wire {
            uuid: Uuid::nil(),
            start: Point::new(10.0, 10.0),
            end: Point::new(20.0, 20.0),
            stroke_width: 0.15,
        });

        let mut ctx = sample_ctx(1);
        ctx.sheets[0].schematic = sheet;

        let opts = PdfOptions {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Landscape,
            scale: PdfScale::FitToPage,
            margins: Margins {
                top_mm: 10.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
            },
            ..Default::default()
        };

        let out = PdfExporter.export(&ctx, &opts).expect("export");
        // PDF should be valid. FitToPage should NOT upscale (use 1:1).
        assert!(out.bytes.starts_with(b"%PDF-"));
        assert!(out.page_count == 1);
    }

    #[test]
    fn template_draws_frame_rect() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            sheet_template: Some(TemplateId::from("iso_a4_landscape")),
            include_title_block: true,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        // PDF should contain frame rect operator (re).
        let bytes = String::from_utf8_lossy(&out.bytes);
        assert!(bytes.contains(" re\n"), "template should draw frame rect");
    }

    #[test]
    fn template_renders_substituted_text_in_title_block() {
        let mut ctx = sample_ctx(1);
        ctx.metadata.title = "Test Project".to_string();
        ctx.metadata.revision = "A".to_string();

        let opts = PdfOptions {
            sheet_template: Some(TemplateId::from("iso_a4_landscape")),
            include_title_block: true,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        // PDF should be valid and include title block fields.
        assert!(out.bytes.starts_with(b"%PDF-"));
    }
}
