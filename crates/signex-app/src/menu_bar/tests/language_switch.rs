//! #2055: the Language setting changes the File menu title, in the in-window
//! menu model and in the native macOS spec, without a restart.

use std::sync::Arc;

use signex_i18n::LanguagePref;

use super::*;
use crate::app::PreferencesMsg;
use crate::menu_bar::native::{MenuInputs, build};
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
    let inputs = |app: &Signex| {
        MenuInputs::new(
            app.command_context(None),
            app.ui_state.recent_projects.entries(),
            false,
            Vec::new(),
        )
        .with_i18n(Arc::clone(&app.ui_state.i18n))
    };
    let before = inputs(&app);
    assert_eq!(before, inputs(&app), "the same locale is the same input");

    pick_and_save(&mut app, "en-XA");
    assert_ne!(before, inputs(&app));
}
