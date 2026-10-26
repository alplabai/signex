//! Project sheet numbering in the export scope (#1081): one number per sheet
//! placement, from the hierarchy, the same whichever pages an export prints.

use std::path::PathBuf;

use signex_output::{PageRange, PdfOptions, PreviewOptions, PreviewRasterizer};
use signex_types::schematic::SheetInstance;
use uuid::Uuid;

use super::protel_hierarchy::placed_app;
use super::{open_with, pdf_request_all_sheets, sheet_with_net};
use crate::app::handlers::menu::export::{
    build_export_scope, build_page_scope, isolate_undeclared_current,
};

/// `(page name, sheet number, sheet count)` for every printed page.
fn printed(app: &crate::app::Signex) -> Vec<(String, usize, usize)> {
    let (ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    ctx.sheets
        .iter()
        .map(|page| (page.sheet_name.clone(), page.sheet_number, page.sheet_count))
        .collect()
}

/// The export's `Sheet numbering:` lines, as the PDF and print-preview paths
/// log them.
fn numbering_lines(app: &crate::app::Signex) -> Vec<String> {
    let (_, issues) = build_export_scope(&app.document_state).expect("export scope");
    issues
        .sheet_numbering
        .as_ref()
        .map(|numbering| numbering.messages())
        .unwrap_or_default()
}

/// `top` places the child as A and B; `other` is a declared page the root does
/// not reach. The `.snxprj` lists `top`, `other`, `child`.
fn listed_out_of_walk_order(dir: &str) -> crate::app::Signex {
    let mut app = placed_app(
        dir,
        &["top.snxsch", "other.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("other.snxsch"),
        sheet_with_net("R_OTHER", "OTHER_NET", &[]),
    );
    app
}

/// The sheet numbers the preview shows for `range`.
fn previewed(app: &crate::app::Signex, range: PageRange) -> Vec<(usize, usize)> {
    let (ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    let opts = PreviewOptions {
        pdf: PdfOptions {
            page_range: range,
            ..PdfOptions::default()
        },
        dpi: 10.0,
    };
    PreviewRasterizer
        .rasterize(&ctx, &opts)
        .iter()
        .map(|page| (page.page_number, page.sheet_count))
        .collect()
}

/// Put `entry` on `top.snxsch`'s sheet symbol `symbol`.
fn with_top_entry(app: &mut crate::app::Signex, dir: &str, symbol: usize, entry: SheetInstance) {
    let top_path = PathBuf::from(dir).join("top.snxsch");
    let mut top = app.document_state.engines[&top_path].document().clone();
    top.child_sheets[symbol].instances.push(entry);
    open_with(&mut app.document_state, &top_path, top);
}

fn root_entry(page: &str) -> SheetInstance {
    SheetInstance {
        project: String::new(),
        path: "/".to_string(),
        page: page.to_string(),
    }
}

#[test]
fn pages_take_their_number_from_the_hierarchy_not_the_declared_list() {
    let app = listed_out_of_walk_order("/w/numbering-order");
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 4),
            ("child.snxsch (A)".to_string(), 2, 4),
            ("child.snxsch (B)".to_string(), 3, 4),
            ("other.snxsch".to_string(), 4, 4),
        ]
    );
}

#[test]
fn the_count_includes_placements_that_are_not_printed_pages() {
    let app = placed_app(
        "/w/numbering-count",
        &["top.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    assert_eq!(printed(&app), vec![("top.snxsch".to_string(), 1, 3)]);
}

#[test]
fn a_page_range_selects_sheet_numbers_in_hierarchy_order() {
    let app = listed_out_of_walk_order("/w/numbering-range");
    assert_eq!(previewed(&app, PageRange::Specific(vec![2])), [(2, 4)]);
    assert_eq!(
        previewed(&app, PageRange::Range(1, 4)),
        [(1, 4), (2, 4), (3, 4), (4, 4)]
    );
}

#[test]
fn a_manual_number_is_printed_and_reported() {
    let dir = "/w/numbering-manual";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    with_top_entry(&mut app, dir, 1, root_entry("7"));
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 3),
            ("child.snxsch (A)".to_string(), 2, 3),
            ("child.snxsch (B)".to_string(), 7, 3),
        ]
    );
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page B is numbered 7, above the page count 3".to_string(),
            "Sheet numbering: no page prints number 3".to_string(),
        ]
    );
}

#[test]
fn an_unusable_manual_number_is_reported_and_ignored() {
    let dir = "/w/numbering-invalid";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    with_top_entry(&mut app, dir, 0, root_entry("abc"));
    assert_eq!(printed(&app)[1], ("child.snxsch (A)".to_string(), 2, 3));
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page A has the manual number 'abc', which is not a positive whole \
             number; it is numbered 2"
                .to_string()
        ]
    );
}

#[test]
fn a_clean_project_has_no_numbering_lines() {
    let app = listed_out_of_walk_order("/w/numbering-clean");
    assert!(numbering_lines(&app).is_empty());
}

#[test]
fn a_numbering_issue_is_not_an_export_issue() {
    let dir = "/w/numbering-not-an-issue";
    let mut app = placed_app(dir, &["top.snxsch"], &["A", "B"], Vec::new());
    with_top_entry(&mut app, dir, 1, root_entry("7"));
    let (_, issues) = build_export_scope(&app.document_state).expect("export scope");
    assert!(!numbering_lines(&app).is_empty(), "precondition");
    assert!(
        issues
            .messages()
            .iter()
            .all(|line| !line.contains("Sheet numbering")),
        "{:?}",
        issues.messages()
    );
}

/// The PDF export logs each numbering line once, as-is.
#[test]
fn the_pdf_export_logs_the_numbering_lines() {
    let _ = crate::diagnostics::init_logging();
    let marker = format!("Num{}", Uuid::new_v4().simple());
    let dir = std::env::temp_dir().join(format!("signex-numbering-{marker}"));
    let dir_str = dir.to_string_lossy().to_string();
    let mut app = placed_app(&dir_str, &["top.snxsch"], &["A", &marker], Vec::new());
    with_top_entry(&mut app, &dir_str, 1, root_entry("7"));
    let expected = format!("Sheet numbering: page {marker} is numbered 7, above the page count 3");
    let logged = || {
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(&marker) && entry.message.contains("numbering"))
            .map(|entry| entry.message.clone())
            .collect::<Vec<_>>()
    };
    assert!(logged().is_empty());

    let source = app.document_state.active_path.clone().expect("active path");
    let request = Box::new(pdf_request_all_sheets(&app, &source));
    let out = std::env::temp_dir().join(format!("signex-{marker}.pdf"));
    let task = app.handle_export_pdf_finished(request, Ok(out.clone()));
    crate::app::handlers::menu::export::settle(&mut app, task);

    assert_eq!(logged(), [expected]);
    std::fs::remove_file(&out).ok();
}

/// #1081: a child placed twice, viewed alone and printed as Current, prints
/// its placement's project number and the project's count.
#[test]
fn current_page_on_a_project_child_prints_its_project_number() {
    let mut app = placed_app("/w/cur-num", &["top.snxsch"], &["A", "B"], Vec::new());
    let child = PathBuf::from("/w/cur-num").join("child.snxsch");
    app.document_state.active_path = Some(child.clone());
    let (mut ctx, issues) = build_page_scope(&app.document_state).expect("scope");
    assert!(isolate_undeclared_current(
        &mut ctx,
        &app.document_state,
        &child,
        &PageRange::Current,
        issues.sheet_numbering.as_ref(),
    ));
    let page = &ctx.sheets[0];
    assert_eq!((page.sheet_number, page.sheet_count), (2, 3));
}

/// A loose document has no project numbering: its pages keep the export
/// order's numbers and nothing is reported.
#[test]
fn a_loose_document_keeps_the_export_order_numbers() {
    let mut app = super::app_workspace("/w/numbering-project", &["top.snxsch"]);
    let active = PathBuf::from("/w/numbering-loose").join("m.snxsch");
    let other = PathBuf::from("/w/numbering-loose").join("a.snxsch");
    open_with(
        &mut app.document_state,
        &other,
        sheet_with_net("R_A", "N_A", &[]),
    );
    open_with(
        &mut app.document_state,
        &active,
        sheet_with_net("R_M", "N_M", &[]),
    );
    app.document_state.active_path = Some(active);
    assert_eq!(
        printed(&app),
        vec![("m".to_string(), 1, 2), ("a".to_string(), 2, 2)]
    );
    assert!(numbering_lines(&app).is_empty());
}
