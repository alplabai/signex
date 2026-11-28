//! A stitch problem is said once per user action, also when the sheet has a
//! Global label and the action sets off the net index build its cross
//! references read (#1273), which would otherwise say it again.

use uuid::Uuid;

use super::{app_with_missing_child, diagnostic_count};

#[test]
fn print_preview_says_a_stitch_problem_once_with_the_cross_references_build() {
    let marker = format!("gone-preview-{}.snxsch", Uuid::now_v7());
    let _ = crate::diagnostics::init_logging();
    let before = diagnostic_count(&marker);
    let mut app = app_with_missing_child(&marker);
    let task = app.handle_print_preview_requested();
    crate::app::handlers::menu::export::settle(&mut app, task);
    assert!(app.document_view.preview.is_some(), "the preview opens");
    assert_eq!(
        diagnostic_count(&marker) - before,
        1,
        "the stitch warning reaches the user once for the preview"
    );
}
