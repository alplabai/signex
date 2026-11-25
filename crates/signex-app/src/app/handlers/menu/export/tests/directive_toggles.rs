//! #2656 — "Blankets" and "Parameter Sets" decide what the page draws, so
//! ticking either one redraws the open print preview, as No-ERC Markers and
//! Notes do. A toggle the page does not draw yet (Probes) does not.

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
