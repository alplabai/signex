//! #1294 — the print preview draws not-fitted parts as the user's "Cross out
//! not-fitted parts" preference says, read at each render, not when the
//! dialog opened.

use super::net_text_hierarchy::placed_app;

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
