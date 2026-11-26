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

/// A window locked by a detached dialog keeps the wheel from what is under it.
#[test]
fn a_locked_window_keeps_the_wheel_from_the_canvas() {
    let (app, _) = app();
    let under = iced::widget::mouse_area(
        iced::widget::Space::new()
            .width(iced::Length::Fill)
            .height(iced::Length::Fill),
    )
    .on_scroll(|_| Message::CanvasEvent(CanvasEvent::ViewMoved));
    let view = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(under.boxed())
        .push(app.locked_window_layer());
    let messages = messages_on_scroll_at(
        view.boxed(),
        WINDOW,
        Point::new(WINDOW.width / 2.0, WINDOW.height / 2.0),
        ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    assert!(
        !moved_view(&messages),
        "the wheel went through the lock: {messages:?}"
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
    let m = signex_widgets::active_bar::Metrics::GRAPHITE;
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

// The zoom pill's presets (Graphite mockup #64).

fn pill_rect(app: &Signex) -> iced::Rectangle {
    let (w, h) = crate::canvas::zoom_pill::pill_size();
    let (x, y) = overlays::zoom_pill_origin(app.canvas_bounds());
    iced::Rectangle::new(Point::new(x, y), Size::new(w, h))
}

/// Centre of the laid-out text `label` that `area` holds.
fn text_centre_in(
    app: &Signex,
    main: iced::window::Id,
    label: &str,
    area: iced::Rectangle,
) -> Point {
    let texts = lay_out_texts(app.view(main), WINDOW);
    texts
        .iter()
        .find(|t| t.text == label && area.contains(t.bounds.center()))
        .unwrap_or_else(|| panic!("no {label:?} in {area:?}: {texts:#?}"))
        .bounds
        .center()
}

/// The window above the pill, where the preset menu opens.
fn above_the_pill(app: &Signex) -> iced::Rectangle {
    let pill = pill_rect(app);
    iced::Rectangle::new(Point::ORIGIN, Size::new(WINDOW.width, pill.y))
}

fn click_at(app: &Signex, main: iced::window::Id, at: Point) -> Vec<Message> {
    crate::layout_probe::messages_on_click_at(app.view(main), WINDOW, None, at)
}

fn open_presets(app: &mut Signex) {
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    assert!(app.interaction_state.zoom_presets_open);
}

/// The pill keeps its size and corner; the hairline is inside its width.
#[test]
fn the_pill_keeps_its_corner_and_holds_fit() {
    let (app, main) = app();
    let pill = pill_rect(&app);
    let canvas = app.canvas_bounds();
    assert_eq!(pill.x + pill.width, canvas.x + canvas.width - 12.0);
    assert_eq!(pill.y + pill.height, canvas.y + canvas.height - 12.0);
    let fit = text_centre_in(&app, main, "Fit", pill);
    assert!(pill.contains(fit));
}

/// A click on the percentage opens the presets.
#[test]
fn clicking_the_percentage_opens_the_presets() {
    let (app, main) = app();
    let pill = pill_rect(&app);
    let texts = lay_out_texts(app.view(main), WINDOW);
    let readout = texts
        .iter()
        .find(|t| t.text.ends_with('%') && pill.contains(t.bounds.center()))
        .expect("the pill's percentage");
    let messages = click_at(&app, main, readout.bounds.center());
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Ui(UiMsg::ZoomPresetsToggled))),
        "{messages:?}"
    );
}

/// The menu opens above the pill; picking 200% doubles the 100% scale about
/// the canvas centre and closes the menu.
#[test]
fn picking_200_percent_doubles_the_100_percent_scale() {
    let (mut app, main) = app();
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(100)));
    let at_100 = camera(&app).2;
    open_presets(&mut app);
    let row = text_centre_in(&app, main, "200%", above_the_pill(&app));
    for message in click_at(&app, main, row) {
        let _ = app.update(message);
    }
    let at_200 = camera(&app).2;
    assert!((at_200 / at_100 - 2.0).abs() < 1e-4, "{at_100} -> {at_200}");
    assert!(
        !app.interaction_state.zoom_presets_open,
        "the pick left the menu open"
    );
}

/// Esc closes the menu and leaves the camera alone.
#[test]
fn esc_closes_the_presets() {
    let (mut app, _main) = app();
    open_presets(&mut app);
    let before = camera(&app);
    let _ = app.update(Message::Input(crate::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(!app.interaction_state.zoom_presets_open);
    assert_eq!(camera(&app), before);
}

/// A click on the canvas beside the menu closes it and goes no further.
#[test]
fn an_outside_click_closes_the_presets() {
    let (mut app, main) = app();
    open_presets(&mut app);
    let messages = click_at(&app, main, canvas_centre(&app));
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Ui(UiMsg::ZoomPresetsDismissed))),
        "{messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::Clicked { .. }))),
        "the click went through to the canvas: {messages:?}"
    );
}

/// With nothing selected Fit Selection is greyed: a click on it runs nothing.
#[test]
fn fit_selection_does_nothing_without_a_selection() {
    let (mut app, main) = app();
    app.interaction_state.active_canvas_mut().selected.clear();
    open_presets(&mut app);
    let row = text_centre_in(&app, main, "Fit Selection", above_the_pill(&app));
    let messages = click_at(&app, main, row);
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::Ui(UiMsg::ZoomPresetCommandPicked(_)))),
        "{messages:?}"
    );
}

/// The menu belongs to the sheet it opened over: switching to another tab
/// closes it, so it does not hang over the next sheet's pill.
#[test]
fn a_tab_switch_closes_the_presets() {
    let (mut app, _main) = app();
    let other = std::path::PathBuf::from("other.snxsch");
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    app.document_state.engines.insert(other.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "other".to_string(),
        path: other,
        project_id: None,
        kind: TabKind::Schematic,
    });
    open_presets(&mut app);
    let _ = app.step_document_tab(true);
    assert!(
        !app.interaction_state.zoom_presets_open,
        "the menu stayed open over the next tab"
    );
}
