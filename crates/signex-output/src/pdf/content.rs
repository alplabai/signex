//! Per-page PDF content-stream builder + markup/text helpers.

use super::*;

/// Build a content stream for a single page.
pub(super) fn build_page_content(
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
                        surface.text_at(
                            cursor_x,
                            page_h_pt - run_y,
                            chosen_alias,
                            run_size,
                            &run_text,
                        );
                    }

                    if run.overbar && run_advance > 0.1 {
                        surface.set_stroke_color(sr, sg, sb);
                        surface.set_stroke_width((run_size * 0.08).max(0.25));
                        let y_bar = run_y - run_size * 0.78;
                        let (x1, y1, x2, y2) = if rotation_deg.abs() > 0.001 {
                            let (rx1, ry1) =
                                rotate_about(cursor_x, y_bar, cursor_x, run_y, -*rotation_deg);
                            let (rx2, ry2) = rotate_about(
                                cursor_x + run_advance,
                                y_bar,
                                cursor_x,
                                run_y,
                                -*rotation_deg,
                            );
                            (rx1, ry1, rx2, ry2)
                        } else {
                            (cursor_x, y_bar, cursor_x + run_advance, y_bar)
                        };
                        surface.stroke_line(
                            x1,
                            page_h_pt - y1,
                            x2,
                            page_h_pt - y2,
                            (run_size * 0.08).max(0.25),
                        );
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
                    variant: opts.variant.clone(),
                    physical_structure: opts.use_physical_structure,
                    physical_sheet_number: opts.physical_sheet_number,
                    physical_document_number: opts.physical_document_number,
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
pub(super) fn resolve_page_range(range: &PageRange, sheet_count: usize) -> Result<Vec<usize>, PdfError> {
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
                return Err(PdfError::OutOfRange((*start).max(*end).max(1), sheet_count));
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
    let segments = parse_signex_markup(&expanded);
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
            // TODO(v0.x): visual decoration for bold/italic/strike
            RichSegment::Bold(t) | RichSegment::Italic(t) | RichSegment::Strike(t) => PdfTextRun {
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
            // Links render as plain label text on the canvas — URL is ignored
            // until link rendering ships in a later phase.
            RichSegment::Link { label, .. } => PdfTextRun {
                text: label,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
        })
        .filter(|run| !run.text.is_empty())
        .collect()
}

fn normalize_standard_text(input: &str) -> String {
    let ctx = ExpressionEvalContext::default();
    // Standard-specific char-escape expansion (`{slash}` → `/`, etc.) was removed
    // in Phase 2.3 of the Apache-clean remediation. Inputs no longer carry
    // those tokens because the main repo no longer parses Standard files.
    evaluate_expressions(input, &ctx)
}

fn rotate_about(px: f32, py: f32, ox: f32, oy: f32, rotation_deg: f32) -> (f32, f32) {
    let rad = rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let dx = px - ox;
    let dy = py - oy;
    (ox + dx * cos - dy * sin, oy + dx * sin + dy * cos)
}
