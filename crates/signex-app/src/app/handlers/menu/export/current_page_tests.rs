//! #109 — "Current page" prints the sheet being viewed: a hierarchical child
//! that is not a declared page, the preview's own source after a tab switch,
//! and a path spelled differently from the page list on a case-folding volume.

use std::path::{Path, PathBuf};

use signex_output::{
    Exporter, PageRange, PdfExporter, PdfOptions, PreviewOptions, PreviewRasterizer,
};

use super::tests::{add_project, app_workspace, open_with, sheet_with_net};

fn current_options() -> PdfOptions {
    PdfOptions {
        page_range: PageRange::Current,
        ..PdfOptions::default()
    }
}

/// `top` is the only declared page; `child` is open (reached by descending
/// into a sheet symbol) and active.
fn viewing_a_child() -> (crate::app::Signex, PathBuf) {
    let mut app = app_workspace("/w/a", &["top.snxsch"]);
    let top = Path::new("/w/a").join("top.snxsch");
    let child = Path::new("/w/a").join("child.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "N_TOP", &["child.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &child,
        sheet_with_net("R_CHILD", "N_CHILD", &[]),
    );
    app.document_state.active_path = Some(child.clone());
    (app, child)
}

#[test]
fn current_page_on_an_undeclared_child_renders_the_child() {
    let (app, child) = viewing_a_child();
    let (mut ctx, issues) = super::build_page_scope(&app.document_state).expect("scope");
    assert!(
        ctx.sheets.iter().all(|s| s.path != child),
        "child is not a declared page"
    );

    assert!(super::isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        &child,
        app.document_state.tab_placements.get(&child),
        &PageRange::Current
    ));

    assert_eq!(ctx.sheets.len(), 1);
    assert_eq!(ctx.sheets[ctx.current_sheet_index()].path, child);
    // Both consumers: the PDF writer and the preview rasteriser.
    let pdf = PdfExporter.export(&ctx, &current_options()).expect("pdf");
    assert_eq!(pdf.page_count, 1);
    let pages = PreviewRasterizer.rasterize(
        &ctx,
        &PreviewOptions {
            pdf: current_options(),
            dpi: 72.0,
        },
    );
    assert_eq!(pages.len(), 1);
}

#[test]
fn other_ranges_and_declared_pages_are_left_alone() {
    let (app, child) = viewing_a_child();
    let (mut ctx, issues) = super::build_page_scope(&app.document_state).expect("scope");
    assert!(!super::isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        &child,
        app.document_state.tab_placements.get(&child),
        &PageRange::All
    ));
    let top = Path::new("/w/a").join("top.snxsch");
    ctx.current_sheet = Some(top.clone());
    assert!(!super::isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        &top,
        app.document_state.tab_placements.get(&top),
        &PageRange::Current
    ));
}

#[test]
fn the_preview_rasterises_its_source_after_a_tab_switch() {
    let mut app = app_workspace("/w/a", &["a.snxsch"]);
    add_project(&mut app, "/w/b", &["b1.snxsch"]);
    let a = Path::new("/w/a").join("a.snxsch");
    let b = Path::new("/w/b").join("b1.snxsch");
    open_with(
        &mut app.document_state,
        &a,
        sheet_with_net("R_A", "N_A", &[]),
    );
    open_with(
        &mut app.document_state,
        &b,
        sheet_with_net("R_B", "N_B", &[]),
    );
    app.document_state.active_path = Some(a.clone());
    let _ = app.handle_print_preview_requested();

    app.document_state.active_path = Some(b);
    let _ = app.handle_print_preview_rerender();

    let preview = app.document_state.preview.as_ref().expect("preview");
    let files: Vec<&PathBuf> = preview.sheet_files.iter().map(|(p, _)| &p.0).collect();
    assert_eq!(files, vec![&a], "the picker and pages stay on the source");
}

#[test]
fn current_sheet_matches_the_page_under_the_projects_case_fold() {
    let mut app = app_workspace("/w/a", &["top.snxsch", "power.snxsch"]);
    app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::from_probe(true);
    let top = Path::new("/w/a").join("top.snxsch");
    let power = Path::new("/w/a").join("power.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R1", "N1", &[]),
    );
    open_with(
        &mut app.document_state,
        &power,
        sheet_with_net("R2", "N2", &[]),
    );
    // The tab was opened under a different spelling of the same file.
    let shouty = Path::new("/w/a").join("POWER.snxsch");
    open_with(
        &mut app.document_state,
        &shouty,
        sheet_with_net("R2", "N2", &[]),
    );
    app.document_state.active_path = Some(shouty);

    let (ctx, _) = super::build_page_scope(&app.document_state).expect("scope");

    assert_eq!(ctx.current_sheet.as_ref(), Some(&power));
    assert_eq!(ctx.sheets[ctx.current_sheet_index()].path, power);
}

/// Without a project numbering (a loose document) the page is 1 of 1.
#[test]
fn current_page_without_a_project_numbering_prints_one_of_one() {
    let (app, child) = viewing_a_child();
    let (mut ctx, mut issues) = super::build_page_scope(&app.document_state).expect("scope");
    issues.sheet_numbering = None;
    assert!(super::isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        &child,
        None,
        &PageRange::Current,
    ));
    assert_eq!(
        (ctx.sheets[0].sheet_number, ctx.sheets[0].sheet_count),
        (1, 1)
    );
}
