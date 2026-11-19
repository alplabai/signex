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

    // GG-10: the navigation has no Open Sample Project row in either form.
    assert!(
        wide.iter().all(|t| t.text != "Open Sample Project"),
        "{wide:#?}"
    );
    assert!(
        wide.iter().any(|t| t.text == "Keyboard Shortcuts"),
        "{wide:#?}"
    );

    let narrow = lay_out_texts(app.view(window), NARROW);
    assert!(narrow.iter().any(|t| t.text == HERO), "{narrow:#?}");
    for label in ["Parts search", "Keyboard Shortcuts"] {
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

/// m1: the Projects row drives `Signex::update` to the Projects page, whose
/// filter and switch are kept in the model; Home's hero is gone meanwhile.
#[test]
fn the_projects_row_opens_the_projects_page_through_update() {
    use crate::start_screen::{ProjectScope, StartPage, StartScreenMsg};
    let (mut app, window) = start_screen_app();
    let page = crate::layout_probe::messages_on_click(app.view(window), WIDE, "Projects")
        .into_iter()
        .find(|m| {
            matches!(
                m,
                Message::Project(crate::app::ProjectMsg::StartScreen(
                    StartScreenMsg::PageSelected(StartPage::Projects)
                ))
            )
        })
        .expect("the Projects row opens its page");
    let _ = app.update(page);
    assert_eq!(app.ui_state.start_screen.page, StartPage::Projects);
    let texts = lay_out_texts(app.view(window), WIDE);
    assert!(texts.iter().all(|t| t.text != HERO), "{texts:#?}");

    for msg in [
        StartScreenMsg::ProjectFilterChanged("motor".to_owned()),
        StartScreenMsg::ProjectScopePicked(ProjectScope::Pinned),
    ] {
        let _ = app.update(crate::start_screen::message(msg));
    }
    assert_eq!(app.ui_state.start_screen.project_filter, "motor");
    assert_eq!(
        app.ui_state.start_screen.project_scope,
        ProjectScope::Pinned
    );
    let _ = app.update(crate::start_screen::message(StartScreenMsg::FilterCleared));
    assert!(app.ui_state.start_screen.project_filter.is_empty());
    assert_eq!(
        app.ui_state.start_screen.project_scope,
        ProjectScope::Pinned,
        "Clear filter clears the text only"
    );
}
