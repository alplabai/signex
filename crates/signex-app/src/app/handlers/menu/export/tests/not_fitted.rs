//! #1294 — the print preview draws not-fitted parts as the user's "Cross out
//! not-fitted parts" preference says, read at each render, not when the
//! dialog opened. #2962 — a page's not-fitted parts are decided across the
//! project, as the BOM decides them, not from that page alone.

use std::path::{Path, PathBuf};

use signex_output::svg::{SvgElement, SvgEvaluatorInputs, SvgRenderContext};
use signex_output::{ExportContext, PdfOptions, PlacedFits};
use signex_types::schematic::{FillType, Graphic, LibGraphic, Point, greyed_ink};
use uuid::Uuid;

use super::net_text_hierarchy::placed_app;
use super::{app_workspace, open_with, sheet_with_net};

#[test]
fn the_print_preview_follows_the_cross_out_preference() {
    let mut app = placed_app("/w/not_fitted", &["top.snxsch"], &[], Vec::new());
    let _ = app.handle_print_preview_requested();
    let job = app.preview_raster_job().expect("a page to render");
    assert!(job.options.pdf.draw_not_fitted, "on by default");

    app.ui_state.cross_out_not_fitted = false;
    let job = app.preview_raster_job().expect("a page to render");
    assert!(!job.options.pdf.draw_not_fitted);
}

const DIR: &str = "/w/split_part";

/// A project whose two-unit part U1 has unit A, marked DNP, on the root page
/// and unit B on the declared child page `child.snxsch`, placed once as "A".
/// The BOM leaves U1 out, so both units draw not fitted.
fn split_part_app() -> crate::app::Signex {
    let mut app = app_workspace(DIR, &["top.snxsch", "child.snxsch"]);
    let mut top = sheet_with_net("U1", "TOP_NET", &["child.snxsch"]);
    top.child_sheets[0].name = "A".to_string();
    top.child_sheets[0].uuid = Uuid::from_u128(1);
    let mut child = sheet_with_net("U1", "CHILD_NET", &[]);
    for (sheet, unit, dnp) in [(&mut top, 1, true), (&mut child, 2, false)] {
        let lib = sheet
            .lib_symbols
            .get_mut("Device:R")
            .expect("the part's library");
        lib.graphics.push(LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-2.0, -1.0),
                end: Point::new(2.0, 1.0),
                width: 0.25,
                fill: FillType::None,
            },
        });
        sheet.symbols[0].unit = unit;
        sheet.symbols[0].dnp = dnp;
    }
    open_with(&mut app.document_state, &path("top.snxsch"), top);
    open_with(&mut app.document_state, &path("child.snxsch"), child);
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(DIR).join(name)
}

/// How unit B draws on the child page of `ctx`, rendered as the PDF and
/// the preview render a page: the fits decided over `ctx`, then the page.
struct ChildDrawn {
    /// Two-point strokes in the symbol ink beyond those the page draws with
    /// the preference off: the cross. (A theme palette can draw wires in
    /// the symbol ink.)
    crosses: usize,
    /// Whether the body outline is in the greyed symbol ink.
    greyed: bool,
}

fn child_drawn(ctx: &ExportContext, opts: &PdfOptions) -> ChildDrawn {
    let ink = opts.palette.symbol_stroke;
    let (strokes, paper) = child_strokes(ctx, opts);
    let plain = PdfOptions {
        draw_not_fitted: false,
        ..opts.clone()
    };
    let ink_lines = |strokes: &Strokes| {
        strokes
            .iter()
            .filter(|(count, stroke)| *count == 2 && *stroke == Some(ink))
            .count()
    };
    let [r, g, b] = greyed_ink([ink.0, ink.1, ink.2], [paper.0, paper.1, paper.2]);
    ChildDrawn {
        crosses: ink_lines(&strokes) - ink_lines(&child_strokes(ctx, &plain).0),
        greyed: strokes
            .iter()
            .any(|(count, stroke)| *count == 5 && *stroke == Some((r, g, b))),
    }
}

/// Every path on the child page as (command count, stroke), and the paper.
type Strokes = Vec<(usize, Option<(f32, f32, f32)>)>;

fn child_strokes(ctx: &ExportContext, opts: &PdfOptions) -> (Strokes, (f32, f32, f32)) {
    let page = ctx
        .sheets
        .iter()
        .find(|s| s.path == path("child.snxsch"))
        .expect("the child page is printed");
    assert!(
        !page.placement.is_root(),
        "the child page prints at its placement"
    );
    let fits = PlacedFits::of_context(ctx, opts.variant.as_deref());
    let (refdes, nets) = Default::default();
    let inputs = SvgEvaluatorInputs {
        special_strings: None,
        global_refdes: &refdes,
        net_name_by_symbol_pin: &nets,
        cell: "",
        variant_settings: Some(&ctx.metadata.variant_settings),
        placed_fits: Some(&fits),
    };
    let svg = SvgRenderContext::from_sheet(page, opts, 297.0, 210.0, 1.0, Some(&inputs));
    let strokes = svg
        .elements
        .iter()
        .filter_map(|e| match e {
            SvgElement::Path { commands, style } => Some((commands.len(), style.stroke_rgb)),
            SvgElement::Text { .. } => None,
        })
        .collect();
    (strokes, svg.paper)
}

fn assert_not_fitted(drawn: &ChildDrawn, what: &str) {
    assert_eq!(
        drawn.crosses, 2,
        "{what}: U1B is crossed, as the BOM leaves U1 out"
    );
    assert!(drawn.greyed, "{what}: U1B's body is greyed");
}

#[test]
fn the_svg_page_draws_a_unit_not_fitted_when_its_part_is_dnp_on_the_root() {
    let app = split_part_app();
    let (ctx, _) = super::super::build_page_scope(&app.document_state).expect("scope");
    let opts = PdfOptions {
        draw_not_fitted: true,
        ..PdfOptions::default()
    };
    assert_not_fitted(&child_drawn(&ctx, &opts), "SVG page");
}

#[test]
fn the_pdf_draws_a_child_unit_not_fitted_even_with_only_the_child_page_picked() {
    let app = split_part_app();
    let source: &Path = &path("top.snxsch");
    let (mut ctx, _) =
        super::super::build_page_scope_for_source(&app.document_state, source).expect("scope");
    let mut opts = PdfOptions {
        draw_not_fitted: true,
        ..PdfOptions::default()
    };
    // The user ticks only the child page in the PDF dialog.
    ctx.sheets.retain(|s| s.path == path("child.snxsch"));
    super::super::apply_printed_variant(&mut ctx.metadata, &mut opts.variant);
    assert_not_fitted(&child_drawn(&ctx, &opts), "PDF");
}

#[test]
fn the_print_preview_draws_a_child_unit_not_fitted_when_its_part_is_dnp_on_the_root() {
    let mut app = split_part_app();
    let _ = app.handle_print_preview_requested();
    let job = app.preview_raster_job().expect("a page to render");
    assert_not_fitted(&child_drawn(&job.ctx, &job.options.pdf), "print preview");
}
