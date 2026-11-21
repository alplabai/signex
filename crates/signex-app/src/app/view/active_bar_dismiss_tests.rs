//! An Active Bar flyout closes on a press outside it and on Esc, however it
//! was opened (#2377): a right-click on its group button or a left click on
//! the Filter button. The closing press goes no further, so it never starts
//! a selection box or a placement.
//!
//! Driven through the whole main-window view, so the press meets the layers
//! `collect_overlays` builds around the flyout.

use iced::mouse::{Button, Event as MouseEvent};
use iced::{Point, Size};

use super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg};
use crate::app::InputMsg;
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_showing;
use crate::layout_probe::{messages_for_events, messages_on_click};

const WINDOW: Size = Size::new(1400.0, 900.0);

fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_showing(|_| {});
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    (app, main)
}

fn press(button: Button) -> iced::Event {
    iced::Event::Mouse(MouseEvent::ButtonPressed(button))
}

fn release(button: Button) -> iced::Event {
    iced::Event::Mouse(MouseEvent::ButtonReleased(button))
}

fn left_click() -> [iced::Event; 2] {
    [press(Button::Left), release(Button::Left)]
}

/// A canvas point below-left of every flyout, clear of the zoom pill.
fn outside(app: &Signex) -> Point {
    let canvas = app.canvas_bounds();
    Point::new(
        canvas.x + canvas.width * 0.2,
        canvas.y + canvas.height * 0.85,
    )
}

/// The window-space centre of the drawn button that opens `menu`.
fn trigger_centre(app: &Signex, menu: ActiveBarMenu) -> Point {
    let tool = app.interaction_state.current_tool;
    let tid = app.ui_state.effective_theme_id();
    let x = crate::active_bar::drawn_trigger_centres_for(tool, WINDOW.width, tid)
        .into_iter()
        .find_map(|(m, centre)| (m == menu).then_some(centre))
        .unwrap_or_else(|| panic!("no drawn button opens {menu:?}"));
    let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
    let y = app.canvas_bounds().y
        + crate::active_bar::bar_top_margin()
        + crate::active_bar::bar_height_for(&metrics) / 2.0;
    Point::new(x, y)
}

fn run(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Right-click the button behind `menu` through the view, as the user does.
fn right_click_open(app: &mut Signex, main: iced::window::Id, menu: ActiveBarMenu) {
    let at = trigger_centre(app, menu);
    let pressed = messages_for_events(app.view(main), WINDOW, at, &[press(Button::Right)]);
    run(app, pressed);
    let released = messages_for_events(app.view(main), WINDOW, at, &[release(Button::Right)]);
    run(app, released);
    assert_eq!(
        app.interaction_state.active_bar_menu,
        Some(menu),
        "the right-click did not open {menu:?}"
    );
}

fn closes(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::ActiveBar(ActiveBarMsg::CloseMenus)))
}

fn reached_the_canvas(messages: &[Message]) -> bool {
    messages.iter().any(|m| {
        matches!(
            m,
            Message::CanvasEvent(CanvasEvent::Clicked { .. })
                | Message::CanvasEvent(CanvasEvent::BoxSelect { .. })
        )
    })
}

/// A left click outside the open flyout closes it and goes no further.
fn assert_outside_click_closes(app: &mut Signex, main: iced::window::Id) {
    let messages = messages_for_events(app.view(main), WINDOW, outside(app), &left_click());
    assert!(
        closes(&messages),
        "the press outside did not close the flyout: {messages:?}"
    );
    assert!(
        !reached_the_canvas(&messages),
        "the closing press reached the canvas: {messages:?}"
    );
    run(app, messages);
    assert!(app.interaction_state.active_bar_menu.is_none());
}

#[test]
fn a_right_click_opened_text_flyout_closes_on_a_canvas_press() {
    let (mut app, main) = app();
    right_click_open(&mut app, main, ActiveBarMenu::TextTools);
    assert_outside_click_closes(&mut app, main);
}

#[test]
fn every_right_click_opened_flyout_closes_on_a_canvas_press() {
    let tool = Tool::Select;
    let tid = signex_types::theme::ThemeId::GraphiteLight;
    for (menu, _) in crate::active_bar::drawn_trigger_centres_for(tool, WINDOW.width, tid) {
        let (mut app, main) = app();
        right_click_open(&mut app, main, menu);
        assert_outside_click_closes(&mut app, main);
    }
}

#[test]
fn a_left_click_opened_flyout_closes_on_a_canvas_press() {
    let (mut app, main) = app();
    let at = trigger_centre(&app, ActiveBarMenu::Filter);
    let opened = messages_for_events(app.view(main), WINDOW, at, &left_click());
    run(&mut app, opened);
    assert_eq!(
        app.interaction_state.active_bar_menu,
        Some(ActiveBarMenu::Filter)
    );
    assert_outside_click_closes(&mut app, main);
}

/// With a placement tool armed, the press that closes the flyout places
/// nothing and leaves the tool as it was.
#[test]
fn the_closing_press_does_not_place_with_a_tool_armed() {
    let (mut app, main) = app();
    let _ = app.update(Message::Tool(crate::app::ToolMessage::SelectTool(
        Tool::Wire,
    )));
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
        ActiveBarMenu::TextTools,
    )));
    assert_outside_click_closes(&mut app, main);
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    assert!(!app.interaction_state.wire_drawing);
    assert!(app.interaction_state.wire_points.is_empty());
}

#[test]
fn esc_closes_a_right_click_opened_flyout() {
    let (mut app, main) = app();
    right_click_open(&mut app, main, ActiveBarMenu::TextTools);
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(app.interaction_state.active_bar_menu.is_none());
}

/// Esc closes the flyout only: an armed tool stays armed.
#[test]
fn esc_closes_the_flyout_before_leaving_the_tool() {
    let (mut app, _main) = app();
    let _ = app.update(Message::Tool(crate::app::ToolMessage::SelectTool(
        Tool::Wire,
    )));
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
        ActiveBarMenu::TextTools,
    )));
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(app.interaction_state.active_bar_menu.is_none());
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
}

/// A press on a flyout row picks it; the dismiss layer behind the card does
/// not see it.
#[test]
fn a_press_inside_the_flyout_picks_the_row() {
    let (mut app, main) = app();
    right_click_open(&mut app, main, ActiveBarMenu::TextTools);
    let messages = messages_on_click(app.view(main), WINDOW, "Text String");
    assert!(
        messages.iter().any(|m| matches!(
            m,
            Message::ActiveBar(ActiveBarMsg::Action(ActiveBarAction::PlaceTextString))
        )),
        "the row was not picked: {messages:?}"
    );
    assert!(!closes(&messages), "the row press dismissed first: {messages:?}");
    run(&mut app, messages);
    assert!(app.interaction_state.active_bar_menu.is_none());
}

/// The real event loop, one frame per call: the widget tree (and its
/// overlays, such as a button's tooltip) persists from frame to frame, and
/// every message the frame publishes runs through `Signex::update`.
struct EventLoop {
    renderer: iced::Renderer,
    cache: Option<iced_runtime::user_interface::Cache>,
    messages: Vec<Message>,
}

impl EventLoop {
    fn new() -> Self {
        Self {
            renderer: crate::layout_probe::headless_renderer(),
            cache: None,
            messages: Vec::new(),
        }
    }

    fn frame(&mut self, app: &mut Signex, main: iced::window::Id, at: Point, event: iced::Event) {
        let mut ui = iced_runtime::UserInterface::build(
            app.view(main),
            WINDOW,
            self.cache.take().unwrap_or_default(),
            &mut self.renderer,
        );
        let mut bus = iced::advanced::shell::Bus::new();
        let waker = iced::advanced::shell::Waker::noop();
        let cursor = iced::mouse::Cursor::Available(at);
        let redraw = iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ));
        let _ = ui.update(
            &iced::window::Headless,
            &waker,
            &[event, redraw],
            cursor,
            &mut self.renderer,
            &mut bus,
        );
        self.cache = Some(ui.into_cache());
        let messages: Vec<Message> = bus.drain().map(|(message, _)| message).collect();
        self.messages.extend(messages.iter().cloned());
        run(app, messages);
    }

    fn move_to(&mut self, app: &mut Signex, main: iced::window::Id, at: Point) {
        let moved = iced::Event::Mouse(MouseEvent::CursorMoved { position: at });
        self.frame(app, main, at, moved);
    }
}

/// The maintainer's report (2026-11-21): the Text flyout opened by a
/// right-click on its button stayed open when the canvas was clicked. Driven
/// through frames that keep the widget tree, as the running app does.
#[test]
fn a_right_click_opened_flyout_closes_on_a_canvas_click_in_the_event_loop() {
    let (mut app, main) = app();
    let mut ev = EventLoop::new();
    let at = trigger_centre(&app, ActiveBarMenu::TextTools);
    ev.move_to(&mut app, main, at);
    ev.frame(&mut app, main, at, press(Button::Right));
    ev.frame(&mut app, main, at, release(Button::Right));
    assert_eq!(
        app.interaction_state.active_bar_menu,
        Some(ActiveBarMenu::TextTools)
    );
    let target = outside(&app);
    for step in 1..=4 {
        let t = step as f32 / 4.0;
        let p = Point::new(at.x + (target.x - at.x) * t, at.y + (target.y - at.y) * t);
        ev.move_to(&mut app, main, p);
    }
    ev.messages.clear();
    ev.frame(&mut app, main, target, press(Button::Left));
    ev.frame(&mut app, main, target, release(Button::Left));
    assert!(
        app.interaction_state.active_bar_menu.is_none(),
        "the flyout stayed open: {:?}",
        ev.messages
    );
    assert!(
        !reached_the_canvas(&ev.messages),
        "the closing press reached the canvas: {:?}",
        ev.messages
    );
}

impl EventLoop {
    fn click(&mut self, app: &mut Signex, main: iced::window::Id, at: Point, button: Button) {
        self.move_to(app, main, at);
        self.frame(app, main, at, press(button));
        self.frame(app, main, at, release(button));
    }
}

/// The report's flow end to end: Text String picked from the flyout, the
/// flyout opened again by a right-click while that tool is armed, then a
/// click on empty canvas at several spots around the flyout.
#[test]
fn a_flyout_reopened_with_its_tool_armed_closes_on_a_canvas_click() {
    let probe = |at_of: &dyn Fn(&Signex, iced::Rectangle) -> Point| {
        let (mut app, main) = app();
        let mut ev = EventLoop::new();
        let trigger = trigger_centre(&app, ActiveBarMenu::TextTools);
        ev.click(&mut app, main, trigger, Button::Right);
        let texts = crate::layout_probe::lay_out_texts(app.view(main), WINDOW);
        let row = crate::layout_probe::find(&texts, "Text String").bounds;
        ev.click(&mut app, main, row.center(), Button::Left);
        assert!(app.interaction_state.active_bar_menu.is_none());
        let away = outside(&app);
        ev.move_to(&mut app, main, away);
        let trigger = trigger_centre(&app, ActiveBarMenu::TextTools);
        ev.click(&mut app, main, trigger, Button::Right);
        assert_eq!(
            app.interaction_state.active_bar_menu,
            Some(ActiveBarMenu::TextTools),
            "the second right-click did not open the flyout: {:?}",
            ev.messages
        );
        let target = at_of(&app, row);
        ev.messages.clear();
        ev.click(&mut app, main, target, Button::Left);
        assert!(
            app.interaction_state.active_bar_menu.is_none(),
            "the flyout stayed open after a click at {target:?}: {:?}",
            ev.messages
        );
    };
    probe(&|app, _| outside(app));
    probe(&|_, row| Point::new(row.center_x(), row.y + 300.0));
    probe(&|_, row| Point::new(row.x + row.width + 200.0, row.y + 40.0));
    probe(&|_, row| Point::new(row.x - 200.0, row.y + 120.0));
}

/// Esc closes the board's Active Bar dropdown too.
#[test]
fn esc_closes_the_board_active_bar_dropdown() {
    let (mut app, _main) = app();
    app.interaction_state.pcb_bar_menu = Some(crate::app::PcbBarMenu::Align);
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert_eq!(app.interaction_state.pcb_bar_menu, None);
}
