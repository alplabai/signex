//! #2345 — maximising the window left the sheet in the top-left corner. A
//! resize now refits a view still at its fit, and keeps the centre and the
//! zoom of a view the user moved.

use iced::Point;

use super::super::super::*;
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_with_selected_wire;
use crate::menu_bar::MenuMessage;

const WINDOW: (f32, f32) = (1400.0, 900.0);
const MAXIMISED: (f32, f32) = (1920.0, 1040.0);

/// The fixture in a `WINDOW`-sized main window, its canvas laid out there.
fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = WINDOW;
    let bounds = app.canvas_bounds();
    // Stands in for the canvas event of a drawn frame.
    app.interaction_state.active_canvas().set_viewport(bounds);
    app.interaction_state.pcb_canvas.set_viewport(bounds);
    (app, main)
}

fn resize(app: &mut Signex, main: iced::window::Id, (w, h): (f32, f32)) {
    let _ = app.update(Message::Window(WindowMsg::WindowResizedFor(main, w, h)));
}

/// View > Fit All, resolved by the canvas at its current layout.
fn fit_all(app: &mut Signex) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));
    let bounds = app.canvas_bounds();
    assert!(
        app.interaction_state
            .active_canvas()
            .resolve_pending_fit(bounds),
        "Fit All queued a fit"
    );
}

fn camera(app: &Signex) -> (f32, f32, f32) {
    app.interaction_state.active_canvas().live_camera()
}

/// The world point at the centre of the canvas laid out for the window now.
fn centre_world(app: &Signex) -> Point {
    let bounds = app.canvas_bounds();
    app.interaction_state
        .active_canvas()
        .screen_to_world(Point::new(bounds.width / 2.0, bounds.height / 2.0))
}

#[test]
fn maximising_refits_a_fitted_view() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let fitted_small = camera(&app);

    resize(&mut app, main, MAXIMISED);

    let followed = camera(&app);
    assert_ne!(followed, fitted_small, "the view stayed where it was");
    fit_all(&mut app);
    assert_eq!(followed, camera(&app), "not the fit of the new size");
}

#[test]
fn restoring_a_maximised_fitted_view_fits_it_again() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let fitted_small = camera(&app);

    resize(&mut app, main, MAXIMISED);
    resize(&mut app, main, WINDOW);

    assert_eq!(camera(&app), fitted_small);
}

#[test]
fn maximising_keeps_the_centre_and_zoom_of_a_zoomed_view() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let (centre, scale) = (centre_world(&app), camera(&app).2);

    resize(&mut app, main, MAXIMISED);

    let after = centre_world(&app);
    assert!(
        (after.x - centre.x).abs() < 1e-3 && (after.y - centre.y).abs() < 1e-3,
        "the centre moved: {centre:?} -> {after:?}"
    );
    assert_eq!(camera(&app).2, scale, "the zoom changed");
}

#[test]
fn a_view_never_fitted_keeps_its_centre_too() {
    let (mut app, main) = app();
    let (centre, scale) = (centre_world(&app), camera(&app).2);

    resize(&mut app, main, MAXIMISED);

    let after = centre_world(&app);
    assert!(
        (after.x - centre.x).abs() < 1e-3 && (after.y - centre.y).abs() < 1e-3,
        "the centre moved: {centre:?} -> {after:?}"
    );
    assert_eq!(camera(&app).2, scale);
}

#[test]
fn a_resize_to_the_same_size_leaves_the_view() {
    let (mut app, main) = app();
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let before = camera(&app);

    resize(&mut app, main, WINDOW);

    assert_eq!(camera(&app), before);
}

#[test]
fn maximising_refits_a_fitted_board_view() {
    let (mut app, main) = app();
    let board = iced::Rectangle::new(Point::new(10.0, 5.0), iced::Size::new(80.0, 50.0));
    let pcb = &app.interaction_state.pcb_canvas;
    pcb.pending_fit.set(Some(board));
    assert!(pcb.resolve_pending_fit(app.canvas_bounds()));

    resize(&mut app, main, MAXIMISED);

    let expected = crate::pcb_canvas::PcbCanvas::new();
    expected.pending_fit.set(Some(board));
    assert!(expected.resolve_pending_fit(app.canvas_bounds()));
    assert_eq!(
        app.interaction_state.pcb_canvas.live_camera(),
        expected.live_camera()
    );
}
