//! Per-page PDF content-stream builder + markup/text helpers.

use super::links::{LinkTarget, PageLink, linkable_uri};
use super::surface::PageContent;
use super::*;

/// Build a content stream for a single page. `page_index` is the page's
/// 0-based position among the exported pages.
pub(super) fn build_page_content(
    sheet: &crate::SheetSnapshot,
    page_index: usize,
    opts: &PdfOptions,
    ctx: &ExportContext,
    page_h_pt: f32,
    expr_tables: &ExpressionTables,
    fonts: &mut FontUsage,
) -> Result<PageContent, PdfError> {
    let mut surface = PdfSurface::new(fonts);
    let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);

    // Explicit two-step pipeline:
    // schematic sheet -> SVG render context -> PDF content stream operators.
    let cell = sheet_cell_value(sheet);
    let sheet_metadata = ctx.metadata.for_schematic(&sheet.schematic);
    let special = super::frame::sheet_substitution(sheet, opts, &sheet_metadata);
    let eval_inputs = SvgEvaluatorInputs {
        special_strings: Some(&special),
        global_refdes: &expr_tables.global_refdes,
        net_name_by_symbol_pin: &expr_tables.net_name_by_symbol_pin,
        cell: &cell,
    };
    let (mut svg_ctx, mut layer_spans) = SvgRenderContext::from_sheet_layered(
        sheet,
        opts,
        page_w_mm,
        page_h_mm,
        MM_TO_PT,
        Some(&eval_inputs),
    );
    // The sheet's own paper colour, if it has one (#2034).
    let colour_map = ColourMap::new(opts.colour_mode, svg_ctx.paper);
    // Template frame and title block (if enabled), drawn over the content.
    let frame_start = svg_ctx.elements.len();
    svg_ctx.elements.extend(super::frame::sheet_frame_elements(
        sheet, opts, ctx, page_w_mm, page_h_mm, MM_TO_PT,
    ));
    layer_spans.mark(
        PdfLayer::TitleBlockAndBorder,
        frame_start..svg_ctx.elements.len(),
    );

    // Set default colour for all drawings.
    let (r, g, b) = colour_map.map_stroke_bw(0.0, 0.0, 0.0);
    surface.set_stroke_color(r, g, b);

    // Render all mapped schematic elements from the intermediate SVG page,
    // each layer's run wrapped in its marked content when layers are on.
    let layered = opts.layers.is_some();
    let mut open_until: Option<usize> = None;
    let mut text_links = Vec::new();
    for (index, element) in svg_ctx.elements.iter().enumerate() {
        if open_until == Some(index) {
            surface.raw_operator(optional_content::END_OPERATOR);
            open_until = None;
        }
        if layered && let Some((layer, end)) = layer_spans.starting_at(index) {
            surface.raw_operator(&optional_content::begin_operator(layer));
            open_until = Some(end);
        }
        let links = opts.links.then_some(&mut text_links);
        draw_element(&mut surface, element, &colour_map, page_h_pt, links);
    }
    if open_until.is_some() {
        surface.raw_operator(optional_content::END_OPERATOR);
    }

    // The watermark is drawn last, over the whole page.
    if let Some(watermark) = opts
        .watermark
        .as_ref()
        .filter(|w| w.applies_to_page(page_index))
    {
        if layered {
            surface.raw_operator(&optional_content::begin_operator(PdfLayer::Watermark));
        }
        watermark.draw(&mut surface, page_w_mm, page_h_mm, &colour_map);
        if layered {
            surface.raw_operator(optional_content::END_OPERATOR);
        }
    }

    let mut content = surface.finish_page();
    content.links = text_links;
    Ok(content)
}

/// Write one element of the intermediate SVG page as PDF operators. A text
/// link's area goes into `links` when given.
fn draw_element(
    surface: &mut PdfSurface<'_>,
    element: &SvgElement,
    colour_map: &ColourMap,
    page_h_pt: f32,
    mut links: Option<&mut Vec<PageLink>>,
) {
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
                // A wire colour's alpha, as SVG stroke-opacity (#2236).
                surface.set_stroke_alpha(style.stroke_alpha);
            }
            if let Some((r, g, b)) = style.fill_rgb {
                let (fr, fg, fb) = colour_map.map_fill_bw(r, g, b);
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
            let preferred_text = normalize_pdf_text(&plain_text);
            let chosen_alias = best_alias_for_text(font_alias, &preferred_text);
            let text_w: f32 = runs
                .iter()
                .map(|run| {
                    let t = normalize_pdf_text(&run.text);
                    text_advance_pt(chosen_alias, &t, *size_pt * run.scale)
                })
                .sum();
            let asc = size_pt * 0.8;
            let desc = -size_pt * 0.2;
            // Alignment and run advances are offsets in the text's own
            // frame (u along the baseline, v down across it); `place`
            // turns them with the text, so rotated text is aligned along
            // its baseline rather than along the page axes (#1119).
            let start_u = match align {
                SvgTextAlign::Left => 0.0,
                SvgTextAlign::Center => -text_w * 0.5,
                SvgTextAlign::Right => -text_w,
            };
            let baseline_v = match v_align {
                SvgTextVAlign::Top => asc,
                SvgTextVAlign::Center => (asc + desc) * 0.5,
                SvgTextVAlign::Bottom => desc,
            };
            let rotated = rotation_deg.abs() > 0.001;
            let place = |u: f32, v: f32| text_frame_point(*x, *y, u, v, *rotation_deg);

            let mut cursor_u = start_u;
            for run in runs {
                if run.text.is_empty() {
                    continue;
                }
                let run_size = *size_pt * run.scale;
                let run_text = normalize_pdf_text(&run.text);
                let run_v = baseline_v + *size_pt * run.baseline_offset;
                let run_advance = text_advance_pt(chosen_alias, &run_text, run_size);
                let (gx, gy) = place(cursor_u, run_v);

                if rotated {
                    surface.text_at_rotated(
                        gx,
                        page_h_pt - gy,
                        chosen_alias,
                        run_size,
                        &run_text,
                        -*rotation_deg,
                    );
                } else {
                    surface.text_at(gx, page_h_pt - gy, chosen_alias, run_size, &run_text);
                }

                if run.overbar && run_advance > 0.1 {
                    surface.set_stroke_color(sr, sg, sb);
                    surface.set_stroke_alpha(1.0);
                    surface.set_stroke_width((run_size * 0.08).max(0.25));
                    let v_bar = run_v - run_size * 0.78;
                    let (x1, y1) = place(cursor_u, v_bar);
                    let (x2, y2) = place(cursor_u + run_advance, v_bar);
                    surface.stroke_line(
                        x1,
                        page_h_pt - y1,
                        x2,
                        page_h_pt - y2,
                        (run_size * 0.08).max(0.25),
                    );
                }

                if let (Some(url), Some(links)) = (run.url, links.as_deref_mut()) {
                    // The run's box in its own frame: ascent above the
                    // baseline, descent below, turned with the text.
                    let corners = [
                        place(cursor_u, run_v - run_size * 0.8),
                        place(cursor_u + run_advance, run_v - run_size * 0.8),
                        place(cursor_u, run_v + run_size * 0.2),
                        place(cursor_u + run_advance, run_v + run_size * 0.2),
                    ];
                    let xs = corners.map(|(cx, _)| cx);
                    let ys = corners.map(|(_, cy)| page_h_pt - cy);
                    let min = |v: [f32; 4]| v.into_iter().fold(f32::INFINITY, f32::min);
                    let max = |v: [f32; 4]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
                    links.push(PageLink::over(
                        min(xs),
                        min(ys),
                        max(xs),
                        max(ys),
                        LinkTarget::Uri(url),
                    ));
                }

                cursor_u += run_advance;
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
    /// The address a link run opens (#1298); `None` for other text and for
    /// an address no link may open (`links::linkable_uri`).
    url: Option<String>,
}

impl PdfTextRun {
    fn new(text: String, scale: f32, baseline_offset: f32) -> Self {
        Self {
            text,
            scale,
            baseline_offset,
            overbar: false,
            url: None,
        }
    }
}

fn pdf_markup_runs(input: &str) -> Vec<PdfTextRun> {
    let expanded = normalize_standard_text(input);
    let segments = parse_signex_markup(&expanded);
    if segments.is_empty() {
        return vec![PdfTextRun::new(expanded, 1.0, 0.0)];
    }

    segments
        .into_iter()
        .map(|seg| match seg {
            RichSegment::Normal(t) => PdfTextRun::new(t, 1.0, 0.0),
            // TODO: visual decoration for bold/italic/strike
            RichSegment::Bold(t) | RichSegment::Italic(t) | RichSegment::Strike(t) => {
                PdfTextRun::new(t, 1.0, 0.0)
            }
            RichSegment::Overbar(t) => PdfTextRun {
                overbar: true,
                ..PdfTextRun::new(t, 1.0, 0.0)
            },
            RichSegment::Subscript(t) => PdfTextRun::new(t, 0.72, 0.26),
            RichSegment::Superscript(t) => PdfTextRun::new(t, 0.72, -0.34),
            // The label prints as plain text; the address becomes a link
            // annotation over it.
            RichSegment::Link { label, url } => PdfTextRun {
                url: linkable_uri(&url).map(str::to_owned),
                ..PdfTextRun::new(label, 1.0, 0.0)
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

/// Page point (y down) of the text-frame offset `(u, v)` from the anchor
/// `(x, y)`: `u` runs along the baseline, `v` down across it, and both turn
/// with the text's `rotation_deg` (clockwise-positive on the page, as in
/// SVG).
fn text_frame_point(x: f32, y: f32, u: f32, v: f32, rotation_deg: f32) -> (f32, f32) {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    (x + u * cos - v * sin, y + u * sin + v * cos)
}

#[cfg(test)]
mod frame_tests {
    use super::text_frame_point;

    #[test]
    fn an_upright_offset_is_a_plain_translation() {
        assert_eq!(text_frame_point(10.0, 20.0, 3.0, -1.0, 0.0), (13.0, 19.0));
    }

    /// Bottom-to-top text (-90°, y down): along the baseline is up the page,
    /// below the baseline is to the right.
    #[test]
    fn a_bottom_to_top_offset_turns_with_the_text() {
        let (x, y) = text_frame_point(10.0, 20.0, 5.0, 2.0, -90.0);
        assert!(
            (x - 12.0).abs() < 1e-4 && (y - 15.0).abs() < 1e-4,
            "{x}, {y}"
        );
    }
}
