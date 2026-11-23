//! A right-click menu closes on a click outside it, as in any desktop app
//! (#2349): a left click closes it and goes no further, a right click closes
//! it and opens the menu of the spot it landed on, and Esc closes it.
//!
//! Driven through the whole main-window view, so the layers
//! `collect_overlays` wraps around the menu are the ones the click meets.

use iced::mouse::{Button, Event as MouseEvent};
use iced::{Point, Size};

use super::*;
use crate::app::{ContextMenuMsg, InputMsg};
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_showing;
use crate::layout_probe::messages_for_events;

const WINDOW: Size = Size::new(1400.0, 900.0);

fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_showing(|_| {});
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    (app, main)
}

/// Where the menus open (near the canvas's top-left, below the Active Bar)
/// and a canvas point well clear of any card opened there.
fn menu_and_outside(app: &Signex) -> (Point, Point) {
    let canvas = app.canvas_bounds();
    let menu = Point::new(canvas.x + 40.0, canvas.y + 120.0);
    let outside = Point::new(
        canvas.x + canvas.width * 0.75,
        canvas.y + canvas.height * 0.5,
    );
    (menu, outside)
}

fn press(button: Button) -> iced::Event {
    iced::Event::Mouse(MouseEvent::ButtonPressed(button))
}

fn release(button: Button) -> iced::Event {
    iced::Event::Mouse(MouseEvent::ButtonReleased(button))
}

fn open_tree_menu(app: &mut Signex, at: Point) {
    app.interaction_state.last_mouse_pos = (at.x, at.y);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(None)));
    assert!(
        app.interaction_state.project_tree_context_menu.is_some(),
        "the fixture opened no Projects menu"
    );
}

fn clicked_the_canvas(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::Clicked { .. })))
}

/// A left click on the canvas beside the Projects menu closes the menu, and
/// the canvas never sees it: no click selects or deselects through it.
#[test]
fn an_outside_left_click_closes_the_menu_and_goes_no_further() {
    let (mut app, main) = app();
    let (menu, outside) = menu_and_outside(&app);
    let left_click = [press(Button::Left), release(Button::Left)];

    let control = messages_for_events(app.view(main), WINDOW, outside, &left_click);
    assert!(
        clicked_the_canvas(&control),
        "with no menu open the click must reach the canvas: {control:?}"
    );

    open_tree_menu(&mut app, menu);
    let messages = messages_for_events(app.view(main), WINDOW, outside, &left_click);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::CloseProjectTree))),
        "the click outside did not close the menu: {messages:?}"
    );
    assert!(
        !clicked_the_canvas(&messages),
        "the click went through to the canvas: {messages:?}"
    );
    for message in messages {
        let _ = app.update(message);
    }
    assert!(app.interaction_state.project_tree_context_menu.is_none());
}

/// A right click elsewhere on the canvas closes the open canvas menu and
/// opens a new one where it landed.
#[test]
fn an_outside_right_click_reopens_the_menu_at_the_new_spot() {
    let (mut app, main) = app();
    let (menu, outside) = menu_and_outside(&app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(menu.x, menu.y)));
    assert!(app.interaction_state.context_menu.is_some());

    let pressed = messages_for_events(app.view(main), WINDOW, outside, &[press(Button::Right)]);
    assert!(
        pressed
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::Close))),
        "the right press outside did not close the menu: {pressed:?}"
    );
    for message in pressed {
        let _ = app.update(message);
    }
    assert!(app.interaction_state.context_menu.is_none());

    // With the menu gone the release lands on the canvas, which opens its
    // menu there. That the press itself passes the backdrop is pinned in
    // `context_menu::backdrop::tests`.
    let released = messages_for_events(app.view(main), WINDOW, outside, &[release(Button::Right)]);
    assert!(
        released.iter().any(|m| matches!(
            m,
            Message::ContextMenu(ContextMenuMsg::Show(x, y))
                if (x - outside.x).abs() < 0.5 && (y - outside.y).abs() < 0.5
        )),
        "no menu opened at {outside:?}: {released:?}"
    );
    for message in released {
        let _ = app.update(message);
    }
    let reopened = app
        .interaction_state
        .context_menu
        .as_ref()
        .expect("the menu reopens");
    assert!((reopened.x - outside.x).abs() < 0.5 && (reopened.y - outside.y).abs() < 0.5);
}

/// A right click on the canvas while the Projects menu is open closes that
/// menu too: the press is not held back by the menu's backdrop.
#[test]
fn a_right_press_outside_the_projects_menu_closes_it_and_reaches_the_canvas() {
    let (mut app, main) = app();
    let (menu, outside) = menu_and_outside(&app);
    open_tree_menu(&mut app, menu);
    let pressed = messages_for_events(app.view(main), WINDOW, outside, &[press(Button::Right)]);
    assert!(
        pressed
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::CloseProjectTree))),
        "{pressed:?}"
    );
    for message in pressed {
        let _ = app.update(message);
    }
    let released = messages_for_events(app.view(main), WINDOW, outside, &[release(Button::Right)]);
    assert!(
        released
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::Show(..)))),
        "{released:?}"
    );
}

/// A paused placement leaves the window chrome working: only the banner
/// takes the pointer, so the minimize button still answers.
#[test]
fn a_paused_placement_leaves_the_window_buttons_working() {
    use crate::chrome::controls::WindowControlsPlacement::Trailing;
    let (mut app, main) = app();
    // The placement every platform draws itself: macOS's default leaves the
    // buttons to the OS, so no widget of ours could answer there (#2739).
    app.ui_state.window_controls_placement = Trailing;
    app.ui_state.window_controls_placement_at_boot = Trailing;
    app.interaction_state.active_canvas_mut().placement_paused = true;
    assert!(app.placement_paused_overlay(main).is_some());
    let minimizes = |point: Point| {
        messages_for_events(
            app.view(main),
            WINDOW,
            point,
            &[press(Button::Left), release(Button::Left)],
        )
        .iter()
        .any(|m| {
            matches!(
                m,
                Message::Window(crate::app::WindowMsg::MinimizeMainWindow)
            )
        })
    };
    // The whole bar: the window buttons sit on the left on macOS.
    let along_the_title_bar = (0..(WINDOW.width as u32 / 4)).flat_map(|step| {
        let x = 2.0 + step as f32 * 4.0;
        [8.0, 16.0, 24.0].map(|y| Point::new(x, y))
    });
    assert!(
        along_the_title_bar.into_iter().any(minimizes),
        "no point on the title bar reached the minimize button"
    );
}

/// An open Active Bar dropdown closes on a left click on the canvas beside
/// it, and the canvas never sees that click.
#[test]
fn an_outside_left_click_closes_an_active_bar_dropdown() {
    use crate::active_bar::{ActiveBarMenu, ActiveBarMsg};
    let (mut app, main) = app();
    let canvas = app.canvas_bounds();
    // Below-left of the bar's dropdowns, clear of the bottom-right zoom pill.
    let outside = Point::new(
        canvas.x + canvas.width * 0.2,
        canvas.y + canvas.height * 0.85,
    );
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
        ActiveBarMenu::Align,
    )));
    assert!(app.interaction_state.active_bar_menu.is_some());

    let left_click = [press(Button::Left), release(Button::Left)];
    let messages = messages_for_events(app.view(main), WINDOW, outside, &left_click);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::ActiveBar(ActiveBarMsg::CloseMenus))),
        "the click outside did not close the dropdown: {messages:?}"
    );
    assert!(
        !clicked_the_canvas(&messages),
        "the click went through to the canvas: {messages:?}"
    );
    for message in messages {
        let _ = app.update(message);
    }
    assert!(app.interaction_state.active_bar_menu.is_none());
}

/// The menu's backdrop still stops the wheel beside the card (#562): the
/// scroll guard no longer wraps menu layers, so the backdrop does it.
#[test]
fn the_wheel_beside_an_open_menu_does_not_reach_the_canvas() {
    let (mut app, main) = app();
    let (menu, outside) = menu_and_outside(&app);
    let wheel = |app: &Signex| {
        crate::layout_probe::messages_on_scroll_at(
            app.view(main),
            WINDOW,
            outside,
            iced::mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        )
    };
    let moved_view = |messages: &[Message]| {
        messages
            .iter()
            .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::ViewMoved)))
    };
    let control = wheel(&app);
    assert!(
        moved_view(&control),
        "with no menu the wheel zooms: {control:?}"
    );

    open_tree_menu(&mut app, menu);
    let messages = wheel(&app);
    assert!(
        !moved_view(&messages),
        "the wheel went through: {messages:?}"
    );
}

/// Esc closes each right-click menu, and only the menu.
#[test]
fn esc_closes_an_open_right_click_menu() {
    let esc = || Message::Input(InputMsg::EscapePressed { window: None });

    let (mut app, _main) = app();
    let (menu, _) = menu_and_outside(&app);
    open_tree_menu(&mut app, menu);
    let _ = app.update(esc());
    assert!(app.interaction_state.project_tree_context_menu.is_none());

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(menu.x, menu.y)));
    assert!(app.interaction_state.context_menu.is_some());
    let _ = app.update(esc());
    assert!(app.interaction_state.context_menu.is_none());

    app.interaction_state.last_mouse_pos = (menu.x, menu.y);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowTab(0)));
    assert!(app.interaction_state.tab_context_menu.is_some());
    let _ = app.update(esc());
    assert!(app.interaction_state.tab_context_menu.is_none());
}
