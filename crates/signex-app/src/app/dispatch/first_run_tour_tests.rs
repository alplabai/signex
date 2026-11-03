//! The first-run tour card: shown once, closed by ✕, Esc or a deliberate
//! pointer gesture on a canvas, never by a hover or a key's command (#1994).

use super::super::*;
use crate::canvas::{BoxDirection, CanvasEvent, MoveConnections};

fn app_with_tour() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = true;
    app
}

fn has_work(task: &Task<Message>) -> bool {
    task.units() > 0
}

fn gestures() -> Vec<CanvasEvent> {
    vec![
        CanvasEvent::Clicked {
            world_x: 1.0,
            world_y: 1.0,
        },
        CanvasEvent::CtrlClicked {
            world_x: 1.0,
            world_y: 1.0,
        },
        CanvasEvent::DoubleClicked {
            world_x: 1.0,
            world_y: 1.0,
            screen_x: 1.0,
            screen_y: 1.0,
        },
        CanvasEvent::BoxSelect {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
            direction: BoxDirection::LeftToRight,
        },
        CanvasEvent::MoveSelected {
            dx: 1.0,
            dy: 1.0,
            connections: MoveConnections::Preference,
        },
        CanvasEvent::MovePicked {
            world_x: 1.0,
            world_y: 1.0,
        },
        CanvasEvent::ViewMoved,
    ]
}

fn not_gestures() -> Vec<CanvasEvent> {
    vec![
        CanvasEvent::CursorAt {
            x: 1.0,
            y: 1.0,
            zoom_pct: 100.0,
        },
        CanvasEvent::CursorMoved,
        CanvasEvent::FitAll,
        CanvasEvent::FitSelection,
        CanvasEvent::CenterAtCursor,
    ]
}

#[test]
fn a_pointer_gesture_closes_the_card_and_saves_the_choice() {
    for event in gestures() {
        assert!(event.is_user_gesture(), "{event:?}");
        let mut app = app_with_tour();
        let task = app.dismiss_first_run_tour_on(&event);
        assert!(!app.ui_state.first_run_tour_open, "{event:?} left it open");
        assert!(has_work(&task), "{event:?} saved nothing");

        let mut app = app_with_tour();
        let _ = app.update(Message::CanvasEvent(event.clone()));
        assert!(!app.ui_state.first_run_tour_open, "{event:?}");
        let mut app = app_with_tour();
        let _ = app.update(Message::CanvasEventInWindow {
            window_id: iced::window::Id::unique(),
            event: event.clone(),
        });
        assert!(!app.ui_state.first_run_tour_open, "in a window: {event:?}");
    }
}

#[test]
fn a_hover_or_a_fit_command_leaves_the_card_open_and_saves_nothing() {
    for event in not_gestures() {
        assert!(!event.is_user_gesture(), "{event:?}");
        let mut app = app_with_tour();
        let task = app.dismiss_first_run_tour_on(&event);
        assert!(app.ui_state.first_run_tour_open, "{event:?} closed it");
        assert!(!has_work(&task), "{event:?} saved a pref");

        let _ = app.update(Message::CanvasEvent(event.clone()));
        assert!(app.ui_state.first_run_tour_open, "{event:?}");
        let _ = app.update(Message::CanvasEventInWindow {
            window_id: iced::window::Id::unique(),
            event,
        });
        assert!(app.ui_state.first_run_tour_open);
    }
}

#[test]
fn the_home_key_and_the_menu_fit_all_leave_the_card_open() {
    let id = crate::keymap::AppCommandId::new("zoom_to_fit").unwrap();
    let mut app = app_with_tour();
    let _ = app.dispatch_command(&id, crate::app::command::CommandArgs::none(), None);
    assert!(app.ui_state.first_run_tour_open, "the Home key closed it");

    let mut app = app_with_tour();
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::Command(id)));
    assert!(app.ui_state.first_run_tour_open, "the menu closed it");
}

#[test]
fn a_right_click_on_the_canvas_closes_the_card() {
    let mut app = app_with_tour();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(10.0, 10.0)));
    assert!(!app.ui_state.first_run_tour_open);
}

#[test]
fn the_close_button_and_escape_still_close_it_and_save() {
    let mut app = app_with_tour();
    let task = app.update(Message::Overlay(OverlayMsg::DismissFirstRunTour));
    assert!(!app.ui_state.first_run_tour_open);
    assert!(has_work(&task), "the close button saved nothing");

    let mut app = app_with_tour();
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(!app.ui_state.first_run_tour_open, "Esc left it open");
}

#[test]
fn a_launch_with_no_pref_opens_the_card_and_saves_that_it_was_shown() {
    let (open, task) = crate::app::bootstrap::first_run_tour_boot(None);
    assert!(open);
    assert!(has_work(&task), "the pref write is not scheduled");

    let prefs = serde_json::json!({ "other": 1 });
    let (open, task) = crate::app::bootstrap::first_run_tour_boot(Some(&prefs));
    assert!(open && has_work(&task));
}

/// The boot write is built when the runtime runs the task, so constructing the
/// app (as every test does) leaves the in-flight write count alone; a raised
/// count would make the quit path wait out its whole limit.
#[test]
fn building_the_app_takes_no_preference_write_slot() {
    let before = crate::fonts::pref_writes_in_flight();
    let (_app, _task) = Signex::new();
    let (_open, _task) = crate::app::bootstrap::first_run_tour_boot(None);
    assert_eq!(crate::fonts::pref_writes_in_flight(), before);
}

#[test]
fn a_launch_with_the_pref_set_does_not_open_the_card() {
    let prefs = serde_json::json!({ "first_run_tour_dismissed": true });
    let (open, task) = crate::app::bootstrap::first_run_tour_boot(Some(&prefs));
    assert!(!open);
    assert!(!has_work(&task));
}
