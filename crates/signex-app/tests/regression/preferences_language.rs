//! #144a — the Preferences Language page. The chosen language is a draft:
//! nothing changes until Apply or OK, which switch every window with no
//! restart; Cancel drops it. Only the Language page is marked dirty.
//!
//! The tests never drive the tasks Save returns: one of them writes the
//! language into the shared test prefs file, and every later
//! `Signex::new()` in this binary would boot in it.

use signex_app::app::{Message, PreferencesMsg, Signex, UiMsg};
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_i18n::LanguagePref;

use crate::preferences_prefs_recovery::{PrefsPathGuard, serial};

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn pseudo() -> LanguagePref {
    LanguagePref::Locale("en-XA".to_owned())
}

/// Open Preferences on an app whose language is not the pseudo-locale.
fn open() -> Signex {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert_ne!(
        app.ui_state.i18n.pref(),
        &pseudo(),
        "fixture precondition: the app does not boot in the pseudo-locale"
    );
    app
}

#[test]
fn a_drafted_language_is_not_applied_before_save() {
    // Arrange
    let mut app = open();
    let before = app.ui_state.i18n.pref().clone();

    // Act
    let _ = app.update(inner(PrefMsg::DraftLanguage(pseudo())));

    // Assert
    assert_eq!(app.ui_state.i18n.pref(), &before, "not applied before Save");
    assert_eq!(app.ui_state.preferences_pages.draft_language, pseudo());
    assert!(
        app.ui_state.preferences_dirty,
        "a drafted language is unsaved"
    );
    let preview = app
        .ui_state
        .preferences_pages
        .language_preview
        .as_ref()
        .expect("the preview catalog is built in update");
    assert_eq!(preview.locale(), "en-XA", "the preview reads the draft");
}

#[test]
fn save_switches_the_language_without_a_restart() {
    // Arrange: Save also writes the keymap file, which the guard restores.
    let _serial = serial();
    let _guard = PrefsPathGuard::capture();
    let mut app = open();
    let _ = app.update(inner(PrefMsg::DraftLanguage(pseudo())));

    // Act
    let _ = app.update(inner(PrefMsg::Save));

    // Assert
    assert_eq!(app.ui_state.i18n.pref(), &pseudo());
    assert_eq!(
        app.ui_state.i18n.locale(),
        "en-XA",
        "the live catalog switched"
    );
    assert!(!app.ui_state.preferences_dirty);
    assert!(app.ui_state.preferences_pages.dirty_pages.is_empty());
    assert!(app.ui_state.preferences_open, "Apply keeps the dialog open");
}

#[test]
fn cancel_drops_the_drafted_language() {
    // Arrange
    let mut app = open();
    let before = app.ui_state.i18n.pref().clone();
    let _ = app.update(inner(PrefMsg::DraftLanguage(pseudo())));

    // Act
    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    // Assert
    assert_eq!(app.ui_state.i18n.pref(), &before);
    assert_eq!(app.ui_state.preferences_pages.draft_language, before);
    assert!(!app.ui_state.preferences_dirty);
    assert!(app.ui_state.preferences_pages.dirty_pages.is_empty());
}

#[test]
fn a_drafted_language_marks_only_the_language_page() {
    // Arrange
    let mut app = open();

    // Act
    let _ = app.update(inner(PrefMsg::DraftLanguage(pseudo())));

    // Assert
    assert_eq!(
        app.ui_state.preferences_pages.dirty_pages,
        [PrefNav::Language]
    );
}

#[test]
fn drafting_the_language_in_force_again_is_clean() {
    // Arrange
    let mut app = open();
    let before = app.ui_state.i18n.pref().clone();
    let _ = app.update(inner(PrefMsg::DraftLanguage(pseudo())));

    // Act
    let _ = app.update(inner(PrefMsg::DraftLanguage(before)));

    // Assert
    assert!(!app.ui_state.preferences_dirty);
    assert!(app.ui_state.preferences_pages.dirty_pages.is_empty());
}

/// A language switched from the palette or the menu while Preferences is
/// open is not an unsaved Language change: the draft follows it.
#[test]
fn a_palette_switch_while_open_keeps_the_language_page_clean() {
    // Arrange
    let mut app = open();

    // Act
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pseudo())));

    // Assert
    assert_eq!(app.ui_state.preferences_pages.draft_language, pseudo());
    assert!(
        !app.ui_state
            .preferences_pages
            .dirty_pages
            .contains(&PrefNav::Language),
        "{:?}",
        app.ui_state.preferences_pages.dirty_pages
    );
}

/// The first row of the language list names what the system resolves to.
#[test]
fn opening_preferences_reads_the_system_language_name() {
    let app = open();
    assert!(
        !app.ui_state
            .preferences_pages
            .system_language_name
            .is_empty(),
        "the Follow the system row needs a language name"
    );
}
