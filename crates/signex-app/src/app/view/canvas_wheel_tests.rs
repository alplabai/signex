//! The mouse wheel and the touchpad reach the schematic canvas through the
//! whole main-window view, Graphite overlays included.
//!
//! The Graphite zoom pill is positioned by a window-sized `Translate`, and
//! `collect_overlays` wrapped every Active Bar layer in a wheel-capturing
//! `mouse_area`. That `mouse_area` took the bounds of the `Translate`, so
//! every wheel event in the window stopped at the pill's layer: no wheel zoom,
//! no two-finger pan and no pinch (Ctrl+wheel on Windows) on the canvas.

use iced::keyboard::Modifiers;
use iced::mouse::ScrollDelta;
use iced::{Point, Size};

use super::*;
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_with_selected_wire;
use crate::layout_probe::{lay_out_texts, messages_for_events, messages_on_scroll_at};

const WINDOW: Size = Size::new(1400.0, 900.0);

fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    (app, main)
}

fn canvas_centre(app: &Signex) -> Point {
    app.canvas_bounds().center()
}

fn camera(app: &Signex) -> (f32, f32, f32) {
    app.interaction_state.active_canvas().live_camera()
}

fn wheel(delta: ScrollDelta) -> iced::Event {
    iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta })
}

fn moved_view(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::ViewMoved)))
}

/// The fixture paints the overlays this regression is about.
#[test]
fn the_fixture_shows_the_zoom_pill() {
    let (app, main) = app();
    let texts = lay_out_texts(app.view(main), WINDOW);
    assert!(
        texts.iter().any(|t| t.text == "Fit"),
        "no zoom pill: {texts:#?}"
    );
}

#[test]
fn a_mouse_wheel_over_the_canvas_zooms_it() {
    let (app, main) = app();
    let before = camera(&app);
    let messages = messages_on_scroll_at(
        app.view(main),
        WINDOW,
        canvas_centre(&app),
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    assert!(moved_view(&messages), "the wheel never reached the canvas");
    let after = camera(&app);
    assert!(after.2 > before.2, "no zoom: {before:?} -> {after:?}");
}

#[test]
fn a_two_finger_scroll_pans_the_canvas() {
    let (mut app, main) = app();
    app.ui_state.trackpad_pan = true;
    let before = camera(&app);
    let messages = messages_on_scroll_at(
        app.view(main),
        WINDOW,
        canvas_centre(&app),
        ScrollDelta::Pixels { x: 12.0, y: -30.0 },
    );
    assert!(moved_view(&messages), "the scroll never reached the canvas");
    let after = camera(&app);
    assert_eq!(after.2, before.2, "a pan keeps the zoom");
    assert_ne!((after.0, after.1), (before.0, before.1), "no pan");
}

/// A Windows precision touchpad sends a pinch as Ctrl+wheel: fractional
/// line deltas, or pixel deltas through a driver that reports them. Both zoom,
/// even with two-finger pan on.
#[test]
fn a_pinch_as_ctrl_wheel_zooms_the_canvas() {
    for delta in [
        ScrollDelta::Lines { x: 0.0, y: 0.25 },
        ScrollDelta::Pixels { x: 0.0, y: 40.0 },
    ] {
        let (mut app, main) = app();
        app.ui_state.trackpad_pan = true;
        let at = canvas_centre(&app);
        let before = camera(&app);
        let messages = messages_for_events(
            app.view(main),
            WINDOW,
            at,
            &[
                iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(Modifiers::CTRL)),
                wheel(delta),
            ],
        );
        assert!(moved_view(&messages), "{delta:?} never reached the canvas");
        let after = camera(&app);
        assert!(
            after.2 > before.2,
            "{delta:?} did not zoom: {before:?} -> {after:?}"
        );
    }
}

/// The y of the middle of the floating Active Bar, in window coordinates.
fn bar_middle_y(app: &Signex) -> f32 {
    let m = crate::active_bar::metrics_for(true);
    app.canvas_bounds().y
        + crate::active_bar::bar_top_margin()
        + crate::active_bar::bar_height_for(&m) / 2.0
}

/// The pill and the Active Bar still stop the wheel over their own bounds.
#[test]
fn the_pill_and_the_bar_still_stop_the_wheel_over_themselves() {
    let (app, main) = app();
    let (w, h) = crate::canvas::zoom_pill::pill_size();
    let (x, y) = overlays::zoom_pill_origin(app.canvas_bounds());
    let pill = iced::Rectangle::new(Point::new(x, y), Size::new(w, h));
    let texts = lay_out_texts(app.view(main), WINDOW);
    let fit = texts.iter().find(|t| t.text == "Fit").expect("the pill");
    assert!(
        pill.contains(fit.bounds.center()),
        "the pill is not at {pill:?}"
    );
    let over_bar = Point::new(WINDOW.width / 2.0, bar_middle_y(&app));
    for at in [pill.center(), over_bar] {
        let before = camera(&app);
        let messages = messages_on_scroll_at(
            app.view(main),
            WINDOW,
            at,
            ScrollDelta::Lines { x: 0.0, y: 1.0 },
        );
        assert!(!moved_view(&messages), "the wheel went through at {at:?}");
        assert_eq!(camera(&app), before, "the canvas moved under {at:?}");
    }
}

/// The bar's row is a `Fill`-wide layer: the canvas beside the bar, at the
/// bar's height, still takes the wheel.
#[test]
fn the_canvas_beside_the_bar_takes_the_wheel() {
    let (app, main) = app();
    let beside = Point::new(app.canvas_bounds().x + 24.0, bar_middle_y(&app));
    let before = camera(&app);
    let messages = messages_on_scroll_at(
        app.view(main),
        WINDOW,
        beside,
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    assert!(moved_view(&messages), "the bar's row took the wheel");
    assert!(camera(&app).2 > before.2);
}

fn scroll_mode_events(
    app: &mut Signex,
    main: iced::window::Id,
    mods: Modifiers,
    delta: ScrollDelta,
) {
    app.ui_state.wheel_mode = crate::canvas::WheelMode::Scroll;
    let at = canvas_centre(app);
    let messages = messages_for_events(
        app.view(main),
        WINDOW,
        at,
        &[
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)),
            wheel(delta),
        ],
    );
    assert!(moved_view(&messages), "{delta:?} never reached the canvas");
}

/// "Mouse wheel: Scroll": a plain wheel notch pans vertically.
#[test]
fn in_scroll_mode_the_wheel_pans_vertically() {
    let (mut app, main) = app();
    let before = camera(&app);
    scroll_mode_events(
        &mut app,
        main,
        Modifiers::empty(),
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    let after = camera(&app);
    assert_eq!(after.2, before.2, "a pan keeps the zoom");
    assert_eq!(after.0, before.0, "no sideways move");
    assert_ne!(after.1, before.1, "no vertical pan");
}

/// A two-finger scroll pans in scroll mode without "Two-finger scroll pans".
#[test]
fn in_scroll_mode_a_two_finger_scroll_pans() {
    let (mut app, main) = app();
    assert!(!app.ui_state.trackpad_pan);
    let before = camera(&app);
    scroll_mode_events(
        &mut app,
        main,
        Modifiers::empty(),
        ScrollDelta::Pixels { x: 0.0, y: -30.0 },
    );
    let after = camera(&app);
    assert_eq!(after.2, before.2);
    assert_ne!(after.1, before.1);
}

#[test]
fn in_scroll_mode_shift_wheel_pans_horizontally() {
    let (mut app, main) = app();
    let before = camera(&app);
    scroll_mode_events(
        &mut app,
        main,
        Modifiers::SHIFT,
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    let after = camera(&app);
    assert_eq!(after.2, before.2);
    assert_ne!(after.0, before.0, "no sideways pan");
    assert_eq!(after.1, before.1, "no vertical move");
}

#[test]
fn in_scroll_mode_ctrl_wheel_zooms() {
    let (mut app, main) = app();
    let before = camera(&app);
    scroll_mode_events(
        &mut app,
        main,
        Modifiers::CTRL,
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    assert!(camera(&app).2 > before.2);
}
