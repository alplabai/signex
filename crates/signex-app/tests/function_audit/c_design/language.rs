//! Row 144a: Preferences > Language. The choice is a draft with a preview;
//! Apply or OK switches the interface with no restart, Cancel drops it.

use signex_app::preferences::{PrefMsg, PrefNav};
use signex_i18n::LanguagePref;

use super::support::{Prefs, opened};

fn pseudo() -> LanguagePref {
    LanguagePref::Locale("en-XA".to_string())
}

fn english() -> LanguagePref {
    LanguagePref::Locale("en-US".to_string())
}

fn preview_locale(prefs: &Prefs) -> &'static str {
    prefs
        .app
        .ui_state
        .preferences_pages
        .language_preview
        .as_ref()
        .expect("the preview catalog")
        .locale()
}

#[test]
fn picking_a_language_redraws_the_preview_and_leaves_the_interface_alone() {
    let mut prefs = opened();
    let before = prefs.app.ui_state.i18n.pref().clone();

    prefs.send(PrefMsg::DraftLanguage(pseudo()));

    assert_eq!(preview_locale(&prefs), "en-XA");
    assert_eq!(prefs.app.ui_state.i18n.pref(), &before);
    assert_eq!(prefs.dirty_pages(), [PrefNav::Language]);
}

#[test]
fn apply_switches_the_language_with_no_restart_and_keeps_the_dialog_open() {
    let mut prefs = opened();
    prefs.send(PrefMsg::DraftLanguage(pseudo()));

    prefs.apply();

    assert_eq!(prefs.app.ui_state.i18n.pref(), &pseudo());
    assert_eq!(prefs.app.ui_state.i18n.locale(), "en-XA");
    assert!(prefs.is_open());
    assert!(!prefs.is_dirty());
}

#[test]
fn ok_switches_the_language_and_closes_the_dialog() {
    let mut prefs = opened();
    prefs.send(PrefMsg::DraftLanguage(pseudo()));

    prefs.ok();

    assert_eq!(prefs.app.ui_state.i18n.locale(), "en-XA");
    assert!(!prefs.is_open());
}

#[test]
fn cancel_drops_the_language_and_its_preview() {
    let mut prefs = opened();
    let before = prefs.app.ui_state.i18n.pref().clone();
    let locale = prefs.app.ui_state.i18n.locale();
    prefs.send(PrefMsg::DraftLanguage(pseudo()));

    prefs.cancel();

    assert_eq!(prefs.app.ui_state.i18n.pref(), &before);
    assert_eq!(prefs.app.ui_state.preferences_pages.draft_language, before);
    assert_eq!(preview_locale(&prefs), locale);
    assert!(!prefs.is_dirty());
}

#[test]
fn follow_the_system_can_be_chosen_after_a_fixed_language() {
    let mut prefs = opened();
    prefs.send(PrefMsg::DraftLanguage(english()));
    prefs.apply();
    assert_eq!(prefs.app.ui_state.i18n.pref(), &english());

    prefs.send(PrefMsg::DraftLanguage(LanguagePref::System));
    assert!(prefs.is_dirty());
    prefs.apply();

    assert_eq!(prefs.app.ui_state.i18n.pref(), &LanguagePref::System);
    assert!(!prefs.is_dirty());
}

#[test]
fn the_follow_the_system_row_names_the_language_it_resolves_to() {
    let prefs = opened();

    assert!(
        !prefs
            .app
            .ui_state
            .preferences_pages
            .system_language_name
            .is_empty()
    );
}
