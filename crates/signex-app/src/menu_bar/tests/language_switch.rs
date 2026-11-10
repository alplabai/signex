//! #2055: switching the language through the one runtime entry changes the
//! File menu title, in the in-window menu model and in the native macOS spec,
//! without a restart.

use signex_i18n::LanguagePref;

use super::*;
use crate::app::UiMsg;
use crate::menu_bar::native::{build, probe::root_titles};

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
    root_titles(&build(&model, false))[1].clone()
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

/// The path the macOS bar takes: Model -> `Signex::native_menu`, which the
/// iced runtime calls after every update.
#[test]
fn a_switched_language_reaches_the_native_menu_through_the_runtime_path() {
    let (mut app, _task) = Signex::new();
    switch(&mut app, "en-US");
    assert_eq!(root_titles(&app.native_menu())[1], EN_US_FILE);

    switch(&mut app, "en-XA");
    let title = root_titles(&app.native_menu())[1].clone();
    assert_ne!(title, EN_US_FILE);
    assert_eq!(title, in_window_title(&app));
}
