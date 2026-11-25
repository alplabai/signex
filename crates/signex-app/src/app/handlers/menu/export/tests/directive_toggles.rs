//! #2656 — "Net Blankets", "Net Parameters", "Compile Masks" and
//! "Differential Pairs" decide what the page draws, so ticking any of them
//! redraws the open print preview, as No-ERC Markers and Notes do. A toggle
//! the page does not draw yet (Probes) does not.

use super::net_text_hierarchy::placed_app;
use crate::app::{Message, PrintPreviewMsg, Signex};

fn open_preview(dir: &str) -> Signex {
    let mut app = placed_app(dir, &["top.snxsch"], &[], Vec::new());
    let _ = app.handle_print_preview_requested();
    assert!(
        app.document_state.preview.is_some(),
        "fixture: preview open"
    );
    app
}

/// The newest raster generation: it moves when a new render job starts.
fn generation(app: &Signex) -> u64 {
    app.document_state.raster_queue.current().0
}

fn options(app: &Signex) -> &signex_output::PdfOptions {
    &app.document_state
        .preview
        .as_ref()
        .expect("preview")
        .pdf_options
}

#[test]
fn unticking_blankets_redraws_the_preview() {
    let mut app = open_preview("/w/directive_toggles_blankets");
    let before = generation(&app);
    let _ = app.update(Message::PrintPreview(PrintPreviewMsg::SetIncludeBlankets(
        false,
    )));
    assert!(!options(&app).include_blankets);
    assert!(generation(&app) > before, "a new raster job");
}

#[test]
fn unticking_parameter_sets_redraws_the_preview() {
    let mut app = open_preview("/w/directive_toggles_parameters");
    let before = generation(&app);
    let _ = app.update(Message::PrintPreview(
        PrintPreviewMsg::SetIncludeParameterSets(false),
    ));
    assert!(!options(&app).include_parameter_sets);
    assert!(generation(&app) > before, "a new raster job");
}

/// Compile Masks and Differential Pairs start off (Caner, 2026-11-25) and
/// each tick redraws the preview.
#[test]
fn ticking_compile_masks_or_diff_pairs_redraws_the_preview() {
    let mut app = open_preview("/w/directive_toggles_masks_pairs");
    assert!(!options(&app).include_compile_masks);
    assert!(!options(&app).include_diff_pairs);
    let before = generation(&app);
    let _ = app.update(Message::PrintPreview(
        PrintPreviewMsg::SetIncludeCompileMasks(true),
    ));
    assert!(options(&app).include_compile_masks);
    let after_masks = generation(&app);
    assert!(after_masks > before, "a new raster job");
    let _ = app.update(Message::PrintPreview(
        PrintPreviewMsg::SetIncludeDiffPairs(true),
    ));
    assert!(options(&app).include_diff_pairs);
    assert!(generation(&app) > after_masks, "a new raster job");
}

#[test]
fn a_toggle_the_page_does_not_draw_keeps_the_preview() {
    let mut app = open_preview("/w/directive_toggles_probes");
    let before = generation(&app);
    let _ = app.update(Message::PrintPreview(PrintPreviewMsg::SetIncludeProbes(
        false,
    )));
    assert!(!options(&app).include_probes);
    assert_eq!(generation(&app), before, "no raster job");
}
