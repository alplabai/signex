//! #1941 - a new project's sheet opens fitted and centred in the canvas, using
//! the canvas's real size once it is known.

use signex_app::app::{FileMsg, Message, Signex};
use tempfile::TempDir;

#[test]
fn new_project_sheet_fits_the_canvas_once_its_size_is_known() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::NewProject(Some(tmp.path().join("Fit.snxprj")))),
    );
    assert!(app.document_state.error_notice.is_none());

    // The canvas is laid out at a size the sheet was never loaded against.
    let viewport = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(1600.0, 900.0));
    let canvas = app.interaction_state.active_canvas();
    assert!(
        canvas.resolve_pending_fit(viewport),
        "the first show queues a fit"
    );

    let (w, h) = (canvas.paper_width_mm, canvas.paper_height_mm);
    let (x, y, scale) = canvas.live_camera();
    // Fit Document: the sheet fills the canvas inside its margin on the
    // tighter axis, and is centred on both.
    let fits_w = (viewport.width - 80.0) / w;
    let fits_h = (viewport.height - 80.0) / h;
    assert!(
        (scale - fits_w.min(fits_h)).abs() < 1e-3,
        "scale {scale} vs {}",
        fits_w.min(fits_h)
    );
    assert!((x + w * scale / 2.0 - viewport.width / 2.0).abs() < 1e-2);
    assert!((y + h * scale / 2.0 - viewport.height / 2.0).abs() < 1e-2);
    assert!(
        !canvas.resolve_pending_fit(viewport),
        "the fit is consumed, not re-applied over the user's pan"
    );
}
