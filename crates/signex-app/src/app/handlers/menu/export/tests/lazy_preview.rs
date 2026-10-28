//! #111 — print-preview pages are rasterised lazily: the shown page when the
//! preview opens, any other when the user navigates to it, each once per set
//! of options.

use super::protel_hierarchy::placed_app;
use crate::app::Signex;
use crate::app::handlers::menu::export::settle;

/// A preview of five printed pages (`top` plus four placements of `child`,
/// range "All"), opened and settled.
fn open_five_page_preview(dir: &str) -> Signex {
    let mut app = placed_app(
        dir,
        &["top.snxsch", "child.snxsch"],
        &["A", "B", "C", "D"],
        Vec::new(),
    );
    let task = app.handle_print_preview_requested();
    settle(&mut app, task);
    let task = app.handle_print_preview_set_page_range_all();
    settle(&mut app, task);
    app
}

fn drawn_pages(app: &Signex) -> Vec<usize> {
    let preview = app.document_state.preview.as_ref().expect("preview");
    (0..preview.pages.len())
        .filter(|&i| preview.pages[i].handle().is_some())
        .collect()
}

fn page_count(app: &Signex) -> usize {
    app.document_state
        .preview
        .as_ref()
        .expect("preview")
        .pages
        .len()
}

#[test]
fn opening_a_five_page_preview_draws_only_the_shown_page() {
    let app = open_five_page_preview("/w/lazy-open");
    assert_eq!(page_count(&app), 5, "every page is laid out for the rail");
    assert_eq!(drawn_pages(&app), [0], "only the shown page is drawn");
}

#[test]
fn navigating_draws_the_next_page_once() {
    let mut app = open_five_page_preview("/w/lazy-nav");

    let task = app.handle_print_preview_select_page(1);
    assert_eq!(task.units(), 1, "an undrawn page starts one render");
    settle(&mut app, task);
    assert_eq!(drawn_pages(&app), [0, 1]);

    // Showing a drawn page again, in either direction, is free.
    assert_eq!(app.handle_print_preview_select_page(1).units(), 0);
    assert_eq!(app.handle_print_preview_select_page(0).units(), 0);
    assert_eq!(drawn_pages(&app), [0, 1]);
}

#[test]
fn a_page_already_being_drawn_is_not_asked_for_twice() {
    let mut app = open_five_page_preview("/w/lazy-twice");
    let first = app.handle_print_preview_select_page(2);
    assert_eq!(first.units(), 1);
    assert_eq!(
        app.handle_print_preview_select_page(2).units(),
        0,
        "the render is already on its way"
    );
    settle(&mut app, first);
    assert_eq!(drawn_pages(&app), [0, 2]);
}

#[test]
fn an_options_change_discards_the_drawn_pages() {
    let mut app = open_five_page_preview("/w/lazy-options");
    let task = app.handle_print_preview_select_page(3);
    settle(&mut app, task);
    assert_eq!(drawn_pages(&app), [0, 3]);

    let task = app.handle_print_preview_set_include_title_block(false);
    settle(&mut app, task);
    assert_eq!(
        drawn_pages(&app),
        [3],
        "only the page on screen is redrawn with the new options"
    );
}

#[test]
fn a_page_result_from_before_an_options_change_is_dropped() {
    use crate::app::{Message, PrintPreviewMsg};
    let mut app = open_five_page_preview("/w/lazy-stale");
    let old_generation = app
        .document_state
        .preview
        .as_ref()
        .expect("preview")
        .raster_generation;

    let task = app.handle_print_preview_set_include_title_block(false);
    settle(&mut app, task);
    assert_eq!(drawn_pages(&app), [0]);

    let late = Message::PrintPreview(PrintPreviewMsg::PagesRasterized {
        generation: old_generation,
        pages: std::sync::Arc::new(vec![(
            1,
            signex_output::PreviewPage {
                page_number: 2,
                sheet_count: 5,
                width_px: 1,
                height_px: 1,
                rgba: vec![0, 0, 0, 255],
            },
        )]),
    });
    let task = app.update(late);
    settle(&mut app, task);
    assert_eq!(drawn_pages(&app), [0], "the stale page was not shown");
}
