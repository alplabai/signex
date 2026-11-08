//! #2055: switching the language through the one runtime entry changes the
//! File menu title, in the in-window menu model and in the native macOS spec,
//! without a restart.

use signex_i18n::LanguagePref;

use super::*;
use crate::app::UiMsg;
use crate::menu_bar::native::{build, build_for};

const EN_US_FILE: &str = "File";

fn switch(app: &mut Signex, tag: &str) {
    let pref = LanguagePref::Locale(tag.to_owned());
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
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
    switch(&mut app, "en-US");
    assert_eq!(in_window_title(&app), EN_US_FILE);

    switch(&mut app, "en-XA");
    let pseudo = in_window_title(&app);
    assert_ne!(pseudo, EN_US_FILE, "en-XA must change the title");
    assert_eq!(
        native_title(&app),
        pseudo,
        "the native menu reads the same title"
    );

    switch(&mut app, "en-US");
    assert_eq!(in_window_title(&app), EN_US_FILE);
    assert_eq!(native_title(&app), EN_US_FILE);
}

#[test]
fn switching_to_the_language_in_force_keeps_the_same_instance() {
    let (mut app, _task) = Signex::new();
    switch(&mut app, "en-XA");
    let before = std::sync::Arc::clone(&app.ui_state.i18n);
    switch(&mut app, "en-XA");
    assert!(std::sync::Arc::ptr_eq(&before, &app.ui_state.i18n));
}

#[test]
fn a_language_change_makes_the_native_menu_rebuild() {
    let (mut app, _task) = Signex::new();
    switch(&mut app, "en-US");
    let before = app.native_menu_inputs();
    assert_eq!(
        before,
        app.native_menu_inputs(),
        "the same locale is the same input"
    );

    switch(&mut app, "en-XA");
    assert_ne!(before, app.native_menu_inputs());
}

/// The path the macOS sync takes: Model -> `native_menu_inputs` -> `build_for`.
#[test]
fn a_switched_language_reaches_the_native_spec_through_the_sync_path() {
    let (mut app, _task) = Signex::new();
    switch(&mut app, "en-US");
    assert_eq!(
        build_for(&app.native_menu_inputs()).spec.menus[1].title,
        EN_US_FILE
    );

    switch(&mut app, "en-XA");
    let title = build_for(&app.native_menu_inputs()).spec.menus[1]
        .title
        .clone();
    assert_ne!(title, EN_US_FILE);
    assert_eq!(title, in_window_title(&app));
}
