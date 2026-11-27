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
    // The first-run docks, not a layout another test saved to the shared
    // test prefs: the left dock drag below needs a left dock.
    app.document_view.dock = crate::dock::graphite_first_run();
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
    // `update` cannot see the laid-out canvas: it adds the growth of
    // `canvas_bounds()` to the size last laid out, and the next canvas
    // event records the real one. Laid out at `canvas_bounds()` here, the
    // two agree.
    assert_eq!(
        app.interaction_state.active_canvas().viewport_size(),
        app.canvas_bounds().size(),
        "the recorded viewport is the new canvas size"
    );
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
    // No sheet fit owed either: an owed fit would win over the follow.
    assert!(!app.interaction_state.active_canvas().has_pending_fit());
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

/// Drag the left dock's splitter `dx` px wider, through `Signex::update`.
fn drag_left_dock(app: &mut Signex, main: iced::window::Id, dx: f32) {
    let before = app.canvas_bounds().width;
    let _ = app.update(Message::Ui(UiMsg::DragStart(DragTarget::LeftPanel)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(300.0, 400.0)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(300.0 + dx, 400.0)));
    let _ = app.update(Message::Ui(UiMsg::DragEnd(main)));
    assert_ne!(
        app.canvas_bounds().width,
        before,
        "the drag resized the canvas"
    );
}

#[test]
fn dragging_a_dock_refits_a_fitted_view() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let fitted_before = camera(&app);

    drag_left_dock(&mut app, main, 120.0);

    let followed = camera(&app);
    assert_ne!(followed, fitted_before, "the view stayed where it was");
    fit_all(&mut app);
    assert_eq!(followed, camera(&app), "not the fit of the new size");
}

#[test]
fn dragging_a_dock_keeps_the_centre_and_zoom_of_a_zoomed_view() {
    let (mut app, main) = app();
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let (centre, scale) = (centre_world(&app), camera(&app).2);

    drag_left_dock(&mut app, main, 120.0);

    let after = centre_world(&app);
    assert!(
        (after.x - centre.x).abs() < 1e-3 && (after.y - centre.y).abs() < 1e-3,
        "the centre moved: {centre:?} -> {after:?}"
    );
    assert_eq!(camera(&app).2, scale, "the zoom changed");
}

const UNDOCKED: (f32, f32) = (1000.0, 700.0);
const UNDOCKED_VIEW: iced::Size = iced::Size::new(800.0, 600.0);

/// A main window plus an undocked schematic window whose canvas is laid out
/// at `UNDOCKED_VIEW` and zoomed in.
fn undocked_window(recorded: bool) -> (Signex, iced::window::Id) {
    let (mut app, _) = app();
    let window = iced::window::Id::unique();
    let slot = crate::canvas::CanvasSlot::new();
    slot.set_viewport(iced::Rectangle::with_size(UNDOCKED_VIEW));
    assert!(slot.zoom_about_centre(true, 1.5));
    app.interaction_state.canvases.insert(window, slot);
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: std::path::PathBuf::from("halo-camera.snxsch"),
            title: String::new(),
        },
    );
    if recorded {
        app.ui_state.set_undocked_window_size(window, UNDOCKED);
        assert_eq!(app.ui_state.size_of_window(window), UNDOCKED);
    }
    (app, window)
}

fn undocked_centre(app: &Signex, window: iced::window::Id) -> (Point, f32) {
    let slot = &app.interaction_state.canvases[&window];
    let size = slot.viewport_size();
    let centre = slot.screen_to_world(Point::new(size.width / 2.0, size.height / 2.0));
    (centre, slot.live_camera().2)
}

#[test]
fn resizing_an_undocked_window_keeps_its_centre_and_zoom() {
    let (mut app, window) = undocked_window(true);
    let (centre, scale) = undocked_centre(&app, window);

    resize(&mut app, window, (UNDOCKED.0 + 300.0, UNDOCKED.1 + 200.0));

    let slot = &app.interaction_state.canvases[&window];
    assert_eq!(
        slot.viewport_size(),
        iced::Size::new(UNDOCKED_VIEW.width + 300.0, UNDOCKED_VIEW.height + 200.0),
        "the canvas grew by the window's growth"
    );
    let (after, after_scale) = undocked_centre(&app, window);
    assert!(
        (after.x - centre.x).abs() < 1e-3 && (after.y - centre.y).abs() < 1e-3,
        "the centre moved: {centre:?} -> {after:?}"
    );
    assert_eq!(after_scale, scale);
}

#[test]
fn an_undocked_window_with_no_earlier_size_leaves_its_camera() {
    let (mut app, window) = undocked_window(false);
    let before = app.interaction_state.canvases[&window].live_camera();

    resize(&mut app, window, (UNDOCKED.0 + 300.0, UNDOCKED.1 + 200.0));

    let slot = &app.interaction_state.canvases[&window];
    assert_eq!(slot.live_camera(), before);
    assert_eq!(slot.viewport_size(), UNDOCKED_VIEW);
}

/// Windows reports a minimised window as 0x0. Measured from that, the restore
/// looked like a canvas twice the window's size and moved the view.
const MINIMISED: (f32, f32) = (0.0, 0.0);

#[test]
fn minimising_and_restoring_leaves_a_zoomed_view() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let before = camera(&app);

    resize(&mut app, main, MINIMISED);
    assert_eq!(camera(&app), before, "minimising moved the view");
    resize(&mut app, main, WINDOW);

    assert_eq!(camera(&app), before, "restoring moved the view");
    assert_eq!(
        app.interaction_state.active_canvas().viewport_size(),
        app.canvas_bounds().size()
    );
}

#[test]
fn minimising_and_restoring_leaves_a_fitted_view() {
    let (mut app, main) = app();
    fit_all(&mut app);
    let fitted = camera(&app);

    resize(&mut app, main, MINIMISED);
    assert_eq!(camera(&app), fitted, "minimising moved the view");
    resize(&mut app, main, WINDOW);

    assert_eq!(camera(&app), fitted, "restoring refitted to another size");
}

#[test]
fn minimising_and_restoring_an_undocked_window_leaves_its_camera() {
    let (mut app, window) = undocked_window(true);
    let before = app.interaction_state.canvases[&window].live_camera();

    resize(&mut app, window, MINIMISED);
    assert_eq!(
        app.ui_state.size_of_window(window),
        UNDOCKED,
        "the empty size became the base"
    );
    resize(&mut app, window, UNDOCKED);

    let slot = &app.interaction_state.canvases[&window];
    assert_eq!(slot.live_camera(), before);
    assert_eq!(slot.viewport_size(), UNDOCKED_VIEW);
}

/// A minimised main window's 0x0 is not stored: the window size and the
/// canvas viewport keep their real values, so the restore lays nothing out
/// again and no frame is drawn for an empty window size.
#[test]
fn a_minimised_main_window_keeps_its_size_and_viewport() {
    let (mut app, main) = app();
    let viewport = app.interaction_state.active_canvas().viewport_size();

    resize(&mut app, main, MINIMISED);

    assert_eq!(
        app.ui_state.window_size, WINDOW,
        "the empty size was stored"
    );
    assert_eq!(
        app.interaction_state.active_canvas().viewport_size(),
        viewport,
        "the canvas viewport followed the empty size"
    );
    assert_eq!(app.canvas_bounds().size(), viewport);
}
