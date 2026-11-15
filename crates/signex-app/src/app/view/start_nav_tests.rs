//! #2346 — the Home screen navigation in the running app's view: an icon
//! strip in a narrow window, the full column in a wide one, and a strip row
//! that drives `Signex::update` like its full row.

use super::*;
use crate::layout_probe::{find, lay_out_texts, messages_on_click_at};

const NARROW: iced::Size = iced::Size::new(700.0, 500.0);
const WIDE: iced::Size = iced::Size::new(1400.0, 900.0);
const HERO: &str = "What are you building?";

/// A Graphite start-screen app with the first-run tour closed.
fn start_screen_app() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main_window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main_window);
    assert!(app.document_state.tabs.is_empty());
    (app, main_window)
}

#[test]
fn the_home_navigation_is_a_strip_when_narrow_and_full_when_wide() {
    let (app, window) = start_screen_app();
    let wide = lay_out_texts(app.view(window), WIDE);
    assert!(wide.iter().any(|t| t.text == "Parts search"), "{wide:#?}");

    let narrow = lay_out_texts(app.view(window), NARROW);
    assert!(narrow.iter().any(|t| t.text == HERO), "{narrow:#?}");
    for label in [
        "Parts search",
        "Open Sample Project",
        "Keyboard Shortcuts (F1)",
    ] {
        assert!(
            narrow.iter().all(|t| t.text != label),
            "{label:?} is drawn in a {NARROW:?} window"
        );
    }
}

#[test]
fn a_strip_row_opens_keyboard_shortcuts_through_update() {
    let (mut app, window) = start_screen_app();
    let texts = lay_out_texts(app.view(window), NARROW);
    let hero = find(&texts, HERO).bounds;
    let x = crate::start_screen::strip_centre_from_hero(hero.x);
    let step = crate::styles::Spacing::DEFAULT.m;
    let shortcuts = (0..)
        .map(|i| i as f32 * step)
        .take_while(|y| *y < NARROW.height)
        .flat_map(|y| messages_on_click_at(app.view(window), NARROW, None, iced::Point::new(x, y)))
        .find(|m| {
            matches!(
                m,
                Message::Menu(crate::menu_bar::MenuMessage::OpenKeyboardShortcuts)
            )
        })
        .expect("no strip row sends Keyboard Shortcuts");
    assert!(!app.ui_state.keyboard_shortcuts_open);
    let _ = app.update(shortcuts);
    assert!(app.ui_state.keyboard_shortcuts_open);
}
