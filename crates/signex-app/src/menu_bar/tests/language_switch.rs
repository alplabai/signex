//! #2055: the Language setting changes the File menu title, in the in-window
//! menu model and in the native macOS spec, without a restart.

use signex_i18n::LanguagePref;

use super::*;
use crate::app::PreferencesMsg;
use crate::menu_bar::native::{build, build_for};
use crate::preferences::PrefMsg;

const EN_US_FILE: &str = "File";

fn pick_and_save(app: &mut Signex, tag: &str) {
    let draft = Message::Preferences(PreferencesMsg::Inner(PrefMsg::DraftLanguage(
        LanguagePref::Locale(tag.to_owned()),
    )));
    let _ = app.update(draft);
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(PrefMsg::Save)));
}

fn in_window_title(app: &Signex) -> String {
    menu_bar_model(&menu_context(app)).menus[0].title.clone()
}

fn native_title(app: &Signex) -> String {
    let model = menu_bar_model(&menu_context(app));
    // Menu 0 is the app menu; File follows it.
    build(&model, false).spec.menus[1].title.clone()
}

#[test]
fn the_file_title_follows_the_language_without_a_restart() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    pick_and_save(&mut app, "en-US");
    assert_eq!(in_window_title(&app), EN_US_FILE);

    pick_and_save(&mut app, "en-XA");
    let pseudo = in_window_title(&app);
    assert_ne!(pseudo, EN_US_FILE, "en-XA must change the title");
    assert_eq!(
        native_title(&app),
        pseudo,
        "the native menu reads the same title"
    );

    pick_and_save(&mut app, "en-US");
    assert_eq!(in_window_title(&app), EN_US_FILE);
    assert_eq!(native_title(&app), EN_US_FILE);
}

#[test]
fn a_committed_language_is_not_dirty_and_a_draft_is() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    pick_and_save(&mut app, "en-US");
    assert!(!app.ui_state.preferences_draft_differs());

    let draft = PrefMsg::DraftLanguage(LanguagePref::Locale("en-XA".into()));
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(draft)));
    assert!(app.ui_state.preferences_draft_differs());
    assert_eq!(in_window_title(&app), EN_US_FILE, "a draft must not commit");
}

#[test]
fn a_language_change_makes_the_native_menu_rebuild() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    pick_and_save(&mut app, "en-US");
    let inputs = |app: &Signex| app.native_menu_inputs();
    let before = inputs(&app);
    assert_eq!(before, inputs(&app), "the same locale is the same input");

    pick_and_save(&mut app, "en-XA");
    assert_ne!(before, inputs(&app));
}

/// The path the macOS sync takes: Model -> `native_menu_inputs` -> `build_for`.
#[test]
fn a_switched_language_reaches_the_native_spec_through_the_sync_path() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    pick_and_save(&mut app, "en-US");
    assert_eq!(
        build_for(&app.native_menu_inputs()).spec.menus[1].title,
        EN_US_FILE
    );

    pick_and_save(&mut app, "en-XA");
    let title = build_for(&app.native_menu_inputs()).spec.menus[1]
        .title
        .clone();
    assert_ne!(title, EN_US_FILE);
    assert_eq!(title, in_window_title(&app));
}
