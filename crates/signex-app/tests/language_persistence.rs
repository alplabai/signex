// Standalone binary: the test writes `language: en-XA` into prefs.json and leaves it, which would flip every later boot in a shared process to pseudo-locale.

//! #2055: a language switch is saved and the next boot reads it back, under
//! a config root of its own so no other test sees the saved language.

use signex_app::app::{Message, Signex, UiMsg};
use signex_app::config_root::set_config_root_override;
use signex_app::fonts::prefs_file_path;
use signex_app::task_driver::TaskDriver;
use signex_i18n::LanguagePref;

#[test]
fn a_switched_language_is_saved_and_read_back_at_boot() {
    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");

    let (mut app, _task) = Signex::new();
    let mut driver = TaskDriver::new();
    for tag in ["en-US", "en-XA"] {
        let pref = LanguagePref::Locale(tag.to_owned());
        let task = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
        driver.settle(&mut app, task).expect("the write settled");
    }

    let text = std::fs::read_to_string(prefs_file_path()).expect("prefs.json was written");
    assert!(prefs_file_path().starts_with(isolated.path()));
    let json: serde_json::Value = serde_json::from_str(&text).expect("prefs.json parses");
    assert_eq!(json["language"], "en-XA");

    let (second, _task) = Signex::new();
    assert_eq!(second.ui_state.i18n.locale(), "en-XA");
    assert_eq!(
        second.ui_state.i18n.pref(),
        &LanguagePref::Locale("en-XA".to_owned())
    );
}
